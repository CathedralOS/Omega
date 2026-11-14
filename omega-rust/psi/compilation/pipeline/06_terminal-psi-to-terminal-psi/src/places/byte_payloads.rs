//! A write through a byte view changes the storage it points to. Invalidate
//! facts about those bytes while keeping the view's address and length separate.
//! Authenticate the source write log before adding these derived effects.

use super::{Lineage, Logged, Shape, Storage, Stored};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{ByteSequenceCarrier, OperationKind, StructuralAccess, StructuralTypeShape};
use terminal_semantics::descriptor_lifetimes::{descriptor_location_type, descriptor_path};
use terminal_semantics::descriptor_results::{
    DescriptorCallOrigin, DescriptorSlotKind, DescriptorSourceOrigin,
};

impl Storage<'_> {
    pub(super) fn byte_payload_writes(&mut self) -> Checked<()> {
        let mut writes = Vec::new();
        for block in &self.lowered.blocks {
            for (position, operation) in block.operations.iter().enumerate() {
                let concerns = Construct::Operation(operation.id);
                let mut descriptors = Vec::new();
                if let OperationKind::ByteSequenceWrite { destination, .. } = operation.kind {
                    let destination = self.stored_at(destination, &[], concerns)?;
                    descriptors = self.byte_payload_descriptors(&destination, concerns)?;
                } else if operation.kind.declared_invocation().is_some() {
                    // Most calls have no mutable byte descriptor. Inventory
                    // actual operands once before requesting their frame.
                    for argument in operation
                        .kind
                        .structural_arguments()
                        .iter()
                        .filter(|argument| argument.access != StructuralAccess::SharedBorrow)
                    {
                        let actual = self.argument(argument, concerns)?;
                        descriptors.extend(self.byte_payload_descriptors(&actual, concerns)?);
                    }
                    if descriptors.is_empty() {
                        continue;
                    }
                    if let Some(regions) = terminal_semantics::borrowed_storage::call_write_regions(
                        self.module,
                        self.lowered,
                        operation,
                    )
                    .map_err(|detail| self.foreign(concerns, &detail))?
                    {
                        let affected = regions
                            .into_iter()
                            .map(|region| {
                                let root = region.root.ok_or_else(|| {
                                    self.foreign(
                                        concerns,
                                        "payload write frame lost its actual root",
                                    )
                                })?;
                                self.stored_at(root, &region.projection.path, concerns)
                            })
                            .collect::<Checked<Vec<_>>>()?;
                        descriptors.retain(|descriptor| {
                            affected
                                .iter()
                                .any(|region| super::overlap(region, descriptor))
                        });
                    }
                } else {
                    continue;
                }
                let cut = u32::try_from(position)
                    .map_err(|_| self.foreign(concerns, "payload write cut exceeds u32"))?;
                for descriptor in descriptors {
                    for backing in
                        self.byte_payload_backings(&descriptor, (block.id, cut), concerns)?
                    {
                        writes.push(Logged {
                            captured: descriptor.clone(),
                            lineage: backing,
                            block: block.id,
                            position: position + 1,
                            operation: operation.id,
                            stored: Stored::Established,
                        });
                    }
                }
            }
        }
        if writes.is_empty() {
            return Ok(());
        }
        self.log.extend(writes);
        // Reaching walks choose the latest write in reverse operation order.
        self.log.sort_by_key(|row| (row.block, row.position));
        Ok(())
    }

    fn byte_payload_descriptors(
        &self,
        region: &Lineage,
        concerns: Construct,
    ) -> Checked<Vec<Lineage>> {
        let slots = match self.shape(region, concerns)? {
            Shape::Typed(row) => DescriptorSlotKind::Views
                .slots(self.types, region.root, row)
                .ok_or_else(|| {
                    self.unsupported(
                        concerns,
                        "payload write has no complete descriptor inventory",
                    )
                })?,
            Shape::View => return Ok(vec![region.clone()]),
            Shape::Scalar(_) | Shape::Payload(_) => return Ok(Vec::new()),
        };
        if slots.is_empty() {
            return Ok(Vec::new());
        }
        let prefix = region.descriptor_location().ok_or_else(|| {
            self.unsupported(concerns, "payload write has no captured descriptor path")
        })?;
        let root = self.root_type(region.root, concerns)?;
        let mut descriptors = Vec::new();
        for mut slot in slots {
            let mut path = prefix.path.clone();
            path.append(&mut slot.path);
            let path = descriptor_path(self.types, root, &path)
                .ok_or_else(|| self.foreign(concerns, "payload descriptor lost its typed path"))?;
            let descriptor = self.stored_at(region.root, &path, concerns)?;
            if matches!(
                self.byte_shape(region.root, &path, concerns)?,
                StructuralTypeShape::ByteSequence(
                    ByteSequenceCarrier::BorrowedView {
                        access: None | Some(StructuralAccess::MutableBorrow)
                    } | ByteSequenceCarrier::BoundedView {
                        access: None | Some(StructuralAccess::MutableBorrow),
                        ..
                    }
                )
            ) {
                descriptors.push(descriptor);
            }
        }
        Ok(descriptors)
    }

    fn byte_payload_backings(
        &self,
        descriptor: &Lineage,
        at: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
        concerns: Construct,
    ) -> Checked<Vec<Lineage>> {
        let location = descriptor.descriptor_location().ok_or_else(|| {
            self.unsupported(concerns, "byte payload has no exact descriptor location")
        })?;
        let origins = self
            .references
            .borrow_mut()
            .symbolic_source_origins(self.machine, at, &location)
            .filter(|origins| !origins.is_empty())
            .ok_or_else(|| {
                self.unsupported(
                    concerns,
                    "byte payload write has no captured backing origins",
                )
            })?;
        let mut backings = Vec::new();
        for origin in origins {
            match origin {
                DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                    source,
                    projection,
                }) => {
                    let enclosing = projection.enclosing_storage().ok_or_else(|| {
                        self.unsupported(
                            concerns,
                            "byte payload write has an unresolved indirect backing",
                        )
                    })?;
                    let backing = self.stored_at(source, &enclosing.path, concerns)?;
                    // An incoming descriptor names external payload, not an
                    // inline array in this activation. Never overwrite its
                    // own descriptor or retire its independent extent.
                    if let Shape::Typed(row) = self.shape(&backing, concerns)? {
                        let shape = &self.declaration(row, concerns)?.shape;
                        if shape.is_borrowed_byte_carrier() {
                            continue;
                        }
                        if matches!(
                            shape,
                            StructuralTypeShape::ByteSequence(
                                ByteSequenceCarrier::BoundedOwned { .. }
                            )
                        ) {
                            return Err(self.unsupported(concerns,
                                "byte payload writes into owned byte storage need their resident payload and length distinction"));
                        }
                    }
                    if !backings.contains(&backing) {
                        backings.push(backing);
                    }
                }
                DescriptorSourceOrigin::IncomingSlot(slot) => {
                    let row = descriptor_location_type(
                        self.types,
                        self.root_type(slot.root, concerns)?,
                        &slot.path,
                    )
                    .ok_or_else(|| {
                        self.foreign(concerns, "incoming payload lost its descriptor type")
                    })?;
                    if !self
                        .declaration(row, concerns)?
                        .shape
                        .is_borrowed_byte_carrier()
                    {
                        return Err(self.unsupported(
                            concerns,
                            "incoming payload is not an authenticated byte descriptor",
                        ));
                    }
                }
                DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Empty) => {}
                _ => {
                    return Err(self.unsupported(
                        concerns,
                        "byte payload write has unserved captured backing correspondence",
                    ));
                }
            }
        }
        Ok(backings)
    }
}
