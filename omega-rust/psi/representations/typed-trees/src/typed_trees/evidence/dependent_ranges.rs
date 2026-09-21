//! R1 dependent-range recognizer (chapter 12): the ADMISSIBLE symbolic
//! bound classes a declared range's endpoint may take. One classifier,
//! [`dependent_bound_class`], decides which class a bound is; the three
//! policy consumers -- the validation fence (which non-constant bounds are
//! legal, validation type_references.rs), the proof-plan atom minting
//! (proof obligations.rs), and the callee-side range substitution
//! (typed-trees-to-checked-trees index proofs) -- share that decision
//! directly or through the per-class views below, so "admissible" can
//! never drift between the gate and the dischargers.
//!
//! Rung R1a admits exactly three classes, each a named anchor plus an
//! optional literal offset. `self.<field>` is the self-field class:
//! `[0..=self.count]` -> (count, 0); `[0..self.count]` retains its end-kind
//! and normalizes to (count, -1) in bounded proof metadata. The
//! sibling-length class (`[0..items.len]`, chapter 12's Buffer::get shape)
//! admits `<name>.len` on a non-`self` bare name, interpreted by policies
//! as a SIBLING PARAMETER's slice length. The scope-value class
//! (`[0..=limit]`) admits a bare single-segment name; POLICIES resolve the
//! name's symbol to an integer scalar parameter (or, on a local
//! declaration, an earlier local) of the owning state and bind it as a
//! proof atom. Everything else stays behind the non-constant-bound fence.

use crate::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable};
use crate::name::Identifier;

/// The recognized symbolic maximum: the named `self` FIELD and a literal
/// offset applied to its entry value (`self.count - 1` -> offset -1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicMaxBound {
    pub field: Identifier,
    pub offset: i64,
}

/// The recognized sibling-length maximum: `<sibling>.len + offset`, where
/// `sibling` is a bare name the POLICIES must resolve to a same-state
/// parameter of slice/array type (`[0..items.len]` -> (items, -1) after
/// normalization of the retained boundary kind).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiblingLenBound {
    pub sibling: Identifier,
    pub offset: i64,
}

/// The recognized scope-value maximum: the bare single-segment NAME
/// expression itself plus a literal offset (`[0..=limit]` -> (limit, 0)).
/// The handle is returned rather than the identifier so each policy can read
/// the resolved `path.symbol` directly; symbol-keyed proof atoms never depend
/// on display spellings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedNameBound {
    pub name: ExpressionHandle,
    pub offset: i64,
}

/// The closed set of R1a bound classes a symbolic maximum can take. Every
/// admissible bound is exactly one variant, so a consumer that accepts
/// several classes (the validation fence accepts all three; the proof plan
/// mints atoms for the self-field and sibling classes) reads the class as a
/// value instead of running separate recognizers -- the class can never
/// disagree with what the per-class views below return, because they ARE
/// this classification filtered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependentBoundClass {
    /// `self.<field>` -- a field of the machine's attached data; policies
    /// must check the owner is a machine state parameter whose field can
    /// actually discharge.
    SelfField(SymbolicMaxBound),
    /// `<sibling>.len` -- a same-state slice/array parameter's length.
    SiblingLen(SiblingLenBound),
    /// A bare single-segment name; the calling POLICY resolves its symbol.
    ScopedName(ScopedNameBound),
}

impl DependentBoundClass {
    /// Each class carries its literal offset in the same-named field.
    fn offset_mut(&mut self) -> &mut i64 {
        match self {
            Self::SelfField(bound) => &mut bound.offset,
            Self::SiblingLen(bound) => &mut bound.offset,
            Self::ScopedName(bound) => &mut bound.offset,
        }
    }

    /// Apply `delta` to the recognized offset; `None` on overflow. A
    /// wrapped offset would silently change which range was admitted, so
    /// overflow refuses rather than clamps.
    fn shifted(mut self, delta: i64) -> Option<Self> {
        let offset = self.offset_mut();
        *offset = offset.checked_add(delta)?;
        Some(self)
    }
}

/// Classify an authored symbolic bound expression into its R1a class, with
/// the literal offset applied. `None` means the bound is not in any
/// admissible class (callers keep their literal path or their fence --
/// never treat `None` as unbounded).
pub fn dependent_bound_class(
    table: &ExpressionTable,
    bound: ExpressionHandle,
) -> Option<DependentBoundClass> {
    let (anchor, offset) = bound_anchor_and_offset(table, bound)?;
    bound_anchor_class(table, anchor)?.shifted(offset)
}

/// Classify and normalize an authored symbolic upper end into the bounded
/// proof offset. Failure is not an unbounded range: callers must reject
/// unsupported metadata.
pub fn dependent_range_maximum(
    table: &ExpressionTable,
    bound: ExpressionHandle,
    end_inclusive: bool,
) -> Option<DependentBoundClass> {
    let class = dependent_bound_class(table, bound)?;
    if end_inclusive {
        Some(class)
    } else {
        class.shifted(-1)
    }
}

/// Split `anchor [+/- <integer literal>]` into the anchor expression and
/// its offset; a non-Binary bound is its own anchor with offset 0. The
/// offset arithmetic is identical for every class, so it is peeled before
/// classification -- `k + name`, `name * k`, and non-literal right
/// operands are not bounds at all.
fn bound_anchor_and_offset(
    table: &ExpressionTable,
    bound: ExpressionHandle,
) -> Option<(ExpressionHandle, i64)> {
    if !bound.is_valid() {
        return None;
    }
    let ExpressionNode::Binary(binary) = table.expression(bound) else {
        return Some((bound, 0));
    };
    let ExpressionNode::Integer(literal) = table.expression(binary.right) else {
        return None;
    };
    let magnitude = literal.value_i64()?;
    let offset = match binary.operator {
        BinaryOperator::Add => magnitude,
        BinaryOperator::Subtract => magnitude.checked_neg()?,
        _ => return None,
    };
    Some((binary.left, offset))
}

/// Classify the offset-free anchor into its bound class. The classes are
/// disjoint on the anchor shape: a Member anchor is a self field when its
/// receiver is `self`, a sibling length when its member is `len` on a
/// non-`self` bare name, and neither otherwise; a bare Name anchor is the
/// scope-value class. No anchor satisfies two classes, so the checks need
/// no precedence order.
fn bound_anchor_class(
    table: &ExpressionTable,
    anchor: ExpressionHandle,
) -> Option<DependentBoundClass> {
    if let Some(field) = self_field_name(table, anchor) {
        return Some(DependentBoundClass::SelfField(SymbolicMaxBound {
            field,
            offset: 0,
        }));
    }
    if let Some(sibling) = bare_name_len(table, anchor) {
        return Some(DependentBoundClass::SiblingLen(SiblingLenBound {
            sibling,
            offset: 0,
        }));
    }
    scoped_name_anchor(table, anchor)
        .map(|name| DependentBoundClass::ScopedName(ScopedNameBound { name, offset: 0 }))
}

/// Recognizes an R1a-admissible `self.<field>` bound. `None` means the
/// bound is not in that class (callers keep their literal path or their
/// fence -- never treat `None` as unbounded).
pub fn symbolic_max_bound(
    table: &ExpressionTable,
    bound: ExpressionHandle,
) -> Option<SymbolicMaxBound> {
    match dependent_bound_class(table, bound)? {
        DependentBoundClass::SelfField(bound) => Some(bound),
        _ => None,
    }
}

/// Normalize an authored `self.<field>` upper end into the bounded proof
/// offset. Failure is not an unbounded range: callers must reject
/// unsupported metadata.
pub fn symbolic_range_maximum(
    table: &ExpressionTable,
    bound: ExpressionHandle,
    end_inclusive: bool,
) -> Option<SymbolicMaxBound> {
    match dependent_range_maximum(table, bound, end_inclusive)? {
        DependentBoundClass::SelfField(bound) => Some(bound),
        _ => None,
    }
}

/// `self.<field>` (a Member whose receiver is the bare `self` name), or
/// `None` for any other shape -- locals, params, and deeper chains are not
/// in the R1a class (a field's range is store-enforced machine-wide, which
/// is what makes the substitution in the callee sound).
fn self_field_name(table: &ExpressionTable, expression: ExpressionHandle) -> Option<Identifier> {
    let ExpressionNode::Member(member) = table.expression(expression) else {
        return None;
    };
    let ExpressionNode::Name(path) = table.expression(member.receiver) else {
        return None;
    };
    let [only] = table.name_path_members(path.members) else {
        return None;
    };
    (only.as_str() == "self").then(|| member.member.clone())
}

/// Recognizes `<name>.len [+/- k]` -- the sibling-length class. `None` is
/// never unbounded; callers keep their fence.
pub fn sibling_len_bound(
    table: &ExpressionTable,
    bound: ExpressionHandle,
) -> Option<SiblingLenBound> {
    match dependent_bound_class(table, bound)? {
        DependentBoundClass::SiblingLen(bound) => Some(bound),
        _ => None,
    }
}

/// The same checked boundary normalization for a sibling's length.
pub fn sibling_range_maximum(
    table: &ExpressionTable,
    bound: ExpressionHandle,
    end_inclusive: bool,
) -> Option<SiblingLenBound> {
    match dependent_range_maximum(table, bound, end_inclusive)? {
        DependentBoundClass::SiblingLen(bound) => Some(bound),
        _ => None,
    }
}

/// `<name>.len` where `<name>` is a bare single-segment name (NOT `self.x` --
/// that is the field class).
fn bare_name_len(table: &ExpressionTable, expression: ExpressionHandle) -> Option<Identifier> {
    let ExpressionNode::Member(member) = table.expression(expression) else {
        return None;
    };
    if member.member.as_str() != "len" {
        return None;
    }
    let ExpressionNode::Name(path) = table.expression(member.receiver) else {
        return None;
    };
    let [only] = table.name_path_members(path.members) else {
        return None;
    };
    (only.as_str() != "self").then(|| only.clone())
}

/// Recognizes `<name> [+/- k]` where `<name>` is a bare single-segment Name
/// node. `None` is never unbounded; callers keep their fence. Whether the
/// named symbol is admissible -- an integer scalar parameter of the same
/// state, or an earlier local on a local declaration -- is the policy's
/// scope check, not this recognizer's.
pub fn scoped_name_bound(
    table: &ExpressionTable,
    bound: ExpressionHandle,
) -> Option<ScopedNameBound> {
    match dependent_bound_class(table, bound)? {
        DependentBoundClass::ScopedName(bound) => Some(bound),
        _ => None,
    }
}

/// The same checked boundary normalization for a scope-named value.
pub fn scoped_range_maximum(
    table: &ExpressionTable,
    bound: ExpressionHandle,
    end_inclusive: bool,
) -> Option<ScopedNameBound> {
    match dependent_range_maximum(table, bound, end_inclusive)? {
        DependentBoundClass::ScopedName(bound) => Some(bound),
        _ => None,
    }
}

/// `<name>` as a bound anchor: a bare single-segment Name that is not
/// `self`, with `head_symbol == symbol` (the whole path resolved to the
/// same symbol -- a partial path like `a.b` is not a scope value).
fn scoped_name_anchor(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Name(path) = table.expression(expression) else {
        return None;
    };
    let [only] = table.name_path_members(path.members) else {
        return None;
    };
    (only.as_str() != "self" && path.head_symbol == path.symbol).then_some(expression)
}

#[cfg(test)]
mod tests {
    use super::{
        DependentBoundClass, SiblingLenBound, SymbolicMaxBound, dependent_bound_class,
        dependent_range_maximum, scoped_name_bound, scoped_range_maximum, sibling_len_bound,
        sibling_range_maximum, symbolic_max_bound,
    };
    use crate::expression::{BinaryExpression, Expression, MemberExpression, NamePath};
    use crate::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable};
    use crate::name::Identifier;
    use symbols::SymbolHandle;

    fn name_expression(text: &str, symbol_index: u32) -> Expression {
        let symbol = SymbolHandle::from_arena_index(symbol_index);
        Expression::Name(NamePath::resolved(
            vec![Identifier::generated(text)],
            symbol,
            symbol,
        ))
    }

    fn member_expression(receiver: Expression, member: &str, symbol_index: u32) -> Expression {
        Expression::Member(Box::new(MemberExpression {
            receiver,
            member_symbol: SymbolHandle::from_arena_index(symbol_index),
            member: Identifier::generated(member),
            case_variant: None,
        }))
    }

    fn binary_expression(
        left: Expression,
        operator: BinaryOperator,
        right: Expression,
    ) -> Expression {
        Expression::Binary(Box::new(BinaryExpression {
            left,
            operator,
            right,
        }))
    }

    fn integer_expression(value: i64) -> Expression {
        Expression::Integer(numerics::literals::IntegerLiteral::from_value(value))
    }

    fn classify(expression: Expression) -> (ExpressionTable, Option<DependentBoundClass>) {
        let mut table = ExpressionTable::new();
        let bound = table.insert_tree(&expression);
        let class = dependent_bound_class(&table, bound);
        (table, class)
    }

    #[test]
    fn bound_classes_partition_the_admitted_anchor_shapes() {
        let (_table, class) = classify(member_expression(name_expression("self", 1), "count", 2));
        assert_eq!(
            class,
            Some(DependentBoundClass::SelfField(SymbolicMaxBound {
                field: Identifier::generated("count"),
                offset: 0,
            }))
        );

        let (_table, class) = classify(member_expression(name_expression("items", 3), "len", 4));
        assert_eq!(
            class,
            Some(DependentBoundClass::SiblingLen(SiblingLenBound {
                sibling: Identifier::generated("items"),
                offset: 0,
            }))
        );

        let (table, class) = classify(name_expression("limit", 5));
        let Some(DependentBoundClass::ScopedName(scoped)) = class else {
            panic!("bare resolved name should be the scope-value class");
        };
        assert_eq!(scoped.offset, 0);
        let ExpressionNode::Name(path) = table.expression(scoped.name) else {
            panic!("scoped bound keeps its name expression handle");
        };
        assert_eq!(path.symbol, SymbolHandle::from_arena_index(5));

        // Disjointness is structural: `self.len` is a field bound, never a
        // sibling length, and a `len` member on a non-name receiver admits
        // no class at all.
        let (_table, class) = classify(member_expression(name_expression("self", 1), "len", 6));
        assert!(matches!(class, Some(DependentBoundClass::SelfField(_))));
        let (_table, class) = classify(member_expression(
            member_expression(name_expression("state", 7), "items", 8),
            "len",
            9,
        ));
        assert_eq!(class, None);
    }

    #[test]
    fn bound_offsets_compose_with_the_exclusive_end_normalization() {
        let mut table = ExpressionTable::new();
        let bound = table.insert_tree(&binary_expression(
            member_expression(name_expression("self", 1), "count", 2),
            BinaryOperator::Subtract,
            integer_expression(1),
        ));
        assert_eq!(
            dependent_bound_class(&table, bound),
            Some(DependentBoundClass::SelfField(SymbolicMaxBound {
                field: Identifier::generated("count"),
                offset: -1,
            }))
        );
        // `[0..self.count - 1]` keeps its exclusive end, normalizing to -2.
        assert_eq!(
            dependent_range_maximum(&table, bound, false),
            Some(DependentBoundClass::SelfField(SymbolicMaxBound {
                field: Identifier::generated("count"),
                offset: -2,
            }))
        );

        let bound = table.insert_tree(&binary_expression(
            member_expression(name_expression("items", 3), "len", 4),
            BinaryOperator::Add,
            integer_expression(2),
        ));
        assert_eq!(
            dependent_bound_class(&table, bound),
            Some(DependentBoundClass::SiblingLen(SiblingLenBound {
                sibling: Identifier::generated("items"),
                offset: 2,
            }))
        );
    }

    #[test]
    fn non_class_shapes_stay_behind_the_fence() {
        let mut table = ExpressionTable::new();
        let invalid = ExpressionHandle::invalid();
        assert_eq!(dependent_bound_class(&table, invalid), None);

        for expression in [
            // Bare `self` is not a field read, a sibling length, or a scope
            // value -- the `self` keyword is excluded from all three classes.
            name_expression("self", 1),
            // A multi-segment path is not a bare name in any class.
            Expression::Name(NamePath::resolved(
                vec![Identifier::generated("a"), Identifier::generated("b")],
                SymbolHandle::from_arena_index(10),
                SymbolHandle::from_arena_index(11),
            )),
            // `<name>.count` on a non-`self` name matches no class.
            member_expression(name_expression("items", 3), "count", 4),
            // Non-`+`/`-` operators and non-literal offsets are not bounds.
            binary_expression(
                name_expression("limit", 5),
                BinaryOperator::Multiply,
                integer_expression(2),
            ),
            binary_expression(
                name_expression("limit", 5),
                BinaryOperator::Add,
                name_expression("other", 6),
            ),
            // `k + name` does not commute into the admissible shape.
            binary_expression(
                integer_expression(1),
                BinaryOperator::Add,
                name_expression("limit", 5),
            ),
        ] {
            let bound = table.insert_tree(&expression);
            assert_eq!(
                dependent_bound_class(&table, bound),
                None,
                "expression must stay behind the non-constant-bound fence"
            );
        }
    }

    #[test]
    fn per_class_views_are_the_classification_filtered() {
        let mut table = ExpressionTable::new();
        let sibling_bound =
            table.insert_tree(&member_expression(name_expression("items", 3), "len", 4));
        let scoped_bound = table.insert_tree(&name_expression("limit", 5));
        let field_bound =
            table.insert_tree(&member_expression(name_expression("self", 1), "count", 2));

        assert!(symbolic_max_bound(&table, field_bound).is_some());
        assert!(symbolic_max_bound(&table, sibling_bound).is_none());
        assert!(symbolic_max_bound(&table, scoped_bound).is_none());
        assert_eq!(
            sibling_range_maximum(&table, sibling_bound, false).map(|bound| bound.offset),
            Some(-1)
        );
        assert!(sibling_len_bound(&table, field_bound).is_none());
        assert_eq!(
            scoped_range_maximum(&table, scoped_bound, false).map(|bound| bound.offset),
            Some(-1)
        );
        assert!(scoped_name_bound(&table, sibling_bound).is_none());
    }
}
