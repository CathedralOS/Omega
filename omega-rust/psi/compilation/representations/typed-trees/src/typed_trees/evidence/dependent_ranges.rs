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
//! optional literal offset. `<param>.<field>` on a subject parameter is
//! the self-field class: `[0..=main.count]` -> (count, 0);
//! `[0..main.count]` retains its end-kind and normalizes to (count, -1)
//! in bounded proof metadata. The sibling-length class
//! (`[0..items.len]`, chapter 12's Buffer::get shape) admits `<name>.len`
//! on a non-subject bare name, interpreted by policies as a SIBLING
//! PARAMETER's slice length. The scope-value class (`[0..=limit]`)
//! admits a bare single-segment name; POLICIES resolve the name's symbol
//! to an integer scalar parameter (or, on a local declaration, an
//! earlier local) of the owning state and bind it as a proof atom.
//! Everything else stays behind the non-constant-bound fence.

use crate::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable};
use crate::name::Identifier;
use language_semantics::declaration_selection::CollectionMeasure;
use symbols::SymbolHandle;

/// The recognized symbolic maximum: the named subject parameter's FIELD
/// and a literal offset applied to its entry value (`main.count - 1` ->
/// (count, -1)). `receiver` keeps the authored receiver spelling for
/// diagnostics; classification decided it by symbol, not by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicMaxBound {
    pub receiver: Identifier,
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
    /// `<param>.<field>` -- a field read on the signature's subject
    /// parameter (the state's data binding); policies must check the
    /// owner is a machine state parameter whose field can actually
    /// discharge.
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
    program: &crate::TypedTrees,
    bound: ExpressionHandle,
) -> Option<DependentBoundClass> {
    let table = &program.expression_table;
    let (anchor, offset) = bound_anchor_and_offset(table, bound)?;
    bound_anchor_class(program, anchor)?.shifted(offset)
}

/// Classify and normalize an authored symbolic upper end into the bounded
/// proof offset. Failure is not an unbounded range: callers must reject
/// unsupported metadata.
pub fn dependent_range_maximum(
    program: &crate::TypedTrees,
    bound: ExpressionHandle,
    end_inclusive: bool,
) -> Option<DependentBoundClass> {
    let class = dependent_bound_class(program, bound)?;
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
/// disjoint on the anchor shape: a Member anchor is a subject field when
/// its receiver resolves to a subject parameter, a sibling length when
/// its member is `len` on a non-subject bare name, and neither otherwise;
/// a bare Name anchor is the scope-value class. No anchor satisfies two
/// classes, so the checks need no precedence order.
fn bound_anchor_class(
    program: &crate::TypedTrees,
    anchor: ExpressionHandle,
) -> Option<DependentBoundClass> {
    if let Some((receiver, field)) = subject_field_name(program, anchor) {
        return Some(DependentBoundClass::SelfField(SymbolicMaxBound {
            receiver,
            field,
            offset: 0,
        }));
    }
    if let Some(sibling) = bare_name_len(program, anchor) {
        return Some(DependentBoundClass::SiblingLen(SiblingLenBound {
            sibling,
            offset: 0,
        }));
    }
    scoped_name_anchor(program, anchor)
        .map(|name| DependentBoundClass::ScopedName(ScopedNameBound { name, offset: 0 }))
}

/// Recognizes an R1a-admissible `<param>.<field>` bound on a subject
/// parameter. `None` means the bound is not in that class (callers keep
/// their literal path or their fence -- never treat `None` as unbounded).
pub fn symbolic_max_bound(
    program: &crate::TypedTrees,
    bound: ExpressionHandle,
) -> Option<SymbolicMaxBound> {
    match dependent_bound_class(program, bound)? {
        DependentBoundClass::SelfField(bound) => Some(bound),
        _ => None,
    }
}

/// Normalize an authored `<param>.<field>` upper end on a subject
/// parameter into the bounded proof offset. Failure is not an unbounded
/// range: callers must reject unsupported metadata.
pub fn symbolic_range_maximum(
    program: &crate::TypedTrees,
    bound: ExpressionHandle,
    end_inclusive: bool,
) -> Option<SymbolicMaxBound> {
    match dependent_range_maximum(program, bound, end_inclusive)? {
        DependentBoundClass::SelfField(bound) => Some(bound),
        _ => None,
    }
}

/// `<param>.<field>` where `<param>` names a subject parameter (a Member
/// whose receiver is a bare name resolving to the signature's data
/// binding), or `None` for any other shape -- locals, non-subject params,
/// and deeper chains are not in the R1a class (a field's range is
/// store-enforced machine-wide, which is what makes the substitution in
/// the callee sound). The receiver is matched by its resolved symbol, so
/// the class does not depend on what the parameter happens to be called.
fn subject_field_name(
    program: &crate::TypedTrees,
    expression: ExpressionHandle,
) -> Option<(Identifier, Identifier)> {
    let table = &program.expression_table;
    let ExpressionNode::Member(member) = table.expression(expression) else {
        return None;
    };
    let ExpressionNode::Name(path) = table.expression(member.receiver) else {
        return None;
    };
    let [only] = table.name_path_members(path.members) else {
        return None;
    };
    receiver_is_subject_parameter(program, path.symbol)
        .then(|| (only.clone(), member.member.clone()))
}

/// Whether `symbol` names a parameter that holds a subject position: a
/// machine state parameter carrying `is_self`, declaring the owning
/// machine's attached data, or typed by the `Self` carrier. A resolved
/// name's symbol identifies one parameter program-wide, so the answer
/// never depends on which signature asks.
fn receiver_is_subject_parameter(program: &crate::TypedTrees, receiver: SymbolHandle) -> bool {
    if !receiver.is_valid() {
        return false;
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                if parameter.symbol == receiver {
                    return parameter.is_self
                        || program.type_reference_names_attached_data(
                            parameter.type_reference,
                            machine.attached_data_symbol,
                        )
                        || type_reference_is_self(program, parameter.type_reference);
                }
            }
        }
    }
    false
}

/// Whether `type_reference` spells the `Self` carrier once references and
/// constraint shells are stripped. On an attached machine the named-data
/// read already covers it; on an unattached signature `Self` is the
/// binder the subject position still reads through.
fn type_reference_is_self(
    program: &crate::TypedTrees,
    type_reference: crate::types::TypeReferenceHandle,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(type_reference) {
        crate::types::TypeReferenceNode::Reference { referee, .. } => {
            type_reference_is_self(program, *referee)
        }
        crate::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_is_self(program, *base_type)
        }
        crate::types::TypeReferenceNode::Named { name, .. } => name.as_str() == "Self",
        _ => false,
    }
}

/// Recognizes `<name>.len [+/- k]` -- the sibling-length class. `None` is
/// never unbounded; callers keep their fence.
pub fn sibling_len_bound(
    program: &crate::TypedTrees,
    bound: ExpressionHandle,
) -> Option<SiblingLenBound> {
    match dependent_bound_class(program, bound)? {
        DependentBoundClass::SiblingLen(bound) => Some(bound),
        _ => None,
    }
}

/// The same checked boundary normalization for a sibling's length.
pub fn sibling_range_maximum(
    program: &crate::TypedTrees,
    bound: ExpressionHandle,
    end_inclusive: bool,
) -> Option<SiblingLenBound> {
    match dependent_range_maximum(program, bound, end_inclusive)? {
        DependentBoundClass::SiblingLen(bound) => Some(bound),
        _ => None,
    }
}

/// `<name>.len` where `<name>` is a bare single-segment name that does
/// not hold the subject position (`main.len` is the field class).
fn bare_name_len(program: &crate::TypedTrees, expression: ExpressionHandle) -> Option<Identifier> {
    let table = &program.expression_table;
    let ExpressionNode::Member(member) = table.expression(expression) else {
        return None;
    };
    if CollectionMeasure::from_authored_spelling(member.member.as_str())
        != Some(CollectionMeasure::Length)
    {
        return None;
    };
    let ExpressionNode::Name(path) = table.expression(member.receiver) else {
        return None;
    };
    let [only] = table.name_path_members(path.members) else {
        return None;
    };
    (!receiver_is_subject_parameter(program, path.symbol)).then(|| only.clone())
}

/// Recognizes `<name> [+/- k]` where `<name>` is a bare single-segment Name
/// node. `None` is never unbounded; callers keep their fence. Whether the
/// named symbol is admissible -- an integer scalar parameter of the same
/// state, or an earlier local on a local declaration -- is the policy's
/// scope check, not this recognizer's.
pub fn scoped_name_bound(
    program: &crate::TypedTrees,
    bound: ExpressionHandle,
) -> Option<ScopedNameBound> {
    match dependent_bound_class(program, bound)? {
        DependentBoundClass::ScopedName(bound) => Some(bound),
        _ => None,
    }
}

/// The same checked boundary normalization for a scope-named value.
pub fn scoped_range_maximum(
    program: &crate::TypedTrees,
    bound: ExpressionHandle,
    end_inclusive: bool,
) -> Option<ScopedNameBound> {
    match dependent_range_maximum(program, bound, end_inclusive)? {
        DependentBoundClass::ScopedName(bound) => Some(bound),
        _ => None,
    }
}

/// `<name>` as a bound anchor: a bare single-segment Name that does not
/// hold the subject position, with `head_symbol == symbol` (the whole
/// path resolved to the same symbol -- a partial path like `a.b` is not
/// a scope value).
fn scoped_name_anchor(
    program: &crate::TypedTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let table = &program.expression_table;
    let ExpressionNode::Name(path) = table.expression(expression) else {
        return None;
    };
    if table.name_path_members(path.members).len() != 1 {
        return None;
    };
    (!receiver_is_subject_parameter(program, path.symbol) && path.head_symbol == path.symbol)
        .then_some(expression)
}

#[cfg(test)]
mod tests {
    use super::{
        DependentBoundClass, SiblingLenBound, SymbolicMaxBound, dependent_bound_class,
        scoped_name_bound, scoped_range_maximum, sibling_len_bound, sibling_range_maximum,
        symbolic_max_bound,
    };
    use crate::expression::{BinaryExpression, Expression, MemberExpression, NamePath};
    use crate::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use crate::machine::Machine;
    use crate::name::Identifier;
    use crate::state::State;
    use crate::types::TypeReferenceNode;
    use crate::{TypedTrees, typed_trees::calls::signature::StateParameter};
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

    fn state_parameter(
        name: &str,
        symbol_index: u32,
        type_reference: crate::types::TypeReferenceHandle,
        is_self: bool,
    ) -> StateParameter {
        StateParameter {
            symbol: SymbolHandle::from_arena_index(symbol_index),
            name: Identifier::generated(name),
            type_reference,
            is_self,
            ..StateParameter::default()
        }
    }

    /// One attached machine `Main` with one state `main`: `main` (symbol 1)
    /// declares the attached data by name, `legacy` (symbol 7) carries the
    /// receiver flag, `carrier` (symbol 8) is `Self`-typed; `items` (3) and
    /// `limit` (5) are ordinary parameters.
    fn subject_program() -> TypedTrees {
        let mut program = TypedTrees::default();
        let attached = SymbolHandle::from_arena_index(100);
        let attached_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: attached,
                name: Identifier::generated("Main"),
            });
        let self_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("Self"),
            });
        let mut machine = Machine {
            name: Identifier::generated("Main"),
            attached_data_symbol: attached,
            ..Machine::default()
        };
        let mut state = State {
            name: Identifier::generated("main"),
            ..State::default()
        };
        program.push_state_parameter(&mut state, state_parameter("main", 1, attached_type, false));
        program.push_state_parameter(
            &mut state,
            state_parameter(
                "legacy",
                7,
                crate::types::TypeReferenceHandle::invalid(),
                true,
            ),
        );
        program.push_state_parameter(&mut state, state_parameter("carrier", 8, self_type, false));
        program.push_state_parameter(
            &mut state,
            state_parameter(
                "items",
                3,
                crate::types::TypeReferenceHandle::invalid(),
                false,
            ),
        );
        program.push_state_parameter(
            &mut state,
            state_parameter(
                "limit",
                5,
                crate::types::TypeReferenceHandle::invalid(),
                false,
            ),
        );
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
        program
    }

    fn classify(expression: Expression) -> (TypedTrees, Option<DependentBoundClass>) {
        let mut program = subject_program();
        let bound = program.expression_table.insert_tree(&expression);
        let class = dependent_bound_class(&program, bound);
        (program, class)
    }

    #[test]
    fn bound_classes_partition_the_admitted_anchor_shapes() {
        // The subject position is decided by the resolved parameter symbol,
        // not by the receiver's spelling: the data-binding parameter, the
        // receiver-flagged parameter, and a `Self`-typed parameter all admit.
        for (receiver, index) in [("main", 1), ("legacy", 7), ("carrier", 8)] {
            let (_program, class) = classify(member_expression(
                name_expression(receiver, index),
                "count",
                2,
            ));
            assert_eq!(
                class,
                Some(DependentBoundClass::SelfField(SymbolicMaxBound {
                    receiver: Identifier::generated(receiver),
                    field: Identifier::generated("count"),
                    offset: 0,
                })),
                "{receiver}.count must classify as the self-field bound"
            );
        }
        // Even a `self` spelling classifies through its resolved symbol.
        let (_program, class) = classify(member_expression(name_expression("self", 1), "count", 2));
        assert!(matches!(class, Some(DependentBoundClass::SelfField(_))));

        let (_program, class) = classify(member_expression(name_expression("items", 3), "len", 4));
        assert_eq!(
            class,
            Some(DependentBoundClass::SiblingLen(SiblingLenBound {
                sibling: Identifier::generated("items"),
                offset: 0,
            }))
        );

        let (program, class) = classify(name_expression("limit", 5));
        let Some(DependentBoundClass::ScopedName(scoped)) = class else {
            panic!("bare resolved name should be the scope-value class");
        };
        assert_eq!(scoped.offset, 0);
        let ExpressionNode::Name(path) = program.expression_table.expression(scoped.name) else {
            panic!("scoped bound keeps its name expression handle");
        };
        assert_eq!(path.symbol, SymbolHandle::from_arena_index(5));

        // Disjointness is structural: `main.len` is a field bound, never a
        // sibling length, and a `len` member on a non-name receiver admits
        // no class at all.
        let (_program, class) = classify(member_expression(name_expression("main", 1), "len", 6));
        assert!(matches!(class, Some(DependentBoundClass::SelfField(_))));
        let (_program, class) = classify(member_expression(
            member_expression(name_expression("state", 12), "items", 8),
            "len",
            9,
        ));
        assert_eq!(class, None);
    }

    #[test]
    fn non_class_shapes_stay_behind_the_fence() {
        let mut program = subject_program();
        let invalid = ExpressionHandle::invalid();
        assert_eq!(dependent_bound_class(&program, invalid), None);

        for expression in [
            // Bare `main` is a subject parameter, not a scope value.
            name_expression("main", 1),
            // A bare name that is not a subject parameter stays out too:
            // the scope-value class is decided by the policy's symbol check,
            // not by name spelling.
            // (kept here so the anchor itself is witnessed rejected)
            // A multi-segment path is not a bare name in any class.
            Expression::Name(NamePath::resolved(
                vec![Identifier::generated("a"), Identifier::generated("b")],
                SymbolHandle::from_arena_index(10),
                SymbolHandle::from_arena_index(11),
            )),
            // `<name>.count` on a non-subject name matches no class.
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
            let bound = program.expression_table.insert_tree(&expression);
            assert_eq!(
                dependent_bound_class(&program, bound),
                None,
                "expression must stay behind the non-constant-bound fence"
            );
        }
    }

    #[test]
    fn per_class_views_are_the_classification_filtered() {
        let mut program = subject_program();
        let sibling_bound = program.expression_table.insert_tree(&member_expression(
            name_expression("items", 3),
            "len",
            4,
        ));
        let scoped_bound = program
            .expression_table
            .insert_tree(&name_expression("limit", 5));
        let field_bound = program.expression_table.insert_tree(&member_expression(
            name_expression("main", 1),
            "count",
            2,
        ));

        assert!(symbolic_max_bound(&program, field_bound).is_some());
        assert!(symbolic_max_bound(&program, sibling_bound).is_none());
        assert!(symbolic_max_bound(&program, scoped_bound).is_none());
        assert_eq!(
            sibling_range_maximum(&program, sibling_bound, false).map(|bound| bound.offset),
            Some(-1)
        );
        assert!(sibling_len_bound(&program, field_bound).is_none());
        assert_eq!(
            scoped_range_maximum(&program, scoped_bound, false).map(|bound| bound.offset),
            Some(-1)
        );
        assert!(scoped_name_bound(&program, sibling_bound).is_none());
    }
}
