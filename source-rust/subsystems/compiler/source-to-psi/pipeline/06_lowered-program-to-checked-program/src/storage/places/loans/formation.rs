//! Recover the lender, access and borrowed region from a reference-producing
//! operation before comparing its loan row with the descriptor lifetime.
use super::super::{DescriptorAnalysis, Lineage, Shape};
use super::*;
use terminal_psi::{OperationKind, StructuralTypeShape};

pub(super) struct Formation {
    pub(super) captured: terminal_check_tables::ExactPlaceId,
    pub(super) access: ReferenceAccess,
    pub(super) lineage: LoanRegion,
    pub(super) expected_lender: Option<Lender>,
    pub(super) parent: Option<ReferenceAccess>,
    pub(super) from: Option<DescriptorLocation>,
    pub(super) permitted: bool,
}

impl<'a> Storage<'a> {
    pub(super) fn loan_formation(
        &self,
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        operation: &terminal_psi::Operation,
        result: &terminal_psi::StructuralOperationResult,
        at: DescriptorPosition,
        concerns: Construct,
    ) -> Checked<Formation> {
        if let OperationKind::BorrowAt { address } = operation.kind {
            return self.raw_address_formation(references, tables, result, address, concerns);
        }
        let upgrades_shared = matches!(operation.kind, OperationKind::UpgradeMutability { .. });
        let upgraded;
        let (source, declared, reference_referent) = match &operation.kind {
            OperationKind::EstablishReference { source, storage } => {
                let StructuralTypeShape::Reference { referent, access } =
                    self.declaration(result.structural_type, concerns)?.shape
                else {
                    return Err(
                        self.foreign(concerns, "reference formation produces another constructor")
                    );
                };
                (source, access, Some((referent, storage.as_ref())))
            }
            OperationKind::UpgradeMutability { source } => {
                let destination = &self.declaration(result.structural_type, concerns)?.shape;
                let input = terminal_semantics::mutability_upgrade_source(
                    self.lowered,
                    self.types,
                    source,
                    destination,
                )
                .ok_or_else(|| {
                    self.foreign(
                        concerns,
                        "mutability upgrade must preserve the shared referent or view geometry",
                    )
                })?;
                // A sized descriptor lends its referent. A view already spans
                // its exact window; the ordinary view-payload loan retains it.
                let mut actual = source.clone();
                let referent = match input.shape {
                    StructuralTypeShape::Reference { referent, .. } => {
                actual
                    .path
                    .push(terminal_psi::StructuralPathSegment::Referent);
                        Some((referent, None))
                    }
                    _ => None,
                };
                actual.access = StructuralAccess::MutableBorrow;
                upgraded = actual;
                (&upgraded, StructuralAccess::MutableBorrow, referent)
            }
            OperationKind::EstablishElementView { source, .. }
            | OperationKind::EstablishMutableElementView { source, .. } => {
                self.array_view_formation(operation)?;
                (source, source.access, None)
            }
            _ => return Err(self.foreign(concerns, "operation has no borrowed storage operand")),
        };
        if source.access != declared {
            return Err(self.foreign(concerns, "reference formation changes its source access"));
        }
        let access = reference_access(source.access)
            .ok_or_else(|| self.foreign(concerns, "reference formation has no borrowed access"))?;
        if access == ReferenceAccess::WriteOnly {
            return Err(self.unsupported(concerns, "a write-only persistent loan"));
        }
        let raw = self.argument(source, concerns)?;
        let resource = if reference_referent.is_none()
            && match self.shape(&raw, concerns)? {
                Shape::View => true,
                Shape::Typed(ty) => {
                    terminal_semantics::descriptor_results::is_descriptor(self.types.iter(), ty)
                }
                _ => false,
            } {
            BorrowedResource::ViewPayload
        } else {
            BorrowedResource::Storage
        };
        if let Some((referent, storage)) = reference_referent {
            let expected = self.declaration(referent, concerns)?;
            let compatible = if let Some(storage) = storage {
                if access != ReferenceAccess::Shared {
                    return Err(self.unsupported(
                        concerns,
                        "exclusive representation view without establishment and handback evidence",
                    ));
                }
                terminal_semantics::validate_reference_view_source(
                    self.module,
                    self.lowered,
                    source.place,
                )
                .map_err(|reason| self.unsupported(concerns, reason))?;
                let offset = tables.values.get(storage.byte_offset).ok_or_else(|| {
                    self.foreign(concerns, "reference view lost its offset value")
                })?;
                let terminal_check_tables::values::ValueOrigin::Operation(producer) = offset.origin
                else {
                    return Err(self.unsupported(
                        concerns,
                        "reference view offset without captured geometry evidence",
                    ));
                };
                let (block, after) = self.graph.locate(ProgramPoint::AfterOperation(producer))?;
                let producer = after
                    .checked_sub(1)
                    .and_then(|position| {
                        self.lowered
                            .blocks
                            .get(index(block.get()))?
                            .operations
                            .get(position)
                    })
                    .ok_or_else(|| {
                        self.foreign(concerns, "reference view offset has no producer")
                    })?;
                if offset.scalar_type
                    != producer
                        .result
                        .as_scalar()
                        .map(|row| row.scalar_type)
                        .ok_or_else(|| {
                            self.foreign(concerns, "reference view offset has no scalar producer")
                        })?
                    || !terminal_semantics::reference_view_offset_is_zero(
                        producer,
                        storage.byte_offset,
                    )
                {
                    return Err(self.unsupported(
                        concerns,
                        "reference view offset without captured geometry evidence",
                    ));
                }
                terminal_semantics::reference_view_compatible(
                    self.module,
                    self.root_type(source.place, concerns)?,
                    &source.path,
                    result.structural_type,
                    storage.stored_scalar,
                )
                .map_err(|reason| self.unsupported(concerns, reason))?
            } else {
                match self.shape(&raw, concerns)? {
                    Shape::Scalar(scalar) => {
                        expected.shape == StructuralTypeShape::PrimitiveScalar(scalar)
                    }
                    Shape::Atomic(cell) => expected.shape == StructuralTypeShape::AtomicCell(cell),
                    Shape::Typed(actual) => {
                        actual == referent
                            || terminal_semantics::shared_direct_byte_descriptor_source(
                                self.module,
                                self.lowered,
                                source,
                                result.structural_type,
                            ) == Some(true)
                    }
                    Shape::View | Shape::Payload(_) => false,
                }
            };
            if !compatible {
                return Err(self.foreign(
                    concerns,
                    "reference formation changes its captured referent type",
                ));
            }
        }
        let captured = self.interned(tables, &raw, concerns)?.ok_or_else(|| {
            self.foreign(concerns, "reference formation has no captured place row")
        })?;
        let (expected_lender, parent, from, permitted) = self.source_lender(
            &raw,
            &source.path,
            if upgrades_shared {
                ReferenceAccess::Shared
            } else {
                access
            },
            resource,
            concerns,
        )?;
        let lineage =
            self.borrow_backing(references, &raw, &source.path, resource, at, concerns)?;
        Ok(Formation {
            captured,
            access,
            lineage,
            expected_lender,
            parent,
            from,
            permitted,
        })
    }

    fn raw_address_formation(
        &self,
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        result: &terminal_psi::StructuralOperationResult,
        address: semantic_vocabulary::ValueId,
        concerns: Construct,
    ) -> Checked<Formation> {
        let StructuralTypeShape::Reference {
            access: declared,
            referent,
        } = self.declaration(result.structural_type, concerns)?.shape
        else {
            return Err(self.foreign(
                concerns,
                "native address formation must produce a reference",
            ));
        };
        let access = match declared {
            StructuralAccess::SharedBorrow => ReferenceAccess::Shared,
            StructuralAccess::MutableBorrow => ReferenceAccess::Mutable,
            _ => {
                return Err(self.foreign(
                    concerns,
                    "native address formation requires shared or mutable access",
                ));
            }
        };
        if !tables.values.get(address).is_some_and(|row| {
            matches!(row.scalar_type,
                semantic_vocabulary::ScalarType::Integer(i) if i.is_address())
        }) {
            return Err(self.foreign(concerns, "native borrow requires an address operand"));
        }
        let descriptor = self.stored_at(result.place, &[], concerns)?;
        let captured = self
            .interned(tables, &descriptor, concerns)?
            .ok_or_else(|| {
                self.foreign(
                    concerns,
                    "native address formation has no captured result row",
                )
            })?;
        let raw =
            self.native_address_loan_region(references, address, &descriptor, referent, concerns)?;
        Ok(Formation {
            captured,
            access,
            lineage: raw,
            expected_lender: Some(Lender::NativeAddress),
            parent: None,
            from: None,
            permitted: true,
        })
    }
    fn native_address_loan_region(
        &self,
        references: &mut DescriptorAnalysis<'a>,
        address: semantic_vocabulary::ValueId,
        descriptor: &Lineage,
        referent: semantic_vocabulary::StructuralTypeId,
        concerns: Construct,
    ) -> Checked<LoanRegion> {
        let origin = terminal_semantics::storage::native_borrows::address_origin(
            self.module,
            self.lowered,
            address,
        );
        let mut sources = Vec::new();
        let mut exact = origin.exact && origin.sources.len() == 1;
        for (at, source) in origin.observed_sources() {
            exact &= terminal_semantics::runtime_structural_path_tip(
                self.types.iter(),
                self.root_type(source.place, concerns)?,
                &source.path,
            )
            .is_some_and(|ty| ty.id == referent);
            let place = self.argument(source, concerns)?;
            match self.storage_loan_region(references, &place, at, concerns)? {
                LoanRegion::Storage(backing) => sources.push(backing),
                LoanRegion::Provider(_) => sources.push(Backing::direct(place)),
            }
        }
        let backing = Backing::native_address(address, descriptor, sources, exact);
        Ok(LoanRegion::Storage(backing))
    }
}
