//! One scalar store through a representation view: `view = v` where `view`
//! is an immutable `&mut`/`&write` local bound by `&mut place as &mut T`.
//!
//! The checked plan stays one row per authored statement, the store mirror
//! of `CheckedScalarExpression::RepresentationView`. The destination's
//! rejoined place decides the leaf family:
//!
//! - a whole primitive field of the same integer width (`&mut self.x as
//!   &mut u8` over an `i8` field, or a direct field of a record view whose
//!   source leaf has the other sign) lowers to one primitive store whose
//!   value is retagged to the leaf's declared carrier at emission;
//! - one `u8` element selected by `FixedIndex`/`RuntimeIndex` below a
//!   bounded-owned byte field (`&mut self.buf[k] as &mut u32`) lowers to one
//!   primitive store per little-endian footprint byte, each byte's `index +
//!   j` re-proving its own bound against the field's declared capacity —
//!   the same contract the view's read side composes from.
use super::super::{
    CheckFacts, CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment, PrimitiveType,
    TypedTrees,
};
use super::destination::{Carrier, Destination};
use super::value::{self, AssignmentSource};
use crate::execution::terminal_unit::calls::borrowed_index_segment;
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{byte_sequence_carrier, terminal_field_identity};

/// The checked row for `view = v`, or `None` when the leaf family stays
/// with another store lane. The leaf's primitive width selects the arm: a
/// view equal to its leaf writes one whole store; a view wider than its
/// leaf decomposes only over a bounded-owned `u8` carrier — a borrowed
/// view's live extent has no declared footprint bound to distribute the
/// per-byte obligations against, so it stays refused.
pub(super) fn plan(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &Destination<'_>,
    view: PrimitiveType,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    // Byte leaves and their views have no local-rooted lowering: only an
    // exclusive borrowed parameter roots one.
    let store_root = checked_trees::CheckedStoreRoot::Parameter {
        position: destination.root.parameter_position()?,
    };
    let view_bytes = store_width(view)?;
    let (leaf, carriers) = destination.place.segments.split_last()?;
    let (path, element) = match leaf {
        facts::PlaceSegment::Field { symbol } => {
            let carrier = destination.leaf_carrier(program, carriers)?;
            let field = super::destination::exact_relevant_field(program, carrier.owner, *symbol)?;
            let element = program.primitive_type_reference(field.type_reference)?;
            if store_width(element)? != view_bytes {
                return None;
            }
            let mut path = carrier.path;
            path.push(CheckedUnitStructuralPathSegment::Field(
                terminal_field_identity(program, *symbol)?,
            ));
            (path, element)
        }
        facts::PlaceSegment::FixedIndex { index } => {
            let mut path = byte_field(program, destination, carriers)?;
            path.push(CheckedUnitStructuralPathSegment::FixedIndex(
                u64::try_from(*index).ok()?,
            ));
            (path, PrimitiveType::U8)
        }
        facts::PlaceSegment::Index { expression } => {
            let mut path = byte_field(program, destination, carriers)?;
            let segment = borrowed_index_segment(
                program,
                machine.symbol,
                state.symbol,
                usize::try_from(statement_index).ok()?,
                *expression,
            );
            let segment = segment?;
            path.push(segment);
            (path, PrimitiveType::U8)
        }
        _ => {
            return None;
        }
    };
    // Only a `u8` leaf decomposes into footprint bytes; every other leaf
    // must re-cover the view's whole width.
    if element != PrimitiveType::U8 && store_width(element)? != view_bytes {
        return None;
    }
    let value = value::assignment_value(
        program,
        facts,
        machine,
        state,
        &destination.root,
        statement_index,
        assignment,
        source,
        view,
        trace,
    );
    let value = value?;
    // A byte view carries no computation lane, exactly like a byte-field
    // store: the decomposition at lowering needs the authored pure value or
    // the statement's own call result.
    match value {
        checked_trees::CheckedStoreValue::Computation(_) => return None,
        checked_trees::CheckedStoreValue::Pure(_)
        | checked_trees::CheckedStoreValue::ScalarResult { .. } => {}
    }
    Some(CheckedUnitEffectOperationPlan::RepresentationViewStore(
        checked_trees::CheckedRepresentationViewStorePlan {
            statement_index,
            destination: store_root,
            path,
            element,
            view,
            value,
        },
    ))
}

/// The bounded-owned byte field one indexed recast leaf selects inside.
/// `carriers` ends at the field segment; the checked path it yields is the
/// record prefix only — the field's own segment is appended here, and the
/// caller appends the literal or runtime element segment after it.
fn byte_field(
    program: &TypedTrees,
    destination: &Destination<'_>,
    carriers: &[facts::PlaceSegment],
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    let (facts::PlaceSegment::Field { symbol }, prefix) = carriers.split_last()? else {
        return None;
    };
    let Carrier { owner, mut path } = destination.leaf_carrier(program, prefix)?;
    let field = super::destination::exact_relevant_field(program, owner, *symbol)?;
    // A bounded-owned byte sequence carries its declared capacity; a plain
    // `[u8; N]` field is the same bounded carrier spelled without a domain
    // restriction, its literal length the footprint bound. Borrowed
    // carriers stay refused — a live extent is no declared bound to
    // distribute the per-byte obligations against.
    let bounded = matches!(
        byte_sequence_carrier(program, field.type_reference, &[]),
        Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. })
    ) || matches!(
        program.type_reference_table.type_reference(field.type_reference),
        typed_trees::types::TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(_),
        } if program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
    );
    if !bounded {
        return None;
    }
    path.push(CheckedUnitStructuralPathSegment::Field(
        terminal_field_identity(program, *symbol)?,
    ));
    Some(path)
}

/// The storage width of one primitive carrier the store decomposes
/// against: a Boolean rides one byte beside the integers, so the view and
/// the leaf compare by footprint rather than by arithmetic family. Wider
/// than the element's own width the little-endian byte decomposition still
/// names only a bounded-owned `u8` carrier.
fn store_width(primitive_type: PrimitiveType) -> Option<u64> {
    Some(match primitive_type {
        PrimitiveType::Bool | PrimitiveType::U8 | PrimitiveType::I8 => 1,
        PrimitiveType::U16 | PrimitiveType::I16 => 2,
        PrimitiveType::U32 | PrimitiveType::I32 => 4,
        PrimitiveType::U64 | PrimitiveType::I64 => 8,
        _ => return None,
    })
}
