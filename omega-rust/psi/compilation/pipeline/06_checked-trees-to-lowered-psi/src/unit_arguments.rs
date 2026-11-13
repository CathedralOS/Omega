/*
Call argument evaluation (`argument_evaluation`), cleanup control, state emission, and evaluation
scheduling (`argument_schedule`) for attached Unit machine bodies.
*/

pub(crate) use crate::unit::*;

pub(crate) mod attached_unit {
    pub(crate) use crate::unit::attached_unit::*;
    pub(crate) mod argument_evaluation {

        use super::super::{
            BlockId, QualifiedScalarType, StructuralAccess, StructuralParameterDeclaration,
            StructuralTypeDeclaration, ValueId, block_id, scalar_carriers,
        };
        use super::{
            Block, CheckedScalarExpressionRole, CheckedTrees, CheckedUnitEffectOperationPlan,
            LoweringError, OperationKind, OperationResult, PlaceId, ScalarType,
            StructuralMultiplicity, StructuralTypeShape, Terminator, ValueDeclaration,
            allocate_dense, direct_expression_contains_short_circuit, edge_id,
            emit_direct_expression, terminal_scalar_type, unsupported,
            validate_direct_parameter_types, value_id,
        };
        use crate::emission::affine_cleanup::structural_operands;
        use crate::emission::borrowed_window::terminal_multiplicity;
        use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
        use crate::emission::operation_emission::buffer::ComputedValueBinding;
        use crate::emission::operation_emission::buffer::{OperationBuffer, StructuralBinding};
        use crate::emission::operation_emission::calls::CallEmissionContext;
        use crate::emission::operation_emission::expressions::LoweredDirectExpression;
        use crate::emission::operation_emission::expressions::emit_scalar_leaf;
        use crate::emission::value_successor;
        use crate::expression_preparation::bindings::ScalarBindings;
        use crate::expression_preparation::bindings::StructuralLocalBinding;
        use crate::expression_preparation::bindings::StructuralScalarFieldBinding;
        use crate::expression_preparation::bindings::place_observations::PlaceObservation;
        use crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding;
        use crate::expression_preparation::bindings::structural_local_by_root;
        use crate::expression_preparation::bindings::structural_paths::terminal_access;
        use crate::expression_preparation::bindings::view_locals::ViewCarrier;
        use crate::expression_preparation::bindings::view_locals::ViewLocalBinding;
        use crate::expression_preparation::qualifications::PreparedScalarQualifications;
        use crate::expression_preparation::qualifications::received_scalar_type;
        use crate::scalar_lowering::branch_states::prepared::scalar_state_emission_order;
        use crate::scalar_lowering::contracts::LoweredProofTerm;
        use crate::scalar_lowering::contracts::{
            erased_proof_formal_declarations, lowered_proof_terms,
        };
        use crate::shared::checked_facts::value_facts;
        use crate::unit::attached_unit::OperationEnvironment;
        use checked_trees::CheckedCallScalarArgument;
        use state_emission::emit_state;

        pub(crate) mod cleanup_control {

            use super::{
                Evaluation, LoweringError, OperationBuffer, Terminator, ValueDeclaration,
                allocate_dense, block_id, declarations, edge_id,
            };
            use crate::scalar_lowering::contracts::erased_proof_formal_declarations;
            use crate::unit::unit_cleanup::CleanupRegion;
            use semantic_vocabulary::{BlockId, EdgeId, ProofTerm};
            use terminal_psi::{Block, OperationKind, SuccessorEdge, TerminalAffineCleanupAction};

            pub(crate) mod case_observations {

                use super::{
                    CleanupEmission, LoweringError, OperationBuffer, OperationKind,
                    TerminalAffineCleanupAction, ValueDeclaration,
                };

                impl CleanupEmission<'_> {
                    pub(crate) fn case_subject(
                        &self,
                        condition: &crate::unit::unit_cleanup::CleanupCaseCondition,
                        operations: &OperationBuffer,
                    ) -> Result<&terminal_psi::StructuralTypeDeclaration, LoweringError>
                    {
                        let mut declarations = self
                            .source_parameters
                            .iter()
                            .map(|(_, declaration)| declaration)
                            .chain(self.previous_parameter_homes.iter())
                            .chain(self.structural_parameters.iter())
                            .chain(
                                self.blocks
                                    .iter()
                                    .flat_map(|block| &block.structural_parameters),
                            )
                            .filter(|declaration| declaration.place == condition.place)
                            .map(|declaration| (declaration.structural_type, declaration.access))
                            .chain(
                                operations
                                    .iter()
                                    .chain(self.blocks.iter().flat_map(|block| &block.operations))
                                    .filter_map(|operation| operation.result.structural())
                                    .filter(|result| result.place == condition.place)
                                    .map(|result| {
                                        (
                                            result.structural_type,
                                            terminal_psi::StructuralAccess::Owned,
                                        )
                                    }),
                            );
                        let declaration = declarations.next().ok_or(LoweringError::Unsupported(
                            "retained cleanup case has no emitted root declaration",
                        ))?;
                        if declaration.1 != terminal_psi::StructuralAccess::Owned {
                            return Err(LoweringError::Unsupported(
                                "retained cleanup case has a borrowed root declaration",
                            ));
                        }
                        if declarations.any(|other| other != declaration) {
                            return Err(LoweringError::Unsupported(
                                "retained cleanup case has inconsistent root declarations",
                            ));
                        }
                        let subject = terminal_semantics::runtime_structural_path_tip(
                            self.structural_types.iter(),
                            declaration.0,
                            &condition.path,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "retained cleanup case has no exact declared path",
                        ))?;
                        if subject.id != condition.structural_type {
                            return Err(LoweringError::Unsupported(
                                "retained cleanup case disagrees with its emitted subject type",
                            ));
                        }
                        Ok(subject)
                    }

                    pub(crate) fn declared_single_case(
                        &self,
                        condition: &crate::unit::unit_cleanup::CleanupCaseCondition,
                        operations: &OperationBuffer,
                    ) -> bool {
                        self.case_subject(condition, operations)
                            .is_ok_and(|declaration| match &declaration.shape {
                                terminal_psi::StructuralTypeShape::Sum { cases }
                                | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => {
                                    cases.len() == 1 && cases[0].id == condition.case
                                }
                                _ => false,
                            })
                    }

                    /// Retain decisions whose sums contain an actual transferred argument.
                    /// Other cleanup regions keep their ordinary selected scope: in particular
                    /// an inactive sibling payload is never read ahead of its enclosing test.
                    pub(super) fn capture_transfer_cases(
                        &self,
                        regions: &[crate::unit::unit_cleanup::CleanupRegion],
                        arguments: &[terminal_psi::StructuralArgument],
                        next_value: &mut u64,
                        operations: &mut OperationBuffer,
                    ) -> Result<(), LoweringError> {
                        let owned = arguments
                            .iter()
                            .filter(|argument| {
                                argument.access == terminal_psi::StructuralAccess::Owned
                                    && !argument.path.is_empty()
                            })
                            .map(|argument| (argument.place, argument.path.as_slice()))
                            .collect::<std::collections::BTreeSet<_>>();
                        if owned.is_empty() {
                            return Ok(());
                        }
                        let mut seen = std::collections::BTreeSet::new();
                        for condition in regions.iter().flat_map(|region| &region.conditions) {
                            if !inline_case_path(&condition.path)
                                || !owned
                                    .range((condition.place, condition.path.as_slice())..)
                                    .next()
                                    .is_some_and(|(place, path)| {
                                        *place == condition.place
                                            && path.starts_with(&condition.path)
                                    })
                                || !seen.insert((
                                    condition.place,
                                    condition.path.as_slice(),
                                    condition.structural_type,
                                    condition.case,
                                ))
                            {
                                continue;
                            }
                            // The argument already names a readable selected descendant of
                            // this sum. Its exact declaration still owns the case geometry.
                            self.case_subject(condition, operations)?;
                            if !self.declared_single_case(condition, operations) {
                                self.observe_case(condition, next_value, operations)?;
                            }
                        }
                        Ok(())
                    }

                    pub(super) fn observe_case(
                        &self,
                        condition: &crate::unit::unit_cleanup::CleanupCaseCondition,
                        next_value: &mut u64,
                        operations: &mut OperationBuffer,
                    ) -> Result<ValueDeclaration, LoweringError> {
                        if let Some(observed) = self.retained_case(condition, operations) {
                            return Ok(observed);
                        }
                        let value = ValueDeclaration {
                            id: crate::shared::terminal_identities::value_id(
                                crate::shared::terminal_identities::allocate_dense(next_value)?,
                            ),
                            scalar_type: semantic_vocabulary::ScalarType::Boolean,
                            qualifications: Default::default(),
                        };
                        let producer = operations.push_leaf(
                            terminal_psi::OperationResult::Scalar(value),
                            OperationKind::StructuralCaseMembership {
                                source: condition.place,
                                path: condition.path.clone(),
                                case: condition.case,
                            },
                        );
                        operations.storage_observations.cases.push(
                crate::emission::operation_emission::buffer::CapturedStructuralCaseObservation {
                    source: condition.place,
                    path: condition.path.clone(),
                    structural_type: condition.structural_type,
                    case: condition.case,
                    value,
                    producer,
                },
            );
                        Ok(value)
                    }

                    /// Reuse the completed Boolean, never a tag load on the current payload.
                    /// Its scope supplies dominance. The append-only operation prefix also
                    /// authenticates the unchanged storage epoch, including effects emitted in
                    /// siblings before their observation scopes were restored.
                    pub(crate) fn retained_case(
                        &self,
                        condition: &crate::unit::unit_cleanup::CleanupCaseCondition,
                        operations: &OperationBuffer,
                    ) -> Option<ValueDeclaration> {
                        if !inline_case_path(&condition.path) {
                            return None;
                        }
                        self.case_subject(condition, operations).ok()?;
                        let captured = operations.storage_observations.cases.iter().rev().find(
                            |captured| {
                                captured.source == condition.place
                                    && captured.path == condition.path
                                    && captured.structural_type == condition.structural_type
                                    && captured.case == condition.case
                            },
                        )?;
                        let position = operations.iter().position(|operation| {
                            operation.id == captured.producer
                                && operation.result.scalar_ref() == Some(&captured.value)
                                && matches!(&operation.kind,
                                    OperationKind::StructuralCaseMembership { source, path, case }
                                    if *source == captured.source && *path == captured.path && *case == captured.case)
                        })?;
                        if !operations[position + 1..].iter().all(|operation| {
                            self.preserves_case_observation(operation, captured.source, operations)
                        }) {
                            return None;
                        }
                        for action in self.blocks.iter().flat_map(|block| {
                            block
                                .terminator
                                .cleanup_rosters()
                                .flat_map(|(_, actions)| actions)
                        }) {
                            match action {
                                TerminalAffineCleanupAction::RetireHome(place)
                                | TerminalAffineCleanupAction::DiscardRoot(place)
                                    if *place == captured.source =>
                                {
                                    return None;
                                }
                                TerminalAffineCleanupAction::DiscardResidual(discard)
                                    if discard.place == captured.source
                                        && captured.path.starts_with(&discard.path) =>
                                {
                                    return None;
                                }
                                TerminalAffineCleanupAction::InvokeNominal(_) => return None,
                                _ => {}
                            }
                        }
                        Some(captured.value)
                    }

                    fn preserves_case_observation(
                        &self,
                        operation: &terminal_psi::Operation,
                        source: semantic_vocabulary::PlaceId,
                        operations: &OperationBuffer,
                    ) -> bool {
                        if matches!(operation.kind, OperationKind::AtomicAccess { .. }) {
                            return false;
                        }
                        if terminal_semantics::is_unconditionally_total_scalar(&operation.kind) {
                            return true;
                        }
                        match &operation.kind {
                            OperationKind::StructuralScalarFieldStore {
                                destination, path, ..
                            }
                            | OperationKind::WriteOnlyPrimitiveStore {
                                destination, path, ..
                            } if inline_case_path(path)
                                && self.fresh_inline_home(*destination, operations) =>
                            {
                                true
                            }
                            OperationKind::StoreStructuralField {
                                destination, path, ..
                            } if *destination != source
                                && inline_case_path(path)
                                && self.fresh_inline_home(*destination, operations) =>
                            {
                                true
                            }
                            _ => {
                                let Ok(row) =
                                    terminal_semantics::operation_semantic_row(&operation.kind)
                                else {
                                    return false;
                                };
                                if row.custody()
                                    == terminal_semantics::OperationSemanticCustody::CallComposition
                                {
                                    return false;
                                }
                                let mut observed = false;
                                let mut readonly = true;
                                operation.kind.clone().map_structural_uses_with_access(
                                    &mut |place, access| {
                                        observed = true;
                                        readonly &=
                                            access == terminal_psi::StructuralAccess::SharedBorrow;
                                        place
                                    },
                                );
                                observed && readonly
                            }
                        }
                    }

                    fn fresh_inline_home(
                        &self,
                        place: semantic_vocabulary::PlaceId,
                        operations: &OperationBuffer,
                    ) -> bool {
                        let mut producers = operations.iter().filter_map(|operation| {
                            let result = operation.result.structural()?;
                            (result.place == place).then_some((operation, result))
                        });
                        let Some((producer, result)) = producers.next() else {
                            return false;
                        };
                        if producers.next().is_some()
                            || !matches!(
                                producer.kind,
                                OperationKind::EstablishRecord { .. }
                                    | OperationKind::EstablishStructuralCase { .. }
                                    | OperationKind::EstablishPrimitiveLocal { .. }
                                    | OperationKind::EstablishStructuralLocal { .. }
                                    | OperationKind::EstablishScalarArray { .. }
                                    | OperationKind::EstablishStructuralArray { .. }
                            )
                        {
                            return false;
                        }
                        terminal_psi::find_structural_type(
                            self.structural_types,
                            result.structural_type,
                        )
                        .is_some_and(|declaration| {
                            matches!(
                                declaration.shape,
                                terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
                                    | terminal_psi::StructuralTypeShape::Record { .. }
                                    | terminal_psi::StructuralTypeShape::FixedArray { .. }
                                    | terminal_psi::StructuralTypeShape::Sum { .. }
                                    | terminal_psi::StructuralTypeShape::Mixed { .. }
                            )
                        })
                    }
                }

                fn inline_case_path(path: &[terminal_psi::StructuralPathSegment]) -> bool {
                    path.iter().all(|segment| {
                        matches!(
                            segment,
                            terminal_psi::StructuralPathSegment::Field(_)
                                | terminal_psi::StructuralPathSegment::FixedIndex(_)
                                | terminal_psi::StructuralPathSegment::Case(_)
                        )
                    })
                }
            }

            pub(crate) struct CleanupEmission<'a> {
                pub(crate) structural_types: &'a [terminal_psi::StructuralTypeDeclaration],
                pub(crate) source_parameters:
                    &'a [(u32, terminal_psi::StructuralParameterDeclaration)],
                pub(crate) previous_parameter_homes:
                    &'a [terminal_psi::StructuralParameterDeclaration],
                pub(crate) checked: &'a checked_trees::CheckedTrees,
                pub(crate) current: &'a mut BlockId,
                pub(crate) parameters: &'a mut Vec<ValueDeclaration>,
                pub(crate) structural_parameters:
                    &'a mut Vec<terminal_psi::StructuralParameterDeclaration>,
                pub(crate) block_erased_scalar_formals: &'a mut Vec<ValueDeclaration>,
                pub(crate) erased_proof_formals:
                    &'a [checked_trees::CheckedErasedProofParameterPlan],
                pub(crate) operation_start: &'a mut usize,
                pub(crate) blocks: &'a mut Vec<Block>,
            }

            impl Evaluation {
                pub(crate) fn cleanup_emission<'a>(
                    &'a mut self,
                    checked: &'a checked_trees::CheckedTrees,
                    block_erased_scalar_formals: &'a mut Vec<ValueDeclaration>,
                ) -> CleanupEmission<'a> {
                    CleanupEmission {
                        structural_types: &self.structural_types,
                        source_parameters: &self.structural_parameters,
                        previous_parameter_homes: &self.previous_parameter_homes,
                        checked,
                        current: &mut self.current,
                        parameters: &mut self.parameters,
                        structural_parameters: &mut self.block_structural_parameters,
                        block_erased_scalar_formals,
                        erased_proof_formals: &self.erased_proof_formals,
                        operation_start: &mut self.operation_start,
                        blocks: &mut self.blocks,
                    }
                }

                pub(crate) fn carrier_multiplicity(
                    &self,
                    place: semantic_vocabulary::PlaceId,
                    operations: &OperationBuffer,
                ) -> Result<terminal_psi::StructuralMultiplicity, LoweringError> {
                    self.emitted_carrier(place, operations)
                        .map(|(_, multiplicity)| multiplicity)
                }

                pub(crate) fn emitted_carrier(
                    &self,
                    place: semantic_vocabulary::PlaceId,
                    operations: &OperationBuffer,
                ) -> Result<
                    (
                        semantic_vocabulary::StructuralTypeId,
                        terminal_psi::StructuralMultiplicity,
                    ),
                    LoweringError,
                > {
                    let mut carriers = self
                        .emitted_carriers(operations)
                        .filter(|(carrier_place, _, _)| *carrier_place == place)
                        .map(|(_, structural_type, multiplicity)| (structural_type, multiplicity));
                    let carrier = carriers.next().ok_or(LoweringError::Unsupported(
                        "descriptor retirement lost its emitted carrier declaration",
                    ))?;
                    if carriers.any(|other| other != carrier) {
                        return Err(LoweringError::Unsupported(
                            "descriptor retirement has inconsistent carrier declarations",
                        ));
                    }
                    Ok(carrier)
                }

                fn emitted_carriers<'a>(
                    &'a self,
                    operations: &'a OperationBuffer,
                ) -> impl Iterator<
                    Item = (
                        semantic_vocabulary::PlaceId,
                        semantic_vocabulary::StructuralTypeId,
                        terminal_psi::StructuralMultiplicity,
                    ),
                > + 'a {
                    self.structural_root_parameters()
                        .map(|parameter| {
                            (
                                parameter.place,
                                parameter.structural_type,
                                parameter.multiplicity,
                            )
                        })
                        .chain(self.structural_value_owners.iter().map(|owner| {
                            (
                                owner.value.place,
                                owner.value.structural_type,
                                owner.value.multiplicity,
                            )
                        }))
                        .chain(
                            operations
                                .iter()
                                .chain(self.blocks.iter().flat_map(|block| block.operations.iter()))
                                .filter_map(|operation| operation.result.structural())
                                .map(|result| {
                                    (result.place, result.structural_type, result.multiplicity)
                                }),
                        )
                        .chain(
                            operations
                                .structural_values
                                .iter()
                                .filter(|(_, binding)| binding.path().is_empty())
                                .map(|(_, binding)| {
                                    (
                                        binding.place(),
                                        binding.structural_type(),
                                        binding.multiplicity(),
                                    )
                                }),
                        )
                        .chain(
                            operations
                                .literal_types
                                .iter()
                                .map(|&(place, structural_type)| {
                                    (
                                        place,
                                        structural_type,
                                        terminal_psi::StructuralMultiplicity::Unrestricted,
                                    )
                                }),
                        )
                }

                pub(crate) fn completed_carrier_inventory(
                    &self,
                    operations: &OperationBuffer,
                ) -> Result<
                    Vec<(
                        semantic_vocabulary::PlaceId,
                        terminal_psi::StructuralMultiplicity,
                    )>,
                    LoweringError,
                > {
                    let mut carriers = self.emitted_carriers(operations).collect::<Vec<_>>();
                    carriers.sort_unstable_by_key(|(place, _, _)| *place);
                    let mut inventory = Vec::with_capacity(carriers.len());
                    let mut remaining = carriers.as_slice();
                    while let Some(&(place, structural_type, multiplicity)) = remaining.first() {
                        let end =
                            remaining.partition_point(|(candidate, _, _)| *candidate == place);
                        if remaining[..end].iter().any(
                            |(_, candidate_type, candidate_multiplicity)| {
                                *candidate_type != structural_type
                                    || *candidate_multiplicity != multiplicity
                            },
                        ) {
                            return Err(LoweringError::Unsupported(
                                "descriptor retirement has inconsistent carrier declarations",
                            ));
                        }
                        inventory.push((place, multiplicity));
                        remaining = &remaining[end..];
                    }
                    Ok(inventory)
                }

                pub(crate) fn descriptor_retirement(
                    &self,
                    place: semantic_vocabulary::PlaceId,
                    operations: &OperationBuffer,
                ) -> Result<TerminalAffineCleanupAction, LoweringError> {
                    match self.carrier_multiplicity(place, operations)? {
                        terminal_psi::StructuralMultiplicity::Unrestricted => {
                            Ok(TerminalAffineCleanupAction::RetireHome(place))
                        }
                        terminal_psi::StructuralMultiplicity::Affine => {
                            Ok(TerminalAffineCleanupAction::DiscardRoot(place))
                        }
                        terminal_psi::StructuralMultiplicity::Linear => {
                            Err(LoweringError::Unsupported(
                                "descriptor retirement cannot discharge linear custody",
                            ))
                        }
                    }
                }

                pub(crate) fn normal_discard_regions(
                    &self,
                    place: semantic_vocabulary::PlaceId,
                    types: &[terminal_psi::StructuralTypeDeclaration],
                    operations: &OperationBuffer,
                ) -> Result<Vec<CleanupRegion>, LoweringError> {
                    let moved = self
                        .selection_cleanups
                        .iter()
                        .flat_map(|cleanup| &cleanup.retired_reference_paths)
                        .filter(|(source, _)| *source == place)
                        .map(|(_, path)| path.clone())
                        .collect::<std::collections::BTreeSet<_>>();
                    if moved.is_empty() {
                        return Ok(vec![CleanupRegion::unconditional(
                            self.descriptor_retirement(place, operations)?,
                        )]);
                    }
                    let (root_type, _) = self.emitted_carrier(place, operations)?;
                    if moved.iter().any(Vec::is_empty) {
                        if moved.len() != 1 {
                            return Err(LoweringError::Unsupported(
                                "selected complement retirement overlaps its whole root",
                            ));
                        }
                        return Ok(vec![CleanupRegion::unconditional(
                            TerminalAffineCleanupAction::RetireHome(place),
                        )]);
                    }
                    let residuals = terminal_semantics::partial_affine_residuals(
                        types,
                        root_type,
                        &moved,
                        usize::MAX,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "selected complement death has no exact remaining partition",
                    ))?;
                    let mut regions = Vec::new();
                    for (path, structural_type) in residuals {
                        let mut conditions = Vec::new();
                        for (position, segment) in path.iter().enumerate() {
                            let terminal_psi::StructuralPathSegment::Case(case) = segment else {
                                continue;
                            };
                            let sum = terminal_semantics::runtime_structural_path_tip(
                                types.iter(),
                                root_type,
                                &path[..position],
                            )
                            .ok_or(LoweringError::Unsupported(
                                "selected complement remaining case lost its carrier",
                            ))?;
                            conditions.push(crate::unit::unit_cleanup::CleanupCaseCondition {
                                place,
                                path: path[..position].to_vec(),
                                structural_type: sum.id,
                                case: *case,
                            });
                        }
                        regions.push(CleanupRegion {
                            partition_leaf: None,
                            declaration_order: None,
                            selected_region: None,
                            conditions,
                            action: crate::unit::unit_cleanup::CleanupRegionAction::Run(
                                TerminalAffineCleanupAction::DiscardResidual(
                                    terminal_psi::StructuralAffineDiscard {
                                        place,
                                        path,
                                        structural_type,
                                    },
                                ),
                            ),
                        });
                    }
                    regions.push(CleanupRegion::unconditional(
                        TerminalAffineCleanupAction::RetireHome(place),
                    ));
                    Ok(regions)
                }

                pub(crate) fn capture_structural_arguments(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &checked_trees::CheckedTrees,
                    arguments: &mut Vec<terminal_psi::StructuralArgument>,
                    declarations: &[terminal_psi::StructuralParameterDeclaration],
                    regions: &[CleanupRegion],
                    values: &mut Vec<ValueDeclaration>,
                    next_place: &mut u64,
                    places: &mut Vec<terminal_psi::StructuralPlaceDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                ) -> Result<(), LoweringError> {
                    let mut source_names = Vec::with_capacity(arguments.len());
                    for argument in arguments.iter() {
                        source_names.push(
                            if argument.access == terminal_psi::StructuralAccess::Owned {
                                Some(self.current_claims.source(argument)?.ok_or_else(|| {
                                    LoweringError::MissingCapturedArgumentNamespace {
                                        block: self.current,
                                        argument: argument.clone(),
                                        last_operation: operations
                                            .operations
                                            .last()
                                            .map(|operation| operation.id),
                                    }
                                })?)
                            } else {
                                None
                            },
                        );
                    }
                    let arrivals = self
                        .cleanup_emission(checked, &mut Vec::new())
                        .capture_arguments(
                            identities,
                            arguments,
                            declarations,
                            regions,
                            values,
                            next_place,
                            places,
                            next_value,
                            next_block,
                            next_edge,
                            operations,
                        )?;
                    for (place, names) in arrivals.into_iter().zip(source_names) {
                        if let Some(names) = names {
                            self.current_claims.publish(place, names)?;
                        }
                    }
                    Ok(())
                }

                pub(crate) fn emit_cleanup_regions(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &checked_trees::CheckedTrees,
                    regions: Vec<CleanupRegion>,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                ) -> Result<Vec<TerminalAffineCleanupAction>, LoweringError> {
                    if regions.iter().all(|region| {
                        region.partition_leaf.is_none()
                            && region.conditions.is_empty()
                            && region.selected_region.is_none()
                    }) {
                        return Ok(regions
                            .into_iter()
                            .filter_map(|region| region.action.into_physical())
                            .collect());
                    }
                    self.continue_cleanup_regions(
                        identities, checked, regions, values, next_value, next_block, next_edge,
                        operations,
                    )?;
                    Ok(Vec::new())
                }

                pub(crate) fn continue_cleanup_regions(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &checked_trees::CheckedTrees,
                    regions: Vec<CleanupRegion>,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                ) -> Result<(), LoweringError> {
                    self.cleanup_emission(checked, &mut Vec::new()).emit(
                        identities, &regions, values, next_value, next_block, next_edge, operations,
                    )
                }
            }

            impl CleanupEmission<'_> {
                pub(crate) fn capture_arguments(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    arguments: &mut Vec<terminal_psi::StructuralArgument>,
                    declarations: &[terminal_psi::StructuralParameterDeclaration],
                    regions: &[CleanupRegion],
                    values: &mut Vec<ValueDeclaration>,
                    next_place: &mut u64,
                    places: &mut Vec<terminal_psi::StructuralPlaceDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                ) -> Result<Vec<semantic_vocabulary::PlaceId>, LoweringError> {
                    if arguments.len() != declarations.len() {
                        return Err(LoweringError::Unsupported(
                            "cleanup capture has no exact structural roster",
                        ));
                    }
                    if arguments.is_empty() {
                        return Ok(Vec::new());
                    }
                    // All authored operands have completed. Capture the required case
                    // decisions while the original owner is whole, before this edge moves
                    // its selected children into the cleanup continuation.
                    self.capture_transfer_cases(regions, arguments, next_value, operations)?;
                    let target = block_id(allocate_dense(next_block)?);
                    let parameters = super::declarations(
                        &values
                            .iter()
                            .map(|value| value.value_type())
                            .collect::<Vec<_>>(),
                        next_value,
                    )?;
                    let mut captures = Vec::with_capacity(arguments.len());
                    let mut forwarded = Vec::with_capacity(arguments.len());
                    for (ordinal, (argument, declaration)) in
                        arguments.iter().zip(declarations).enumerate()
                    {
                        let position = u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported(
                                "cleanup capture structural roster exceeds u32",
                            )
                        })?;
                        let place = crate::shared::terminal_identities::place_id(allocate_dense(
                            next_place,
                        )?);
                        places.push(terminal_psi::StructuralPlaceDeclaration {
                            id: place,
                            kind: semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                                block: target,
                                position,
                            },
                        });
                        let mut capture = declaration.clone();
                        capture.place = place;
                        capture.position = position;
                        capture.access = argument.access;
                        captures.push(capture);
                        forwarded.push(terminal_psi::StructuralArgument {
                            place,
                            path: Vec::new(),
                            access: argument.access,
                        });
                    }
                    self.cleanup_jump(
                        identities,
                        target,
                        &parameters,
                        Vec::new(),
                        std::mem::take(arguments),
                        values,
                        next_edge,
                        operations,
                    )?;
                    let arrivals = forwarded.iter().map(|argument| argument.place).collect();
                    *self.structural_parameters = captures;
                    *arguments = forwarded;
                    Ok(arrivals)
                }

                pub(crate) fn emit(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    regions: &[CleanupRegion],
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                ) -> Result<(), LoweringError> {
                    let mut position = 0;
                    while let Some(first) = regions.get(position) {
                        if let Some(leaf) = &first.partition_leaf {
                            let end = position.checked_add(leaf.leaf_count).ok_or(
                                LoweringError::InvalidCleanupPartition {
                                    reason: "cleanup partition action span overflows",
                                },
                            )?;
                            let selected = regions.get(position..end).ok_or(
                                LoweringError::InvalidCleanupPartition {
                                    reason: "cleanup partition action span is truncated",
                                },
                            )?;
                            if selected.iter().enumerate().any(|(ordinal, region)| {
                                region.partition_leaf.as_ref().is_none_or(|candidate| {
                                    candidate.owner != leaf.owner
                                        || candidate.ordinal as usize != ordinal
                                })
                            }) {
                                return Err(LoweringError::InvalidCleanupPartition {
                                    reason: "cleanup partition actions are interleaved or reordered",
                                });
                            }
                            let selected = if leaf.leaf_count == 0 {
                                &[][..]
                            } else {
                                selected
                            };
                            let recorded = operations.completed_cleanup_frontiers.len();
                            self.partition_node(
                                identities, &leaf.root, selected, None, 0, values, next_value,
                                next_block, next_edge, operations,
                            )?;
                            position = if leaf.leaf_count == 0 {
                                position + 1
                            } else {
                                end
                            };
                            // One root can be split between partitions: the fields a
                            // conditional move left behind and the moved field itself.
                            // A scope this tree closed is not finished while a later
                            // action of the sequence still disposes storage inside it,
                            // so it must not be recorded as a completed value.
                            let later = &regions[position..];
                            let unfinished = |completion: &crate::emission::selected_continuations::CompletedCleanupFrontier| {
                                later.iter().any(|region| {
                                    let inside = |place, path: &[terminal_psi::StructuralPathSegment]| {
                                        place == completion.region.place
                                            && path.starts_with(&completion.region.path)
                                    };
                                    // Retiring the home follows its completed value.
                                    let retires = matches!(
                                        region.action.physical(),
                                        Some(TerminalAffineCleanupAction::RetireHome(_))
                                    );
                                    let selected = region
                                        .selected_region
                                        .as_ref()
                                        .is_some_and(|selected| inside(selected.place, &selected.path));
                                    !retires
                                        && (selected
                                            || inside(region.action.place(), region.action.path()))
                                })
                            };
                            let fresh = operations.completed_cleanup_frontiers.split_off(recorded);
                            operations.completed_cleanup_frontiers.extend(
                                fresh
                                    .into_iter()
                                    .filter(|completion| !unfinished(completion)),
                            );
                        } else {
                            let end = position
                                + regions[position..]
                                    .iter()
                                    .take_while(|region| region.partition_leaf.is_none())
                                    .count();
                            self.cleanup_scope(
                                identities,
                                &regions[position..end],
                                0,
                                None,
                                values,
                                next_value,
                                next_block,
                                next_edge,
                                operations,
                            )?;
                            position = end;
                        }
                    }
                    Ok(())
                }

                #[allow(clippy::too_many_arguments)]
                fn completed_partition_edge(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    place: semantic_vocabulary::PlaceId,
                    path: &[terminal_psi::StructuralPathSegment],
                    kind: crate::emission::selected_continuations::CompletedCleanupKind,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                ) -> Result<(), LoweringError> {
                    let edge = self.continue_actions(
                        identities,
                        Vec::new(),
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                    )?;
                    operations.completed_cleanup_frontiers.push(
                        crate::emission::selected_continuations::CompletedCleanupFrontier {
                            edge,
                            region: terminal_psi::StructuralArgument {
                                place,
                                path: path.to_vec(),
                                access: terminal_psi::StructuralAccess::Owned,
                            },
                            kind,
                        },
                    );
                    Ok(())
                }

                #[allow(clippy::too_many_arguments)]
                fn partition_node(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    node: &crate::unit::unit_cleanup::CleanupPartitionNode,
                    leaves: &[CleanupRegion],
                    selected_scope: Option<&terminal_psi::StructuralArgument>,
                    depth: usize,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                ) -> Result<(), LoweringError> {
                    use crate::unit::unit_cleanup::CleanupPartitionNode as Node;
                    match node {
                        Node::Leaf(ordinal) => {
                            let region =
                                leaves
                                    .get(*ordinal as usize)
                                    .ok_or(LoweringError::InvalidCleanupPartition { reason: "cleanup partition leaf exceeds its ordered action span" })?;
                            self.cleanup_scope(
                                identities,
                                std::slice::from_ref(region),
                                depth,
                                selected_scope,
                                values,
                                next_value,
                                next_block,
                                next_edge,
                                operations,
                            )
                        }
                        Node::Scope {
                            place,
                            path,
                            steps,
                            leaf_span,
                            ..
                        } => {
                            // This checked scope has no cleanup actions. Retain its
                            // completed-value record without testing absent children.
                            if leaf_span.is_empty() {
                                return self.completed_partition_edge(
                                    identities,
                                    *place,
                                    path,
                                    crate::emission::selected_continuations::CompletedCleanupKind::Value,
                                    values,
                                    next_value,
                                    next_block,
                                    next_edge,
                                    operations,
                                );
                            }
                            let candidate = leaves
                                .get(leaf_span.start)
                                .and_then(|leaf| leaf.selected_region.as_ref())
                                .filter(|region| region.place == *place && region.path == *path);
                            let bypass = if let Some(region) =
                                candidate.filter(|candidate| Some(*candidate) != selected_scope)
                            {
                                Some(self.select_storage(
                                    identities, region, false, values, next_value, next_block,
                                    next_edge, operations,
                                )?)
                            } else {
                                None
                            };
                            let scope = if bypass.is_some() {
                                candidate
                            } else {
                                selected_scope
                            };
                            for step in steps {
                                self.partition_node(
                                    identities, step, leaves, scope, depth, values, next_value,
                                    next_block, next_edge, operations,
                                )?;
                            }
                            let whole_leaf = match steps.as_slice() {
                                [Node::Leaf(ordinal)] => {
                                    leaves.get(*ordinal as usize).is_some_and(|region| {
                                        region.action.place() == *place
                                            && region.action.path() == path
                                            && region.action.physical().is_some_and(|action| {
                                                match action {
                                                    TerminalAffineCleanupAction::InvokeNominal(
                                                        cleanup,
                                                    ) => !cleanup.retains_descendants,
                                                    _ => true,
                                                }
                                            })
                                    })
                                }
                                _ => false,
                            };
                            if !whole_leaf {
                                self.completed_partition_edge(identities, *place, path, crate::emission::selected_continuations::CompletedCleanupKind::Value, values, next_value, next_block, next_edge, operations)?;
                            }
                            if let Some((bypass, parameters)) = bypass {
                                self.cleanup_jump(
                                    identities,
                                    bypass,
                                    &parameters,
                                    Vec::new(),
                                    Vec::new(),
                                    values,
                                    next_edge,
                                    operations,
                                )?;
                            }
                            Ok(())
                        }
                        Node::Cases {
                            place,
                            path,
                            structural_type,
                            arms,
                            common,
                            leaf_span,
                        } => self.partition_cases(
                            identities,
                            *place,
                            path,
                            *structural_type,
                            arms,
                            common,
                            leaves,
                            leaf_span.clone(),
                            selected_scope,
                            depth,
                            values,
                            next_value,
                            next_block,
                            next_edge,
                            operations,
                        ),
                    }
                }

                #[allow(clippy::too_many_arguments)]
                fn partition_cases(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    place: semantic_vocabulary::PlaceId,
                    path: &[terminal_psi::StructuralPathSegment],
                    structural_type: semantic_vocabulary::StructuralTypeId,
                    arms: &[crate::unit::unit_cleanup::CleanupPartitionArm],
                    common: &[crate::unit::unit_cleanup::CleanupPartitionNode],
                    leaves: &[CleanupRegion],
                    leaf_span: std::ops::Range<usize>,
                    selected_scope: Option<&terminal_psi::StructuralArgument>,
                    depth: usize,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                ) -> Result<(), LoweringError> {
                    use checked_trees::CheckedCleanupCaseReachability as Reachability;
                    // The full checked subtree contains no cleanup action. Its
                    // payloads are complete without another case-tag read.
                    if leaf_span.is_empty() {
                        return self.completed_partition_edge(
                            identities,
                            place,
                            path,
                            crate::emission::selected_continuations::CompletedCleanupKind::CasePayloads(
                                arms.iter().map(|arm| arm.case).collect(),
                            ),
                            values,
                            next_value,
                            next_block,
                            next_edge,
                            operations,
                        );
                    }
                    let reachable = arms
                        .iter()
                        .filter(|arm| arm.reachability != Reachability::Inactive)
                        .collect::<Vec<_>>();
                    if reachable.is_empty() {
                        return Err(LoweringError::InvalidCleanupCasePartition {
                            reason: "cleanup partition has no reachable alternative",
                            place,
                            path: path.to_vec(),
                            structural_type,
                            leaf_span: leaf_span.clone(),
                            declared_cases: arms.len(),
                            common_scopes: common.len(),
                        });
                    }
                    let condition = crate::unit::unit_cleanup::CleanupCaseCondition {
                        place,
                        path: path.to_vec(),
                        structural_type,
                        case: reachable[0].case,
                    };
                    self.case_subject(&condition, operations)?;
                    let observation = terminal_psi::StructuralArgument {
                        place,
                        path: path.to_vec(),
                        access: terminal_psi::StructuralAccess::Owned,
                    };
                    let relevant = leaves
                        .get(leaf_span)
                        .ok_or(LoweringError::InvalidCleanupPartition {
                            reason: "cleanup case leaves exceed their owning action span",
                        })?
                        .iter()
                        .filter(|leaf| {
                            leaf.conditions
                                .iter()
                                .any(|candidate| candidate.place == place && candidate.path == path)
                        })
                        .collect::<Vec<_>>();
                    let selected = !relevant.is_empty()
                        && relevant.iter().all(|leaf| {
                            leaf.selected_region.as_ref().is_some_and(|source| {
                                source.place == place
                                    && (source.path.starts_with(path)
                                        || path.starts_with(&source.path))
                            })
                        });
                    let bypass = if selected
                        && selected_scope != Some(&observation)
                        && !self.declared_single_case(&condition, operations)
                        && self.retained_case(&condition, operations).is_none()
                    {
                        Some(self.select_storage(
                            identities,
                            &observation,
                            false,
                            values,
                            next_value,
                            next_block,
                            next_edge,
                            operations,
                        )?)
                    } else {
                        None
                    };
                    // A case observation may add a bypass, but it does not replace
                    // the active owner selected around the containing cleanup.
                    let scope = selected_scope;
                    let join = block_id(allocate_dense(next_block)?);
                    let join_parameters = declarations(
                        &values
                            .iter()
                            .map(|value| value.value_type())
                            .collect::<Vec<_>>(),
                        next_value,
                    )?;
                    let mut common_observation_count = None;
                    for (position, arm) in reachable.iter().enumerate() {
                        let more = position + 1 < reachable.len();
                        let mut branch_observation_count = None;
                        let mut remaining = None;
                        if more {
                            let condition = crate::unit::unit_cleanup::CleanupCaseCondition {
                                case: arm.case,
                                ..condition.clone()
                            };
                            let observed = self.observe_case(&condition, next_value, operations)?;
                            if common_observation_count.is_none() {
                                common_observation_count =
                                    Some(operations.storage_observations.cases.len());
                            }
                            branch_observation_count =
                                Some(operations.storage_observations.cases.len());
                            let selected = block_id(allocate_dense(next_block)?);
                            let selected_parameters = declarations(
                                &values
                                    .iter()
                                    .map(|value| value.value_type())
                                    .collect::<Vec<_>>(),
                                next_value,
                            )?;
                            let later = block_id(allocate_dense(next_block)?);
                            let later_parameters = declarations(
                                &values
                                    .iter()
                                    .map(|value| value.value_type())
                                    .collect::<Vec<_>>(),
                                next_value,
                            )?;
                            let proof_arguments = self.proof_arguments()?;
                            let successor = |edge, target| SuccessorEdge {
                                edge,
                                target,
                                arguments: values.iter().map(|value| value.id).collect(),
                                erased_arguments: Vec::new(),
                                erased_proof_arguments: proof_arguments.clone(),
                                structural_arguments: Vec::new(),
                                cleanup_actions: Vec::new(),
                            };
                            self.blocks.push(Block {
                                id: *self.current,
                                parameters: std::mem::take(self.parameters),
                                structural_parameters: std::mem::take(self.structural_parameters),
                                erased_scalar_formals: std::mem::take(
                                    self.block_erased_scalar_formals,
                                ),
                                erased_proof_formals: erased_proof_formal_declarations(
                                    identities,
                                    self.checked,
                                    self.erased_proof_formals,
                                ),
                                operations: operations[*self.operation_start..].to_vec(),
                                terminator: Terminator::Conditional {
                                    condition: observed.id,
                                    when_true: successor(
                                        edge_id(allocate_dense(next_edge)?),
                                        selected,
                                    ),
                                    when_false: successor(
                                        edge_id(allocate_dense(next_edge)?),
                                        later,
                                    ),
                                },
                            });
                            *self.current = selected;
                            *self.parameters = selected_parameters.clone();
                            *values = selected_parameters;
                            *self.operation_start = operations.len();
                            remaining = Some((later, later_parameters));
                        }
                        // The tree covers the exact declared alternatives. After all earlier
                        // tests failed, the last reachable alternative is exhaustive; no
                        // additional tag load or unreachable fallthrough is needed.
                        let arm_depth =
                            depth + usize::from(arm.reachability == Reachability::Runtime);
                        for step in &arm.steps {
                            self.partition_node(
                                identities, step, leaves, scope, arm_depth, values, next_value,
                                next_block, next_edge, operations,
                            )?;
                        }
                        let edge = self.cleanup_jump(
                            identities,
                            join,
                            &join_parameters,
                            Vec::new(),
                            Vec::new(),
                            values,
                            next_edge,
                            operations,
                        )?;
                        operations.completed_cleanup_frontiers.push(crate::emission::selected_continuations::CompletedCleanupFrontier {
                            edge, region: observation.clone(), kind: crate::emission::selected_continuations::CompletedCleanupKind::CasePayloads(arms.iter().map(|arm| arm.case).collect()),
                        });
                        if let Some(count) = branch_observation_count {
                            operations.storage_observations.cases.truncate(count);
                        }
                        if let Some((later, parameters)) = remaining {
                            *self.current = later;
                            *self.parameters = parameters.clone();
                            *values = parameters;
                            *self.operation_start = operations.len();
                        }
                    }
                    *self.current = join;
                    *self.parameters = join_parameters.clone();
                    *values = join_parameters;
                    *self.operation_start = operations.len();
                    if let Some(count) = common_observation_count {
                        operations.storage_observations.cases.truncate(count);
                    }
                    // All payload alternatives have the same settled frontier here. Emit
                    // this carrier's shared fields once, retaining their live obligations.
                    for step in common {
                        self.partition_node(
                            identities, step, leaves, scope, depth, values, next_value, next_block,
                            next_edge, operations,
                        )?;
                    }
                    if let Some((bypass, parameters)) = bypass {
                        self.cleanup_jump(
                            identities,
                            bypass,
                            &parameters,
                            Vec::new(),
                            Vec::new(),
                            values,
                            next_edge,
                            operations,
                        )?;
                    }
                    Ok(())
                }

                fn select_storage(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    region: &terminal_psi::StructuralArgument,
                    retirement: bool,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                ) -> Result<(BlockId, Vec<ValueDeclaration>), LoweringError> {
                    let types = values
                        .iter()
                        .map(|value| value.value_type())
                        .collect::<Vec<_>>();
                    let entry = block_id(allocate_dense(next_block)?);
                    let entry_parameters = declarations(&types, next_value)?;
                    let bypass = block_id(allocate_dense(next_block)?);
                    let bypass_parameters = declarations(&types, next_value)?;
                    let edge = self.cleanup_jump(
                        identities,
                        entry,
                        &entry_parameters,
                        Vec::new(),
                        Vec::new(),
                        values,
                        next_edge,
                        operations,
                    )?;
                    operations.selected_cleanup_groups.push(
                crate::emission::selected_continuations::SelectedCleanupGroup {
                    edge,
                    region: region.clone(),
                    requirement: if retirement {
                        crate::emission::selected_continuations::CleanupRequirement::ResidentHome
                    } else {
                        crate::emission::selected_continuations::CleanupRequirement::WholeValue
                    },
                    bypass,
                },
            );
                    Ok((bypass, bypass_parameters))
                }

                fn proof_arguments(&self) -> Result<Vec<ProofTerm>, LoweringError> {
                    (0..self.erased_proof_formals.len())
                        .map(|position| {
                            Ok(ProofTerm::Formal {
                                position: u32::try_from(position).map_err(|_| {
                                    LoweringError::Unsupported(
                                        "cleanup erased-proof roster exceeds u32",
                                    )
                                })?,
                            })
                        })
                        .collect()
                }

                pub(crate) fn continue_actions(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    actions: Vec<TerminalAffineCleanupAction>,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &OperationBuffer,
                ) -> Result<EdgeId, LoweringError> {
                    let continuation = block_id(allocate_dense(next_block)?);
                    let parameters = declarations(
                        &values
                            .iter()
                            .map(|value| value.value_type())
                            .collect::<Vec<_>>(),
                        next_value,
                    )?;
                    self.cleanup_jump(
                        identities,
                        continuation,
                        &parameters,
                        actions,
                        Vec::new(),
                        values,
                        next_edge,
                        operations,
                    )
                }

                fn cleanup_scope(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    regions: &[CleanupRegion],
                    depth: usize,
                    selected_scope: Option<&terminal_psi::StructuralArgument>,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                ) -> Result<(), LoweringError> {
                    let mut remaining = regions;
                    while let Some(first) = remaining.first() {
                        if first.conditions.len() < depth {
                            return Err(LoweringError::Unsupported(
                                "cleanup lost its enclosing case scope",
                            ));
                        }
                        if first.conditions.len() == depth {
                            let retirement = matches!(
                                first.action.physical(),
                                Some(TerminalAffineCleanupAction::RetireHome(_))
                            );
                            if let Some(region) = first.selected_region.as_ref()
                                && Some(region) != selected_scope
                            {
                                let count = if retirement {
                                    1
                                } else {
                                    remaining
                                        .iter()
                                        .take_while(|entry| {
                                            entry.selected_region == first.selected_region
                                                && !matches!(
                                                    entry.action.physical(),
                                                    Some(TerminalAffineCleanupAction::RetireHome(
                                                        _
                                                    ))
                                                )
                                        })
                                        .count()
                                };
                                let (bypass, parameters) = self.select_storage(
                                    identities, region, retirement, values, next_value, next_block,
                                    next_edge, operations,
                                )?;
                                self.cleanup_scope(
                                    identities,
                                    &remaining[..count],
                                    depth,
                                    Some(region),
                                    values,
                                    next_value,
                                    next_block,
                                    next_edge,
                                    operations,
                                )?;
                                self.cleanup_jump(
                                    identities,
                                    bypass,
                                    &parameters,
                                    Vec::new(),
                                    Vec::new(),
                                    values,
                                    next_edge,
                                    operations,
                                )?;
                                remaining = &remaining[count..];
                                continue;
                            }
                            let count = if retirement {
                                1
                            } else {
                                remaining
                                    .iter()
                                    .take_while(|entry| {
                                        entry.conditions.len() == depth
                                            && entry.selected_region == first.selected_region
                                            && !matches!(
                                                entry.action.physical(),
                                                Some(TerminalAffineCleanupAction::RetireHome(_))
                                            )
                                    })
                                    .count()
                            };
                            let actions = remaining[..count]
                                .iter()
                                .filter_map(|region| region.action.clone().into_physical())
                                .collect();
                            self.continue_actions(
                                identities, actions, values, next_value, next_block, next_edge,
                                operations,
                            )?;
                            remaining = &remaining[count..];
                            continue;
                        }
                        let selected = &first.conditions[depth];
                        let count = remaining
                            .iter()
                            .take_while(|region| {
                                region.conditions.get(depth).is_some_and(|condition| {
                                    condition.place == selected.place
                                        && condition.path == selected.path
                                        && condition.structural_type == selected.structural_type
                                })
                            })
                            .count();
                        let observation = terminal_psi::StructuralArgument {
                            place: selected.place,
                            path: selected.path.clone(),
                            access: terminal_psi::StructuralAccess::Owned,
                        };
                        let selected_observation = remaining[..count].iter().all(|region| {
                            region.selected_region.as_ref().is_some_and(|source| {
                                source.place == observation.place
                                    && (source.path.starts_with(&observation.path)
                                        || observation.path.starts_with(&source.path))
                            })
                        });
                        let bypass = if selected_observation
                            && !self.declared_single_case(selected, operations)
                            && self.retained_case(selected, operations).is_none()
                        {
                            Some(self.select_storage(
                                identities,
                                &observation,
                                false,
                                values,
                                next_value,
                                next_block,
                                next_edge,
                                operations,
                            )?)
                        } else {
                            None
                        };
                        let join = block_id(allocate_dense(next_block)?);
                        let join_parameters = declarations(
                            &values
                                .iter()
                                .map(|value| value.value_type())
                                .collect::<Vec<_>>(),
                            next_value,
                        )?;
                        self.cleanup_cases(
                            identities,
                            &remaining[..count],
                            depth,
                            selected_scope,
                            join,
                            &join_parameters,
                            values,
                            next_value,
                            next_block,
                            next_edge,
                            operations,
                        )?;
                        if let Some((bypass, parameters)) = bypass {
                            self.cleanup_jump(
                                identities,
                                bypass,
                                &parameters,
                                Vec::new(),
                                Vec::new(),
                                values,
                                next_edge,
                                operations,
                            )?;
                        }
                        remaining = &remaining[count..];
                    }
                    Ok(())
                }

                fn cleanup_cases(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                    regions: &[CleanupRegion],
                    depth: usize,
                    selected_scope: Option<&terminal_psi::StructuralArgument>,
                    join: BlockId,
                    join_parameters: &[ValueDeclaration],
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                ) -> Result<(), LoweringError> {
                    let mut remaining = regions;
                    let mut common_case_count = None;
                    while let Some(first) = remaining.first() {
                        let condition = &first.conditions[depth];
                        let count = remaining
                            .iter()
                            .take_while(|region| region.conditions.get(depth) == Some(condition))
                            .count();
                        let declaration = self.case_subject(condition, operations)?;
                        let cases = match &declaration.shape {
                            terminal_psi::StructuralTypeShape::Sum { cases }
                            | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => cases,
                            _ => {
                                return Err(LoweringError::Unsupported(
                                    "cleanup case has no sum carrier",
                                ));
                            }
                        };
                        if terminal_psi::find_structural_case(cases, condition.case).is_none() {
                            return Err(LoweringError::Unsupported(
                                "cleanup case is not declared by its sum",
                            ));
                        }
                        if cases.len() == 1 {
                            if count != remaining.len() {
                                return Err(LoweringError::Unsupported(
                                    "single-case cleanup has competing arms",
                                ));
                            }
                            self.cleanup_scope(
                                identities,
                                remaining,
                                depth + 1,
                                selected_scope,
                                values,
                                next_value,
                                next_block,
                                next_edge,
                                operations,
                            )?;
                            self.cleanup_jump(
                                identities,
                                join,
                                join_parameters,
                                Vec::new(),
                                Vec::new(),
                                values,
                                next_edge,
                                operations,
                            )?;
                            return Ok(());
                        }
                        let when_true = block_id(allocate_dense(next_block)?);
                        let when_true_parameters = declarations(
                            &values
                                .iter()
                                .map(|value| value.value_type())
                                .collect::<Vec<_>>(),
                            next_value,
                        )?;
                        let more_cases = count < remaining.len();
                        let (when_false, when_false_parameters) = if more_cases {
                            (
                                block_id(allocate_dense(next_block)?),
                                declarations(
                                    &values
                                        .iter()
                                        .map(|value| value.value_type())
                                        .collect::<Vec<_>>(),
                                    next_value,
                                )?,
                            )
                        } else {
                            (join, join_parameters.to_vec())
                        };
                        let observation = self.observe_case(condition, next_value, operations)?;
                        // The first discriminator dominates every arrival at this dispatch's
                        // join. Observations made inside an arm or its later false-chain do
                        // not: preserve only this common scope after the complete dispatch.
                        if common_case_count.is_none() {
                            common_case_count = Some(operations.storage_observations.cases.len());
                        }
                        let branch_case_count = operations.storage_observations.cases.len();
                        let proof_arguments = self.proof_arguments()?;
                        let successor = |edge, target| SuccessorEdge {
                            edge,
                            target,
                            arguments: values.iter().map(|value| value.id).collect(),
                            erased_arguments: Vec::new(),
                            erased_proof_arguments: proof_arguments.clone(),
                            structural_arguments: Vec::new(),
                            cleanup_actions: Vec::new(),
                        };
                        self.blocks.push(Block {
                            id: *self.current,
                            parameters: std::mem::take(self.parameters),
                            structural_parameters: std::mem::take(self.structural_parameters),
                            erased_scalar_formals: std::mem::take(self.block_erased_scalar_formals),
                            erased_proof_formals: erased_proof_formal_declarations(
                                identities,
                                self.checked,
                                self.erased_proof_formals,
                            ),
                            operations: operations[*self.operation_start..].to_vec(),
                            terminator: Terminator::Conditional {
                                condition: observation.id,
                                when_true: successor(
                                    edge_id(allocate_dense(next_edge)?),
                                    when_true,
                                ),
                                when_false: successor(
                                    edge_id(allocate_dense(next_edge)?),
                                    when_false,
                                ),
                            },
                        });
                        *self.current = when_true;
                        *self.parameters = when_true_parameters.clone();
                        *values = when_true_parameters;
                        *self.operation_start = operations.len();
                        self.cleanup_scope(
                            identities,
                            &remaining[..count],
                            depth + 1,
                            selected_scope,
                            values,
                            next_value,
                            next_block,
                            next_edge,
                            operations,
                        )?;
                        self.cleanup_jump(
                            identities,
                            join,
                            join_parameters,
                            Vec::new(),
                            Vec::new(),
                            values,
                            next_edge,
                            operations,
                        )?;
                        operations
                            .storage_observations
                            .cases
                            .truncate(branch_case_count);
                        *self.current = when_false;
                        *self.parameters = when_false_parameters.clone();
                        *values = when_false_parameters;
                        *self.operation_start = operations.len();
                        remaining = &remaining[count..];
                    }
                    if let Some(count) = common_case_count {
                        operations.storage_observations.cases.truncate(count);
                    }
                    Ok(())
                }

                fn cleanup_jump(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                    target: BlockId,
                    parameters: &[ValueDeclaration],
                    cleanup_actions: Vec<TerminalAffineCleanupAction>,
                    structural_arguments: Vec<terminal_psi::StructuralArgument>,
                    values: &mut Vec<ValueDeclaration>,
                    next_edge: &mut u64,
                    operations: &OperationBuffer,
                ) -> Result<EdgeId, LoweringError> {
                    let edge = edge_id(allocate_dense(next_edge)?);
                    self.blocks.push(Block {
                        id: *self.current,
                        parameters: std::mem::take(self.parameters),
                        structural_parameters: std::mem::take(self.structural_parameters),
                        erased_scalar_formals: std::mem::take(self.block_erased_scalar_formals),
                        erased_proof_formals: erased_proof_formal_declarations(
                            identities,
                            self.checked,
                            self.erased_proof_formals,
                        ),
                        operations: operations[*self.operation_start..].to_vec(),
                        terminator: Terminator::Jump {
                            edge,
                            target,
                            arguments: values.iter().map(|value| value.id).collect(),
                            erased_arguments: Vec::new(),
                            erased_proof_arguments: self.proof_arguments()?,
                            structural_arguments,
                            cleanup_actions,
                        },
                    });
                    *self.current = target;
                    *self.parameters = parameters.to_vec();
                    *values = parameters.to_vec();
                    *self.operation_start = operations.len();
                    Ok(edge)
                }
            }
        }

        #[derive(Clone, Default)]
        pub(crate) struct ScalarArgumentCaptures {
            slots: Vec<Option<usize>>,
        }

        impl ScalarArgumentCaptures {
            pub(crate) fn record(&mut self, ordinal: usize, slot: usize) {
                if self.slots.len() <= ordinal {
                    self.slots.resize(ordinal + 1, None);
                }
                self.slots[ordinal] = Some(slot);
            }

            pub(crate) fn values(&self, values: &[ValueDeclaration]) -> Vec<ValueDeclaration> {
                self.slots
                    .iter()
                    .filter_map(|slot| slot.and_then(|slot| values.get(slot)).copied())
                    .collect()
            }
        }

        pub(crate) mod qualification_edge {
            use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
            use crate::emission::operation_emission::expressions::LoweredDirectExpression;
            use crate::shared::lowering_error::{LoweringError, unsupported};
            use semantic_vocabulary::EdgeId;
            use semantic_vocabulary::{BlockId, MachineId};
            use terminal_psi::{StructuralArgument, Terminator, ValueDeclaration};

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn emit(
                successor: BlockId,
                edge: EdgeId,
                machine: MachineId,
                arguments: &[LoweredDirectExpression],
                structural_arguments: &[StructuralArgument],
                sources: &[ValueDeclaration],
                destinations: &[ValueDeclaration],
                catalog: &terminal_psi::ScalarQualificationCatalog,
                coercions: &mut Vec<terminal_psi::ScalarQualificationCoercion>,
            ) -> Result<Terminator, LoweringError> {
                if !structural_arguments.is_empty()
                    || arguments.is_empty()
                    || arguments.len() != destinations.len()
                {
                    return unsupported(
                        "scalar qualification requires its exact scalar successor arguments",
                    );
                }
                let mut values = Vec::with_capacity(arguments.len());
                for (ordinal, (argument, destination)) in
                    arguments.iter().zip(destinations).enumerate()
                {
                    let position = match argument {
                        LoweredDirectExpression::Parameter { position, .. }
                        | LoweredDirectExpression::Local { position, .. } => *position,
                        LoweredDirectExpression::Boolean { expression } => {
                            match expression.as_ref() {
                                LoweredBooleanReturnExpression::Parameter { position }
                                | LoweredBooleanReturnExpression::Local { position } => *position,
                                _ => {
                                    return unsupported(
                                        "scalar qualification operand must already be evaluated",
                                    );
                                }
                            }
                        }
                        _ => {
                            return unsupported(
                                "scalar qualification operand must already be evaluated",
                            );
                        }
                    };
                    let source = sources.get(position).ok_or(LoweringError::Unsupported(
                        "scalar qualification source is absent",
                    ))?;
                    if ordinal + 1 != arguments.len() {
                        if position != ordinal
                            || source.qualifications != destination.qualifications
                        {
                            return unsupported(
                                "scalar qualification changed a carried prefix membership",
                            );
                        }
                    } else {
                        let source_domains = if source.qualifications.is_empty() {
                            &[][..]
                        } else {
                            catalog
                                .sets
                                .iter()
                                .find(|set| set.id == source.qualifications)
                                .map(|set| set.domains.as_slice())
                                .ok_or(LoweringError::Unsupported(
                                    "scalar qualification input set is absent",
                                ))?
                        };
                        let destination_domains = if destination.qualifications.is_empty() {
                            &[][..]
                        } else {
                            catalog
                                .sets
                                .iter()
                                .find(|set| set.id == destination.qualifications)
                                .map(|set| set.domains.as_slice())
                                .ok_or(LoweringError::Unsupported(
                                    "scalar qualification output set is absent",
                                ))?
                        };
                        let introduction = source_domains.len() < destination_domains.len()
                            && source_domains
                                .iter()
                                .all(|domain| destination_domains.binary_search(domain).is_ok());
                        let erasure = destination_domains.len() < source_domains.len()
                            && destination_domains
                                .iter()
                                .all(|domain| source_domains.binary_search(domain).is_ok());
                        if source.id == destination.id || !(introduction || erasure) {
                            return unsupported(
                                "scalar qualification must add or erase membership on a fresh value",
                            );
                        }
                        if introduction {
                            for domain in destination_domains
                                .iter()
                                .filter(|domain| !source_domains.contains(domain))
                            {
                                let declaration = catalog
                                    .domains
                                    .iter()
                                    .find(|declaration| declaration.id == *domain)
                                    .ok_or(LoweringError::Unsupported(
                                        "scalar coercion lost its domain declaration",
                                    ))?;
                                if declaration.predicate.is_some()
                                    || !declaration.establishment_routes.is_empty()
                                {
                                    return unsupported(
                                        "scalar coercion cannot replace checked predicate or issuer evidence",
                                    );
                                }
                            }
                        }
                        coercions.push(terminal_psi::ScalarQualificationCoercion {
                            machine,
                            edge,
                            argument_ordinal: u32::try_from(ordinal).map_err(|_| {
                                LoweringError::Unsupported(
                                    "scalar qualification ordinal exceeds u32",
                                )
                            })?,
                            source: source.id,
                            destination: destination.id,
                        });
                    }
                    values.push(source.id);
                }
                Ok(Terminator::Jump {
                    edge,
                    target: successor,
                    arguments: values,
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    cleanup_actions: Vec::new(),
                })
            }
        }
        pub(crate) mod record_captures {
            use super::{Evaluation, LoweringError, StructuralTypeDeclaration, ValueDeclaration};
            use crate::emission::operation_emission::buffer::OperationBuffer;
            use crate::emission::operation_emission::expressions::emit_direct_expression;
            use crate::expression_preparation::computation_graph::borrowed_arguments_from_roots;
            use crate::expression_preparation::prepare_expression::exact_u64_index;
            use crate::shared::checked_facts::value_facts;
            use terminal_psi::{StructuralPlaceDeclaration, StructuralTypeShape};

            impl Evaluation {
                #[allow(clippy::too_many_arguments)]
                pub(crate) fn prepare_storage_before(
                    &mut self,

                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &checked_trees::CheckedTrees,
                    state: symbols::SymbolHandle,
                    statement: u32,
                    types: &[StructuralTypeDeclaration],
                    domains: &[terminal_psi::StructuralDomainDeclaration],
                    values: &[ValueDeclaration],
                    next_place: &mut u64,
                    next_value: &mut u64,
                    operations: &mut OperationBuffer,
                    private_places: &mut Vec<StructuralPlaceDeclaration>,
                ) -> Result<(), LoweringError> {
                    if !self
                        .computation_address_roots
                        .iter()
                        .any(|(owner, _)| *owner == state)
                    {
                        let roots = value_facts(checked)
                            .scalar_computations
                            .roots
                            .iter()
                            .filter_map(|(_, root)| {
                                (root.state == state).then_some((root.statement_ordinal, root.root))
                            })
                            .collect();
                        self.computation_address_roots.push((state, roots));
                    }
                    let roots = self
                        .computation_address_roots
                        .iter()
                        .find(|(owner, _)| *owner == state)
                        .ok_or(LoweringError::Unsupported(
                            "computed address demand lost its retained state",
                        ))?
                        .1
                        .iter()
                        .filter_map(|(ordinal, root)| (*ordinal == statement).then_some(*root))
                        .collect::<Vec<_>>();
                    let borrowed = borrowed_arguments_from_roots(checked, &roots)?;
                    for argument in borrowed {
                        self.prepare_borrowed_argument(
                            identities,
                            checked,
                            state,
                            argument,
                            values,
                            types,
                            domains,
                            next_place,
                            operations,
                            private_places,
                        )?;
                    }
                    for position in 0..self.place_observations.len() {
                        let loan = &self.place_observations[position];
                        if loan.statement_index >= statement {
                            continue;
                        }
                        if let checked_trees::CheckedPlaceLoanRoot::Local { symbol } =
                            loan.loan.root
                        {
                            self.prepare_immutable_local_borrow(
                                identities,
                                checked,
                                state,
                                Some(facts::PlaceRoot::Symbol(symbol)),
                                values,
                                types,
                                domains,
                                next_place,
                                operations,
                                private_places,
                            )?;
                        }
                        let loan = &self.place_observations[position];
                        let selectors = loan
                            .loan
                            .path
                            .iter()
                            .filter_map(|segment| match segment {
                                checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                                    selector,
                                ) => Some(selector),
                                _ => None,
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        if selectors.is_empty() || loan.selectors.len() == selectors.len() {
                            continue;
                        }
                        if !loan.selectors.is_empty() {
                            return super::unsupported(
                                "place loan has an incomplete selector capture",
                            );
                        }
                        let bindings = self
                            .scalar_bindings
                            .clone()
                            .with_structural_parameters(&self.structural_parameters)
                            .with_resolved_structural_observations(
                                &self.structural_fields,
                                &self.structural_cases,
                            )
                            .with_primitive_storage(&self.primitive_storage)
                            .with_structural_locals(&self.structural_locals);
                        let mut coordinates = Vec::with_capacity(selectors.len());
                        for selector in &selectors {
                            let selector = match selector {
                                checked_trees::CheckedRuntimeIndex::StorageRead { symbol } => {
                                    bindings.runtime_index_storage(*symbol)?
                                }
                                checked_trees::CheckedRuntimeIndex::Parameter { position } => {
                                    bindings.runtime_index_parameter(*position)?
                                }
                                checked_trees::CheckedRuntimeIndex::Local { position } => {
                                    bindings.runtime_index_local(*position)?
                                }
                                checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                                    bindings.runtime_index_field(identities, field)?
                                }
                                _ => {
                                    return super::unsupported(
                                        "loan coordinate has no declaration-time scalar source",
                                    );
                                }
                            };
                            let selector = exact_u64_index(selector)?;
                            let scalar_type = selector.scalar_type();
                            if scalar_type
                                != super::terminal_scalar_type(
                                    typed_trees::types::PrimitiveType::U64,
                                )?
                            {
                                return super::unsupported(
                                    "loan coordinate lost its exact index carrier",
                                );
                            }
                            let mut carriers = types.iter().filter(|declaration| {
                                declaration.shape
                                    == StructuralTypeShape::PrimitiveScalar(scalar_type)
                                    && declaration.scalar_restriction.is_none()
                            });
                            let structural_type = carriers
                                .next()
                                .ok_or(LoweringError::Unsupported(
                                    "loan coordinate has no declared primitive snapshot carrier",
                                ))?
                                .id;
                            if carriers.next().is_some() {
                                return super::unsupported(
                                    "loan coordinate primitive snapshot carrier is ambiguous",
                                );
                            }
                            let value =
                                emit_direct_expression(&selector, values, next_value, operations);
                            let value = super::super::primitive_locals::completed_scalar(
                                value, values, operations,
                            )?;
                            let (place, declaration) =
                                super::super::primitive_locals::push_established_local(
                                    next_place,
                                    operations,
                                    value,
                                    structural_type,
                                    super::super::primitive_locals::PrimitiveStorageCatalog {
                                        types,
                                        domains,
                                        scalars: self.shared_qualifications.catalog(),
                                    },
                                    None,
                                )?;
                            private_places.push(declaration);
                            coordinates.push(place);
                        }
                        let loan =
                            &mut std::sync::Arc::make_mut(&mut self.place_observations)[position];
                        if let Ok(capture) = &mut loan.capture
                            && capture.runtime_index.is_some()
                        {
                            capture.coordinate = coordinates.first().copied();
                        }
                        loan.selectors = coordinates;
                    }
                    self.scalar_bindings = self
                        .scalar_bindings
                        .clone()
                        .with_place_observations(&self.place_observations);
                    Ok(())
                }
            }
        }
        pub(crate) mod scalar_control {

            use super::{
                CheckedScalarExpressionRole, CheckedTrees, Evaluation,
                LoweredBooleanReturnExpression, LoweredDirectExpression, LoweringError,
                QualifiedScalarType, ValueDeclaration, scalar_source_types, terminal_scalar_type,
                unsupported,
            };
            use crate::emission::operation_emission::buffer::OperationBuffer;
            use crate::emission::operation_emission::calls::CallEmissionContext;
            use crate::expression_preparation::bindings::structural_cases::sum_cases;
            use crate::scalar_lowering::branch_states::guards::is_computed;
            use crate::scalar_lowering::branch_states::prepared::{
                CaseDispatchArm, LoweredScalarBranchState, LoweredScalarBranchTerminator,
            };
            use crate::scalar_lowering::computations::parameters;
            use crate::scalar_lowering::computations::{
                NormalCompletionTarget, NormalResultTarget,
            };
            use crate::unit::attached_unit::OperationEnvironment;
            use crate::unit::attached_unit::scalar_completion::control::returning_exits;
            use checked_trees::{CheckedScalarBranchDestination, CheckedScalarStateTerminator};

            /// Cleanup already selected by source ownership, before scalar arm expansion.
            /// Obligation identities belong to this exit, so all routes to that authored
            /// return consume its admitted result tree before their scalar join.
            pub(crate) struct ScalarReturnCleanup {
                pub(crate) exit: checked_trees::FlowClaimJoinExit,
                pub(crate) destination: CheckedScalarBranchDestination,
                pub(crate) actions: Vec<crate::unit::unit_cleanup::CleanupRegion>,
                pub(crate) result:
                    Option<crate::unit::attached_unit::completion_cleanup::NormalResultCleanup>,
                pub(crate) copied_payloads: Vec<checked_trees::CheckedSelectedPayloadCopy>,
            }

            fn completion_target<'a>(
                destination: &CheckedScalarBranchDestination,
                completions: &'a [(
                    CheckedScalarBranchDestination,
                    NormalCompletionTarget,
                    checked_trees::FlowClaimJoinExit,
                    &'a [checked_trees::CheckedSelectedPayloadCopy],
                )],
            ) -> Result<
                (
                    &'a NormalCompletionTarget,
                    checked_trees::FlowClaimJoinExit,
                    &'a [checked_trees::CheckedSelectedPayloadCopy],
                ),
                LoweringError,
            > {
                let mut selected = completions
                    .iter()
                    .filter(|(source, _, _, _)| source == destination);
                let (_, target, exit, copies) = selected.next().ok_or(
                    LoweringError::Unsupported("scalar return has no selected cleanup completion"),
                )?;
                if selected.next().is_some() {
                    return unsupported("scalar return repeats its cleanup completion");
                }
                Ok((target, *exit, *copies))
            }

            fn cleanup_target(
                expansion: &mut crate::scalar_lowering::computations::Expansion<'_>,
                types: &[QualifiedScalarType],
                actions: &[crate::unit::unit_cleanup::CleanupRegion],
                proof_arguments: Vec<crate::scalar_lowering::contracts::LoweredProofTerm>,
            ) -> usize {
                expansion.push(LoweredScalarBranchState::continuing(
                    types.to_vec(),
                    LoweredScalarBranchTerminator::Jump {
                        cleanup_actions: actions
                            .iter()
                            .map(crate::unit::unit_cleanup::PreparedNormalCleanupAction::from)
                            .collect(),
                        structural_arguments: Vec::new(),
                        target: 0,
                        arguments: parameters(types),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: proof_arguments,
                    },
                ))
            }

            fn result_targets(
                expansion: &mut crate::scalar_lowering::computations::Expansion<'_>,
                result: &crate::unit::attached_unit::completion_cleanup::NormalResultCleanup,
                types: &[QualifiedScalarType],
                proof_arguments: &[crate::scalar_lowering::contracts::LoweredProofTerm],
            ) -> std::sync::Arc<NormalResultTarget> {
                use crate::unit::attached_unit::completion_cleanup::NormalResultCleanup;
                std::sync::Arc::new(match result {
                    NormalResultCleanup::NoNormal { computation } => NormalResultTarget::NoNormal {
                        computation: *computation,
                    },
                    NormalResultCleanup::Value {
                        computation,
                        actions,
                    } => NormalResultTarget::Value {
                        computation: *computation,
                        target: cleanup_target(expansion, types, actions, proof_arguments.to_vec()),
                    },
                    NormalResultCleanup::Dispatch { computation, arms } => {
                        NormalResultTarget::Dispatch {
                            computation: *computation,
                            arms: arms
                                .iter()
                                .map(|(arm, result)| {
                                    (
                                        arm.clone(),
                                        result_targets(expansion, result, types, proof_arguments),
                                    )
                                })
                                .collect::<Vec<_>>()
                                .into(),
                        }
                    }
                })
            }

            #[derive(Clone)]
            struct ScalarCaseDispatchArm {
                destination: CheckedScalarBranchDestination,
                source: semantic_vocabulary::PlaceId,
                selected: semantic_vocabulary::StructuralCaseId,
                cases: Vec<semantic_vocabulary::StructuralCaseId>,
            }

            fn push_arm_target(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                expansion: &mut crate::scalar_lowering::computations::Expansion<'_>,
                bindings: &crate::expression_preparation::bindings::ScalarBindings,
                source_types: &[QualifiedScalarType],
                result_type: QualifiedScalarType,
                destination: &CheckedScalarBranchDestination,
                completions: &[(
                    CheckedScalarBranchDestination,
                    NormalCompletionTarget,
                    checked_trees::FlowClaimJoinExit,
                    &[checked_trees::CheckedSelectedPayloadCopy],
                )],
            ) -> Result<usize, LoweringError> {
                let CheckedScalarBranchDestination::Return {
                    is_continuation, ..
                } = destination
                else {
                    return unsupported(
                        "ordered scalar completion cannot drop a non-returning edge",
                    );
                };
                let role = if *is_continuation {
                    CheckedScalarExpressionRole::ContinuationReturn
                } else {
                    CheckedScalarExpressionRole::Return
                };
                let (target, exit, copies) = completion_target(destination, completions)?;
                expansion.normal_return(
                    identities,
                    &exit,
                    copies,
                    role,
                    bindings,
                    source_types,
                    result_type,
                    target,
                )
            }

            fn scalar_case_dispatch_terminator(
                dispatch: ScalarCaseDispatchArm,
                when_true_target: usize,
                when_false_target: usize,
                source_types: &[QualifiedScalarType],
                proof_formals: Vec<crate::scalar_lowering::contracts::LoweredProofTerm>,
            ) -> LoweredScalarBranchTerminator {
                let when_true_arguments = parameters(source_types);
                LoweredScalarBranchTerminator::CaseDispatch {
                    source: dispatch.source,
                    selected: dispatch.selected,
                    cases: dispatch.cases,
                    payloads: Vec::new(),
                    when_true_target,
                    when_true_arguments,
                    when_true_erased_arguments: Vec::new(),
                    when_true_erased_proof_arguments: proof_formals.clone(),
                    when_false_target,
                    when_false_arguments: parameters(source_types),
                    when_false_erased_arguments: Vec::new(),
                    when_false_erased_proof_arguments: proof_formals,
                }
            }

            impl Evaluation {
                fn scalar_case_dispatch(
                    &self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &CheckedTrees,
                    state: symbols::SymbolHandle,
                    bindings: &crate::expression_preparation::bindings::ScalarBindings,
                    guard_statement_ordinal: u32,
                    destination: &CheckedScalarBranchDestination,
                ) -> Result<Option<ScalarCaseDispatchArm>, LoweringError> {
                    if is_computed(checked, state, guard_statement_ordinal) {
                        return Ok(None);
                    }
                    let LoweredDirectExpression::Boolean { expression: guard } = bindings
                        .expression_at(
                            identities,
                            checked,
                            state,
                            guard_statement_ordinal,
                            CheckedScalarExpressionRole::Guard,
                        )?
                    else {
                        return Ok(None);
                    };
                    let LoweredBooleanReturnExpression::StructuralCaseMembership {
                        source,
                        path,
                        case,
                        ..
                    } = guard.as_ref()
                    else {
                        return Ok(None);
                    };
                    if !path.is_empty() {
                        return Ok(None);
                    }
                    let Some(cases) = sum_cases(&self.structural_cases, *source) else {
                        return Ok(None);
                    };
                    if terminal_psi::find_structural_case(cases, *case).is_none() {
                        return unsupported("case dispatch selects a case outside its root's sum");
                    }
                    Ok(Some(ScalarCaseDispatchArm {
                        destination: destination.clone(),
                        source: *source,
                        selected: *case,
                        cases: cases.iter().map(|declared| declared.id).collect(),
                    }))
                }

                #[allow(clippy::too_many_arguments)]
                fn case_dispatch_split(
                    &self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    expansion: &mut crate::scalar_lowering::computations::Expansion<'_>,
                    bindings: &crate::expression_preparation::bindings::ScalarBindings,
                    source_types: &[QualifiedScalarType],
                    result_type: QualifiedScalarType,
                    dispatches: Vec<ScalarCaseDispatchArm>,
                    fallback: Option<&CheckedScalarBranchDestination>,
                    completions: &[(
                        CheckedScalarBranchDestination,
                        NormalCompletionTarget,
                        checked_trees::FlowClaimJoinExit,
                        &[checked_trees::CheckedSelectedPayloadCopy],
                    )],
                ) -> Result<LoweredScalarBranchTerminator, LoweringError> {
                    let source = dispatches[0].source;
                    let cases = dispatches[0].cases.clone();
                    if dispatches
                        .iter()
                        .any(|dispatch| dispatch.source != source || dispatch.cases != cases)
                    {
                        return unsupported("case split dispatch requires one subject sum");
                    }
                    let mut armed = Vec::with_capacity(dispatches.len());
                    for dispatch in dispatches {
                        let target = push_arm_target(
                            identities,
                            expansion,
                            bindings,
                            source_types,
                            result_type,
                            &dispatch.destination,
                            completions,
                        )?;
                        let arguments = parameters(source_types);
                        armed.push(CaseDispatchArm {
                            case: dispatch.selected,
                            payloads: Vec::new(),
                            target,
                            arguments,
                            erased_arguments: Vec::new(),
                            erased_proof_arguments: self.proof_formal_forwarding(),
                        });
                    }
                    let (fallback_target, fallback_arguments) = if let Some(fallback) = fallback {
                        (
                            push_arm_target(
                                identities,
                                expansion,
                                bindings,
                                source_types,
                                result_type,
                                fallback,
                                completions,
                            )?,
                            parameters(source_types),
                        )
                    } else {
                        if cases
                            .iter()
                            .any(|case| !armed.iter().any(|arm| arm.case == *case))
                        {
                            return unsupported("case split dispatch lost an unarmed case");
                        }
                        (armed[0].target, Vec::new())
                    };
                    let target = expansion.push(LoweredScalarBranchState::continuing(
                        source_types.to_vec(),
                        LoweredScalarBranchTerminator::CaseDispatchSplit {
                            source,
                            cases,
                            armed,
                            fallback_target,
                            fallback_arguments,
                            fallback_erased_arguments: Vec::new(),
                            fallback_erased_proof_arguments: self.proof_formal_forwarding(),
                        },
                    ));
                    Ok(LoweredScalarBranchTerminator::Jump {
                        cleanup_actions: Vec::new(),
                        structural_arguments: Vec::new(),
                        target,
                        arguments: parameters(source_types),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: self.proof_formal_forwarding(),
                    })
                }

                #[allow(clippy::too_many_arguments)]
                pub(crate) fn scalar_control_result(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &CheckedTrees,
                    machine: symbols::SymbolHandle,
                    state: symbols::SymbolHandle,
                    control: &checked_trees::CheckedUnitScalarControlPlan,
                    cleanups: &[ScalarReturnCleanup],
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                    calls: &mut CallEmissionContext<'_>,
                    environment: &mut OperationEnvironment<'_>,
                ) -> Result<ValueDeclaration, LoweringError> {
                    let bindings = self.contextual_bindings();
                    let qualifications = &self.shared_qualifications;
                    let source_types = scalar_source_types(values);
                    let result_type =
                        crate::unit::attached_unit::parameters::unit_scalar_result_type(
                            checked,
                            state,
                            terminal_scalar_type(control.primitive_type)?,
                            qualifications,
                        )?;
                    let mut expansion = self.scalar_computation_expansion(
                        identities,
                        checked,
                        qualifications,
                        machine,
                        operations,
                        environment.next_place,
                    )?;
                    let mut completion_types = source_types.clone();
                    completion_types.push(result_type);
                    let completions = cleanups
                        .iter()
                        .map(|cleanup| {
                            let target = if let Some(result) = &cleanup.result {
                                NormalCompletionTarget::Selected(result_targets(
                                    &mut expansion,
                                    result,
                                    &completion_types,
                                    &self.proof_formal_forwarding(),
                                ))
                            } else {
                                NormalCompletionTarget::Ordinary(cleanup_target(
                                    &mut expansion,
                                    &completion_types,
                                    &cleanup.actions,
                                    self.proof_formal_forwarding(),
                                ))
                            };
                            (
                                cleanup.destination.clone(),
                                target,
                                cleanup.exit,
                                cleanup.copied_payloads.as_slice(),
                            )
                        })
                        .collect::<Vec<_>>();
                    let terminator = match &control.terminator {
                        CheckedScalarStateTerminator::Return { statement_ordinal } => {
                            let target = push_arm_target(
                                identities,
                                &mut expansion,
                                &bindings,
                                &source_types,
                                result_type,
                                &CheckedScalarBranchDestination::Return {
                                    statement_ordinal: *statement_ordinal,
                                    is_continuation: false,
                                },
                                &completions,
                            )?;
                            LoweredScalarBranchTerminator::Jump {
                                cleanup_actions: Vec::new(),
                                structural_arguments: Vec::new(),
                                target,
                                arguments: parameters(&source_types),
                                erased_arguments: Vec::new(),
                                erased_proof_arguments: self.proof_formal_forwarding(),
                            }
                        }
                        CheckedScalarStateTerminator::Conditional {
                            guard_statement_ordinal,
                            when_true,
                            when_false,
                        } => {
                            let true_target = push_arm_target(
                                identities,
                                &mut expansion,
                                &bindings,
                                &source_types,
                                result_type,
                                when_true,
                                &completions,
                            )?;
                            let false_target = push_arm_target(
                                identities,
                                &mut expansion,
                                &bindings,
                                &source_types,
                                result_type,
                                when_false,
                                &completions,
                            )?;
                            if let Some(dispatch) = self.scalar_case_dispatch(
                                identities,
                                checked,
                                state,
                                &bindings,
                                *guard_statement_ordinal,
                                when_true,
                            )? {
                                scalar_case_dispatch_terminator(
                                    dispatch,
                                    true_target,
                                    false_target,
                                    &source_types,
                                    self.proof_formal_forwarding(),
                                )
                            } else {
                                crate::scalar_lowering::branch_states::guards::evaluate(
                                    identities,
                                    checked,
                                    state,
                                    *guard_statement_ordinal,
                                    &bindings,
                                    &source_types,
                                    (
                                        true_target,
                                        parameters(&source_types),
                                        Vec::new(),
                                        self.proof_formal_forwarding(),
                                    ),
                                    (
                                        false_target,
                                        parameters(&source_types),
                                        Vec::new(),
                                        self.proof_formal_forwarding(),
                                    ),
                                    &mut expansion,
                                )?
                            }
                        }
                        CheckedScalarStateTerminator::Guarded { arms, fallback } => {
                            let arms = returning_exits(checked, *arms, fallback.as_ref())?;
                            let mut dispatches = Vec::with_capacity(arms.len());
                            for guard in arms.iter() {
                                dispatches.push(self.scalar_case_dispatch(
                                    identities,
                                    checked,
                                    state,
                                    &bindings,
                                    guard.guard_statement_ordinal,
                                    &guard.destination,
                                )?);
                            }
                            let one_subject = dispatches
                                .first()
                                .and_then(Option::as_ref)
                                .is_some_and(|first| {
                                    dispatches.iter().all(|dispatch| {
                                        dispatch.as_ref().is_some_and(|dispatch| {
                                            dispatch.source == first.source
                                                && dispatch.cases == first.cases
                                        })
                                    })
                                });
                            if one_subject {
                                let dispatches =
                                    dispatches.into_iter().flatten().collect::<Vec<_>>();
                                self.case_dispatch_split(
                                    identities,
                                    &mut expansion,
                                    &bindings,
                                    &source_types,
                                    result_type,
                                    dispatches,
                                    fallback.as_ref(),
                                    &completions,
                                )?
                            } else {
                                let mut dispatches = dispatches;
                                let closing_subject =
                                    dispatches.last().and_then(Option::as_ref).cloned();
                                let trailing = if fallback.is_none() {
                                    dispatches
                                        .iter()
                                        .rev()
                                        .take_while(|dispatch| {
                                            dispatch
                                                .as_ref()
                                                .zip(closing_subject.as_ref())
                                                .is_some_and(|(dispatch, closing)| {
                                                    dispatch.source == closing.source
                                                        && dispatch.cases == closing.cases
                                                })
                                        })
                                        .count()
                                } else {
                                    0
                                };
                                let trailing = dispatches.split_off(dispatches.len() - trailing);
                                let mut closing = Vec::<ScalarCaseDispatchArm>::new();
                                if !trailing.is_empty() {
                                    for dispatch in trailing.into_iter().flatten().chain(
                                        dispatches
                                            .iter()
                                            .flatten()
                                            .filter(|dispatch| {
                                                closing_subject.as_ref().is_some_and(|closing| {
                                                    dispatch.source == closing.source
                                                        && dispatch.cases == closing.cases
                                                })
                                            })
                                            .cloned(),
                                    ) {
                                        if !closing
                                            .iter()
                                            .any(|armed| armed.selected == dispatch.selected)
                                        {
                                            closing.push(dispatch);
                                        }
                                    }
                                }
                                let closing = if closing.is_empty() {
                                    None
                                } else {
                                    let terminator = self.case_dispatch_split(
                                        identities,
                                        &mut expansion,
                                        &bindings,
                                        &source_types,
                                        result_type,
                                        closing,
                                        None,
                                        &completions,
                                    )?;
                                    Some(expansion.push(LoweredScalarBranchState::continuing(
                                        source_types.clone(),
                                        terminator,
                                    )))
                                };
                                let mut entries = Vec::with_capacity(arms.len());
                                for (guard, dispatch) in arms.iter().zip(dispatches) {
                                    let target = push_arm_target(
                                        identities,
                                        &mut expansion,
                                        &bindings,
                                        &source_types,
                                        result_type,
                                        &guard.destination,
                                        &completions,
                                    )?;
                                    entries.push((guard.guard_statement_ordinal, target, dispatch));
                                }
                                let mut next = if let Some(closing) = closing {
                                    closing
                                } else if let Some(fallback) = fallback {
                                    push_arm_target(
                                        identities,
                                        &mut expansion,
                                        &bindings,
                                        &source_types,
                                        result_type,
                                        fallback,
                                        &completions,
                                    )?
                                } else {
                                    match entries
                                        .last()
                                        .map(|(_, target, dispatch)| (*target, dispatch))
                                    {
                                        Some((target, None)) => target,
                                        Some((_, Some(_))) => {
                                            return unsupported(
                                                "a trailing case dispatch lost its closing split",
                                            );
                                        }
                                        None => {
                                            return Err(LoweringError::Unsupported(
                                                "ordered scalar guards are empty",
                                            ));
                                        }
                                    }
                                };
                                for (guard_ordinal, target, dispatch) in entries.into_iter().rev() {
                                    let terminator = match dispatch {
                                        Some(dispatch) => scalar_case_dispatch_terminator(
                                            dispatch,
                                            target,
                                            next,
                                            &source_types,
                                            self.proof_formal_forwarding(),
                                        ),
                                        None => {
                                            crate::scalar_lowering::branch_states::guards::evaluate(
                                                identities,
                                                checked,
                                                state,
                                                guard_ordinal,
                                                &bindings,
                                                &source_types,
                                                (
                                                    target,
                                                    parameters(&source_types),
                                                    Vec::new(),
                                                    self.proof_formal_forwarding(),
                                                ),
                                                (
                                                    next,
                                                    parameters(&source_types),
                                                    Vec::new(),
                                                    self.proof_formal_forwarding(),
                                                ),
                                                &mut expansion,
                                            )?
                                        }
                                    };
                                    next = expansion.push(LoweredScalarBranchState::continuing(
                                        source_types.clone(),
                                        terminator,
                                    ));
                                }
                                LoweredScalarBranchTerminator::Jump {
                                    cleanup_actions: Vec::new(),
                                    structural_arguments: Vec::new(),
                                    target: next,
                                    arguments: parameters(&source_types),
                                    erased_arguments: Vec::new(),
                                    erased_proof_arguments: self.proof_formal_forwarding(),
                                }
                            }
                        }
                        _ => {
                            return unsupported(
                                "ordered scalar completion requires a returning tail",
                            );
                        }
                    };
                    let entry = expansion.push(LoweredScalarBranchState::continuing(
                        source_types,
                        terminator,
                    ));
                    let states = expansion.finish();
                    self.single_completed_expansion(
                        identities,
                        checked,
                        &states,
                        entry,
                        result_type,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        calls,
                        "ordered scalar control has no unique completed value",
                        environment,
                    )
                }
            }
        }
        pub(crate) mod source_values {
            use super::{
                CheckedCallScalarArgument, CheckedScalarExpressionRole, CheckedTrees, Evaluation,
                LoweringError, ScalarType, ValueDeclaration,
                direct_expression_contains_short_circuit, emit_direct_expression, scalar_carriers,
                scalar_source_types, terminal_scalar_type, unsupported,
                validate_direct_parameter_types,
            };
            use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
            use crate::emission::operation_emission::buffer::OperationBuffer;
            use crate::emission::operation_emission::calls::CallEmissionContext;
            use crate::emission::operation_emission::expressions::LoweredDirectExpression;
            use crate::shared::checked_facts::value_facts;
            use crate::unit::attached_unit::OperationEnvironment;

            impl Evaluation {
                #[allow(clippy::too_many_arguments)]
                pub(crate) fn receiving_value(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    value: ValueDeclaration,
                    receiving_type: semantic_vocabulary::QualifiedScalarType,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                    calls: &mut CallEmissionContext<'_>,
                    environment: &mut OperationEnvironment<'_>,
                ) -> Result<ValueDeclaration, LoweringError> {
                    if value.value_type() == receiving_type {
                        return Ok(value);
                    }
                    if value.scalar_type != receiving_type.scalar_type {
                        return unsupported("scalar receiving membership changes its carrier");
                    }
                    let domains = |qualification: semantic_vocabulary::ScalarQualificationSetId| {
                        if qualification.is_empty() {
                            return Ok(&[][..]);
                        }
                        self.shared_qualifications
                            .catalog()
                            .sets
                            .iter()
                            .find(|set| set.id == qualification)
                            .map(|set| set.domains.as_slice())
                            .ok_or(LoweringError::Unsupported(
                                "scalar receiving value lost its qualification set",
                            ))
                    };
                    let source_domains = domains(value.qualifications)?;
                    let receiving_domains = domains(receiving_type.qualifications)?;
                    if receiving_domains
                        .iter()
                        .all(|domain| source_domains.contains(domain))
                    {
                        return Ok(value);
                    }
                    values.push(value);
                    let source_types = scalar_source_types(values);
                    let operand = crate::scalar_lowering::computations::parameters(&source_types)
                        .pop()
                        .ok_or(LoweringError::Unsupported(
                            "scalar receiving value lost its operand",
                        ))?;
                    let mut expansion = self.scalar_computation_expansion(
                        identities,
                        environment.checked,
                        &self.shared_qualifications,
                        environment.machine,
                        operations,
                        environment.next_place,
                    )?;
                    let qualification =
                        expansion.qualification_edge(&source_types, value.value_type(), 0);
                    let entry = expansion.direct_value(&source_types, qualification, operand)?;
                    let states = expansion.finish();
                    let completed = self.single_completed_expansion(
                        identities,
                        environment.checked,
                        &states,
                        entry,
                        receiving_type,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        calls,
                        "scalar receiving declaration has no completed value",
                        environment,
                    )?;
                    values.pop().ok_or(LoweringError::Unsupported(
                        "scalar receiving continuation lost its staged operand",
                    ))?;
                    Ok(completed)
                }

                pub(crate) fn retained_observation(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &CheckedTrees,
                    root: &checked_trees::CheckedScalarComputationRoot,
                    bindings: &crate::expression_preparation::bindings::ScalarBindings,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                    calls: &mut CallEmissionContext<'_>,
                    environment: &mut OperationEnvironment<'_>,
                ) -> Result<ValueDeclaration, LoweringError> {
                    let source_types = scalar_source_types(values);
                    let result_type = crate::scalar_lowering::computations::computation_value_type(
                        identities,
                        checked,
                        &self.shared_qualifications,
                        root.root,
                        bindings,
                        &source_types,
                    )?;
                    let observation_scope = operations.reserve_observation_scope()?;
                    let mut expansion = self
                        .scalar_computation_expansion(
                            identities,
                            checked,
                            &self.shared_qualifications,
                            root.machine,
                            operations,
                            environment.next_place,
                        )?
                        .with_declaration_observation(self.current, observation_scope);
                    let entry = expansion.retained_root(
                        identities,
                        root,
                        bindings,
                        &source_types,
                        result_type,
                        0,
                    )?;
                    let states = expansion.finish();
                    self.single_completed_expansion(
                        identities,
                        checked,
                        &states,
                        entry,
                        result_type,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        calls,
                        "retained observation has no single completed value",
                        environment,
                    )
                }

                pub(crate) fn branch_guard(
                    &self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &CheckedTrees,
                    state: symbols::SymbolHandle,
                    statement: u32,
                    values: &[ValueDeclaration],
                    successor_reads_payloads: bool,
                ) -> Result<Option<LoweredBooleanReturnExpression>, LoweringError> {
                    let role = CheckedScalarExpressionRole::Guard;
                    if value_facts(checked)
                        .scalar_computations
                        .roots
                        .iter()
                        .any(|(_, root)| {
                            root.state == state
                                && root.statement_ordinal == statement
                                && root.role == role
                        })
                    {
                        return Ok(None);
                    }
                    let (_, value) = value_facts(checked)
                        .scalar_expressions
                        .bound_expression_at(state, statement, role)
                        .ok_or(LoweringError::Unsupported(
                            "guard has no retained source value",
                        ))?;
                    let bindings = self.contextual_bindings();
                    let expression = bindings.expression(
                        identities,
                        &checked.facts.proven_scalar_indexes,
                        value,
                    )?;
                    if !direct_expression_contains_short_circuit(&expression)
                        && !(successor_reads_payloads
                            && direct_expression_tests_whole_root_case(&expression))
                    {
                        return Ok(None);
                    }
                    validate_direct_parameter_types(
                        &expression,
                        &values
                            .iter()
                            .map(|value| value.scalar_type)
                            .collect::<Vec<_>>(),
                    )?;
                    let LoweredDirectExpression::Boolean { expression } = expression else {
                        return unsupported("branch guard source value is not Boolean");
                    };
                    Ok(Some(*expression))
                }

                #[allow(clippy::too_many_arguments)]
                pub(crate) fn guard_value(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &CheckedTrees,
                    machine: symbols::SymbolHandle,
                    state: symbols::SymbolHandle,
                    statement: u32,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                    calls: &mut CallEmissionContext<'_>,
                    environment: &mut OperationEnvironment<'_>,
                ) -> Result<ValueDeclaration, LoweringError> {
                    let role = CheckedScalarExpressionRole::Guard;
                    let mut computations = value_facts(checked)
                        .scalar_computations
                        .roots
                        .iter()
                        .map(|(_, root)| root)
                        .filter(|root| {
                            root.state == state
                                && root.statement_ordinal == statement
                                && root.role == role
                        });
                    let value = if let Some(root) = computations.next() {
                        if computations.next().is_some() || root.machine != machine {
                            return unsupported("guard computation has no unique source owner");
                        }
                        CheckedCallScalarArgument::Computation(root.root)
                    } else {
                        let (_, value) = value_facts(checked)
                            .scalar_expressions
                            .bound_expression_at(state, statement, role)
                            .ok_or(LoweringError::Unsupported(
                                "guard has no retained source value",
                            ))?;
                        CheckedCallScalarArgument::Pure(value.clone())
                    };
                    let value = self.source_value(
                        identities,
                        checked,
                        machine,
                        state,
                        statement,
                        role,
                        &value,
                        values.len(),
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        calls,
                        environment,
                    )?;
                    if value.scalar_type != ScalarType::Boolean {
                        return unsupported("guard source value is not Boolean");
                    }
                    Ok(value)
                }

                #[allow(clippy::too_many_arguments)]
                pub(crate) fn source_value(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &CheckedTrees,
                    machine: symbols::SymbolHandle,
                    state: symbols::SymbolHandle,
                    statement: u32,
                    role: CheckedScalarExpressionRole,
                    value: &CheckedCallScalarArgument,
                    source_value_count: usize,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                    calls: &mut CallEmissionContext<'_>,
                    environment: &mut OperationEnvironment<'_>,
                ) -> Result<ValueDeclaration, LoweringError> {
                    if source_value_count > values.len() {
                        return unsupported("scalar source prefix exceeds its retained values");
                    }
                    let primitive_type = match value {
                        CheckedCallScalarArgument::Pure(expression) => expression.primitive_type(),
                        CheckedCallScalarArgument::Computation(handle) => {
                            let nodes = &checked.facts.values.scalar_computations.nodes;
                            nodes
                                .is_valid(*handle)
                                .then(|| nodes.get(*handle).primitive_type)
                        }
                    }
                    .ok_or(LoweringError::Unsupported(
                        "scalar value plan has no primitive carrier",
                    ))?;
                    let scalar_type = terminal_scalar_type(primitive_type)?;
                    match value {
                        CheckedCallScalarArgument::Pure(expression) => {
                            if value_facts(checked)
                                .scalar_expressions
                                .bound_expression_at(state, statement, role)
                                .is_none()
                                && !matches!(
                                    expression,
                                    checked_trees::CheckedScalarExpression::IntegerLiteral { .. }
                                )
                            {
                                return unsupported("scalar value lost its pure source binding");
                            }
                        }
                        CheckedCallScalarArgument::Computation(_) => {
                            if !value_facts(checked).scalar_computations.roots.iter().any(
                                |(_, root)| {
                                    root.state == state
                                        && root.statement_ordinal == statement
                                        && root.role == role
                                },
                            ) {
                                return unsupported("scalar value lost its computation root");
                            }
                        }
                    }
                    let bindings = self.contextual_bindings();
                    let qualifications = &self.shared_qualifications;
                    let source_types = scalar_source_types(values);
                    let (pure_expression, result_type) = match value {
                        CheckedCallScalarArgument::Pure(value) => {
                            let expression = bindings.expression(
                                identities,
                                &checked.facts.proven_scalar_indexes,
                                value,
                            )?;
                            let result_type = expression.value_type(&source_types)?;
                            (Some(expression), result_type)
                        }
                        CheckedCallScalarArgument::Computation(handle) => (
                            None,
                            crate::scalar_lowering::computations::computation_value_type(
                                identities,
                                checked,
                                qualifications,
                                *handle,
                                &bindings,
                                &source_types,
                            )?,
                        ),
                    };
                    if result_type.scalar_type != scalar_type {
                        return unsupported(
                            "scalar source value disagrees with its retained carrier",
                        );
                    }
                    if let Some(expression) = pure_expression
                        && !direct_expression_contains_short_circuit(&expression)
                    {
                        validate_direct_parameter_types(
                            &expression,
                            &scalar_carriers(&source_types),
                        )?;
                        return Ok(ValueDeclaration {
                            qualifications: result_type.qualifications,
                            id: emit_direct_expression(&expression, values, next_value, operations),
                            scalar_type,
                        });
                    }
                    let mut expansion = self.scalar_computation_expansion(
                        identities,
                        checked,
                        qualifications,
                        machine,
                        operations,
                        environment.next_place,
                    )?;
                    let entry = match value {
                        CheckedCallScalarArgument::Pure(_) => expansion.retained_pure_value(
                            identities,
                            state,
                            statement,
                            role,
                            &bindings,
                            &source_types,
                            result_type,
                            0,
                        )?,
                        CheckedCallScalarArgument::Computation(_) => expansion.retained_value(
                            identities,
                            state,
                            statement,
                            role,
                            &bindings,
                            &source_types,
                            result_type,
                            0,
                        )?,
                    };
                    let states = expansion.finish();
                    self.single_completed_expansion(
                        identities,
                        checked,
                        &states,
                        entry,
                        result_type,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        calls,
                        "scalar source has no single completed value",
                        environment,
                    )
                }
            }

            fn direct_expression_tests_whole_root_case(
                expression: &LoweredDirectExpression,
            ) -> bool {
                fn tests(expression: &LoweredBooleanReturnExpression) -> bool {
                    match expression {
                        LoweredBooleanReturnExpression::StructuralCaseMembership {
                            path, ..
                        } => path.is_empty(),
                        LoweredBooleanReturnExpression::Not { operand } => tests(operand),
                        LoweredBooleanReturnExpression::Equal { left, right } => {
                            tests(left) || tests(right)
                        }
                        LoweredBooleanReturnExpression::And { left, right }
                        | LoweredBooleanReturnExpression::Or { left, right } => {
                            tests(left) || tests(right)
                        }
                        _ => false,
                    }
                }
                matches!(expression, LoweredDirectExpression::Boolean { expression } if tests(expression))
            }
        }
        pub(crate) mod state_emission {

            use super::super::super::{
                BlockId, SuccessorEdge, ValueId, block_id, boolean_decision_block_count,
                contains_short_circuit, emit_boolean_expression,
                emit_reserved_boolean_tuple_stage_blocks, emit_scalar_binding,
                lower_boolean_value_decision,
            };
            use super::super::{
                Block, LoweringError, Terminator, ValueDeclaration, allocate_dense,
                direct_expression_contains_short_circuit, edge_id, emit_direct_expression,
                unsupported, validate_direct_parameter_types, value_id,
            };
            use super::declarations;
            use crate::emission::boolean_control::{
                LoweredBooleanDecision, LoweredBooleanDecisionTarget,
                boolean_guard_decision_block_count, emit_inlined_boolean_guard_blocks,
                lower_boolean_control_decision,
            };
            use crate::emission::operation_emission::LoweredScalarBinding;
            use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
            use crate::emission::operation_emission::buffer::OperationBuffer;
            use crate::emission::operation_emission::calls::CallEmissionContext;
            use crate::emission::operation_emission::expressions::LoweredDirectExpression;
            use crate::emission::operation_emission::expressions::emit_prepared_path;
            use crate::expression_preparation::qualifications::SourceScalarReturn;
            use crate::proofs::crash_routes::lowered_erased_scalar_terms;
            use crate::scalar_lowering::branch_states::prepared::{
                LoweredScalarBranchState, LoweredScalarBranchTerminator,
            };
            use crate::scalar_lowering::contracts::{
                erased_proof_formal_declarations, lowered_proof_terms,
            };

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn emit_state(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &checked_trees::CheckedTrees,
                state: &LoweredScalarBranchState,
                mut block: BlockId,
                parameters: &[ValueDeclaration],
                caller_erased_formals: &[ValueDeclaration],
                source_erased_proof_formals: &[checked_trees::CheckedErasedProofParameterPlan],
                targets: &[BlockId],
                state_parameters: &[Vec<ValueDeclaration>],
                machine: Option<semantic_vocabulary::MachineId>,
                qualifications: &terminal_psi::ScalarQualificationCatalog,
                coercions: &mut Vec<terminal_psi::ScalarQualificationCoercion>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &mut OperationBuffer,
                calls: &mut CallEmissionContext<'_>,
                blocks: &mut Vec<Block>,
                environment: &mut crate::unit::attached_unit::OperationEnvironment<'_>,
                evaluation: &mut super::Evaluation,
                claim_edge: Option<&crate::scalar_lowering::computations::PreparedClaimEdge>,
            ) -> Result<(), LoweringError> {
                let erased_formals = declarations(&state.erased_formal_types, next_value)?;
                macro_rules! proof_terms {
                    ($arguments:expr, $namespace:expr $(,)?) => {
                        lowered_proof_terms(checked, $arguments, $namespace, &erased_formals)
                    };
                }
                let mut block_erased_formals = erased_formals.clone();
                let mut block_erased_proof_formals = erased_proof_formal_declarations(
                    identities,
                    checked,
                    source_erased_proof_formals,
                );
                operations.storage_observations.clear();
                let mut values = parameters.to_vec();
                let mut block_parameters = parameters.to_vec();
                let mut block_structural_parameters = state.structural_parameters.clone();
                let mut operation_start = operations.len();
                for binding in &state.bindings {
                    if let LoweredScalarBinding::Expression(LoweredDirectExpression::Boolean {
                        expression,
                    }) = binding
                        && contains_short_circuit(expression)
                    {
                        let decision = lower_boolean_value_decision(expression);
                        let count = boolean_decision_block_count(&decision);
                        let reserved_root = *next_block;
                        *next_block = next_block
                            .checked_add(u64::try_from(count).map_err(|_| {
                                LoweringError::Unsupported("call Boolean block count exceeds u64")
                            })?)
                            .ok_or(LoweringError::Unsupported(
                                "call Boolean block identities exhausted",
                            ))?;
                        let continuation = block_id(allocate_dense(next_block)?);
                        let mut types = values
                            .iter()
                            .map(|value| value.value_type())
                            .collect::<Vec<_>>();
                        types.push(binding.value_type(&types)?);
                        let continuation_parameters = declarations(&types, next_value)?;
                        let prefix = operations[operation_start..].to_vec();
                        let mut decisions = Vec::new();
                        emit_reserved_boolean_tuple_stage_blocks(
                            &decision,
                            &values,
                            block_parameters,
                            continuation,
                            &values.iter().map(|value| value.id).collect::<Vec<_>>(),
                            reserved_root,
                            next_value,
                            next_edge,
                            operations,
                            &mut decisions,
                        );
                        for (index, decision) in decisions.into_iter().enumerate() {
                            let mut decision = decision.ok_or(LoweringError::Unsupported(
                                "call Boolean block is incomplete",
                            ))?;
                            if index == 0 {
                                decision.id = block;
                                decision.structural_parameters =
                                    std::mem::take(&mut block_structural_parameters);
                                decision.erased_scalar_formals =
                                    std::mem::take(&mut block_erased_formals);
                                decision.erased_proof_formals =
                                    std::mem::take(&mut block_erased_proof_formals);
                                decision.operations.splice(0..0, prefix.iter().cloned());
                            }
                            blocks.push(decision);
                        }
                        block = continuation;
                        values = continuation_parameters.clone();
                        block_parameters = continuation_parameters;
                        operation_start = operations.len();
                    } else if let LoweredScalarBinding::DynamicCall(call) = binding
                        && !call.family_actuals.is_empty()
                    {
                        let operands = std::iter::once(&call.selector)
                            .chain(call.arguments.iter())
                            .map(|operand| ValueDeclaration {
                                id: emit_direct_expression(
                                    operand, &values, next_value, operations,
                                ),
                                scalar_type: operand.scalar_type(),
                                qualifications: Default::default(),
                            })
                            .collect::<Vec<_>>();
                        let family_actuals = call
                            .family_actuals
                            .iter()
                            .map(|actual| ValueDeclaration {
                                id: emit_direct_expression(actual, &values, next_value, operations),
                                scalar_type: actual.scalar_type(),
                                qualifications: Default::default(),
                            })
                            .collect::<Vec<_>>();
                        let structural_arguments = call
                            .structural_arguments
                            .iter()
                            .map(|argument| argument.emit(&values, calls, next_value, operations))
                            .collect::<Result<Vec<_>, _>>()?;
                        let claim_transfers =
                            crate::emission::claim_bindings::emit_prepared_claim_transfers(
                                &call.claim_transfers,
                                calls.claims.bindings(),
                                &structural_arguments,
                                &mut evaluation.current_claims,
                            )?;
                        let produced = terminal_psi::OperationResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: value_id(allocate_dense(next_value)?),
                            scalar_type: call.result_type.scalar_type,
                        });
                        let operation = operations.allocate();
                        let frontier = environment.suspension_source_frontier(
                            call.source_coordinate,
                            evaluation,
                            &values,
                            calls,
                            operations,
                        )?;
                        operations.record_source_call(
                            call.source_coordinate,
                            Some(checked_trees::NominalMachineUseSite::Expression(
                                call.source_expression,
                            )),
                            operation,
                            call.slot,
                            &frontier,
                        )?;
                        let mut span = super::BlockEmissionSpan {
                            current_claims: evaluation.current_claims.clone(),
                            erased_scalar_formals: std::mem::take(&mut block_erased_formals),
                            erased_proof_formals: std::mem::take(&mut block_erased_proof_formals),
                            id: block,
                            parameters: std::mem::take(&mut block_parameters),
                            structural_parameters: std::mem::take(&mut block_structural_parameters),
                            operation_start,
                        };
                        let split =
                            crate::emission::operation_emission::dynamic_calls::emit_completed_dynamic_call(
                                &call.interface_identity,
                                &call.requirement_identity,
                                &call.family_tuple,
                                &operands,
                                &family_actuals,
                                structural_arguments,
                                claim_transfers,
                                &produced,
                                operation,
                                &mut span,
                                blocks,
                                next_value,
                                next_block,
                                next_edge,
                                operations,
                                |_| unsupported("scalar call requested structural result storage"),
                            )?;
                        evaluation.current_claims = span.current_claims;
                        block = span.id;
                        block_erased_formals = span.erased_scalar_formals;
                        block_erased_proof_formals = span.erased_proof_formals;
                        block_parameters = span.parameters.clone();
                        block_structural_parameters = span.structural_parameters;
                        if split {
                            values.extend(span.parameters);
                        } else if let terminal_psi::OperationResult::Scalar(value) = produced {
                            values.push(value);
                        }
                        operation_start = span.operation_start;
                        continue;
                    } else {
                        let value_type = binding.value_type(
                            &values
                                .iter()
                                .map(|value| value.value_type())
                                .collect::<Vec<_>>(),
                        )?;
                        let coordinate = match binding {
                            LoweredScalarBinding::DirectCall(call) => {
                                call.source_coordinate.statement()
                            }
                            LoweredScalarBinding::BoundaryCall(call) => {
                                Some(call.source_coordinate)
                            }
                            LoweredScalarBinding::DynamicCall(call) => Some(call.source_coordinate),
                            _ => None,
                        };
                        let frontier = coordinate
                            .map(|coordinate| {
                                environment.suspension_source_frontier(
                                    coordinate, evaluation, &values, calls, operations,
                                )
                            })
                            .transpose()?
                            .unwrap_or_default();
                        let id = emit_scalar_binding(
                            identities,
                            checked,
                            binding,
                            &values,
                            &frontier,
                            caller_erased_formals,
                            next_value,
                            operations,
                            calls,
                            &mut evaluation.current_claims,
                        )?;
                        values.push(ValueDeclaration {
                            qualifications: value_type.qualifications,
                            id,
                            scalar_type: value_type.scalar_type,
                        });
                    }
                }
                let mut effect_span = super::BlockEmissionSpan {
                    current_claims: evaluation.current_claims.clone(),
                    erased_scalar_formals: std::mem::take(&mut block_erased_formals),
                    erased_proof_formals: std::mem::take(&mut block_erased_proof_formals),
                    id: block,
                    parameters: std::mem::take(&mut block_parameters),
                    structural_parameters: std::mem::take(&mut block_structural_parameters),
                    operation_start,
                };
                crate::scalar_lowering::effects::emit(
                    identities,
                    environment,
                    &state.structural_effects,
                    &mut values,
                    next_value,
                    operations,
                    calls,
                    &mut effect_span,
                    blocks,
                    next_block,
                    next_edge,
                    source_erased_proof_formals,
                    evaluation,
                )?;
                evaluation.current_claims = effect_span.current_claims;
                block = effect_span.id;
                block_erased_formals = effect_span.erased_scalar_formals;
                block_erased_proof_formals = effect_span.erased_proof_formals;
                block_parameters = effect_span.parameters;
                block_structural_parameters = effect_span.structural_parameters;
                operation_start = effect_span.operation_start;
                let mut arguments =
            |expressions: &[LoweredDirectExpression]| -> Result<Vec<ValueId>, LoweringError> {
                expressions
                    .iter()
                    .map(|expression| {
                        validate_direct_parameter_types(
                            expression,
                            &values
                                .iter()
                                .map(|value| value.scalar_type)
                                .collect::<Vec<_>>(),
                        )?;
                        if direct_expression_contains_short_circuit(expression) {
                            return unsupported(
                                "call computation transfer retains unexpanded Boolean control",
                            );
                        }
                        Ok(emit_direct_expression(
                            expression, &values, next_value, operations,
                        ))
                    })
                    .collect()
            };
                let terminator = match &state.terminator {
                    LoweredScalarBranchTerminator::CompleteReturn { target, exit } => {
                        let edge = edge_id(allocate_dense(next_edge)?);
                        operations.source_returns.push(SourceScalarReturn {
                            edge,
                            exits: vec![*exit],
                        });
                        Terminator::Jump {
                            edge,
                            target: *targets.get(*target).ok_or(LoweringError::Unsupported(
                                "scalar return completion target is absent",
                            ))?,
                            arguments: values.iter().map(|value| value.id).collect(),
                            erased_arguments: Vec::new(),
                            erased_proof_arguments: (0..block_erased_proof_formals.len())
                                .map(|position| {
                                    Ok(semantic_vocabulary::ProofTerm::Formal {
                                        position: u32::try_from(position).map_err(|_| {
                                            LoweringError::Unsupported(
                                                "scalar completion proof position exceeds u32",
                                            )
                                        })?,
                                    })
                                })
                                .collect::<Result<Vec<_>, LoweringError>>()?,
                            structural_arguments: Vec::new(),
                            cleanup_actions: Vec::new(),
                        }
                    }
                    LoweredScalarBranchTerminator::Qualify {
                        target,
                        arguments: qualify_arguments,
                        erased_arguments: qualify_erased_arguments,
                        erased_proof_arguments: qualify_erased_proof_arguments,
                        structural_arguments: qualify_structural_arguments,
                    } => {
                        let mut terminator = super::qualification_edge::emit(
                            *targets.get(*target).ok_or(LoweringError::Unsupported(
                                "call computation qualification target is absent",
                            ))?,
                            edge_id(allocate_dense(next_edge)?),
                            machine.ok_or(LoweringError::Unsupported(
                                "attached scalar qualification has no machine identity",
                            ))?,
                            qualify_arguments,
                            qualify_structural_arguments,
                            &values,
                            state_parameters
                                .get(*target)
                                .ok_or(LoweringError::Unsupported(
                                    "scalar qualification has no successor declarations",
                                ))?,
                            qualifications,
                            coercions,
                        )?;
                        let Terminator::Jump {
                            erased_arguments: scalar_actuals,
                            erased_proof_arguments: proof_actuals,
                            ..
                        } = &mut terminator
                        else {
                            return unsupported("scalar qualification lost its successor edge");
                        };
                        *scalar_actuals = lowered_erased_scalar_terms(
                            qualify_erased_arguments,
                            &values,
                            &erased_formals,
                        )?;
                        *proof_actuals = completion_roster(
                            proof_terms!(qualify_erased_proof_arguments, &values)?,
                            source_erased_proof_formals,
                        );
                        terminator
                    }
                    LoweredScalarBranchTerminator::Capture {
                        target,
                        arguments: outgoing,
                        source,
                        path,
                        access,
                    } => {
                        let arguments = arguments(outgoing)?;
                        let source = source.resolve(*access, operations)?;
                        let mut captured_path = source.path;
                        captured_path
                            .extend(emit_prepared_path(path, 0, &values, next_value, operations));
                        Terminator::Jump {
                            edge: edge_id(allocate_dense(next_edge)?),
                            target: *targets.get(*target).ok_or(LoweringError::Unsupported(
                                "storage observation capture has no continuation",
                            ))?,
                            arguments,
                            erased_arguments: Vec::new(),
                            erased_proof_arguments: Vec::new(),
                            structural_arguments: vec![terminal_psi::StructuralArgument {
                                place: source.place,
                                path: captured_path,
                                access: *access,
                            }],
                            cleanup_actions: Vec::new(),
                        }
                    }
                    LoweredScalarBranchTerminator::Jump {
                        target,
                        arguments: outgoing,
                        erased_arguments,
                        erased_proof_arguments,
                        structural_arguments,
                        cleanup_actions,
                    } => {
                        let arguments = arguments(outgoing)?;
                        let erased_arguments = lowered_erased_scalar_terms(
                            erased_arguments,
                            &values,
                            &erased_formals,
                        )?;
                        let erased_proof_arguments = completion_roster(
                            lowered_proof_terms(
                                checked,
                                erased_proof_arguments,
                                &values,
                                &erased_formals,
                            )?,
                            source_erased_proof_formals,
                        );
                        let regions = crate::unit::unit_cleanup::emit_prepared_cleanup_regions(
                            cleanup_actions,
                            calls,
                        )?;
                        let cleanup_actions = if regions.iter().any(|region| {
                            region.partition_leaf.is_some()
                                || !region.conditions.is_empty()
                                || region.selected_region.is_some()
                        }) {
                            super::cleanup_control::CleanupEmission {
                                structural_types: calls.structural_types,
                                source_parameters: &evaluation.structural_parameters,
                                previous_parameter_homes: &evaluation.previous_parameter_homes,
                                checked,
                                current: &mut block,
                                parameters: &mut block_parameters,
                                structural_parameters: &mut block_structural_parameters,
                                block_erased_scalar_formals: &mut block_erased_formals,
                                erased_proof_formals: source_erased_proof_formals,
                                operation_start: &mut operation_start,
                                blocks,
                            }
                            .emit(
                                identities,
                                &regions,
                                &mut values,
                                next_value,
                                next_block,
                                next_edge,
                                operations,
                            )?;
                            Vec::new()
                        } else {
                            regions
                                .into_iter()
                                .filter_map(|region| region.action.into_physical())
                                .collect()
                        };
                        let edge = edge_id(allocate_dense(next_edge)?);
                        if let Some(plan) = claim_edge {
                            let aliases =
                                crate::emission::claim_bindings::selections::edge_bindings(
                                    checked,
                                    &plan.destinations,
                                    &plan.route,
                                    edge,
                                    &calls.claims,
                                )?;
                            evaluation.current_claims =
                                evaluation.current_claims.with_edge_aliases(&aliases)?;
                            operations.claim_edge_bindings.extend(aliases);
                        }
                        Terminator::Jump {
                            structural_arguments: structural_arguments.clone(),
                            edge,
                            target: *targets.get(*target).ok_or(LoweringError::Unsupported(
                                "call computation target is absent",
                            ))?,
                            arguments,
                            erased_arguments,
                            erased_proof_arguments,
                            cleanup_actions,
                        }
                    }
                    LoweredScalarBranchTerminator::Conditional {
                        condition,
                        when_true_target,
                        when_true_arguments,
                        when_true_erased_arguments,
                        when_false_target,
                        when_false_arguments,
                        when_false_erased_arguments,
                        when_true_erased_proof_arguments,
                        when_false_erased_proof_arguments,
                    } => {
                        let when_true_arguments = arguments(when_true_arguments)?;
                        let when_false_arguments = arguments(when_false_arguments)?;
                        if contains_short_circuit(condition) {
                            if !when_true_erased_arguments.is_empty()
                                || !when_false_erased_arguments.is_empty()
                                || !when_true_erased_proof_arguments.is_empty()
                                || !when_false_erased_proof_arguments.is_empty()
                            {
                                return unsupported(
                                    "erased call operands do not stage through guard decisions",
                                );
                            }
                            let when_true_block = *targets
                                .get(*when_true_target)
                                .ok_or(LoweringError::Unsupported("call true target is absent"))?;
                            let when_false_block = *targets
                                .get(*when_false_target)
                                .ok_or(LoweringError::Unsupported("call false target is absent"))?;
                            let decision = lower_boolean_control_decision(
                                condition,
                                LoweredBooleanDecision::Value(
                                    LoweredBooleanReturnExpression::Constant { value: true },
                                ),
                                LoweredBooleanDecision::Value(
                                    LoweredBooleanReturnExpression::Constant { value: false },
                                ),
                            );
                            let decision_tests = boolean_guard_decision_block_count(&decision);
                            debug_assert!(decision_tests > 0);
                            let first_synthetic_block = block_id(*next_block);
                            *next_block = next_block
                                .checked_add(u64::try_from(decision_tests - 1).map_err(|_| {
                                    LoweringError::Unsupported(
                                        "call guard decision count exceeds u64",
                                    )
                                })?)
                                .ok_or(LoweringError::Unsupported(
                                    "call guard decision identities exhausted",
                                ))?;
                            let terminator_operation_start = operations.len();
                            let (root, children) = emit_inlined_boolean_guard_blocks(
                                &decision,
                                &values,
                                block_parameters,
                                &LoweredBooleanDecisionTarget {
                                    block: when_true_block,
                                    arguments: when_true_arguments,
                                },
                                &LoweredBooleanDecisionTarget {
                                    block: when_false_block,
                                    arguments: when_false_arguments,
                                },
                                block,
                                first_synthetic_block,
                                next_value,
                                next_edge,
                                operations,
                            );
                            let mut root = root;
                            root.structural_parameters =
                                std::mem::take(&mut block_structural_parameters);
                            root.erased_scalar_formals = block_erased_formals;
                            root.erased_proof_formals = block_erased_proof_formals;
                            root.operations.splice(
                                0..0,
                                operations[operation_start..terminator_operation_start]
                                    .iter()
                                    .cloned(),
                            );
                            blocks.push(root);
                            blocks.extend(children);
                            return Ok(());
                        }
                        let when_true_erased_arguments = lowered_erased_scalar_terms(
                            when_true_erased_arguments,
                            &values,
                            &erased_formals,
                        )?;
                        let when_false_erased_arguments = lowered_erased_scalar_terms(
                            when_false_erased_arguments,
                            &values,
                            &erased_formals,
                        )?;
                        let when_true_erased_proof_arguments = completion_roster(
                            proof_terms!(when_true_erased_proof_arguments, &values)?,
                            source_erased_proof_formals,
                        );
                        let when_false_erased_proof_arguments = completion_roster(
                            proof_terms!(when_false_erased_proof_arguments, &values)?,
                            source_erased_proof_formals,
                        );
                        let condition =
                            emit_boolean_expression(condition, &values, next_value, operations);
                        Terminator::Conditional {
                            condition,
                            when_true: SuccessorEdge {
                                structural_arguments: Vec::new(),
                                edge: edge_id(allocate_dense(next_edge)?),
                                target: *targets.get(*when_true_target).ok_or(
                                    LoweringError::Unsupported("call true target is absent"),
                                )?,
                                arguments: when_true_arguments,
                                erased_arguments: when_true_erased_arguments,
                                erased_proof_arguments: when_true_erased_proof_arguments,
                                cleanup_actions: Vec::new(),
                            },
                            when_false: SuccessorEdge {
                                structural_arguments: Vec::new(),
                                edge: edge_id(allocate_dense(next_edge)?),
                                target: *targets.get(*when_false_target).ok_or(
                                    LoweringError::Unsupported("call false target is absent"),
                                )?,
                                arguments: when_false_arguments,
                                erased_arguments: when_false_erased_arguments,
                                erased_proof_arguments: when_false_erased_proof_arguments,
                                cleanup_actions: Vec::new(),
                            },
                        }
                    }
                    LoweredScalarBranchTerminator::CaseDispatch {
                        source,
                        selected,
                        cases,
                        payloads,
                        when_true_target,
                        when_true_arguments,
                        when_true_erased_arguments,
                        when_true_erased_proof_arguments,
                        when_false_target,
                        when_false_arguments,
                        when_false_erased_arguments,
                        when_false_erased_proof_arguments,
                    } => {
                        let mut dispatch_namespace = values.clone();
                        let payload_parameters = payloads
                            .iter()
                            .map(|(_, value_type)| {
                                let declaration = ValueDeclaration {
                                    qualifications: value_type.qualifications,
                                    id: value_id(allocate_dense(next_value)?),
                                    scalar_type: value_type.scalar_type,
                                };
                                dispatch_namespace.push(declaration);
                                Ok(declaration)
                            })
                            .collect::<Result<Vec<_>, LoweringError>>()?;
                        let selected_block = block_id(allocate_dense(next_block)?);
                        let has_fallback = cases.iter().any(|case| *case != *selected);
                        let fallback_block = if has_fallback {
                            Some(block_id(allocate_dense(next_block)?))
                        } else {
                            None
                        };
                        let when_true_arguments = dispatch_arguments(
                            when_true_arguments,
                            &dispatch_namespace,
                            next_value,
                            operations,
                        )?;
                        let when_false_arguments = if has_fallback {
                            dispatch_arguments(
                                when_false_arguments,
                                &values,
                                next_value,
                                operations,
                            )?
                        } else {
                            Vec::new()
                        };
                        let when_true_erased_arguments = lowered_erased_scalar_terms(
                            when_true_erased_arguments,
                            &dispatch_namespace,
                            &erased_formals,
                        )?;
                        let when_true_erased_proof_arguments =
                            proof_terms!(when_true_erased_proof_arguments, &dispatch_namespace)?;
                        let (when_false_erased_arguments, when_false_erased_proof_arguments) =
                            if has_fallback {
                                (
                                    lowered_erased_scalar_terms(
                                        when_false_erased_arguments,
                                        &values,
                                        &erased_formals,
                                    )?,
                                    proof_terms!(when_false_erased_proof_arguments, &values)?,
                                )
                            } else {
                                (Vec::new(), Vec::new())
                            };
                        blocks.push(Block::plain(
                            selected_block,
                            payload_parameters,
                            Terminator::Jump {
                                structural_arguments: Vec::new(),
                                edge: edge_id(allocate_dense(next_edge)?),
                                target: *targets.get(*when_true_target).ok_or(
                                    LoweringError::Unsupported("call true target is absent"),
                                )?,
                                arguments: when_true_arguments,
                                erased_arguments: when_true_erased_arguments,
                                erased_proof_arguments: when_true_erased_proof_arguments,
                                cleanup_actions: Vec::new(),
                            },
                        ));
                        if let Some(fallback_block) = fallback_block {
                            blocks.push(Block::plain(
                                fallback_block,
                                Vec::new(),
                                Terminator::Jump {
                                    structural_arguments: Vec::new(),
                                    edge: edge_id(allocate_dense(next_edge)?),
                                    target: *targets.get(*when_false_target).ok_or(
                                        LoweringError::Unsupported("call false target is absent"),
                                    )?,
                                    arguments: when_false_arguments,
                                    erased_arguments: when_false_erased_arguments,
                                    erased_proof_arguments: when_false_erased_proof_arguments,
                                    cleanup_actions: Vec::new(),
                                },
                            ));
                        }
                        let mut case_edges = Vec::with_capacity(cases.len());
                        for case in cases {
                            case_edges.push(terminal_psi::StructuralCaseSuccessorEdge {
                                structural_arguments: Vec::new(),
                                edge: edge_id(allocate_dense(next_edge)?),
                                target: if *case == *selected {
                                    selected_block
                                } else {
                                    fallback_block
                                        .expect("non-selected case edge needs a fallback block")
                                },
                                case: *case,
                                payload_fields: if *case == *selected {
                                    payloads.iter().map(|(field, _)| *field).collect()
                                } else {
                                    Vec::new()
                                },
                                cleanup_actions: Vec::new(),
                            });
                        }
                        Terminator::StructuralCase {
                            source: *source,
                            path: Vec::new(),
                            cases: case_edges,
                        }
                    }
                    LoweredScalarBranchTerminator::CaseDispatchSplit {
                        source,
                        cases,
                        armed,
                        fallback_target,
                        fallback_arguments,
                        fallback_erased_arguments,
                        fallback_erased_proof_arguments,
                    } => {
                        let mut case_edges = Vec::with_capacity(cases.len());
                        let mut fallback_block = None;
                        for case in cases {
                            if let Some(arm) = armed.iter().find(|arm| arm.case == *case) {
                                let mut dispatch_namespace = values.clone();
                                let mut payload_parameters = Vec::with_capacity(arm.payloads.len());
                                for (_, value_type) in &arm.payloads {
                                    let declaration = ValueDeclaration {
                                        qualifications: value_type.qualifications,
                                        id: value_id(allocate_dense(next_value)?),
                                        scalar_type: value_type.scalar_type,
                                    };
                                    dispatch_namespace.push(declaration);
                                    payload_parameters.push(declaration);
                                }
                                let selected_block = block_id(allocate_dense(next_block)?);
                                let arguments = dispatch_arguments(
                                    &arm.arguments,
                                    &dispatch_namespace,
                                    next_value,
                                    operations,
                                )?;
                                let erased_arguments = lowered_erased_scalar_terms(
                                    &arm.erased_arguments,
                                    &dispatch_namespace,
                                    &erased_formals,
                                )?;
                                let erased_proof_arguments =
                                    proof_terms!(&arm.erased_proof_arguments, &dispatch_namespace)?;
                                blocks.push(Block::plain(
                                    selected_block,
                                    payload_parameters,
                                    Terminator::Jump {
                                        structural_arguments: Vec::new(),
                                        edge: edge_id(allocate_dense(next_edge)?),
                                        target: *targets.get(arm.target).ok_or(
                                            LoweringError::Unsupported(
                                                "call case arm target is absent",
                                            ),
                                        )?,
                                        arguments,
                                        erased_arguments,
                                        erased_proof_arguments,
                                        cleanup_actions: Vec::new(),
                                    },
                                ));
                                case_edges.push(terminal_psi::StructuralCaseSuccessorEdge {
                                    structural_arguments: Vec::new(),
                                    edge: edge_id(allocate_dense(next_edge)?),
                                    target: selected_block,
                                    case: *case,
                                    payload_fields: arm
                                        .payloads
                                        .iter()
                                        .map(|(field, _)| *field)
                                        .collect(),
                                    cleanup_actions: Vec::new(),
                                });
                            } else {
                                if fallback_block.is_none() {
                                    let block = block_id(allocate_dense(next_block)?);
                                    let arguments = dispatch_arguments(
                                        fallback_arguments,
                                        &values,
                                        next_value,
                                        operations,
                                    )?;
                                    let erased_arguments = lowered_erased_scalar_terms(
                                        fallback_erased_arguments,
                                        &values,
                                        &erased_formals,
                                    )?;
                                    let erased_proof_arguments =
                                        proof_terms!(fallback_erased_proof_arguments, &values)?;
                                    blocks.push(Block::plain(
                                        block,
                                        Vec::new(),
                                        Terminator::Jump {
                                            structural_arguments: Vec::new(),
                                            edge: edge_id(allocate_dense(next_edge)?),
                                            target: *targets.get(*fallback_target).ok_or(
                                                LoweringError::Unsupported(
                                                    "call case fallback target is absent",
                                                ),
                                            )?,
                                            arguments,
                                            erased_arguments,
                                            erased_proof_arguments,
                                            cleanup_actions: Vec::new(),
                                        },
                                    ));
                                    fallback_block = Some(block);
                                }
                                case_edges.push(terminal_psi::StructuralCaseSuccessorEdge {
                                    structural_arguments: Vec::new(),
                                    edge: edge_id(allocate_dense(next_edge)?),
                                    target: fallback_block
                                        .expect("armed-edge loop emitted fallback"),
                                    case: *case,
                                    payload_fields: Vec::new(),
                                    cleanup_actions: Vec::new(),
                                });
                            }
                        }
                        Terminator::StructuralCase {
                            source: *source,
                            path: Vec::new(),
                            cases: case_edges,
                        }
                    }
                };
                blocks.push(Block {
                    structural_parameters: block_structural_parameters,
                    id: block,
                    parameters: block_parameters,
                    erased_scalar_formals: block_erased_formals,
                    erased_proof_formals: block_erased_proof_formals,
                    operations: operations[operation_start..].to_vec(),
                    terminator,
                });
                Ok(())
            }

            fn dispatch_arguments(
                expressions: &[LoweredDirectExpression],
                namespace: &[ValueDeclaration],
                next_value: &mut u64,
                operations: &mut OperationBuffer,
            ) -> Result<Vec<ValueId>, LoweringError> {
                expressions
                    .iter()
                    .map(|expression| {
                        validate_direct_parameter_types(
                            expression,
                            &namespace
                                .iter()
                                .map(|value| value.scalar_type)
                                .collect::<Vec<_>>(),
                        )?;
                        if direct_expression_contains_short_circuit(expression) {
                            return unsupported(
                                "call computation transfer retains unexpanded Boolean control",
                            );
                        }
                        Ok(emit_direct_expression(
                            expression, namespace, next_value, operations,
                        ))
                    })
                    .collect()
            }

            fn completion_roster(
                actuals: Vec<semantic_vocabulary::ProofTerm>,
                roster: &[checked_trees::CheckedErasedProofParameterPlan],
            ) -> Vec<semantic_vocabulary::ProofTerm> {
                if !actuals.is_empty() || roster.is_empty() {
                    return actuals;
                }
                roster
                    .iter()
                    .map(|parameter| semantic_vocabulary::ProofTerm::Formal {
                        position: parameter.proof_position,
                    })
                    .collect()
            }
        }
        pub(crate) mod straight_line {
            use super::{
                Evaluation, LoweringError, QualifiedScalarType, ValueDeclaration,
                direct_expression_contains_short_circuit, emit_direct_expression, unsupported,
                validate_direct_parameter_types,
            };
            use crate::emission::operation_emission::buffer::OperationBuffer;
            use crate::emission::operation_emission::calls::CallEmissionContext;
            use crate::emission::operation_emission::{LoweredScalarBinding, emit_scalar_binding};
            use crate::scalar_lowering::branch_states::prepared::{
                LoweredScalarBranchState, LoweredScalarBranchTerminator,
            };

            impl Evaluation {
                pub(crate) fn complete_straight_line(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &checked_trees::CheckedTrees,
                    states: &[LoweredScalarBranchState],
                    entry: usize,
                    result_types: &[QualifiedScalarType],
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    operations: &mut OperationBuffer,
                    calls: &mut CallEmissionContext<'_>,
                    environment: &mut crate::unit::attached_unit::OperationEnvironment<'_>,
                ) -> Result<Option<Vec<ValueDeclaration>>, LoweringError> {
                    let source_count = values.len();
                    let mut chain = Vec::new();
                    let mut index = entry;
                    let mut types = values
                        .iter()
                        .map(|value| value.value_type())
                        .collect::<Vec<_>>();
                    let proof_forwarding = self.proof_formal_forwarding();
                    while index != 0 {
                        if chain.contains(&index) {
                            return Ok(None);
                        }
                        let state = states.get(index - 1).ok_or(LoweringError::Unsupported(
                            "scalar computation chain lost its continuation",
                        ))?;
                        let LoweredScalarBranchTerminator::Jump {
                            target,
                            arguments,
                            erased_arguments,
                            erased_proof_arguments,
                            structural_arguments,
                            cleanup_actions,
                        } = &state.terminator
                        else {
                            return Ok(None);
                        };
                        if !state.structural_parameters.is_empty()
                            || !state.structural_effects.is_empty()
                            || !state.erased_formal_types.is_empty()
                            || !erased_arguments.is_empty()
                            || *erased_proof_arguments != proof_forwarding
                            || !structural_arguments.is_empty()
                            || !cleanup_actions.is_empty()
                            || state.parameter_types != types
                        {
                            return Ok(None);
                        }
                        for binding in &state.bindings {
                            match binding {
                                LoweredScalarBinding::Expression(expression)
                                    if !direct_expression_contains_short_circuit(expression) => {}
                                LoweredScalarBinding::SelectedComparison {
                                    left, right, ..
                                } if !direct_expression_contains_short_circuit(left)
                                    && !direct_expression_contains_short_circuit(right) => {}
                                LoweredScalarBinding::DirectCall(call)
                                    if call.arguments.iter().all(|argument| {
                                        !direct_expression_contains_short_circuit(argument)
                                    }) => {}
                                _ => return Ok(None),
                            }
                            types.push(binding.value_type(&types)?);
                        }
                        if arguments
                            .iter()
                            .any(direct_expression_contains_short_circuit)
                        {
                            return Ok(None);
                        }
                        types = arguments
                            .iter()
                            .map(|value| value.value_type(&types))
                            .collect::<Result<Vec<_>, _>>()?;
                        chain.push(index);
                        index = *target;
                    }
                    let mut completion_types = values
                        .iter()
                        .map(|value| value.value_type())
                        .collect::<Vec<_>>();
                    completion_types.extend_from_slice(result_types);
                    if types != completion_types {
                        return unsupported(
                            "scalar computation chain changes its completion telescope",
                        );
                    }
                    let mut completed = values.clone();
                    for index in chain {
                        operations.storage_observations.clear();
                        let state = &states[index - 1];
                        for binding in &state.bindings {
                            let value_type = binding.value_type(
                                &completed
                                    .iter()
                                    .map(|value| value.value_type())
                                    .collect::<Vec<_>>(),
                            )?;
                            let frontier = if let LoweredScalarBinding::DirectCall(call) = binding {
                                call.source_coordinate
                                    .statement()
                                    .map(|coordinate| {
                                        environment.suspension_source_frontier(
                                            coordinate, self, &completed, calls, operations,
                                        )
                                    })
                                    .transpose()?
                                    .unwrap_or_default()
                            } else {
                                Default::default()
                            };
                            let id = emit_scalar_binding(
                                identities,
                                checked,
                                binding,
                                &completed,
                                &frontier,
                                &self.erased_scalar_formals,
                                next_value,
                                operations,
                                calls,
                                &mut self.current_claims,
                            )?;
                            completed.push(ValueDeclaration {
                                id,
                                scalar_type: value_type.scalar_type,
                                qualifications: value_type.qualifications,
                            });
                        }
                        let LoweredScalarBranchTerminator::Jump { arguments, .. } =
                            &state.terminator
                        else {
                            return unsupported(
                                "preflighted scalar chain ends in a continuation terminator",
                            );
                        };
                        completed = arguments
                            .iter()
                            .map(|expression| {
                                validate_direct_parameter_types(
                                    expression,
                                    &completed
                                        .iter()
                                        .map(|value| value.scalar_type)
                                        .collect::<Vec<_>>(),
                                )?;
                                let value_type = expression.value_type(
                                    &completed
                                        .iter()
                                        .map(|value| value.value_type())
                                        .collect::<Vec<_>>(),
                                )?;
                                Ok(ValueDeclaration {
                                    id: emit_direct_expression(
                                        expression, &completed, next_value, operations,
                                    ),
                                    scalar_type: value_type.scalar_type,
                                    qualifications: value_type.qualifications,
                                })
                            })
                            .collect::<Result<Vec<_>, LoweringError>>()?;
                    }
                    let results = completed[source_count..].to_vec();
                    *values = completed[..source_count].to_vec();
                    operations.storage_observations.clear();
                    Ok(Some(results))
                }
            }
        }

        pub(crate) use scalar_control::ScalarReturnCleanup;

        #[derive(Clone, PartialEq, Eq)]
        pub(crate) struct StructuralValueStorage {
            pub(crate) place: PlaceId,
            pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
            pub(crate) selected_type: semantic_vocabulary::StructuralTypeId,
        }

        impl From<&StructuralBinding> for StructuralValueStorage {
            fn from(binding: &StructuralBinding) -> Self {
                Self {
                    place: binding.place(),
                    path: binding.path().to_vec(),
                    selected_type: binding.structural_type(),
                }
            }
        }

        pub(crate) struct Evaluation {
            pub(crate) scalar_applications:
                std::sync::Arc<super::scalar_applications::SourceScalarApplications>,
            pub(crate) current_claims: crate::emission::current_claims::CurrentClaimNames,
            pub(crate) shared_qualifications: PreparedScalarQualifications,
            pub(crate) machine: Option<semantic_vocabulary::MachineId>,
            pub(crate) qualification_coercions: Vec<terminal_psi::ScalarQualificationCoercion>,
            pub(crate) structural_types: std::sync::Arc<[StructuralTypeDeclaration]>,
            pub(crate) structural_value_owners: Vec<StructuralValueOwner>,
            pub(crate) selection_cleanups: Vec<SelectionCleanup>,
            pub(crate) structural_locals: Vec<StructuralLocalBinding>,
            pub(crate) completed_operand_scope: Vec<checked_trees::CheckedStructuralValueHandle>,
            pub(crate) completed_cleanup_homes: Vec<StructuralLocalBinding>,
            pub(crate) retired_structural_bindings: Vec<RetiredStructuralBinding>,
            pub(crate) retired_structural_homes: StructuralHomeEndings,
            pub(crate) view_locals: Vec<ViewLocalBinding>,
            pub(crate) element_views: std::collections::BTreeMap<
                semantic_vocabulary::StructuralTypeId,
                Option<ScalarType>,
            >,
            pub(crate) subslices: Vec<crate::scalar_lowering::computations::subslices::Slot>,
            pub(crate) observations: Vec<crate::scalar_lowering::computations::observations::Slot>,
            pub(crate) produced_values:
                Vec<crate::scalar_lowering::computations::produced_values::Slot>,
            pub(crate) literals: Vec<crate::scalar_lowering::computations::literals::Slot>,
            pub(crate) cases: Vec<crate::scalar_lowering::computations::cases::Slot>,
            pub(crate) record_fields: Vec<crate::scalar_lowering::computations::fields::Binding>,
            pub(crate) place_observations: std::sync::Arc<[PlaceObservation]>,
            pub(crate) computation_address_roots: Vec<(
                symbols::SymbolHandle,
                Vec<(u32, checked_trees::CheckedScalarComputationHandle)>,
            )>,
            pub(crate) parameter_storage: Vec<super::primitive_locals::PrimitiveLocal>,
            pub primitive_storage: Vec<(symbols::SymbolHandle, PlaceId, ScalarType)>,
            pub scalar_bindings: ScalarBindings,
            pub structural_parameters: Vec<(u32, StructuralParameterDeclaration)>,
            pub(crate) previous_parameter_homes: Vec<StructuralParameterDeclaration>,
            pub(crate) enclosing_structural_arrivals:
                Vec<(BlockId, StructuralParameterDeclaration)>,
            pub structural_fields: crate::expression_preparation::bindings::StructuralFieldBindings,
            pub structural_cases: Vec<StructuralCaseBinding>,
            pub erased_scalar_formals: Vec<ValueDeclaration>,
            pub erased_proof_formals: Vec<checked_trees::CheckedErasedProofParameterPlan>,
            pub(crate) completion_proof_formals: Vec<terminal_psi::ErasedProofFormal>,
            pub entry: BlockId,
            pub current: BlockId,
            pub parameters: Vec<ValueDeclaration>,
            pub block_structural_parameters: Vec<StructuralParameterDeclaration>,
            pub operation_start: usize,
            pub blocks: Vec<Block>,
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        pub(crate) enum StructuralBindingEnd {
            ConsumedSource,
            RetiredHome,
        }

        #[derive(Clone, Default)]
        pub(crate) struct StructuralHomeEndings {
            rows: Vec<(PlaceId, StructuralBindingEnd)>,
        }

        impl StructuralHomeEndings {
            pub(crate) fn contains(&self, place: &PlaceId) -> bool {
                self.rows.iter().any(|(candidate, _)| candidate == place)
            }

            fn is_retired(&self, place: PlaceId) -> bool {
                self.rows.iter().any(|(candidate, end)| {
                    *candidate == place && *end == StructuralBindingEnd::RetiredHome
                })
            }

            fn record(&mut self, place: PlaceId, end: StructuralBindingEnd) {
                if let Some((_, current)) = self
                    .rows
                    .iter_mut()
                    .find(|(candidate, _)| *candidate == place)
                {
                    if end == StructuralBindingEnd::RetiredHome {
                        *current = end;
                    }
                } else {
                    self.rows.push((place, end));
                }
            }

            fn reestablish(&mut self, place: PlaceId) {
                self.rows.retain(|(candidate, _)| *candidate != place);
            }
        }

        #[derive(Clone)]
        pub(crate) struct RetiredStructuralBinding {
            pub(crate) root: facts::PlaceRoot,
            pub(crate) place: PlaceId,
            pub(crate) statement: Option<u32>,
            pub(crate) end: StructuralBindingEnd,
        }

        pub(crate) struct StructuralSourceFrontier {
            owners: Vec<StructuralValueOwner>,
            locals: Vec<StructuralLocalBinding>,
            retired_bindings: Vec<RetiredStructuralBinding>,
            retired_homes: StructuralHomeEndings,
            views: Vec<ViewLocalBinding>,
            parameters: Vec<(u32, StructuralParameterDeclaration)>,
            previous_parameters: Vec<StructuralParameterDeclaration>,
            enclosing_arrivals: Vec<(BlockId, StructuralParameterDeclaration)>,
            fields: crate::expression_preparation::bindings::StructuralFieldBindings,
            cases: Vec<StructuralCaseBinding>,
        }

        #[derive(Clone)]
        pub(crate) struct SelectionCleanup {
            pub(crate) selection: arena::Handle<checked_trees::FlowOwnedSelectionReceipt>,
            pub(crate) sources: Vec<CapturedSelectionSource>,
            pub(crate) retired_reference_paths:
                Vec<(PlaceId, Vec<terminal_psi::StructuralPathSegment>)>,
        }

        #[derive(Clone)]
        pub(crate) struct CapturedSelectionSource {
            pub(crate) source_ordinal: u32,
            pub(crate) storage: StructuralValueStorage,
        }

        #[derive(Clone)]
        pub(crate) struct StructuralValueOwner {
            pub(crate) symbol: symbols::SymbolHandle,
            pub(crate) statement: u32,
            pub(crate) value: terminal_psi::StructuralOperationResult,
        }

        pub(crate) fn scalar_source_types(values: &[ValueDeclaration]) -> Vec<QualifiedScalarType> {
            values.iter().map(|value| value.value_type()).collect()
        }

        impl Evaluation {
            pub(crate) fn capture_selection_sources(
                &mut self,
                selection: arena::Handle<checked_trees::FlowOwnedSelectionReceipt>,
                sources: Vec<CapturedSelectionSource>,
            ) -> Result<(), LoweringError> {
                if !selection.is_valid() {
                    if !sources.is_empty() {
                        return unsupported("selected source storage has no checked receipt");
                    }
                    return Ok(());
                }
                self.retain_selection_sources(&[SelectionCleanup {
                    selection,
                    sources,
                    retired_reference_paths: Vec::new(),
                }])
            }

            pub(crate) fn bind_completed_value_alias(
                &mut self,
                value: checked_trees::CheckedStructuralValueHandle,
                alias: typed_trees::expression::ExpressionHandle,
                operations: &OperationBuffer,
            ) -> Result<(), LoweringError> {
                if !alias.is_valid() {
                    return Ok(());
                }
                let completed = self
                    .completed_value_binding(value, operations)?
                    .ok_or(LoweringError::Unsupported(
                        "saved subject has no completed producer home",
                    ))?
                    .clone();
                crate::expression_preparation::bindings::retain_completed_structural_homes(
                    &mut self.structural_locals,
                    &[StructuralLocalBinding {
                        result_binding_ordinal: None,
                        root: facts::PlaceRoot::Expression(alias),
                        argument: completed.argument,
                        structural_type: completed.structural_type,
                    }],
                )
            }
            pub(crate) fn saved_value_binding<'operations>(
                &self,
                value: checked_trees::CheckedStructuralValueHandle,
                operations: &'operations OperationBuffer,
            ) -> Result<&'operations ComputedValueBinding, LoweringError> {
                self.completed_value_binding(value, operations)?
                    .filter(|binding| {
                        binding.argument.path.is_empty()
                            && binding.argument.access == StructuralAccess::Owned
                    })
                    .ok_or(LoweringError::Unsupported(
                        "saved value requires a completed whole owned home",
                    ))
            }

            /// One exact producer's current physical name, without strengthening its
            /// path or access. Absence means that producer has not completed; a retired
            /// or out-of-frame name is an error, never permission to produce it again.
            /// Consumers must independently authenticate their current source-root
            /// association and retain ordinary operation/loan custody checks.
            pub(crate) fn completed_value_binding<'operations>(
                &self,
                value: checked_trees::CheckedStructuralValueHandle,
                operations: &'operations OperationBuffer,
            ) -> Result<Option<&'operations ComputedValueBinding>, LoweringError> {
                let Some(binding) = operations
                    .computed_values
                    .iter()
                    .rev()
                    .find(|(root, _)| *root == value)
                    .map(|(_, binding)| binding)
                else {
                    return Ok(None);
                };
                if self
                    .retired_structural_homes
                    .contains(&binding.argument.place)
                    || self.current_claims.source(&binding.argument)?.is_none()
                {
                    return unsupported("completed value lost its current physical home");
                }
                Ok(Some(binding))
            }

            /// Reuse a producer only through its exact current source name and type.
            /// None means the producer has not completed in this frame. A cached
            /// producer with a retired home or a mismatched name is an error; it must
            /// not be evaluated again or rebound under another source.
            pub(crate) fn completed_source_binding<'operations>(
                &self,
                value: checked_trees::CheckedStructuralValueHandle,
                root: facts::PlaceRoot,
                structural_type: semantic_vocabulary::StructuralTypeId,
                operations: &'operations OperationBuffer,
            ) -> Result<Option<&'operations ComputedValueBinding>, LoweringError> {
                let Some(binding) = self.completed_value_binding(value, operations)? else {
                    return Ok(None);
                };
                if binding.structural_type != structural_type
                    || structural_local_by_root(&self.structural_locals, root).is_none_or(
                        |source| {
                            source.structural_type != structural_type
                                || source.argument != binding.argument
                        },
                    )
                {
                    return unsupported("completed value does not match its current source home");
                }
                Ok(Some(binding))
            }

            pub(crate) fn contextual_bindings(&self) -> ScalarBindings {
                self.scalar_bindings
                    .clone()
                    .with_primitive_storage(&self.primitive_storage)
                    .with_structural_locals(&self.structural_locals)
                    .with_view_locals(&self.view_locals)
                    .with_element_views(&self.element_views)
                    .with_structural_parameters(&self.structural_parameters)
                    .with_resolved_structural_observations(
                        &self.structural_fields,
                        &self.structural_cases,
                    )
                    .with_place_observations(&self.place_observations)
                    .with_physical_root_types(
                        self.structural_root_parameters()
                            .map(|parameter| (parameter.place, parameter.structural_type))
                            .chain(
                                self.structural_value_owners
                                    .iter()
                                    .map(|owner| (owner.value.place, owner.value.structural_type)),
                            )
                            .chain(
                                self.blocks
                                    .iter()
                                    .flat_map(|block| block.operations.iter())
                                    .filter_map(|operation| operation.result.structural())
                                    .map(|result| (result.place, result.structural_type)),
                            ),
                    )
            }

            pub(crate) fn completed_bindings(
                &self,
                operations: &OperationBuffer,
                declarations: impl IntoIterator<Item = terminal_psi::StructuralPlaceDeclaration>,
            ) -> ScalarBindings {
                self.contextual_bindings()
                    .with_completed_place_declarations(declarations.into_iter().chain(
                        crate::emission::operation_emission::place_declarations::operation_results(
                            operations.iter().chain(
                                self.blocks.iter().flat_map(|block| block.operations.iter()),
                            ),
                        ),
                    ))
            }

            fn scalar_computation_expansion<'a>(
                &'a self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &'a CheckedTrees,
                qualifications: &'a PreparedScalarQualifications,
                machine: symbols::SymbolHandle,
                operations: &OperationBuffer,
                next_place: &'a mut u64,
            ) -> Result<crate::scalar_lowering::computations::Expansion<'a>, LoweringError>
            {
                Ok(crate::scalar_lowering::computations::Expansion::new(
                    checked,
                    &self.scalar_applications,
                    qualifications,
                    machine,
                    1,
                    next_place,
                )
                .with_structural_types(&self.structural_types)
                .with_produced_values(&self.produced_values)
                .with_completed_values(self.completed_operand_scope.iter().copied())
                .with_literals(&self.literals)
                .with_subslices(&self.subslices)
                .with_observations(&self.observations)
                .with_source_carriers(self.completed_carrier_inventory(operations)?)
                .with_cases(&self.cases)
                .with_fields(&self.record_fields)
                .enter_proof_scope(identities, &self.erased_proof_formals))
            }

            #[track_caller]
            pub(crate) fn value_storage(
                &self,
                source: &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
                parameters: &[StructuralParameterDeclaration],
                mut result: impl FnMut(u32) -> Result<StructuralValueStorage, LoweringError>,
            ) -> Result<StructuralValueStorage, LoweringError> {
                use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
                let storage = match *source {
                    CheckedUnitStructuralArgumentSourcePlan::CapturedStorage {
                        selection,
                        source_ordinal,
                    } => {
                        let mut captures = self
                            .selection_cleanups
                            .iter()
                            .filter(|cleanup| cleanup.selection == selection)
                            .flat_map(|cleanup| &cleanup.sources)
                            .filter(|source| source.source_ordinal == source_ordinal);
                        let source = captures.next().ok_or(LoweringError::Unsupported(
                            "selected cleanup lost its captured source home",
                        ))?;
                        if captures.next().is_some() {
                            return unsupported(
                                "selected cleanup repeats its captured source home",
                            );
                        }
                        source.storage.clone()
                    }
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                        let parameter = self
                            .structural_parameters
                            .get(parameter_index as usize)
                            .map(|(_, parameter)| parameter)
                            .or_else(|| parameters.get(parameter_index as usize))
                            .ok_or(LoweringError::Unsupported(
                                "value storage parameter is absent",
                            ))?;
                        StructuralValueStorage {
                            place: parameter.place,
                            path: Vec::new(),
                            selected_type: parameter.structural_type,
                        }
                    }
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
                    | CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. } => {
                        let root = match *source {
                            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                                facts::PlaceRoot::Symbol(symbol)
                            }
                            CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                                expression,
                                ..
                            } => facts::PlaceRoot::Expression(expression),
                            _ => {
                                return unsupported("value storage source has no storage identity");
                            }
                        };
                        let local = structural_local_by_root(&self.structural_locals, root)
                            .ok_or_else(|| self.missing_value_storage_local(source, root))?;
                        StructuralValueStorage {
                            place: local.argument.place,
                            path: local.argument.path.clone(),
                            selected_type: local.structural_type,
                        }
                    }
                    CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => result(binding_ordinal)?,
                    _ => {
                        return unsupported("value storage source has no owned structural storage");
                    }
                };
                Ok(storage)
            }

            fn missing_value_storage_local(
                &self,
                source: &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
                root: facts::PlaceRoot,
            ) -> LoweringError {
                let mut retired_binding_count = 0;
                let mut retired_bindings = Vec::new();
                for binding in self
                    .retired_structural_bindings
                    .iter()
                    .filter(|binding| binding.root == root)
                {
                    retired_binding_count += 1;
                    if retired_bindings.len() < 4 {
                        retired_bindings.push((
                            binding.place,
                            binding.statement,
                            match binding.end {
                                StructuralBindingEnd::ConsumedSource => "consumed source",
                                StructuralBindingEnd::RetiredHome => "retired home",
                            },
                        ));
                    }
                }
                LoweringError::MissingValueStorageLocal {
                    machine: self.machine,
                    source: source.clone(),
                    current_local_count: self.structural_locals.len(),
                    current_locals: self
                        .structural_locals
                        .iter()
                        .take(8)
                        .map(|local| (local.root, local.argument.place))
                        .collect(),
                    completed_cleanup_home: structural_local_by_root(
                        &self.completed_cleanup_homes,
                        root,
                    )
                    .map(|local| local.argument.place),
                    retired_binding_count,
                    retired_bindings,
                }
            }

            #[track_caller]
            pub(crate) fn cleanup_value_storage(
                &self,
                source: &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
                parameters: &[StructuralParameterDeclaration],
                result: impl FnMut(u32) -> Result<StructuralValueStorage, LoweringError>,
            ) -> Result<StructuralValueStorage, LoweringError> {
                if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                expression,
                ..
            } = *source
            {
                let root = facts::PlaceRoot::Expression(expression);
                if structural_local_by_root(&self.structural_locals, root).is_none()
                    && let Some(home) =
                        structural_local_by_root(&self.completed_cleanup_homes, root)
                {
                    return Ok(StructuralValueStorage {
                        place: home.argument.place,
                        path: home.argument.path.clone(),
                        selected_type: home.structural_type,
                    });
                }
            }
                self.value_storage(source, parameters, result)
            }

            pub(crate) fn cleanup_source_is_retired(
                &self,
                source: &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
                parameters: &[StructuralParameterDeclaration],
                result: impl FnMut(u32) -> Result<StructuralValueStorage, LoweringError>,
            ) -> Result<bool, LoweringError> {
                use checked_trees::CheckedUnitStructuralArgumentSourcePlan as Source;
                let root = match *source {
                    Source::StructuralLocal { symbol } => Some(facts::PlaceRoot::Symbol(symbol)),
                    Source::StructuralExpression { expression, .. } => {
                        Some(facts::PlaceRoot::Expression(expression))
                    }
                    _ => None,
                };
                if let Some(root) = root
                    && structural_local_by_root(&self.structural_locals, root).is_none()
                {
                    let mut ended = self
                        .retired_structural_bindings
                        .iter()
                        .filter(|binding| binding.root == root);
                    if let Some(binding) = ended.next() {
                        if ended.next().is_some() {
                            return Ok(false);
                        }
                        return Ok(binding.end == StructuralBindingEnd::RetiredHome
                            && self.retired_structural_homes.is_retired(binding.place));
                    }
                }
                let storage = self.cleanup_value_storage(source, parameters, result)?;
                Ok(self.retired_structural_homes.is_retired(storage.place))
            }

            fn retain_selection_sources(
                &mut self,
                completed: &[SelectionCleanup],
            ) -> Result<(), LoweringError> {
                for incoming in completed {
                    if let Some(current) = self
                        .selection_cleanups
                        .iter_mut()
                        .find(|current| current.selection == incoming.selection)
                    {
                        if current.sources.len() != incoming.sources.len()
                            || current.sources.iter().any(|source| {
                                !incoming.sources.iter().any(|other| {
                                    other.source_ordinal == source.source_ordinal
                                        && other.storage == source.storage
                                })
                            })
                        {
                            return unsupported(
                                "selected cleanup receipt has different completed source homes",
                            );
                        }
                        if current.retired_reference_paths != incoming.retired_reference_paths {
                            return unsupported(
                                "private scalar completion changed selected descriptor retirement",
                            );
                        }
                    } else {
                        if !incoming.retired_reference_paths.is_empty() {
                            return unsupported(
                                "private scalar completion published selected descriptor retirement",
                            );
                        }
                        self.selection_cleanups.push(incoming.clone());
                    }
                }
                Ok(())
            }

            pub(crate) fn structural_root_parameters(
                &self,
            ) -> impl Iterator<Item = &StructuralParameterDeclaration> + Clone {
                self.structural_parameters
                    .iter()
                    .map(|(_, declaration)| declaration)
                    .chain(self.previous_parameter_homes.iter())
                    .chain(self.block_structural_parameters.iter())
                    .chain(
                        self.blocks
                            .iter()
                            .flat_map(|block| block.structural_parameters.iter()),
                    )
            }

            pub(crate) fn structural_arrivals(
                &self,
            ) -> impl Iterator<Item = (BlockId, &StructuralParameterDeclaration)> {
                self.enclosing_structural_arrivals
                    .iter()
                    .map(|(block, parameter)| (*block, parameter))
                    .chain(
                        self.block_structural_parameters
                            .iter()
                            .map(|parameter| (self.current, parameter)),
                    )
                    .chain(self.blocks.iter().flat_map(|block| {
                        block
                            .structural_parameters
                            .iter()
                            .map(move |parameter| (block.id, parameter))
                    }))
            }

            pub(crate) fn current_parameters<'parameters>(
                &self,
                declared: &'parameters [StructuralParameterDeclaration],
            ) -> std::borrow::Cow<'parameters, [StructuralParameterDeclaration]> {
                if self.previous_parameter_homes.is_empty() {
                    std::borrow::Cow::Borrowed(declared)
                } else {
                    std::borrow::Cow::Owned(
                        self.structural_parameters
                            .iter()
                            .map(|(_, parameter)| parameter.clone())
                            .collect(),
                    )
                }
            }

            pub(crate) fn bind_structural_parameters(
                &mut self,
                parameters: Vec<(u32, StructuralParameterDeclaration)>,
                structural_types: &[StructuralTypeDeclaration],
            ) {
                self.structural_types = structural_types.into();
                self.structural_fields =
                    StructuralScalarFieldBinding::collect(&parameters, structural_types);
                self.structural_cases =
                    StructuralCaseBinding::collect(&parameters, structural_types);
                self.element_views = crate::expression_preparation::bindings::element_views(
                    &parameters,
                    structural_types,
                );
                self.structural_parameters = parameters;
            }

            pub(crate) fn branch(&self, block: BlockId, operation_start: usize) -> Self {
                Self {
                    scalar_applications: std::sync::Arc::clone(&self.scalar_applications),
                    current_claims: self.current_claims.clone(),
                    shared_qualifications: self.shared_qualifications.clone(),
                    machine: self.machine,
                    qualification_coercions: self.qualification_coercions.clone(),
                    structural_types: self.structural_types.clone(),
                    structural_value_owners: self.structural_value_owners.clone(),
                    selection_cleanups: self.selection_cleanups.clone(),
                    structural_locals: self.structural_locals.clone(),
                    completed_operand_scope: self.completed_operand_scope.clone(),
                    completed_cleanup_homes: self.completed_cleanup_homes.clone(),
                    retired_structural_bindings: self.retired_structural_bindings.clone(),
                    retired_structural_homes: self.retired_structural_homes.clone(),
                    view_locals: self.view_locals.clone(),
                    element_views: self.element_views.clone(),
                    produced_values: self.produced_values.clone(),
                    literals: self.literals.clone(),
                    subslices: self.subslices.clone(),
                    observations: self.observations.clone(),
                    cases: self.cases.clone(),
                    record_fields: self.record_fields.clone(),
                    place_observations: self.place_observations.clone(),
                    computation_address_roots: self.computation_address_roots.clone(),
                    primitive_storage: self.primitive_storage.clone(),
                    parameter_storage: self.parameter_storage.clone(),
                    scalar_bindings: self.scalar_bindings.clone(),
                    structural_parameters: self.structural_parameters.clone(),
                    previous_parameter_homes: self
                        .previous_parameter_homes
                        .iter()
                        .chain(self.block_structural_parameters.iter())
                        .chain(
                            self.blocks
                                .iter()
                                .flat_map(|block| block.structural_parameters.iter()),
                        )
                        .cloned()
                        .collect(),
                    enclosing_structural_arrivals: self
                        .structural_arrivals()
                        .map(|(block, parameter)| (block, parameter.clone()))
                        .collect(),
                    structural_fields: self.structural_fields.clone(),
                    structural_cases: self.structural_cases.clone(),
                    erased_scalar_formals: self.erased_scalar_formals.clone(),
                    erased_proof_formals: self.erased_proof_formals.clone(),
                    completion_proof_formals: Vec::new(),
                    entry: self.entry,
                    current: block,
                    parameters: Vec::new(),
                    block_structural_parameters: Vec::new(),
                    operation_start,
                    blocks: Vec::new(),
                }
            }

            fn record_owned_home(
                &mut self,
                owner: StructuralValueOwner,
            ) -> Result<(), LoweringError> {
                if let Some(existing) = self
                    .structural_value_owners
                    .iter_mut()
                    .find(|existing| existing.value.place == owner.value.place)
                {
                    if existing.symbol.is_valid()
                        && owner.symbol.is_valid()
                        && existing.symbol != owner.symbol
                    {
                        return unsupported("affine home is owned by two locals");
                    }
                    if !existing.symbol.is_valid() && owner.symbol.is_valid() {
                        existing.symbol = owner.symbol;
                        existing.statement = owner.statement;
                    }
                    return Ok(());
                }
                self.structural_value_owners.push(owner);
                Ok(())
            }

            pub(crate) fn establish_structural_result(
                &mut self,
                state: symbols::SymbolHandle,
                result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
                produced: terminal_psi::StructuralOperationResult,
                structural_types: &[StructuralTypeDeclaration],
                operations: &mut OperationBuffer,
            ) -> Result<(), LoweringError> {
                let multiplicity = terminal_multiplicity(result.multiplicity);
                if !structural_types
                    .iter()
                    .any(|declaration| declaration.id == produced.structural_type)
                {
                    return unsupported("structural result type was not declared");
                }
                if multiplicity == StructuralMultiplicity::Linear {
                    let mut producers = operations.iter().filter(|operation| {
                        matches!(&operation.result,
                        OperationResult::Structural(value) if value == &produced)
                    });
                    let producer = producers.next().ok_or(LoweringError::Unsupported(
                        "linear local has no exact completed call result",
                    ))?;
                    if producers.next().is_some()
                        || produced.claims.is_empty()
                        || !operations.source_calls.iter().any(|occurrence| {
                            occurrence.origin.statement().is_some_and(|site| {
                                site.state == state
                                    && site.statement_index == result.statement_index as usize
                            }) && occurrence.terminal_site.operation() == Some(producer.id)
                        })
                    {
                        return unsupported("linear local lost its exact call result custody");
                    }
                }
                self.current_claims
                    .publish(produced.place, produced.claims.clone())?;
                self.establish_structural_binding(
                    result,
                    produced.into(),
                    structural_types,
                    operations,
                )
            }

            pub(crate) fn establish_structural_binding(
                &mut self,
                result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
                produced: StructuralBinding,
                structural_types: &[StructuralTypeDeclaration],
                operations: &mut OperationBuffer,
            ) -> Result<(), LoweringError> {
                let multiplicity = produced.multiplicity();
                let declaration = terminal_psi::find_structural_type(
                    structural_types,
                    produced.structural_type(),
                )
                .ok_or(LoweringError::Unsupported(
                    "structural binding type was not declared",
                ))?;
                if operations
                    .structural_values
                    .iter()
                    .any(|(ordinal, _)| *ordinal == result.binding_ordinal)
                {
                    return unsupported("structural value binding was established twice");
                }
                let local = result.local;
                if local.is_named() {
                    if self
                        .structural_locals
                        .iter()
                        .any(|binding| binding.root == facts::PlaceRoot::Symbol(local.symbol))
                    {
                        return unsupported("structural value repeats its local binding");
                    }
                    let access = terminal_access(local.access);
                    let reference_carrier =
                        matches!(declaration.shape, StructuralTypeShape::Reference { .. });
                    self.structural_fields
                        .adopt_declarations(produced.structural_type(), structural_types);
                    self.bind_structural_local(StructuralLocalBinding {
                        result_binding_ordinal: Some(result.binding_ordinal),
                        root: facts::PlaceRoot::Symbol(local.symbol),
                        structural_type: produced.structural_type(),
                        argument:
                            crate::expression_preparation::bindings::structural_paths::structural_argument(
                                produced.place(),
                                produced.path().to_vec(),
                                if reference_carrier {
                                    StructuralAccess::Owned
                                } else {
                                    access
                                },
                            ),
                    });
                    if produced.path().is_empty()
                        && let Some(carrier) =
                            ViewCarrier::of(produced.structural_type(), structural_types)
                    {
                        self.view_locals.push(ViewLocalBinding {
                            symbol: local.symbol,
                            place: produced.place(),
                            structural_type: produced.structural_type(),
                            carrier,
                            access,
                        });
                    }
                }
                if matches!(
                    multiplicity,
                    StructuralMultiplicity::Affine | StructuralMultiplicity::Linear
                ) && produced.path().is_empty()
                {
                    self.record_owned_home(StructuralValueOwner {
                        symbol: local.symbol,
                        statement: result.statement_index,
                        value: produced.operation_result()?.clone(),
                    })?;
                }
                operations.bind_structural_value(result.binding_ordinal, produced);
                Ok(())
            }

            pub(crate) fn bind_structural_local(&mut self, binding: StructuralLocalBinding) {
                self.retired_structural_bindings
                    .retain(|retired| retired.root != binding.root);
                self.retired_structural_homes
                    .reestablish(binding.argument.place);
                self.structural_locals.push(binding);
            }

            pub(crate) fn structural_source_frontier(&self) -> StructuralSourceFrontier {
                StructuralSourceFrontier {
                    owners: self.structural_value_owners.clone(),
                    locals: self.structural_locals.clone(),
                    retired_bindings: self.retired_structural_bindings.clone(),
                    retired_homes: self.retired_structural_homes.clone(),
                    views: self.view_locals.clone(),
                    parameters: self.structural_parameters.clone(),
                    previous_parameters: self.previous_parameter_homes.clone(),
                    enclosing_arrivals: self.enclosing_structural_arrivals.clone(),
                    fields: self.structural_fields.clone(),
                    cases: self.structural_cases.clone(),
                }
            }

            pub(crate) fn restore_structural_sources(
                &mut self,
                frontier: StructuralSourceFrontier,
            ) -> Result<(), LoweringError> {
                for home in &self.structural_locals {
                    if matches!(home.root, facts::PlaceRoot::Expression(_)) {
                        crate::expression_preparation::bindings::retain_completed_structural_homes(
                            &mut self.completed_cleanup_homes,
                            std::slice::from_ref(home),
                        )?;
                    }
                }
                self.structural_value_owners = frontier.owners;
                self.structural_locals = frontier.locals;
                self.retired_structural_bindings = frontier.retired_bindings;
                self.retired_structural_homes = frontier.retired_homes;
                self.view_locals = frontier.views;
                self.structural_parameters = frontier.parameters;
                self.previous_parameter_homes = frontier.previous_parameters;
                self.enclosing_structural_arrivals = frontier.enclosing_arrivals;
                self.structural_fields = frontier.fields;
                self.structural_cases = frontier.cases;
                Ok(())
            }

            pub(crate) fn retire_consumed_structural_arguments<'argument>(
                &mut self,
                arguments: impl IntoIterator<Item = &'argument terminal_psi::StructuralArgument>,
                operations: &OperationBuffer,
            ) -> Result<(), LoweringError> {
                for argument in arguments {
                    if argument.access == StructuralAccess::Owned
                        && argument.path.is_empty()
                        && self.carrier_multiplicity(argument.place, operations)?
                            != StructuralMultiplicity::Unrestricted
                    {
                        self.retire_structural_sources(argument.place);
                    }
                }
                Ok(())
            }

            pub(crate) fn retire_structural_operation_operands(
                &mut self,
                operation: semantic_vocabulary::OperationId,
                operations: &OperationBuffer,
            ) -> Result<(), LoweringError> {
                let operation = operations
                    .iter()
                    .find(|emitted| emitted.id == operation)
                    .ok_or(LoweringError::Unsupported(
                        "structural source transfer lost its emitted operation",
                    ))?;
                self.retire_consumed_structural_arguments(
                    structural_operands(&operation.kind),
                    operations,
                )
            }

            #[track_caller]
            pub(crate) fn retire_structural_home(&mut self, place: PlaceId) {
                if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                    eprintln!(
                        "CHECKSITE structural-home-retired place={place:?} caller={} local_roots={:?}",
                        std::panic::Location::caller(),
                        self.structural_locals
                            .iter()
                            .filter(|local| local.argument.place == place)
                            .take(16)
                            .map(|local| local.root)
                            .collect::<Vec<_>>(),
                    );
                }
                self.retire_structural_sources(place);
                self.retired_structural_homes
                    .record(place, StructuralBindingEnd::RetiredHome);
                for binding in self
                    .retired_structural_bindings
                    .iter_mut()
                    .filter(|binding| binding.place == place)
                {
                    binding.end = StructuralBindingEnd::RetiredHome;
                }
                self.structural_locals
                    .retain(|local| local.argument.place != place);
            }

            fn retire_structural_sources(&mut self, place: PlaceId) {
                self.retired_structural_homes
                    .record(place, StructuralBindingEnd::ConsumedSource);
                for local in self
                    .structural_locals
                    .iter()
                    .filter(|local| local.argument.place == place)
                {
                    if !self
                        .retired_structural_bindings
                        .iter()
                        .any(|binding| binding.root == local.root && binding.place == place)
                    {
                        let statement = self
                            .structural_value_owners
                            .iter()
                            .find(|owner| {
                                Some(owner.symbol) == local.root.as_symbol()
                                    && owner.value.place == place
                            })
                            .map(|owner| owner.statement);
                        self.retired_structural_bindings
                            .push(RetiredStructuralBinding {
                                root: local.root,
                                place,
                                statement,
                                end: StructuralBindingEnd::ConsumedSource,
                            });
                    }
                }
                self.structural_value_owners
                    .retain(|owner| owner.value.place != place);
                self.structural_locals.retain(|local| {
                    local.argument.place != place || local.root.as_symbol().is_none()
                });
                self.view_locals.retain(|view| view.place != place);
            }

            pub(crate) fn continue_values(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &OperationBuffer,
            ) -> Result<semantic_vocabulary::EdgeId, LoweringError> {
                self.cleanup_emission(checked, &mut Vec::new())
                    .continue_actions(
                        identities,
                        cleanup_actions,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                    )
            }

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn require_value_refinements(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                machine: semantic_vocabulary::MachineId,
                predicates: Vec<checked_trees::CheckedBooleanExpression>,
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &mut OperationBuffer,
                calls: &mut CallEmissionContext<'_>,
            ) -> Result<(), LoweringError> {
                if predicates.is_empty() {
                    return Ok(());
                }
                let edge = self.continue_values(
                    identities,
                    checked,
                    Vec::new(),
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                )?;
                let clauses = predicates
                    .into_iter()
                    .map(|predicate| {
                        Some(checked_trees::ClosedScalarContractValue::Predicate(
                            predicate,
                        ))
                    })
                    .collect::<Vec<_>>();
                let predicate = crate::scalar_lowering::contracts::clauses(
                    identities,
                    &clauses,
                    std::slice::from_ref(values.last().ok_or(LoweringError::Unsupported(
                        "local refinement lost its completed subject",
                    ))?),
                    &self.erased_scalar_formals,
                    &crate::scalar_lowering::contracts::ContractViewNamespace {
                        scalar_applications: &self.scalar_applications,
                        constant_values: &checked.typed.constant_recipe_closure.values,
                        ..crate::scalar_lowering::contracts::ContractViewNamespace::EMPTY
                    },
                )?
                .ok_or(LoweringError::Unsupported(
                    "local refinement lost its predicate",
                ))?;
                operations
                    .scalar_block_invariants
                    .push(terminal_psi::ScalarBlockInvariant {
                        machine,
                        header: self.current,
                        predicate,
                        arrivals: vec![terminal_psi::ScalarBlockInvariantArrival {
                            edge,
                            obligation: calls.allocate_requirement()?,
                        }],
                    });
                Ok(())
            }

            pub(crate) fn new(
                next_block: &mut u64,
                scalar_applications: std::sync::Arc<
                    super::scalar_applications::SourceScalarApplications,
                >,
            ) -> Result<Self, LoweringError> {
                let entry = block_id(allocate_dense(next_block)?);
                Ok(Self {
                    scalar_applications,
                    current_claims: Default::default(),
                    shared_qualifications: PreparedScalarQualifications::empty(),
                    machine: None,
                    qualification_coercions: Vec::new(),
                    structural_types: std::sync::Arc::from([]),
                    structural_value_owners: Vec::new(),
                    selection_cleanups: Vec::new(),
                    structural_locals: Vec::new(),
                    completed_operand_scope: Vec::new(),
                    completed_cleanup_homes: Vec::new(),
                    retired_structural_bindings: Vec::new(),
                    retired_structural_homes: StructuralHomeEndings::default(),
                    view_locals: Vec::new(),
                    element_views: std::collections::BTreeMap::new(),
                    produced_values: Vec::new(),
                    literals: Vec::new(),
                    subslices: Vec::new(),
                    observations: Vec::new(),
                    cases: Vec::new(),
                    record_fields: Vec::new(),
                    place_observations: std::sync::Arc::from([]),
                    computation_address_roots: Vec::new(),
                    primitive_storage: Vec::new(),
                    parameter_storage: Vec::new(),
                    scalar_bindings: ScalarBindings::new(std::iter::empty()),
                    structural_fields: Default::default(),
                    structural_cases: Vec::new(),
                    structural_parameters: Vec::new(),
                    previous_parameter_homes: Vec::new(),
                    enclosing_structural_arrivals: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    completion_proof_formals: Vec::new(),
                    entry,
                    current: entry,
                    parameters: Vec::new(),
                    block_structural_parameters: Vec::new(),
                    operation_start: 0,
                    blocks: Vec::new(),
                })
            }

            fn proof_formal_forwarding(&self) -> Vec<LoweredProofTerm> {
                (0..self.erased_proof_formals.len())
                    .map(|position| LoweredProofTerm::Formal {
                        position: u32::try_from(position)
                            .expect("erased-proof roster positions fit u32"),
                    })
                    .collect()
            }

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn arguments(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                machine: symbols::SymbolHandle,
                state: symbols::SymbolHandle,
                operation: &CheckedUnitEffectOperationPlan,
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &mut OperationBuffer,
                calls: &mut CallEmissionContext<'_>,
                environment: &mut OperationEnvironment<'_>,
            ) -> Result<Option<Vec<ValueDeclaration>>, LoweringError> {
                self.arguments_slice(
                    identities,
                    checked,
                    machine,
                    state,
                    operation,
                    None,
                    values.len(),
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                    calls,
                    environment,
                )
            }

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn call_selector(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                machine: symbols::SymbolHandle,
                state: symbols::SymbolHandle,
                operation: &CheckedUnitEffectOperationPlan,
                argument: usize,
                segment: usize,
                source_value_count: usize,
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &mut OperationBuffer,
                calls: &mut CallEmissionContext<'_>,
                environment: &mut OperationEnvironment<'_>,
            ) -> Result<ValueDeclaration, LoweringError> {
                let coordinate = operation
                    .invocation_coordinate()
                    .ok_or(LoweringError::Unsupported("selector has no invocation"))?;
                let argument = operation.structural_operands().get(argument).ok_or(
                    LoweringError::Unsupported("selector has no structural actual"),
                )?;
                let path =
                    argument
                        .call_projection()
                        .get(segment)
                        .ok_or(LoweringError::Unsupported(
                            "selector has no argument path occurrence",
                        ))?;
                let computation = argument
                    .selector_computation(
                        segment,
                        &checked.facts.values.scalar_computations.operands,
                    )
                    .or(match path {
                        checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(root) => {
                            Some(*root)
                        }
                        _ => None,
                    });
                let value = match (computation.as_ref(), path) {
                    (Some(root), _) => {
                        let source = value_facts(checked)
                            .scalar_computations
                            .roots
                            .iter()
                            .map(|(_, source)| source)
                            .find(|source| {
                                source.root == *root
                                    && source.state == state
                                    && source.statement_ordinal == coordinate.statement_index
                            })
                            .ok_or(LoweringError::Unsupported(
                                "call selector lost its source computation",
                            ))?;
                        self.source_value(
                            identities,
                            checked,
                            machine,
                            state,
                            source.statement_ordinal,
                            source.role,
                            &CheckedCallScalarArgument::Computation(*root),
                            source_value_count,
                            values,
                            next_value,
                            next_block,
                            next_edge,
                            operations,
                            calls,
                            environment,
                        )?
                    }
                    (
                        None,
                        checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                            index @ (checked_trees::CheckedRuntimeIndex::StructuralField(_)
                            | checked_trees::CheckedRuntimeIndex::StorageRead { .. }),
                        ),
                    ) => {
                        let bindings = self.contextual_bindings();
                        let expression = match index {
                            checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                                bindings.runtime_index_field(identities, field)?
                            }
                            checked_trees::CheckedRuntimeIndex::StorageRead { symbol } => {
                                bindings.runtime_index_storage(*symbol)?
                            }
                            _ => return unsupported("scheduled storage selector lost its source"),
                        };
                        let source_types = scalar_source_types(values);
                        let carrier = expression.value_type(&source_types)?;
                        ValueDeclaration {
                            id: emit_direct_expression(&expression, values, next_value, operations),
                            scalar_type: carrier.scalar_type,
                            qualifications: carrier.qualifications,
                        }
                    }
                    _ => return unsupported("scheduled selector has no evaluated operand"),
                };
                if !matches!(value.scalar_type, ScalarType::Integer(integer) if !integer.is_address())
                {
                    return unsupported("call selector is not an integer operand");
                }
                Ok(value)
            }

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn argument_at(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                machine: symbols::SymbolHandle,
                state: symbols::SymbolHandle,
                operation: &CheckedUnitEffectOperationPlan,
                argument_ordinal: usize,
                source_value_count: usize,
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &mut OperationBuffer,
                calls: &mut CallEmissionContext<'_>,
                environment: &mut OperationEnvironment<'_>,
            ) -> Result<ValueDeclaration, LoweringError> {
                let arguments = self.arguments_slice(
                    identities,
                    checked,
                    machine,
                    state,
                    operation,
                    Some(argument_ordinal),
                    source_value_count,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                    calls,
                    environment,
                )?;
                match arguments.as_deref() {
                    Some([argument]) => Ok(*argument),
                    _ => unsupported("call has no single completed scalar operand"),
                }
            }

            #[allow(clippy::too_many_arguments)]
            fn arguments_slice(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                machine: symbols::SymbolHandle,
                state: symbols::SymbolHandle,
                operation: &CheckedUnitEffectOperationPlan,
                argument_ordinal: Option<usize>,
                source_value_count: usize,
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &mut OperationBuffer,
                calls: &mut CallEmissionContext<'_>,
                environment: &mut OperationEnvironment<'_>,
            ) -> Result<Option<Vec<ValueDeclaration>>, LoweringError> {
                if source_value_count > values.len() {
                    return unsupported("call argument source prefix exceeds its retained values");
                }
                let source_bindings = self.contextual_bindings();
                let Some(call) = operation.call() else {
                    return Ok(None);
                };
                let operands = call
                    .scalar_operands()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                let (coordinate, arguments) = (call.coordinate, operands.as_slice());
                let (arguments, argument_ordinal_start) = match argument_ordinal {
                    Some(ordinal) => {
                        let argument = arguments.get(ordinal).ok_or(LoweringError::Unsupported(
                            "call scalar operand ordinal is outside its argument list",
                        ))?;
                        (std::slice::from_ref(argument), ordinal)
                    }
                    None => (arguments, 0),
                };
                let qualifications = &self.shared_qualifications;
                let source_types = scalar_source_types(values);
                let argument_types = arguments
                    .iter()
                    .map(|argument| match argument {
                        CheckedCallScalarArgument::Pure(expression) => source_bindings
                            .expression(
                                identities,
                                &checked.facts.proven_scalar_indexes,
                                expression,
                            )?
                            .value_type(&source_types),
                        CheckedCallScalarArgument::Computation(root) => {
                            let nodes = &checked.facts.values.scalar_computations.nodes;
                            if !nodes.is_valid(*root) {
                                return unsupported("call argument computation has no live root");
                            }
                            crate::scalar_lowering::computations::computation_value_type(
                                identities,
                                checked,
                                qualifications,
                                *root,
                                &source_bindings,
                                &source_types,
                            )
                        }
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                let formals = self.callee_scalar_formals(
                    checked,
                    call,
                    argument_ordinal_start,
                    arguments.len(),
                );
                let arrival_types = argument_types
                    .iter()
                    .enumerate()
                    .map(|(index, actual)| {
                        received_scalar_type(
                            formals
                                .as_deref()
                                .and_then(|types| types.get(index))
                                .copied(),
                            *actual,
                        )
                    })
                    .collect::<Vec<_>>();
                let needs_control = arrival_types != argument_types
                    || arguments.iter().any(|argument| match argument {
                        CheckedCallScalarArgument::Computation(_) => true,
                        CheckedCallScalarArgument::Pure(expression) => source_bindings
                            .expression(
                                identities,
                                &checked.facts.proven_scalar_indexes,
                                expression,
                            )
                            .is_ok_and(|expression| {
                                direct_expression_contains_short_circuit(&expression)
                            }),
                    });
                if !needs_control {
                    return arguments
                        .iter()
                        .zip(&argument_types)
                        .map(|(argument, scalar_type)| {
                            let expression = source_bindings.expression(
                                identities,
                                &checked.facts.proven_scalar_indexes,
                                argument.as_pure().ok_or(LoweringError::Unsupported(
                                    "computed call argument requires ordered control",
                                ))?,
                            )?;
                            validate_direct_parameter_types(
                                &expression,
                                &scalar_carriers(&source_types),
                            )?;
                            Ok(ValueDeclaration {
                                qualifications: scalar_type.qualifications,
                                id: emit_direct_expression(
                                    &expression,
                                    values,
                                    next_value,
                                    operations,
                                ),
                                scalar_type: scalar_type.scalar_type,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()
                        .map(Some);
                }

                let mut expansion = self.scalar_computation_expansion(
                    identities,
                    checked,
                    qualifications,
                    machine,
                    operations,
                    environment.next_place,
                )?;
                let entry_index = expansion.call_arguments(
                    identities,
                    state,
                    coordinate,
                    arguments,
                    argument_ordinal_start,
                    &source_bindings,
                    &source_types,
                    formals.as_deref(),
                    0,
                )?;
                let states = expansion.finish();
                self.complete_expansion(
                    identities,
                    checked,
                    &states,
                    entry_index,
                    &arrival_types,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                    calls,
                    environment,
                )
                .map(Some)
            }

            fn callee_scalar_formals(
                &self,
                checked: &CheckedTrees,
                call: &checked_trees::CheckedUnitCallPlan,
                first: usize,
                count: usize,
            ) -> Option<Vec<QualifiedScalarType>> {
                let formals = match call.route() {
                    checked_trees::CheckedUnitCallRoute::Module { .. } => {
                        self.shared_qualifications
                            .scalar_state_types(checked, call.target.state)
                            .ok()?
                            .0
                    }
                    checked_trees::CheckedUnitCallRoute::Boundary => self
                        .shared_qualifications
                        .boundary_scalar_formals(checked, call.target.machine)?,
                    _ => return None,
                };
                formals
                    .get(first..first.checked_add(count)?)
                    .map(<[QualifiedScalarType]>::to_vec)
            }

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn field_assignment_value(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                machine: symbols::SymbolHandle,
                state: symbols::SymbolHandle,
                statement_index: u32,
                value: &checked_trees::CheckedScalarStoreValue,
                primitive_type: typed_trees::types::PrimitiveType,
                field_identity: Option<&checked_trees::CheckedMemberIdentity>,
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &mut OperationBuffer,
                calls: &mut CallEmissionContext<'_>,
                environment: &mut OperationEnvironment<'_>,
            ) -> Result<ValueDeclaration, LoweringError> {
                let bindings = self.contextual_bindings();
                let qualifications = &self.shared_qualifications;
                let source_types = scalar_source_types(values);
                let scalar_type = terminal_scalar_type(primitive_type)?;
                let computation =
                    crate::emission::structural_scalar_store_source::computation_root(
                        checked,
                        machine,
                        state,
                        statement_index,
                        value,
                        field_identity,
                    )?;
                if let checked_trees::CheckedScalarStoreValue::ScalarResult { position } = value {
                    let position = usize::try_from(*position).map_err(|_| {
                        LoweringError::Unsupported("field store call-result position exceeds usize")
                    })?;
                    if position.checked_add(1) != Some(values.len()) {
                        return unsupported(
                            "field store call result is not this statement's established scalar result",
                        );
                    }
                    let value = values[position];
                    return Ok(value);
                }
                if value.as_pure().is_some() {
                    let expression = bindings.expression_at(
                        identities,
                        checked,
                        state,
                        statement_index,
                        CheckedScalarExpressionRole::AssignmentValue,
                    )?;
                    if expression.scalar_type() != scalar_type {
                        return unsupported("field store requires a matching pure value");
                    }
                    validate_direct_parameter_types(&expression, &scalar_carriers(&source_types))?;
                    if !direct_expression_contains_short_circuit(&expression) {
                        return Ok(ValueDeclaration {
                            qualifications: Default::default(),
                            id: emit_direct_expression(&expression, values, next_value, operations),
                            scalar_type,
                        });
                    }
                    let mut expansion = crate::scalar_lowering::computations::Expansion::new(
                        checked,
                        &self.scalar_applications,
                        qualifications,
                        machine,
                        1,
                        environment.next_place,
                    )
                    .with_structural_types(&self.structural_types)
                    .enter_proof_scope(identities, &self.erased_proof_formals);
                    let entry = expansion.direct_value(&source_types, 0, expression)?;
                    let states = expansion.finish();
                    return self.single_completed_expansion(
                        identities,
                        checked,
                        &states,
                        entry,
                        scalar_type.into(),
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        calls,
                        "field assignment has no single completed RHS",
                        environment,
                    );
                }
                let (root, role) = computation.ok_or(LoweringError::Unsupported(
                    "field assignment lost its exact scalar computation root",
                ))?;
                let result_type = crate::scalar_lowering::computations::computation_value_type(
                    identities,
                    checked,
                    qualifications,
                    root,
                    &bindings,
                    &source_types,
                )?;
                let mut expansion = self.scalar_computation_expansion(
                    identities,
                    checked,
                    qualifications,
                    machine,
                    operations,
                    environment.next_place,
                )?;
                let entry = expansion.retained_value(
                    identities,
                    state,
                    statement_index,
                    role,
                    &bindings,
                    &source_types,
                    result_type,
                    0,
                )?;
                let states = expansion.finish();
                self.single_completed_expansion(
                    identities,
                    checked,
                    &states,
                    entry,
                    result_type,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                    calls,
                    "field assignment has no single completed RHS",
                    environment,
                )
            }

            #[allow(clippy::too_many_arguments)]
            fn single_completed_expansion(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                expansion: &crate::scalar_lowering::computations::PreparedExpansion,
                entry: usize,
                result_type: QualifiedScalarType,
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &mut OperationBuffer,
                calls: &mut CallEmissionContext<'_>,
                message: &'static str,
                environment: &mut OperationEnvironment<'_>,
            ) -> Result<ValueDeclaration, LoweringError> {
                let result = self.complete_expansion(
                    identities,
                    checked,
                    expansion,
                    entry,
                    &[result_type],
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                    calls,
                    environment,
                )?;
                match result.as_slice() {
                    [value] => Ok(*value),
                    _ => unsupported(message),
                }
            }

            #[allow(clippy::too_many_arguments)]
            fn complete_expansion(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                checked: &CheckedTrees,
                expansion: &crate::scalar_lowering::computations::PreparedExpansion,
                entry_index: usize,
                result_types: &[QualifiedScalarType],
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &mut OperationBuffer,
                calls: &mut CallEmissionContext<'_>,
                environment: &mut OperationEnvironment<'_>,
            ) -> Result<Vec<ValueDeclaration>, LoweringError> {
                let states = &expansion.states;
                for plan in &expansion.claim_edges {
                    for destination in &plan.destinations {
                        calls.claims.reserve_checked_binding(*destination)?;
                    }
                }
                if expansion.claim_edges.is_empty()
                    && let Some(completed) = self.complete_straight_line(
                        identities,
                        checked,
                        states,
                        entry_index,
                        result_types,
                        values,
                        next_value,
                        operations,
                        calls,
                        environment,
                    )?
                {
                    return Ok(completed);
                }
                let source_types = scalar_source_types(values);
                let mut completion_types = source_types.clone();
                completion_types.extend_from_slice(result_types);
                let completion_parameters = declarations(&completion_types, next_value)?;
                let completion = block_id(allocate_dense(next_block)?);
                let mut targets = vec![completion];
                let mut parameters = vec![completion_parameters.clone()];
                for state in states {
                    targets.push(block_id(allocate_dense(next_block)?));
                    parameters.push(declarations(&state.parameter_types, next_value)?);
                }
                self.blocks.push(Block {
                    structural_parameters: std::mem::take(&mut self.block_structural_parameters),
                    id: self.current,
                    parameters: std::mem::take(&mut self.parameters),
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: erased_proof_formal_declarations(
                        identities,
                        checked,
                        &self.erased_proof_formals,
                    ),
                    operations: operations[self.operation_start..].to_vec(),
                    terminator: Terminator::Jump {
                        structural_arguments: Vec::new(),
                        edge: edge_id(allocate_dense(next_edge)?),
                        target: *targets.get(entry_index).ok_or(LoweringError::Unsupported(
                            "call computation entry is absent",
                        ))?,
                        arguments: values.iter().map(|value| value.id).collect(),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: lowered_proof_terms(
                            checked,
                            &self.proof_formal_forwarding(),
                            values,
                            &self.erased_scalar_formals,
                        )?,
                        cleanup_actions: Vec::new(),
                    },
                });
                let mut incoming_names = vec![None; states.len() + 1];
                *incoming_names
                    .get_mut(entry_index)
                    .ok_or(LoweringError::Unsupported(
                        "computation entry namespace is absent",
                    ))? = Some(self.current_claims.clone());
                for index in scalar_state_emission_order(states, entry_index)? {
                    let state = &states[index];
                    let mut state_evaluation = self.branch(targets[index + 1], operations.len());
                    state_evaluation.qualification_coercions.clear();
                    let reachable = incoming_names[index + 1].is_some();
                    let current_names = incoming_names[index + 1]
                        .clone()
                        .unwrap_or_else(|| self.current_claims.clone());
                    state_evaluation.current_claims = current_names;
                    emit_state(
                        identities,
                        checked,
                        state,
                        targets[index + 1],
                        &parameters[index + 1],
                        &self.erased_scalar_formals,
                        &self.erased_proof_formals,
                        &targets,
                        &parameters,
                        self.machine,
                        self.shared_qualifications.catalog(),
                        &mut self.qualification_coercions,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        calls,
                        &mut self.blocks,
                        environment,
                        &mut state_evaluation,
                        expansion
                            .claim_edges
                            .iter()
                            .find(|plan| plan.state_position == index),
                    )?;
                    crate::expression_preparation::bindings::retain_completed_structural_homes(
                        &mut self.structural_locals,
                        &state_evaluation.structural_locals,
                    )?;
                    crate::expression_preparation::bindings::retain_completed_structural_homes(
                        &mut self.completed_cleanup_homes,
                        &state_evaluation.completed_cleanup_homes,
                    )?;
                    self.retain_selection_sources(&state_evaluation.selection_cleanups)?;
                    self.qualification_coercions
                        .append(&mut state_evaluation.qualification_coercions);
                    if reachable {
                        state.terminator.for_each_target(|target| {
                            let mut arriving = state_evaluation.current_claims.clone();
                            let arguments = state.terminator.owned_successor_arguments();
                            if let Some(receiving) = target
                                .checked_sub(1)
                                .and_then(|position| states.get(position))
                            {
                                if arguments.len() == receiving.structural_parameters.len() {
                                    for (argument, parameter) in
                                        arguments.iter().zip(&receiving.structural_parameters)
                                    {
                                        if argument.access == StructuralAccess::Owned {
                                            let claims = state_evaluation
                                            .current_claims
                                            .source(argument)?
                                            .ok_or(LoweringError::Unsupported(
                                                "scalar arrival has no completed owned namespace",
                                            ))?;
                                            arriving.publish(parameter.place, claims)?;
                                        }
                                    }
                                } else if arguments
                                    .iter()
                                    .any(|argument| argument.access == StructuralAccess::Owned)
                                {
                                    return unsupported(
                                        "scalar arrival changes its owned receiving roster",
                                    );
                                }
                            }
                            let receiving = incoming_names.get_mut(target).ok_or(
                                LoweringError::Unsupported(
                                    "computation successor namespace is absent",
                                ),
                            )?;
                            if let Some(previous) = receiving {
                                previous.meet(&arriving)?;
                            } else {
                                *receiving = Some(arriving);
                            }
                            Ok(())
                        })?;
                    }
                }
                self.current_claims = incoming_names[0].take().ok_or(
                    LoweringError::Unsupported("computation completion has no reachable namespace"),
                )?;
                operations.storage_observations.clear();
                let result = completion_parameters[source_types.len()..].to_vec();
                *values = completion_parameters[..source_types.len()].to_vec();
                self.current = completion;
                self.parameters = completion_parameters;
                self.operation_start = operations.len();
                self.completion_proof_formals = erased_proof_formal_declarations(
                    identities,
                    checked,
                    &self.erased_proof_formals,
                );
                Ok(result)
            }
        }

        fn declarations(
            types: &[QualifiedScalarType],
            next_value: &mut u64,
        ) -> Result<Vec<ValueDeclaration>, LoweringError> {
            types
                .iter()
                .map(|scalar_type| {
                    Ok(ValueDeclaration {
                        qualifications: scalar_type.qualifications,
                        id: value_id(allocate_dense(next_value)?),
                        scalar_type: scalar_type.scalar_type,
                    })
                })
                .collect()
        }

        pub(crate) fn validated_values(
            values: Option<&[ValueDeclaration]>,
            types: &[QualifiedScalarType],
        ) -> Result<Vec<ValueDeclaration>, LoweringError> {
            let values = values.ok_or(LoweringError::Unsupported(
                "call has no completed scalar operands",
            ))?;
            if values.len() != types.len()
                || values
                    .iter()
                    .zip(types)
                    .any(|(value, expected)| value.value_type() != *expected)
            {
                return unsupported(
                    "completed call scalar operands disagree with the target signature",
                );
            }
            Ok(values.to_vec())
        }

        pub(crate) struct BlockEmissionSpan {
            pub(crate) current_claims: crate::emission::current_claims::CurrentClaimNames,
            pub(crate) erased_scalar_formals: Vec<ValueDeclaration>,
            pub(crate) erased_proof_formals: Vec<terminal_psi::ErasedProofFormal>,
            pub(crate) id: BlockId,
            pub(crate) parameters: Vec<ValueDeclaration>,
            pub(crate) structural_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
            pub(crate) operation_start: usize,
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn emit_dynamic_family_dispatch(
            span: &mut BlockEmissionSpan,
            blocks: &mut Vec<Block>,
            operations: &mut OperationBuffer,
            next_value: &mut u64,
            next_block: &mut u64,
            next_edge: &mut u64,
            tuples: &[Vec<String>],
            family_actuals: &[ValueId],
            family_actual_types: &[ScalarType],
            merge_result: &OperationResult,
            mut emit_leaf: impl FnMut(
                &mut OperationBuffer,
                &mut u64,
                usize,
            ) -> Result<OperationResult, LoweringError>,
        ) -> Result<(), LoweringError> {
            let arity = family_actuals.len();
            if arity == 0 || arity != family_actual_types.len() {
                return unsupported("dynamic family call's actuals disagree with its binders");
            }
            if tuples.len() < 2 || tuples.iter().any(|tuple| tuple.len() != arity) {
                return unsupported("dynamic family roster does not cover the call's binders");
            }
            let merge = block_id(allocate_dense(next_block)?);
            let crash = block_id(allocate_dense(next_block)?);
            let next_block_id = |next_block: &mut u64| -> Result<BlockId, LoweringError> {
                Ok(block_id(allocate_dense(next_block)?))
            };
            let dispatch = (0..tuples.len())
                .map(|_| next_block_id(next_block))
                .collect::<Result<Vec<_>, _>>()?;
            let tests = tuples
                .iter()
                .map(|tuple| {
                    (0..tuple.len())
                        .map(|_| next_block_id(next_block))
                        .collect::<Result<Vec<_>, _>>()
                })
                .collect::<Result<Vec<_>, _>>()?;
            let matched = emit_tuple_test(
                tuples,
                family_actuals,
                family_actual_types,
                next_value,
                operations,
                0,
                0,
            )?;
            let continue_target = |tuple: usize, binder: usize| -> BlockId {
                if binder + 1 < arity {
                    tests[tuple][binder + 1]
                } else {
                    dispatch[tuple]
                }
            };
            let next_tuple_target = |tuple: usize| -> BlockId {
                if tuple + 1 < tuples.len() {
                    tests[tuple + 1][0]
                } else {
                    crash
                }
            };
            blocks.push(Block {
                id: span.id,
                parameters: std::mem::take(&mut span.parameters),
                erased_scalar_formals: std::mem::take(&mut span.erased_scalar_formals),
                erased_proof_formals: std::mem::take(&mut span.erased_proof_formals),
                structural_parameters: std::mem::take(&mut span.structural_parameters),
                operations: operations[span.operation_start..].to_vec(),
                terminator: Terminator::Conditional {
                    condition: matched,
                    when_true: dispatch_successor_edge(next_edge, continue_target(0, 0))?,
                    when_false: dispatch_successor_edge(next_edge, next_tuple_target(0))?,
                },
            });
            for (tuple_index, test_blocks) in tests.iter().enumerate() {
                for (binder, &test_block) in test_blocks
                    .iter()
                    .enumerate()
                    .skip(usize::from(tuple_index == 0))
                {
                    let start = operations.len();
                    let matched = emit_tuple_test(
                        tuples,
                        family_actuals,
                        family_actual_types,
                        next_value,
                        operations,
                        tuple_index,
                        binder,
                    )?;
                    blocks.push(Block {
                        id: test_block,
                        parameters: Vec::new(),
                        erased_scalar_formals: Vec::new(),
                        erased_proof_formals: Vec::new(),
                        structural_parameters: Vec::new(),
                        operations: operations[start..].to_vec(),
                        terminator: Terminator::Conditional {
                            condition: matched,
                            when_true: dispatch_successor_edge(
                                next_edge,
                                continue_target(tuple_index, binder),
                            )?,
                            when_false: dispatch_successor_edge(
                                next_edge,
                                next_tuple_target(tuple_index),
                            )?,
                        },
                    });
                }
            }
            for (tuple_index, block) in dispatch.iter().enumerate() {
                let start = operations.len();
                let produced = emit_leaf(operations, next_value, tuple_index)?;
                blocks.push(Block {
                    id: *block,
                    parameters: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    operations: operations[start..].to_vec(),
                    terminator: Terminator::Jump {
                        edge: edge_id(allocate_dense(next_edge)?),
                        target: merge,
                        arguments: produced
                            .as_scalar()
                            .map(|declaration| declaration.id)
                            .into_iter()
                            .collect(),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        structural_arguments: produced
                            .structural()
                            .map(|result| terminal_psi::StructuralArgument {
                                place: result.place,
                                path: Vec::new(),
                                access: StructuralAccess::Owned,
                            })
                            .into_iter()
                            .collect(),
                        cleanup_actions: Vec::new(),
                    },
                });
            }
            blocks.push(Block {
                id: crash,
                parameters: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Crash {
                    edge: edge_id(allocate_dense(next_edge)?),
                    cause: terminal_psi::CrashCause::Abort,
                    site_guard: Vec::new(),
                    frontier_lower_bound: Vec::new(),
                },
            });
            span.id = merge;
            span.parameters = merge_result.as_scalar().into_iter().collect();
            span.structural_parameters = merge_result
                .structural()
                .map(|result| StructuralParameterDeclaration {
                    place: result.place,
                    position: 0,
                    structural_type: result.structural_type,
                    multiplicity: result.multiplicity,
                    access: StructuralAccess::Owned,
                    qualifications: result.qualifications.clone(),
                    projected_qualifications: result.projected_qualifications.clone(),
                    additional_input_qualifications: result.additional_input_qualifications.clone(),
                })
                .into_iter()
                .collect();
            span.operation_start = operations.len();
            Ok(())
        }

        fn emit_tuple_test(
            tuples: &[Vec<String>],
            family_actuals: &[ValueId],
            family_actual_types: &[ScalarType],
            next_value: &mut u64,
            operations: &mut OperationBuffer,
            tuple_index: usize,
            binder: usize,
        ) -> Result<ValueId, LoweringError> {
            let carrier =
                family_actual_types[binder]
                    .as_integer()
                    .ok_or(LoweringError::Unsupported(
                        "dynamic family actual has no integer carrier",
                    ))?;
            let value = family_tuple_integer(&tuples[tuple_index][binder], carrier.sign())?;
            let constant = emit_scalar_leaf(
                OperationKind::IntegerConstant { value },
                family_actual_types[binder],
                next_value,
                operations,
            );
            Ok(emit_scalar_leaf(
                OperationKind::IntegerEqual {
                    left: family_actuals[binder],
                    right: constant,
                },
                ScalarType::Boolean,
                next_value,
                operations,
            ))
        }

        fn family_tuple_integer(
            identity: &str,
            sign: semantic_vocabulary::IntegerSign,
        ) -> Result<semantic_vocabulary::IntegerValue, LoweringError> {
            let value = identity
                .strip_prefix("named(integer-const(")
                .and_then(|inner| inner.strip_suffix("))"))
                .and_then(|digits| digits.parse::<i128>().ok())
                .ok_or(LoweringError::Unsupported(
                    "dynamic family tuple identity is not an integer constant",
                ))?;
            semantic_vocabulary::IntegerValue::for_sign(sign, value).map_err(|_| {
                LoweringError::Unsupported("dynamic family tuple overflows its binder carrier")
            })
        }

        fn dispatch_successor_edge(
            next_edge: &mut u64,
            target: BlockId,
        ) -> Result<terminal_psi::SuccessorEdge, LoweringError> {
            Ok(value_successor(edge_id(allocate_dense(next_edge)?), target))
        }
    }
    pub(crate) mod argument_schedule {

        use super::{
            CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
            LoweringError, byte_subslices, unsupported,
        };
        pub(crate) enum Step {
            CaptureBefore(u32),
            Ordinary(usize),
            Begin,
            Argument {
                operation: usize,
                ordinal: usize,
            },
            Subslice {
                operation: usize,
                ordinal: usize,
            },
            Selector {
                operation: usize,
                argument: usize,
                segment: usize,
            },
            Structural {
                operation: usize,
                argument: usize,
                complete: bool,
            },
            Operation(usize),
            Call(usize),
            End,
        }

        pub(crate) fn build(
            checked: &CheckedTrees,
            plan: &CheckedUnitEffectMachinePlan,
        ) -> Result<Vec<Step>, LoweringError> {
            let operations = match plan.operations.split_last() {
                Some((CheckedUnitEffectOperationPlan::Complete { .. }, prefix)) => prefix,
                _ => plan.operations.as_slice(),
            };
            let mut steps = build_operations(checked, plan.machine, plan.state, operations)?;
            steps.push(Step::CaptureBefore(plan.statement_count));
            Ok(steps)
        }

        pub(crate) fn build_operations(
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
            operations: &[CheckedUnitEffectOperationPlan],
        ) -> Result<Vec<Step>, LoweringError> {
            let mut steps = Vec::new();
            let mut index = 0;
            while index < operations.len() {
                if let Some(statement) = operations[index].source_statement_index() {
                    steps.push(Step::CaptureBefore(statement));
                }
                if byte_subslices::contains(&operations[index])
                    && !operations[index].call().is_some_and(|call| {
                        call.is_boundary()
                            && call.structural_result().is_some()
                            && call.coordinate.call_ordinal != 0
                    })
                {
                    steps.push(Step::Begin);
                    append(
                        checked,
                        machine,
                        state,
                        operations,
                        index..index + 1,
                        index,
                        &mut Vec::new(),
                        &mut Vec::new(),
                        &mut steps,
                    )?;
                    steps.push(Step::End);
                    index += 1;
                    continue;
                }
                let statement = match operations[index].structural_result_binding() {
                    Some(result)
                        if operations[index + 1..].iter().any(|operation| {
                            operation.invocation_coordinate().is_some_and(|coordinate| {
                                coordinate.statement_index == result.statement_index
                                    && operation.structural_operands().iter().any(|argument| {
                                        argument.source_structural_result_binding_ordinal()
                                            == Some(result.binding_ordinal)
                                    })
                            })
                        }) =>
                    {
                        result.statement_index
                    }
                    _ => {
                        if operations[index].invocation_coordinate().is_some()
                            && !operations[index].structural_operands().is_empty()
                        {
                            steps.push(Step::Begin);
                            append(
                                checked,
                                machine,
                                state,
                                operations,
                                index..index + 1,
                                index,
                                &mut Vec::new(),
                                &mut Vec::new(),
                                &mut steps,
                            )?;
                            steps.push(Step::End);
                            index += 1;
                            continue;
                        }
                        steps.push(Step::Ordinary(index));
                        index += 1;
                        continue;
                    }
                };
                let start = index;
                let root = call_group_root(operations, start, statement)?;
                steps.push(Step::Begin);
                let mut emitted = Vec::new();
                append(
                    checked,
                    machine,
                    state,
                    operations,
                    start..root + 1,
                    root,
                    &mut Vec::new(),
                    &mut emitted,
                    &mut steps,
                )?;
                if !emitted.iter().copied().eq(start..root + 1) {
                    return unsupported(
                        "argument schedule disagrees with checked structural call order",
                    );
                }
                steps.push(Step::End);
                index = root + 1;
            }
            Ok(steps)
        }

        fn call_group_root(
            operations: &[CheckedUnitEffectOperationPlan],
            start: usize,
            statement: u32,
        ) -> Result<usize, LoweringError> {
            let mut root = start;
            loop {
                let Some(result) = operations[root].structural_result_binding() else {
                    return Ok(root);
                };
                let mut consumers = (root + 1..operations.len()).filter(|index| {
                    operations[*index]
                        .invocation_coordinate()
                        .is_some_and(|coordinate| {
                            coordinate.statement_index == statement
                                && operations[*index]
                                    .structural_operands()
                                    .iter()
                                    .any(|argument| {
                                        argument.source_structural_result_binding_ordinal()
                                            == Some(result.binding_ordinal)
                                    })
                        })
                });
                let Some(consumer) = consumers.next() else {
                    return Ok(root);
                };
                if consumers.next().is_some() {
                    return unsupported("argument result has multiple enclosing call occurrences");
                }
                root = consumer;
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn append(
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
            operations: &[CheckedUnitEffectOperationPlan],
            group: std::ops::Range<usize>,
            index: usize,
            active: &mut Vec<usize>,
            emitted: &mut Vec<usize>,
            steps: &mut Vec<Step>,
        ) -> Result<(), LoweringError> {
            if active.contains(&index) || emitted.contains(&index) || !group.contains(&index) {
                return unsupported(
                    "nested argument schedule repeats or escapes its producer group",
                );
            }
            if operations[index].call().is_none()
                && operations[index].structural_result_binding().is_some()
            {
                emitted.push(index);
                steps.push(Step::Operation(index));
                append_completions(operations, group, index, emitted, steps)?;
                return Ok(());
            }
            let operation = &operations[index];
            let invocation =
                operation
                    .structural_invocation()
                    .ok_or(LoweringError::Unsupported(
                        "argument schedule requires a retained invocation coordinate",
                    ))?;
            let coordinate = invocation.coordinate();
            let structural_arguments = invocation.structural_operands();
            let (evaluation_order, scalar_positions, structural_positions) = match invocation {
                checked_trees::CheckedStructuralInvocation::Call(call) => {
                    let authored = crate::shared::call_sites::locate(checked, state, coordinate)?;
                    let target = crate::shared::declarations::target_signature(
                        checked,
                        machine,
                        authored.source_target,
                    )?;
                    let structural_positions =
                        super::call_parameters::structural_positions(checked, call, &target)?;
                    let scalar_positions =
                        super::call_parameters::scalar_positions(checked, call, &target)?;
                    if target.parameters.len()
                        != authored.scalar_arguments.len() + authored.structural_arguments.len()
                        || call.scalar_operands().len() != scalar_positions.len()
                        || structural_arguments.len() != structural_positions.len()
                    {
                        return unsupported(
                            "nested argument schedule has no exact positional signature",
                        );
                    }
                    (
                        authored.evaluation_order,
                        scalar_positions,
                        structural_positions,
                    )
                }
                checked_trees::CheckedStructuralInvocation::BorrowedInstruction { .. } => {
                    (vec![0], Vec::new(), vec![0])
                }
            };
            active.push(index);
            for position in evaluation_order.iter().copied() {
                if let Some(scalar_ordinal) = scalar_positions
                    .iter()
                    .position(|scalar| *scalar == position)
                {
                    steps.push(Step::Argument {
                        operation: index,
                        ordinal: scalar_ordinal,
                    });
                    continue;
                }
                let Some(argument_ordinal) = structural_positions
                    .iter()
                    .position(|structural| *structural as usize == position)
                else {
                    continue;
                };
                // A runtime window argument evaluates its start and end exactly
                // once, inside the step that forms the window: `Subslice` for a
                // direct range, or the joined structural-value producer below.
                // Scheduling the endpoint roots again would re-run any call in
                // them (`cells[start()..end()]`) and give one call coordinate
                // two terminal sites.
                if byte_subslices::arguments(&operations[index])
                    .get(argument_ordinal)
                    .is_some_and(|argument| byte_subslices::range(argument).is_some())
                {
                    steps.push(Step::Subslice {
                        operation: index,
                        ordinal: argument_ordinal,
                    });
                    continue;
                }
                let joined = structural_arguments
                    .get(argument_ordinal)
                    .and_then(|argument| argument.source_structural_result_binding_ordinal())
                    .and_then(|binding| {
                        group.clone().find(|producer| {
                            operations[*producer]
                                .structural_result_binding()
                                .is_some_and(|result| result.binding_ordinal == binding)
                        })
                    });
                if let Some(producer) = joined {
                    append(
                        checked,
                        machine,
                        state,
                        operations,
                        group.clone(),
                        producer,
                        active,
                        emitted,
                        steps,
                    )?;
                }
                for segment in structural_arguments[argument_ordinal].call_selector_positions() {
                    steps.push(Step::Structural {
                        operation: index,
                        argument: argument_ordinal,
                        complete: false,
                    });
                    steps.push(Step::Selector {
                        operation: index,
                        argument: argument_ordinal,
                        segment,
                    });
                }
                if !matches!(
                    structural_arguments[argument_ordinal].source,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
                        | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
                        | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. }
                ) {
                    steps.push(Step::Structural {
                        operation: index,
                        argument: argument_ordinal,
                        complete: true,
                    });
                }
            }
            active.pop();
            emitted.push(index);
            steps.push(Step::Call(index));
            append_completions(operations, group, index, emitted, steps)?;
            Ok(())
        }

        fn append_completions(
            operations: &[CheckedUnitEffectOperationPlan],
            group: std::ops::Range<usize>,
            producer: usize,
            emitted: &mut Vec<usize>,
            steps: &mut Vec<Step>,
        ) -> Result<(), LoweringError> {
            for (index, operation) in operations[..group.end]
                .iter()
                .enumerate()
                .skip(producer + 1)
            {
                if operation.call().is_some() || operation.structural_result_binding().is_some() {
                    break;
                }
                if emitted.contains(&index) {
                    return unsupported("argument completion repeats a checked operation");
                }
                emitted.push(index);
                steps.push(Step::Operation(index));
            }
            Ok(())
        }
    }
}
