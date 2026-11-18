//! A write through a byte view changes the storage it points to. Invalidate
//! facts about those bytes while keeping the view's address and length separate.
//! Authenticate the source write log before adding these derived effects.

use super::{Backing, Lineage, Logged, Shape, Storage, Stored};
use std::collections::{BTreeMap, BTreeSet};
use terminal_check_tables::{Checked, Construct, MachineTables};
use terminal_psi::{ByteSequenceCarrier, OperationKind, StructuralAccess, StructuralTypeShape};
use terminal_semantics::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPathSegment, DescriptorPosition, descriptor_location_type,
    descriptor_path,
};
use terminal_semantics::descriptor_results::{
    DescriptorCallOrigin, DescriptorSlotKind, DescriptorSourceOrigin, UnknownDescriptorContents,
};

#[derive(Default)]
struct PayloadInventory {
    descriptors: Vec<Lineage>,
    unknown_contents: Vec<PendingPayloadWrite>,
}

/// Keep the exact unenumerated slot separate from its containing storage.
/// It names pending contents, not an address or permission to that contents.
struct PendingPayloadWrite {
    carrier: Lineage,
    contents: UnknownDescriptorContents,
}

struct PayloadWrite {
    at: DescriptorPosition,
    operation: semantic_vocabulary::OperationId,
    inventory: PayloadInventory,
}

impl<'a> Storage<'a> {
    pub(super) fn byte_payload_writes(
        &mut self,
        references: &mut super::DescriptorAnalysis<'a>,
        tables: &MachineTables,
    ) -> Checked<()> {
        let mut pending = Vec::new();
        for block in &self.lowered.blocks {
            for (position, operation) in block.operations.iter().enumerate() {
                let concerns = Construct::Operation(operation.id);
                let mut inventory = PayloadInventory::default();
                if let OperationKind::ByteSequenceWrite { destination, .. } = operation.kind {
                    let destination = self.stored_at(destination, &[], concerns)?;
                    inventory = self.byte_payload_descriptors(&destination, concerns)?;
                } else if crate::call_contracts::operation_inputs(operation).is_some() {
                    // Most calls have no mutable byte descriptor. Inventory
                    // actual operands once before requesting their frame.
                    for argument in operation
                        .kind
                        .structural_arguments()
                        .iter()
                        .filter(|argument| argument.access != StructuralAccess::SharedBorrow)
                    {
                        let actual = self.argument(argument, concerns)?;
                        let actual = self.byte_payload_descriptors(&actual, concerns)?;
                        inventory.descriptors.extend(actual.descriptors);
                        inventory.unknown_contents.extend(actual.unknown_contents);
                    }
                    if inventory.descriptors.is_empty() && inventory.unknown_contents.is_empty() {
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
                        inventory.unknown_contents.retain(|pending| {
                            affected
                                .iter()
                                .any(|region| super::overlap(region, &pending.carrier))
                        });
                        inventory.descriptors.retain(|descriptor| {
                            affected
                                .iter()
                                .any(|region| super::overlap(region, descriptor))
                        });
                    }
                } else {
                    continue;
                }
                if inventory.descriptors.is_empty() && inventory.unknown_contents.is_empty() {
                    continue;
                }
                let cut = u32::try_from(position)
                    .map_err(|_| self.foreign(concerns, "payload write cut exceeds u32"))?;
                pending.push(PayloadWrite {
                    at: (block.id, cut),
                    operation: operation.id,
                    inventory,
                });
            }
        }
        let targets = pending
            .iter()
            .filter(|write| !write.inventory.unknown_contents.is_empty())
            .map(|write| write.at)
            .collect::<BTreeSet<_>>();
        let possible = if targets.is_empty() {
            BTreeMap::new()
        } else {
            self.escaping_payload_backings(references, tables, &targets)?
        };
        let mut writes = Vec::new();
        for write in pending {
            let concerns = Construct::Operation(write.operation);
            let (block, position) = write.at;
            let before = position as usize;
            let aliases: &[Lineage] = if write.inventory.unknown_contents.is_empty() {
                &[]
            } else {
                possible.get(&write.at).ok_or_else(|| {
                    self.unsupported(
                        concerns,
                        "pending payload effect has no prepared backing account",
                    )
                })?
            };
            for pending in write.inventory.unknown_contents {
                let receiving =
                    self.backing(references, &pending.carrier, block, before, concerns)?;
                for lineage in aliases {
                    writes.push(Logged {
                        destination: pending.carrier.clone(),
                        backing: Backing::direct(lineage.clone()),
                        block,
                        position: before + 1,
                        operation: write.operation,
                        stored: Stored::UnknownPayloadWrite {
                            receiving: receiving.clone(),
                            contents: pending.contents.clone(),
                        },
                    });
                }
            }
            for descriptor in write.inventory.descriptors {
                for backing in self.byte_payload_backings(
                    references,
                    &descriptor,
                    (block, position),
                    concerns,
                )? {
                    writes.push(Logged {
                        destination: descriptor.clone(),
                        backing: Backing::direct(backing),
                        block,
                        position: before + 1,
                        operation: write.operation,
                        stored: Stored::Established,
                    });
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

    fn escaping_payload_backings(
        &self,
        references: &mut super::DescriptorAnalysis<'a>,
        tables: &MachineTables,
        targets: &BTreeSet<DescriptorPosition>,
    ) -> Checked<BTreeMap<DescriptorPosition, Vec<Lineage>>> {
        let concerns = Construct::Machine;
        let flow = references
            .flow(self.machine)
            .map_err(|error| self.unsupported(concerns, &error.in_module(self.module)))?;
        let mut predecessors = BTreeMap::<_, Vec<_>>::new();
        for (position, step) in &flow {
            for edge in &step.successors {
                predecessors.entry(edge.target).or_default().push(*position);
            }
        }
        let mut relevant = BTreeSet::new();
        let mut pending = targets.iter().copied().collect::<Vec<_>>();
        while let Some(position) = pending.pop() {
            if relevant.insert(position) {
                pending.extend(predecessors.get(&position).into_iter().flatten().copied());
            }
        }
        let mut incoming = Vec::new();
        let entry = (self.graph.entry(), 0);
        for parameter in &self.lowered.structural_parameters {
            if parameter.access != StructuralAccess::Owned
                && !terminal_semantics::borrowed_view_payload_parameter(
                    parameter,
                    self.types.iter(),
                )
            {
                incoming.push(self.stored_at(parameter.place, &[], concerns)?);
            }
            let inventory = DescriptorSlotKind::References
                .owned_inventory(self.types, parameter.place, parameter.structural_type)
                .ok_or_else(|| {
                    self.unsupported(
                        concerns,
                        "pending payload effect has no incoming reference inventory",
                    )
                })?;
            for mut slot in inventory.slots {
                let Some(ty) =
                    descriptor_location_type(self.types, parameter.structural_type, &slot.path)
                else {
                    continue;
                };
                if !matches!(
                    self.declaration(ty, concerns)?.shape,
                    StructuralTypeShape::Reference { .. }
                ) {
                    continue;
                }
                slot.path.push(DescriptorPathSegment::Referent);
                let path = descriptor_path(self.types, parameter.structural_type, &slot.path)
                    .ok_or_else(|| {
                        self.unsupported(
                            concerns,
                            "incoming reference lost its pending-effect backing",
                        )
                    })?;
                let raw = self.stored_at(slot.root, &path, concerns)?;
                let backing = self.backing(references, &raw, entry.0, 0, concerns)?;
                incoming.extend(backing.possible_regions().cloned());
            }
        }
        incoming.sort();
        incoming.dedup();
        let mut invocations = tables
            .invocations
            .iter()
            .filter_map(|(id, row)| match row.site {
                semantic_vocabulary::InvocationSite::Operation(operation) => {
                    Some((operation, id, row))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        invocations.sort_by_key(|(operation, _, _)| *operation);
        let mut exposed = BTreeMap::<Lineage, Vec<DescriptorPosition>>::new();
        for (at, step) in &flow {
            if !step.may_escape || !relevant.contains(at) {
                continue;
            }
            let mut backings = Vec::new();
            if let Some(operation) = self.block(at.0)?.operations.get(at.1 as usize)
                && let Ok(position) =
                    invocations.binary_search_by_key(&operation.id, |(id, _, _)| *id)
            {
                let (_, id, row) = invocations[position];
                let call =
                    crate::call_contracts::invocation(self.module, self.lowered, tables, id, row)?;
                // Borrowing inline storage exposes its address. Passing an
                // owned descriptor exposes its captured backing instead; it
                // does not hand out the address of the descriptor's home.
                for (actual, formal) in call
                    .inputs
                    .structural_arguments
                    .iter()
                    .zip(call.contract.structural_parameters)
                {
                    if formal.access != StructuralAccess::Owned
                        && matches!(
                            self.borrowed_resource(formal),
                            super::loans::BorrowedResource::Storage
                        )
                    {
                        let raw = self.argument(actual, concerns)?;
                        backings.push(self.backing(
                            references,
                            &raw,
                            at.0,
                            at.1 as usize,
                            concerns,
                        )?);
                    }
                }
            }
            for input in step.observed_inputs() {
                // A packet has no address. Its existing hidden loans remain
                // pending; it cannot expose arbitrary local storage by itself.
                if input.path.contains(&DescriptorPathSegment::UnknownContents) {
                    continue;
                }
                let root = self.root_type(input.root, concerns)?;
                let path = descriptor_path(self.types, root, &input.path).ok_or_else(|| {
                    self.unsupported(concerns, "escaping input has no exact storage path")
                })?;
                let raw = self.stored_at(input.root, &path, concerns)?;
                let is_pointer = match self.shape(&raw, concerns)? {
                    Shape::View => true,
                    Shape::Typed(ty) => DescriptorSlotKind::BorrowedStorage.is_slot(self.types, ty),
                    _ => false,
                };
                let mut carriers = vec![(input.clone(), is_pointer)];
                if let Some(ty) = descriptor_location_type(self.types, root, &input.path) {
                    let inventory = DescriptorSlotKind::BorrowedStorage
                        .owned_inventory(self.types, input.root, ty)
                        .ok_or_else(|| {
                            self.unsupported(
                                concerns,
                                "escaping input has no retained borrowed-slot inventory",
                            )
                        })?;
                    for mut slot in inventory.slots {
                        let mut path = input.path.clone();
                        path.append(&mut slot.path);
                        slot.path = path;
                        carriers.push((slot, true));
                    }
                }
                carriers.sort();
                carriers.dedup();
                // A pointer field exposes its captured backing just as a
                // pointer passed alone does. The record's inline home is
                // not a substitute for the loans its fields retain.
                for (carrier, is_pointer) in carriers {
                    let origins = references.symbolic_source_origins(self.machine, *at, &carrier);
                    if is_pointer && origins.as_ref().is_none_or(Vec::is_empty) {
                        return Err(self.unsupported(
                            concerns,
                            "escaping pointer has no complete captured backing origins",
                        ));
                    }
                    for origin in origins.into_iter().flatten() {
                        if let DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                            source,
                            projection,
                        }) = origin
                        {
                            let path = projection
                                .captured_storage_path()
                                .or_else(|| {
                                    projection.enclosing_storage().map(|region| region.path)
                                })
                                .ok_or_else(|| {
                                    self.unsupported(
                                        concerns,
                                        "escaping pointer has no containing backing",
                                    )
                                })?;
                            let raw = self.stored_at(source, &path, concerns)?;
                            backings.push(self.backing(
                                references,
                                &raw,
                                at.0,
                                at.1 as usize,
                                concerns,
                            )?);
                        }
                    }
                }
            }
            for backing in backings {
                // Exposure only adds conservative possible effects. It grants
                // no equality between independently selected references.
                for region in backing.possible_regions() {
                    exposed.entry(region.clone()).or_default().push(*at);
                }
            }
        }
        // Repeated calls often expose the same backing. Propagate each exact
        // region once, instead of rescanning predecessors for every call.
        // An empty list separates known storage; the unknown packet's own
        // pending effects and loans remain in descriptor replay.
        let mut possible = targets
            .iter()
            .map(|at| (*at, incoming.clone()))
            .collect::<BTreeMap<_, _>>();
        for (backing, mut pending) in exposed {
            let mut visited = BTreeSet::new();
            while let Some(position) = pending.pop() {
                if !relevant.contains(&position) || !visited.insert(position) {
                    continue;
                }
                if let Some(backings) = possible.get_mut(&position) {
                    backings.push(backing.clone());
                }
                let step = flow.get(&position).ok_or_else(|| {
                    self.unsupported(
                        concerns,
                        "pending payload effect has an incomplete flow position",
                    )
                })?;
                pending.extend(step.successors.iter().map(|edge| edge.target));
            }
        }
        for backings in possible.values_mut() {
            backings.sort();
            backings.dedup();
        }
        Ok(possible)
    }

    fn byte_payload_descriptors(
        &self,
        region: &Lineage,
        concerns: Construct,
    ) -> Checked<PayloadInventory> {
        let inventory = match self.shape(region, concerns)? {
            Shape::Typed(row) => DescriptorSlotKind::Views
                .inventory(self.types, region.root, row)
                .ok_or_else(|| {
                    self.unsupported(
                        concerns,
                        "payload write has no complete descriptor inventory",
                    )
                })?,
            Shape::View => {
                return Ok(PayloadInventory {
                    descriptors: vec![region.clone()],
                    unknown_contents: Vec::new(),
                });
            }
            Shape::Scalar(_) | Shape::Atomic(_) | Shape::Payload(_) => {
                return Ok(PayloadInventory::default());
            }
        };
        let prefix = region.descriptor_location().ok_or_else(|| {
            self.unsupported(concerns, "payload write has no captured descriptor path")
        })?;
        let root = self.root_type(region.root, concerns)?;
        let mut found = PayloadInventory::default();
        for mut contents in inventory.unknown_contents {
            let mut path = prefix.path.clone();
            path.append(&mut contents.location.path);
            contents.location = DescriptorLocation {
                root: region.root,
                path,
            };
            let mut carrier = contents.location.clone();
            if carrier.path.last() == Some(&DescriptorPathSegment::UnknownContents) {
                carrier.path.pop();
            }
            let path = descriptor_path(self.types, root, &carrier.path).ok_or_else(|| {
                self.unsupported(concerns, "unknown payload lost its exact containing path")
            })?;
            found.unknown_contents.push(PendingPayloadWrite {
                carrier: self.stored_at(region.root, &path, concerns)?,
                contents,
            });
        }
        for mut slot in inventory.slots {
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
                found.descriptors.push(descriptor);
            }
        }
        Ok(found)
    }

    fn byte_payload_backings(
        &self,
        references: &mut super::DescriptorAnalysis<'a>,
        descriptor: &Lineage,
        at: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
        concerns: Construct,
    ) -> Checked<Vec<Lineage>> {
        let location = descriptor.descriptor_location().ok_or_else(|| {
            self.unsupported(concerns, "byte payload has no exact descriptor location")
        })?;
        let origins = references
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
