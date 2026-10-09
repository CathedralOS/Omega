//! Case terminators inspect exact sums. An owned subject is consumed and its
//! disposal obligation retained; a shared or mutable loan is read in place.
//! Captured locals retain their real descriptor result and cross its referee,
//! never re-evaluating the reference's formation or borrowing its owned custody.
//! A field or fixed-index subject observes a subtree of that same carrier;
//! it neither transfers the carrier nor turns a one-case sum into a guard.
//!
//! The dispatched sum is a parameter, an earlier immutable local, or a
//! computed call subject. A computed subject's call is planned once at the
//! first arm's statement; the parser's per-arm copies of it (each guard's
//! subject and each payload read's receiver) all read that one result.
use super::super::{
    CheckedClosedSumCaseOutcomePlan, CheckedClosedSumCasePlan, CheckedClosedSumPayloadTransferPlan,
    DataMember, PermissionAccess, PermissionClaimIdentity,
};
use super::{
    CheckFacts, CheckedComposedUnitControlTerminatorPlan, CheckedScalarExpressionRole,
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, Multiplicity, PermissionEventSource,
    ScalarCalleeClosure, ShapeCollector, Signature, TransitionExit, TransitionGuardNode,
    TransitionTargetNode, TypeReferenceNode, returns, terminal_calls,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::state_graph::{
    EdgeCalls, FusedStates, SuccessorEdge, successor_bindings,
};
use crate::semantic::calls::dispatch_subjects;

#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    fused: &FusedStates<'_>,
    state_index: usize,
    signatures: &[Signature],
    edge_calls: EdgeCalls<'_>,
    result: &checked_trees::CheckedControlResultPlan,
    next_scalar_binding: usize,
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
    transition_start: usize,
    trace: &LocalConstructionTrace,
) -> Option<CheckedComposedUnitControlTerminatorPlan> {
    let EdgeCalls {
        scalar_callees,
        entry_claims,
        local_results,
    } = edge_calls;
    let &(machine, state) = fused.get(state_index)?;
    let (structural, _) = &signatures[state_index];
    let statements = program.statement_table.statements(state.statement_nodes);
    let first = statements.get(transition_start)?.as_transition()?;
    let TransitionGuardNode::When(guard) = first.guard else {
        return None;
    };
    let (expression, _) = crate::proof::exact_outcome_case_test(program, guard)?;
    let subject_place = validation::contextual_canonical_place_from_expression(
        program,
        state.symbol,
        transition_start,
        expression,
    );
    let dispatch = match subject_place.as_ref() {
        Some(place) => {
            let _ = place.root.as_symbol()?;
            DispatchSubject::Place(place)
        }
        None => DispatchSubject::Computed(dispatch_subjects::evaluated_case_subject(
            program,
            statements,
            transition_start,
        )?),
    };
    let (mut subject, mut type_reference, expected_provenance) =
        if let DispatchSubject::Computed(subject_expression) = dispatch {
            trace.phase("state graph: terminator: case subject call");
            trace.statement(u32::try_from(transition_start).ok());
            let (result, type_reference) = subject_call(
                program,
                facts,
                scalar_callees,
                shapes,
                machine,
                state,
                structural,
                entry_claims,
                operations,
                transition_start,
                subject_expression,
                trace,
            )?;
            (
                CheckedUnitStructuralArgumentPlan::structural_result(
                    result.binding_ordinal,
                    result.type_identity,
                    CheckedStructuralAccess::Owned,
                ),
                type_reference,
                language_semantics::PermissionProvenance::Unknown,
            )
        } else if let Some((index, parameter)) = program
            .state_parameters(state)
            .iter()
            .enumerate()
            .find(|(_, parameter)| dispatch.is_symbol(parameter.symbol))
        {
            let (position, retained) = structural
                .iter()
                .enumerate()
                .find(|(_, retained)| retained.position as usize == index)?;
            // An owned subject is consumed by the dispatch. A shared or
            // mutable loan is only inspected: each case edge reads the
            // referent's tag and payloads in place and the loan stays the
            // caller's.
            let inspected = match retained.access {
                CheckedStructuralAccess::Owned => parameter.type_reference,
                checked_trees::checked_borrowed_access_pair_pattern!() => {
                    let TypeReferenceNode::Reference { referee, .. } = program
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                    else {
                        return None;
                    };
                    *referee
                }
                CheckedStructuralAccess::WriteOnlyBorrow => return None,
            };
            if !matches!(
                retained.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) || retained.multiplicity != program.type_multiplicity(parameter.type_reference)
                || !retained.qualifications.is_empty()
                || retained.primitive_referent.is_some()
            {
                return None;
            }
            (
                CheckedUnitStructuralArgumentPlan {
                    source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: u32::try_from(position).ok()?,
                    },
                    path: Vec::new(),
                    type_identity: retained.type_identity.clone(),
                    access: retained.access,
                },
                inspected,
                language_semantics::PermissionProvenance::Unknown,
            )
        } else {
            let mut matching = operations
                .iter()
                .filter_map(|operation| {
                    Some((operation, super::retained_structural_result(operation)?))
                })
                .filter_map(|(operation, result)| {
                    let local = statements
                        .get(result.statement_index as usize)?
                        .as_local_data()?;
                    (dispatch.is_symbol(local.symbol)
                        && !local.is_mutable
                        && result.statement_index < transition_start as u32)
                        .then_some((operation, result, local))
                });
            let (operation, result, local) = matching.next()?;
            if matching.next().is_some()
                || !matches!(
                    result.multiplicity,
                    Multiplicity::Affine | Multiplicity::Unrestricted
                )
            {
                return None;
            }
            let (subject, inspected) = if let Some(shell) =
                validation::reference_shell(program, local.type_reference)
            {
                // The descriptor's affine custody belongs to its producer,
                // not to the unrestricted shared-reference source type.
                if !matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishReference { .. }
                ) || result.multiplicity != Multiplicity::Affine
                    || shell.access == language_semantics::ReferenceAccess::WriteOnly
                {
                    return None;
                }
                shapes.add_type(shell.referee, &[], &[])?;
                (
                    CheckedUnitStructuralArgumentPlan {
                        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: result.binding_ordinal,
                        },
                        path: vec![checked_trees::CheckedUnitStructuralPathSegment::Referent],
                        type_identity: crate::execution::terminal_unit::types::base_type_identity(
                            program,
                            shell.referee,
                            &[],
                        )?,
                        access: CheckedStructuralAccess::SharedBorrow,
                    },
                    shell.referee,
                )
            } else {
                if result.multiplicity != program.type_multiplicity(local.type_reference) {
                    return None;
                }
                (
                    CheckedUnitStructuralArgumentPlan::structural_result(
                        result.binding_ordinal,
                        result.type_identity.clone(),
                        CheckedStructuralAccess::Owned,
                    ),
                    local.type_reference,
                )
            };
            (
                subject,
                inspected,
                language_semantics::PermissionProvenance::Established {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                    source: PermissionEventSource::Statement {
                        statement_index: result.statement_index as usize,
                    },
                },
            )
        };
    if let DispatchSubject::Place(place) = dispatch
        && !place.segments.is_empty()
    {
        let (projected, path) = crate::execution::terminal_unit::calls::projected_argument_path(
            program,
            state.symbol,
            transition_start,
            place,
        )?;
        subject.path.extend(path);
        subject.type_identity =
            crate::execution::terminal_unit::types::base_type_identity(program, projected, &[])?;
        // Dispatch observes a subtree, not an owned move of its enclosing
        // record. Ordinary selected-edge cleanup still owns that root.
        subject.access = CheckedStructuralAccess::SharedBorrow;
        type_reference = projected;
        shapes.add_type(projected, &[], &[])?;
    }
    if !validation::has_contents(
        program,
        type_reference,
        validation::ContentsRequirement::NumericOwned,
    ) && !validation::has_contents(
        program,
        type_reference,
        validation::ContentsRequirement::OwnedOrSharedView,
    ) {
        return None;
    }
    let symbol = &program
        .type_reference_table
        .type_reference(type_reference)
        .as_named_symbol()?;
    let data = program.data_definition_by_symbol(*symbol)?;
    let variants = program
        .data_members(data)
        .iter()
        .map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    // A case edge forwards only primitive payloads, each into a scalar
    // successor parameter (`payload_field` below). A structural payload such
    // as `IoResult::Error { kind: ErrorKind }` may still sit in a case whose
    // arm leaves it unread: the subject's plain owned contents, checked
    // above for the whole type graph, give that field no references, slices
    // or cleanup code, so the dispatch's one disposal of the whole subject
    // covers it exactly as it covers an unread primitive payload.
    // Each arm selects at least one case, except a final `_` that follows
    // complete coverage (see below).
    if variants.is_empty() || statements.len() - transition_start > variants.len() + 1 {
        return None;
    }
    let mut cases = Vec::new();
    let mut count = returns::next_result_ordinal(operations.as_slice())?;
    // Every copy of a computed subject the dispatch reads: the guards'
    // subjects and the payload reads' receivers.
    let mut subject_reads = Vec::new();
    for (index, statement) in statements[transition_start..].iter().enumerate() {
        let transition = statement.as_transition()?;
        let ordinal = u32::try_from(transition_start + index).ok()?;
        let selected_variants = match transition.guard {
            TransitionGuardNode::When(guard) => {
                let (expression, case_symbol) =
                    crate::proof::exact_outcome_case_test(program, guard)?;
                if !dispatch.names(program, state, ordinal as usize, expression) {
                    return None;
                }
                subject_reads.push(expression);
                vec![
                    variants
                        .iter()
                        .copied()
                        .find(|variant| variant.symbol == case_symbol)?,
                ]
            }
            TransitionGuardNode::Always if transition_start + index + 1 == statements.len() => {
                variants
                    .iter()
                    .copied()
                    .filter(|variant| {
                        let identity = variant.path_identity();
                        !cases
                            .iter()
                            .any(|case: &CheckedClosedSumCasePlan| case.case_identity == identity)
                    })
                    .collect()
            }
            _ => return None,
        };
        // A final `_` after the authored arms already cover every case is
        // admitted and never selected (first-match order,
        // wiki/spec/language/patterns.md "Coverage and domain tests"): it
        // contributes no case edge.
        if selected_variants.is_empty() {
            if matches!(transition.guard, TransitionGuardNode::Always) {
                continue;
            }
            return None;
        }
        for variant in selected_variants {
            let identity = variant.path_identity();
            if cases
                .iter()
                .any(|case: &CheckedClosedSumCasePlan| case.case_identity == identity)
            {
                return None;
            }
            let (outcome, payloads) = match program
                .statement_table
                .transition_target(transition.target)
            {
                // An authored `(expression)` arm closes this case's edge
                // with the value it returns — the same producer plan a
                // `ConditionalReturn` arm carries. Case payloads stay out
                // of scope here: a return arm has no argument list the
                // checker could attribute a payload read through.
                TransitionTargetNode::Value(expression)
                    if transition.exit == TransitionExit::Ordinary
                        && !transition.continuation.is_valid() =>
                {
                    trace.phase("state graph: terminator: case successors: return arm value");
                    trace.statement(Some(ordinal));
                    let arm =
                        if let checked_trees::CheckedControlResultPlan::Scalar { primitive_type } =
                            result
                        {
                            // The arm alone evaluates the scalar checking kept
                            // under the `Return` role at its transition.
                            let role = CheckedScalarExpressionRole::Return;
                            let retained = facts
                                .values
                                .scalar_expressions
                                .expression_at(state.symbol, ordinal, role)
                                .is_some()
                                || facts
                                    .values
                                    .scalar_computations
                                    .root_at(state.symbol, ordinal, role)
                                    .is_some_and(|root| root.machine == machine.symbol);
                            if !retained {
                                return None;
                            }
                            checked_trees::CheckedConditionalReturnArm::Scalar {
                                statement_ordinal: ordinal,
                                primitive_type: *primitive_type,
                            }
                        } else {
                            let (operation, operand_calls) = returns::return_value_operation(
                                program,
                                facts,
                                scalar_callees,
                                shapes,
                                machine,
                                state,
                                structural,
                                entry_claims,
                                &mut count,
                                ordinal,
                                *expression,
                                trace,
                            )?;
                            operations.extend(operand_calls);
                            checked_trees::CheckedConditionalReturnArm::Structural(operation)
                        };
                    (CheckedClosedSumCaseOutcomePlan::Return(arm), Vec::new())
                }
                TransitionTargetNode::Named { arguments, .. }
                    if terminal_calls::external_callable(program, machine, fused, transition)
                        .is_some() =>
                {
                    // Operand computations can contain payload reads below
                    // nested calls or projections. Every receiver copy still
                    // belongs to this selected case of the one saved subject.
                    if matches!(dispatch, DispatchSubject::Computed(_)) {
                        let mut pending = program
                            .statement_table
                            .expression_handles(*arguments)
                            .to_vec();
                        let mut visited = Vec::new();
                        while let Some(expression) = pending.pop() {
                            if !program.expression_table.expression_is_valid(expression)
                                || visited.contains(&expression)
                            {
                                continue;
                            }
                            visited.push(expression);
                            if dispatch
                                .payload_field(
                                    program,
                                    state,
                                    ordinal as usize,
                                    expression,
                                    variant,
                                    &mut subject_reads,
                                )?
                                .is_some()
                            {
                                continue;
                            }
                            validation::expression_traversal::children(
                                program,
                                program.expression_table.expression(expression),
                                |child| pending.push(child),
                            );
                        }
                    }
                    trace.phase("state graph: terminator: case successors: terminal call");
                    let arm = terminal_calls::arm_call(
                        program,
                        facts,
                        scalar_callees,
                        shapes,
                        machine,
                        state,
                        structural,
                        entry_claims,
                        local_results,
                        operations,
                        result,
                        next_scalar_binding,
                        transition,
                        ordinal,
                        trace,
                    )?;
                    (CheckedClosedSumCaseOutcomePlan::Return(arm), Vec::new())
                }
                TransitionTargetNode::Named {
                    path, arguments, ..
                } => {
                    let target_index =
                        crate::execution::terminal_unit::state_graph::fused_target_index(
                            program,
                            fused,
                            path.symbol,
                        )?;
                    let &(_, target) = fused.get(target_index)?;
                    let arguments = program.statement_table.expression_handles(*arguments);
                    let target_parameters = program.state_parameters(target);
                    let mut payloads = Vec::new();
                    for (parameter_index, parameter) in
                        signatures[target_index].1.iter().enumerate()
                    {
                        let argument_index = target_parameters
                            .iter()
                            .take(parameter.source_position as usize)
                            .count();
                        let argument = *arguments.get(argument_index)?;
                        let Some(field) = dispatch.payload_field(
                            program,
                            state,
                            ordinal as usize,
                            argument,
                            variant,
                            &mut subject_reads,
                        )?
                        else {
                            continue;
                        };
                        if program.primitive_type_reference(field.type_reference)?
                            != parameter.primitive_type
                        {
                            return None;
                        }
                        payloads.push(CheckedClosedSumPayloadTransferPlan {
                            field_identity: field.path_identity(),
                            primitive_type: parameter.primitive_type,
                            target_scalar_parameter_index: u32::try_from(parameter_index).ok()?,
                        });
                    }
                    let payload_parameters = payloads
                        .iter()
                        .map(|payload| payload.target_scalar_parameter_index)
                        .collect::<Vec<_>>();
                    let successor = successor_bindings(
                        program,
                        facts,
                        fused,
                        state_index,
                        signatures,
                        operations.as_slice(),
                        transition,
                        ordinal,
                        &payload_parameters,
                        SuccessorEdge::ClosedCase,
                        edge_calls,
                        shapes,
                        trace,
                    )?;
                    if subject.access == CheckedStructuralAccess::Owned
                        && successor.transfers.iter().any(|transfer| {
                        match (&subject.source, &transfer.source) {
                            (
                                CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                    parameter_index,
                                },
                                checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                                    index,
                                },
                            ) => parameter_index == index,
                            (
                                CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                    binding_ordinal: subject,
                                },
                                checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult {
                                    binding_ordinal,
                                },
                            ) => subject == binding_ordinal,
                            // A structural payload copied out of the subject
                            // would read storage the dispatch has consumed;
                            // only primitive payloads travel, as scalars.
                            (
                                _,
                                checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload {
                                    subject: payload_subject,
                                    ..
                                },
                            ) => payload_subject.source == subject.source,
                            _ => false,
                        }
                    }) {
                        return None;
                    }
                    (
                        CheckedClosedSumCaseOutcomePlan::Successor(successor),
                        payloads,
                    )
                }
                _ => return None,
            };
            cases.push(CheckedClosedSumCasePlan {
                case_identity: identity,
                outcome,
                payloads,
            });
        }
    }
    if cases.len() != variants.len() {
        return None;
    }
    if let DispatchSubject::Computed(subject_expression) = dispatch
        && !subject_reads_are_complete(
            program,
            facts,
            machine,
            state,
            transition_start,
            subject_expression,
            &subject_reads,
        )
    {
        return None;
    }
    // The selected case makes its checked payload observations available; it
    // does not require forwarding every field. Payload transfers retain the
    // exact subject, case and field identities, independent of generated
    // destructure markers or preparations preceding the dispatch.
    // Affine dispatch requires the actual source exit disposition. Copyable
    // subjects have no disposal debt; do not invent an affine-drop receipt.
    // Parameter-origin carriers and fresh results have different provenance.
    // An inspected loan disposes nothing.
    let needs_discard = subject.access == CheckedStructuralAccess::Owned
        && program.type_multiplicity(type_reference) == Multiplicity::Affine;
    // Source exit rows describe entry custody. An ordinary preceding call
    // can already own a whole parameter's disposal, including the call that
    // produces this dispatch subject. Projected moves leave their debt here.
    let consumed_parameters = operations
        .iter()
        .flat_map(|operation| operation.call_structural_arguments())
        .filter(|argument| {
            argument.access == CheckedStructuralAccess::Owned && argument.path.is_empty()
        })
        .filter_map(|argument| argument.source_parameter_index())
        .collect::<Vec<_>>();
    let mut consumed_drops = Vec::new();
    let mut found = false;
    for event in facts
        .flow
        .ownership
        .state_exit_cleanup_roots(machine.symbol, state.symbol)?
    {
        if !dispatch.is_symbol_root(event.root) {
            if let Some(source_position) = program
                .state_parameters(state)
                .iter()
                .position(|parameter| event.root == facts::PlaceRoot::Symbol(parameter.symbol))
            {
                let (index, parameter) = structural
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.position as usize == source_position)?;
                let index = u32::try_from(index).ok()?;
                if !consumed_parameters.contains(&index)
                    || parameter.access != CheckedStructuralAccess::Owned
                    || parameter.multiplicity != Multiplicity::Affine
                    || event.access != PermissionAccess::Owned
                    || event.multiplicity != Multiplicity::Affine
                    || event.claim_identity != PermissionClaimIdentity::Unknown
                    || event.provenance != language_semantics::PermissionProvenance::Unknown
                    || event.obligation_live
                    || !event.segments.is_empty()
                    || consumed_drops.contains(&index)
                {
                    return None;
                }
                consumed_drops.push(index);
            }
            continue;
        }
        // Other locals use the ordinary graph's result-custody accounting.
        if !subject.path.is_empty() {
            // This root remains available to the selected arm. Its authored
            // disposition is retained by the common edge-cleanup planner.
            continue;
        }
        if !needs_discard
            || event.access != PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Affine
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.provenance != expected_provenance
            || event.obligation_live
            || !event.segments.is_empty()
            || found
        {
            return None;
        }
        found = true;
    }
    // A computed subject is an anonymous temporary: the ownership facts name
    // no place for it, so no exit event can carry its disposal. The dispatch
    // edge consumes it, and Terminal replays that consumption on each case.
    if found != (needs_discard && matches!(dispatch, DispatchSubject::Place(_))) {
        return None;
    }
    Some(CheckedComposedUnitControlTerminatorPlan::ClosedSum {
        subject,
        subject_type: type_reference,
        subject_kind: match dispatch {
            DispatchSubject::Place(_) => checked_trees::CheckedClosedSumSubjectKind::Place,
            DispatchSubject::Computed(_) => checked_trees::CheckedClosedSumSubjectKind::Computed,
        },
        cases,
    })
}

/// The sum a closed dispatch inspects.
#[derive(Clone, Copy)]
enum DispatchSubject<'place> {
    /// A parameter or an earlier immutable local, named by every arm.
    Place(&'place validation::CanonicalPlace),
    /// One authored call. The parser copies the dispatch subject into every
    /// arm's guard and payload read; each copy keeps the one authored call's
    /// selection occurrence. The dispatch evaluates that call once, at the
    /// first arm's statement, and every copy reads its single result
    /// (wiki/spec/language/expressions.md, "Transition dispatch").
    Computed(typed_trees::expression::ExpressionHandle),
}

impl DispatchSubject<'_> {
    fn is_symbol(self, symbol: symbols::SymbolHandle) -> bool {
        matches!(self, Self::Place(subject) if subject.root == facts::PlaceRoot::Symbol(symbol))
    }

    fn is_symbol_root(self, root: facts::PlaceRoot) -> bool {
        matches!(root, facts::PlaceRoot::Symbol(symbol) if self.is_symbol(symbol))
    }

    fn is_symbol_place(self, place: &validation::CanonicalPlace) -> bool {
        matches!(self, Self::Place(subject) if subject == place)
    }

    /// Whether `expression` names this subject at `statement_index`.
    fn names(
        self,
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        match self {
            Self::Place(_) => validation::contextual_canonical_place_from_expression(
                program,
                state.symbol,
                statement_index,
                expression,
            )
            .is_some_and(|place| self.is_symbol_place(&place)),
            Self::Computed(subject) => {
                dispatch_subjects::repeats_subject(program, subject, expression)
            }
        }
    }

    /// The selected case's payload field `argument` reads, `Some(None)` when
    /// the argument does not read this subject at all, and `None` when it
    /// reads the subject in a way a case edge cannot transfer.
    #[allow(clippy::too_many_arguments)]
    fn payload_field<'program>(
        self,
        program: &'program dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        argument: typed_trees::expression::ExpressionHandle,
        variant: &'program typed_trees::data::DataVariant,
        subject_reads: &mut Vec<typed_trees::expression::ExpressionHandle>,
    ) -> Option<Option<&'program typed_trees::data::DataField>> {
        match self {
            Self::Place(subject) => {
                let Some(place) = validation::contextual_canonical_place_from_expression(
                    program,
                    state.symbol,
                    statement_index,
                    argument,
                ) else {
                    return Some(None);
                };
                if place.root != subject.root || !place.segments.starts_with(&subject.segments) {
                    return Some(None);
                }
                let [
                    facts::PlaceSegment::Case { variant: selected },
                    facts::PlaceSegment::Field { symbol },
                ] = &place.segments[subject.segments.len()..]
                else {
                    return None;
                };
                if *selected != variant.symbol {
                    return None;
                }
                // Semantic checking authorized this projection at the arm.
                // Rejoin its canonical identities to the exact selected case,
                // not to the parser's generated marker names or their order.
                let field = program.data_payload_field_by_symbol(variant, *symbol)?;
                Some(Some(field))
            }
            // The parser rewrites a bound payload name to a case-qualified
            // member read of its own subject copy; only that spelling reads
            // the one evaluated result.
            Self::Computed(subject) => {
                let Some(member) = program.expression_table.member_node(argument) else {
                    return Some(None);
                };
                if !dispatch_subjects::repeats_subject(program, subject, member.receiver) {
                    return Some(None);
                }
                if member
                    .case_variant
                    .as_ref()
                    .is_none_or(|name| name.as_str() != variant.name.as_str())
                {
                    return None;
                }
                let field = program.data_payload_fields(variant).iter().find(|field| {
                    field.name.as_str() == member.member.as_str()
                        && (!member.member_symbol.is_valid()
                            || member.member_symbol == field.symbol)
                })?;
                subject_reads.push(member.receiver);
                Some(Some(field))
            }
        }
    }
}

/// Plan the computed subject's one call at the first arm's statement. Its
/// structural result is bound like a `let` local's, anchored at the subject
/// expression, and consumed by the dispatch.
#[allow(clippy::too_many_arguments)]
fn subject_call(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural: &[checked_trees::CheckedUnitStructuralParameterPlan],
    entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
    transition_start: usize,
    subject: typed_trees::expression::ExpressionHandle,
    trace: &LocalConstructionTrace,
) -> Option<(
    checked_trees::CheckedUnitStructuralResultBindingPlan,
    typed_trees::types::TypeReferenceHandle,
)> {
    let flow =
        crate::execution::terminal_unit::types::state_flow(facts, machine.symbol, state.symbol)?;
    let mut matching = facts
        .flow
        .control
        .calls
        .span_or_empty(flow.calls)
        .iter()
        .filter(|call| {
            call.statement_index == transition_start && call.authored_expression == subject
        });
    let call = matching.next()?;
    if matching.next().is_some()
        || facts.flow.control.is_retired(state.symbol, call)
        || operations
            .iter()
            .filter_map(|operation| operation.call())
            .any(|planned| {
                planned.coordinate.statement_index as usize == call.statement_index
                    && planned.coordinate.call_ordinal as usize == call.call_ordinal
            })
    {
        return None;
    }
    let return_type = validation::call_target_return_type(program, call.target_symbol)?;
    // The subject may lend a named local of this state, exactly as a `let`
    // initializer's call does.
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut structural_results = operations
        .iter()
        .filter_map(|operation| {
            let result = operation.structural_result_binding()?;
            let local = statements
                .get(result.statement_index as usize)?
                .as_local_data()?;
            local
                .symbol
                .is_valid()
                .then(|| (result.clone(), facts::PlaceRoot::Symbol(local.symbol)))
        })
        .collect::<Vec<_>>();
    let mut structural_count = returns::next_result_ordinal(operations)?;
    crate::execution::terminal_unit::control::structural_operands::plan_call_operands(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        state,
        structural,
        &[],
        entry_claims,
        &mut structural_results,
        operations,
        &mut structural_count,
        call,
        trace,
        &mut crate::execution::terminal_unit::borrowed_windows::OpenWindows::default(),
    )?;
    // Preparations bind their own results first. The saved subject receives
    // the next ordinary result ordinal and consumes those exact operands.
    let result = checked_trees::CheckedUnitStructuralResultBindingPlan {
        statement_index: u32::try_from(transition_start).ok()?,
        binding_ordinal: u32::try_from(structural_count).ok()?,
        type_identity: shapes.add_type(return_type, &[], &[])?,
        multiplicity: program.type_multiplicity(return_type),
        local: Default::default(),
    };
    let operation = crate::execution::terminal_unit::calls::build_call_operation(
        program,
        facts,
        Some(scalar_callees),
        machine,
        state,
        structural,
        &[],
        entry_claims,
        call,
        Some(crate::execution::terminal_unit::calls::ExpectedCallValueResult::Structural(&result)),
        &structural_results,
        operations,
        trace,
    )?;
    let mut operation = crate::execution::terminal_unit::control::bind_structural_call_result(
        operation,
        result.clone(),
    )?;
    // The dispatch, not the state's return, consumes the subject.
    let (_, discard_result_on_return) = operation.discardable_structural_result_mut()?;
    *discard_result_on_return = false;
    let CheckedUnitEffectOperationPlan::Call(planned) = &operation else {
        return None;
    };
    // A boundary result is the same once-evaluated owned subject as a named
    // local's result. Keep the complete boundary call plan: normal source and
    // receiver replay still own its authority. Composed structural-result
    // calls do not yet discharge completion receipts here, so preserve that
    // custody fence rather than rejecting every boundary-produced subject.
    if (planned.is_boundary() && !planned.claim_transfers().is_empty())
        || !super::operation_custody::call_custody_admitted(planned, operations)
    {
        return None;
    }
    crate::execution::terminal_unit::control::statement_sequence::consume_results(
        operations, &operation,
    )?;
    operations.push(operation);
    Some((result, return_type))
}

/// Every call row at the dispatch that copies the computed subject is one
/// the dispatch reads: the planned subject itself, a later guard's subject,
/// or a payload read's receiver. A copy in any other position would be a
/// second evaluation the single dispatch cannot account for.
fn subject_reads_are_complete(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    transition_start: usize,
    subject: typed_trees::expression::ExpressionHandle,
    subject_reads: &[typed_trees::expression::ExpressionHandle],
) -> bool {
    let Some(flow) =
        crate::execution::terminal_unit::types::state_flow(facts, machine.symbol, state.symbol)
    else {
        return false;
    };
    facts
        .flow
        .control
        .calls
        .span_or_empty(flow.calls)
        .iter()
        .filter(|call| call.statement_index >= transition_start)
        .filter(|call| {
            dispatch_subjects::repeats_subject(program, subject, call.authored_expression)
        })
        .all(|call| {
            call.authored_expression == subject || subject_reads.contains(&call.authored_expression)
        })
}
