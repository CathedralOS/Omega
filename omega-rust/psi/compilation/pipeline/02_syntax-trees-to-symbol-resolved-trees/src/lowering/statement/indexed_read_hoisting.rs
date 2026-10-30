//! Hoisting runtime-indexed operand reads into synthetic locals.

use crate::lowering::expression::expression_table;
use crate::lowering::statement::hoist_temps::hoist_into_temp;
use crate::lowering::statement::match_subject_hoisting::{
    borrow_membership_subject, is_reference_struct_parameter_member,
};
use crate::lowering::statement::place_operand_capture::capture_address_operands;
use crate::lowering::statement::statement_nodes::set_expression;
use crate::resolution::lowerer::Lowerer;
use symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableCastExpression,
    TableMembershipExpression, TableRangeExpression, TableUnaryExpression,
};
use symbol_resolved_trees::statement::Statement;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperandHoisting {
    Value,
    Guard,
    /// Checked computations retain calls at their evaluation points.
    Computation,
}

/// Hoists every runtime-indexed read in OPERAND position out of `value`.
///
/// `value` is the ROOT of an assignment value, local initializer, or return.
/// A root-level `Indexed` is left in place (the existing whole-value copy path
/// handles it). For every OTHER node, each child that is an `Indexed`
/// (transitively) is hoisted into a fresh `let __hoist_N = <indexed>;` appended
/// to `hoisted`, and the child is replaced by a name referencing that temp.
/// Returns the root handle to use in the rewritten statement.
pub(crate) fn hoist_operand_indexed_reads(
    lowerer: &mut Lowerer,
    value: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
    mode: OperandHoisting,
) -> ExpressionHandle {
    // The root itself stays as-is, but rewrite its children so any nested
    // operand-position indexed read is hoisted.
    rewrite_children(lowerer, value, hoisted, mode);
    value
}

/// Rewrites the CHILDREN of `expression` in place, hoisting any child that is an
/// operand-position indexed read. Recurses so deeply nested cases work.
fn rewrite_children(
    lowerer: &mut Lowerer,
    expression: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
    mode: OperandHoisting,
) {
    let node = expression_table(lowerer).expression(expression).clone();
    match node {
        ExpressionNode::Binary(binary) => {
            let left = hoist_child(lowerer, binary.left, hoisted, mode);
            // A selective RHS is not part of the enclosing statement's eager
            // evaluation. Its calls and reads must remain behind selection;
            // checked computation planning materializes supported operands there.
            let right = if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                binary.right
            } else {
                hoist_child(lowerer, binary.right, hoisted, mode)
            };
            set_expression(
                lowerer,
                expression,
                ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }),
            );
        }
        ExpressionNode::Unary(unary) => {
            let operand = hoist_child(lowerer, unary.operand, hoisted, mode);
            set_expression(
                lowerer,
                expression,
                ExpressionNode::Unary(TableUnaryExpression {
                    operator: unary.operator,
                    operand,
                }),
            );
        }
        ExpressionNode::Cast(cast) => {
            // A §5b RECAST re-views its operand PLACE's bytes -- hoisting a
            // runtime-indexed operand into a value temp would destroy the
            // place (the view must address `buf[k]`, not a copied byte;
            // rung C1's runtime-offset judgment reads the raw Indexed
            // shape). Deeper hoistables INSIDE the index still hoist via
            // the index rewrite; only the top-level indexed READ is kept.
            let value = if cast.form.is_recast()
                && matches!(
                    expression_table(lowerer).expression(cast.value),
                    ExpressionNode::Indexed(_)
                ) {
                cast.value
            } else if mode == OperandHoisting::Value
                && !cast.form.is_recast()
                && is_hoistable_value_cast_call(lowerer, cast.value)
            {
                // A value-machine call directly beneath a value cast
                // (including erased domain qualification) must first
                // materialize through the ordinary let-bound call-result
                // route. Otherwise Cast(Call(..)) reaches native selection as
                // one compound operand and surrounding arithmetic can consume
                // the call scratch slot instead of the delivered result. This
                // is exactly the authored equivalent:
                //
                //   (convert(x) as T in Policy)
                //     -> let __hoist = convert(x);
                //        (__hoist as T in Policy)
                //
                // The callee's declared return types the synthetic local in
                // `infer_hoist_temp_type`. Checked computations and guards
                // instead retain the original call beneath the
                // cast: moving it here could run it before an earlier sibling.
                hoist_into_temp(lowerer, cast.value, hoisted)
            } else {
                hoist_child(lowerer, cast.value, hoisted, mode)
            };
            set_expression(
                lowerer,
                expression,
                ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type: cast.target_type,
                    target_label: cast.target_label,
                    domain: cast.domain,
                    semantic_domain: cast.semantic_domain,
                    semantic_domain_arguments: cast.semantic_domain_arguments,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    form: cast.form,
                }),
            );
        }
        ExpressionNode::Membership(membership) => {
            let value = if is_runtime_indexed_read(lowerer, membership.value) {
                let observed = borrow_membership_subject(lowerer, membership.value);
                hoist_into_temp(lowerer, observed, hoisted)
            } else {
                hoist_child(lowerer, membership.value, hoisted, mode)
            };
            set_expression(
                lowerer,
                expression,
                ExpressionNode::Membership(TableMembershipExpression {
                    value,
                    domain: membership.domain,
                    domain_symbol: membership.domain_symbol,
                    case_type_symbol: membership.case_type_symbol,
                    case_symbol: membership.case_symbol,
                }),
            );
        }
        // `&mut <place>` / `&<place>` borrows a PLACE, not a value. An indexed
        // read inside (`&mut self.entries[0]`) is the borrow TARGET, not an
        // operand to materialize -- hoisting it into a temp would borrow the
        // temp instead and silently change aliasing. Leave the whole subtree
        // untouched.
        ExpressionNode::Borrow(_) => {}
        ExpressionNode::Range(range) => {
            let start = if range.start.is_valid() {
                hoist_child(lowerer, range.start, hoisted, mode)
            } else {
                range.start
            };
            let end = if range.end.is_valid() {
                hoist_child(lowerer, range.end, hoisted, mode)
            } else {
                range.end
            };
            set_expression(
                lowerer,
                expression,
                ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                }),
            );
        }
        ExpressionNode::Indexed(_) => {
            // The value root remains storage. Its collection and selectors
            // evaluate in order through the same owner as loans and stores;
            // expression depth and local/parameter spelling are not admission
            // criteria. Selection boundaries above still keep skipped
            // operands out of this unconditional capture.
            capture_address_operands(lowerer, expression, hoisted);
        }
        // Array literals, calls, member accesses, struct literals, and the leaf
        // nodes (names, integers, ...) are left untouched: an operand-position
        // indexed read appears only as a direct child handled above, and these
        // forms either do not surface the blocker or are out of scope.
        _ => {}
    }
}

/// Whether a direct call beneath a non-recast value cast can use the let-bound
/// value-call result path. Proof-only integer embedding stays in its proof
/// expression because it has no runtime result slot.
fn is_hoistable_value_cast_call(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let Some(call) = expressions.call_node(expression) else {
        return false;
    };
    !call.receiver.is_valid() && !is_integer_embedding_call(lowerer, call)
}

/// A compiler proof term has no runtime call-result slot to materialize.
/// Keep its source expression intact for proof typing and ordered coercion
/// checks. Calls are initially unresolved at this normalization site, so the
/// reserved syntax is preserved without assigning it semantic authority. The
/// later proof-only gate requires the exact builtin symbol and rejects both
/// executable uses and any authored same-spelled replacement.
pub(crate) fn is_integer_embedding_call(
    lowerer: &Lowerer,
    call: &symbol_resolved_trees::expression::TableCallExpression,
) -> bool {
    !call.receiver.is_valid()
        && call.target.as_str() == symbols::BuiltinFunction::IntegerEmbed.name()
        && (!call.target_symbol.is_valid()
            || lowerer
                .symbol_resolved_trees
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
                == Some(symbols::BuiltinFunction::IntegerEmbed))
}

/// Hoists `child` if it is an `Indexed` node, otherwise recurses into it.
///
/// Returns the handle to use in the parent: a `Name` referencing the new temp
/// when hoisted, or the original handle (with its children rewritten) otherwise.
pub(crate) fn hoist_child(
    lowerer: &mut Lowerer,
    child: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
    mode: OperandHoisting,
) -> ExpressionHandle {
    // A member read through a shared reference-to-struct PARAM
    // (`table.con_out`) must dereference the pointer slot; left in operand or
    // guard position it folds flat (slot + field offset in the FRAME) and
    // silently reads garbage -- the entry-ref-param face. Hoisting it into a
    // `let` routes it through the boot-verified pointee-copy path. The param's
    // `&Named` type is DECLARED on the state signature, so this predicate is
    // not type-blind.
    if is_reference_struct_parameter_member(lowerer, child) {
        return hoist_into_temp(lowerer, child, hoisted);
    }

    if is_runtime_indexed_read(lowerer, child) {
        // Rewrite the indexed read's OWN index first (nested `arr[other[i]]`),
        // then hoist the whole indexed read into a fresh temp.
        rewrite_children(lowerer, child, hoisted, mode);
        return hoist_into_temp(lowerer, child, hoisted);
    }

    // A FIELD read of a runtime-indexed ELEMENT (`cells[k].v`) is the same
    // operand shape one field deeper: unhoisted, it reaches state-values with
    // no operand lowering and blocks ("needs runtime value lowering"). Hoist
    // the WHOLE member chain -- the temp's materialization resolves the
    // element field through the machine-indexed copy (field_byte_offset), the
    // same path a transition argument uses. VALUE positions ONLY
    // (`Guard` marks the guard path): a guard's comparison
    // subject is hoisted ONCE and SHARED across arms by
    // `hoist_comparison_match_subject`, and a per-arm hoist here would split
    // the subject into distinct temps, un-pairing the `true`/`false` arms
    // (exhaustiveness then reports a fall-through on working guards).
    if mode != OperandHoisting::Guard && is_member_of_runtime_indexed_read(lowerer, child) {
        rewrite_children(lowerer, child, hoisted, mode);
        return hoist_into_temp(lowerer, child, hoisted);
    }

    // Not a runtime-indexed read: descend so deeper operand-position runtime
    // indexed reads (`(a + arr[i]) * b`) are still hoisted.
    rewrite_children(lowerer, child, hoisted, mode);
    child
}

/// Whether `expression` is an `Indexed` read whose index is NOT a constant
/// integer -- the RUNTIME-indexed case that needs operand hoisting. A
/// constant-index read (`arr[0]`) lowers as a plain place path and is left
/// alone (so existing whole-value copies / borrows are untouched).
/// Whether `expression` is a MEMBER chain whose receiver bottoms out at a
/// runtime-indexed read of a MACHINE-owned collection (`self.cells[k].v`).
/// Restricted to `self.<field>` collections because the hoist temp's type is
/// inferred from the machine's attached data (`infer_hoist_temp_type`); a
/// LOCAL array's element field would mint an untypeable Unit temp AND break
/// the local-array RMW write path that pattern-matches the unhoisted read.
fn is_member_of_runtime_indexed_read(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let Some(member) = expressions.member_node(expression) else {
        return false;
    };
    let mut receiver = member.receiver;
    while let Some(inner) = expressions.member_node(receiver) {
        receiver = inner.receiver;
    }
    if !is_runtime_indexed_read(lowerer, receiver) {
        return false;
    }
    let Some(indexed) = expressions.indexed_node(receiver) else {
        return false;
    };
    // The collection must be a `self.<field>` place (typeable from attached data).
    match expressions.expression(indexed.collection) {
        ExpressionNode::Member(collection_member) => matches!(
            expressions.expression(collection_member.receiver),
            ExpressionNode::Name(path)
                if expressions
                    .name_path_members(path.members)
                    .first()
                    .is_some_and(|name| name.is_self_receiver())
        ),
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            members.len() == 2 && members[0].is_self_receiver()
        }
        _ => false,
    }
}

pub(crate) fn is_runtime_indexed_read(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let Some(indexed) = expressions.indexed_node(expression) else {
        return false;
    };
    !matches!(
        expressions.expression(indexed.index),
        ExpressionNode::Integer(_)
    )
}
