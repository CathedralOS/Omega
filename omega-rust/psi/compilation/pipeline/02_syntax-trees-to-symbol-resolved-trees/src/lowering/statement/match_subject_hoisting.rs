//! Hoisting membership and comparison match subjects.

use crate::lowering::statement::hoist_temps::mint_hoist_temp;
use crate::lowering::statement::indexed_read_hoisting::{
    OperandHoisting, hoist_operand_indexed_reads, is_runtime_indexed_read,
};
use crate::lowering::statement::statement_nodes::set_expression;
use crate::resolution::lowerer::Lowerer;
use symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableBorrowExpression,
    TableMembershipExpression,
};
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::statement::Statement;
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
    let subject_key = syntax_membership.value;

    // Reuse the sibling arm's temp if the first arm already minted one; otherwise mint it here and
    // emit the single `let __hoist_N = &<subject>;` (reusing this arm's lowered indexed read as the
    // initializer -- later arms' lowered reads are simply left orphaned).
    let name_reference = mint_hoist_temp(
        lowerer,
        hoisted,
        TypeReference::Unit,
        Some(subject_key),
        false,
        |lowerer, _| borrow_membership_subject(lowerer, membership.value),
    );

    // Rewrite this arm's Membership to test the shared temp instead of the raw indexed read.
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

/// Capture a comparison's hoistable storage reads once for Boolean dispatch.
/// The parser shares one syntax subject across its true/false arms, but each
/// arm gets its own lowered expression. Independently hoisting `values[index]`
/// or a reference-backed field would give the arms different locals and lose
/// their common exhaustive subject. Key the shared Boolean local by that syntax
/// subject; its initializer owns the storage-read preparation. Ordinary call
/// sequencing stays with the common computation owner, not a builtin-name rule.
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
    // The subject must be a comparison containing a hoistable storage read.
    let subject_is_comparison = matches!(
        lowerer
            .symbol_resolved_trees
            .tables
            .bodies
            .expressions
            .expression(outer.left),
        ExpressionNode::Binary(inner) if inner.operator.is_comparison()
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
    let subject_key = syntax_outer.left;

    // The comparison's result carrier is already Boolean. Prepare its indexed
    // operands inside that shared initializer, then reuse the same Boolean
    // observation for every arm instead of rereading the source place.
    let declared_bool = TypeReference::Named {
        supplied_operand_owner: Default::default(),
        symbol: SymbolHandle::invalid(),
        name: DiagnosticName::generated("bool"),
    };
    let name_reference = mint_hoist_temp(
        lowerer,
        hoisted,
        declared_bool,
        Some(subject_key),
        false,
        |lowerer, hoisted| {
            hoist_operand_indexed_reads(lowerer, outer.left, hoisted, OperandHoisting::Computation);
            outer.left
        },
    );

    // Rewrite the guard to test the shared temp: `__hoist_N ==/!= <bool>`.
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
    let Some(member) = expressions.member_node(expression) else {
        return false;
    };
    let Some(path) = expressions.name_path_node(member.receiver) else {
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
    if is_runtime_indexed_read(lowerer, expression)
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
