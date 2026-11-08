/*
  Structural, primitive-place, borrowed-window, and atomic operation lowering
  from Terminal Psi into `AbstractOperation` rows.

  Lowers record, case, and scalar-array establishments, structural scalar field
  reads and stores, leaf copies, primitive local storage and projections,
  borrowed byte/element window slices, and sequentially consistent atomic memory
  operations.
*/

use super::{LoweredAffineLocal, StructuralLiteral, structural_result, unique_row};
pub(crate) mod atomic_events {
    use std::collections::{BTreeMap, BTreeSet};

    use abstract_operations::atomic::AbstractAtomicLocation;
    use abstract_operations::{
        AbstractAtomicEvent, AbstractOperation, AtomicResidencyEdge, AtomicResidencySource,
    };
    use semantic_vocabulary::{OperationId, PlaceId, ScalarType};
    use terminal_psi::{
        AtomicAccessEvent, Operation, OperationKind, StructuralPathSegment,
        StructuralTypeDeclaration, TerminalMachine,
    };

    use crate::lowering::LoweringError;

    pub(crate) fn lower(
        operation: &Operation,
        machine: &TerminalMachine,
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<AbstractOperation, LoweringError> {
        let OperationKind::AtomicAccess {
            place,
            path,
            event,
            resident_obligation,
        } = &operation.kind
        else {
            unreachable!("the atomic router dispatches only AtomicAccess");
        };
        let malformed = || LoweringError::VerifiedAtomicAccessMalformed(operation.id);
        let root = terminal_semantics::arrival_parameter(machine, *place)
            .map(|parameter| parameter.structural_type)
            .or_else(|| {
                machine
                    .producing_operation(*place)
                    .map(|(_, structural_type)| structural_type)
            })
            .ok_or(LoweringError::UnsupportedAtomicAccess(operation.id))?;
        let leaf = field_type(structural_types, root, path).ok_or_else(malformed)?;
        let location = AbstractAtomicLocation {
            root: *place,
            path: path.clone(),
        };
        let prior = || {
            operation
                .result
                .scalar()
                .filter(|result| result.scalar_type == leaf)
                .map(|result| result.into())
                .ok_or_else(malformed)
        };
        let lowered = match *event {
            AtomicAccessEvent::Load { ordering } => AbstractAtomicEvent::Load {
                location,
                ordering,
                result: prior()?,
            },
            AtomicAccessEvent::Store { value, ordering } => {
                if operation.result != terminal_psi::OperationResult::Unit {
                    return Err(malformed());
                }
                AbstractAtomicEvent::Store {
                    location,
                    ordering,
                    value,
                }
            }
            AtomicAccessEvent::ReadModifyWrite {
                operation: arithmetic,
                operand,
                ordering,
            } => AbstractAtomicEvent::ReadModifyWrite {
                location,
                operation: arithmetic.into(),
                ordering,
                operand,
                prior: prior()?,
            },
            AtomicAccessEvent::Swap { value, ordering } => AbstractAtomicEvent::Swap {
                location,
                ordering,
                value,
                prior: prior()?,
            },
            AtomicAccessEvent::CompareExchange {
                expected,
                replacement,
                success,
                failure,
            } => AbstractAtomicEvent::CompareExchange {
                location,
                success,
                failure,
                expected,
                replacement,
                observed: prior()?,
            },
            AtomicAccessEvent::CompareExchangeOnce {
                custody,
                expected,
                replacement,
                success,
                failure,
            } => {
                let outcome = operation
                    .result
                    .as_structural()
                    .filter(|result| {
                        terminal_psi::types::atomic_once_result_is_plain(result, custody)
                    })
                    .ok_or_else(malformed)?;
                terminal_psi::types::atomic_once_outcome_cases(
                    structural_types
                        .iter()
                        .find(|row| row.id == outcome.structural_type)
                        .ok_or_else(malformed)?,
                    leaf,
                    custody,
                )
                .ok_or_else(malformed)?;
                AbstractAtomicEvent::CompareExchangeOnce {
                    location,
                    success,
                    failure,
                    expected,
                    replacement,
                    outcome: outcome.clone(),
                    custody,
                }
            }
        };
        if !lowered.ordering_is_legal() || !lowered.custody_is_consistent() {
            return Err(malformed());
        }
        let reached = reaching_fact(machine, operation.id, structural_types)?;
        let retained = if !lowered.observes_resident() && !lowered.joins_modification_order() {
            None
        } else {
            let source = |reaching| match reaching {
                Reaching::Initial => Ok(AtomicResidencySource::Initial),
                Reaching::Atomic(operation) => Ok(AtomicResidencySource::Write { operation }),
                Reaching::Effect(operation) => Ok(AtomicResidencySource::AfterEffect { operation }),
                Reaching::Unknown => Err(LoweringError::UnsupportedAtomicCoherence(operation.id)),
            };
            let sources = reached
                .iter()
                .copied()
                .map(source)
                .collect::<Result<Vec<_>, _>>()?;
            Some(match sources.as_slice() {
                [AtomicResidencySource::Initial] => AtomicResidencyEdge::InitialResidency,
                [AtomicResidencySource::Write { operation }] => AtomicResidencyEdge::Write {
                    operation: *operation,
                },
                [AtomicResidencySource::AfterEffect { operation }] => {
                    AtomicResidencyEdge::AfterEffect {
                        operation: *operation,
                    }
                }
                [] => return Err(LoweringError::UnsupportedAtomicCoherence(operation.id)),
                _ => AtomicResidencyEdge::PathAlternatives { sources },
            })
        };
        let (reads_from, modification_after) = match retained {
            None => (None, None),
            Some(edge) => (
                lowered.observes_resident().then(|| edge.clone()),
                lowered.joins_modification_order().then_some(edge),
            ),
        };
        Ok(AbstractOperation::AtomicEvent {
            psi_operation: operation.id,
            event: lowered,
            resident_obligation: *resident_obligation,
            reads_from,
            modification_after,
        })
    }

    fn field_type(
        structural_types: &[StructuralTypeDeclaration],
        root: semantic_vocabulary::StructuralTypeId,
        path: &[terminal_psi::StructuralPathSegment],
    ) -> Option<ScalarType> {
        terminal_semantics::atomic_cell_type(structural_types.iter(), root, path)
            .map(|cell| cell.scalar_type())
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Reaching {
        Initial,
        Atomic(OperationId),
        Effect(OperationId),
        Unknown,
    }

    fn reaching_fact(
        machine: &TerminalMachine,
        target: OperationId,
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<BTreeSet<Reaching>, LoweringError> {
        let unsupported = || LoweringError::UnsupportedAtomicCoherence(target);
        let (target_block, target_index, location) = machine
            .blocks
            .iter()
            .find_map(|block| {
                block
                    .operations
                    .iter()
                    .enumerate()
                    .find_map(|(index, operation)| {
                        (operation.id == target).then(|| match &operation.kind {
                            OperationKind::AtomicAccess { place, path, .. } => Some((
                                block.id,
                                index,
                                AbstractAtomicLocation {
                                    root: *place,
                                    path: path.clone(),
                                },
                            )),
                            _ => None,
                        })
                    })
            })
            .flatten()
            .ok_or_else(unsupported)?;
        let blocks = machine
            .blocks
            .iter()
            .map(|block| (block.id, block))
            .collect::<BTreeMap<_, _>>();
        let mut reached = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut pending = vec![(target_block, target_index, location, Vec::new())];
        while let Some((block, before, location, mut ancestry)) = pending.pop() {
            if !visited.insert((block, before, location.clone())) {
                continue;
            }
            let coordinate = (block, before, location.root);
            if ancestry.contains(&coordinate) {
                reached.insert(Reaching::Unknown);
                continue;
            }
            ancestry.push(coordinate);
            let declaration = blocks.get(&block).ok_or_else(unsupported)?;
            let local = (location.root, location.path.clone());
            if let Some(write) = declaration.operations[..before]
                .iter()
                .rev()
                .find_map(|operation| latest_write(operation, &local, machine, structural_types))
            {
                reached.insert(write);
                continue;
            }
            if block == machine.entry {
                reached.insert(
                    if machine
                        .structural_parameters
                        .iter()
                        .any(|row| row.place == location.root)
                    {
                        Reaching::Initial
                    } else {
                        Reaching::Unknown
                    },
                );
            }
            let mut has_predecessor = false;
            for predecessor in &machine.blocks {
                if !predecessor.terminator.successor_targets().contains(&block) {
                    continue;
                }
                has_predecessor = true;
                match terminal_semantics::projection_before_edge(
                    machine,
                    block,
                    predecessor,
                    location.root,
                ) {
                    Some(source) => {
                        let mut path = source.path.to_vec();
                        path.extend_from_slice(&location.path);
                        pending.push((
                            predecessor.id,
                            predecessor.operations.len(),
                            AbstractAtomicLocation {
                                root: source.root,
                                path,
                            },
                            ancestry.clone(),
                        ));
                    }
                    None => {
                        reached.insert(Reaching::Unknown);
                    }
                }
            }
            if block != machine.entry && !has_predecessor {
                reached.insert(Reaching::Unknown);
            }
        }
        Ok(reached)
    }

    type Location = (PlaceId, Vec<StructuralPathSegment>);

    fn latest_write(
        operation: &Operation,
        location: &Location,
        machine: &TerminalMachine,
        structural_types: &[StructuralTypeDeclaration],
    ) -> Option<Reaching> {
        let (root, path) = location;
        if operation
            .result
            .structural()
            .is_some_and(|result| result.place == *root)
        {
            return Some(Reaching::Effect(operation.id));
        }
        let may_alias = |other, other_path: &[terminal_psi::StructuralPathSegment]| {
            abstract_operations::atomic::atomic_projections_may_alias(
                &machine.structural_places,
                std::iter::once(machine.structural_parameters.as_slice()).chain(
                    machine
                        .blocks
                        .iter()
                        .map(|block| block.structural_parameters.as_slice()),
                ),
                structural_types.iter(),
                *root,
                path,
                other,
                other_path,
            )
        };
        match &operation.kind {
            OperationKind::AtomicAccess {
                place: other_root,
                path: other_path,
                event,
                ..
            } => {
                if other_root == root && other_path == path && event.modifies_resident() {
                    return Some(Reaching::Atomic(operation.id));
                }
                if event.modifies_resident()
                    && ((other_root != root && may_alias(*other_root, other_path))
                        || other_root == root
                            && (path.contains(&terminal_psi::StructuralPathSegment::Referent)
                                || other_path
                                    .contains(&terminal_psi::StructuralPathSegment::Referent)))
                {
                    return Some(Reaching::Effect(operation.id));
                }
            }
            OperationKind::StructuralScalarFieldStore {
                destination,
                path: other_path,
                field: other_field,
                ..
            } => {
                let Some(other_type) = terminal_semantics::arrival_parameter(machine, *destination)
                    .map(|parameter| parameter.structural_type)
                    .or_else(|| machine.producing_operation(*destination).map(|(_, ty)| ty))
                else {
                    return Some(Reaching::Unknown);
                };
                let Some(declaration) = terminal_semantics::projected_field_declaration(
                    structural_types.iter(),
                    other_type,
                    other_path,
                    *other_field,
                ) else {
                    return Some(Reaching::Unknown);
                };
                let mut complete = other_path.clone();
                complete.push(StructuralPathSegment::Field(declaration.identity));
                let other_path = &complete;
                if (destination != root && may_alias(*destination, other_path))
                    || (destination == root
                        && (path.contains(&terminal_psi::StructuralPathSegment::Referent)
                            || other_path.contains(&terminal_psi::StructuralPathSegment::Referent)
                            || terminal_semantics::structural_paths_may_overlap(other_path, path)))
                {
                    return Some(Reaching::Effect(operation.id));
                }
            }
            OperationKind::PrimitiveScalarRead { .. }
            | OperationKind::ScalarStructuralField { .. }
            | OperationKind::BooleanStructuralField { .. }
            | OperationKind::StructuralCaseMembership { .. }
            | terminal_psi::structural_leaf_copy_pair_pattern!()
            | OperationKind::StructuralByteSequenceFieldLength { .. }
            | OperationKind::StructuralByteSequenceFieldRead { .. } => {}
            _ => {
                let lends_root = operation
                    .kind
                    .structural_projections()
                    .iter()
                    .any(|projection| {
                        projection.root == *root || may_alias(projection.root, projection.path)
                    });
                if lends_root {
                    return Some(Reaching::Effect(operation.id));
                }
            }
        }
        None
    }
}

pub(crate) mod borrowed_windows {
    use abstract_operations::{
        AbstractOperation, StructuralScalarStoreDestination, StructuralWindowRoot,
    };
    use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId};
    use terminal_psi::{
        Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
        StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    };

    use crate::lowering::LoweringError;

    use super::structural_scalar_fields::{exact_parameter, owned_storage_destination};

    pub(crate) fn lower(
        operation: &Operation,
        machine: &TerminalMachine,
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<AbstractOperation, LoweringError> {
        match &operation.kind {
            OperationKind::MoveStructuralField {
                source,
                path,
                field,
            } => lower_move(operation, machine, structural_types, *source, path, *field),
            OperationKind::StoreStructuralField {
                disposition,
                destination,
                path,
                field,
                value,
            } => lower_store(
                operation,
                machine,
                structural_types,
                *destination,
                *disposition,
                path,
                *field,
                value,
            ),
            _ => unreachable!("borrowed-window router is exhaustive"),
        }
    }

    fn lower_move(
        operation: &Operation,
        machine: &TerminalMachine,
        structural_types: &[StructuralTypeDeclaration],
        source: PlaceId,
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
    ) -> Result<AbstractOperation, LoweringError> {
        let invalid = |reason: &str| LoweringError::InvalidBorrowedStorageWindow {
            machine: machine.id,
            operation: operation.id,
            place: source,
            reason: reason.to_owned(),
        };
        let result = operation
            .result
            .structural()
            .ok_or_else(|| invalid("window extraction has no structural result"))?;
        if !matches!(
            machine.structural_place(result.place)
                .map(|place| place.kind),
            Some(StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            }) if producer == operation.id && structural_type == result.structural_type
        ) {
            return Err(invalid(
                "extracted result does not name its exact producer and carrier",
            ));
        }
        let source = borrowed_window_root(machine, source, StructuralAccess::MutableBorrow)
        .ok_or_else(|| {
            invalid(&format!(
                "extraction root has no admitted mutable storage: declaration={:?}, signature={:?}, block={:?}, block_claims={:?}",
                machine.structural_place(source),
                machine.structural_parameter(source),
                block_parameter(machine, source),
                machine.block_entry_claims.iter().filter(|claim| claim.input == source).collect::<Vec<_>>(),
            ))
        })?;
        let projection = declared_window_projection(
            structural_types,
            &source,
            path,
            field,
            StructuralAccess::MutableBorrow,
        )
        .ok_or_else(|| invalid("extraction path has no admitted structural window"))?;
        let signature = source_signature(machine, source.place())
            .ok_or_else(|| invalid("extraction root has no declared source signature"))?;
        let (qualifications, projected) =
            terminal_psi::StructuralPathQualification::select_restorable(
                signature.qualifications,
                signature.projected_qualifications,
                &projection.path,
            )
            .ok_or_else(|| invalid("extraction memberships have no exact restorable projection"))?;
        if projection.structural_type != result.structural_type {
            return Err(invalid(&format!(
                "extracted carrier {:?} differs from window carrier {:?}",
                result.structural_type, projection.structural_type,
            )));
        }
        if result.qualifications != qualifications {
            return Err(invalid(
                "extracted memberships differ from the declared window",
            ));
        }
        if result.projected_qualifications != projected {
            return Err(invalid(
                "extracted child memberships differ from the declared window",
            ));
        }
        if !result.qualification_establishments.is_empty() {
            return Err(invalid("extraction attempts to establish new memberships"));
        }
        if !root_window_loan(&source, path, field) {
            return Err(invalid(
                "extraction has no admitted child or whole-referent loan",
            ));
        }
        Ok(AbstractOperation::MoveStructuralField {
            psi_operation: operation.id,
            result: result.clone(),
            source,
            path: path.to_vec(),
            field,
        })
    }

    fn lower_store(
        operation: &Operation,
        machine: &TerminalMachine,
        structural_types: &[StructuralTypeDeclaration],
        destination: PlaceId,
        disposition: terminal_psi::StructuralStoreDisposition,
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
        value: &StructuralArgument,
    ) -> Result<AbstractOperation, LoweringError> {
        let invalid = |reason: &str| LoweringError::InvalidBorrowedStorageWindow {
            machine: machine.id,
            operation: operation.id,
            place: destination,
            reason: reason.to_owned(),
        };
        if operation.result != OperationResult::Unit {
            return Err(invalid("window store must produce Unit"));
        }
        if !terminal_semantics::structural_store_value_admitted(
            machine,
            structural_types.iter(),
            disposition,
            value.place,
        ) {
            return Err(invalid(
                "stored value has no admitted custody for this disposition",
            ));
        }
        if value.access != StructuralAccess::Owned {
            return Err(invalid("window store must consume an owned value"));
        }
        if !value.path.is_empty() {
            return Err(invalid(
                "window store must consume a complete value occurrence",
            ));
        }
        let destination = borrowed_window_root(machine, destination, disposition.required_access())
            .ok_or_else(|| invalid("store root has no admitted storage access"))?;
        let projection = declared_window_projection(
            structural_types,
            &destination,
            path,
            field,
            disposition.required_access(),
        )
        .ok_or_else(|| invalid("store path has no admitted structural window"))?;
        let signature = source_signature(machine, destination.place())
            .ok_or_else(|| invalid("store root has no declared source signature"))?;
        let (required, required_projected) =
            terminal_psi::StructuralPathQualification::select_restorable(
                signature.qualifications,
                signature.projected_qualifications,
                &projection.path,
            )
            .ok_or_else(|| invalid("store memberships have no exact restorable projection"))?;
        let Some(source) = source_signature(machine, value.place) else {
            return Err(invalid("stored value has no declared source signature"));
        };
        if source.structural_type != projection.structural_type {
            return Err(invalid(&format!(
                "stored carrier {:?} differs from window carrier {:?}",
                source.structural_type, projection.structural_type,
            )));
        }
        if !required
            .iter()
            .all(|domain| source.qualifications.contains(domain))
        {
            return Err(invalid("stored value lacks required window memberships"));
        }
        if !required_projected
            .iter()
            .all(|row| source.projected_qualifications.contains(row))
        {
            return Err(invalid("stored value lacks required child memberships"));
        }
        let whole_owned_store = path.is_empty()
            && field.is_none()
            && destination.parameter().is_none()
            && disposition == terminal_psi::StructuralStoreDisposition::Vacant;
        if !root_window_loan(&destination, path, field) && !whole_owned_store {
            return Err(invalid(
                "store has no admitted child or whole-referent loan",
            ));
        }
        Ok(AbstractOperation::StoreStructuralField {
            disposition,
            psi_operation: operation.id,
            destination,
            path: path.to_vec(),
            field,
            value: value.clone(),
        })
    }

    fn borrowed_window_root(
        machine: &TerminalMachine,
        place: PlaceId,
        required_access: StructuralAccess,
    ) -> Option<StructuralWindowRoot> {
        if let Some(parameter) =
            exact_parameter(machine, place).or_else(|| block_parameter(machine, place).cloned())
            && ((parameter.access == StructuralAccess::Owned
                && exact_parameter(machine, place).is_some())
                || (required_access.admits_referent(parameter.access)
                    && matches!(
                        parameter.multiplicity,
                        terminal_psi::nonlinear_multiplicity_pair_pattern!()
                    )
                    && !machine
                        .entry_claims
                        .iter()
                        .any(|claim| claim.input == place)
                    && !machine
                        .content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == place)))
        {
            return Some(StructuralWindowRoot::Parameter(parameter));
        }
        if let Some((block, parameter)) =
            super::primitive_storage::owned_state_storage_parameter(machine, place)
            && block != machine.entry
            && !machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == place)
        {
            return Some(StructuralWindowRoot::OwnedBlockParameter {
                block,
                parameter: parameter.clone(),
            });
        }
        let StructuralScalarStoreDestination::OwnedResult { producer, result } =
            owned_storage_destination(machine, place)?
        else {
            return None;
        };
        Some(StructuralWindowRoot::OwnedResult { producer, result })
    }

    fn root_window_loan(
        root: &StructuralWindowRoot,
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
    ) -> bool {
        terminal_semantics::structural_window_selects_child(path, field)
            || path.contains(&StructuralPathSegment::Referent)
            || (path.is_empty() && root.parameter().is_some())
    }

    fn declared_window_projection(
        structural_types: &[StructuralTypeDeclaration],
        root: &StructuralWindowRoot,
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
        required_access: StructuralAccess,
    ) -> Option<terminal_semantics::StructuralWindowProjection> {
        if let Some(crossing) = path
            .iter()
            .rposition(|segment| *segment == StructuralPathSegment::Referent)
        {
            let carrier = terminal_semantics::structural_window_projection(
                structural_types.iter(),
                root.structural_type(),
                &path[..crossing],
                None,
            )?;
            let StructuralTypeShape::Reference { access, .. } =
                terminal_psi::find_structural_type(structural_types, carrier.structural_type)?
                    .shape
            else {
                return None;
            };
            if !required_access.admits_referent(access)
                || matches!(root, StructuralWindowRoot::OwnedResult { result, .. } if !result.qualification_establishments.is_empty())
            {
                return None;
            }
        }
        terminal_semantics::structural_window_projection(
            structural_types.iter(),
            root.structural_type(),
            path,
            field,
        )
    }

    pub(crate) struct SourceSignature<'declaration> {
        pub(crate) structural_type: StructuralTypeId,
        access: StructuralAccess,
        multiplicity: StructuralMultiplicity,
        qualifications: &'declaration [semantic_vocabulary::StructuralDomainId],
        projected_qualifications: &'declaration [terminal_psi::StructuralPathQualification],
    }

    impl SourceSignature<'_> {
        pub(crate) fn descriptor_access(
            &self,
            structural_types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> StructuralAccess {
            if self.access != StructuralAccess::Owned {
                return self.access;
            }
            terminal_semantics::descriptor_results::descriptor_value_access(
                structural_types.iter(),
                self.structural_type,
                self.multiplicity,
            )
            .unwrap_or(self.access)
        }
    }

    pub(crate) fn source_signature(
        machine: &TerminalMachine,
        place: PlaceId,
    ) -> Option<SourceSignature<'_>> {
        if let Some(parameter) = machine.structural_parameter(place) {
            return Some(SourceSignature {
                structural_type: parameter.structural_type,
                access: parameter.access,
                multiplicity: parameter.multiplicity,
                qualifications: &parameter.qualifications,
                projected_qualifications: &parameter.projected_qualifications,
            });
        }
        if let Some(parameter) = block_parameter(machine, place) {
            return Some(SourceSignature {
                structural_type: parameter.structural_type,
                access: parameter.access,
                multiplicity: parameter.multiplicity,
                qualifications: &parameter.qualifications,
                projected_qualifications: &parameter.projected_qualifications,
            });
        }
        if let StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        } = machine.structural_place(place)?.kind
        {
            let mut establishments = machine.operations().filter_map(|operation| {
                let OperationKind::EstablishByteSequenceLiteral {
                    destination,
                    qualifications,
                    ..
                } = &operation.kind
                else {
                    return None;
                };
                (*destination == place && matches!(operation.result, OperationResult::Unit))
                    .then_some(qualifications)
            });
            let qualifications = establishments.next()?;
            return establishments.next().is_none().then_some(SourceSignature {
                structural_type,
                access: StructuralAccess::SharedBorrow,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications,
                projected_qualifications: &[],
            });
        }
        machine.operations().find_map(|operation| {
            operation
                .result
                .structural()
                .filter(|result| result.place == place)
                .map(|result| SourceSignature {
                    structural_type: result.structural_type,
                    access: StructuralAccess::Owned,
                    multiplicity: result.multiplicity,
                    qualifications: &result.qualifications,
                    projected_qualifications: &result.projected_qualifications,
                })
        })
    }

    pub(crate) fn block_parameter(
        machine: &TerminalMachine,
        place: PlaceId,
    ) -> Option<&StructuralParameterDeclaration> {
        let declaration = machine.structural_place(place)?;
        let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind else {
            return None;
        };
        machine
            .block(&block)?
            .structural_parameters
            .get(position as usize)
            .filter(|parameter| parameter.place == place && parameter.position == position)
    }
}

pub(crate) mod primitive_projection {
    use semantic_vocabulary::{ScalarType, StructuralTypeId, ValueId};
    use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration};

    pub(crate) fn leaf_type(
        structural_types: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        selector_type: impl Fn(ValueId) -> Option<ScalarType>,
    ) -> Option<ScalarType> {
        path.iter()
            .filter_map(StructuralPathSegment::runtime_index)
            .all(|(index, _)| runtime_selector(selector_type(index)))
            .then(|| {
                terminal_semantics::primitive_projection_type(structural_types.iter(), root, path)
            })
            .flatten()
    }

    pub(crate) fn read_leaf_type(
        structural_types: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        selector_type: impl Fn(ValueId) -> Option<ScalarType>,
    ) -> Option<ScalarType> {
        path.iter()
            .filter_map(StructuralPathSegment::runtime_index)
            .all(|(index, _)| runtime_selector(selector_type(index)))
            .then(|| {
                terminal_semantics::primitive_read_projection_type(
                    structural_types.iter(),
                    root,
                    path,
                )
            })
            .flatten()
    }

    pub(crate) fn runtime_selector(selector: Option<ScalarType>) -> bool {
        matches!(selector, Some(ScalarType::Integer(integer)) if integer.bits() <= 64)
    }
}

pub(crate) mod primitive_storage {
    use std::collections::BTreeMap;

    use abstract_operations::{AbstractOperation, AbstractResult};
    use semantic_vocabulary::{
        PlaceId, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId,
    };
    use terminal_psi::{
        Operation, OperationKind, StructuralAccess, StructuralMultiplicity,
        StructuralOperationResult, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    };

    use crate::lowering::LoweringError;

    use super::unique_row;

    pub(crate) fn parameter(
        machine: &TerminalMachine,
        place: PlaceId,
    ) -> Option<&terminal_psi::StructuralParameterDeclaration> {
        if let Some(parameter) = machine
            .structural_parameters
            .iter()
            .find(|row| row.place == place)
        {
            return Some(parameter);
        }
        let declaration = machine.structural_place(place)?;
        let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind else {
            return None;
        };
        let parameter = machine
            .block(&block)?
            .structural_parameters
            .get(position as usize)?;
        (parameter.place == place && parameter.position == position).then_some(parameter)
    }

    pub(crate) fn owned_state_storage_parameter(
        machine: &TerminalMachine,
        place: PlaceId,
    ) -> Option<(
        semantic_vocabulary::BlockId,
        &terminal_psi::StructuralParameterDeclaration,
    )> {
        let declaration = unique_row(&machine.structural_places, |row| row.id == place)?;
        let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind else {
            return None;
        };
        let parameter = machine
            .block(&block)?
            .structural_parameters
            .get(position as usize)?;
        (parameter.place == place
            && parameter.position == position
            && parameter.access == StructuralAccess::Owned)
            .then_some((block, parameter))
    }

    pub(crate) fn local(
        machine: &TerminalMachine,
        place: PlaceId,
    ) -> Option<&StructuralOperationResult> {
        let (operation, result) = owned_result(machine, place)?;
        (result.multiplicity == StructuralMultiplicity::Unrestricted
            && matches!(
                operation.kind,
                OperationKind::EstablishPrimitiveLocal { .. }
            ))
        .then_some(result)
    }

    fn owned_result(
        machine: &TerminalMachine,
        place: PlaceId,
    ) -> Option<(&Operation, &StructuralOperationResult)> {
        let declaration = unique_row(&machine.structural_places, |entry| entry.id == place)?;
        let (producer, structural_type) = declaration.kind.as_operation_result()?;
        let operation = unique_row(machine.operations(), |operation| operation.id == producer)?;
        let result = operation.result.structural()?;
        (result.place == place && result.structural_type == structural_type)
            .then_some((operation, result))
    }

    pub(crate) fn scalar_type(
        types: &[StructuralTypeDeclaration],
        identity: StructuralTypeId,
    ) -> Option<ScalarType> {
        types
            .iter()
            .find_map(|declaration| match declaration.shape {
                StructuralTypeShape::PrimitiveScalar(scalar_type) if declaration.id == identity => {
                    Some(scalar_type)
                }
                _ => None,
            })
    }

    pub(crate) fn lower(
        operation: &Operation,
        machine: &TerminalMachine,
        types: &[StructuralTypeDeclaration],
        values: &BTreeMap<ValueId, ScalarType>,
    ) -> Result<AbstractOperation, LoweringError> {
        match operation.kind {
            OperationKind::EstablishPrimitiveLocal {
                value,
                range_obligation,
            } => {
                let invalid = || LoweringError::InvalidPrimitiveLocalEstablishment(operation.id);
                let result = operation.result.structural().ok_or_else(invalid)?;
                if local(machine, result.place) != Some(result) {
                    return Err(invalid());
                }
                let scalar_type = scalar_type(types, result.structural_type).ok_or_else(invalid)?;
                if values.get(&value) != Some(&scalar_type) {
                    return Err(invalid());
                }
                Ok(AbstractOperation::EstablishPrimitiveLocal {
                    psi_operation: operation.id,
                    result: result.clone(),
                    value: AbstractResult { value, scalar_type },
                    range_obligation,
                })
            }
            OperationKind::PrimitiveScalarRead { source, ref path } => {
                let invalid = || LoweringError::InvalidPrimitiveScalarRead(operation.id);
                let result = operation.result.scalar().ok_or_else(invalid)?;
                let identity = owned_result(machine, source)
                    .map(|(_, result)| result.structural_type)
                    .or_else(|| {
                        parameter(machine, source)
                            .filter(|parameter| {
                                matches!(
                                    parameter.access,
                                    terminal_psi::reading_borrow_pair_pattern!()
                                ) || parameter.access == StructuralAccess::Owned
                            })
                            .map(|parameter| parameter.structural_type)
                    })
                    .ok_or_else(invalid)?;
                if super::primitive_projection::read_leaf_type(types, identity, path, |index| {
                    values.get(&index).copied()
                }) != Some(result.scalar_type)
                {
                    return Err(invalid());
                }
                Ok(AbstractOperation::PrimitiveScalarRead {
                    psi_operation: operation.id,
                    source,
                    path: path.clone(),
                    result: result.into(),
                })
            }
            _ => Err(LoweringError::InvalidPrimitiveLocalEstablishment(
                operation.id,
            )),
        }
    }
}

pub(crate) mod structural_establishment {
    use abstract_operations::AbstractOperation;
    use terminal_psi::{
        Block, Operation, OperationKind, StructuralTypeDeclaration, StructuralTypeShape,
        TerminalMachine, find_structural_type,
    };

    use super::{LoweredAffineLocal, StructuralLiteral};
    use crate::lowering::LoweringError;

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower(
        operation: &Operation,
        block: &Block,
        machine: &TerminalMachine,
        structural_types: &[StructuralTypeDeclaration],
        byte_sequence_literals: &[StructuralLiteral<'_>],
        unit_affine_locals: &[StructuralLiteral<'_>],
        lowered_unit_affine_locals: &mut Vec<LoweredAffineLocal>,
    ) -> Result<AbstractOperation, LoweringError> {
        Ok(match operation.kind.clone() {
            OperationKind::EstablishNativeCodeLayout { machine } => {
                let result =
                    super::structural_result(operation, LoweringError::UnsupportedScalarArray)?;
                AbstractOperation::EstablishNativeCodeLayout {
                    psi_operation: operation.id,
                    result,
                    machine,
                }
            }
            OperationKind::EstablishByteSequenceConcatenation {
                left,
                right,
                capacity_obligation,
            } => {
                let result = super::structural_result(
                    operation,
                    LoweringError::InvalidByteSequenceConcatenation,
                )?;
                AbstractOperation::EstablishByteSequenceConcatenation {
                    psi_operation: operation.id,
                    result,
                    left,
                    right,
                    capacity_obligation,
                }
            }
            OperationKind::EstablishStructuralArray { elements } => {
                let result =
                    super::structural_result(operation, LoweringError::UnsupportedScalarArray)?;
                AbstractOperation::EstablishStructuralArray {
                    psi_operation: operation.id,
                    result,
                    elements,
                }
            }
            OperationKind::EstablishScalarArray { elements } => {
                let result =
                    super::structural_result(operation, LoweringError::UnsupportedScalarArray)?;
                AbstractOperation::EstablishScalarArray {
                    psi_operation: operation.id,
                    result,
                    elements,
                }
            }
            OperationKind::EstablishScalarCase {
                result_case,
                fields,
            } => {
                let result =
                    super::structural_result(operation, LoweringError::UnsupportedPayloadlessCase)?;
                AbstractOperation::EstablishCase {
                    psi_operation: operation.id,
                    result,
                    result_case,
                    fields: fields
                        .into_iter()
                        .map(|field| terminal_psi::RecordFieldInitializer {
                            field: field.field,
                            value: terminal_psi::RecordFieldValue::Scalar {
                                value: field.value,
                                range_obligation: field.range_obligation,
                            },
                        })
                        .collect(),
                }
            }
            OperationKind::EstablishStructuralCase {
                result_case,
                fields,
            } => {
                let result =
                    super::structural_result(operation, LoweringError::UnsupportedStructuralCase)?;
                AbstractOperation::EstablishCase {
                    psi_operation: operation.id,
                    result,
                    result_case,
                    fields,
                }
            }
            OperationKind::EstablishByteSequenceLiteral {
                destination,
                bytes,
                qualifications,
                ..
            } => {
                let (place, _, structural_type) = byte_sequence_literals
                    .iter()
                    .find(|(place, _, _)| place.id == destination)
                    .copied()
                    .ok_or(LoweringError::UnsupportedByteSequenceLiteral(operation.id))?;
                let declaration = find_structural_type(structural_types, structural_type)
                    .cloned()
                    .ok_or(LoweringError::UnsupportedByteSequenceLiteral(operation.id))?;
                if !matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    )
                ) {
                    return Err(LoweringError::UnsupportedByteSequenceLiteral(operation.id));
                }
                AbstractOperation::EstablishByteSequenceLiteral {
                    psi_operation: operation.id,
                    place: *place,
                    structural_type: declaration,
                    bytes,
                    qualifications,
                }
            }
            OperationKind::EstablishTrivialAffineLocal { destination } => {
                let (place, ordinal, structural_type) = unit_affine_locals
                    .iter()
                    .find(|(place, _, _)| place.id == destination)
                    .copied()
                    .ok_or_else(|| unsupported_return(machine, block))?;
                let declaration = find_structural_type(structural_types, structural_type)
                    .cloned()
                    .ok_or_else(|| unsupported_return(machine, block))?;
                if usize::try_from(ordinal) != Ok(lowered_unit_affine_locals.len())
                    || !matches!(declaration.shape, StructuralTypeShape::Record { ref fields, .. } if fields.is_empty())
                {
                    return Err(unsupported_return(machine, block));
                }
                lowered_unit_affine_locals.push((operation.id, *place, declaration.clone()));
                AbstractOperation::EstablishTrivialAffineLocal {
                    psi_operation: operation.id,
                    place: *place,
                    structural_type: declaration,
                }
            }
            OperationKind::EstablishZeroedStructural { .. } => {
                let result = structural_result(operation, machine, block)?;
                AbstractOperation::EstablishZeroedStructural {
                    psi_operation: operation.id,
                    result,
                }
            }
            OperationKind::EstablishStructuralLocal { .. } => {
                let result = structural_result(operation, machine, block)?;
                AbstractOperation::EstablishStructuralLocal {
                    psi_operation: operation.id,
                    result,
                }
            }
            OperationKind::EstablishRecord { fields } => {
                let result = structural_result(operation, machine, block)?;
                AbstractOperation::EstablishRecord {
                    psi_operation: operation.id,
                    result,
                    fields,
                }
            }
            _ => unreachable!("structural-establishment router is exhaustive"),
        })
    }

    fn unsupported_return(machine: &TerminalMachine, block: &Block) -> LoweringError {
        LoweringError::UnsupportedStructuralReturn {
            machine: machine.id,
            edge: block
                .terminator
                .edges()
                .next()
                .expect("every terminator carries an edge"),
        }
    }

    fn structural_result(
        operation: &Operation,
        machine: &TerminalMachine,
        block: &Block,
    ) -> Result<terminal_psi::StructuralOperationResult, LoweringError> {
        operation
            .result
            .structural()
            .cloned()
            .ok_or_else(|| unsupported_return(machine, block))
    }
}

pub(crate) mod structural_leaf_copy {
    use abstract_operations::{AbstractOperation, StructuralCopyPath};
    use semantic_vocabulary::{
        IntegerSign, IntegerType, PlaceId, ScalarType, StructuralPlaceKind, StructuralTypeId,
    };
    use terminal_psi::{
        Operation, StructuralAccess, StructuralMultiplicity, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    };

    use crate::lowering::LoweringError;

    use super::borrowed_windows::{block_parameter, source_signature};

    pub(crate) fn lower(
        operation: &Operation,
        machine: &TerminalMachine,
        module: &terminal_psi::TerminalModule,
        structural_types: &[StructuralTypeDeclaration],
        source: PlaceId,
        path: StructuralCopyPath,
    ) -> Result<AbstractOperation, LoweringError> {
        let invalid = |reason| LoweringError::InvalidStructuralLeafCopy {
            machine: machine.id,
            operation: operation.id,
            reason,
        };
        let result = operation
            .result
            .structural()
            .ok_or_else(|| invalid("copy has no structural result"))?;
        let signature = source_signature(machine, source)
            .ok_or_else(|| invalid("copy source has no retained signature"))?;
        let shared_descriptor_copy = result.multiplicity == StructuralMultiplicity::Affine
            && terminal_semantics::is_copyable_carrier(
                structural_types.iter(),
                result.structural_type,
            );
        if (result.multiplicity != StructuralMultiplicity::Unrestricted && !shared_descriptor_copy)
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
            || !matches!(
                machine.structural_place(result.place)
                    .map(|place| place.kind),
                Some(StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                }) if producer == operation.id && structural_type == result.structural_type
            )
        {
            return Err(invalid("copy result is not a fresh copyable occurrence"));
        }
        if machine
            .structural_parameters
            .iter()
            .chain(block_parameter(machine, source))
            .find(|parameter| parameter.place == source)
            .is_some_and(|parameter| parameter.access == StructuralAccess::WriteOnlyBorrow)
        {
            return Err(invalid("copy source is write-only"));
        }
        let selected_type = match &path {
            StructuralCopyPath::Ordinary(path) => terminal_semantics::runtime_structural_copy_type(
                structural_types.iter(),
                signature.structural_type,
                path,
                Some(result.structural_type),
            ),
            StructuralCopyPath::Case(path) => terminal_semantics::canonical_case_leaf_type(
                structural_types.iter(),
                signature.structural_type,
                path,
            ),
        }
        .ok_or_else(|| invalid("copy path does not select a readable declared leaf"))?;
        let expected_type = result.structural_type;
        let occurrence_access = match &path {
            StructuralCopyPath::Ordinary(path) if path.is_empty() => {
                Some(signature.descriptor_access(structural_types))
            }
            _ => None,
        };
        if selected_type != expected_type
            && !terminal_semantics::shared_descriptor_copy_matches(
                structural_types.iter(),
                selected_type,
                occurrence_access,
                expected_type,
                result.multiplicity,
            )
            && terminal_semantics::quotients::quotient_copy_adaptation(module, machine, operation)
                .is_none()
            && !byte_carrier_leaf_copy(
                structural_types,
                signature.structural_type,
                &path,
                expected_type,
            )
        {
            return Err(invalid("copy result type does not match the selected leaf"));
        }
        Ok(AbstractOperation::StructuralLeafCopy {
            psi_operation: operation.id,
            result: result.clone(),
            source,
            path,
        })
    }

    fn byte_carrier_leaf_copy(
        structural_types: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
        path: &StructuralCopyPath,
        expected: StructuralTypeId,
    ) -> bool {
        let StructuralCopyPath::Ordinary(path) = path else {
            return false;
        };
        let Some((StructuralPathSegment::FixedIndex(index), prefix)) = path.split_last() else {
            return false;
        };
        let Some(parent) = terminal_semantics::runtime_structural_copy_type(
            structural_types.iter(),
            root,
            prefix,
            None,
        ) else {
            return false;
        };
        let Some(declaration) = terminal_psi::find_structural_type(structural_types, parent) else {
            return false;
        };
        let StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } = &declaration.shape
        else {
            return false;
        };
        let byte_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"));
        if !structural_types.iter().any(|candidate| {
            candidate.id == *element
                && candidate.shape == StructuralTypeShape::PrimitiveScalar(byte_type)
        }) {
            return false;
        };
        let Some(layout) =
            terminal_semantics::byte_carrier_scalar_layout(structural_types.iter(), expected)
        else {
            return false;
        };
        index
            .checked_add(layout.size)
            .is_some_and(|end| end <= *length)
    }
}

pub(crate) mod structural_scalar_fields {
    use std::collections::BTreeMap;

    use abstract_operations::{
        AbstractOperation, AbstractResult, StructuralScalarStoreDestination,
    };
    use semantic_vocabulary::{
        PlaceId, ScalarType, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
    };
    use terminal_psi::{
        BindingRelevance, Operation, OperationKind, OperationResult, StructuralAccess,
        StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
        StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
        find_structural_type,
    };

    use crate::lowering::LoweringError;

    use super::unique_row;

    pub(crate) fn lower(
        operation: &Operation,
        machine: &TerminalMachine,
        structural_types: &[StructuralTypeDeclaration],
        module: &terminal_psi::TerminalModule,
        value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
    ) -> Result<AbstractOperation, LoweringError> {
        match &operation.kind {
            OperationKind::StructuralScalarFieldStore {
                destination,
                path,
                field,
                value,
                range_obligation,
            } => lower_store(
                operation,
                machine,
                structural_types,
                module,
                value_types,
                *destination,
                path,
                *field,
                *value,
                *range_obligation,
            ),
            OperationKind::ScalarStructuralField {
                source,
                path,
                field,
            } => lower_scalar_read(operation, machine, structural_types, *source, path, *field),
            _ => unreachable!("structural scalar-field router is exhaustive"),
        }
    }

    pub(crate) fn store_destination(
        machine: &TerminalMachine,
        destination: PlaceId,
    ) -> Option<StructuralScalarStoreDestination> {
        exact_parameter(machine, destination)
            .or_else(|| {
                let (owner, position, parameter) = unique_row(
                    machine.blocks.iter().flat_map(|block| {
                        block
                            .structural_parameters
                            .iter()
                            .enumerate()
                            .map(move |(position, parameter)| (block.id, position, parameter))
                    }),
                    |(_, _, parameter)| parameter.place == destination,
                )?;
                let expected_kind = StructuralPlaceKind::BlockParameter {
                    block: owner,
                    position: parameter.position,
                };
                if owner == machine.entry
                    || parameter.position as usize != position
                    || !matches!(
                        parameter.access,
                        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                    )
                    || machine
                        .structural_parameters
                        .iter()
                        .any(|row| row.place == destination)
                {
                    return None;
                }
                let declaration =
                    unique_row(&machine.structural_places, |row| row.id == destination)?;
                if declaration.kind != expected_kind {
                    return None;
                }
                Some(parameter.clone())
            })
            .map(StructuralScalarStoreDestination::Parameter)
            .or_else(|| owned_storage_destination(machine, destination))
            .or_else(|| {
                let (block, parameter) =
                    super::primitive_storage::owned_state_storage_parameter(machine, destination)?;
                (block != machine.entry).then(|| {
                    StructuralScalarStoreDestination::OwnedBlockParameter {
                        block,
                        parameter: parameter.clone(),
                    }
                })
            })
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_store(
        operation: &Operation,
        machine: &TerminalMachine,
        structural_types: &[StructuralTypeDeclaration],
        module: &terminal_psi::TerminalModule,
        value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
        destination: PlaceId,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
        value: semantic_vocabulary::ValueId,
        range_obligation: Option<semantic_vocabulary::ObligationId>,
    ) -> Result<AbstractOperation, LoweringError> {
        let invalid = |reason| LoweringError::InvalidStructuralScalarFieldStore {
            machine: machine.id,
            operation: operation.id,
            reason,
        };
        let destination = store_destination(machine, destination)
            .or_else(|| owned_storage_destination(machine, destination))
            .ok_or_else(|| invalid("destination has no exact writable storage declaration"))?;
        let scalar_type = value_types
            .get(&value)
            .copied()
            .ok_or_else(|| invalid("stored value has no scalar declaration"))?;
        let (storage_type, storage_path) = if let Some(crossing) = path
            .iter()
            .rposition(|segment| *segment == StructuralPathSegment::Referent)
        {
            let suffix = &path[crossing + 1..];
            let carrier_type = destination.structural_type();
            let Some(StructuralTypeShape::Reference {
                referent,
                access: StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow,
            }) = terminal_semantics::runtime_structural_path_tip(
                structural_types.iter(),
                carrier_type,
                &path[..crossing],
            )
            .map(|declaration| &declaration.shape)
            else {
                return Err(invalid(
                    "referent crossing has no exclusive reference carrier",
                ));
            };
            if find_structural_type(structural_types, *referent).is_none()
                || !terminal_semantics::reference_source_projection_accessible(
                    structural_types.iter(),
                    carrier_type,
                    destination.multiplicity(),
                    destination
                        .parameter()
                        .map_or(StructuralAccess::Owned, |parameter| parameter.access),
                    path,
                    StructuralAccess::WriteOnlyBorrow,
                )
            {
                return Err(invalid("referent projection has no write authority"));
            }
            (*referent, suffix)
        } else {
            (destination.structural_type(), path)
        };
        if !terminal_semantics::scalar_field_store_preserves_domain_content(
            structural_types.iter(),
            module.structural_domains.iter(),
            &module.identities,
            destination.structural_type(),
            destination.qualifications(),
            destination.projected_qualifications(),
            path,
            field,
        ) {
            return Err(invalid(
                "field write changes a conserved domain Content definition",
            ));
        }
        if !terminal_semantics::scalar_field_store_preserves_content(
            structural_types.iter(),
            module.structural_domains.iter(),
            &module.identities,
            destination.structural_type(),
            destination.place(),
            machine.content_entry_claims.iter(),
            path,
            field,
        ) {
            return Err(invalid("field write changes conserved entry content"));
        }
        if operation.result != OperationResult::Unit
            || destination.parameter().is_some_and(|parameter| {
                !matches!(
                    parameter.access,
                    StructuralAccess::Owned
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                )
            })
            || !terminal_psi::is_structural_scalar_store_path(storage_path)
            || !path
                .iter()
                .filter_map(StructuralPathSegment::runtime_index)
                .all(|(index, _)| {
                    super::primitive_projection::runtime_selector(value_types.get(&index).copied())
                })
        {
            return Err(invalid(
                "store result, access, path or selector type is incompatible",
            ));
        }
        let declaration = terminal_semantics::projected_field_declaration(
            structural_types.iter(),
            storage_type,
            storage_path,
            field,
        )
        .ok_or_else(|| invalid("selected path has no exact field declaration"))?;
        if terminal_semantics::scalar_field_requires_evidence(&declaration.field_type)
            != range_obligation.is_some()
            || declaration.field_type.scalar_type() != Some(scalar_type)
        {
            return Err(invalid(
                "stored scalar type or receiving obligation differs from the field",
            ));
        }
        Ok(AbstractOperation::StructuralScalarFieldStore {
            psi_operation: operation.id,
            destination,
            path: path.to_vec(),
            field,
            value: AbstractResult { value, scalar_type },
            range_obligation,
        })
    }

    pub(crate) fn owned_storage_destination(
        machine: &TerminalMachine,
        place: PlaceId,
    ) -> Option<StructuralScalarStoreDestination> {
        let declaration = unique_row(&machine.structural_places, |row| row.id == place)?;
        let (producer, structural_type) = declaration.kind.as_operation_result()?;
        let operation = unique_row(machine.operations(), |operation| operation.id == producer)?;
        let result = operation.result.structural()?;
        (result.place == place
            && result.structural_type == structural_type
            && machine
                .content_entry_claims
                .iter()
                .all(|claim| claim.input.root != place))
        .then(|| StructuralScalarStoreDestination::OwnedResult {
            producer,
            result: result.clone(),
        })
    }

    fn lower_scalar_read(
        operation: &Operation,
        machine: &TerminalMachine,
        structural_types: &[StructuralTypeDeclaration],
        source: PlaceId,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        field: StructuralFieldId,
    ) -> Result<AbstractOperation, LoweringError> {
        let invalid = || LoweringError::InvalidScalarStructuralField(operation.id);
        let (structural_type, access) =
            readable_source_type(machine, structural_types, source).ok_or_else(invalid)?;
        let result = operation.result.scalar().ok_or_else(invalid)?;
        let (observed_root, access) = match find_structural_type(structural_types, structural_type)
            .map(|declaration| &declaration.shape)
        {
            Some(StructuralTypeShape::Reference { referent, access }) => (*referent, *access),
            _ => (structural_type, access),
        };
        let access = terminal_semantics::observed_field_backing_access(
            structural_types.iter(),
            observed_root,
            path,
            access,
        )
        .ok_or_else(invalid)?;
        let declaration = terminal_semantics::observed_field_declaration(
            structural_types.iter(),
            observed_root,
            path,
            field,
        )
        .ok_or_else(invalid)?;
        if matches!(
            declaration.field_type,
            StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_)
        ) && !matches!(access, terminal_psi::mutating_access_pair_pattern!())
        {
            return Err(invalid());
        }
        if observed_scalar_field(structural_types, observed_root, path, field)
            != Some(result.scalar_type)
        {
            return Err(invalid());
        }
        Ok(AbstractOperation::StructuralScalarFieldRead {
            psi_operation: operation.id,
            result: result.into(),
            source,
            path: path.to_vec(),
            field,
        })
    }

    fn readable_source_type(
        machine: &TerminalMachine,
        structural_types: &[StructuralTypeDeclaration],
        source: PlaceId,
    ) -> Option<(StructuralTypeId, StructuralAccess)> {
        let owned_storage = |structural_type, access| {
            access == StructuralAccess::Owned
                && structural_types.iter().any(|declaration| {
                    declaration.id == structural_type && (declaration.shape).is_composite_shape()
                })
        };
        let has_entry_claim = machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == source)
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == source);
        if let Some(parameter) = machine.parameter(source) {
            return (parameter.access != StructuralAccess::WriteOnlyBorrow
                && (owned_storage(parameter.structural_type, parameter.access)
                    || (parameter.multiplicity != StructuralMultiplicity::Linear
                        && !has_entry_claim)))
                .then_some((parameter.structural_type, parameter.access));
        }
        let result = machine
            .operations()
            .filter_map(|operation| operation.result.structural())
            .find(|result| result.place == source)?;
        (owned_storage(result.structural_type, StructuralAccess::Owned)
            || (result.multiplicity != StructuralMultiplicity::Linear
                && result.claims.is_empty()
                && !has_entry_claim))
            .then_some((result.structural_type, StructuralAccess::Owned))
    }

    pub(crate) fn exact_parameter(
        machine: &TerminalMachine,
        place: PlaceId,
    ) -> Option<StructuralParameterDeclaration> {
        let parameter = unique_row(&machine.structural_parameters, |parameter| {
            parameter.place == place
        })?;
        let declaration = unique_row(&machine.structural_places, |declaration| {
            declaration.id == place
        })?;
        matches!(
            declaration.kind,
            StructuralPlaceKind::Parameter { position } if position == parameter.position
        )
        .then(|| parameter.clone())
    }

    fn observed_scalar_field(
        structural_types: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        field: StructuralFieldId,
    ) -> Option<ScalarType> {
        if let Some(carrier) =
            terminal_semantics::case_payload_carrier(structural_types.iter(), root, path)
        {
            exact_structural_type(structural_types, carrier.sum)?;
            return unique_relevant_scalar_field(carrier.fields, field, true);
        }
        let carrier =
            terminal_semantics::record_field_carrier(structural_types.iter(), root, path)?;
        let declaration = exact_structural_type(structural_types, carrier.structural_type)?;
        let fields = declaration.shape.direct_fields()?;
        unique_relevant_scalar_field(fields, field, true)
    }

    fn unique_relevant_scalar_field(
        fields: &[terminal_psi::StructuralFieldDeclaration],
        field: StructuralFieldId,
        allow_refined_scalar: bool,
    ) -> Option<ScalarType> {
        let mut matching = fields.iter().filter(|candidate| {
            candidate.id == field && candidate.relevance == BindingRelevance::Relevant
        });
        let field = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        match &field.field_type {
            StructuralFieldType::Scalar(scalar_type) | StructuralFieldType::Atomic(scalar_type) => {
                Some(*scalar_type)
            }
            StructuralFieldType::AtomicBoundedInteger(bounds) => {
                Some(ScalarType::Integer(bounds.integer_type()))
            }
            StructuralFieldType::IeeeFloat(format) => Some(ScalarType::IeeeFloat(*format)),
            StructuralFieldType::BoundedInteger(bounds) if allow_refined_scalar => {
                Some(ScalarType::Integer(bounds.integer_type()))
            }
            StructuralFieldType::RefinedScalar(refinement) if allow_refined_scalar => {
                Some(refinement.carrier())
            }
            StructuralFieldType::ByteSequence(_)
            | StructuralFieldType::PredicateByteSequence { .. }
            | StructuralFieldType::RefinedScalar(_)
            | StructuralFieldType::BoundedInteger(_)
            | StructuralFieldType::Structural(_)
            | StructuralFieldType::Erased { .. } => None,
        }
    }

    fn exact_structural_type(
        structural_types: &[StructuralTypeDeclaration],
        structural_type: StructuralTypeId,
    ) -> Option<&StructuralTypeDeclaration> {
        let mut matching = structural_types
            .iter()
            .filter(|declaration| declaration.id == structural_type);
        let declaration = matching.next()?;
        matching.next().is_none().then_some(declaration)
    }
}
