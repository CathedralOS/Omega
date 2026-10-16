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
            current_rank,
            inherited_lengths,
        } = *frame;
        let state = &plan.states[position];
        let arrival_decided = checked.facts.call_requires_decided(
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
                let mut completed_binding = None;
                let mut captured_backing = None;
                let place = match transfer.source {
                        checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue { ref operands, ref establishment, ref presentation } => {
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
                                    .map(|binding| binding.into_owned())
                                    .and_then(|binding| {
                                        let place = binding.place();
                                        let suffix = crate::expression_preparation::bindings::structural_paths::prepare_argument_path(
                                            identities, &presentation.path, binding.structural_type(),
                                            &self.catalogs.structural_types,
                                        )?;
                                        // A completed move can name a subtree
                                        // of an existing owner. Its path is
                                        // part of the value, not an optional
                                        // hint: dropping it transfers the
                                        // containing record instead.
                                        let prepared = binding.path().iter().cloned()
                                            .map(crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static)
                                            .chain(suffix).collect::<Vec<_>>();
                                        let mut calls = self.catalogs.scalar_calls.emission_context();
                                        argument_path = crate::expression_preparation::bindings::structural_paths::emit_captured_argument_path(
                                            &prepared, edge_evaluation.current_structural_place(place),
                                            Some(&edge_values), &mut std::iter::empty(), Some(&mut calls),
                                            Some((next_value, operations)),
                                        )?;
                                        self.catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
                                        completed_binding = Some(binding);
                                        Ok(place)
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
                            carrier_type_identity,
                        } => {
                            let storage = edge_evaluation.cleanup_storage(
                                &subject.source, state_parameters, |binding| {
                                    let produced = case_emission::result(state, binding, operations)?;
                                    Ok(produced.as_ref().into())
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
                            let suffix =
                                crate::expression_preparation::bindings::structural_paths::prepare_argument_path(
                                    identities,
                                    &subject.path,
                                    storage.selected_type,
                                    &self.catalogs.structural_types,
                                )?;
                            let root = storage.place;
                            // The selected edge owns the operand frame. Keep
                            // its actual storage prefix before emitting view
                            // extents and selector obligations, just as for a
                            // reference formation in an ordinary operation.
                            let prepared = storage.path.into_iter()
                                .map(crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static)
                                .chain(suffix).collect::<Vec<_>>();
                            let path = crate::expression_preparation::bindings::structural_paths::emit_captured_argument_path(
                                &prepared, root, Some(&edge_values), &mut selectors.into_iter(),
                                Some(&mut calls), Some((next_value, operations)),
                            )?;
                            self.catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
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
                                        backing_type: validation::expression_type_reference_in_state(
                                            &checked.typed,
                                            state.state,
                                            edge.statement_ordinal as usize,
                                            validation::reference_formation_source(&checked.typed, formation_expression)
                                                .map_or(formation_expression, |formation| formation.source),
                                        ).ok_or(LoweringError::Unsupported("state loan lost its supplying type"))?,
                                        qualification_decisions: Default::default(),
                                        domain_ids: &self.catalogs.domain_ids,
                                    },
                                    carrier_type, source, crate::unit::attached_unit::reference_results::PublishedReferenceStorage::Declared, &self.catalogs.structural_types,
                                    edge_evaluation.structural_root_parameters(),
                                    place_id(allocate_dense(&mut self.catalogs.next_place)?), operations,
                                )?;
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
                            let storage = edge_evaluation.cleanup_storage(
                                &subject.source, state_parameters, |binding| {
                                    let produced = case_emission::result(state, binding, operations)?;
                                    Ok(produced.as_ref().into())
                                },
                            )?;
                            let source = storage.place;
                            let suffix = crate::expression_preparation::bindings::structural_paths::prepare_argument_path(
                                identities, &subject.path, storage.selected_type, &self.catalogs.structural_types,
                            )?;
                            let prepared = storage.path.into_iter()
                                .map(crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static)
                                .chain(suffix).collect::<Vec<_>>();
                            let mut calls = self.catalogs.scalar_calls.emission_context();
                            let path = crate::expression_preparation::bindings::structural_paths::emit_captured_argument_path(
                                &prepared, source, Some(&edge_values), &mut std::iter::empty(),
                                Some(&mut calls), Some((next_value, operations)),
                            )?;
                            self.catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
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
                    let loaned_binding = match &transfer.source {
                        checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } => Some(*binding_ordinal),
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan { subject, .. } => subject.source.current_result_binding(&state.operations),
                        // A subslice of an owned local lends that local's home,
                        // not a fresh array. The descriptor is the successor's
                        // parameter; the home stays with the activation.
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ViewSubslice(argument) => {
                            crate::unit::attached_unit::view_ranges::range(argument).and_then(|range| {
                                match range.root {
                                    checked_trees::CheckedStorageRoot::OwnedLocal { symbol } => {
                                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
                                            .current_result_binding(&state.operations)
                                    }
                                    _ => None,
                                }
                            })
                        }
                        _ => None,
                    };
                    if let Some(binding_ordinal) = loaned_binding {
                        let result = case_emission::result(state, binding_ordinal, operations)?;
                        // A reference or view lends the home of the local it was
                        // formed from; that home stays live with the loan
                        // and dies with it, exactly as a directly lent one.
                        let backing = super::super::result_custody::local_loan_backing(
                            checked,
                            state,
                            binding_ordinal,
                        )
                        .map(|backing| {
                            case_emission::result(state, backing, operations)
                                .map(|home| (backing, home))
                        })
                        .transpose()?
                        .filter(|(_, home)| home.multiplicity() == StructuralMultiplicity::Affine);
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
                        let subslice_view = matches!(
                            transfer.source,
                            checked_trees::CheckedStructuralControlTransferSourcePlan::ViewSubslice(
                                _
                            )
                        );
                        // A subslice's parameter place is the fresh descriptor.
                        // Both windows of one local share that local's home;
                        // recording each descriptor would look like two homes.
                        let lent_home = if subslice_view {
                            edge_evaluation.current_structural_place(result.place())
                        } else {
                            captured_backing
                                .as_ref()
                                .map_or(place, |(backing, _)| *backing)
                        };
                        // Unrestricted array storage has no affine death. A
                        // subslice formed in a later state still names that
                        // storage; the entry-home roster is only for homes
                        // that must be disposed once at return.
                        let unrestricted_subslice = subslice_view
                            && result.multiplicity() != StructuralMultiplicity::Affine
                            && backing.is_none();
                        if needs_home
                            && !unrestricted_subslice
                            && (position != 0 || self.entry_reentered)
                        {
                            return unsupported(
                                "Unit graph local loan requires an activation entry home",
                            );
                        }
                        let homes = (result.multiplicity() == StructuralMultiplicity::Affine)
                            .then_some((binding_ordinal, lent_home))
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
            self.catalogs.cleanup_control,
            state,
            edge.statement_ordinal as usize,
            false,
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
        // Proof-only actuals never evaluate: each checked erased row
        // lowers to a term over the emitting state's scalar values and
        // erased formals, in the target's erased-roster order.
        let erased_arguments = edge
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
        let erased_proof_arguments = edge
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
        // The arriving rank strictly decreases unless the rank judgment
        // proves this edge only preserves it.
        let arriving_comparison = if plan.preserving_rank_edges.iter().any(|weak| {
            weak.source_state == state.state
                && weak.statement_ordinal == edge.statement_ordinal
                && weak.target_state == edge.target_state
        }) {
            terminal_psi::TerminalNaturalRankComparison::Preserving
        } else {
            terminal_psi::TerminalNaturalRankComparison::Strict
        };
        let arriving_rank = if current_rank.is_some()
            && let Some(root) = ranking::observation(checked, plan, target_state)?
        {
            // Scalar and structural operands are already complete. Rejoin the
            // exact target telescope here, before projected transfer or cleanup.
            let mut scalar_slots = Vec::with_capacity(arguments.len());
            for (argument, formal) in arguments.iter().zip(&target_state.scalar_parameters) {
                scalar_slots.push((
                    edge_values.len(),
                    terminal_scalar_type(formal.primitive_type)?,
                ));
                edge_values.push(ValueDeclaration {
                    id: *argument,
                    scalar_type: terminal_scalar_type(formal.primitive_type)?,
                    qualifications: Default::default(),
                });
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
        let target = self.state_ids[target];
        // All operands and rank observations are already captured. Transfer
        // projected structural custody before selected cleanup can retire the
        // old home; the authored successor receives only whole captures.
        if cleanup_regions
            .iter()
            .any(|region| !region.conditions.is_empty() || region.selected_region.is_some())
        {
            edge_evaluation.capture_structural_arguments(
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
        }
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
                self.block_ranks.insert(staged, rank);
                self.block_ranks.insert(edge_evaluation.current, rank);
                self.block_ranks
                    .extend(edge_evaluation.blocks.iter().map(|block| (block.id, rank)));
                self.preserve_edges(
                    edge_evaluation
                        .blocks
                        .iter()
                        .flat_map(|block| block.terminator.edges()),
                    rank,
                );
                self.rank_edges.insert(
                    selection_edge,
                    (
                        rank,
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
