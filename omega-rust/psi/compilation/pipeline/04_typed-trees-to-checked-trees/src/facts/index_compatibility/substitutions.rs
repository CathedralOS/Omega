//! Call and fact substitutions: enclosing-call and value-call argument
//! substitution, established index equalities, and substituted-expression
//! matching.

use super::index_arguments_structurally_equal;
use super::index_terms::{
    IndexTerm, OpenNameMatch, bound_index_matches_expression, expression_name_atom,
    index_argument_term, index_leaf_term, name_paths_equal, named_integer_value,
    open_index_matches_expression,
};
use super::instance_collection::StateCallIndex;
use arena::HandleSpan;
use checked_trees::{FlowFacts, FlowSemanticContextRef};
use facts::{FactHandle, FactPayload, FactPlan, ProgramPoint};
use language_semantics::const_value::boolean_literal_spelling;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, StaticMachineArgument,
};
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Copy)]
pub(super) struct ExpressionSubstitution<'program> {
    pub(super) symbol: SymbolHandle,
    pub(super) name: &'program str,
    pub(super) value: ExpressionHandle,
}

pub(super) fn established_index_equalities(
    program: &TypedTrees,
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
    program: &TypedTrees,
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
    program: &TypedTrees,
    expression: ExpressionHandle,
    substitutions: &[ExpressionSubstitution<'_>],
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    bound_substitutions: &BoundIndexSubstitutions,
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
    let Some(call_site) = crate::semantic::calls::find_call_site(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    ) else {
        return Vec::new();
    };
    let target_symbol = match &call_site {
        crate::semantic::calls::CallSite::Statement(call) => call.target_symbol,
        crate::semantic::calls::CallSite::Expression { call, .. } => {
            crate::flow::call_target_symbol(program, call)
        }
        crate::semantic::calls::CallSite::TransitionNamed { .. } => call_flow_at_point(flow, point)
            .map_or_else(SymbolHandle::invalid, |call| call.target_symbol),
    };
    let Some(parameters) = crate::semantic::calls::call_target_parameters(program, target_symbol)
    else {
        return Vec::new();
    };
    let arguments = crate::semantic::calls::call_site_argument_expressions(program, &call_site);
    parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, value)| ExpressionSubstitution {
            symbol: parameter.symbol,
            name: parameter.name.as_str(),
            value: *value,
        })
        .collect()
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

/// Whether two index arguments are spelled identically, before any call
/// A symbolic call to a generic target compares the callee's declared
/// `Coordinate<I>` against the caller's instance as the bound index: `I` is
/// the template's spelling, and this records what this call supplied.
/// One index argument a call bound to its callee's declared index binder.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        crate::semantic::calls::call_target_type_parameters(program, target_symbol);
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
pub(super) fn bound_index_argument(
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
pub(super) fn resolve_scope_subject(
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
    crate::lookup::nearest_scoped_local(program, state, scope_limit, argument.path.as_ref())
}

/// A call's index substitutions: one entry pairing each of the callee's
/// const-position binders with the caller-side index bound at this site.
pub(crate) type BoundIndexSubstitutions = Vec<(SymbolHandle, BoundIndexArgument)>;

/// The substitutions implied by the call at `point`, if `point` is a call
/// boundary — this is what lets an argument in `Coordinate<N>` discharge a
/// parameter declared `in Coordinate<I>` when the call bound `I` to `N`.
pub(super) fn enclosing_call_substitutions(
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
        crate::semantic::calls::CallSite::Statement(call) => {
            (call.target_symbol, call.machine_arguments.as_ref())
        }
        crate::semantic::calls::CallSite::Expression { call, .. } => (
            crate::flow::call_target_symbol(program, call),
            call.machine_arguments.as_ref(),
        ),
        crate::semantic::calls::CallSite::TransitionNamed { .. } => return Vec::new(),
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
pub(super) fn value_call_substitutions(
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
        crate::flow::call_target_symbol(program, call),
        &call.machine_arguments,
    )
}

/// One side of an index comparison after the call's binder substitutions.
/// parameters substituted, denotes the index `argument` spells under the
/// enclosing call's binder substitutions.
pub(super) fn substituted_expression_matches_index_argument(
    program: &TypedTrees,
    expression: ExpressionHandle,
    substitutions: &[ExpressionSubstitution<'_>],
    argument: TypeReferenceHandle,
    bound_substitutions: &BoundIndexSubstitutions,
) -> bool {
    let operand = || substituted_root(program, expression, substitutions).unwrap_or(expression);
    match index_argument_term(program, argument, bound_substitutions) {
        // The named position is a callee binder the enclosing call bound: it
        // denotes the bound index, not its own spelling.
        IndexTerm::Bound(bound) => bound_index_matches_expression(program, bound, operand()),
        IndexTerm::Open { symbol, name } => open_index_matches_expression(
            program,
            symbol,
            name,
            operand(),
            OpenNameMatch::ResolvedSymbolFirst,
        ),
        IndexTerm::Expression(expected) => substituted_expressions_equal(
            program,
            expression,
            substitutions,
            expected,
            bound_substitutions,
        ),
        IndexTerm::Opaque(_) => false,
    }
}

pub(super) fn substituted_root(
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

pub(super) fn substituted_expressions_equal(
    program: &TypedTrees,
    authored: ExpressionHandle,
    substitutions: &[ExpressionSubstitution<'_>],
    local: ExpressionHandle,
    bound_substitutions: &BoundIndexSubstitutions,
) -> bool {
    if authored == local {
        return true;
    }
    if !authored.is_valid() || !local.is_valid() {
        return false;
    }
    if let Some(substituted) = substituted_root(program, authored, substitutions) {
        return substituted_expressions_equal(
            program,
            substituted,
            &[],
            local,
            bound_substitutions,
        );
    }
    // A leaf on the compared index side can name a callee binder the
    // enclosing call bound to a caller-side index; the leaf denotes the
    // bound index, not its own spelling.
    if let IndexTerm::Bound(bound) = index_leaf_term(program, local, bound_substitutions) {
        return bound_index_matches_expression(program, bound, authored);
    }
    if substitutions.is_empty()
        && bound_substitutions.is_empty()
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
            expression_name_atom(program, right).and_then(boolean_literal_spelling) == Some(*left)
        }
        (ExpressionNode::String(left), ExpressionNode::String(right)) => left == right,
        (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            name_paths_equal(program, left, right)
        }
        (ExpressionNode::Borrow(left), ExpressionNode::Borrow(right)) => {
            left.access == right.access
                && substituted_expressions_equal(
                    program,
                    left.target,
                    substitutions,
                    right.target,
                    bound_substitutions,
                )
        }
        (ExpressionNode::Unary(left), ExpressionNode::Unary(right)) => {
            left.operator == right.operator
                && substituted_expressions_equal(
                    program,
                    left.operand,
                    substitutions,
                    right.operand,
                    bound_substitutions,
                )
        }
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            left.operator == right.operator
                && substituted_expressions_equal(
                    program,
                    left.left,
                    substitutions,
                    right.left,
                    bound_substitutions,
                )
                && substituted_expressions_equal(
                    program,
                    left.right,
                    substitutions,
                    right.right,
                    bound_substitutions,
                )
        }
        (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
            substituted_expressions_equal(
                program,
                left.collection,
                substitutions,
                right.collection,
                bound_substitutions,
            ) && substituted_expressions_equal(
                program,
                left.index,
                substitutions,
                right.index,
                bound_substitutions,
            )
        }
        (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
            left.member_symbol == right.member_symbol
                && left.member.as_str() == right.member.as_str()
                && substituted_expressions_equal(
                    program,
                    left.receiver,
                    substitutions,
                    right.receiver,
                    bound_substitutions,
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
                    bound_substitutions,
                )
                && left_arguments.len() == right_arguments.len()
                && left_arguments
                    .iter()
                    .zip(right_arguments)
                    .all(|(left, right)| {
                        substituted_expressions_equal(
                            program,
                            *left,
                            substitutions,
                            *right,
                            bound_substitutions,
                        )
                    })
        }
        (ExpressionNode::ArrayLiteral(left), ExpressionNode::ArrayLiteral(right)) => {
            let left = program.expression_table.expression_handles(*left);
            let right = program.expression_table.expression_handles(*right);
            left.len() == right.len()
                && left.iter().zip(right).all(|(left, right)| {
                    substituted_expressions_equal(
                        program,
                        *left,
                        substitutions,
                        *right,
                        bound_substitutions,
                    )
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
                            bound_substitutions,
                        )
                })
        }
        _ => false,
    }
}
