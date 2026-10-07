//! Index-argument terms and equality: bound-index leaves, structural
//! index equality, and the open/bound spellings an index argument takes.

use super::{BoundIndexArgument, BoundIndexSubstitutions};
use language_semantics::const_value::boolean_literal_spelling;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Each argument or leaf is classified once, and `index_terms_equal` decides
/// every pairing of kinds in one table. A `Named` binder the call bound
/// becomes the caller-side index it denotes; any other `Named` argument stays
/// an open name; a const expression stays an expression, whose own leaves
/// substitute only when it is compared against another expression.
#[derive(Debug, Clone, Copy)]
pub(super) enum IndexTerm<'a> {
    /// A callee binder this call bound: the caller-side subject or literal.
    Bound(&'a BoundIndexArgument),
    /// A `Named` index the call did not bind — a binder, subject, or const by
    /// symbol, or a literal atom by spelling.
    Open { symbol: SymbolHandle, name: &'a str },
    /// A const expression, compared leaf-wise.
    Expression(ExpressionHandle),
    /// Any other type-reference shape, which denotes only itself.
    Opaque(TypeReferenceHandle),
}

impl IndexTerm<'_> {
    /// Literals close a position; bound subjects, open binders, and
    /// expressions keep it open. An open name is closed when it spells a
    /// canonical const atom or an integer.
    pub(super) fn is_closed(self) -> bool {
        match self {
            IndexTerm::Bound(BoundIndexArgument::Literal(_)) => true,
            IndexTerm::Bound(BoundIndexArgument::Subject(_)) => false,
            IndexTerm::Open { name, .. } => {
                language_semantics::const_value::CanonicalConstValue::from_atom(name).is_some()
                    || name.parse::<i128>().is_ok()
            }
            IndexTerm::Expression(_) | IndexTerm::Opaque(_) => false,
        }
    }
}

/// The caller-side index a call bound `binder` to; the first binding wins.
pub(super) fn bound_index(
    substitutions: &BoundIndexSubstitutions,
    binder: SymbolHandle,
) -> Option<&BoundIndexArgument> {
    substitutions
        .iter()
        .find(|(candidate, _)| *candidate == binder)
        .map(|(_, bound)| bound)
}

pub(super) fn index_argument_term<'a>(
    program: &'a TypedTrees,
    argument: TypeReferenceHandle,
    substitutions: &'a BoundIndexSubstitutions,
) -> IndexTerm<'a> {
    match program.type_reference_table.type_reference(argument) {
        TypeReferenceNode::Named { symbol, name } => bound_index(substitutions, *symbol).map_or(
            IndexTerm::Open {
                symbol: *symbol,
                name: name.as_str(),
            },
            IndexTerm::Bound,
        ),
        TypeReferenceNode::ConstExpression(expression) => IndexTerm::Expression(*expression),
        _ => IndexTerm::Opaque(argument),
    }
}

/// A const-expression leaf naming a binder the call bound — by its resolved
/// symbol, or by its head symbol when unresolved — denotes the bound index;
/// every other node stays an expression.
pub(super) fn index_leaf_term<'a>(
    program: &TypedTrees,
    expression: ExpressionHandle,
    substitutions: &'a BoundIndexSubstitutions,
) -> IndexTerm<'a> {
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression) {
        let leaf_symbol = if path.symbol.is_valid() {
            path.symbol
        } else {
            path.head_symbol
        };
        if leaf_symbol.is_valid()
            && let Some(bound) = bound_index(substitutions, leaf_symbol)
        {
            return IndexTerm::Bound(bound);
        }
    }
    IndexTerm::Expression(expression)
}

/// Substitute binder positions on both sides, then compare pairwise. `true`
/// only when every pair denotes the same index after substitution.
pub(super) fn bound_index_arguments_equal(
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
                index_terms_equal(
                    program,
                    index_argument_term(program, *actual, substitutions),
                    index_argument_term(program, *expected, substitutions),
                    substitutions,
                )
            })
}

/// Whether two index terms denote the same index. Every pairing of term kinds
/// has its own row, and each row is symmetric in its sides.
pub(super) fn index_terms_equal(
    program: &TypedTrees,
    left: IndexTerm<'_>,
    right: IndexTerm<'_>,
    substitutions: &BoundIndexSubstitutions,
) -> bool {
    match (left, right) {
        // The same subject symbol, or the same literal spelling.
        (IndexTerm::Bound(left), IndexTerm::Bound(right)) => left == right,
        // A bound subject is the open name's symbol; a bound literal is its
        // spelling, not its value.
        (IndexTerm::Bound(bound), IndexTerm::Open { symbol, name })
        | (IndexTerm::Open { symbol, name }, IndexTerm::Bound(bound)) => match bound {
            BoundIndexArgument::Subject(subject) => *subject == symbol,
            BoundIndexArgument::Literal(spelling) => spelling.as_str() == name,
        },
        // Against a bound index the expression keeps its own spelling: its
        // leaves are not substituted, so `A` bound to `N` does not match `A`
        // spelled as an expression.
        (IndexTerm::Bound(bound), IndexTerm::Expression(expression))
        | (IndexTerm::Expression(expression), IndexTerm::Bound(bound)) => {
            bound_index_matches_expression(program, bound, expression)
        }
        (IndexTerm::Open { symbol, name }, IndexTerm::Expression(expression))
        | (IndexTerm::Expression(expression), IndexTerm::Open { symbol, name }) => {
            open_index_matches_expression(
                program,
                symbol,
                name,
                expression,
                OpenNameMatch::SymbolOrSpelling,
            )
        }
        (
            IndexTerm::Open {
                symbol: left_symbol,
                name: left_name,
            },
            IndexTerm::Open {
                symbol: right_symbol,
                name: right_name,
            },
        ) => left_symbol == right_symbol && left_name == right_name,
        // The callee's `Indexed<A + 0>` compares against the caller's
        // `Indexed<I + 0>` leaf-wise under `A -> I`.
        (IndexTerm::Expression(left), IndexTerm::Expression(right)) => {
            bound_index_expressions_equal(program, left, right, substitutions)
        }
        (IndexTerm::Opaque(left), IndexTerm::Opaque(right)) => left == right,
        (IndexTerm::Opaque(_), _) | (_, IndexTerm::Opaque(_)) => false,
    }
}

/// Whether a bound index denotes the same index an expression leaf spells: a
/// bound subject is the leaf's own binder symbol, and a bound literal is the
/// leaf's closed value — an integer leaf by value, a name leaf by spelling or
/// value. A bound literal never matches a Boolean leaf.
pub(super) fn bound_index_matches_expression(
    program: &TypedTrees,
    bound: &BoundIndexArgument,
    expression: ExpressionHandle,
) -> bool {
    match (bound, program.expression_table.expression(expression)) {
        (BoundIndexArgument::Subject(subject), ExpressionNode::Name(path)) => {
            *subject == path.symbol || *subject == path.head_symbol
        }
        (BoundIndexArgument::Literal(spelling), ExpressionNode::Integer(literal)) => {
            named_integer_value(spelling.as_str())
                .is_some_and(|expected| literal.value_bignum() == Some(expected))
        }
        (BoundIndexArgument::Literal(spelling), ExpressionNode::Name(path)) => {
            expression_name_atom(program, path) == Some(spelling.as_str())
                || expression_name_atom(program, path)
                    .and_then(named_integer_value)
                    .zip(named_integer_value(spelling.as_str()))
                    .is_some_and(|(left, right)| left == right)
        }
        _ => false,
    }
}

/// How an open name meets a name leaf. The two comparisons differ only when
/// both sides carry resolved symbols that disagree.
#[derive(Debug, Clone, Copy)]
pub(super) enum OpenNameMatch {
    /// Between index arguments: the leaf's symbol, or its single-member
    /// spelling even when the symbols disagree.
    SymbolOrSpelling,
    /// Against an established fact's operand: resolved symbols on both sides
    /// decide alone; spelling decides only when a side is unresolved.
    ResolvedSymbolFirst,
}

/// Whether an open name denotes the index an expression spells: a name leaf
/// by symbol or spelling (per `rule`), an integer leaf by value, and a
/// Boolean leaf by its `true`/`false` spelling.
pub(super) fn open_index_matches_expression(
    program: &TypedTrees,
    symbol: SymbolHandle,
    name: &str,
    expression: ExpressionHandle,
    rule: OpenNameMatch,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let symbol_matches =
                symbol.is_valid() && (path.symbol == symbol || path.head_symbol == symbol);
            let spelling_matches = matches!(
                program.expression_table.name_path_members(path.members),
                [only] if only.as_str() == name
            );
            match rule {
                OpenNameMatch::SymbolOrSpelling => symbol_matches || spelling_matches,
                OpenNameMatch::ResolvedSymbolFirst => {
                    if symbol.is_valid() && (path.symbol.is_valid() || path.head_symbol.is_valid())
                    {
                        symbol_matches
                    } else {
                        spelling_matches
                    }
                }
            }
        }
        ExpressionNode::Integer(literal) => named_integer_value(name)
            .is_some_and(|expected| literal.value_bignum() == Some(expected)),
        ExpressionNode::Boolean(value) => boolean_literal_spelling(name) == Some(*value),
        _ => false,
    }
}

/// Whether any leaf of `expression` names a binder this call bound — i.e. the
/// expression is spelled on the CALLEE's side of the substitution. A match
/// candidate containing a bound key is the callee's own spelling (or another
/// callee-side site), not the caller's.
pub(crate) fn expression_uses_binder(
    program: &TypedTrees,
    expression: ExpressionHandle,
    substitutions: &BoundIndexSubstitutions,
) -> bool {
    matches!(
        index_leaf_term(program, expression, substitutions),
        IndexTerm::Bound(_)
    ) || match program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary) => {
            expression_uses_binder(program, unary.operand, substitutions)
        }
        ExpressionNode::Binary(binary) => {
            expression_uses_binder(program, binary.left, substitutions)
                || expression_uses_binder(program, binary.right, substitutions)
        }
        ExpressionNode::Member(member) => {
            expression_uses_binder(program, member.receiver, substitutions)
        }
        _ => false,
    }
}

/// Compare two index expressions leaf-wise under the call's substitutions: a
/// leaf naming a bound binder denotes its bound index, and every other node
/// must have the same shape over equal children.
pub(crate) fn bound_index_expressions_equal(
    program: &TypedTrees,
    actual: ExpressionHandle,
    expected: ExpressionHandle,
    substitutions: &BoundIndexSubstitutions,
) -> bool {
    if actual == expected {
        return true;
    }
    if !actual.is_valid() || !expected.is_valid() {
        return false;
    }
    match (
        index_leaf_term(program, actual, substitutions),
        index_leaf_term(program, expected, substitutions),
    ) {
        (IndexTerm::Expression(_), IndexTerm::Expression(_)) => {}
        (left, right) => return index_terms_equal(program, left, right, substitutions),
    }
    match (
        program.expression_table.expression(actual),
        program.expression_table.expression(expected),
    ) {
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
        (ExpressionNode::Integer(left), ExpressionNode::Name(right))
        | (ExpressionNode::Name(right), ExpressionNode::Integer(left)) => {
            expression_name_atom(program, right)
                .and_then(named_integer_value)
                .is_some_and(|right| left.value_bignum() == Some(right))
        }
        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            name_paths_equal(program, left, right)
        }
        (ExpressionNode::Unary(left), ExpressionNode::Unary(right)) => {
            left.operator == right.operator
                && bound_index_expressions_equal(
                    program,
                    left.operand,
                    right.operand,
                    substitutions,
                )
        }
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            left.operator == right.operator
                && bound_index_expressions_equal(program, left.left, right.left, substitutions)
                && bound_index_expressions_equal(program, left.right, right.right, substitutions)
        }
        (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
            left.member_symbol == right.member_symbol
                && left.member.as_str() == right.member.as_str()
                && bound_index_expressions_equal(
                    program,
                    left.receiver,
                    right.receiver,
                    substitutions,
                )
        }
        _ => false,
    }
}

/// Whether an established fact's operand, with the fact's own call
pub(super) fn expression_name_atom<'program>(
    program: &'program TypedTrees,
    path: &typed_trees::expression::TableNamePath,
) -> Option<&'program str> {
    let [only] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    Some(only.as_str())
}

pub(super) fn is_closed_index_argument(
    program: &TypedTrees,
    argument: TypeReferenceHandle,
) -> bool {
    index_argument_term(program, argument, &Vec::new()).is_closed()
}

pub(super) fn named_integer_value(name: &str) -> Option<numerics::bignum::BigInt> {
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

/// Structural equality of two name paths: same head and tail symbols plus the
/// same member segments in order — shared by the bound-index and
/// substituted-expression equality walks.
pub(super) fn name_paths_equal(
    program: &TypedTrees,
    left: &typed_trees::expression::TableNamePath,
    right: &typed_trees::expression::TableNamePath,
) -> bool {
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
