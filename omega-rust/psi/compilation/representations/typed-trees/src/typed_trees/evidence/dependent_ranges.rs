//! R1 dependent-range recognizer (chapter 12): the ADMISSIBLE symbolic
//! bound classes a declared range's endpoint may take. One classifier,
//! [`dependent_bound_class`], decides which class a bound is, and every
//! policy consumer -- the validation fence (which non-constant bounds are
//! legal, validation type_references.rs), the proof-plan atom minting
//! (proof obligations.rs), and the callee-side range substitution
//! (typed-trees-to-checked-trees index proofs) -- reads that class as a
//! value, so "admissible" can never drift between the gate and the
//! dischargers. A per-class projection would instead re-run the classifier
//! once per class a consumer tries, and would let a consumer order its
//! class checks differently from the classifier's own precedence.
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

use crate::expression::{BinaryOperator, ExpressionHandle, ExpressionTable};
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
/// value instead of running one recognizer per class. The classes are
/// disjoint on the anchor node, so a reader of the variant cannot disagree
/// with the classifier's own precedence.
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
    let Some(binary) = table.binary_node(bound) else {
        return Some((bound, 0));
    };
    let literal = table.integer_node(binary.right)?;
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
    let member = table.member_node(expression)?;
    let path = table.name_path_node(member.receiver)?;
    let [only] = table.name_path_members(path.members) else {
        return None;
    };
    receiver_is_subject_parameter(program, path.symbol)
        .then(|| (only.clone(), member.member.clone()))
}

/// Whether `symbol` names a parameter that holds a subject position: a
/// machine state parameter declaring the owning machine's attached data. A resolved
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
                    return program.type_reference_names_attached_data(
                        parameter.type_reference,
                        machine.attached_data_symbol,
                    );
                }
            }
        }
    }
    false
}

/// `<name>.len` where `<name>` is a bare single-segment name that does
/// not hold the subject position (`main.len` is the field class).
fn bare_name_len(program: &crate::TypedTrees, expression: ExpressionHandle) -> Option<Identifier> {
    let table = &program.expression_table;
    let member = table.member_node(expression)?;
    if CollectionMeasure::from_authored_spelling(member.member.as_str())
        != Some(CollectionMeasure::Length)
    {
        return None;
    };
    let path = table.name_path_node(member.receiver)?;
    let [only] = table.name_path_members(path.members) else {
        return None;
    };
    (!receiver_is_subject_parameter(program, path.symbol)).then(|| only.clone())
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
    let path = table.name_path_node(expression)?;
    if table.name_path_members(path.members).len() != 1 {
        return None;
    };
    (!receiver_is_subject_parameter(program, path.symbol) && path.head_symbol == path.symbol)
        .then_some(expression)
}
