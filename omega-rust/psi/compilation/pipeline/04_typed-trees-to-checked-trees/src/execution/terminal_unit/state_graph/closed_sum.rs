//! Case terminators inspect exact owned sums and retain their disposal obligations.
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
    CheckedUnitStructuralArgumentSourcePlan, ExpressionNode, Multiplicity, PermissionEventKind,
    PermissionEventSource, ScalarCalleePlans, ShapeCollector, Signature, StatementNode,
    TransitionExit, TransitionGuardNode, TransitionTargetNode, TypeReferenceNode, TypedTrees,
    returns, terminal_calls,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::state_graph::{
    EdgeCalls, FusedStates, SuccessorEdge, successor_bindings,
};
use crate::semantic::calls::dispatch_subjects;

#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    program: &TypedTrees,
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
    let StatementNode::Transition(first) = statements.get(transition_start)? else {
        return None;
    };
    let TransitionGuardNode::When(guard) = first.guard else {
        return None;
    };
    let (expression, _) = crate::proof::exact_outcome_case_test(program, guard)?;
    let dispatch = match crate::flow::contextual_canonical_place_from_expression(
        program,
        state.symbol,
        transition_start,
        expression,
    ) {
        Some(place) => {
            let facts::PlaceRoot::Symbol(result_symbol) = place.root else {
                return None;
            };
            if !place.segments.is_empty() {
                return None;
            }
            DispatchSubject::Place(result_symbol)
        }
        None => DispatchSubject::Computed(dispatch_subjects::evaluated_case_subject(
            program,
            statements,
            transition_start,
        )?),
    };
    let (subject, type_reference, expected_provenance) =
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
                CheckedUnitStructuralArgumentPlan {
                    source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: result.binding_ordinal,
                    },
                    path: Vec::new(),
                    type_identity: result.type_identity,
                    access: CheckedStructuralAccess::Owned,
                },
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
            if retained.access != CheckedStructuralAccess::Owned
                || !matches!(
                    retained.multiplicity,
                    Multiplicity::Affine | Multiplicity::Unrestricted
                )
                || retained.multiplicity != program.type_multiplicity(parameter.type_reference)
                || !retained.qualifications.is_empty()
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
                    access: CheckedStructuralAccess::Owned,
                },
                parameter.type_reference,
                language_semantics::PermissionProvenance::Unknown,
            )
        } else {
            let mut matching = operations
                .iter()
                .filter_map(super::retained_structural_result)
                .filter_map(|result| {
                    let StatementNode::LocalData(local) =
                        statements.get(result.statement_index as usize)?
                    else {
                        return None;
                    };
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
                || result.multiplicity != program.type_multiplicity(local.type_reference)
            {
                return None;
            }
            (
                CheckedUnitStructuralArgumentPlan {
                    source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: result.binding_ordinal,
                    },
                    path: Vec::new(),
                    type_identity: result.type_identity.clone(),
                    access: CheckedStructuralAccess::Owned,
                },
                local.type_reference,
                language_semantics::PermissionProvenance::Established {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                    source: PermissionEventSource::Statement {
                        statement_index: result.statement_index as usize,
                    },
                },
            )
        };
    if !validation::has_plain_owned_contents_with_numeric_constraints(program, type_reference)
        && !validation::has_owned_or_shared_view_contents(program, type_reference)
    {
        return None;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)?;
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
    if variants.is_empty() || statements.len() - transition_start > variants.len() {
        return None;
    }
    let marker_start = statements[..transition_start].iter().rposition(|statement| !matches!(statement, StatementNode::LocalData(local) if local.name.as_str().starts_with("__arm_destructure#V="))).map_or(0, |index| index + 1);
    // Destructure markers re-read a place subject. A computed subject has no
    // place to re-read: its payload reads are the parser's copies below.
    if matches!(dispatch, DispatchSubject::Computed(_)) && marker_start != transition_start {
        return None;
    }
    let mut destructures = Vec::new();
    for (index, statement) in statements[marker_start..transition_start]
        .iter()
        .enumerate()
    {
        let StatementNode::LocalData(local) = statement else {
            return None;
        };
        let encoded = local.name.as_str().strip_prefix("__arm_destructure#V=")?;
        let (variant, encoded) = encoded.split_once('#')?;
        let (fields, _) = encoded.split_once("#~subject=")?;
        let selected_variant = variants
            .iter()
            .find(|candidate| candidate.name.as_str() == variant)?;
        let place = crate::flow::contextual_canonical_place_from_expression(
            program,
            state.symbol,
            marker_start + index,
            local.initial_value,
        )?;
        if local.is_mutable || !dispatch.is_symbol_place(&place) {
            return None;
        }
        for field in fields.split('#') {
            if field.is_empty()
                || destructures.contains(&(variant, field))
                || !program
                    .data_payload_fields(selected_variant)
                    .iter()
                    .any(|candidate| candidate.name.as_str() == field)
            {
                return None;
            }
            destructures.push((variant, field));
        }
    }
    let mut cases = Vec::new();
    let mut count = returns::next_result_ordinal(operations.as_slice())?;
    // Every copy of a computed subject the dispatch reads: the guards'
    // subjects and the payload reads' receivers.
    let mut subject_reads = Vec::new();
    for (index, statement) in statements[transition_start..].iter().enumerate() {
        let StatementNode::Transition(transition) = statement else {
            return None;
        };
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
        if selected_variants.is_empty() {
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
                    if terminal_calls::invoked_callable(program, machine, transition).is_some() =>
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
                                    &destructures,
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
                            .filter(|parameter| !parameter.is_self)
                            .count();
                        let argument = *arguments.get(argument_index)?;
                        let Some(field) = dispatch.payload_field(
                            program,
                            state,
                            ordinal as usize,
                            argument,
                            variant,
                            &destructures,
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
                    if successor.transfers.iter().any(|transfer| {
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
    // Destructuring makes a payload available; it does not require forwarding
    // every binding. The subject's contents are plain owned storage, so unused
    // fields carry no separate ownership obligation. Validate their declared
    // identities above and retain the whole subject's disposal below.
    // Affine dispatch requires the actual source exit disposition. Copyable
    // subjects have no disposal debt; do not invent an affine-drop receipt.
    // Parameter-origin carriers and fresh results have different provenance.
    let needs_discard = program.type_multiplicity(type_reference) == Multiplicity::Affine;
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
    for (_, event) in facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
        })
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
    Some(CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, cases })
}

/// The sum a closed dispatch inspects.
#[derive(Clone, Copy)]
enum DispatchSubject {
    /// A parameter or an earlier immutable local, named by every arm.
    Place(symbols::SymbolHandle),
    /// One authored call. The parser copies the dispatch subject into every
    /// arm's guard and payload read; each copy keeps the one authored call's
    /// selection occurrence. The dispatch evaluates that call once, at the
    /// first arm's statement, and every copy reads its single result
    /// (wiki/spec/language/expressions.md, "Transition dispatch").
    Computed(typed_trees::expression::ExpressionHandle),
}

impl DispatchSubject {
    fn is_symbol(self, symbol: symbols::SymbolHandle) -> bool {
        matches!(self, Self::Place(subject) if subject == symbol)
    }

    fn is_symbol_root(self, root: facts::PlaceRoot) -> bool {
        matches!(root, facts::PlaceRoot::Symbol(symbol) if self.is_symbol(symbol))
    }

    fn is_symbol_place(self, place: &crate::flow::CanonicalPlace) -> bool {
        self.is_symbol_root(place.root) && place.segments.is_empty()
    }

    /// Whether `expression` names this subject at `statement_index`.
    fn names(
        self,
        program: &TypedTrees,
        state: &typed_trees::state::State,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        match self {
            Self::Place(_) => crate::flow::contextual_canonical_place_from_expression(
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

    /// The selected case's payload field `argument` reads, `Ok(None)` when
    /// the argument does not read this subject at all, and `Err` when it
    /// reads the subject in a way a case edge cannot transfer.
    #[allow(clippy::too_many_arguments)]
    fn payload_field<'program>(
        self,
        program: &'program TypedTrees,
        state: &typed_trees::state::State,
        statement_index: usize,
        argument: typed_trees::expression::ExpressionHandle,
        variant: &'program typed_trees::data::DataVariant,
        destructures: &[(&str, &str)],
        subject_reads: &mut Vec<typed_trees::expression::ExpressionHandle>,
    ) -> Option<Option<&'program typed_trees::data::DataField>> {
        match self {
            Self::Place(_) => {
                let Some(place) = crate::flow::contextual_canonical_place_from_expression(
                    program,
                    state.symbol,
                    statement_index,
                    argument,
                ) else {
                    return Some(None);
                };
                if !self.is_symbol_root(place.root) {
                    return Some(None);
                }
                let [
                    facts::PlaceSegment::Case { variant: selected },
                    facts::PlaceSegment::Field { symbol },
                ] = place.segments.as_slice()
                else {
                    return None;
                };
                if *selected != variant.symbol {
                    return None;
                }
                let field = program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.symbol == *symbol)?;
                destructures
                    .contains(&(variant.name.as_str(), field.name.as_str()))
                    .then_some(Some(field))
            }
            // The parser rewrites a bound payload name to a case-qualified
            // member read of its own subject copy; only that spelling reads
            // the one evaluated result.
            Self::Computed(subject) => {
                let ExpressionNode::Member(member) = program.expression_table.expression(argument)
                else {
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
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
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
    let return_type = crate::flow::call_target_return_type(program, call.target_symbol)?;
    // The subject may lend a named local of this state, exactly as a `let`
    // initializer's call does.
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut structural_results = operations
        .iter()
        .filter_map(|operation| {
            let result = operation.structural_result_binding()?;
            let StatementNode::LocalData(local) =
                statements.get(result.statement_index as usize)?
            else {
                return None;
            };
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
    )?;
    // Preparations bind their own results first. The saved subject receives
    // the next ordinary result ordinal and consumes those exact operands.
    let result = checked_trees::CheckedUnitStructuralResultBindingPlan {
        statement_index: u32::try_from(transition_start).ok()?,
        binding_ordinal: u32::try_from(structural_count).ok()?,
        type_identity: shapes.add_type(return_type, &[], &[])?,
        multiplicity: program.type_multiplicity(return_type),
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
        false,
        Some(crate::execution::terminal_unit::calls::ExpectedCallValueResult::Structural(&result)),
        &structural_results,
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
    program: &TypedTrees,
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
