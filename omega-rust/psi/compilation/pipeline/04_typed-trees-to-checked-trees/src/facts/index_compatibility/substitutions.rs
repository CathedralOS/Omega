//! Call and fact substitutions: enclosing-call and value-call argument
//! substitution, established index equalities, and substituted-expression
//! matching.

use super::index_terms::{index_arguments_equal, index_identity_bindings};
use super::instance_collection::StateCallIndex;
use arena::HandleSpan;
use checked_trees::{FlowFacts, FlowSemanticContextRef};
use facts::{FactHandle, FactPayload, FactPlan, ProgramPoint};
use symbols::SymbolHandle;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, StaticMachineArgument,
};
use typed_trees::type_identity::{IndexIdentityArgument, normalized_index_identity};
use typed_trees::types::TypeReferenceHandle;

pub(super) fn established_index_equalities(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    flow: &FlowFacts,
    contexts: HandleSpan<FlowSemanticContextRef>,
    actual: &[TypeReferenceHandle],
    expected: &[TypeReferenceHandle],
    bound_substitutions: &BoundIndexSubstitutions,
) -> Option<Vec<FactHandle>> {
    if actual.len() != expected.len() {
        return None;
    }
    let bindings = index_identity_bindings(bound_substitutions);
    let differing = actual
        .iter()
        .zip(expected)
        .filter(|(left, right)| !index_arguments_equal(program, **left, **right, &bindings))
        .collect::<Vec<_>>();
    if differing.is_empty() {
        return None;
    }

    let mut evidence = Vec::new();
    for (actual, expected) in differing {
        let fact = established_index_equality_for_argument(
            program,
            semantic,
            flow,
            contexts,
            *actual,
            *expected,
            bound_substitutions,
        )?;
        if !evidence.contains(&fact) {
            evidence.push(fact);
        }
    }
    Some(evidence)
}

pub(super) fn established_index_equality_for_argument(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    flow: &FlowFacts,
    contexts: HandleSpan<FlowSemanticContextRef>,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    bound_substitutions: &BoundIndexSubstitutions,
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
                bound_substitutions,
            ) {
                return Some(fact_ref.fact);
            }
        }
    }
    None
}

pub(super) fn expression_proves_index_equality(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    substitutions: &BoundIndexSubstitutions,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    bound_substitutions: &BoundIndexSubstitutions,
) -> bool {
    let Some(binary) = program.expression_table.binary_node(expression) else {
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
            bound_substitutions,
        ) || expression_proves_index_equality(
            program,
            binary.right,
            substitutions,
            actual,
            expected,
            bound_substitutions,
        );
    }
    if binary.operator != BinaryOperator::Equal {
        return false;
    }
    let direct = substituted_expression_matches_index_argument(
        program,
        binary.left,
        substitutions,
        actual,
        bound_substitutions,
    ) && substituted_expression_matches_index_argument(
        program,
        binary.right,
        substitutions,
        expected,
        bound_substitutions,
    );
    let symmetric = substituted_expression_matches_index_argument(
        program,
        binary.left,
        substitutions,
        expected,
        bound_substitutions,
    ) && substituted_expression_matches_index_argument(
        program,
        binary.right,
        substitutions,
        actual,
        bound_substitutions,
    );
    direct || symmetric
}

pub(super) fn fact_substitutions<'program>(
    program: &'program dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    point: ProgramPoint,
) -> BoundIndexSubstitutions<'program> {
    let ProgramPoint::CallEnsures {
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    } = point
    else {
        return Vec::new();
    };
    let Some(call_site) = validation::find_call_site(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    ) else {
        return Vec::new();
    };
    let target_symbol = match &call_site {
        validation::CallSite::Statement(call) => call.target_symbol,
        validation::CallSite::Expression { call, .. } => {
            validation::call_target_symbol(program, call)
        }
        validation::CallSite::TransitionNamed { .. } => call_flow_at_point(flow, point)
            .map_or_else(SymbolHandle::invalid, |call| call.target_symbol),
    };
    let Some(parameters) = validation::call_target_parameters(program, target_symbol) else {
        return Vec::new();
    };
    let arguments = validation::call_site_argument_expressions(program, &call_site);
    let application = match &call_site {
        validation::CallSite::Statement(call) => call.authored_machine_application,
        validation::CallSite::Expression { call, .. } => call.authored_machine_application,
        validation::CallSite::TransitionNamed { .. } => {
            typed_trees::static_applications::StaticMachineApplicationHandle::invalid()
        }
    };
    let mut substitutions = bound_index_substitutions(program, application);
    substitutions.extend(
        parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, value)| (parameter.symbol, BoundIndexArgument::Expression(*value))),
    );
    substitutions
}

pub(super) fn call_flow_at_point(
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

/// Compatibility uses the producer's exact binder associations, not another
/// walk over argument spellings or a reconstruction of the erased telescope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundIndexArgument<'program> {
    Reference(TypeReferenceHandle),
    Subject(SymbolHandle),
    Algebra(&'program StaticMachineArgument),
    Expression(ExpressionHandle),
}

pub(crate) type BoundIndexSubstitutions<'program> =
    Vec<(SymbolHandle, BoundIndexArgument<'program>)>;

pub(crate) fn bound_index_substitutions(
    program: &dyn ::validation::CheckingInput,
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
) -> BoundIndexSubstitutions<'_> {
    use typed_trees::static_applications::StaticMachineBindingValue;
    if !application.is_valid() {
        return Vec::new();
    }
    program
        .expression_table
        .static_machine_application_bindings(application)
        .iter()
        .filter_map(|binding| {
            Some((
                binding.parameter,
                match &binding.value {
                    StaticMachineBindingValue::Const(reference) => {
                        BoundIndexArgument::Reference(*reference)
                    }
                    StaticMachineBindingValue::RuntimeValue { subject, .. }
                        if subject.is_valid() =>
                    {
                        BoundIndexArgument::Subject(*subject)
                    }
                    StaticMachineBindingValue::Conformance(argument) => {
                        BoundIndexArgument::Algebra(argument)
                    }
                    _ => return None,
                },
            ))
        })
        .collect()
}

/// The substitutions implied by the call at `point`, if `point` is a call
/// boundary — this is what lets an argument in `Coordinate<N>` discharge a
/// parameter declared `in Coordinate<I>` when the call bound `I` to `N`.
pub(super) fn enclosing_call_substitutions<'program>(
    program: &'program dyn ::validation::CheckingInput,
    state_calls: &StateCallIndex<'_, '_>,
    point: ProgramPoint,
) -> BoundIndexSubstitutions<'program> {
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
    let application = match &resolved.site {
        validation::CallSite::Statement(call) => call.authored_machine_application,
        validation::CallSite::Expression { call, .. } => call.authored_machine_application,
        validation::CallSite::TransitionNamed { .. } => return Vec::new(),
    };
    bound_index_substitutions(program, application)
}

/// The substitutions implied by `value` itself when the checked value is a
/// call whose declared result carries the callee's binders — a call result
/// `Coordinate<I>` compares as `Coordinate<N>` under the call's own binding.
pub(super) fn value_call_substitutions(
    program: &dyn ::validation::CheckingInput,
    value: ExpressionHandle,
) -> BoundIndexSubstitutions<'_> {
    let mut node = value;
    let call = loop {
        match program.expression_table.expression(node) {
            ExpressionNode::Borrow(inner) => node = inner.target,
            ExpressionNode::Atomic(atomic) => node = atomic.result,
            ExpressionNode::Call(call) => break call,
            _ => return Vec::new(),
        }
    };
    bound_index_substitutions(program, call.authored_machine_application)
}

/// Substitute exact fact parameters and call indices in their respective scopes,
/// then compare through the canonical selected-operation normalizer. Established
/// facts discharge compatibility; they never rewrite the declared index identity.
pub(super) fn substituted_expression_matches_index_argument(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    substitutions: &BoundIndexSubstitutions,
    argument: TypeReferenceHandle,
    bound_substitutions: &BoundIndexSubstitutions,
) -> bool {
    let fact_bindings = index_identity_bindings(substitutions);
    let index_bindings = index_identity_bindings(bound_substitutions);
    let fact_identity = normalized_index_identity(
        program,
        IndexIdentityArgument::Expression(expression),
        &fact_bindings,
    );
    let index_identity = normalized_index_identity(
        program,
        IndexIdentityArgument::TypeReference(argument),
        &index_bindings,
    );
    fact_identity
        .zip(index_identity)
        .is_some_and(|(fact, index)| fact == index)
}
