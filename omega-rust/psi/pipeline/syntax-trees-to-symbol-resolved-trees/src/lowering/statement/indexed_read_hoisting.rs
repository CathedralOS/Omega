//! Hoisting runtime-indexed operand reads into synthetic locals.

use crate::lowering::statement::match_subject_hoisting::{
    borrow_membership_subject, is_reference_struct_parameter_member,
};
use crate::lowering::statement::statement_nodes::set_expression;
use crate::resolution::lowerer::Lowerer;
use arena::HandleSpan;
use symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableCastExpression,
    TableIndexedExpression, TableMembershipExpression, TableNamePath, TableRangeExpression,
    TableUnaryExpression,
};
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::statement::{LocalData, LocalDataStorage, Statement};
use symbol_resolved_trees::types::TypeReference;
use symbols::SymbolHandle;

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
    let node = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(expression)
        .clone();
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
                    lowerer
                        .symbol_resolved_trees
                        .tables
                        .bodies
                        .expressions
                        .expression(cast.value),
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
        ExpressionNode::Indexed(indexed) => {
            // A NON-root indexed node reached via recursion would already have
            // been hoisted by `hoist_child`; reaching here means this is the
            // value root. Leave it whole (the root whole-value copy path), but
            // still rewrite its index sub-expression so a runtime-indexed read
            // INSIDE the index (`arr[other[i]]`) is hoisted.
            let index = hoist_index(lowerer, indexed.index, hoisted, mode);
            set_expression(
                lowerer,
                expression,
                ExpressionNode::Indexed(TableIndexedExpression {
                    collection: indexed.collection,
                    index,
                }),
            );
        }
        // Array literals, calls, member accesses, struct literals, and the leaf
        // nodes (names, integers, ...) are left untouched: an operand-position
        // indexed read appears only as a direct child handled above, and these
        // forms either do not surface the blocker or are out of scope.
        _ => {}
    }
}

/// Whether a direct call beneath a non-recast value cast can use the let-bound
/// value-call result path. Pure compiler builtins keep their existing
/// expression lowering because their synthetic result type is inferred from an
/// operand rather than a declared machine return.
fn is_hoistable_value_cast_call(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let ExpressionNode::Call(call) = expressions.expression(expression) else {
        return false;
    };
    !call.receiver.is_valid()
        && !is_integer_embedding_call(lowerer, call)
        && !matches!(call.target.as_str(), "min" | "max" | "sqrt")
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

/// Rewrites an `Indexed` node's INDEX position. A hoistable COMPUTED index
/// (`arr[k + 1]` -- see `index_is_hoistable_computed`) is hoisted into a
/// `let __hoist_N = k + 1;` temp so the access indexes a slotted plain place:
/// the checker proves the temp's bounds from its env interval, state-storage
/// keeps its slot (the runtime-index carve-out), and simplify never folds a
/// computed binding back into an index position. Anything else goes through
/// `hoist_child` unchanged (a runtime-indexed read INSIDE the index,
/// `arr[other[i]]`, still hoists there).
pub(crate) fn hoist_index(
    lowerer: &mut Lowerer,
    index: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
    mode: OperandHoisting,
) -> ExpressionHandle {
    if index_is_hoistable_computed(lowerer, index) {
        return hoist_into_temp(lowerer, index, hoisted);
    }
    hoist_child(lowerer, index, hoisted, mode)
}

/// A `Binary` index up to TWO levels deep (`k + 1`, `2 * k`, and the
/// row-major idiom `y * 4 + x`) whose leaf operands are each TYPEABLE at the
/// hoist-temp layer: an integer literal, a `self.<field>` member, or a
/// state-PARAMETER name -- with at least one non-literal (a pure-const binary
/// is left for the const fold, which resolves it to a fixed index without a
/// temp). A LOCAL leaf is refused: `infer_hoist_temp_type` resolves
/// self-fields and params only, and an untypeable temp would mint a Unit
/// layout error where the checker's computed-index fence gives a clear
/// message today. Deeper nests (`(a+b)*(c+d)` and beyond) are likewise left
/// fenced -- the interval synthesis in `computed_index_interval` composes to
/// the same depth, so hoistable-here and range-synthesizable-there stay in
/// lockstep (R0 of the dependent-types ladder: the two-level shape was
/// neither hoisted NOR fenced, and silently read ZII natively).
fn index_is_hoistable_computed(lowerer: &Lowerer, index: ExpressionHandle) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let ExpressionNode::Binary(binary) = expressions.expression(index) else {
        return false;
    };
    let leaf_is_typeable = |operand: ExpressionHandle| -> bool {
        match expressions.expression(operand) {
            ExpressionNode::Integer(_) => true,
            ExpressionNode::Member(member) => matches!(
                expressions.expression(member.receiver),
                ExpressionNode::Name(path)
                    if expressions
                        .name_path_members(path.members)
                        .first()
                        .is_some_and(|name| name.as_str() == "self")
            ),
            ExpressionNode::Name(path) => {
                let members = expressions.name_path_members(path.members);
                let [only] = members else {
                    return false;
                };
                lowerer
                    .current_state_parameter_names
                    .iter()
                    .any(|name| name == only.as_str())
            }
            _ => false,
        }
    };
    let operand_is_typeable = |operand: ExpressionHandle| -> bool {
        if leaf_is_typeable(operand) {
            return true;
        }
        // One nested level: a binary of typeable LEAVES (`y * 4` inside
        // `y * 4 + x`).
        match expressions.expression(operand) {
            ExpressionNode::Binary(inner) => {
                leaf_is_typeable(inner.left) && leaf_is_typeable(inner.right)
            }
            _ => false,
        }
    };
    let contains_non_literal = |operand: ExpressionHandle| -> bool {
        match expressions.expression(operand) {
            ExpressionNode::Integer(_) => false,
            ExpressionNode::Binary(inner) => {
                !matches!(
                    expressions.expression(inner.left),
                    ExpressionNode::Integer(_)
                ) || !matches!(
                    expressions.expression(inner.right),
                    ExpressionNode::Integer(_)
                )
            }
            _ => true,
        }
    };
    if !contains_non_literal(binary.left) && !contains_non_literal(binary.right) {
        return false;
    }
    operand_is_typeable(binary.left) && operand_is_typeable(binary.right)
}

/// Hoists computed indices inside an assignment TARGET's place chain
/// (`self.arr[self.k + 1] = v`), walking `Member` receivers and `Indexed`
/// collections. Only INDEX positions are rewritten -- the place structure
/// itself is never hoisted (it is a write target, not a value).
pub(crate) fn hoist_target_computed_indices(
    lowerer: &mut Lowerer,
    target: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) {
    let node = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(target)
        .clone();
    match node {
        ExpressionNode::Indexed(indexed) => {
            hoist_target_computed_indices(lowerer, indexed.collection, hoisted);
            if index_is_hoistable_computed(lowerer, indexed.index) {
                let index = hoist_into_temp(lowerer, indexed.index, hoisted);
                set_expression(
                    lowerer,
                    target,
                    ExpressionNode::Indexed(TableIndexedExpression {
                        collection: indexed.collection,
                        index,
                    }),
                );
            }
        }
        ExpressionNode::Member(member) => {
            hoist_target_computed_indices(lowerer, member.receiver, hoisted);
        }
        ExpressionNode::Borrow(inner) => {
            hoist_target_computed_indices(lowerer, inner.target, hoisted);
        }
        _ => {}
    }
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

    // In guard position, a pure-builtin call subject (`min(self.a, self.b)`) is
    // hoisted whole into a temp so the guard compares a materialized local. The
    // builtins are effect-free, so this never changes an effectful evaluation
    // count (unlike a general value-call hoist). Only calls whose first argument
    // is a `self.<field>` place are hoisted -- the symbol-resolved->typed lowering
    // types the temp from that field (`infer_hoist_temp_type`); a non-place first
    // argument (a nested call, a literal) is left for the "bind to a local first"
    // diagnostic, unchanged.
    if mode == OperandHoisting::Guard && is_hoistable_builtin_guard_call(lowerer, child) {
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
    let ExpressionNode::Member(member) = expressions.expression(expression) else {
        return false;
    };
    let mut receiver = member.receiver;
    while let ExpressionNode::Member(inner) = expressions.expression(receiver) {
        receiver = inner.receiver;
    }
    if !is_runtime_indexed_read(lowerer, receiver) {
        return false;
    }
    let ExpressionNode::Indexed(indexed) = expressions.expression(receiver) else {
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
                    .is_some_and(|name| name.as_str() == "self")
        ),
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            members.len() == 2 && members[0].as_str() == "self"
        }
        _ => false,
    }
}

pub(crate) fn is_runtime_indexed_read(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let ExpressionNode::Indexed(indexed) = expressions.expression(expression) else {
        return false;
    };
    !matches!(
        expressions.expression(indexed.index),
        ExpressionNode::Integer(_)
    )
}

/// Emits `let __hoist_N = <indexed_value>;` and returns a `Name` referencing it.
pub(crate) fn hoist_into_temp(
    lowerer: &mut Lowerer,
    indexed_value: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) -> ExpressionHandle {
    let name = DiagnosticName::generated(lowerer.next_hoist_name());

    hoisted.push(Statement::LocalData(LocalData {
        symbol: SymbolHandle::invalid(),
        name: name.clone(),
        storage: LocalDataStorage {
            // No annotation here. The element type (with its arithmetic domain)
            // is filled in by the symbol-resolved -> typed lowering, which has
            // the resolved data-field types available
            // (`statement::infer_hoist_temp_type`). Until then it is `Unit`,
            // the inference sentinel.
            type_reference: TypeReference::Unit,
            initial_value: indexed_value,
            is_mutable: false,
            type_is_inferred: true,
            relevance: language_core::BindingRelevance::Relevant,
        },
    }));

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
    lowerer
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
        }))
}

/// Whether `expression` is a pure-builtin call (`min`/`max`/`sqrt`; `abs`/`clamp`
/// are already desugared to these) that Phase-1 guard hoisting materializes: a
/// free call (no receiver) whose FIRST argument is a `self.<field>` place, so the
/// synthetic temp's type is resolvable from that field. `abs(self.x)` desugars to
/// `max(self.x, 0 - self.x)` (first arg `self.x`, hoisted); `clamp(self.x, ..)`
/// desugars to `min(max(self.x, ..), ..)` whose first arg is a call, so it is
/// left alone (not hoisted) -- the temp would be untypeable.
pub(super) fn is_hoistable_builtin_guard_call(
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
