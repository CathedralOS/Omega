//! Hoisting membership and comparison match subjects.

use crate::lowering::statement::indexed_read_hoisting::{
    OperandHoisting, hoist_index, hoist_operand_indexed_reads, is_hoistable_builtin_guard_call,
    is_runtime_indexed_read,
};
use crate::lowering::statement::statement_nodes::set_expression;
use crate::resolution::lowerer::Lowerer;
use arena::HandleSpan;
use symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableBorrowExpression,
    TableIndexedExpression, TableMembershipExpression, TableNamePath,
};
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::statement::{LocalData, LocalDataStorage, Statement};
use symbol_resolved_trees::types::TypeReference;
use symbols::SymbolHandle;
use syntax_trees::{self as syntax, SyntaxTrees};

/// Capture a membership place for observation without extracting its value.
/// The ordinary borrow preserves address evaluation and loan checking; an
/// owned snapshot would illegally move an affine element out of its array.
pub(crate) fn borrow_membership_subject(
    lowerer: &mut Lowerer,
    subject: ExpressionHandle,
) -> ExpressionHandle {
    lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .insert(ExpressionNode::Borrow(TableBorrowExpression {
            target: subject,
            access: language_semantics::ReferenceAccess::Shared,
        }))
}

/// Hoists a runtime-indexed ENUM-VARIANT MATCH subject into a SINGLE shared temp so every arm of
/// the match tests one plain local. A match `transition self.grid[self.i] { Cell::Wall -> .. }`
/// lowers (in the parser) to one `When(subject is Variant)` -- an `ExpressionNode::Membership` --
/// per arm, and every arm's Membership references the SAME syntax subject handle. Naively hoisting
/// each arm's subject (as the comparison hoist does for `Binary` guards) would mint a DISTINCT temp
/// per arm, and the exhaustiveness checker -- which groups the arms by a shared subject -- would then
/// report "match does not cover Variant". Instead this keys a memo on the shared syntax subject
/// handle: the FIRST arm mints `let __hoist_N = &self.grid[self.i]` and records the name; the siblings
/// reuse it. All arms end up testing `__hoist_N`, so exhaustiveness still groups them, and the temp
/// retains the original element's address rather than a copied affine value.
/// Const-index / field / string-slice subjects are not runtime-indexed and are left
/// untouched.
pub(crate) fn hoist_membership_match_subject(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    syntax_guard: syntax::statement::TransitionGuardNode,
    guard_expression: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) {
    // The LOWERED guard must be a Membership over a runtime-indexed subject.
    let ExpressionNode::Membership(membership) = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(guard_expression)
        .clone()
    else {
        return;
    };
    if !is_runtime_indexed_read(lowerer, membership.value) {
        return;
    }
    // The SYNTAX guard carries the subject handle shared across every arm of this match.
    let syntax::statement::TransitionGuardNode::When(syntax_expression) = syntax_guard else {
        return;
    };
    let syntax::expression::ExpressionNode::Membership(syntax_membership) =
        syntax_trees.expressions.expression(syntax_expression)
    else {
        return;
    };
    let subject_key = syntax_membership.value.arena_index();

    // Reuse the sibling arm's temp if the first arm already minted one; otherwise mint it here and
    // emit the single `let __hoist_N = &<subject>;` (reusing this arm's lowered indexed read as the
    // initializer -- later arms' lowered reads are simply left orphaned).
    let name = match lowerer.match_subject_temp(subject_key) {
        Some(existing) => DiagnosticName::generated(existing),
        None => {
            // The subject's INDEX may itself be a hoistable COMPUTED
            // expression (`grid[r * 6 + c]` as a match subject -- the
            // dungeon_render shape): hoist it into its own temp FIRST,
            // exactly as operand positions do, so the emitted subject let
            // indexes a slotted plain place (otherwise the #40 computed-
            // index fence refuses the initializer). Only on the minting
            // arm: sibling arms' lowered reads are orphaned anyway.
            if let ExpressionNode::Indexed(indexed) = lowerer
                .symbol_resolved_trees
                .tables
                .bodies
                .expressions
                .expression(membership.value)
                .clone()
            {
                let index = hoist_index(lowerer, indexed.index, hoisted, OperandHoisting::Value);
                set_expression(
                    lowerer,
                    membership.value,
                    ExpressionNode::Indexed(TableIndexedExpression {
                        collection: indexed.collection,
                        index,
                    }),
                );
            }
            let fresh = lowerer.next_hoist_name();
            lowerer.record_match_subject_temp(subject_key, fresh.clone());
            let name = DiagnosticName::generated(fresh);
            let observed = borrow_membership_subject(lowerer, membership.value);
            hoisted.push(Statement::LocalData(LocalData {
                symbol: SymbolHandle::invalid(),
                name: name.clone(),
                storage: LocalDataStorage {
                    type_reference: TypeReference::Unit,
                    initial_value: observed,
                    is_mutable: false,
                    type_is_inferred: true,
                    relevance: language_core::BindingRelevance::Relevant,
                },
            }));
            name
        }
    };

    // Rewrite this arm's Membership to test the shared temp instead of the raw indexed read.
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
    set_expression(
        lowerer,
        guard_expression,
        ExpressionNode::Membership(TableMembershipExpression {
            value: name_reference,
            domain: membership.domain,
            domain_symbol: membership.domain_symbol,
            case_type_symbol: membership.case_type_symbol,
            case_symbol: membership.case_symbol,
        }),
    );
}

/// Hoists a bool-arm dispatch SUBJECT that contains a hoistable read/call into a
/// SINGLE shared temp, so a `{ true -> .. false -> .. }` pair still shares one
/// subject and exhaustiveness pairs the arms.
///
/// A `transition min(self.a, self.b) == 3 { true -> A false -> B }` lowers (per
/// arm) to the wrapper `(min(self.a, self.b) == 3) == <bool>`. The operand hoist
/// would pull `min(self.a, self.b)` into a DISTINCT temp per arm (each arm
/// re-lowers the subject to its own handle), so the two arms no longer test a
/// structurally-equal subject and the dispatch is rejected as non-exhaustive
/// (`arr[i] > 5 { true/false }` fails identically -- this is not builtin-specific).
/// Instead this keys the shared `match_subject_temps` memo on the SYNTAX subject
/// handle (the parser reuses `subject[0]` across every arm's guard, guards.rs):
/// the first arm mints `let __hoist_N: bool = <subject>` and the siblings reuse
/// it, so all arms test one local. Only fires when the subject is a COMPARISON
/// CONTAINING a builtin/indexed read (exactly the shapes the operand hoist would
/// otherwise break); a bare-place subject (`self.flag`, `self.a > self.b`) is
/// structurally equal across arms already and is left untouched. Returns whether
/// it hoisted, so the caller skips the operand hoist.
///
/// Handles both a pure-BUILTIN subject (min/max/sqrt, which lowers directly in the
/// let value) and a runtime-INDEXED subject (`arr[i] > 5 { true/false }`, whose
/// read is hoisted INSIDE the shared temp -- `let __t = arr[i]; let __b = __t >
/// 5`; `__t` keeps its slot as a compare operand, so it reads correctly).
pub(crate) fn hoist_comparison_match_subject(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    syntax_guard: syntax::statement::TransitionGuardNode,
    guard_expression: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) -> bool {
    // The LOWERED guard must be the bool-arm wrapper `SUBJECT ==/!= <bool>`.
    let ExpressionNode::Binary(outer) = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(guard_expression)
        .clone()
    else {
        return false;
    };
    if !matches!(
        outer.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        return false;
    }
    if !matches!(
        lowerer
            .symbol_resolved_trees
            .tables
            .bodies
            .expressions
            .expression(outer.right),
        ExpressionNode::Boolean(_)
    ) {
        return false;
    }
    // The SUBJECT must be a comparison that CONTAINS a hoistable read/call.
    let subject_is_comparison = matches!(
        lowerer
            .symbol_resolved_trees
            .tables
            .bodies
            .expressions
            .expression(outer.left),
        ExpressionNode::Binary(inner) if is_comparison_operator(inner.operator)
    );
    if !subject_is_comparison || !subject_contains_hoistable(lowerer, outer.left) {
        return false;
    }

    // The SHARED syntax subject handle: `subject[0]`, the left of every arm's
    // wrapper (guards.rs builds `Binary { left: subject[0], ==, right: arm }`).
    let syntax::statement::TransitionGuardNode::When(syntax_expression) = syntax_guard else {
        return false;
    };
    let syntax::expression::ExpressionNode::Binary(syntax_outer) =
        syntax_trees.expressions.expression(syntax_expression)
    else {
        return false;
    };
    let subject_key = syntax_outer.left.arena_index();

    // Reuse the sibling arm's temp, or mint `let __hoist_N: bool = <subject>`.
    // The subject is a comparison, so the temp is `bool` -- known here (unlike the
    // indexed/builtin operand hoists, whose element type needs the resolved field
    // types via `infer_hoist_temp_type`). A pure-builtin subject lowers directly in
    // the let value (`let __b = min(a, b) == 3`); a runtime-INDEXED subject needs
    // its read hoisted INSIDE the temp first (`let __t = arr[i]; let __b = __t >
    // 5`), which the let-value operand hoist does -- `__t` (an indexed-read local
    // used as a compare operand) keeps its slot, so `__t > 5` reads correctly.
    let name = match lowerer.match_subject_temp(subject_key) {
        Some(existing) => DiagnosticName::generated(existing),
        None => {
            hoist_operand_indexed_reads(lowerer, outer.left, hoisted, OperandHoisting::Computation);
            let fresh = lowerer.next_hoist_name();
            lowerer.record_match_subject_temp(subject_key, fresh.clone());
            let name = DiagnosticName::generated(fresh);
            hoisted.push(Statement::LocalData(LocalData {
                symbol: SymbolHandle::invalid(),
                name: name.clone(),
                storage: LocalDataStorage {
                    type_reference: TypeReference::Named {
                        symbol: SymbolHandle::invalid(),
                        name: DiagnosticName::generated("bool"),
                    },
                    initial_value: outer.left,
                    is_mutable: false,
                    type_is_inferred: false,
                    relevance: language_core::BindingRelevance::Relevant,
                },
            }));
            name
        }
    };

    // Rewrite the guard to test the shared temp: `__hoist_N ==/!= <bool>`.
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
    set_expression(
        lowerer,
        guard_expression,
        ExpressionNode::Binary(TableBinaryExpression {
            left: name_reference,
            operator: outer.operator,
            right: outer.right,
        }),
    );
    true
}

fn is_comparison_operator(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    )
}

/// Whether `expression` contains (transitively through comparison/arith/cast
/// operands) a hoistable pure-builtin call OR a runtime-indexed read -- the shapes
/// the shared-subject hoist handles (both would otherwise be pulled into per-arm
/// temps by the operand hoist, breaking the true/false pairing).
/// Whether `expression` is a member read through a SHARED reference-to-struct
/// parameter of the current state (`table.con_out` with `table:
/// &EfiSystemTable`) -- the shape whose flat fold reads frame garbage. The
/// receiver must be a bare single-segment name matching one of the recorded
/// `&Named` params.
pub(crate) fn is_reference_struct_parameter_member(
    lowerer: &Lowerer,
    expression: ExpressionHandle,
) -> bool {
    if lowerer.reference_struct_parameters.is_empty() {
        return false;
    }
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let ExpressionNode::Member(member) = expressions.expression(expression) else {
        return false;
    };
    let ExpressionNode::Name(path) = expressions.expression(member.receiver) else {
        return false;
    };
    let members = expressions.name_path_members(path.members);
    let [only] = members else {
        return false;
    };
    lowerer
        .reference_struct_parameters
        .iter()
        .any(|name| name == only.as_str())
}

fn subject_contains_hoistable(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    if is_hoistable_builtin_guard_call(lowerer, expression)
        || is_runtime_indexed_read(lowerer, expression)
        || is_reference_struct_parameter_member(lowerer, expression)
    {
        return true;
    }
    let node = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(expression)
        .clone();
    match node {
        ExpressionNode::Binary(binary) => {
            subject_contains_hoistable(lowerer, binary.left)
                || subject_contains_hoistable(lowerer, binary.right)
        }
        ExpressionNode::Unary(unary) => subject_contains_hoistable(lowerer, unary.operand),
        ExpressionNode::Cast(cast) => subject_contains_hoistable(lowerer, cast.value),
        _ => false,
    }
}

/// Whether a `When` guard is a COMPARISON/boolean guard whose runtime-indexed operands should be
/// hoisted. A match arm lowers to a `When(subject is Variant)` -- an `ExpressionNode::Membership`
/// root -- and hoisting each arm's subject into a distinct temp would break match exhaustiveness
/// (all arms of one match must share a single subject). Only a `Binary` root (`arr[i] > 5`,
/// `arr[i] == 66`, `a && b`) is hoisted; membership/pattern guards are left for the separate
/// shared-subject match rewrite.
pub(crate) fn guard_hoists_operands(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    matches!(
        lowerer
            .symbol_resolved_trees
            .tables
            .bodies
            .expressions
            .expression(expression),
        ExpressionNode::Binary(_)
    )
}
