//! One successor edge of a composed-control state: the owners that die on
//! it, the staged evaluation of its scalar and structural arguments, its
//! erased actuals, and the rank it arrives with.

use super::super::super::super::super::{
    StructuralAccess, StructuralArgument, StructuralFieldId, StructuralParameterDeclaration,
    SuccessorEdge, ValueId, block_id,
};
use super::super::super::super::{
    Block, CheckedScalarExpression, CheckedScalarExpressionRole, Operation, OperationKind,
    OperationResult, PlaceId, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralPlaceKind, Terminator, ValueDeclaration, allocate_dense,
    direct_expression_contains_short_circuit, edge_id, emit_direct_expression, place_id,
    terminal_scalar_type, unsupported, validate_direct_parameter_types,
};
use super::super::super::LoweringError;
use super::super::{
    CheckedStructuralControlSuccessorPlan, case_emission, case_leaf_copy, ranking, result_custody,
    scalars, subslices,
};
use super::StateGraphEmission;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::expression_preparation::bindings::ScalarBindings;
use crate::expression_preparation::bindings::structural_fields::{
    EstablishedCasePayload, NestedEstablishedCasePayload,
};
use crate::terminal_identities::lookup_domain_id_for_carrier;
use crate::terminal_identities::value_id;
use semantic_vocabulary::ScalarType;

impl StateGraphEmission<'_, '_> {
    /// Lower one edge leaving the state at `frame.position`: dispose of the
    /// owners its target does not receive, evaluate its arguments (in a
    /// private staging block when a case edge or established payload needs
    /// them after selection), and record the target's arrival and rank.
    pub(super) fn successor_edge(
        &mut self,
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
            rank_ceiling,
            case_subject,
            guarded,
            inherited_lengths,
            inherited_field_lengths,
            consumed_case_subjects,
        } = *frame;
        let state = &plan.states[position];
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
        // Every edge retains its local remainder until selected operands
        // finish. A case edge has already consumed only its subject.
        let mut trivial_affine_discards = if evaluation.selection_cleanups.is_empty() {
            result_custody::local_discards(
                checked,
                owner,
                self.admitted.authored[position].1,
                state,
                Some(edge),
            )?
            .into_iter()
            .map(|ordinal| {
                let result = case_emission::result(state, ordinal, operations)?;
                Ok(evaluation.current_structural_place(result.place()))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?
        } else {
            // An owned selection's residual and transported parameters die
            // or transfer per edge; the receipt's cleanup correspondence
            // substitutes each source's positional row exactly.
            result_custody::selection_edge_discards(
                checked,
                owner,
                self.admitted.authored[position].1,
                state,
                edge,
                operations,
                evaluation,
            )?
        };
        // Owned parameters the target does not receive die on this edge,
        // at whatever place their value occupies when control leaves, in
        // reverse declaration order.
        let mut dying = edge.trivial_affine_discard_parameter_positions.clone();
        if let Some((_, subject)) = consumed_case_subjects
            .iter()
            .find(|(ordinal, _)| *ordinal == edge.statement_ordinal)
            && !dying.contains(subject)
            && evaluation
                .structural_parameters
                .iter()
                .any(|(position, parameter)| {
                    position == subject && parameter.access == StructuralAccess::Owned
                })
        {
            dying.push(*subject);
            dying.sort_by_key(|position| std::cmp::Reverse(*position));
        }
        for position in &dying {
            let (_, parameter) = evaluation
                .structural_parameters
                .iter()
                .find(|(source_position, _)| source_position == position)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph edge discards a parameter its state does not declare",
                ))?;
            trivial_affine_discards.push(evaluation.current_structural_place(parameter.place));
        }
        if case_edge {
            let consumed = case_subject.ok_or(LoweringError::Unsupported(
                "Unit graph case edge lost its consumed subject",
            ))?;
            trivial_affine_discards.retain(|place| *place != consumed);
        }
        operations.byte_lengths = inherited_lengths.to_vec();
        operations.field_byte_lengths = inherited_field_lengths.to_vec();
        let target = plan
            .states
            .iter()
            .position(|state| state.state == edge.target_state)
            .ok_or(LoweringError::Unsupported(
                "Unit graph target disappeared during emission",
            ))?;
        // Established payloads are parameters of a guard dispatch block,
        // so the edge's arguments must be evaluated after selection. Every
        // transfer that emits its own operation on the edge -- a case-payload
        // or projected copy, an established value, a literal's octets --
        // needs that staged block to hold it.
        let stage = case_edge || !established.is_empty() || edge.scalar_arguments.iter().any(|argument| matches!(
            argument.source, checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression
        )) || edge.transfers.iter().any(|transfer| matches!(
            transfer.source, checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload { .. }
                | checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue { .. }
                | checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedCopy { .. }
                | checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceLiteral { .. }
        )) || guarded
                && ((current_rank.is_some() && ranking::has_rank(plan, &plan.states[target])) || edge.transfers.iter().any(|transfer| matches!(
                    transfer.source, checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice { .. }
                        | checked_trees::CheckedStructuralControlTransferSourcePlan::ElementViewSubslice { .. }
                )));
        let operation_start = operations.len();
        let staged = if stage {
            block_id(allocate_dense(next_block)?)
        } else {
            evaluation.current
        };
        let mut edge_evaluation = evaluation.branch(staged, operation_start);
        let mut edge_values = values.to_vec();
        // A case edge's payloads are the staged block's formals; they join the
        // edge namespace ahead of every later read so `established` positions
        // name this vector's tail slots directly.
        for (_, value) in payload_values {
            edge_values.push(*value);
        }
        let mut nested_rows = Vec::new();
        if stage {
            // A read below a case payload's record member in this edge's
            // scalar arguments mints the member leaf copy and its scalar
            // field read in the staged block, which the selecting case
            // edge dominates, then the read binds at the value's
            // namespace position.
            for transfer in &edge.scalar_arguments {
                if !matches!(
                    transfer.source,
                    checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression
                ) {
                    continue;
                }
                let argument = scalars::successor_value(checked, state, edge, transfer)?;
                let Some(expression) = argument.as_pure() else {
                    continue;
                };
                let mut reads = Vec::new();
                scalars::nested_case_payload_reads(expression, &mut reads);
                for (parameter_position, path) in reads {
                    let Some(payload) = crate::expression_preparation::bindings::structural_fields::plan_case_payload_leaf(
                        &edge_evaluation.structural_fields,
                        parameter_position,
                        path,
                    ) else {
                        continue;
                    };
                    if nested_rows
                        .iter()
                        .any(|row: &NestedEstablishedCasePayload| {
                            row.source == payload.source
                                && row.case == payload.case
                                && row.member == payload.member
                                && row.path == payload.path
                                && row.field == payload.field
                        })
                    {
                        continue;
                    }
                    let value = ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(allocate_dense(next_value)?),
                        scalar_type: payload.scalar_type,
                    };
                    let position = edge_values.len();
                    edge_values.push(value);
                    let source = edge_evaluation.current_structural_place(payload.source);
                    let destination = place_id(allocate_dense(&mut self.catalogs.next_place)?);
                    let producer = operations.allocate();
                    operations.push(Operation {
                        static_reach_binding: None,
                        suspension_crossing: None,
                        id: producer,
                        result: OperationResult::Structural(
                            terminal_psi::StructuralOperationResult {
                                qualification_establishments: Vec::new(),
                                place: destination,
                                structural_type: payload.member_type,
                                multiplicity: StructuralMultiplicity::Unrestricted,
                                qualifications: Vec::new(),
                                projected_qualifications: Vec::new(),
                                claims: Vec::new(),
                            },
                        ),
                        kind: OperationKind::StructuralCaseLeafCopy {
                            source,
                            path: vec![
                                semantic_vocabulary::CanonicalStructuralPathSegment::Case(
                                    payload.case,
                                ),
                                semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                                    payload.member,
                                ),
                            ],
                        },
                    });
                    self.structural_places.push(StructuralPlaceDeclaration {
                        id: destination,
                        kind: StructuralPlaceKind::OperationResult {
                            producer,
                            structural_type: payload.member_type,
                        },
                    });
                    let leaf_producer = operations.allocate();
                    operations.push(Operation {
                        static_reach_binding: None,
                        suspension_crossing: None,
                        id: leaf_producer,
                        result: OperationResult::Scalar(value),
                        kind: match payload.scalar_type {
                            ScalarType::Boolean => OperationKind::BooleanStructuralField {
                                source: destination,
                                path: payload.path.clone(),
                                field: payload.field,
                            },
                            _ => OperationKind::IntegerStructuralField {
                                source: destination,
                                path: payload.path.clone(),
                                field: payload.field,
                            },
                        },
                    });
                    nested_rows.push(NestedEstablishedCasePayload {
                        source: payload.source,
                        case: payload.case,
                        member: payload.member,
                        path: payload.path,
                        field: payload.field,
                        position,
                    });
                }
            }
        }
        crate::expression_preparation::bindings::structural_fields::establish_case_payloads(
            &mut edge_evaluation.structural_fields,
            established,
            &nested_rows,
        );
        edge_evaluation.parameters = payload_values.iter().map(|(_, value)| *value).collect();
        let mut arguments = Vec::new();
        let mut structural_arguments = Vec::new();
        let target_state = &plan.states[target];
        // Signature rows carry authored parameter positions, which go sparse
        // the moment a slot contributes no runtime operand: a borrowed `self`
        // the graph erased, or a proof-only formal the erased lane emits
        // below. Iterate the target's authored parameter list so every
        // retained row's position is still visited.
        let (_, authored_target) = crate::expression_preparation::source_custody::authored_state(
            checked,
            target_state.state,
        )?;
        let authored_parameters = checked.state_parameters(authored_target);
        for argument_position in 0..authored_parameters.len() {
            let runtime_row = target_state
                .structural_parameters
                .iter()
                .any(|parameter| parameter.position as usize == argument_position)
                || target_state
                    .scalar_parameters
                    .iter()
                    .any(|parameter| parameter.source_position as usize == argument_position);
            if !runtime_row {
                // The proof-only formals emit through the erased lane below;
                // a `self` slot no signature row claims is a receiver the
                // graph erased and owes no operand.
                let erased_row = target_state
                    .erased_scalar_parameters
                    .iter()
                    .any(|parameter| parameter.source_position as usize == argument_position)
                    || target_state
                        .erased_proof_parameters
                        .iter()
                        .any(|parameter| parameter.source_position as usize == argument_position);
                if erased_row || authored_parameters[argument_position].is_self {
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
                if target_parameter.is_self {
                    // A carried `self` names its source parameter's place;
                    // a self-less source forwards the machine's ambient
                    // receiver — the entry place itself in either shape.
                    let source_place = match transfer.source {
                        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                            index,
                        } => state_parameters
                            .get(index as usize)
                            .map(|parameter| parameter.place),
                        checked_trees::CheckedStructuralControlTransferSourcePlan::MachineReceiver => {
                            self.parameters
                                .iter()
                                .find(|parameter| parameter.is_self)
                                .map(|parameter| parameter.place)
                        }
                        _ => return unsupported("Unit graph receiver cannot be rebound"),
                    };
                    if source_place
                        != self
                            .parameters
                            .iter()
                            .find(|parameter| parameter.is_self)
                            .map(|parameter| parameter.place)
                    {
                        return unsupported("Unit graph receiver lost original invocation place");
                    }
                    continue;
                }
                if let Some(root) = self
                    .admitted
                    .claim_transport
                    .aliased
                    .get(target)
                    .and_then(|aliased| aliased.get(&(target_index as u32)))
                {
                    // A claim or a forwarded whole receiver carried across
                    // the edge keeps the entry parameter's place; the block
                    // does not rebind it, so the transfer row must still
                    // name that exact parameter and no edge argument is
                    // emitted. The entry place is read from the machine
                    // parameters directly: state_views drains as each target
                    // block seals, so a backward edge sees an emptied roster
                    // there. The ambient receiver is that entry place itself.
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
                        checked_trees::CheckedStructuralControlTransferSourcePlan::MachineReceiver
                            if self
                                .parameters
                                .get(*root as usize)
                                .is_some_and(|parameter| parameter.is_self) =>
                        {
                            entry_place
                        }
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
                                checked,
                                plan.machine,
                                state,
                                &selected_operations,
                                self.catalogs,
                                state_parameters,
                                &self.claims.source_claims,
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
                                    .map(|binding| binding.place())
                            });
                            operations.structural_values = state_bindings;
                            edge_evaluation.current_structural_place(emitted?)
                        }
                        checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } => edge_evaluation.current_structural_place(case_emission::result(state, binding_ordinal, operations)?.place()),
                        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
                            edge_evaluation.current_structural_place(state_parameters.get(index as usize).ok_or(
                                LoweringError::Unsupported("Unit graph transfer source descriptor disappeared"),
                            )?.place)
                        }
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice { .. }
                        | checked_trees::CheckedStructuralControlTransferSourcePlan::ElementViewSubslice { .. } => {
                            let destination = place_id(allocate_dense(&mut self.catalogs.next_place)?);
                            let emitted = subslices::emit(
                                checked, state, edge.statement_ordinal, target_parameter.position, &transfer.source,
                                state_parameters, &edge_evaluation, destination, bindings, &edge_values,
                                next_value, operations, self.catalogs, &mut self.structural_places,
                                &target_parameter.type_identity, target_parameter.access,
                            )?;
                            self.structural_places.push(emitted);
                            destination
                        }
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                            ref subject, ref carrier_identity,
                        } => {
                            let root = match subject.source {
                                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                                    edge_evaluation.current_structural_place(state_parameters.get(parameter_index as usize).ok_or(
                                        LoweringError::Unsupported("Unit graph loan subject descriptor disappeared"),
                                    )?.place)
                                }
                                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                                    edge_evaluation.current_structural_place(
                                        case_emission::result(state, binding_ordinal, operations)?.place(),
                                    )
                                }
                                _ => {
                                    return unsupported(
                                        "Unit graph loan subject source unsupported",
                                    );
                                }
                            };
                            let mut path = subject.path.iter().map(|segment| match segment {
                                checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => Some(
                                    terminal_psi::StructuralPathSegment::Field(identity.clone())),
                                checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => Some(
                                    terminal_psi::StructuralPathSegment::FixedIndex(*index)),
                                _ => None,
                            }).collect::<Option<Vec<_>>>().ok_or(
                                LoweringError::Unsupported("Unit graph loan path segment unsupported"),
                            )?;
                            if path.is_empty() {
                                // A whole-place loan forwards the rooted
                                // place itself — the target borrows exactly
                                // the storage the borrow expression names.
                                root
                            } else if matches!(
                                subject.access,
                                checked_trees::CheckedStructuralAccess::SharedBorrow
                                    | checked_trees::CheckedStructuralAccess::MutableBorrow
                            ) {
                                // A projected loan joins the rooted
                                // place and its projection path directly as
                                // the continuation argument: the verifier
                                // resolves the projected leaf against the
                                // target's declared referent — no reference
                                // carrier forms.
                                argument_path = path;
                                root
                            } else {
                                // A mutable projected loan mints a fresh
                                // reference carrier over the subject's
                                // rooted place: an EstablishReference op
                                // captures the path below the root and the
                                // argument names the carrier, never the
                                // rooted place itself. The carrier's source
                                // path ends in `Referent` — the terminal
                                // verifier resolves the projected leaf
                                // through that dereference marker.
                                path.push(terminal_psi::StructuralPathSegment::Referent);
                                let declaration = crate::unit::attached_unit::reference_results::emit_carrier(
                                    crate::terminal_identities::lookup_type_id(
                                        &self.catalogs.type_ids,
                                        carrier_identity,
                                    )?,
                                    StructuralArgument {
                                        place: root,
                                        path,
                                        access: match subject.access {
                                            checked_trees::CheckedStructuralAccess::MutableBorrow => {
                                                StructuralAccess::MutableBorrow
                                            }
                                            _ => StructuralAccess::SharedBorrow,
                                        },
                                    },
                                    &mut self.catalogs.next_place,
                                    operations,
                                )?;
                                self.structural_places.push(declaration);
                                declaration.id
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
                                &self.catalogs.structural_types, source, root_type, &subject.path,
                                &subject.type_identity, case_identity, field_identity, path,
                                target_parameter, destination, operations,
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
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedCopy {
                            ref subject,
                        } => {
                            // A copyable leaf has no custody root of its
                            // own: the edge mints one StructuralLeafCopy
                            // reading the projected subtree off its live
                            // parameter root into fresh owned storage the
                            // target parameter takes.
                            let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                parameter_index,
                            } = subject.source
                            else {
                                return unsupported(
                                    "Unit graph projected copy subject unsupported",
                                );
                            };
                            let source = edge_evaluation.current_structural_place(
                                state_parameters.get(parameter_index as usize).ok_or(
                                    LoweringError::Unsupported(
                                        "Unit graph projected subject descriptor disappeared",
                                    ),
                                )?.place,
                            );
                            let path = subject.path.iter().map(|segment| match segment {
                                checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => Some(
                                    terminal_psi::StructuralPathSegment::Field(identity.clone())),
                                checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => Some(
                                    terminal_psi::StructuralPathSegment::FixedIndex(*index)),
                                _ => None,
                            }).collect::<Option<Vec<_>>>().ok_or(
                                LoweringError::Unsupported("Unit graph projected path segment unsupported"),
                            )?;
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
                        checked_trees::CheckedStructuralControlTransferSourcePlan::MachineReceiver => {
                            return unsupported(
                                "Unit graph machine receiver has no argument place",
                            );
                        }
                    };
                // A member of a later state's loan chain (`loan_chains`)
                // either keeps an exclusive view of its one lender's home or
                // receives the moved storage; neither retains an entry home.
                let chain_arrival = self.loan_chains.arrival(target, target_index as u32);
                if chain_arrival.is_some()
                    && let checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                        subject:
                            checked_trees::CheckedUnitStructuralArgumentPlan {
                                source:
                                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                        binding_ordinal,
                                    },
                                ..
                            },
                        ..
                    }
                    | checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult {
                        binding_ordinal,
                    } = transfer.source
                {
                    let result = case_emission::result(state, binding_ordinal, operations)?;
                    if !result.claims().is_empty() || !argument_path.is_empty() {
                        return unsupported(
                            "Unit graph state loan chain home is not plain whole storage",
                        );
                    }
                    self.loan_chains.lend(target, target_index as u32, place)?;
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
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan { ref subject, .. } => match subject.source {
                            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => Some(binding_ordinal),
                            _ => None,
                        },
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
                                    self.admitted.authored[position].1,
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
                        // A whole shared loan of unrestricted storage, such
                        // as a byte literal local, has no death to place and
                        // records no home, so any state may lend it.
                        let needs_home = target_parameter.access
                            == checked_trees::CheckedStructuralAccess::MutableBorrow
                            || result.multiplicity() == StructuralMultiplicity::Affine
                            || backing.is_some()
                            || !matches!(
                                transfer.source,
                                checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { .. }
                            );
                        if needs_home && (position != 0 || self.entry_reentered) {
                            return unsupported(
                                "Unit graph local loan requires an activation entry home",
                            );
                        }
                        let homes = (result.multiplicity() == StructuralMultiplicity::Affine)
                            .then_some((binding_ordinal, place))
                            .into_iter()
                            .chain(backing.map(|(backing, home)| {
                                (
                                    backing,
                                    edge_evaluation.current_structural_place(home.place()),
                                )
                            }));
                        for (binding, home) in homes {
                            if let Some(previous) = self.loaned_local_homes.insert(binding, home)
                                && previous != home
                            {
                                return unsupported(
                                    "Unit graph local loan changed its backing home",
                                );
                            }
                        }
                    }
                }
                structural_arguments.push(StructuralArgument {
                    place,
                    path: argument_path,
                    // Not `terminal_access`: a write-only target parameter
                    // receives a shared argument on this edge.
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
                        _ => StructuralAccess::SharedBorrow,
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
                    let retained = state.scalar_parameters.get(index as usize).ok_or(
                        LoweringError::Unsupported("Unit graph successor parameter is missing"),
                    )?;
                    let (_, authored_state) =
                        crate::expression_preparation::source_custody::authored_state(
                            checked,
                            state.state,
                        )?;
                    let parameter = checked
                        .state_parameters(authored_state)
                        .get(retained.source_position as usize)
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph successor source parameter is missing",
                        ))?;
                    let expression = if parameter.is_mutable {
                        CheckedScalarExpression::StorageRead {
                            symbol: parameter.symbol,
                            primitive_type: transfer.primitive_type,
                        }
                    } else {
                        CheckedScalarExpression::Parameter {
                            position: index as usize,
                            primitive_type: transfer.primitive_type,
                        }
                    };
                    bindings.expression(&expression)?
                }
                checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression => {
                    let value = scalars::successor_value(checked, state, edge, transfer)?;
                    let mut calls = self.catalogs.scalar_calls.emission_context();
                    let value = edge_evaluation.source_value(
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
        // Proof-only actuals never evaluate: each checked erased row
        // lowers to a term over the emitting state's scalar values and
        // erased formals, in the target's erased-roster order.
        if edge.erased_arguments.len() != self.state_erased[target].len() {
            return unsupported("Unit graph successor erased arity drifted");
        }
        let erased_arguments = edge
            .erased_arguments
            .iter()
            .enumerate()
            .map(|(erased_index, argument)| {
                if argument.target_scalar_parameter_index as usize != erased_index {
                    return unsupported("Unit graph successor erased order drifted");
                }
                let expression = bindings.expression_at(
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
        if edge.erased_proof_arguments.len() != self.state_erased_proof[target].len() {
            return unsupported("Unit graph successor erased proof arity drifted");
        }
        let erased_proof_arguments = edge
            .erased_proof_arguments
            .iter()
            .map(|term| {
                crate::scalar_graph::scalar_contracts::checked_proof_term(
                    checked,
                    term,
                    &self.state_erased_proof[position],
                )
                .and_then(|term| {
                    crate::scalar_graph::scalar_contracts::lowered_proof_term(
                        &term,
                        &edge_values,
                        &self.state_erased[position],
                    )
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let arriving_rank = if current_rank.is_some() {
            // The target's measure over this edge's actual arguments,
            // which the verifier substitutes for the target's own rank.
            if let Some(rank) = ranking::scalar_rank(plan, target_state) {
                Some(
                    ranking::emit_scalar_rank(
                        rank,
                        &arguments,
                        rank_ceiling,
                        next_value,
                        operations,
                    )?
                    .rank,
                )
            } else if let Some(dense) = ranking::parameter_position(plan, target_state) {
                let source = if let Some(root) = self
                    .admitted
                    .claim_transport
                    .aliased
                    .get(target)
                    .and_then(|aliased| aliased.get(&(dense as u32)))
                {
                    // An aliased rank subject is not an edge argument;
                    // its length reads the retained entry place — taken
                    // from the machine parameters since the target's view
                    // roster drains when its block seals.
                    self.parameters
                        .get(*root as usize)
                        .map(|parameter| parameter.place)
                } else {
                    ranking::view_argument_position(
                        plan,
                        target_state,
                        &self.admitted.claim_transport.aliased[target],
                    )
                    .map(|parameter_position| structural_arguments[parameter_position].place)
                };
                source
                    .map(|source| {
                        ranking::emit_view_rank(
                            crate::terminal_identities::lookup_type_id(
                                &self.catalogs.type_ids,
                                &target_state.structural_parameters[dense].type_identity,
                            )?,
                            source,
                            &self.catalogs.structural_types,
                            next_value,
                            operations,
                        )
                    })
                    .transpose()?
            } else {
                None
            }
        } else {
            None
        };
        let target = self.state_ids[target];
        if stage {
            let backedge = edge_id(allocate_dense(next_edge)?);
            let selection_edge = edge_id(allocate_dense(next_edge)?);
            self.arrival_edges.entry(target).or_default().push(backedge);
            if let Some(rank) = current_rank {
                self.block_ranks.insert(staged, rank);
                self.block_ranks.insert(edge_evaluation.current, rank);
                self.block_ranks
                    .extend(edge_evaluation.blocks.iter().map(|block| (block.id, rank)));
                self.rank_edges
                    .extend(edge_evaluation.blocks.iter().flat_map(|block| {
                        block.terminator.edges().map(|edge| {
                            (
                                edge,
                                (
                                    rank,
                                    terminal_psi::TerminalNaturalRankComparison::Preserving,
                                ),
                            )
                        })
                    }));
                self.rank_edges.insert(
                    selection_edge,
                    (
                        rank,
                        terminal_psi::TerminalNaturalRankComparison::Preserving,
                    ),
                );
                if let Some(after) = arriving_rank {
                    self.rank_edges.insert(
                        backedge,
                        (after, terminal_psi::TerminalNaturalRankComparison::Strict),
                    );
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
                    crate::scalar_graph::scalar_contracts::erased_proof_formal_declarations(
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
                    trivial_affine_discards,
                    residual_affine_discards: Vec::new(),
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
                trivial_affine_discards: Vec::new(),
            })
        } else {
            let successor_edge = edge_id(allocate_dense(next_edge)?);
            self.arrival_edges
                .entry(target)
                .or_default()
                .push(successor_edge);
            if let Some(after) = arriving_rank {
                self.rank_edges.insert(
                    successor_edge,
                    (after, terminal_psi::TerminalNaturalRankComparison::Strict),
                );
            }
            Ok(SuccessorEdge {
                edge: successor_edge,
                target,
                arguments,
                structural_arguments,
                erased_arguments,
                erased_proof_arguments,
                trivial_affine_discards,
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
    pub(super) rank_ceiling: Option<ValueId>,
    /// The closed-sum subject a case edge has already consumed.
    pub(super) case_subject: Option<PlaceId>,
    /// Whether the state decides between successors on a scalar guard.
    pub(super) guarded: bool,
    pub(super) inherited_lengths: &'f [(PlaceId, ValueId)],
    pub(super) inherited_field_lengths: &'f [(
        PlaceId,
        Vec<terminal_psi::StructuralPathSegment>,
        StructuralFieldId,
        ValueId,
    )],
    pub(super) consumed_case_subjects: &'f [(u32, u32)],
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
