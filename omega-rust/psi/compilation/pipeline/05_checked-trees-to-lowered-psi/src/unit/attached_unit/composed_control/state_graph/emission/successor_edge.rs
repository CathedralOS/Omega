//! One successor edge of a composed-control state: the owners that die on
//! it, the staged evaluation of its scalar and structural arguments, its
//! erased actuals, and the rank it arrives with.

use super::super::super::super::super::{
    StructuralAccess, StructuralArgument, StructuralParameterDeclaration, SuccessorEdge, ValueId,
    block_id,
};
use super::super::super::super::{
    Block, CheckedScalarExpression, CheckedScalarExpressionRole, Operation, OperationKind,
    OperationResult, StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind,
    Terminator, ValueDeclaration, allocate_dense, direct_expression_contains_short_circuit,
    edge_id, emit_direct_expression, place_id, terminal_scalar_type, unsupported,
    validate_direct_parameter_types,
};
use super::super::super::LoweringError;
use super::super::{
    CheckedStructuralControlSuccessorPlan, case_emission, case_leaf_copy, ranking, scalars,
    subslices,
};
use super::StateGraphEmission;
use crate::emission::operation_emission::buffer::{CollectionLengthObservations, OperationBuffer};
use crate::expression_preparation::bindings::ScalarBindings;
use crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload;
use crate::terminal_identities::lookup_domain_id_for_carrier;
use crate::unit::attached_unit::argument_evaluation::Evaluation;

impl StateGraphEmission<'_, '_> {
    /// Lower one edge leaving the state at `frame.position`: dispose of the
    /// owners its target does not receive, evaluate its arguments (in a
    /// private staging block when a case edge or established payload needs
    /// them after selection), and record the target's arrival and rank.
    pub(super) fn successor_edge(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        frame: &SuccessorFrame<'_>,
        emitted: &mut EmittedState<'_>,
        edge: &CheckedStructuralControlSuccessorPlan,
        payload_values: &[(u32, ValueDeclaration)],
        case_edge: bool,
        values: &[ValueDeclaration],
        established: &[EstablishedCasePayload],
    ) -> Result<SuccessorEdge, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let SuccessorFrame {
            position,
            state_parameters,
            bindings,
            mut current_rank,
            inherited_lengths,
        } = *frame;
        let source_rank = current_rank;
        let state = &plan.states[position];
        let mut arrival_decided = checked.facts.call_requires_decided(
            state.state,
            edge.statement_ordinal,
            edge.target_state,
        );
        // A fused roster mixes owners: this state's authored facts live
        // under its own machine, and the edge's authored target signature
        // resolves through the roster position.
        let owner = self.admitted.authored[position].0.symbol;
        let EmittedState {
            operations,
            evaluation,
            next_value,
            next_block,
            next_edge,
            edge_blocks,
        } = emitted;
        let current_parameters = evaluation.current_parameters(state_parameters);
        let state_parameters = current_parameters.as_ref();
        operations.length_observations.clone_from(inherited_lengths);
        let target = plan
            .state_position(edge.target_state)
            .ok_or(LoweringError::Unsupported(
                "Unit graph target disappeared during emission",
            ))?;
        let operation_start = operations.len();
        // Reserve an independent entry even when the edge appears inline:
        // evaluating any operand may introduce private control flow. Its
        // completed blocks must not reuse the emitting state's block identity.
        let staged = block_id(allocate_dense(next_block)?);
        let mut edge_evaluation = evaluation.branch(staged, operation_start);
        let mut edge_values = values.to_vec();
        // A case edge's payloads are the staged block's formals; they join the
        // edge namespace ahead of every later read so `established` positions
        // name this vector's tail slots directly.
        for (_, value) in payload_values {
            edge_values.push(*value);
        }
        crate::expression_preparation::bindings::structural_fields::establish_case_payloads(
            &mut edge_evaluation.structural_fields,
            established,
        );
        edge_evaluation.parameters = payload_values.iter().map(|(_, value)| *value).collect();
        let mut arguments = Vec::new();
        let mut structural_arguments = Vec::new();
        let mut capture_parameters = Vec::new();
        let mut captured_loans = Vec::new();
        let mut ingress_renewals = Vec::new();
        let target_state = &plan.states[target];
        // Signature rows carry authored parameter positions, which go sparse
        // the moment a slot contributes no runtime operand: a borrowed `self`
        // the graph erased, or a proof-only formal the erased lane emits
        // below. Walk every position up to the highest one any lane names.
        let lane_bound = target_state
            .structural_parameters
            .iter()
            .map(|parameter| parameter.position)
            .chain(
                target_state
                    .scalar_parameters
                    .iter()
                    .map(|p| p.source_position),
            )
            .chain(
                target_state
                    .erased_scalar_parameters
                    .iter()
                    .map(|p| p.source_position),
            )
            .chain(
                target_state
                    .erased_proof_parameters
                    .iter()
                    .map(|p| p.source_position),
            )
            .map(|position| position as usize + 1)
            .max()
            .unwrap_or(0);
        for argument_position in 0..lane_bound {
            let runtime_row = target_state
                .structural_parameters
                .iter()
                .any(|parameter| parameter.position as usize == argument_position)
                || target_state
                    .scalar_parameters
                    .iter()
                    .any(|parameter| parameter.source_position as usize == argument_position);
            if !runtime_row {
                // Proof-only formals emit through the erased lane below.
                let erased_row = target_state
                    .erased_scalar_parameters
                    .iter()
                    .any(|parameter| parameter.source_position as usize == argument_position)
                    || target_state
                        .erased_proof_parameters
                        .iter()
                        .any(|parameter| parameter.source_position as usize == argument_position);
                if erased_row {
                    continue;
                }
            }
            if let Some((target_index, (target_parameter, transfer))) = target_state
                .structural_parameters
                .iter()
                .zip(&edge.transfers)
                .enumerate()
                .find(|(_, (parameter, _))| parameter.position as usize == argument_position)
            {
                if let Some(root) = self
                    .admitted
                    .entry_aliases
                    .get(target)
                    .and_then(|aliased| aliased.get(&(target_index as u32)))
                {
                    // A forwarded whole entry parameter carried across the
                    // edge keeps the entry parameter's place; the block
                    // does not rebind it, so the transfer row must still
                    // name that exact parameter and no edge argument is
                    // emitted. The entry place is read from the machine
                    // parameters directly: state_views drains as each target
                    // block seals, so a backward edge sees an emptied roster
                    // there.
                    let entry_place = self
                        .parameters
                        .get(*root as usize)
                        .map(|parameter| parameter.place);
                    let source_place = match transfer.source {
                        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                            index,
                        } => state_parameters
                            .get(index as usize)
                            .map(|parameter| parameter.place),
                        _ => {
                            return unsupported(
                                "Unit graph aliased successor is not a whole parameter transfer",
                            );
                        }
                    };
                    if source_place != entry_place {
                        return unsupported("Unit graph aliased successor lost its entry place");
                    }
                    continue;
                }
                let mut argument_path = Vec::new();
                let mut argument_bound_origins = Vec::new();
                let mut completed_binding = None;
                let mut captured_backing = None;
                let place = match transfer.source {
                        checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue { ref operands, ref establishment } => {
                            // The edge performs its ordinary establishment
                            // through the state's operation emitter, so the
                            // member calls run here, after selection, exactly
                            // as a `let` initializer's would. Its operands
                            // read the state's own results (a `let` local the
                            // call takes or copies), so the state's registry
                            // stays visible; the edge's bindings number after
                            // it. They are edge-local all the same: the
                            // state's registry is restored untouched once the
                            // argument's place is read, so a sibling edge
                            // neither sees this edge's bindings nor a move
                            // its operand call made.
                            let Some(result) = establishment.structural_result_binding() else {
                                return unsupported(
                                    "Unit graph established argument has no result binding",
                                );
                            };
                            let state_bindings = operations.structural_values.clone();
                            let selected_operations = operands.iter()
                                .chain(std::iter::once(establishment.as_ref()))
                                .cloned().collect::<Vec<_>>();
                            let emitted = super::super::super::emission::emit_call_operations(
                                identities,
                                checked,
                                plan.machine,
                                state,
                                &selected_operations,
                                self.catalogs,
                                state_parameters,
                                self.claims.published(),
                                &mut edge_evaluation,
                                &mut edge_values,
                                &self.state_erased[position],
                                next_value,
                                next_block,
                                next_edge,
                                operations,
                            )
                            .and_then(|()| {
                                case_emission::result(state, result.binding_ordinal, operations)
                                    .map(|binding| {
                                        let place = binding.place();
                                        // A completed move can name a subtree
                                        // of an existing owner. Its path is
                                        // part of the value, not an optional
                                        // hint: dropping it transfers the
                                        // containing record instead.
                                        argument_path = binding.path().to_vec();
                                        completed_binding = Some(binding.into_owned());
                                        place
                                    })
                            });
                            operations.structural_values = state_bindings;
                            edge_evaluation.current_structural_place(emitted?)
                        }
                        checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } => {
                            let binding = case_emission::result(state, binding_ordinal, operations)?;
                            argument_path = binding.path().to_vec();
                            edge_evaluation.current_structural_place(binding.place())
                        }
                        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
                            edge_evaluation.current_structural_place(state_parameters.get(index as usize).ok_or(
                                LoweringError::Unsupported("Unit graph transfer source descriptor disappeared"),
                            )?.place)
                        }
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ViewSubslice(_) => {
                            let destination = place_id(allocate_dense(&mut self.catalogs.next_place)?);
                            let emitted = subslices::emit(identities, checked, state, edge.statement_ordinal, &transfer.source, state_parameters, &mut edge_evaluation, destination, bindings, &mut edge_values, next_value, next_block, next_edge, operations, self.catalogs, &mut self.structural_places)?;
                            self.structural_places.push(emitted);
                            destination
                        }
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                            ref subject,
                            formation_expression,
                            index_occurrences,
                            carrier_type_identity,
                        } => {
                            let (root, root_type) = edge_evaluation.cleanup_storage(
                                &subject.source, state_parameters, |binding| {
                                    let produced = case_emission::result(state, binding, operations)?;
                                    Ok((produced.place(), produced.structural_type()))
                                },
                            )?;
                            // Resolve the projection against its actual source
                            // declaration, as for ordinary call arguments. A
                            // case step selects geometry, not authority: the
                            // receiving verifier still requires live membership
                            // on the edge that borrows its payload.
                            let mut calls = self.catalogs.scalar_calls.emission_context();
                            let mut selectors = Vec::new();
                            let source_value_count = edge_values.len();
                            for segment in &subject.path {
                                if let checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(handle) = segment {
                                    let root = checked
                                        .facts
                                        .values
                                        .scalar_computations
                                        .roots
                                        .iter()
                                        .map(|(_, root)| root)
                                        .find(|root| {
                                            root.root == *handle
                                                && root.machine == owner
                                                && root.state == state.state
                                                && root.statement_ordinal == edge.statement_ordinal
                                                && matches!(root.role, CheckedScalarExpressionRole::TransitionSelector {
                                                argument_ordinal, ..
                                            } if argument_ordinal as usize == argument_position)
                                        })
                                        .ok_or(LoweringError::Unsupported(
                                            "state loan selector lost its exact source occurrence",
                                        ))?;
                                    let value = edge_evaluation.source_value(
                                        identities,
                                        checked,
                                        owner,
                                        state.state,
                                        edge.statement_ordinal,
                                        root.role,
                                        &checked_trees::CheckedCallScalarArgument::Computation(*handle),
                                        source_value_count,
                                        &mut edge_values,
                                        next_value,
                                        next_block,
                                        next_edge,
                                        operations,
                                        &mut calls,
                                    )?;
                                    selectors.push(value.id);
                                }
                            }
                            let path =
                                crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(
                                    identities,
                                    &subject.path,
                                    root_type,
                                    &self.catalogs.structural_types,
                                    Some(&edge_values),
                                    &mut selectors.into_iter(),
                                    Some(&mut calls),
                                )?;
                            self.catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
                            // These are this producer's exact source-to-path
                            // associations, not equal selectors or old aliases.
                            // The path IDs are allocated by this actual operand
                            // frame and later move into its formation operation.
                            let mut decided_bounds = Vec::new();
                            for occurrence in checked.facts.flow.terminal_unit_effects.selector_bounds
                                .span(index_occurrences).ok_or(LoweringError::Unsupported(
                                    "state loan selector lost its source occurrence owner",
                                ))? {
                                let position = usize::try_from(occurrence.path_position)
                                    .map_err(|_| LoweringError::Unsupported("state loan selector position exceeds usize"))?;
                                let Some(terminal_psi::StructuralPathSegment::RuntimeIndex { obligation, .. }) = path.get(position) else {
                                    return unsupported("state loan selector lost its exact emitted carrying segment");
                                };
                                argument_bound_origins.push((*obligation, occurrence.occurrence));
                                if checked.facts.scalar_index_is_proven(occurrence.occurrence) {
                                    decided_bounds.push(*obligation);
                                }
                            }
                            if carrier_type_identity.is_valid()
                                && let Some(group) = self.captured_loans.group(target, target_index as u32)
                            {
                                let carrier_type = crate::terminal_identities::lookup_type_id(
                                    &self.catalogs.type_ids, &carrier_type_identity,
                                )?;
                                captured_backing = Some((root, path.clone()));
                                let (source_place, source_path) = match subject.source {
                                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index }
                                        if let Some(parent) = self.captured_loans.parent_carrier(position, parameter_index) => {
                                            let mut parent_path = if terminal_psi::find_structural_type(
                                                &self.catalogs.structural_types, parent.structural_type,
                                            ).is_some_and(|declaration| matches!(declaration.shape,
                                                terminal_psi::StructuralTypeShape::Reference { .. })) {
                                                vec![terminal_psi::StructuralPathSegment::Referent]
                                            } else { Vec::new() };
                                            parent_path.extend(path);
                                            (parent.place, parent_path)
                                        }
                                    _ => (root, path),
                                };
                                let source = StructuralArgument {
                                    place: source_place, path: source_path,
                                    access: crate::expression_preparation::bindings::structural_paths::terminal_access(subject.access),
                                };
                                let carrier = crate::unit::attached_unit::reference_results::emit_carrier(
                                    crate::unit::attached_unit::reference_results::Formation {
                                        checked, state: state.state,
                                        statement_index: edge.statement_ordinal as usize,
                                        expression: formation_expression,
                                    },
                                    carrier_type, source, None, &self.catalogs.structural_types,
                                    place_id(allocate_dense(&mut self.catalogs.next_place)?), operations,
                                )?;
                                // This formation owns the bounds at its
                                // evaluated source before cleanup. A later
                                // reference crossing grants no new index fact.
                                operations.source_checked_obligations.extend(decided_bounds);
                                captured_loans.push((group, carrier.id));
                                self.structural_places.push(carrier);
                                argument_path = if terminal_psi::find_structural_type(
                                    &self.catalogs.structural_types, carrier_type,
                                ).is_some_and(|declaration| matches!(declaration.shape,
                                    terminal_psi::StructuralTypeShape::Reference { .. })) {
                                    vec![terminal_psi::StructuralPathSegment::Referent]
                                } else { Vec::new() };
                                carrier.id
                            } else {
                                argument_path = path;
                                root
                            }
                        }
                        checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload {
                            ref subject, ref case_identity, ref field_identity, ref path,
                        } => {
                            let (source, root_type) = match subject.source {
                                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                                    let parameter = state_parameters.get(parameter_index as usize).ok_or(
                                        LoweringError::Unsupported("Unit graph case-payload subject descriptor disappeared"),
                                    )?;
                                    (edge_evaluation.current_structural_place(parameter.place), parameter.structural_type)
                                }
                                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                                    let produced = case_emission::result(state, binding_ordinal, operations)?;
                                    (edge_evaluation.current_structural_place(produced.place()), produced.structural_type())
                                }
                                _ => {
                                    return unsupported(
                                        "Unit graph case-payload subject source unsupported",
                                    );
                                }
                            };
                            if matches!(subject.access, checked_trees::CheckedStructuralAccess::WriteOnlyBorrow) {
                                return unsupported(
                                    "Unit graph case-payload subject is write-only",
                                );
                            }
                            let destination = place_id(allocate_dense(&mut self.catalogs.next_place)?);
                            self.structural_places.push(case_leaf_copy::emit(
                                identities,
                                &self.catalogs.structural_types,
                                source,
                                root_type,
                                &subject.path,
                                &case_identity.spelling(),
                                &field_identity.spelling(),
                                path,
                                target_parameter,
                                destination,
                                operations,
                            )?);
                            destination
                        }
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceLiteral {
                            ref bytes,
                        } => {
                            // An authored literal carries no custody root:
                            // the edge establishes the octets into immutable
                            // literal storage the target's shared byte view
                            // reads — the establishment a byte-literal call
                            // argument plans, with the parameter's declared
                            // domains as its membership evidence.
                            let destination =
                                place_id(allocate_dense(&mut self.catalogs.next_place)?);
                            let structural_type = crate::terminal_identities::lookup_type_id(
                                &self.catalogs.type_ids,
                                &target_parameter.type_identity,
                            )?;
                            let declaration_ordinal =
                                crate::emission::next_byte_sequence_literal_ordinal(
                                    &self.catalogs.temporary_places,
                                )?;
                            self.catalogs.temporary_places.push(
                                StructuralPlaceDeclaration {
                                    id: destination,
                                    kind: StructuralPlaceKind::ByteSequenceLiteral {
                                        declaration_ordinal,
                                        structural_type,
                                    },
                                },
                            );
                            let qualifications = target_parameter
                                .qualifications
                                .iter()
                                .map(|domain| {
                                    lookup_domain_id_for_carrier(
                                        &self.catalogs.domain_ids,
                                        *domain,
                                        structural_type,
                                    )
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            let producer = operations.allocate();
                            operations.push(Operation {
                                static_reach_binding: None,
                                suspension_crossing: None,
                                id: producer,
                                result: OperationResult::Unit,
                                kind: OperationKind::EstablishByteSequenceLiteral {
                                    destination,
                                    bytes: bytes.clone(),
                                    qualifications,
                                },
                            });
                            destination
                        }
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedValue {
                            ref subject,
                        } => {
                            // Copyable observations materialize a fresh leaf;
                            // an owned move instead retains the original root
                            // and path on the edge. The edge transaction first
                            // captures that subtree, then disposes its complement.
                            let (source, root_type) = match subject.source {
                                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                    parameter_index,
                                } => {
                                    let parameter = state_parameters.get(parameter_index as usize).ok_or(
                                        LoweringError::Unsupported(
                                            "Unit graph projected subject descriptor disappeared",
                                        ),
                                    )?;
                                    (parameter.place, parameter.structural_type)
                                },
                                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                    binding_ordinal,
                                } => {
                                    let result = case_emission::result(
            state, binding_ordinal, operations)?;
                                    (result.place(), result.structural_type())
                                },
                                _ => return unsupported(
            "Unit graph projected value has no established root"),
                            };
                            let source = edge_evaluation.current_structural_place(
            source);
                            let path = crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(identities, &subject.path, root_type, &self.catalogs.structural_types, None, &mut std::iter::empty(), None)?;
                            if target_parameter.multiplicity != language_semantics::Multiplicity::Unrestricted {
                                argument_path = path;
                                source
                            } else {
                            let destination = place_id(allocate_dense(&mut self.catalogs.next_place)?);
                            let structural_type = crate::terminal_identities::lookup_type_id(
                                &self.catalogs.type_ids,
                                &target_parameter.type_identity,
                            )?;
                            let producer = operations.allocate();
                            operations.push(Operation {
                                static_reach_binding: None,
                                suspension_crossing: None,
                                id: producer,
                                result: OperationResult::Structural(
                                    terminal_psi::StructuralOperationResult {
                                        qualification_establishments: Vec::new(),
                                        place: destination,
                                        structural_type,
                                        multiplicity: StructuralMultiplicity::Unrestricted,
                                        qualifications: Vec::new(),
                                        projected_qualifications: Vec::new(),
                                        claims: Vec::new(),
                                    },
                                ),
                                kind: OperationKind::StructuralLeafCopy { source, path },
                            });
                            self.catalogs.temporary_places.push(
                                StructuralPlaceDeclaration {
                                    id: destination,
                                    kind: StructuralPlaceKind::OperationResult {
                                        producer,
                                        structural_type,
                                    },
                                },
                            );
                            destination
                            }
                        }
                    };
                // A member of a later state's loan chain (`loan_chains`)
                // either keeps a view of its one lender's home or receives the
                // moved storage; neither retains an entry home.
                let chain_arrival = self.loan_chains.arrival(target, target_index as u32);
                if chain_arrival.is_some() {
                    let lender = match &transfer.source {
                        checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue { .. } => {
                            Some(std::borrow::Cow::Borrowed(completed_binding.as_ref().ok_or(
                                LoweringError::Unsupported("Unit graph completed loan lost its emitted home"),
                            )?))
                        }
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan { subject, .. } => {
                            subject.source.current_result_binding(&state.operations)
                                .map(|binding| case_emission::result(state, binding, operations))
                                .transpose()?
                        }
                        checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } => {
                            Some(case_emission::result(state, *binding_ordinal, operations)?)
                        }
                        _ => None,
                    };
                    if let Some(result) = lender {
                        if !result.claims().is_empty()
                            || !result.path().is_empty()
                            || !captured_backing
                                .as_ref()
                                .map_or(&argument_path, |(_, path)| path)
                                .is_empty()
                        {
                            return unsupported(
                                "Unit graph state loan chain home is not plain whole storage",
                            );
                        }
                        self.loan_chains.lend(
                            target,
                            target_index as u32,
                            captured_backing
                                .as_ref()
                                .map_or(place, |(backing, _)| *backing),
                        )?;
                    }
                }
                // Shared loan subjects keep their backing local's storage
                // for the loan's duration exactly like mutable ones: the
                // activation-entry home dies once at the machine's return.
                if chain_arrival.is_none()
                    && matches!(
                        target_parameter.access,
                        checked_trees::CheckedStructuralAccess::MutableBorrow
                            | checked_trees::CheckedStructuralAccess::SharedBorrow
                    )
                {
                    let loaned_binding = match transfer.source {
                        checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } => Some(binding_ordinal),
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan { ref subject, .. } => subject.source.current_result_binding(&state.operations),
                        _ => None,
                    };
                    if let Some(binding_ordinal) = loaned_binding {
                        let result = case_emission::result(state, binding_ordinal, operations)?;
                        // A carried view lends the home of the local it was
                        // formed from; that home stays live with the loan
                        // and dies with it, exactly as a directly lent one.
                        let backing = match transfer.source {
                            checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { .. } => {
                                super::super::result_custody::view_backing(
                                    checked,
                                    state,
                                    binding_ordinal,
                                )
                                .map(|backing| {
                                    case_emission::result(state, backing, operations)
                                        .map(|home| (backing, home))
                                })
                                .transpose()?
                                .filter(|(_, home)| {
                                    home.multiplicity() == StructuralMultiplicity::Affine
                                })
                            }
                            _ => None,
                        };
                        // A shared loan of unrestricted storage has no death
                        // to place and records no cleanup home, whether the
                        // checked edge names the result directly or borrows a
                        // projection of it. Place/type/loan checks above still
                        // retain its actual backing; syntax does not create
                        // an activation-entry cleanup requirement.
                        let needs_home = matches!(
                            target_parameter.access,
                            checked_trees::CheckedStructuralAccess::MutableBorrow
                                | checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
                        ) || result.multiplicity()
                            == StructuralMultiplicity::Affine
                            || backing.is_some();
                        let released_ingress = match &transfer.source {
                            checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan { subject, .. } => {
                                super::loan_arrivals::released_ingress(
                                    checked, owner, state, edge.statement_ordinal,
                                    binding_ordinal, &subject.path, target_parameter.access,
                                )
                            }
                            _ => None,
                        };
                        if let Some(ingress) = released_ingress {
                            let parameter = state_parameters.get(ingress.parameter).ok_or(
                                LoweringError::Unsupported(
                                    "released loan lost its captured ingress formal",
                                ),
                            )?;
                            let mut selectors = std::iter::empty();
                            let mut source_path = crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(
                                identities, &ingress.path, parameter.structural_type,
                                &self.catalogs.structural_types, Some(&edge_values), &mut selectors, None,
                            )?;
                            let referent = argument_path
                                .iter()
                                .position(|segment| {
                                    matches!(segment, terminal_psi::StructuralPathSegment::Referent)
                                })
                                .ok_or(LoweringError::Unsupported(
                                    "released loan lost its evaluated referent projection",
                                ))?;
                            // Reuse completed selector values and bounds obligations;
                            // reconstructing their source expressions would reevaluate.
                            source_path.extend_from_slice(&argument_path[referent + 1..]);
                            let retirement = if ingress.descriptor_path.is_empty() {
                                edge_evaluation.descriptor_retirement(result.place(), operations)?
                            } else {
                                let mut selectors = std::iter::empty();
                                let descriptor_path = crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(
                                    identities, &ingress.descriptor_path, result.structural_type(),
                                    &self.catalogs.structural_types, Some(&edge_values), &mut selectors, None,
                                )?;
                                let descriptor = terminal_semantics::runtime_structural_path_tip(
                                    self.catalogs.structural_types.iter(),
                                    result.structural_type(),
                                    &descriptor_path,
                                )
                                .ok_or(
                                    LoweringError::Unsupported(
                                        "released loan lost its exact descriptor leaf",
                                    ),
                                )?;
                                terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
                                    terminal_psi::StructuralAffineDiscard {
                                        place: result.place(),
                                        path: descriptor_path,
                                        structural_type: descriptor.id,
                                    },
                                )
                            };
                            let mut preserved_bounds = Vec::new();
                            let source_state = checked.typed.state_by_symbol(state.state).ok_or(
                                LoweringError::Unsupported(
                                    "loan arrival lost its original exit owner",
                                ),
                            )?;
                            let source_statement = checked
                                .statement_table
                                .statements(source_state.statement_nodes)
                                .get(edge.statement_ordinal as usize)
                                .ok_or(LoweringError::Unsupported(
                                    "loan arrival lost its original exit occurrence",
                                ))?;
                            let kind = match source_statement {
                                typed_trees::statement::StatementNode::Expression(_) => {
                                    checked_trees::FlowClaimJoinExitKind::FinalExpression
                                }
                                typed_trees::statement::StatementNode::Transition(_)
                                    if edge.is_continuation =>
                                {
                                    checked_trees::FlowClaimJoinExitKind::TransitionContinuation
                                }
                                typed_trees::statement::StatementNode::Transition(_) => {
                                    checked_trees::FlowClaimJoinExitKind::TransitionTarget
                                }
                                _ => {
                                    return unsupported("loan arrival has no original normal exit");
                                }
                            };
                            let source_exit = checked_trees::FlowClaimJoinExit {
                                state_symbol: state.state,
                                statement_index: edge.statement_ordinal as usize,
                                kind,
                            };
                            for (obligation, occurrence) in &argument_bound_origins {
                                // Prefix insertion does not change this allocated carrying ID.
                                // Only the actual suffix transferred to the renewed path qualifies.
                                if source_path.iter().any(|segment| matches!(segment,
                                    terminal_psi::StructuralPathSegment::RuntimeIndex { obligation: actual, .. }
                                        if actual == obligation))
                                    && checked.facts.scalar_index_exit_preservations.iter().any(|(_, fact)|
                                        fact.machine == owner && fact.exit == source_exit
                                            && fact.occurrence == *occurrence)
                                {
                                    preserved_bounds.push(*obligation);
                                }
                            }
                            ingress_renewals.push(super::loan_arrivals::IngressRenewal {
                                argument: structural_arguments.len(),
                                source: StructuralArgument {
                                    place: edge_evaluation.current_structural_place(parameter.place),
                                    path: source_path,
                                    access: crate::expression_preparation::bindings::structural_paths::terminal_access(target_parameter.access),
                                },
                                retirement, preserved_bounds, source_exit,
                            });
                        } else if needs_home && (position != 0 || self.entry_reentered) {
                            return unsupported(
                                "Unit graph local loan requires an activation entry home",
                            );
                        }
                        let homes = (result.multiplicity() == StructuralMultiplicity::Affine
                            && !ingress_renewals
                                .iter()
                                .any(|renewal| renewal.argument == structural_arguments.len()))
                        .then_some((
                            binding_ordinal,
                            captured_backing
                                .as_ref()
                                .map_or(place, |(backing, _)| *backing),
                        ))
                        .into_iter()
                        .chain(backing.map(|(backing, home)| {
                            (
                                backing,
                                edge_evaluation.current_structural_place(home.place()),
                            )
                        }));
                        // Each edge lends the local at its edge-current place;
                        // a value selection on one edge can move it to a join
                        // place. The entry route retains one home per local,
                        // so it has no route for a local lent from two places.
                        for (binding, home) in homes {
                            if let Some(previous) = self.loaned_local_homes.insert(binding, home)
                                && previous != home
                            {
                                return unsupported(
                                    "Unit graph local loan is lent from two entry homes",
                                );
                            }
                        }
                    }
                }
                let mut capture = StructuralParameterDeclaration {
                    place,
                    position: u32::try_from(capture_parameters.len()).map_err(|_| {
                        LoweringError::Unsupported("cleanup capture structural roster exceeds u32")
                    })?,
                    structural_type: crate::terminal_identities::lookup_type_id(
                        &self.catalogs.type_ids,
                        &target_parameter.type_identity,
                    )?,
                    multiplicity: crate::emission::borrowed_window::terminal_multiplicity(
                        target_parameter.multiplicity,
                    ),
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                };
                // Private cleanup transport retains the same owner as the
                // authored arrival, including a hidden affine readonly home.
                self.loan_chains
                    .declare(target, target_index as u32, &mut capture);
                capture_parameters.push(capture);
                structural_arguments.push(StructuralArgument {
                    place,
                    path: argument_path,
                    // The arrival retains its exact permission, except for
                    // the explicit owned/exclusive handoffs below.
                    access: match target_parameter.access {
                        _ if chain_arrival
                            == Some(super::loan_chains::ChainArrival::Transferred) =>
                        {
                            StructuralAccess::Owned
                        }
                        // A shared parameter that holds its source's whole
                        // exclusive loan receives that loan's mutable name.
                        _ if self
                            .admitted
                            .exclusive_handoffs
                            .get(target)
                            .is_some_and(|promoted| promoted.contains(&(target_index as u32))) =>
                        {
                            StructuralAccess::MutableBorrow
                        }
                        checked_trees::CheckedStructuralAccess::Owned => StructuralAccess::Owned,
                        checked_trees::CheckedStructuralAccess::MutableBorrow => {
                            StructuralAccess::MutableBorrow
                        }
                        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow => {
                            StructuralAccess::WriteOnlyBorrow
                        }
                        checked_trees::CheckedStructuralAccess::SharedBorrow => {
                            StructuralAccess::SharedBorrow
                        }
                    },
                });
                continue;
            }
            if let Some((scalar_position, _)) = target_state
                .scalar_parameters
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.source_position as usize == argument_position)
                && let Some((_, payload)) = payload_values
                    .iter()
                    .find(|(position, _)| *position as usize == scalar_position)
            {
                arguments.push(payload.id);
                continue;
            }
            let transfer = edge
                .scalar_arguments
                .iter()
                .find(|transfer| transfer.argument_ordinal as usize == argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph successor argument position missing",
                ))?;
            let expression = match transfer.source {
                checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter { index } => {
                    bindings.expression(
                        identities,
                        &checked.facts.proven_scalar_indexes,
                        &CheckedScalarExpression::Parameter {
                            position: index as usize,
                            primitive_type: transfer.primitive_type,
                        },
                    )?
                }
                checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression => {
                    let value = scalars::successor_value(checked, state, edge, transfer)?;
                    let mut calls = self.catalogs.scalar_calls.emission_context();
                    let value = edge_evaluation.source_value(
                        identities,
                        checked,
                        owner,
                        state.state,
                        edge.statement_ordinal,
                        CheckedScalarExpressionRole::TransitionArgument {
                            argument_ordinal: transfer.argument_ordinal,
                        },
                        &value,
                        edge_values.len(),
                        &mut edge_values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        &mut calls,
                    )?;
                    self.catalogs.scalar_calls.next_call_obligation =
                        calls.next_obligation_identity;
                    if value.scalar_type != terminal_scalar_type(transfer.primitive_type)? {
                        return unsupported("Unit graph successor value has the wrong carrier");
                    }
                    arguments.push(value.id);
                    continue;
                }
            };
            if expression.scalar_type() != terminal_scalar_type(transfer.primitive_type)?
                || direct_expression_contains_short_circuit(&expression)
            {
                return unsupported("Unit graph successor needs a matching branch-free value");
            }
            validate_direct_parameter_types(
                &expression,
                &edge_values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>(),
            )?;
            arguments.push(emit_direct_expression(
                &expression,
                &edge_values,
                next_value,
                operations,
            ));
        }
        // Operand evaluation establishes the outgoing values before the exact
        // selected source exit determines the remaining owned dispositions.
        // Keep action kinds and their checked order through private staging.
        let exit = super::super::cleanup::coordinate(
            state,
            edge.statement_ordinal as usize,
            edge.is_continuation,
        )?;
        let mut cleanup_regions = super::super::cleanup::for_exit_regions(
            identities,
            self.claims.published(),
            self.checked,
            state,
            exit,
            self.catalogs,
            state_parameters,
            &edge_evaluation,
            operations,
        )?;
        super::super::cleanup::selection(&edge_evaluation, operations, &mut cleanup_regions)?;
        // The issued source preservation must cover the cleanup actually emitted,
        // including selected nominal deaths added by the shared selection owner.
        // Missing associations keep the ordinary final-path obligation undecided.
        let cleanup_coverage = cleanup_regions.iter().all(|region| {
            if !matches!(
                region.action,
                terminal_psi::TerminalAffineCleanupAction::InvokeNominal(_)
            ) {
                return true;
            }
            if !region.storage_effect.is_valid() {
                return false;
            }
            let effect = checked
                .facts
                .contextual_cleanup
                .hook_effects
                .get(region.storage_effect);
            let disposition = checked
                .facts
                .contextual_cleanup
                .dispositions
                .get(effect.disposition);
            disposition.machine == owner
                && disposition.state == state.state
                && disposition.exit == exit
        });
        for renewal in &mut ingress_renewals {
            if !cleanup_coverage {
                renewal.preserved_bounds.clear();
            }
        }
        let has_nominal_cleanup = cleanup_regions.iter().any(|region| {
            matches!(
                region.action,
                terminal_psi::TerminalAffineCleanupAction::InvokeNominal(_),
            )
        });
        let mut preservation_rows =
            checked
                .facts
                .call_exit_preservations
                .iter()
                .filter(|(_, row)| {
                    row.machine == owner
                        && row.exit == exit
                        && row.target_state == edge.target_state
                });
        let preservation = preservation_rows.next().map(|(_, row)| row);
        let preservation = if preservation_rows.next().is_none() {
            preservation
        } else {
            None
        };
        // The original verdict is an observation before cleanup. Only an
        // exact source-issued preservation, covering every actual emitted
        // nominal hook, can authorize the final receiver's clause afterward.
        if has_nominal_cleanup {
            arrival_decided &=
                cleanup_coverage && preservation.is_some_and(|row| row.requires_preserved);
        }

        // Proof-only actuals never evaluate: each checked erased row
        // lowers to a term over the emitting state's scalar values and
        // erased formals, in the target's erased-roster order.
        let mut erased_arguments = edge
            .erased_arguments
            .iter()
            .map(|argument| {
                let expression = bindings.expression_at(
                    identities,
                    checked,
                    state.state,
                    edge.statement_ordinal,
                    CheckedScalarExpressionRole::TransitionArgument {
                        argument_ordinal: argument.argument_ordinal,
                    },
                )?;
                crate::proofs::crash_routes::lowered_direct_scalar_term(
                    &expression,
                    &edge_values,
                    &self.state_erased[position],
                )
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        // Proof actuals keep the same rule: each checked term resolves
        // against the emitting state's roster and must land in the
        // target state's roster order.
        let mut erased_proof_arguments = edge
            .erased_proof_arguments
            .iter()
            .map(|term| {
                crate::scalar_lowering::contracts::checked_proof_term(
                    identities,
                    checked,
                    term,
                    &self.state_erased_proof[position],
                )
                .and_then(|term| {
                    crate::scalar_lowering::contracts::lowered_proof_term(
                        checked,
                        &term,
                        &edge_values,
                        &self.state_erased[position],
                    )
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        if has_nominal_cleanup {
            // Erased structural terms are not evaluated runtime snapshots.
            // Relocating a copied descriptor cannot preserve a read of bytes
            // changed by cleanup. Unsupported observations refuse transport;
            // leaving Requires undecided alone would not fix argument meaning.
            for (plan, term) in edge.erased_arguments.iter().zip(&erased_arguments) {
                if super::loan_arrivals::erased_scalar_observes_storage(term)
                    && !(cleanup_coverage
                        && preservation.is_some_and(|row| {
                            checked
                                .facts
                                .erased_exit_observations
                                .span_or_empty(row.erased_observations)
                                .iter()
                                .any(|observation| {
                                    observation.argument_ordinal == plan.argument_ordinal
                                        && observation.preserved
                                })
                        }))
                {
                    return unsupported("cleanup does not preserve an erased argument observation");
                }
            }
            for ((source, term), parameter) in edge
                .erased_proof_arguments
                .iter()
                .zip(&erased_proof_arguments)
                .zip(&target_state.erased_proof_parameters)
            {
                let role = if edge.is_continuation {
                    checked_trees::CheckedProofTermRole::TransitionContinuationArgument {
                        argument_ordinal: parameter.source_position,
                    }
                } else {
                    checked_trees::CheckedProofTermRole::TransitionArgument {
                        argument_ordinal: parameter.source_position,
                    }
                };
                let located = checked
                    .facts
                    .values
                    .proof_terms
                    .located_term_at(state.state, edge.statement_ordinal, role)
                    .filter(|located| &located.term == source);
                if super::loan_arrivals::erased_proof_observes_storage(term)
                    && !(cleanup_coverage
                        && preservation.is_some_and(|row| {
                            located.is_some_and(|located| {
                                checked
                                    .facts
                                    .erased_exit_observations
                                    .span_or_empty(row.erased_observations)
                                    .iter()
                                    .any(|observation| {
                                        observation.expression == located.expression
                                            && observation.argument_ordinal
                                                == parameter.source_position
                                            && observation.preserved
                                    })
                            })
                        }))
                {
                    return unsupported(
                        "cleanup does not preserve an erased proof argument observation",
                    );
                }
            }
        }
        // The arriving rank strictly decreases unless the rank judgment
        // proves this edge only preserves it.
        let mut arriving_comparison = if plan.preserving_rank_edges.iter().any(|weak| {
            weak.source_state == state.state
                && weak.statement_ordinal == edge.statement_ordinal
                && weak.target_state == edge.target_state
        }) {
            terminal_psi::TerminalNaturalRankComparison::Preserving
        } else {
            terminal_psi::TerminalNaturalRankComparison::Strict
        };
        let mut arriving_rank = if current_rank.is_some()
            && let Some(root) = ranking::observation(checked, plan, target_state)?
        {
            // This exact target metric is evaluated before cleanup. Capturing
            // its scalar result does not freeze fields behind its descriptors.
            if has_nominal_cleanup
                && !(cleanup_coverage
                    && preservation
                        .is_some_and(|row| row.rank_observation == root.root && row.rank_preserved))
            {
                return unsupported("cleanup does not preserve the arriving rank observation");
            }
            // Scalar and structural operands are already complete. Rejoin the
            // exact target telescope here, before projected transfer or cleanup.
            let mut scalar_slots = Vec::with_capacity(arguments.len());
            for (argument, formal) in arguments.iter().zip(&target_state.scalar_parameters) {
                let declaration = Evaluation::completed_scalar_declaration(
                    *argument,
                    &[(&*evaluation, values), (&edge_evaluation, &edge_values)],
                    operations,
                )?;
                scalar_slots.push((
                    edge_values.len(),
                    terminal_scalar_type(formal.primitive_type)?,
                ));
                edge_values.push(declaration);
            }
            let mut emitted = structural_arguments.iter();
            let structural_actuals = target_state
                .structural_parameters
                .iter()
                .enumerate()
                .map(|(dense, formal)| {
                    let argument = if let Some(entry) =
                        self.admitted.entry_aliases[target].get(&(dense as u32))
                    {
                        let retained = self.parameters.get(*entry as usize).ok_or(
                            LoweringError::Unsupported(
                                "Unit graph observation lost its retained entry alias",
                            ),
                        )?;
                        StructuralArgument {
                            place: retained.place,
                            path: Vec::new(),
                            access: retained.access,
                        }
                    } else {
                        emitted.next().cloned().ok_or(LoweringError::Unsupported(
                            "Unit graph observation lost its completed structural actual",
                        ))?
                    };
                    Ok((
                        argument,
                        crate::terminal_identities::lookup_type_id(
                            &self.catalogs.type_ids,
                            &formal.type_identity,
                        )?,
                    ))
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            if emitted.next().is_some() || arguments.len() != target_state.scalar_parameters.len() {
                return unsupported("Unit graph observation has excess completed actuals");
            }
            let backing = edge_evaluation.completed_bindings(
                operations,
                self.catalogs
                    .temporary_places
                    .iter()
                    .chain(&self.structural_places)
                    .cloned(),
            );
            let bindings = ranking::formal_bindings(
                checked,
                target_state,
                &backing,
                &scalar_slots,
                &structural_actuals,
            )?;
            let mut calls = self.catalogs.scalar_calls.emission_context();
            let observed = edge_evaluation.retained_observation(
                identities,
                checked,
                root,
                &bindings,
                &mut edge_values,
                next_value,
                next_block,
                next_edge,
                operations,
                &mut calls,
            )?;
            self.catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
            if observed.scalar_type.as_integer().is_none() {
                return unsupported("Unit graph rank observation lost its integer carrier");
            }
            Some(observed.id)
        } else {
            None
        };
        let owner_arguments =
            self.captured_loans
                .arrivals(position, target, edge, &captured_loans)?;
        for (argument, mut declaration) in owner_arguments
            .iter()
            .zip(self.captured_loans.declarations(target))
        {
            declaration.place = argument.place;
            declaration.position = u32::try_from(capture_parameters.len())
                .map_err(|_| LoweringError::Unsupported("state capture roster exceeds u32"))?;
            capture_parameters.push(declaration);
        }
        structural_arguments.extend(owner_arguments);
        let mut target = self.state_ids[target];
        for renewal in &ingress_renewals {
            if !cleanup_regions.iter().any(|region| {
                region.action == renewal.retirement
                    && region.conditions.is_empty()
                    && region.selected_region.is_none()
            }) {
                return unsupported(
                    "released ingress arrival has no unconditional checked descriptor retirement",
                );
            }
        }
        // A renewed ingress resolves after its descriptor ends. Keep that
        // operand out of pre-cleanup captures; all other actuals still take
        // the same ordinary ownership snapshot and selected cleanup route.
        let retained_renewals = ingress_renewals
            .iter()
            .map(|renewal| {
                (
                    renewal.argument,
                    structural_arguments[renewal.argument].clone(),
                    capture_parameters[renewal.argument].clone(),
                )
            })
            .collect::<Vec<_>>();
        for (index, _, _) in retained_renewals.iter().rev() {
            structural_arguments.remove(*index);
            capture_parameters.remove(*index);
        }
        for (index, declaration) in capture_parameters.iter_mut().enumerate() {
            declaration.position = u32::try_from(index)
                .map_err(|_| LoweringError::Unsupported("state capture roster exceeds u32"))?;
        }
        // The source-local roster is not the complete outgoing frame: computed
        // actuals, rank observations and evaluated selectors have their own
        // emitted declarations. Retain those exact results before any cleanup
        // continuation remaps the frame. Erased source formals remain erased.
        let mut completed = arguments.clone();
        completed.extend(current_rank);
        completed.extend(arriving_rank);
        for renewal in &ingress_renewals {
            for segment in &renewal.source.path {
                if let terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                    completed.push(*index);
                }
            }
        }
        let mut retain_term = |term: &semantic_vocabulary::ScalarTerm| {
            term.visit_value_ids(|value| {
                if !edge_evaluation
                    .erased_scalar_formals
                    .iter()
                    .any(|formal| formal.id == value)
                {
                    completed.push(value);
                }
                true
            });
        };
        for term in &erased_arguments {
            retain_term(term);
        }
        let mut proof_terms = erased_proof_arguments.iter().collect::<Vec<_>>();
        while let Some(term) = proof_terms.pop() {
            match term {
                semantic_vocabulary::ProofTerm::Scalar(term) => retain_term(term),
                semantic_vocabulary::ProofTerm::Construction { fields, .. } => {
                    proof_terms.extend(fields.iter().map(|field| &field.term));
                }
                semantic_vocabulary::ProofTerm::Formal { .. } => {}
            }
        }
        for value in completed {
            let declaration = Evaluation::completed_scalar_declaration(
                value,
                &[(&*evaluation, values), (&edge_evaluation, &edge_values)],
                operations,
            )?;
            if !edge_values.iter().any(|retained| retained.id == value) {
                edge_values.push(declaration);
            }
        }
        // All operands and rank observations are already captured. Transfer
        // projected structural custody before selected cleanup can retire the
        // old home; the authored successor receives only whole captures.
        if !cleanup_regions.is_empty() {
            let capture = edge_evaluation.capture_cleanup_arguments(
                identities,
                self.checked,
                &mut structural_arguments,
                &capture_parameters,
                &mut edge_values,
                &mut self.catalogs.next_place,
                &mut self.catalogs.temporary_places,
                next_value,
                next_block,
                next_edge,
                operations,
            )?;
            super::loan_arrivals::transport_captured_arguments(
                &self.catalogs.structural_types,
                &capture,
                &mut arguments,
                &mut erased_arguments,
                &mut erased_proof_arguments,
                &mut ingress_renewals,
                &mut current_rank,
                &mut arriving_rank,
            )?;
        }
        // Cleanup continuations preserve this actual value roster in order.
        // Retain the producer's old/new declarations across all selected arms;
        // no value equality or receiving signature reconstructs the mapping.
        let cleanup_values = edge_values.clone();
        let cleanup_actions = edge_evaluation.emit_cleanup_regions(
            identities,
            self.checked,
            cleanup_regions,
            &mut edge_values,
            next_value,
            next_block,
            next_edge,
            operations,
        )?;
        if cleanup_values.len() != edge_values.len() {
            return unsupported("cleanup continuation lost its captured scalar roster");
        }
        let cleanup_capture = crate::unit::attached_unit::argument_evaluation::cleanup_control::CapturedArgumentFrame {
            values: cleanup_values.into_iter().zip(edge_values.iter().cloned()).collect(),
            structural: Vec::new(),
        };
        super::loan_arrivals::transport_captured_arguments(
            &self.catalogs.structural_types,
            &cleanup_capture,
            &mut arguments,
            &mut erased_arguments,
            &mut erased_proof_arguments,
            &mut ingress_renewals,
            &mut current_rank,
            &mut arriving_rank,
        )?;
        for (index, argument, declaration) in retained_renewals {
            structural_arguments.insert(index, argument);
            capture_parameters.insert(index, declaration);
        }
        if !ingress_renewals.is_empty() {
            target = self.stage_released_ingresses(
                identities,
                position,
                target,
                &mut arguments,
                &mut structural_arguments,
                &capture_parameters,
                &edge_values,
                &ingress_renewals,
                &mut erased_arguments,
                &mut erased_proof_arguments,
                current_rank,
                arriving_rank,
                arriving_comparison,
                arrival_decided,
                edge_blocks,
                next_value,
                next_block,
                next_edge,
            )?;
            // Publish only after the final path and staging edge actually exist.
            // These IDs belong to the retained path; no selector is reevaluated.
            for renewal in &ingress_renewals {
                operations
                    .source_checked_obligations
                    .extend(renewal.preserved_bounds.iter().copied());
            }
            // The private edge preserves its source rank. Only the final
            // post-cleanup edge names the checked receiver and its arrival.
            arriving_rank = current_rank;
            arriving_comparison = terminal_psi::TerminalNaturalRankComparison::Preserving;
            arrival_decided = false;
        }
        // Payloads begin after selection. Actual edge work owns its block,
        // including any operand's private entry, joins and remapped parameters.
        // Pure forwarding alone may remain in the source block.
        if case_edge
            || !established.is_empty()
            || !edge_evaluation.blocks.is_empty()
            || operations.len() > operation_start
        {
            let backedge = edge_id(allocate_dense(next_edge)?);
            let selection_edge = edge_id(allocate_dense(next_edge)?);
            self.arrival_edges.entry(target).or_default().push(backedge);
            if arrival_decided {
                self.decided_arrival_edges.insert(backedge);
            }
            if let Some(rank) = current_rank {
                let source_rank = source_rank.ok_or(LoweringError::Unsupported(
                    "cleanup rank transport lost its actual source observation",
                ))?;
                self.block_ranks.insert(staged, source_rank);
                self.block_ranks.insert(edge_evaluation.current, rank);
                self.block_ranks.extend(
                    edge_evaluation
                        .blocks
                        .iter()
                        .map(|block| (block.id, source_rank)),
                );
                self.preserve_edges(
                    edge_evaluation
                        .blocks
                        .iter()
                        .flat_map(|block| block.terminator.edges()),
                    source_rank,
                );
                self.rank_edges.insert(
                    selection_edge,
                    (
                        source_rank,
                        terminal_psi::TerminalNaturalRankComparison::Preserving,
                    ),
                );
                if let Some(after) = arriving_rank {
                    self.rank_edges
                        .insert(backedge, (after, arriving_comparison));
                }
            }
            edge_blocks.extend(edge_evaluation.blocks);
            edge_blocks.push(Block {
                id: edge_evaluation.current,
                parameters: edge_evaluation.parameters,
                // The forwarding block redeclares the emitting state's
                // erased roster so forwarded proof terms stay in scope.
                erased_scalar_formals: self.state_erased[position].clone(),
                erased_proof_formals:
                    crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                        identities,
                        self.checked,
                        &self.state_erased_proof[position],
                    ),
                structural_parameters: edge_evaluation.block_structural_parameters,
                operations: operations[edge_evaluation.operation_start..].to_vec(),
                terminator: Terminator::Jump {
                    edge: backedge,
                    target,
                    arguments,
                    structural_arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    cleanup_actions,
                },
            });
            Ok(SuccessorEdge {
                edge: selection_edge,
                target: staged,
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: (0..self.state_erased_proof[position].len())
                    .map(|position| semantic_vocabulary::ProofTerm::Formal {
                        position: u32::try_from(position)
                            .expect("erased-proof roster positions fit u32"),
                    })
                    .collect(),
                structural_arguments: Vec::new(),
                cleanup_actions: Vec::new(),
            })
        } else {
            let successor_edge = edge_id(allocate_dense(next_edge)?);
            self.arrival_edges
                .entry(target)
                .or_default()
                .push(successor_edge);
            if arrival_decided {
                self.decided_arrival_edges.insert(successor_edge);
            }
            if let Some(after) = arriving_rank {
                self.rank_edges
                    .insert(successor_edge, (after, arriving_comparison));
            }
            Ok(SuccessorEdge {
                edge: successor_edge,
                target,
                arguments,
                structural_arguments,
                erased_arguments,
                erased_proof_arguments,
                cleanup_actions,
            })
        }
    }
}

/// The emitting state's facts every successor edge reads, fixed once the
/// state's prefix, rank and dispatch are emitted.
#[derive(Clone, Copy)]
pub(super) struct SuccessorFrame<'f> {
    pub(super) position: usize,
    pub(super) state_parameters: &'f [StructuralParameterDeclaration],
    pub(super) bindings: &'f ScalarBindings,
    pub(super) current_rank: Option<ValueId>,
    pub(super) inherited_lengths: &'f CollectionLengthObservations,
}

/// The state's emission in flight that each successor edge extends.
pub(super) struct EmittedState<'e> {
    pub(super) operations: &'e mut OperationBuffer,
    pub(super) evaluation: &'e mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    pub(super) next_value: &'e mut u64,
    pub(super) next_block: &'e mut u64,
    pub(super) next_edge: &'e mut u64,
    pub(super) edge_blocks: &'e mut Vec<Block>,
}
