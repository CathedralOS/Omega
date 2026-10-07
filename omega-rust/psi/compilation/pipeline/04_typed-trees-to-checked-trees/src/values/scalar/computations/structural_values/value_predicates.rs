//! Structural value-kind admission: which expressions already count as a
//! record, case, borrow, view, or fixed-array value, plus the zeroed literal
//! parts fresh establishment seeds.

use super::super::{
    ExpressionHandle, ExpressionNode, PrimitiveType, SymbolHandle, TypeReferenceNode, TypedTrees,
};
use super::place_types::{
    borrowed_place_leaf_type, canonical_place_is_borrowable, copied_place_type,
    projected_leaf_type, symbol_declared_type,
};
use checked_trees::CheckedStructuralValueKind;
use typed_trees::types::TypeReferenceHandle;

/// Lifetime arguments erase without specializing the record's field types.
/// Construction and whole-place moves must agree on that runtime carrier;
/// type arguments still require substitution and remain outside this lookup.
pub(super) fn closed_data_symbol(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } if program
            .type_reference_table
            .type_reference_handles(*arguments)
            .is_empty() =>
        {
            Some(*base_symbol)
        }
        _ => None,
    }
}

// Classify the destination before inserting scalar operand roots. A scalar or
// array Match must not leave partial structural plans when a later arm fails.
// A match arm may also select an owned child projected by exact field or
// fixed-index path; its structural node is a Projection over the root place,
// which the owned-selection transfer evidence then checks path-for-path.
/// A case literal carrying non-scalar payload registers its construction root
/// the same way a record literal does: each authored field keeps its own
/// scalar or nested structural value under the selected case.

// Classify the destination before inserting scalar operand roots. A scalar or
// array Match must not leave partial structural plans when a later arm fails.
// A match arm may also select an owned child projected by exact field or
// fixed-index path; its structural node is a Projection over the root place,
// which the owned-selection transfer evidence then checks path-for-path.
/// A case literal carrying non-scalar payload registers its construction root
/// the same way a record literal does: each authored field keeps its own
/// scalar or nested structural value under the selected case.
pub(in crate::values::scalar::computations) fn is_structural_case_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::StructLiteral(_)
    ) {
        return false;
    }
    let Some(constructor) = validation::structural_case_constructor(program, expression, expected)
    else {
        return false;
    };
    let Some(reference) = validation::unwrapped_type_reference(program, expected) else {
        return false;
    };
    program.normalized_type_identity(constructor.type_reference)
        == program.normalized_type_identity(reference)
}

pub(in crate::values::scalar::computations) fn is_record_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::StructLiteral(_) | ExpressionNode::Match(_)
    ) {
        return false;
    }
    let shared_referent = shared_record_reference(program, expected);
    let Some(reference) = validation::unwrapped_type_reference(program, expected) else {
        return false;
    };
    // A `&T` selection result plans its borrowed arms against the referent
    // record; owned results keep their own carrier.
    let reference = shared_referent.unwrap_or(reference);
    // A primitive referent declares no record to resolve, so there is no field
    // roster to check and no constructor arm it could ever admit: its only
    // admitted arm is the shared borrow of an exact place checked below. The
    // invalid record symbol makes that explicit rather than implied -- no
    // authored `StructLiteral` can carry it.
    let primitive_referent =
        shared_referent.is_some() && program.primitive_type_reference(reference).is_some();
    let mut record_symbol = SymbolHandle::invalid();
    if !primitive_referent {
        let Some(record) = program.data_definitions().iter().find(|record| {
            closed_data_symbol(program, reference) == Some(record.symbol)
                || validation::member_type_substitutions(program, record, reference).is_some()
        }) else {
            return false;
        };
        let symbol = record.symbol;
        if program
            .data_members(record)
            .iter()
            .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
        {
            return false;
        }
        record_symbol = symbol;
    }
    let symbol = &record_symbol;
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match program.expression_table.expression(expression) {
            ExpressionNode::StructLiteral(literal)
                if literal.case_symbol.is_none() && literal.type_symbol == *symbol => {}
            ExpressionNode::Name(_)
                if validation::affine_owned_value_source(program, expression, expected)
                    .is_some() => {}
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
                if shared_referent.is_none()
                    && projected_leaf_type(program, expression).is_some_and(|reference| {
                        program.normalized_type_identity(reference)
                            == program.normalized_type_identity(expected)
                            && program.type_multiplicity(reference)
                                == language_semantics::Multiplicity::Affine
                    }) => {}
            // A shared-borrow arm does not move its referent's owner: the
            // canonical place is rebuilt with exact state scope during value
            // planning and replayed again at lowering.
            ExpressionNode::Borrow(borrow)
                if shared_referent.is_some_and(|referent| {
                    borrow.access == language_semantics::ReferenceAccess::Shared
                        && crate::flow::canonical_place_from_expression(program, borrow.target)
                            .is_some_and(|place| canonical_place_is_borrowable(&place))
                        && borrowed_place_leaf_type(program, borrow.target).is_some_and(|leaf| {
                            program.normalized_type_identity(leaf)
                                == program.normalized_type_identity(referent)
                        })
                }) => {}
            ExpressionNode::Call(call) => {
                // A call's structural product is a fresh owned arm value: the
                // builder admits it under the same exact return-type and
                // whole-value custody rule it checks when constructing the node.
                let Some(returned) = crate::flow::call_target_return_type(
                    program,
                    crate::flow::call_target_symbol(program, call),
                ) else {
                    return false;
                };
                if program.normalized_type_identity(returned)
                    != program.normalized_type_identity(expected)
                    || !(validation::has_contents(
                        program,
                        returned,
                        validation::ContentsRequirement::StructuralConstruction,
                    ) || validation::has_contents(
                        program,
                        returned,
                        validation::ContentsRequirement::LinearOwned,
                    ) || validation::reference_result_custody::is_reference_record(
                        program, returned,
                    ))
                {
                    return false;
                }
            }
            ExpressionNode::Match(dispatch) => {
                let arms = program.expression_table.match_arms(dispatch.arms);
                if arms.is_empty() {
                    return false;
                }
                pending.extend(arms.iter().map(|arm| arm.value));
            }
            _ => return false,
        }
    }
    true
}

/// A `let view: &T = &place` establishment admits the same shared-borrow
/// carrier a selection arm does, at statement scope: the referent's owner is
/// never moved, and `borrowed_place` rebuilds the authored target's canonical
/// root and path into a `SharedBorrow` source. This is the driver's
/// pre-classification only -- `borrowed_place` re-checks every condition when
/// constructing the node, and it does no plan mutation before that admission,
/// so a rejected borrow still leaves no partial structural plan.

/// A `let view: &T = &place` establishment admits the same shared-borrow
/// carrier a selection arm does, at statement scope: the referent's owner is
/// never moved, and `borrowed_place` rebuilds the authored target's canonical
/// root and path into a `SharedBorrow` source. This is the driver's
/// pre-classification only -- `borrowed_place` re-checks every condition when
/// constructing the node, and it does no plan mutation before that admission,
/// so a rejected borrow still leaves no partial structural plan.
pub(in crate::values::scalar::computations) fn is_shared_borrow_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) else {
        return false;
    };
    borrow.access == language_semantics::ReferenceAccess::Shared
        && shared_record_reference(program, expected).is_some_and(|referent| {
            crate::flow::canonical_place_from_expression(program, borrow.target)
                .is_some_and(|place| canonical_place_is_borrowable(&place))
                && borrowed_place_leaf_type(program, borrow.target).is_some_and(|leaf| {
                    program.normalized_type_identity(leaf)
                        == program.normalized_type_identity(referent)
                })
        })
}

/// A LocalData initializer whose declared type is a shared or mutable slice. The
/// value itself is adjudicated by `slice_view_place`, which rejoins
/// the checked loan the view holds; this only decides that the destination is
/// a view at all, so the ordinary owned classifications keep every other local.

/// A LocalData initializer whose declared type is a shared or mutable slice. The
/// value itself is adjudicated by `slice_view_place`, which rejoins
/// the checked loan the view holds; this only decides that the destination is
/// a view at all, so the ordinary owned classifications keep every other local.
pub(in crate::values::scalar::computations) fn is_borrowed_slice_view_value(
    program: &TypedTrees,
    expected: TypeReferenceHandle,
) -> bool {
    crate::execution::terminal_unit::types::borrowed_slice_view_element(program, expected, &[])
        .is_some()
        || matches!(
            crate::execution::terminal_unit::types::byte_sequence_carrier(program, expected, &[]),
            Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
        )
}

/// A LocalData initializer copying one element of a borrowed view
/// (`let chosen: Entry = tail[index]`): a single-element index over a bare
/// name whose declared type is a view of exactly the copyable record the
/// local declares. This is `view_element_copy`'s own admission, consulted by
/// the builder branch that owns it -- the owned-place arm in `computations`
/// offers every projected place and does not re-derive this shape.

/// A LocalData initializer copying one element of a borrowed view
/// (`let chosen: Entry = tail[index]`): a single-element index over a bare
/// name whose declared type is a view of exactly the copyable record the
/// local declares. This is `view_element_copy`'s own admission, consulted by
/// the builder branch that owns it -- the owned-place arm in `computations`
/// offers every projected place and does not re-derive this shape.
pub(super) fn is_view_element_copy(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return false;
    };
    let ExpressionNode::Name(name) = program.expression_table.expression(indexed.collection) else {
        return false;
    };
    !matches!(
        program.expression_table.expression(indexed.index),
        ExpressionNode::Range(_)
    ) && copied_place_type(program, expected)
        && !crate::execution::terminal_unit::types::borrowed_slice_view(program, expected)
        && symbol_declared_type(program, name.symbol)
            .and_then(|declared| {
                crate::execution::terminal_unit::types::borrowed_slice_view_element(
                    program,
                    declared,
                    &[],
                )
            })
            .is_some_and(|element| {
                program.normalized_type_identity(element)
                    == program.normalized_type_identity(expected)
            })
}

/// A view-copied record field's carrier: a bare primitive, or a primitive
/// under exactly one arithmetic policy. A policy changes operations, not the
/// stored bits, so the element read and the fresh record carry the same
/// scalar; a range or any other qualification needs its own evidence and
/// keeps the copy declined.

/// A view-copied record field's carrier: a bare primitive, or a primitive
/// under exactly one arithmetic policy. A policy changes operations, not the
/// stored bits, so the element read and the fresh record carry the same
/// scalar; a range or any other qualification needs its own evidence and
/// keeps the copy declined.
pub(super) fn plain_or_policy_scalar_field(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { .. } | TypeReferenceNode::DynamicTrait { .. } => true,
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            matches!(
                program.type_reference_table.constraint_span(*constraints),
                Some([typed_trees::types::TypeConstraintNode::ArithmeticDomain(_)])
            ) && matches!(
                program.type_reference_table.type_reference(*base_type),
                TypeReferenceNode::Named { .. }
            )
        }
        _ => false,
    }
}

/// A fixed-array construction of structural elements spelled as a literal. Primitive arrays keep
/// their scalar-element route; structural elements each register their own
/// value node, the same way record fields do inside a constructor.

/// A fixed-array construction of structural elements spelled as a literal. Primitive arrays keep
/// their scalar-element route; structural elements each register their own
/// value node, the same way record fields do inside a constructor.
pub(in crate::values::scalar::computations) fn is_fixed_array_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::ArrayLiteral(_)
    ) && validation::unwrapped_type_reference(program, expected).is_some_and(|reference| {
        matches!(
            program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::FixedArray { .. }
        )
    }) && !validation::is_closed_primitive_array_type(program, expected)
}

/// The referent of a shared-borrow result type (`&T` where `T` is a named
/// record, or a primitive, that the structural pipeline can carry). `None` for
/// owned results and for borrows the structural pipeline cannot carry. The
/// carrier rule is linear-tolerant: a shared borrow observes the referent
/// without moving it, so a linear declaration's claim never reaches this join
/// and the referent's own owner keeps the whole discharge obligation.
///
/// A primitive referent resolves to no data declaration, so the record rule
/// cannot name its shape. `add_type` registers it as a `PrimitiveScalar`
/// structural carrier exactly as it does for a borrowed primitive elsewhere,
/// and the borrow still denotes the original storage rather than a snapshot.

/// The referent of a shared-borrow result type (`&T` where `T` is a named
/// record, or a primitive, that the structural pipeline can carry). `None` for
/// owned results and for borrows the structural pipeline cannot carry. The
/// carrier rule is linear-tolerant: a shared borrow observes the referent
/// without moving it, so a linear declaration's claim never reaches this join
/// and the referent's own owner keeps the whole discharge obligation.
///
/// A primitive referent resolves to no data declaration, so the record rule
/// cannot name its shape. `add_type` registers it as a `PrimitiveScalar`
/// structural carrier exactly as it does for a borrowed primitive elsewhere,
/// and the borrow still denotes the original storage rather than a snapshot.
pub(super) fn shared_record_reference(
    program: &TypedTrees,
    expected: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(expected)
    else {
        return None;
    };
    // Borrowing observes the declared storage; it does not transfer that
    // storage's contents. Requiring an owned-content shape here wrongly
    // rejects records that contain references. Their nested loans remain
    // with the referent, under the ordinary borrow and lifetime checks.
    if *access != language_semantics::ReferenceAccess::Shared {
        return None;
    }
    if program.primitive_type_reference(*referee).is_some() {
        return Some(*referee);
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(*referee)
    else {
        return None;
    };
    let record = program
        .data_definitions()
        .iter()
        .find(|record| record.symbol == *symbol)?;
    (!program
        .data_members(record)
        .iter()
        .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_))))
    .then_some(*referee)
}

/// The declared type of a shared borrow's exact target without state scope: a
/// whole local's declared type, or a projected leaf resolved through its
/// root's declaration. `canonical_place_from_expression` keeps the root and
/// every segment exact.

/// An omitted runtime field's zero value is admitted only for a
/// `[scalar; N]` leaf: a literal-length fixed array whose element resolves
/// to one plain integer primitive. Anything else — borrowed carriers,
/// bounded integers, nested composites — has no authored zero spelling to
/// synthesize, so the field must stay authored.
pub(super) fn zeroed_scalar_array_parts(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(PrimitiveType, u64)> {
    let reference = validation::unwrapped_type_reference(program, type_reference)?;
    let TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    let primitive_type = program.primitive_type_reference(*element_type)?;
    if !matches!(
        primitive_type,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) {
        return None;
    }
    Some((primitive_type, u64::try_from(*length).ok()?))
}

/// The checked literal-zero computation one synthesized zeroed field
/// element contributes: a landed `0` in the declared element carrier.

/// The checked literal-zero computation one synthesized zeroed field
/// element contributes: a landed `0` in the declared element carrier.
pub(super) fn zeroed_integer_literal(
    primitive_type: PrimitiveType,
) -> Option<checked_trees::CheckedScalarExpression> {
    let landed_type = match primitive_type {
        PrimitiveType::I8 => numerics::literals::LandedIntegerType::I8,
        PrimitiveType::I16 => numerics::literals::LandedIntegerType::I16,
        PrimitiveType::I32 => numerics::literals::LandedIntegerType::I32,
        PrimitiveType::I64 => numerics::literals::LandedIntegerType::I64,
        PrimitiveType::U8 => numerics::literals::LandedIntegerType::U8,
        PrimitiveType::U16 => numerics::literals::LandedIntegerType::U16,
        PrimitiveType::U32 => numerics::literals::LandedIntegerType::U32,
        PrimitiveType::U64 => numerics::literals::LandedIntegerType::U64,
        _ => return None,
    };
    Some(checked_trees::CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_parts(
            false,
            numerics::literals::IntegerRadix::Decimal,
            "0",
        )
        .ok()?
        .with_landing(numerics::literals::IntegerLanding {
            landed_type,
            domain: numerics::arithmetic::ArithmeticDomain::Exact,
        }),
    })
}

/// The terminal expression carrying a projection's storage: the whole local
/// name for an owned place source, or the producing call for a structural
/// product source.

/// Existing storage transfers through the call's canonical source identity.
/// Copies and evaluated descriptors establish values; a whole-place transfer
/// must not acquire another result identity or another ownership event.
/// Existing storage transfers through the call's canonical source identity.
/// Copies and evaluated descriptors establish values; a whole-place transfer
/// must not acquire another result identity or another ownership event.
pub(super) fn argument_needs_establishment(kind: &CheckedStructuralValueKind) -> bool {
    let CheckedStructuralValueKind::Place(argument) = kind else {
        return true;
    };
    use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
    match &argument.source {
        CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
        | CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { .. }
        | CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
        | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
        | CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }
        | CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal { .. }
        | CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. } => false,
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
        | CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
        | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. } => true,
    }
}
