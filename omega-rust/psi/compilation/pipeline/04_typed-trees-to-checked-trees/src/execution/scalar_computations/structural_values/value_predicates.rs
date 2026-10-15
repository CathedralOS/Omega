//! Structural value-kind admission: which expressions already count as a
//! record, case, borrow, view, or fixed-array value, plus the zeroed literal
//! parts fresh establishment seeds.

use super::super::{
    ExpressionHandle, ExpressionNode, PrimitiveType, SymbolHandle, TypeReferenceNode,
};
use super::place_types::{
    borrowed_place_leaf_type, canonical_place_is_borrowable, projected_leaf_type,
};
use checked_trees::CheckedStructuralValueKind;
use typed_trees::types::TypeReferenceHandle;

/// A call supplies its declared storage carrier; the receiving qualification
/// is a separate occurrence-bound checking judgment. Keeping the full receiving
/// type on the value node preserves that judgment without requiring the callee
/// to spell its ensured memberships in its return type as well.
pub(super) fn structural_call_carrier_matches(
    program: &dyn ::validation::CheckingInput,
    returned: TypeReferenceHandle,
    expected: TypeReferenceHandle,
) -> bool {
    let (Some(returned), Some(expected)) = (
        validation::unwrapped_type_reference(program, returned),
        validation::unwrapped_type_reference(program, expected),
    ) else {
        return false;
    };
    program.normalized_type_identity(returned) == program.normalized_type_identity(expected)
}

// Classify the destination before inserting scalar operand roots. A scalar or
// array Match must not leave partial structural plans when a later arm fails.
// A match arm may also select an owned child projected by exact field or
// fixed-index path; its structural node is a Projection over the root place,
// which the owned-selection transfer evidence then checks path-for-path.
/// A case literal carrying non-scalar payload registers its construction root
/// the same way a record literal does: each authored field keeps its own
/// scalar or nested structural value under the selected case.
pub(in crate::execution::scalar_computations) fn is_structural_case_value(
    program: &dyn ::validation::CheckingInput,
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

pub(in crate::execution::scalar_computations) fn is_record_value(
    program: &dyn ::validation::CheckingInput,
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
        let Some(record) = program.closed_data_definition(reference) else {
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
                if literal.case_symbol.is_none()
                    && program
                        .closed_data_constructor_definition(reference, literal.type_symbol)
                        .is_some_and(|record| record.symbol == *symbol) => {}
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
                        && validation::canonical_place_from_expression(program, borrow.target)
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
                let Some(returned) = validation::call_target_return_type(
                    program,
                    validation::call_target_symbol(program, call),
                ) else {
                    return false;
                };
                if !structural_call_carrier_matches(program, returned, expected)
                    || !(validation::has_contents(
                        program,
                        returned,
                        &[],
                        validation::ContentsRequirement::StructuralConstruction,
                    ) || validation::has_contents(
                        program,
                        returned,
                        &[],
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

/// A fixed-array construction of structural elements spelled as a literal. Primitive arrays keep
/// their scalar-element route; structural elements each register their own
/// value node, the same way record fields do inside a constructor.
pub(in crate::execution::scalar_computations) fn is_fixed_array_value(
    program: &dyn ::validation::CheckingInput,
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
    }) && !validation::scalar_array_preserves_memberships(program, expected)
}

/// The referent of a shared record or primitive view. This classifies the
/// record-value consumer, not general reference formation: arrays and cases
/// retain their own ordinary producers. The
/// carrier rule is linear-tolerant: a shared borrow observes the referent
/// without moving it, so a linear declaration's claim never reaches this join
/// and the referent's own owner keeps the whole discharge obligation.
///
/// A primitive referent resolves to no data declaration, so the record rule
/// cannot name its shape. `add_type` registers it as a `PrimitiveScalar`
/// structural carrier exactly as it does for a borrowed primitive elsewhere,
/// and the borrow still denotes the original storage rather than a snapshot.
pub(super) fn shared_record_reference(
    program: &dyn ::validation::CheckingInput,
    expected: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let shell = validation::reference_shell(program, expected)?;
    // Borrowing observes the declared storage; it does not transfer that
    // storage's contents. Requiring an owned-content shape here wrongly
    // rejects records that contain references. Their nested loans remain
    // with the referent, under the ordinary borrow and lifetime checks.
    if shell.access != language_semantics::ReferenceAccess::Shared {
        return None;
    }
    let carrier = validation::peel_declared_constraints(program, shell.referee);
    if program.primitive_type_reference(carrier).is_some() {
        return Some(shell.referee);
    }
    let record = program.closed_data_definition(carrier)?;
    (!program
        .data_members(record)
        .iter()
        .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_))))
    .then_some(shell.referee)
}

/// An omitted runtime field's zero value is admitted only for a
/// `[scalar; N]` leaf: a literal-length fixed array whose element resolves
/// to one plain integer primitive. Anything else — borrowed carriers,
/// bounded integers, nested composites — has no authored zero spelling to
/// synthesize, so the field must stay authored.
pub(super) fn zeroed_scalar_array_parts(
    program: &dyn ::validation::CheckingInput,
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
    if !primitive_type.is_fixed_integer() {
        return None;
    }
    Some((primitive_type, u64::try_from(*length).ok()?))
}

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
        | CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { .. }
        | CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }
        | CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal { .. }
        | CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. } => false,
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
        | CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
        | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. } => true,
    }
}
