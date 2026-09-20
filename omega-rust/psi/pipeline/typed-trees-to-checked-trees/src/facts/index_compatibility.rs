use arena::HandleSpan;
use checked_trees::{
    CheckedOperatorFacts, ContractProofFactKind, FlowFacts, FlowSemanticContextRef,
    IndexCompatibilityDischarge, IndexCompatibilityFact, IndexCompatibilityFacts, ProofFacts,
};
use diagnostics::Diagnostic;
use facts::{FactHandle, FactPayload, FactPlan, ProgramPoint};
use language_semantics::SemanticDomainId;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::{DataMember, TypeParameterKind};
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, StaticMachineArgument,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use typed_trees::types::{
    DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
struct IndexedInstance {
    family: SymbolHandle,
    semantic_id: SemanticDomainId,
    arguments: Vec<TypeReferenceHandle>,
    label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompatibilityKey {
    point: ProgramPoint,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    family: SymbolHandle,
    actual: SemanticDomainId,
    expected: SemanticDomainId,
}

struct ResolvedStateCall<'program, 'flow> {
    fact: &'flow checked_trees::FlowCallFact,
    site: crate::semantic_calls::CallSite<'program>,
}

struct StateCallIndex<'program, 'flow> {
    calls: Vec<ResolvedStateCall<'program, 'flow>>,
}

impl<'program, 'flow> StateCallIndex<'program, 'flow> {
    fn new(
        program: &'program TypedTrees,
        flow: &'flow FlowFacts,
        state_flow: &'flow checked_trees::FlowStateFact,
    ) -> Self {
        let calls = flow
            .control
            .calls
            .span_or_empty(state_flow.calls)
            .iter()
            .filter_map(|fact| {
                crate::semantic_calls::find_call_site(
                    program,
                    state_flow.machine_symbol,
                    state_flow.state_symbol,
                    fact.statement_index,
                    fact.call_ordinal,
                )
                .map(|site| ResolvedStateCall { fact, site })
            })
            .collect();
        Self { calls }
    }

    fn contexts_after_value(
        &self,
        statement_index: usize,
        value: ExpressionHandle,
        fallback: HandleSpan<FlowSemanticContextRef>,
    ) -> HandleSpan<FlowSemanticContextRef> {
        self.calls
            .iter()
            .filter(|call| call.fact.statement_index == statement_index)
            .find_map(|call| match &call.site {
                crate::semantic_calls::CallSite::Expression { expression, .. }
                    if *expression == value =>
                {
                    Some(call.fact.exit_semantic_contexts)
                }
                _ => None,
            })
            .unwrap_or(fallback)
    }
}

pub(super) fn build_index_compatibility_facts(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    semantic: &FactPlan,
    flow: &FlowFacts,
    proof: &ProofFacts,
) -> Result<IndexCompatibilityFacts, Vec<Diagnostic>> {
    let mut conditions = Vec::new();
    let mut diagnostics = Vec::new();
    let mut unresolved = Vec::new();

    for (_, state_flow) in flow.control.states.iter() {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == state_flow.machine_symbol)
        else {
            continue;
        };
        let Some(state) = crate::semantic_calls::find_state_in_machine(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
        ) else {
            continue;
        };

        let state_calls = StateCallIndex::new(program, flow, state_flow);
        for resolved in &state_calls.calls {
            let call = resolved.fact;
            let Some(parameters) =
                crate::semantic_calls::call_target_parameters(program, call.target_symbol)
            else {
                continue;
            };
            let arguments =
                crate::semantic_calls::call_site_argument_expressions(program, &resolved.site);
            let point = ProgramPoint::Call {
                machine_symbol: state_flow.machine_symbol,
                state_symbol: state_flow.state_symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
            };
            for (argument, parameter) in arguments
                .iter()
                .zip(parameters.iter().filter(|parameter| !parameter.is_self))
            {
                append_expression_compatibilities(
                    program,
                    operators,
                    semantic,
                    flow,
                    proof,
                    machine,
                    state,
                    call.statement_index,
                    *argument,
                    parameter.type_reference,
                    point,
                    call.entry_semantic_contexts,
                    &state_calls,
                    &mut conditions,
                    &mut diagnostics,
                    &mut unresolved,
                );
            }
        }

        let statements = program.statement_table.statements(state.statement_nodes);
        for (statement_index, statement) in statements.iter().enumerate() {
            let statement_contexts = flow
                .state_statement(state_flow, statement_index)
                .map(|statement| statement.entry_semantic_contexts)
                .unwrap_or(state_flow.entry_semantic_contexts);
            let statement_point = ProgramPoint::Statement {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                statement_index,
            };
            match statement {
                StatementNode::LocalData(local) => {
                    let contexts = state_calls.contexts_after_value(
                        statement_index,
                        local.initial_value,
                        statement_contexts,
                    );
                    append_expression_compatibilities(
                        program,
                        operators,
                        semantic,
                        flow,
                        proof,
                        machine,
                        state,
                        statement_index,
                        local.initial_value,
                        local.type_reference,
                        statement_point,
                        contexts,
                        &state_calls,
                        &mut conditions,
                        &mut diagnostics,
                        &mut unresolved,
                    );
                }
                StatementNode::Assignment(assignment) => {
                    if let Some(target_type) = crate::flow::expression_type_reference_in_state(
                        program,
                        state.symbol,
                        statement_index,
                        assignment.target,
                    ) {
                        append_expression_compatibilities(
                            program,
                            operators,
                            semantic,
                            flow,
                            proof,
                            machine,
                            state,
                            statement_index,
                            assignment.value,
                            target_type,
                            statement_point,
                            state_calls.contexts_after_value(
                                statement_index,
                                assignment.value,
                                statement_contexts,
                            ),
                            &state_calls,
                            &mut conditions,
                            &mut diagnostics,
                            &mut unresolved,
                        );
                    }
                }
                StatementNode::Expression(expression)
                    if statement_index + 1 == statements.len() =>
                {
                    append_expression_compatibilities(
                        program,
                        operators,
                        semantic,
                        flow,
                        proof,
                        machine,
                        state,
                        statement_index,
                        *expression,
                        state.return_type,
                        ProgramPoint::Exit {
                            machine_symbol: machine.symbol,
                            state_symbol: state.symbol,
                            statement_index,
                            transition_target: Default::default(),
                        },
                        state_calls.contexts_after_value(
                            statement_index,
                            *expression,
                            statement_contexts,
                        ),
                        &state_calls,
                        &mut conditions,
                        &mut diagnostics,
                        &mut unresolved,
                    );
                }
                StatementNode::Transition(transition) => {
                    for target in [transition.target, transition.continuation] {
                        if !target.is_valid() {
                            continue;
                        }
                        let TransitionTargetNode::Value(value) =
                            program.statement_table.transition_target(target)
                        else {
                            continue;
                        };
                        append_expression_compatibilities(
                            program,
                            operators,
                            semantic,
                            flow,
                            proof,
                            machine,
                            state,
                            statement_index,
                            *value,
                            state.return_type,
                            ProgramPoint::Exit {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                                statement_index,
                                transition_target: target,
                            },
                            state_calls.contexts_after_value(
                                statement_index,
                                *value,
                                statement_contexts,
                            ),
                            &state_calls,
                            &mut conditions,
                            &mut diagnostics,
                            &mut unresolved,
                        );
                    }
                }
                StatementNode::RootBinding(_)
                | StatementNode::AssemblyFact(_)
                | StatementNode::Call(_)
                | StatementNode::Expression(_) => {}
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(IndexCompatibilityFacts { conditions })
    } else {
        Err(diagnostics)
    }
}

#[allow(clippy::too_many_arguments)]
fn append_expression_compatibilities(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    semantic: &FactPlan,
    flow: &FlowFacts,
    proof: &ProofFacts,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    point: ProgramPoint,
    contexts: HandleSpan<FlowSemanticContextRef>,
    state_calls: &StateCallIndex<'_, '_>,
    conditions: &mut Vec<IndexCompatibilityFact>,
    diagnostics: &mut Vec<Diagnostic>,
    unresolved: &mut Vec<CompatibilityKey>,
) {
    if !value.is_valid() || !target_type.is_valid() {
        return;
    }
    // A value-position call's result is checked after that call has completed,
    // so its own established `ensures` are part of the exact local context at
    // this boundary. Recompute this for recursive literal members as well as
    // top-level stores/returns.
    let contexts = state_calls.contexts_after_value(statement_index, value, contexts);
    let mut substitutions = enclosing_call_substitutions(program, state, state_calls, point);
    for substitution in value_call_substitutions(program, state, statement_index, value) {
        if !substitutions
            .iter()
            .any(|(binder, _)| *binder == substitution.0)
        {
            substitutions.push(substitution);
        }
    }
    let mut actual =
        expression_indexed_instances(program, operators, machine, state, statement_index, value);
    append_call_result_ensured_instances(
        program,
        proof,
        state_calls,
        machine,
        state,
        statement_index,
        value,
        &mut actual,
    );
    let mut expected = Vec::new();
    collect_type_indexed_instances(program, target_type, &mut expected, &mut Vec::new());
    for actual in &actual {
        for expected in expected
            .iter()
            .filter(|expected| expected.family == actual.family)
        {
            if !actual.semantic_id.is_valid()
                || !expected.semantic_id.is_valid()
                || actual.arguments.is_empty()
                || expected.arguments.is_empty()
            {
                continue;
            }
            let discharge = if actual.semantic_id == expected.semantic_id {
                if actual
                    .arguments
                    .iter()
                    .chain(&expected.arguments)
                    .all(|argument| is_closed_index_argument(program, *argument))
                {
                    IndexCompatibilityDischarge::ClosedEvaluation
                } else {
                    IndexCompatibilityDischarge::LicensedNormalization {
                        operation_count: selected_operation_count(
                            program,
                            actual.arguments.iter().chain(&expected.arguments).copied(),
                        ),
                    }
                }
            } else if bound_index_arguments_equal(
                program,
                &actual.arguments,
                &expected.arguments,
                &substitutions,
            ) {
                // The two instances differ only in spelling: substituting
                // the call's bound index arguments makes every position
                // denote the same subject or literal. No normalization work
                // is needed -- the bound index IS the compared index.
                if actual
                    .arguments
                    .iter()
                    .chain(&expected.arguments)
                    .all(|argument| {
                        bound_index_argument_is_closed(program, *argument, &substitutions)
                    })
                {
                    IndexCompatibilityDischarge::ClosedEvaluation
                } else {
                    IndexCompatibilityDischarge::LicensedNormalization { operation_count: 0 }
                }
            } else if let Some(facts) = established_index_equalities(
                program,
                semantic,
                flow,
                contexts,
                &actual.arguments,
                &expected.arguments,
            ) {
                IndexCompatibilityDischarge::EstablishedLocalFacts { facts }
            } else {
                let key = CompatibilityKey {
                    point,
                    value,
                    target_type,
                    family: actual.family,
                    actual: actual.semantic_id,
                    expected: expected.semantic_id,
                };
                if !unresolved.contains(&key) {
                    unresolved.push(key);
                    let name = compatibility_name(program, actual, expected, point);
                    diagnostics.push(Diagnostic::error(format!(
                        "index compatibility condition `{name}` is not established: actual `{}` \
                         and expected `{}` are distinct normalized instances; closed evaluation, \
                         licensed normalization, or an exact local equality fact is required",
                        actual.label, expected.label,
                    )));
                }
                continue;
            };
            let name = compatibility_name(program, actual, expected, point);
            let candidate = IndexCompatibilityFact {
                name,
                point,
                value,
                target_type,
                family: actual.family,
                actual_instance: actual.semantic_id,
                expected_instance: expected.semantic_id,
                actual_label: actual.label.clone(),
                expected_label: expected.label.clone(),
                discharge,
            };
            if !conditions.iter().any(|existing| {
                existing.point == candidate.point
                    && existing.value == candidate.value
                    && existing.target_type == candidate.target_type
                    && existing.family == candidate.family
                    && existing.actual_instance == candidate.actual_instance
                    && existing.expected_instance == candidate.expected_instance
                    && existing.actual_label == candidate.actual_label
                    && existing.expected_label == candidate.expected_label
            }) {
                conditions.push(candidate);
            }
        }
    }

    append_unevidenced_establishment_diagnostics(
        program,
        value,
        target_type,
        point,
        &actual,
        &expected,
        diagnostics,
        unresolved,
    );

    match program.expression_table.expression(value) {
        ExpressionNode::StructLiteral(literal) => {
            if let Some(definition) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == literal.type_name.as_str())
                .filter(|definition| definition.type_parameters.is_empty())
            {
                for field in program.expression_table.struct_fields(literal.fields) {
                    let Some(field_type) = construction_field_type(
                        program,
                        definition,
                        literal.case_name.as_ref().map(|name| name.as_str()),
                        field.name.as_str(),
                    ) else {
                        continue;
                    };
                    append_expression_compatibilities(
                        program,
                        operators,
                        semantic,
                        flow,
                        proof,
                        machine,
                        state,
                        statement_index,
                        field.value,
                        field_type,
                        point,
                        contexts,
                        state_calls,
                        conditions,
                        diagnostics,
                        unresolved,
                    );
                }
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            if let Some(element_type) = literal_element_type(program, target_type) {
                for element in program.expression_table.expression_handles(*elements) {
                    append_expression_compatibilities(
                        program,
                        operators,
                        semantic,
                        flow,
                        proof,
                        machine,
                        state,
                        statement_index,
                        *element,
                        element_type,
                        point,
                        contexts,
                        state_calls,
                        conditions,
                        diagnostics,
                        unresolved,
                    );
                }
            }
        }
        _ => {}
    }
}

fn construction_field_type(
    program: &TypedTrees,
    definition: &typed_trees::data::DataDefinition,
    case_name: Option<&str>,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    if let Some(case_name) = case_name
        && let Some(variant) =
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str() == case_name => {
                        Some(variant)
                    }
                    _ => None,
                })
    {
        for field in program.data_payload_fields(variant) {
            if field.name.as_str() == field_name && field.type_reference.is_valid() {
                return Some(field.type_reference);
            }
        }
    }
    program.data_members(definition).iter().find_map(|member| {
        let DataMember::Field(field) = member else {
            return None;
        };
        (field.name.as_str() == field_name && field.type_reference.is_valid())
            .then_some(field.type_reference)
    })
}

fn literal_element_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => literal_element_type(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            literal_element_type(program, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => Some(*element_type),
        _ => None,
    }
}

/// A restating write whose declared type names an indexed instance of a family
/// with no predicate body must be handed that instance by the value itself.
/// Membership in such a family cannot follow from proving predicates, so the
/// write establishes nothing on its own: `wiki/spec/language/domains.md` keeps
/// establishment on the value's own route (transported qualification, an
/// authorized `established by` route, or an explicit qualification of a
/// predicate-free, route-free family), and
/// `wiki/spec/resources/placed_access.md` fixes the entire route set of
/// `Extent::Resident<P, T>`: initialization from `Vacant` and an owned `T`, a
/// `ResidentContentTransfer<P, T>` issuance occurrence with its receipt, or
/// forwarding an existing custody.
///
/// A value-position call's result is exactly what its signature and contract
/// row say it is. When neither the declared return type nor an `ensures` on the
/// reserved `result` names any instance of the family, the restatement has no
/// establishment at all, and refusing it here is what stops the local's
/// declared-type seeding (`flow/transfers.rs`) from manufacturing the fact that
/// later call `requires` consume. An instance either route does name keeps its
/// ordinary index-compatibility treatment above, including the distinct
/// normalized instance refusal, and a predicate-bearing domain keeps its
/// `checks/contracts/writes.rs` discharge untouched.
///
/// A semantic-domain cast is the other introduction surface the same write
/// must not seed from: `x as T in D` mints a predicate-free, route-free family
/// instance unconditionally (validation's staged mint fence judges only
/// predicate-bearing or routed domains). Minting remains the sanctioned
/// introduction on carriers whose domain set is all tag-style qualifications
/// (a `Region::Left -> Region::Right` retag, a scalar's `Wrapping`), but on a
/// custody-marked carrier -- one that already declares a predicate-bearing or
/// `established by`-routed domain, like `Extent` with `Granted` -- a vacuous
/// family is a member of a managed custody system, not a tag namespace:
/// `Resident<P, T>` and `Vacant` assert facts about who owns the range, and
/// `placed_access.md` fixes `Resident`'s route set while no declaration on
/// `Extent` authorizes an `as` mint. Until such a family declares its
/// qualification-carrier route, the only establishment is refusal.
#[allow(clippy::too_many_arguments)]
fn append_unevidenced_establishment_diagnostics(
    program: &TypedTrees,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    point: ProgramPoint,
    actual: &[IndexedInstance],
    expected: &[IndexedInstance],
    diagnostics: &mut Vec<Diagnostic>,
    unresolved: &mut Vec<CompatibilityKey>,
) {
    if let ExpressionNode::Cast(cast) = program.expression_table.expression(value) {
        append_unevidenced_cast_mint_diagnostic(
            program,
            cast,
            value,
            target_type,
            point,
            diagnostics,
            unresolved,
        );
        return;
    }
    if !matches!(
        program.expression_table.expression(value),
        ExpressionNode::Call(_)
    ) {
        return;
    }
    for expected in expected {
        if !expected.semantic_id.is_valid()
            || expected.arguments.is_empty()
            || actual.iter().any(|actual| actual.family == expected.family)
            || !is_bodyless_domain_family(program, expected.family)
        {
            continue;
        }
        let key = CompatibilityKey {
            point,
            value,
            target_type,
            family: expected.family,
            actual: SemanticDomainId::NULL,
            expected: expected.semantic_id,
        };
        if unresolved.contains(&key) {
            continue;
        }
        unresolved.push(key);
        diagnostics.push(Diagnostic::error(format!(
            "declared instance `{}` has no establishment: the call establishes no instance of \
             domain family `{}` -- neither its declared return type nor its `ensures` names one \
             -- and a domain family with no predicate body has no predicate the write could \
             prove; the value must already carry the instance, through the callee's `ensures \
             result in {}`, a declared place of that type, or the family's authorized \
             establishment route (at {})",
            expected.label,
            family_label(&expected.label),
            expected.label,
            point_label(program, point),
        )));
    }
}

/// A semantic-domain cast whose introduced family is a predicate-free,
/// route-free domain declared over a custody-marked carrier mints managed
/// state the caller never produced. Tag-style minting on carriers whose
/// domains are all vacuous keeps the staged `as` surface; on a carrier with
/// any predicate-bearing or routed domain the vacuous member's qualification
/// names custody state, so the write must refuse the value the cast minted.
fn append_unevidenced_cast_mint_diagnostic(
    program: &TypedTrees,
    cast: &typed_trees::expression::TableCastExpression,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    point: ProgramPoint,
    diagnostics: &mut Vec<Diagnostic>,
    unresolved: &mut Vec<CompatibilityKey>,
) {
    if !cast.semantic_domain_symbol.is_valid() {
        return;
    }
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == cast.semantic_domain_symbol)
    else {
        return;
    };
    if domain.predicate_body.is_present() || !domain.establishment_routes.is_empty() {
        return;
    }
    if !domain_target_is_custody_marked(program, domain.target_type) {
        return;
    }
    let key = CompatibilityKey {
        point,
        value,
        target_type,
        family: cast.semantic_domain_symbol,
        actual: SemanticDomainId::NULL,
        expected: cast.semantic_domain_id,
    };
    if unresolved.contains(&key) {
        return;
    }
    unresolved.push(key);
    diagnostics.push(Diagnostic::error(format!(
        "declared instance `{}` has no establishment: `as` mints an instance of \
         domain family `{}` on custody-marked carrier `{}` -- the family \
         declares no predicate body and no `establishment` route, and a \
         carrier that already route-manages a domain does not admit a minted \
         member; the instance must come from an establishing call, an \
         `established by` route, or the family's declared qualification-carrier \
         route (at {})",
        qualification_label(program, cast),
        family_label(&qualification_label(program, cast)),
        carrier_label(program, domain.target_type),
        point_label(program, point),
    )));
}

/// Whether the type a domain is declared over already carries a
/// predicate-bearing or `established by`-routed domain: such a carrier's
/// qualifications are route-managed custody state, so a predicate-free,
/// route-free member cannot be introduced by `as`. Constrained or borrowed
/// spellings of the carrier unwrap to the named root the domains attach to.
fn domain_target_is_custody_marked(program: &TypedTrees, target_type: TypeReferenceHandle) -> bool {
    let mut type_reference = target_type;
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            TypeReferenceNode::Named { symbol, .. } => {
                if !symbol.is_valid() {
                    return false;
                }
                return program.domain_definitions().iter().any(|domain| {
                    domain_carrier_symbol(program, domain.target_type) == Some(*symbol)
                        && (domain.predicate_body.is_present()
                            || !domain.establishment_routes.is_empty())
                });
            }
            _ => return false,
        }
    }
}

/// The named carrier symbol a domain declaration attaches to, unwrapping
/// constrained or borrowed spellings.
fn domain_carrier_symbol(
    program: &TypedTrees,
    target_type: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let mut type_reference = target_type;
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            TypeReferenceNode::Named { symbol, .. } => return Some(*symbol),
            _ => return None,
        }
    }
}

/// Display spelling of a domain's declared carrier for the mint diagnostic.
fn carrier_label(program: &TypedTrees, target_type: TypeReferenceHandle) -> String {
    program.display_type_reference(target_type)
}

/// The family spelling inside an instance label: `Resident<SlotPlacement, Slot>`
/// names family `Resident`, so a diagnostic can name the family whose
/// establishment is missing beside the exact instance that was declared.
fn family_label(instance_label: &str) -> &str {
    instance_label
        .split_once('<')
        .map_or(instance_label, |(family, _)| family)
}

/// Whether the declared family carries no predicate body. An unresolved symbol
/// is not treated as bodyless: only a found declaration states that membership
/// has no predicates a write could prove.
fn is_bodyless_domain_family(program: &TypedTrees, family: SymbolHandle) -> bool {
    program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == family)
        .is_some_and(|domain| !domain.predicate_body.is_present())
}

fn compatibility_name(
    program: &TypedTrees,
    actual: &IndexedInstance,
    expected: &IndexedInstance,
    point: ProgramPoint,
) -> String {
    format!(
        "index-equality:{}:{}:{}=={}",
        program.symbols.display_path(actual.family, "::"),
        point_label(program, point),
        actual.label,
        expected.label,
    )
}

#[derive(Debug, Clone, Copy)]
struct ExpressionSubstitution<'program> {
    symbol: SymbolHandle,
    name: &'program str,
    value: ExpressionHandle,
}

fn established_index_equalities(
    program: &TypedTrees,
    semantic: &FactPlan,
    flow: &FlowFacts,
    contexts: HandleSpan<FlowSemanticContextRef>,
    actual: &[TypeReferenceHandle],
    expected: &[TypeReferenceHandle],
) -> Option<Vec<FactHandle>> {
    if actual.len() != expected.len() {
        return None;
    }
    let differing = actual
        .iter()
        .zip(expected)
        .filter(|(left, right)| !index_arguments_structurally_equal(program, **left, **right))
        .collect::<Vec<_>>();
    if differing.is_empty() {
        return None;
    }

    let mut evidence = Vec::new();
    for (actual, expected) in differing {
        let fact = established_index_equality_for_argument(
            program, semantic, flow, contexts, *actual, *expected,
        )?;
        if !evidence.contains(&fact) {
            evidence.push(fact);
        }
    }
    Some(evidence)
}

fn established_index_equality_for_argument(
    program: &TypedTrees,
    semantic: &FactPlan,
    flow: &FlowFacts,
    contexts: HandleSpan<FlowSemanticContextRef>,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
) -> Option<FactHandle> {
    for context_ref in flow.contexts.semantic_context_refs.span_or_empty(contexts) {
        let context = semantic.contexts.get(context_ref.context);
        for fact_ref in semantic.refs.span_or_empty(context.facts) {
            let fact = semantic.facts.get(fact_ref.fact);
            let expression = match fact.payload {
                FactPayload::BooleanExpression(expression) => expression,
                FactPayload::ContractBooleanExpression { expression, .. } => expression,
                _ => continue,
            };
            let substitutions = fact_substitutions(program, flow, fact.point);
            if expression_proves_index_equality(
                program,
                expression,
                &substitutions,
                actual,
                expected,
            ) {
                return Some(fact_ref.fact);
            }
        }
    }
    None
}

fn expression_proves_index_equality(
    program: &TypedTrees,
    expression: ExpressionHandle,
    substitutions: &[ExpressionSubstitution<'_>],
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    if binary.operator == BinaryOperator::And {
        // Conjunction introduction establishes each authored conjunct. This is
        // still an exact local lookup: do not derive transitive equalities or
        // search for any theorem outside the active fact context.
        return expression_proves_index_equality(
            program,
            binary.left,
            substitutions,
            actual,
            expected,
        ) || expression_proves_index_equality(
            program,
            binary.right,
            substitutions,
            actual,
            expected,
        );
    }
    if binary.operator != BinaryOperator::Equal {
        return false;
    }
    let direct =
        substituted_expression_matches_index_argument(program, binary.left, substitutions, actual)
            && substituted_expression_matches_index_argument(
                program,
                binary.right,
                substitutions,
                expected,
            );
    let symmetric = substituted_expression_matches_index_argument(
        program,
        binary.left,
        substitutions,
        expected,
    ) && substituted_expression_matches_index_argument(
        program,
        binary.right,
        substitutions,
        actual,
    );
    direct || symmetric
}

fn fact_substitutions<'program>(
    program: &'program TypedTrees,
    flow: &FlowFacts,
    point: ProgramPoint,
) -> Vec<ExpressionSubstitution<'program>> {
    let ProgramPoint::CallEnsures {
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    } = point
    else {
        return Vec::new();
    };
    let Some(call_site) = crate::semantic_calls::find_call_site(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    ) else {
        return Vec::new();
    };
    let target_symbol = match &call_site {
        crate::semantic_calls::CallSite::Statement(call) => call.target_symbol,
        crate::semantic_calls::CallSite::Expression { call, .. } => call.target_symbol,
        crate::semantic_calls::CallSite::TransitionNamed { .. } => call_flow_at_point(flow, point)
            .map_or_else(SymbolHandle::invalid, |call| call.target_symbol),
    };
    let Some(parameters) = crate::semantic_calls::call_target_parameters(program, target_symbol)
    else {
        return Vec::new();
    };
    let arguments = crate::semantic_calls::call_site_argument_expressions(program, &call_site);
    parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(arguments)
        .map(|(parameter, value)| ExpressionSubstitution {
            symbol: parameter.symbol,
            name: parameter.name.as_str(),
            value: *value,
        })
        .collect()
}

fn call_flow_at_point(
    flow: &FlowFacts,
    point: ProgramPoint,
) -> Option<&checked_trees::FlowCallFact> {
    let (machine_symbol, state_symbol, statement_index, call_ordinal) = match point {
        ProgramPoint::Call {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        }
        | ProgramPoint::CallRequires {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        }
        | ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        } => (machine_symbol, state_symbol, statement_index, call_ordinal),
        _ => return None,
    };
    let state = flow.control.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
            .then_some(state)
    })?;
    flow.control
        .calls
        .span_or_empty(state.calls)
        .iter()
        .find(|call| call.statement_index == statement_index && call.call_ordinal == call_ordinal)
}

fn index_arguments_structurally_equal(
    program: &TypedTrees,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    if left == right {
        return true;
    }
    match (
        program.type_reference_table.type_reference(left),
        program.type_reference_table.type_reference(right),
    ) {
        (TypeReferenceNode::ConstExpression(left), TypeReferenceNode::ConstExpression(right)) => {
            program
                .expression_table
                .expressions_structurally_equal(*left, *right)
        }
        (
            TypeReferenceNode::Named {
                symbol: left_symbol,
                name: left_name,
            },
            TypeReferenceNode::Named {
                symbol: right_symbol,
                name: right_name,
            },
        ) => left_symbol == right_symbol && left_name.as_str() == right_name.as_str(),
        _ => false,
    }
}

/// One index argument a call bound to its callee's declared index binder.
/// A symbolic call to a generic target compares the callee's declared
/// `Coordinate<I>` against the caller's instance as the bound index: `I` is
/// the template's spelling, and this records what this call supplied.
#[derive(Debug, Clone)]
pub(crate) enum BoundIndexArgument {
    /// The caller-side subject symbol the argument denotes: a parameter, a
    /// `let` declared before the call, a forwarded generic binder, or a named
    /// const whose symbol is its own canonical identity.
    Subject(SymbolHandle),
    /// A closed literal index, kept by canonical spelling.
    Literal(String),
}

/// Pair each const-position index binder (`const` or runtime `Value`) of a
/// call target with the index the call bound it to. Argument dispatch mirrors
/// `monomorphization::selection`'s proposal walk: explicit structural type
/// arguments consume type slots, machine/evidence arguments consume their own
/// slots, and every other shape — forwarded binders, literals, open names, and
/// runtime subjects — consumes one const slot in authored order. Any argument
/// shape this does not classify empties the whole map rather than risk a
/// misaligned substitution.
pub(crate) fn bound_index_substitutions(
    program: &TypedTrees,
    state: &State,
    scope_limit: usize,
    target_symbol: SymbolHandle,
    machine_arguments: &[StaticMachineArgument],
) -> Vec<(SymbolHandle, BoundIndexArgument)> {
    let type_parameters =
        crate::semantic_calls::call_target_type_parameters(program, target_symbol);
    let const_parameters = type_parameters
        .iter()
        .filter(|parameter| {
            matches!(
                parameter.kind,
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
            )
        })
        .collect::<Vec<_>>();
    if const_parameters.is_empty() {
        return Vec::new();
    }
    let mut substitutions = Vec::new();
    let mut const_index = 0;
    for argument in machine_arguments {
        enum Slot {
            Type,
            Const,
            Other,
        }
        let slot = if argument.type_reference.is_valid() {
            Slot::Type
        } else if argument.application.is_some() || argument.evidence_projection.is_some() {
            Slot::Other
        } else if argument.const_literal.is_some() {
            Slot::Const
        } else if argument.symbol.is_valid()
            && program
                .machines()
                .iter()
                .flat_map(|machine| program.machine_type_parameters(machine))
                .any(|parameter| {
                    parameter.symbol == argument.symbol
                        && matches!(
                            parameter.kind,
                            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
                        )
                })
        {
            // A forwarded `const`/`Value` binder occupies a const slot under
            // the callee's own parameter ordinal.
            Slot::Const
        } else if !argument.symbol.is_valid() {
            Slot::Const
        } else {
            match program.symbols.get(argument.symbol).kind {
                SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter => {
                    Slot::Type
                }
                SymbolKind::Conformance
                | SymbolKind::ConformanceParameter
                | SymbolKind::State
                | SymbolKind::MachineParameter => Slot::Other,
                SymbolKind::Local | SymbolKind::Parameter | SymbolKind::Const => Slot::Const,
                _ => return Vec::new(),
            }
        };
        match slot {
            Slot::Const => {
                if let Some(parameter) = const_parameters.get(const_index)
                    && let Some(bound) = bound_index_argument(program, state, scope_limit, argument)
                {
                    substitutions.push((parameter.symbol, bound));
                }
                const_index += 1;
            }
            Slot::Type | Slot::Other => {}
        }
    }
    substitutions
}

/// The caller-side index one static machine argument denotes, resolved
/// against the enclosing state's scope at the call's statement position —
/// the same scope rule `resolve_runtime_subject` applies during selection.
fn bound_index_argument(
    program: &TypedTrees,
    state: &State,
    scope_limit: usize,
    argument: &StaticMachineArgument,
) -> Option<BoundIndexArgument> {
    if let Some(literal) = &argument.const_literal {
        let spelling = literal
            .value_i64()
            .map(i128::from)
            .or_else(|| literal.value_u64().map(i128::from))
            .map_or_else(|| literal.text().to_owned(), |value| value.to_string());
        return Some(BoundIndexArgument::Literal(spelling));
    }
    if argument.type_reference.is_valid()
        || argument.application.is_some()
        || argument.evidence_projection.is_some()
    {
        return None;
    }
    resolve_scope_subject(program, state, scope_limit, argument).map(BoundIndexArgument::Subject)
}

/// Resolve an index argument to the subject symbol it denotes in the
/// caller's scope. A parameter, a generic binder, or a named const is its own
/// canonical identity; a bare or local name resolves to a state parameter or
/// the nearest `let` of the same name declared before the call — the scope
/// rule `resolve_runtime_subject` applies during selection, so a shadowed
/// local cannot bind to an earlier same-named binding.
fn resolve_scope_subject(
    program: &TypedTrees,
    state: &State,
    scope_limit: usize,
    argument: &StaticMachineArgument,
) -> Option<SymbolHandle> {
    if argument.symbol.is_valid() {
        match program.symbols.get(argument.symbol).kind {
            SymbolKind::Parameter | SymbolKind::TypeParameter | SymbolKind::Const => {
                return Some(argument.symbol);
            }
            SymbolKind::Local => {}
            _ => return None,
        }
    }
    let [name] = argument.path.as_ref() else {
        return None;
    };
    let mut resolved = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == name.as_str())
        .map(|parameter| parameter.symbol);
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(scope_limit)
    {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        if local.name.as_str() == name.as_str() {
            resolved = Some(local.symbol);
        }
    }
    resolved
}

/// A call's index substitutions: one entry pairing each of the callee's
/// const-position binders with the caller-side index bound at this site.
pub(crate) type BoundIndexSubstitutions = Vec<(SymbolHandle, BoundIndexArgument)>;

/// The substitutions implied by the call at `point`, if `point` is a call
/// boundary — this is what lets an argument in `Coordinate<N>` discharge a
/// parameter declared `in Coordinate<I>` when the call bound `I` to `N`.
fn enclosing_call_substitutions(
    program: &TypedTrees,
    state: &State,
    state_calls: &StateCallIndex<'_, '_>,
    point: ProgramPoint,
) -> BoundIndexSubstitutions {
    let ProgramPoint::Call {
        statement_index,
        call_ordinal,
        ..
    } = point
    else {
        return Vec::new();
    };
    let Some(resolved) = state_calls.calls.iter().find(|resolved| {
        resolved.fact.statement_index == statement_index
            && resolved.fact.call_ordinal == call_ordinal
    }) else {
        return Vec::new();
    };
    let (target_symbol, machine_arguments) = match &resolved.site {
        crate::semantic_calls::CallSite::Statement(call) => {
            (call.target_symbol, call.machine_arguments.as_ref())
        }
        crate::semantic_calls::CallSite::Expression { call, .. } => {
            (call.target_symbol, call.machine_arguments.as_ref())
        }
        crate::semantic_calls::CallSite::TransitionNamed { .. } => return Vec::new(),
    };
    bound_index_substitutions(
        program,
        state,
        statement_index,
        target_symbol,
        machine_arguments,
    )
}

/// The substitutions implied by `value` itself when the checked value is a
/// call whose declared result carries the callee's binders — a call result
/// `Coordinate<I>` compares as `Coordinate<N>` under the call's own binding.
fn value_call_substitutions(
    program: &TypedTrees,
    state: &State,
    statement_index: usize,
    value: ExpressionHandle,
) -> BoundIndexSubstitutions {
    let mut node = value;
    let call = loop {
        match program.expression_table.expression(node) {
            ExpressionNode::Borrow(inner) => node = inner.target,
            ExpressionNode::Atomic(atomic) => node = atomic.result,
            ExpressionNode::Call(call) => break call,
            _ => return Vec::new(),
        }
    };
    bound_index_substitutions(
        program,
        state,
        statement_index,
        call.target_symbol,
        &call.machine_arguments,
    )
}

/// Substitute binder positions on both sides, then compare pairwise. `true`
/// only when every pair denotes the same bound index — the same subject
/// symbol or the same literal spelling — after substitution.
fn bound_index_arguments_equal(
    program: &TypedTrees,
    actual: &[TypeReferenceHandle],
    expected: &[TypeReferenceHandle],
    substitutions: &BoundIndexSubstitutions,
) -> bool {
    !substitutions.is_empty()
        && !actual.is_empty()
        && actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected.iter())
            .all(|(actual, expected)| {
                bound_index_argument_equal(program, *actual, *expected, substitutions)
            })
}

fn bound_index_argument_equal(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    substitutions: &BoundIndexSubstitutions,
) -> bool {
    let bound_actual = substitute_bound_index(program, actual, substitutions);
    let bound_expected = substitute_bound_index(program, expected, substitutions);
    match (bound_actual, bound_expected) {
        (None, None) => index_arguments_structurally_equal(program, actual, expected),
        (Some(bound), None) => bound_index_argument_matches(program, bound, expected),
        (None, Some(bound)) => bound_index_argument_matches(program, bound, actual),
        (Some(left), Some(right)) => match (left, right) {
            (BoundIndexArgument::Subject(left), BoundIndexArgument::Subject(right)) => {
                left == right
            }
            (BoundIndexArgument::Literal(left), BoundIndexArgument::Literal(right)) => {
                left == right
            }
            _ => false,
        },
    }
}

fn substitute_bound_index<'a>(
    program: &TypedTrees,
    argument: TypeReferenceHandle,
    substitutions: &'a BoundIndexSubstitutions,
) -> Option<&'a BoundIndexArgument> {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(argument)
    else {
        return None;
    };
    substitutions
        .iter()
        .find(|(binder, _)| *binder == *symbol)
        .map(|(_, bound)| bound)
}

fn bound_index_argument_matches(
    program: &TypedTrees,
    bound: &BoundIndexArgument,
    argument: TypeReferenceHandle,
) -> bool {
    match bound {
        BoundIndexArgument::Subject(subject) => matches!(
            program.type_reference_table.type_reference(argument),
            TypeReferenceNode::Named { symbol, .. } if *symbol == *subject
        ),
        BoundIndexArgument::Literal(spelling) => {
            match program.type_reference_table.type_reference(argument) {
                TypeReferenceNode::Named { name, .. } => name.as_str() == spelling.as_str(),
                _ => false,
            }
        }
    }
}

/// Whether a substituted index argument position is a closed index — literals
/// close the position; bound subjects keep it open.
fn bound_index_argument_is_closed(
    program: &TypedTrees,
    argument: TypeReferenceHandle,
    substitutions: &BoundIndexSubstitutions,
) -> bool {
    match substitute_bound_index(program, argument, substitutions) {
        Some(BoundIndexArgument::Literal(_)) => true,
        Some(BoundIndexArgument::Subject(_)) => false,
        None => is_closed_index_argument(program, argument),
    }
}

fn substituted_expression_matches_index_argument(
    program: &TypedTrees,
    expression: ExpressionHandle,
    substitutions: &[ExpressionSubstitution<'_>],
    argument: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(argument) {
        TypeReferenceNode::ConstExpression(expected) => {
            substituted_expressions_equal(program, expression, substitutions, *expected)
        }
        TypeReferenceNode::Named { symbol, name } => {
            let expression =
                substituted_root(program, expression, substitutions).unwrap_or(expression);
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(path) => {
                    if symbol.is_valid() && (path.symbol.is_valid() || path.head_symbol.is_valid())
                    {
                        path.symbol == *symbol || path.head_symbol == *symbol
                    } else {
                        matches!(
                            program.expression_table.name_path_members(path.members),
                            [only] if only.as_str() == name.as_str()
                        )
                    }
                }
                ExpressionNode::Integer(literal) => named_integer_value(name.as_str())
                    .is_some_and(|expected| literal.value_bignum() == Some(expected)),
                ExpressionNode::Boolean(value) => match name.as_str() {
                    "true" => *value,
                    "false" => !*value,
                    _ => false,
                },
                _ => false,
            }
        }
        _ => false,
    }
}

fn substituted_root(
    program: &TypedTrees,
    expression: ExpressionHandle,
    substitutions: &[ExpressionSubstitution<'_>],
) -> Option<ExpressionHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    let members = program.expression_table.name_path_members(path.members);
    let [only] = members else {
        return None;
    };
    substitutions
        .iter()
        .find(|substitution| {
            if substitution.symbol.is_valid()
                && (path.symbol.is_valid() || path.head_symbol.is_valid())
            {
                path.symbol == substitution.symbol || path.head_symbol == substitution.symbol
            } else {
                only.as_str() == substitution.name
            }
        })
        .map(|substitution| substitution.value)
}

fn substituted_expressions_equal(
    program: &TypedTrees,
    authored: ExpressionHandle,
    substitutions: &[ExpressionSubstitution<'_>],
    local: ExpressionHandle,
) -> bool {
    if authored == local {
        return true;
    }
    if !authored.is_valid() || !local.is_valid() {
        return false;
    }
    if let Some(substituted) = substituted_root(program, authored, substitutions) {
        return substituted_expressions_equal(program, substituted, &[], local);
    }
    if substitutions.is_empty()
        && program
            .expression_table
            .expressions_structurally_equal(authored, local)
    {
        return true;
    }
    match (
        program.expression_table.expression(authored),
        program.expression_table.expression(local),
    ) {
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
        (ExpressionNode::Integer(left), ExpressionNode::Name(right))
        | (ExpressionNode::Name(right), ExpressionNode::Integer(left)) => {
            expression_name_atom(program, right)
                .and_then(named_integer_value)
                .is_some_and(|right| left.value_bignum() == Some(right))
        }
        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
        (ExpressionNode::Boolean(left), ExpressionNode::Name(right))
        | (ExpressionNode::Name(right), ExpressionNode::Boolean(left)) => {
            matches!(expression_name_atom(program, right), Some("true") if *left)
                || matches!(expression_name_atom(program, right), Some("false") if !*left)
        }
        (ExpressionNode::String(left), ExpressionNode::String(right)) => left == right,
        (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            left.head_symbol == right.head_symbol
                && left.symbol == right.symbol
                && program
                    .expression_table
                    .name_path_members(left.members)
                    .iter()
                    .map(|member| member.as_str())
                    .eq(program
                        .expression_table
                        .name_path_members(right.members)
                        .iter()
                        .map(|member| member.as_str()))
        }
        (ExpressionNode::Borrow(left), ExpressionNode::Borrow(right)) => {
            left.access == right.access
                && substituted_expressions_equal(program, left.target, substitutions, right.target)
        }
        (ExpressionNode::Unary(left), ExpressionNode::Unary(right)) => {
            left.operator == right.operator
                && substituted_expressions_equal(
                    program,
                    left.operand,
                    substitutions,
                    right.operand,
                )
        }
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            left.operator == right.operator
                && substituted_expressions_equal(program, left.left, substitutions, right.left)
                && substituted_expressions_equal(program, left.right, substitutions, right.right)
        }
        (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
            substituted_expressions_equal(program, left.collection, substitutions, right.collection)
                && substituted_expressions_equal(program, left.index, substitutions, right.index)
        }
        (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
            left.member_symbol == right.member_symbol
                && left.member.as_str() == right.member.as_str()
                && substituted_expressions_equal(
                    program,
                    left.receiver,
                    substitutions,
                    right.receiver,
                )
        }
        (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
            let left_arguments = program.expression_table.expression_handles(left.arguments);
            let right_arguments = program.expression_table.expression_handles(right.arguments);
            left.target_symbol == right.target_symbol
                && left.target.as_str() == right.target.as_str()
                && substituted_expressions_equal(
                    program,
                    left.receiver,
                    substitutions,
                    right.receiver,
                )
                && left_arguments.len() == right_arguments.len()
                && left_arguments
                    .iter()
                    .zip(right_arguments)
                    .all(|(left, right)| {
                        substituted_expressions_equal(program, *left, substitutions, *right)
                    })
        }
        (ExpressionNode::ArrayLiteral(left), ExpressionNode::ArrayLiteral(right)) => {
            let left = program.expression_table.expression_handles(*left);
            let right = program.expression_table.expression_handles(*right);
            left.len() == right.len()
                && left.iter().zip(right).all(|(left, right)| {
                    substituted_expressions_equal(program, *left, substitutions, *right)
                })
        }
        (ExpressionNode::StructLiteral(left), ExpressionNode::StructLiteral(right)) => {
            let left_fields = program.expression_table.struct_fields(left.fields);
            let right_fields = program.expression_table.struct_fields(right.fields);
            left.type_name.as_str() == right.type_name.as_str()
                && left.case_name.as_ref().map(|name| name.as_str())
                    == right.case_name.as_ref().map(|name| name.as_str())
                && left_fields.len() == right_fields.len()
                && left_fields.iter().zip(right_fields).all(|(left, right)| {
                    left.name.as_str() == right.name.as_str()
                        && substituted_expressions_equal(
                            program,
                            left.value,
                            substitutions,
                            right.value,
                        )
                })
        }
        _ => false,
    }
}

fn expression_name_atom<'program>(
    program: &'program TypedTrees,
    path: &typed_trees::expression::TableNamePath,
) -> Option<&'program str> {
    let [only] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    Some(only.as_str())
}

fn expression_indexed_instances(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Vec<IndexedInstance> {
    let mut instances = Vec::new();
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            return expression_indexed_instances(
                program,
                operators,
                machine,
                state,
                statement_index,
                inner.target,
            );
        }
        ExpressionNode::Atomic(atomic) => {
            return expression_indexed_instances(
                program,
                operators,
                machine,
                state,
                statement_index,
                atomic.result,
            );
        }
        ExpressionNode::Cast(cast) => {
            collect_type_indexed_instances(
                program,
                cast.target_type,
                &mut instances,
                &mut Vec::new(),
            );
            if cast.semantic_domain_symbol.is_valid() && !cast.semantic_domain_arguments.is_empty()
            {
                instances.push(IndexedInstance {
                    family: cast.semantic_domain_symbol,
                    semantic_id: cast.semantic_domain_id,
                    arguments: program
                        .type_reference_table
                        .type_reference_handles(cast.semantic_domain_arguments)
                        .to_vec(),
                    label: qualification_label(program, cast),
                });
            }
            return instances;
        }
        ExpressionNode::Call(call) => {
            if let Some(return_type) = call_return_type(program, call.target_symbol) {
                collect_type_indexed_instances(
                    program,
                    return_type,
                    &mut instances,
                    &mut Vec::new(),
                );
            } else if let Some(operator_use) = operators
                .named_uses()
                .find(|operator_use| operator_use.expression == expression)
                && let Some(operator) = program
                    .operators()
                    .iter()
                    .find(|operator| operator.symbol == operator_use.selected_operator_symbol)
            {
                collect_type_indexed_instances(
                    program,
                    operator.return_type,
                    &mut instances,
                    &mut Vec::new(),
                );
            }
            return instances;
        }
        ExpressionNode::Binary(_) | ExpressionNode::Unary(_) | ExpressionNode::Indexed(_) => {
            if let Some(operator_use) = operators.expression_use(expression)
                && let Some(operator) = program
                    .operators()
                    .iter()
                    .find(|operator| operator.symbol == operator_use.selected_operator_symbol)
            {
                collect_type_indexed_instances(
                    program,
                    operator.return_type,
                    &mut instances,
                    &mut Vec::new(),
                );
                return instances;
            }
        }
        _ => {}
    }
    if let Some(type_reference) = crate::flow::expression_type_reference_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    ) {
        collect_type_indexed_instances(program, type_reference, &mut instances, &mut Vec::new());
    } else if let Some(type_reference) =
        validation::declared_place_type_raw(program, machine, Some(state), expression)
    {
        collect_type_indexed_instances(program, type_reference, &mut instances, &mut Vec::new());
    }
    instances
}

/// The indexed instances a value-position call's own `ensures` declare on
/// its reserved `result`. A declared return type `-> Extent` names no
/// instance, so `expression_indexed_instances` finds none there; the
/// requirement's `ensures result in Granted & Resident<SlotPlacement, Slot>`
/// does, and the typed membership fact attached to this call carries the
/// identity the typer interned on it. A restating `let` (or store, or
/// return) then compares against that instance exactly as it compares
/// against a declared field's, instead of joining the family by symbol and
/// keeping whichever index the restatement chose. The facts are read from
/// the call's contract row, before `call_contract_evidence` decides whether
/// the promise is admitted as establishment: like the declared return type,
/// the declared instance is what the value is, whatever its evidence.
fn append_call_result_ensured_instances(
    program: &TypedTrees,
    proof: &ProofFacts,
    state_calls: &StateCallIndex<'_, '_>,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    value: ExpressionHandle,
    instances: &mut Vec<IndexedInstance>,
) {
    if !matches!(
        program.expression_table.expression(value),
        ExpressionNode::Call(_)
    ) {
        return;
    }
    let Some(call) = state_calls.calls.iter().find(|call| {
        call.fact.statement_index == statement_index
            && matches!(
                &call.site,
                crate::semantic_calls::CallSite::Expression { expression, .. }
                    if *expression == value
            )
    }) else {
        return;
    };
    let Some((_, contract_call)) = proof.contract_calls.iter().find(|(_, contract_call)| {
        contract_call.caller_machine_symbol == machine.symbol
            && contract_call.caller_state_symbol == state.symbol
            && contract_call.statement_index == statement_index
            && contract_call.call_ordinal == call.fact.call_ordinal
    }) else {
        return;
    };
    for reference in proof
        .contract_fact_refs
        .span_or_empty(contract_call.ensures)
    {
        let contract = proof.contract_facts.get(reference.fact);
        if contract.kind != ContractProofFactKind::Ensures {
            continue;
        }
        let typed_trees::domain::ProofFact::Membership(membership) =
            program.proof_facts.get(contract.fact)
        else {
            continue;
        };
        // The subject must be the whole reserved result: a projection such
        // as `result.storage` belongs to that field's declared type.
        if !membership.semantic_domain.is_valid()
            || !is_reserved_result_name(program, membership.value)
        {
            continue;
        }
        let arguments = program
            .type_reference_table
            .type_reference_handles(membership.domain_arguments)
            .to_vec();
        if arguments.is_empty()
            || instances.iter().any(|instance| {
                instance.family == membership.domain_symbol
                    && instance.semantic_id == membership.semantic_domain
            })
        {
            continue;
        }
        let name = program
            .domain_path_members(membership.domain)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        let label = domain_label(program, &name, &arguments);
        instances.push(IndexedInstance {
            family: membership.domain_symbol,
            semantic_id: membership.semantic_domain,
            arguments,
            label,
        });
    }
}

/// Whether a contract subject is the bare reserved `result` name: an
/// unresolved single-member path spelled `result`, the same discriminator
/// `validation::reserved_result_owner` applies before it looks the owning
/// machine up. That owner lookup scans machines only, and a boundary trait
/// signature's `result` has no machine; the facts read here are already
/// scoped to one call's own contract row, so the spelling suffices.
fn is_reserved_result_name(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    !path.symbol.is_valid()
        && !path.head_symbol.is_valid()
        && program
            .expression_table
            .name_path_member_symbols(path.member_symbols)
            .iter()
            .all(|symbol| !symbol.is_valid())
        && matches!(
            program.expression_table.name_path_members(path.members),
            [name] if name.as_str() == "result"
        )
}

fn call_return_type(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if let Some(state) = crate::semantic_calls::find_state(program, state_symbol) {
        return Some(state.return_type);
    }
    if let Some((_, signature)) = program.machine_parameter_signature(state_symbol) {
        return Some(signature.return_type);
    }
    program.traits().iter().find_map(|trait_definition| {
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == state_symbol)
            .map(|signature| signature.return_type)
    })
}

fn collect_type_indexed_instances(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    instances: &mut Vec<IndexedInstance>,
    visited: &mut Vec<TypeReferenceHandle>,
) {
    if !type_reference.is_valid() || visited.contains(&type_reference) {
        return;
    }
    visited.push(type_reference);
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_type_indexed_instances(program, *referee, instances, visited)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_indexed_instances(program, *base_type, instances, visited);
            for constraint in program.type_reference_table.constraints(*constraints) {
                let TypeConstraintNode::Domain(domain) = constraint else {
                    continue;
                };
                if domain.symbol.is_valid() && !domain.arguments.is_empty() {
                    instances.push(instance_from_constraint(program, domain));
                }
            }
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => {}
    }
}

fn instance_from_constraint(program: &TypedTrees, domain: &DomainConstraint) -> IndexedInstance {
    IndexedInstance {
        family: domain.symbol,
        semantic_id: domain.semantic_id,
        arguments: domain.arguments.clone(),
        label: domain_label(program, domain.name.as_str(), &domain.arguments),
    }
}

fn domain_label(program: &TypedTrees, name: &str, arguments: &[TypeReferenceHandle]) -> String {
    if arguments.is_empty() {
        return name.to_owned();
    }
    let arguments = arguments
        .iter()
        .map(|argument| index_argument_label(program, *argument))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}<{arguments}>")
}

fn qualification_label(
    program: &TypedTrees,
    cast: &typed_trees::expression::TableCastExpression,
) -> String {
    let name = program
        .expression_table
        .name_path_members(cast.semantic_domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    domain_label(
        program,
        &name,
        program
            .type_reference_table
            .type_reference_handles(cast.semantic_domain_arguments),
    )
}

fn index_argument_label(program: &TypedTrees, argument: TypeReferenceHandle) -> String {
    match program.type_reference_table.type_reference(argument) {
        TypeReferenceNode::Named { name, .. } => {
            language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
                .map_or_else(|| name.to_string(), |value| value.display)
        }
        TypeReferenceNode::ConstExpression(expression) => {
            program.expression_table.display_name(*expression)
        }
        _ => program.display_type_reference(argument),
    }
}

fn is_closed_index_argument(program: &TypedTrees, argument: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(argument) {
        TypeReferenceNode::Named { name, .. } => {
            language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str()).is_some()
                || name.as_str().parse::<i128>().is_ok()
        }
        _ => false,
    }
}

fn named_integer_value(name: &str) -> Option<numerics::bignum::BigInt> {
    let display = language_semantics::const_value::CanonicalConstValue::from_atom(name)
        .map_or_else(|| name.to_owned(), |value| value.display);
    let (negative, unsigned) = display
        .strip_prefix('-')
        .map_or((false, display.as_str()), |unsigned| (true, unsigned));
    let (base, digits) = if let Some(digits) = unsigned.strip_prefix("0b") {
        (2, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0o") {
        (8, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0x") {
        (16, digits)
    } else {
        (10, unsigned)
    };
    let digits = if negative {
        format!("-{digits}")
    } else {
        digits.to_owned()
    };
    numerics::bignum::BigInt::from_str_radix(&digits, base)
}

fn selected_operation_count(
    program: &TypedTrees,
    arguments: impl Iterator<Item = TypeReferenceHandle>,
) -> usize {
    let mut expressions = Vec::new();
    for argument in arguments {
        let TypeReferenceNode::ConstExpression(expression) =
            program.type_reference_table.type_reference(argument)
        else {
            continue;
        };
        if !expressions.contains(expression) {
            expressions.push(*expression);
        }
    }
    program
        .open_index_normalizations
        .iter()
        .filter(|normalization| expressions.contains(&normalization.expression))
        .map(|normalization| normalization.operations.len())
        .sum()
}

fn point_label(program: &TypedTrees, point: ProgramPoint) -> String {
    let symbol = |symbol| {
        let path = program.symbols.display_path(symbol, "::");
        if path.is_empty() {
            format!("symbol-{}", symbol.arena_index())
        } else {
            path
        }
    };
    match point {
        ProgramPoint::Global => "global".to_owned(),
        ProgramPoint::Definition { symbol: definition } => symbol(definition),
        ProgramPoint::Machine { machine_symbol } => symbol(machine_symbol),
        ProgramPoint::State { state_symbol, .. } => symbol(state_symbol),
        ProgramPoint::Call {
            state_symbol,
            statement_index,
            call_ordinal,
            ..
        } => format!(
            "{}:call-{statement_index}-{call_ordinal}",
            symbol(state_symbol)
        ),
        ProgramPoint::CallRequires {
            state_symbol,
            statement_index,
            call_ordinal,
            ..
        } => format!(
            "{}:call-requires-{statement_index}-{call_ordinal}",
            symbol(state_symbol)
        ),
        ProgramPoint::CallEnsures {
            state_symbol,
            statement_index,
            call_ordinal,
            ..
        } => format!(
            "{}:call-ensures-{statement_index}-{call_ordinal}",
            symbol(state_symbol)
        ),
        ProgramPoint::Exit {
            state_symbol,
            statement_index,
            ..
        } => format!("{}:exit-{statement_index}", symbol(state_symbol)),
        ProgramPoint::TransitionArm {
            state_symbol,
            statement_index,
            transition_target,
            ..
        } => format!(
            "{}:arm-{statement_index}-{}-{}",
            symbol(state_symbol),
            transition_target.arena_index(),
            transition_target.generation(),
        ),
        ProgramPoint::Statement {
            state_symbol,
            statement_index,
            ..
        } => format!("{}:statement-{statement_index}", symbol(state_symbol)),
    }
}
