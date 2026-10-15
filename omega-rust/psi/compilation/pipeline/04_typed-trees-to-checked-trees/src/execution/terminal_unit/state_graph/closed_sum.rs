//! Case terminators inspect exact sums. Each selected arm transfers or disposes
//! owned custody through normal-edge cleanup; a shared or mutable loan is read in place.
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
    DataMember,
};
use super::{
    CheckFacts, CheckedComposedUnitControlTerminatorPlan, CheckedScalarExpressionRole,
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, Multiplicity, ScalarCalleeClosure, ShapeCollector,
    Signature, TransitionExit, TransitionGuardNode, TransitionTargetNode, TypeReferenceNode,
    returns, terminal_calls,
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
    trace.phase("state graph: case subject storage");
    let subject_place = validation::contextual_canonical_place_from_expression(
        program,
        state.symbol,
        transition_start,
        expression,
    );
    let dispatch = match subject_place.as_ref() {
        Some(place) if place.root.as_symbol().is_some() => DispatchSubject::Place(place),
        _ => DispatchSubject::Computed(dispatch_subjects::evaluated_case_subject(
            program,
            statements,
            transition_start,
        )?),
    };
    let (mut subject, mut type_reference) = if let DispatchSubject::Computed(subject_expression) =
        dispatch
    {
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
        // Selection observes the same stored subject on every arm. Owned
        // payload transfers and residual cleanup belong to the selected
        // edge; a shared or mutable loan stays the caller's.
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
                type_identity: retained.type_identity,
                access: retained.access,
            },
            inspected,
        )
    } else {
        let mut matching = operations
            .iter()
            .filter_map(super::retained_structural_result)
            .filter_map(|result| {
                let local = statements
                    .get(result.statement_index as usize)?
                    .as_local_data()?;
                (dispatch.is_symbol(local.symbol)
                    && !local.is_mutable
                    && result.statement_index < transition_start as u32)
                    .then_some((result, local))
            });
        let (result, local) = matching.next()?;
        if matching.next().is_some()
            || !matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            )
        {
            return None;
        }
        let (subject, inspected) =
            if let Some(shell) = validation::reference_shell(program, local.type_reference) {
                // The descriptor's affine custody belongs to its producer,
                // not to the unrestricted shared-reference source type. The
                // producer is whichever operation bound the result: a place
                // capture (`EstablishReference`) or a descriptor value
                // retained through a selector computation (`EstablishLocal`).
                if result.multiplicity != Multiplicity::Affine
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
                        result.type_identity,
                        CheckedStructuralAccess::Owned,
                    ),
                    local.type_reference,
                )
            };
        (subject, inspected)
    };
    // Give successor planning the exact saved root, named or anonymous.
    // Canonical places rejoin parser copies to this one expression identity.
    let mut established_results = local_results.to_vec();
    if let DispatchSubject::Computed(expression) = dispatch {
        let CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } =
            subject.source
        else {
            return None;
        };
        let result = operations
            .iter()
            .filter_map(super::retained_structural_result)
            .find(|result| result.binding_ordinal == binding_ordinal)?
            .clone();
        established_results.push((result, facts::PlaceRoot::Expression(expression)));
    }
    let edge_calls = EdgeCalls {
        local_results: &established_results,
        ..edge_calls
    };
    if let DispatchSubject::Place(place) = dispatch
        && !place.segments.is_empty()
    {
        let (projected, path) = crate::execution::terminal_unit::calls::projected_argument_path(
            program,
            state.symbol,
            transition_start,
            place,
            None,
            None,
        )?;
        subject.path.extend(path);
        subject.type_identity = crate::execution::terminal_unit::types::base_type_identity(
            program,
            projected,
            &[],
            &[],
        )?;
        // Dispatch observes a subtree, not an owned move of its enclosing
        // record. Ordinary selected-edge cleanup still owns that root.
        subject.access = CheckedStructuralAccess::SharedBorrow;
        type_reference = projected;
        shapes.add_type(projected, &[], &[])?;
    }
    trace.phase("state graph: case subject contents");
    // Dispatch observes the stored tag; it does not copy, consume or dispose
    // of the payload. Qualification, loan and cleanup obligations belong to
    // the selected arm's existing transfer and normal-exit judgments.
    if !validation::has_contents(
        program,
        type_reference,
        &[],
        validation::ContentsRequirement::BorrowedStorage,
    ) {
        return None;
    }
    let symbol = &program
        .type_reference_table
        .type_reference(type_reference)
        .as_named_symbol()?;
    let data = program.data_definition_by_symbol(*symbol)?;
    // The roster is the cases a value of this instance can carry: a case
    // whose `where` fact this instance refutes (`Integer where T == i32` on
    // `Value<bool>`) is not a coverage obligation, exactly as typing's
    // exhaustiveness count treats it, so an authored dispatch over the
    // possible cases alone is complete without a `_` arm.
    let variants = program
        .data_members(data)
        .iter()
        .map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .filter(|variant| !::validation::impossible_variant(program, variant))
        .collect::<Vec<_>>();
    // Primitive payloads enter the scalar roster below. Structural payloads
    // use ordinary rooted successor transfers; unread fields remain in the
    // selected edge's checked cleanup partition.
    // Each arm selects at least one case, except a final `_` that follows
    // complete coverage (see below).
    if variants.is_empty() || statements.len() - transition_start > variants.len() + 1 {
        return None;
    }
    trace.phase("state graph: case selected arms");
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
                        let identity = checked_trees::CheckedMemberIdentity::of_member(
                            variant.identity,
                            variant.name.clone(),
                        );
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
            let identity = checked_trees::CheckedMemberIdentity::of_member(
                variant.identity,
                variant.name.clone(),
            );
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
                        edge_calls.local_results,
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
                    for argument in arguments {
                        dispatch.payload_field(
                            program,
                            state,
                            ordinal as usize,
                            *argument,
                            variant,
                            &mut subject_reads,
                        )?;
                    }
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
                            field_identity: checked_trees::CheckedMemberIdentity::of_member(
                                field.identity,
                                field.name.clone(),
                            ),
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
    // Selection observes a subject; the chosen arm owns its transfers and
    // remaining disposal. Named subjects use the ordinary normal-edge plan,
    // which retains partial moves, hooks and exact selected-exit coordinates.
    // Requiring a separate whole-root drop here would reject a valid payload
    // move before that shared custody owner can account for its complement.
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
        // The cast judgment already established policy and interval membership.
        // Forward its unchanged carrier through the same field transfer used by
        // a bare payload; real numeric conversions and semantic domains remain
        // opaque and must retain their ordinary computation/custody plans.
        if let Some(operand) = crate::values::scalar::semantic_casts::same_carrier_operand(
            program,
            state.symbol,
            argument,
        ) {
            return self.payload_field(
                program,
                state,
                statement_index,
                operand,
                variant,
                subject_reads,
            );
        }
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
        call,
        Some(crate::execution::terminal_unit::calls::ExpectedCallValueResult::Structural(&result)),
        &structural_results,
        operations,
        trace,
    )?;
    let mut operation = crate::execution::terminal_unit::control::bind_structural_call_result(
        program,
        facts,
        machine.symbol,
        state.symbol,
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
    let coordinate = planned.coordinate;
    crate::execution::terminal_unit::control::statement_sequence::consume_results(
        operations, &operation,
    )?;
    operations.push(operation);
    // Operand temporaries formed for this call, such as a slice descriptor
    // over a borrowed array, die at its normal continuation before the
    // dispatch inspects the result, exactly as a statement call's do.
    crate::execution::terminal_unit::cleanup::anonymous::append_continuation(
        program,
        facts,
        shapes,
        machine,
        state,
        coordinate,
        &structural_results,
        operations,
    )?;
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
