//! Hoisting scalar value calls and terminal value machine calls.

use crate::lowering::statement::guarded_arm_rewrites::expression_contains_call;
use crate::lowering::statement::indexed_read_hoisting::is_integer_embedding_call;
use crate::lowering::statement::statement_nodes::set_expression;
use crate::resolution::lowerer::Lowerer;
use arena::HandleSpan;
use symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableNamePath,
};
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::statement::{LocalData, LocalDataStorage, Statement};
use symbol_resolved_trees::types::TypeReference;
use symbols::SymbolHandle;
use syntax_trees::{self as syntax, SyntaxTrees};

/// Hoists a USER value-machine call out of a guard comparison:
/// `self.next() == expected` becomes
/// `let __hoist_N = self.next(); __hoist_N == expected`. Fires only when the
/// guard ROOT is a comparison with one user Call side and one non-user-Call
/// side (builtin min/max/sqrt calls keep their own dedicated hoist below). The
/// temp's type is resolved from the callee's DECLARED return by the
/// symbol-resolved -> typed lowering
/// (`infer_hoist_temp_type`); an inferred-return callee gets a clear
/// annotate-or-bind diagnostic there.
pub(crate) fn hoist_scalar_value_call_comparison(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    syntax_guard: syntax::statement::TransitionGuardNode,
    guard_expression: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) {
    let syntax::statement::TransitionGuardNode::When(mut syntax_cmp) = syntax_guard else {
        return;
    };
    // Peel bool-arm wrappers in LOCKSTEP on both trees: a bool-arm dispatch
    // wraps the subject per arm (`(dbl(5) == 11) == true`), and a match-over-
    // call arm arrives directly as `roll(..) == <arm literal>` with the SAME
    // syntax subject handle shared across arms.
    let mut resolved_cmp = guard_expression;
    let binary = loop {
        let node = lowerer
            .symbol_resolved_trees
            .tables
            .bodies
            .expressions
            .expression(resolved_cmp)
            .clone();
        let ExpressionNode::Binary(binary) = node else {
            return;
        };
        if matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) && matches!(
            lowerer
                .symbol_resolved_trees
                .tables
                .bodies
                .expressions
                .expression(binary.right),
            ExpressionNode::Boolean(_)
        ) {
            let syntax::expression::ExpressionNode::Binary(syntax_outer) =
                syntax_trees.expressions.expression(syntax_cmp)
            else {
                return;
            };
            resolved_cmp = binary.left;
            syntax_cmp = syntax_outer.left;
            continue;
        }
        break binary;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
    ) {
        return;
    }
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let side_is_user_call = |handle: ExpressionHandle| match expressions.expression(handle) {
        ExpressionNode::Call(call) => {
            !is_integer_embedding_call(lowerer, call)
                && !matches!(call.target.as_str(), "min" | "max" | "sqrt")
        }
        _ => false,
    };
    let call_is_left = side_is_user_call(binary.left) && !side_is_user_call(binary.right);
    let call_is_right = side_is_user_call(binary.right) && !side_is_user_call(binary.left);
    if !call_is_left && !call_is_right {
        return;
    }
    let other_side = if call_is_left {
        binary.right
    } else {
        binary.left
    };
    // A cast or nested expression does not hide its call's evaluation point.
    // Keep both operands intact rather than moving one call ahead of the other.
    // Literal match arms still share the existing single subject binding.
    if expression_contains_call(lowerer, other_side) {
        return;
    }
    // A right-side call cannot move ahead of an earlier read or operation:
    // even without another call, that operand can observe storage or trap.
    if call_is_right
        && !matches!(
            expressions.expression(binary.left),
            ExpressionNode::Boolean(_) | ExpressionNode::Float(_) | ExpressionNode::Integer(_)
        )
    {
        return;
    }
    let call_side = if call_is_left {
        binary.left
    } else {
        binary.right
    };

    // The memo key is the CALL's SYNTAX handle: a match over a call subject
    // (`transition self.roll(t) { 1 -> .. 2 -> .. }`) lowers one comparison
    // PER ARM over the SAME syntax subject, and every arm must share ONE temp
    // -- per-arm temps re-run the callee once per attempted arm (the
    // effectful-subject single-evaluation tripwire).
    let syntax_call = match syntax_trees.expressions.expression(syntax_cmp) {
        syntax::expression::ExpressionNode::Binary(syntax_binary) => {
            if call_is_left {
                syntax_binary.left
            } else {
                syntax_binary.right
            }
        }
        // A match-over-call arm whose SYNTAX guard is the bare subject (the
        // arm value is synthesized): the subject itself is the call.
        _ => syntax_cmp,
    };
    let subject_key = syntax_call.arena_index();

    let name = match lowerer.match_subject_temp(subject_key) {
        Some(existing) => DiagnosticName::generated(existing),
        None => {
            let fresh = lowerer.next_hoist_name();
            lowerer.record_match_subject_temp(subject_key, fresh.clone());
            let name = DiagnosticName::generated(fresh);
            hoisted.push(Statement::LocalData(LocalData {
                symbol: SymbolHandle::invalid(),
                name: name.clone(),
                storage: LocalDataStorage {
                    // Unit is the inference sentinel; the symbol-resolved ->
                    // typed lowering types the temp from the callee's DECLARED
                    // return (`infer_hoist_temp_type`'s Call branch).
                    type_reference: TypeReference::Unit,
                    initial_value: call_side,
                    is_mutable: false,
                    type_is_inferred: true,
                    relevance: language_core::BindingRelevance::Relevant,
                },
            }));
            name
        }
    };

    let mut members = HandleSpan::empty();
    lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .push_name_path_member(&mut members, name);
    let member_symbols = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .reserve_name_path_member_symbols(members.count());
    let name_reference = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            is_self_value: false,
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }));
    let rewritten = TableBinaryExpression {
        left: if call_is_left {
            name_reference
        } else {
            binary.left
        },
        operator: binary.operator,
        right: if call_is_right {
            name_reference
        } else {
            binary.right
        },
    };
    set_expression(lowerer, resolved_cmp, ExpressionNode::Binary(rewritten));
}

/// A free or direct-self value-machine call (`sin(x)` or `self.finish()`; not a
/// pure builtin) as a transition's TERMINAL VALUE (or a state's trailing
/// implicit return) has no dispatch return route: an acyclic callee poisons at the
/// unlowered-terminal fence and a cyclic one refuses at the
/// binding-substitution depth cap (neither ever lowered -- probed 2026-07-11,
/// so this rewrite cannot regress a served shape). The let-bound spelling is
/// the fully served path, so make it automatic: hoist the call into a
/// `let __hoist_N` temp (typed from the callee's DECLARED return by
/// `infer_hoist_temp_type`'s Call branch) and deliver the local. Callers gate
/// transition targets to ALWAYS-guard arms: a hoisted statement runs whenever
/// control reaches it, and hoisting out of a guarded arm would run an
/// effectful callee even when the arm is not taken (trailing returns are
/// unconditional, so they hoist unconditionally).
pub(crate) fn hoist_terminal_value_machine_call(
    lowerer: &mut Lowerer,
    expression: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) -> ExpressionHandle {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let ExpressionNode::Call(call) = expressions.expression(expression) else {
        return expression;
    };
    if is_integer_embedding_call(lowerer, call)
        || matches!(call.target.as_str(), "min" | "max" | "sqrt")
    {
        return expression;
    }
    if call.receiver.is_valid() {
        let ExpressionNode::Name(path) = expressions.expression(call.receiver) else {
            return expression;
        };
        if !matches!(expressions.name_path_members(path.members), [member] if member.as_str() == "self")
        {
            // Contained/dynamic/proof receivers already have their own return
            // routes, and their selected requirement is not a concrete state
            // whose declared result `infer_hoist_temp_type` can copy. Only a
            // direct `self.machine()` sibling call uses this normalization.
            return expression;
        }
    }
    let name = DiagnosticName::generated(lowerer.next_hoist_name());
    hoisted.push(Statement::LocalData(LocalData {
        symbol: SymbolHandle::invalid(),
        name: name.clone(),
        storage: LocalDataStorage {
            // Unit is the inference sentinel; the symbol-resolved -> typed
            // lowering types the temp from the callee's declared return.
            type_reference: TypeReference::Unit,
            initial_value: expression,
            is_mutable: false,
            type_is_inferred: true,
            relevance: language_core::BindingRelevance::Relevant,
        },
    }));
    let expressions = &mut lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let mut members = HandleSpan::empty();
    expressions.push_name_path_member(&mut members, name);
    let member_symbols = expressions.reserve_name_path_member_symbols(members.count());
    expressions.insert(ExpressionNode::Name(TableNamePath {
        members,
        member_symbols,
        is_self_value: false,
        head_symbol: SymbolHandle::invalid(),
        symbol: SymbolHandle::invalid(),
    }))
}

/// Preserve the free scalar spelling for checked return computation planning.
/// This is only a normalization boundary: typing and checked call custody still
/// select the exact callee and reject unsupported operands before publication.
pub(crate) fn is_scalar_computation_state(lowerer: &Lowerer) -> bool {
    let scalar_type = |type_reference: &TypeReference| {
        let mut type_reference = type_reference;
        while let TypeReference::Constrained(constrained) = type_reference {
            type_reference = lowerer
                .symbol_resolved_trees
                .child_type_reference(constrained.base_type);
        }
        // Floating values have the same expression/call boundary as integer
        // scalars. Hoisting only those calls invents a local result in constant
        // probes and loses the authored expression before typed evaluation.
        type_reference.primitive_type().is_some()
    };
    lowerer.current_state_self_parameter.is_none()
        && lowerer
            .current_state_return_type
            .as_ref()
            .is_some_and(scalar_type)
        && lowerer
            .current_state_parameters
            .iter()
            .all(|(_, type_reference, is_mutable)| {
                !is_mutable
                    && (scalar_type(type_reference)
                        // Retain owned named inputs for checked mixed computation
                        // admission. This syntax boundary grants no ownership,
                        // qualification, cleanup, or termination authority.
                        || (matches!(type_reference, TypeReference::Named { .. })
                            && type_reference.primitive_type().is_none())
                        || matches!(type_reference, TypeReference::Reference(reference)
                            if scalar_type(lowerer.symbol_resolved_trees.child_type_reference(reference.referee))))
            })
}

pub(crate) fn is_scalar_return_computation(
    lowerer: &Lowerer,
    expression: ExpressionHandle,
) -> bool {
    if !is_scalar_computation_state(lowerer) {
        return false;
    }
    let ExpressionNode::Call(call) = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(expression)
    else {
        return false;
    };
    // Static arguments belong to ordinary checked specialization, not a
    // different expression schedule. Hoisting them here loses the scalar
    // constant probe before its complete application can be checked.
    !call.receiver.is_valid()
        && !is_integer_embedding_call(lowerer, call)
        && !matches!(call.target.as_str(), "min" | "max" | "sqrt")
}
