//! Rewriting guarded call arms and their synthesizable argument calls.

use crate::lowering::statement::guarded_call_arguments;
use crate::lowering::statement::value_call_hoisting::{
    is_scalar_computation_state, is_scalar_return_computation,
};
use crate::resolution::lowerer::Lowerer;
use arena::HandleSpan;
use symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableNamePath,
};
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::statement::{
    NamedTransitionTarget, NamedTransitionTargetStorage, TransitionTarget,
};
use symbols::SymbolHandle;

/// Keep a guarded return call inside its selected arm. The continuation
/// captures referenced values, then evaluates the original arguments and call.
/// Place-dependent and nested effectful arguments need their own evaluation
/// plans and are not moved by this value-only normalization.
pub(crate) fn rewrite_guarded_call_arm(
    lowerer: &mut Lowerer,
    target: TransitionTarget,
) -> TransitionTarget {
    let TransitionTarget::Value(expression) = target else {
        return target;
    };
    if is_scalar_return_computation(lowerer, expression) {
        return TransitionTarget::Value(expression);
    }
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let ExpressionNode::Call(call) = expressions.expression(expression) else {
        return TransitionTarget::Value(expression);
    };
    if call.receiver.is_valid() || matches!(call.target.as_str(), "min" | "max" | "sqrt") {
        return TransitionTarget::Value(expression);
    }
    // Parentheses do not turn an authored state edge into a value call.
    // Resolution must retain its tail-transition classification, especially
    // for measured recursion; a generated let would make it non-tail.
    if lowerer.current_machine_name.as_deref() == Some(call.target.as_str())
        || lowerer
            .current_machine_state_names
            .iter()
            .any(|name| name == call.target.as_str())
    {
        return TransitionTarget::Value(expression);
    }
    let argument_handles = expressions.expression_handles(call.arguments).to_vec();
    let Some(parameters) = guarded_call_arguments::capture(lowerer, &argument_handles) else {
        return TransitionTarget::Value(expression);
    };
    let Some(return_type) = lowerer.current_state_return_type.clone() else {
        return TransitionTarget::Value(expression);
    };
    let state_name = lowerer.next_arm_state_name();
    lowerer
        .pending_synthesized_states
        .push(crate::resolution::lowerer::SynthesizedArmState {
            name: state_name.clone(),
            parameters: parameters.clone(),
            return_type,
            call: expression,
        });
    let mut path = HandleSpan::empty();
    lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .statement_path_members
        .append_to_span(&mut path, DiagnosticName::generated(state_name));
    // FRESH Name nodes for the target's arguments: the original handles
    // stay inside the synthesized state's call (where they resolve against
    // ITS parameters); sharing one node across two scopes would let the
    // second resolution overwrite the first's symbol.
    let expressions = &mut lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let mut arguments = HandleSpan::empty();
    for (name, _) in &parameters {
        let mut members = HandleSpan::empty();
        expressions.push_name_path_member(&mut members, DiagnosticName::generated(name.clone()));
        let member_symbols = expressions.reserve_name_path_member_symbols(members.count());
        let fresh = expressions.insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            is_self_value: false,
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }));
        expressions.push_expression_handle(&mut arguments, fresh);
    }
    TransitionTarget::Named(NamedTransitionTarget {
        head_symbol: SymbolHandle::invalid(),
        symbol: SymbolHandle::invalid(),
        storage: NamedTransitionTargetStorage {
            path,
            path_starts_at_self: false,
            arguments,
            evidence_arguments: Box::default(),
            source_span: Default::default(),
            authored_call_selection: None,
        },
    })
}

/// Move direct free/`self` value calls used as guarded named-target arguments
/// behind the selected arm. The generated state receives the source state's
/// referenced parameters, evaluates calls left-to-right into locals, then
/// performs the original transition. Unsupported captures (notably source
/// locals whose types are not retained here) keep the existing honest fence.
pub(crate) fn rewrite_guarded_transition_argument_calls(
    lowerer: &mut Lowerer,
    target: TransitionTarget,
) -> TransitionTarget {
    if is_scalar_computation_state(lowerer) {
        return target;
    }
    let TransitionTarget::Named(named) = target else {
        return target;
    };
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let argument_handles = expressions.expression_handles(named.storage.arguments);
    let mut calls = Vec::new();
    for argument in argument_handles {
        if collect_synthesizable_argument_calls(lowerer, *argument, &mut calls).is_none() {
            return TransitionTarget::Named(named);
        }
    }
    if calls.is_empty() {
        return TransitionTarget::Named(named);
    }
    // Evidence identifiers are scoped to the authored source state. Moving
    // this edge behind a synthesized runtime-argument state would orphan a
    // state-arrival term, so keep the exact edge intact and let the existing
    // downstream call-in-argument fence decide whether its runtime shape is
    // supported.
    if !named.evidence_arguments.is_empty() {
        return TransitionTarget::Named(named);
    }

    let mut captured_names = Vec::new();
    let mut uses_self = false;
    if !argument_handles.iter().all(|argument| {
        collect_synthesized_argument_captures(
            lowerer,
            *argument,
            &mut captured_names,
            &mut uses_self,
        )
    }) {
        return TransitionTarget::Named(named);
    }
    let self_parameter = uses_self
        .then(|| lowerer.current_state_self_parameter.clone())
        .flatten();
    if uses_self && self_parameter.is_none() {
        return TransitionTarget::Named(named);
    }
    let parameters = lowerer
        .current_state_parameters
        .iter()
        .filter(|(name, _, _)| captured_names.contains(name))
        .cloned()
        .chain(
            lowerer
                .current_state_locals
                .iter()
                .filter(|(name, _, _)| captured_names.contains(name))
                .cloned(),
        )
        .collect::<Vec<_>>();
    if parameters.len() != captured_names.len() {
        return TransitionTarget::Named(named);
    }

    let state_name = lowerer.next_arm_state_name();
    lowerer.pending_synthesized_transition_argument_states.push(
        crate::resolution::lowerer::SynthesizedTransitionArgumentState {
            name: state_name.clone(),
            self_parameter,
            parameters: parameters.clone(),
            return_type: lowerer.current_state_return_type.clone(),
            target: named,
            calls,
        },
    );

    let mut path = HandleSpan::empty();
    lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .statement_path_members
        .append_to_span(&mut path, DiagnosticName::generated(state_name));
    let expressions = &mut lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let mut arguments = HandleSpan::empty();
    for (name, _, _) in &parameters {
        let mut members = HandleSpan::empty();
        expressions.push_name_path_member(&mut members, DiagnosticName::generated(name.clone()));
        let member_symbols = expressions.reserve_name_path_member_symbols(members.count());
        let fresh = expressions.insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            is_self_value: false,
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }));
        expressions.push_expression_handle(&mut arguments, fresh);
    }
    TransitionTarget::Named(NamedTransitionTarget {
        head_symbol: SymbolHandle::invalid(),
        symbol: SymbolHandle::invalid(),
        storage: NamedTransitionTargetStorage {
            path,
            path_starts_at_self: false,
            arguments,
            evidence_arguments: Box::default(),
            source_span: Default::default(),
            authored_call_selection: None,
        },
    })
}

/// Collect direct free/`self` machine calls in evaluation order. Children are
/// visited first so a nested call result is materialized before its enclosing
/// call. A flat call prefix cannot represent conditional evaluation: refuse
/// to rewrite the entire target if a call belongs to a short-circuit RHS.
fn collect_synthesizable_argument_calls(
    lowerer: &Lowerer,
    expression: ExpressionHandle,
    calls: &mut Vec<ExpressionHandle>,
) -> Option<()> {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let mut visit = |child| collect_synthesizable_argument_calls(lowerer, child, calls);
    match expressions.expression(expression) {
        // Expression-local branches cannot become an unconditional source call prefix.
        ExpressionNode::Match(_) => return None,
        ExpressionNode::Atomic(atomic) => visit(atomic.value)?,
        ExpressionNode::ArrayLiteral(values) => {
            for value in expressions.expression_handles(*values) {
                visit(*value)?;
            }
        }
        ExpressionNode::Binary(binary) => {
            visit(binary.left)?;
            let mut right_calls = Vec::new();
            collect_synthesizable_argument_calls(lowerer, binary.right, &mut right_calls)?;
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                && !right_calls.is_empty()
            {
                return None;
            }
            calls.extend(right_calls);
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                visit(call.receiver)?;
            }
            for argument in expressions.expression_handles(call.arguments) {
                visit(*argument)?;
            }
            if matches!(call.target.as_str(), "min" | "max" | "sqrt") {
                return Some(());
            }
            if call.receiver.is_valid() {
                let ExpressionNode::Name(path) = expressions.expression(call.receiver) else {
                    return Some(());
                };
                if !matches!(expressions.name_path_members(path.members), [member] if member.as_str() == "self")
                {
                    return Some(());
                }
            }
            calls.push(expression);
        }
        ExpressionNode::Cast(cast) => visit(cast.value)?,
        ExpressionNode::Indexed(indexed) => {
            visit(indexed.collection)?;
            visit(indexed.index)?;
        }
        ExpressionNode::Member(member) => visit(member.receiver)?,
        ExpressionNode::Membership(membership) => visit(membership.value)?,
        ExpressionNode::Borrow(inner) => visit(inner.target)?,
        ExpressionNode::Range(range) => {
            visit(range.start)?;
            visit(range.end)?;
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in expressions.struct_fields(literal.fields) {
                visit(field.value)?;
            }
        }
        ExpressionNode::Unary(unary) => visit(unary.operand)?,
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
    Some(())
}

pub(crate) fn expression_contains_call(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let contains = |child| expression_contains_call(lowerer, child);
    match expressions.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            contains(dispatch.subject) || expressions.match_arms(dispatch.arms).iter().any(|arm| {
                matches!(arm.pattern, symbol_resolved_trees::expression::MatchPattern::Value(pattern) if contains(pattern))
                    || contains(arm.value)
            })
        }
        ExpressionNode::Call(_) => true,
        ExpressionNode::Atomic(atomic) => contains(atomic.value),
        ExpressionNode::ArrayLiteral(values) => expressions
            .expression_handles(*values)
            .iter()
            .any(|value| contains(*value)),
        ExpressionNode::Binary(binary) => contains(binary.left) || contains(binary.right),
        ExpressionNode::Cast(cast) => contains(cast.value),
        ExpressionNode::Indexed(indexed) => contains(indexed.collection) || contains(indexed.index),
        ExpressionNode::Member(member) => contains(member.receiver),
        ExpressionNode::Membership(membership) => contains(membership.value),
        ExpressionNode::Borrow(borrow) => contains(borrow.target),
        ExpressionNode::Range(range) => contains(range.start) || contains(range.end),
        ExpressionNode::StructLiteral(literal) => expressions
            .struct_fields(literal.fields)
            .iter()
            .any(|field| contains(field.value)),
        ExpressionNode::Unary(unary) => contains(unary.operand),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

fn collect_synthesized_argument_captures(
    lowerer: &Lowerer,
    expression: ExpressionHandle,
    captured_names: &mut Vec<String>,
    uses_self: &mut bool,
) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let mut visit =
        |child| collect_synthesized_argument_captures(lowerer, child, captured_names, uses_self);
    match expressions.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            visit(dispatch.subject)
                && expressions.match_arms(dispatch.arms).iter().all(|arm| {
                    let pattern_captured = match arm.pattern {
                        symbol_resolved_trees::expression::MatchPattern::Value(pattern) => {
                            visit(pattern)
                        }
                        symbol_resolved_trees::expression::MatchPattern::Wildcard => true,
                    };
                    pattern_captured && visit(arm.value)
                })
        }
        ExpressionNode::Atomic(atomic) => visit(atomic.value),
        ExpressionNode::ArrayLiteral(values) => expressions
            .expression_handles(*values)
            .iter()
            .all(|value| visit(*value)),
        ExpressionNode::Binary(binary) => visit(binary.left) && visit(binary.right),
        ExpressionNode::Call(call) => {
            (!call.receiver.is_valid() || visit(call.receiver))
                && expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .all(|argument| visit(*argument))
        }
        ExpressionNode::Cast(cast) => visit(cast.value),
        ExpressionNode::Indexed(indexed) => visit(indexed.collection) && visit(indexed.index),
        ExpressionNode::Member(member) => visit(member.receiver),
        ExpressionNode::Membership(membership) => visit(membership.value),
        ExpressionNode::Borrow(inner) => visit(inner.target),
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            if members
                .first()
                .is_some_and(|member| member.as_str() == "self")
            {
                *uses_self = true;
                return true;
            }
            let [name] = members else {
                return false;
            };
            let name = name.as_str();
            if !lowerer
                .current_state_parameters
                .iter()
                .any(|(parameter, _, _)| parameter == name)
                && !lowerer
                    .current_state_locals
                    .iter()
                    .any(|(local, _, _)| local == name)
            {
                return false;
            }
            if !captured_names.iter().any(|captured| captured == name) {
                captured_names.push(name.to_owned());
            }
            true
        }
        ExpressionNode::Range(range) => visit(range.start) && visit(range.end),
        ExpressionNode::StructLiteral(literal) => expressions
            .struct_fields(literal.fields)
            .iter()
            .all(|field| visit(field.value)),
        ExpressionNode::Unary(unary) => visit(unary.operand),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => true,
    }
}

/// Whether `expression` is a pure-builtin call (`min`/`max`/`sqrt`; `abs`/`clamp`
/// are already desugared to these) that Phase-1 guard hoisting materializes: a
/// free call (no receiver) whose FIRST argument is a `self.<field>` place, so the
/// synthetic temp's type is resolvable from that field. `abs(self.x)` desugars to
/// `max(self.x, 0 - self.x)` (first arg `self.x`, hoisted); `clamp(self.x, ..)`
/// desugars to `min(max(self.x, ..), ..)` whose first arg is a call, so it is
/// left alone (not hoisted) -- the temp would be untypeable.
pub(crate) fn is_hoistable_builtin_guard_call(
    lowerer: &Lowerer,
    expression: ExpressionHandle,
) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let ExpressionNode::Call(call) = expressions.expression(expression) else {
        return false;
    };
    if call.receiver.is_valid() {
        return false; // a method call, not a free builtin
    }
    if !matches!(call.target.as_str(), "min" | "max" | "sqrt") {
        return false;
    }
    let arguments = expressions.expression_handles(call.arguments);
    let Some(&first) = arguments.first() else {
        return false;
    };
    // The first argument must be a `self.<field>` member access -- the only place
    // shape `infer_hoist_temp_type` can type the temp from.
    let ExpressionNode::Member(member) = expressions.expression(first) else {
        return false;
    };
    matches!(
        expressions.expression(member.receiver),
        ExpressionNode::Name(path)
            if expressions
                .name_path_members(path.members)
                .first()
                .is_some_and(|name| name.as_str() == "self")
    )
}
