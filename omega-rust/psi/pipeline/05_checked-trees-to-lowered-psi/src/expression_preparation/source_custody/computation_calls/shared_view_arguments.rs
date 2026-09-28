//! Shared view operands retain their declared owner and exact call
//! occurrence. A `&[T]` actual loans an existing caller place whose carrier is
//! itself a view, and a `&[u8]` actual may instead spell a literal payload.
//! The borrowed view has no members, so validation stops at the carrier's
//! element and the authored place's identity; nothing here depends on which
//! element the view carries, so byte and element views share this one check.

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::signature::StateParameter;
use checked_trees::state::State;
use checked_trees::types::PrimitiveType;
use checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{
    BorrowCallFact, CheckedStructuralAccess, CheckedTrees, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use language_core::ReferenceAccess;

/// The formal's `&[T]` referent slice when it spells a shared view.
pub(super) fn shared_view_referent(
    checked: &CheckedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let mut reference = type_reference;
    loop {
        match checked.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference {
                access: ReferenceAccess::Shared,
                referee,
                ..
            } => {
                return matches!(
                    checked.type_reference_table.type_reference(*referee),
                    TypeReferenceNode::Slice { .. }
                )
                .then_some(*referee);
            }
            _ => return None,
        }
    }
}

fn is_byte_slice(checked: &CheckedTrees, slice: TypeReferenceHandle) -> bool {
    matches!(
        checked.type_reference_table.type_reference(slice),
        TypeReferenceNode::Slice { element_type }
            if checked.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
    )
}

pub(super) fn validate(
    checked: &CheckedTrees,
    state: &State,
    statement: u32,
    target: &StateParameter,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
    call: &BorrowCallFact,
    explicit_position: Option<usize>,
) -> Result<(), LoweringError> {
    let Some(referee) = shared_view_referent(checked, target.type_reference) else {
        return unsupported("view operand requires a shared slice formal");
    };
    if argument.access != CheckedStructuralAccess::SharedBorrow {
        return unsupported("view operand changed its shared custody");
    }
    if checked.normalized_type_identity(referee).as_str() != argument.type_identity {
        return unsupported("view operand changed its declared referent type");
    }
    if let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } = &argument.source
    {
        if !is_byte_slice(checked, referee) {
            return unsupported("literal view payload requires a byte-slice formal");
        }
        let ExpressionNode::String(source) = checked.expression_table.expression(expression) else {
            return unsupported("literal byte view lost its authored bytes");
        };
        if bytes.as_slice() != source.as_ref() || !argument.path.is_empty() {
            return unsupported("literal byte view differs from its authored bytes");
        }
        return Ok(());
    }
    if !matches!(
        argument.source,
        CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
    ) {
        return unsupported("view operand has no supported source owner");
    }
    let named = match checked.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) if borrow.access == ReferenceAccess::Shared => borrow.target,
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => expression,
        _ => return unsupported("view operand has no named or projected source"),
    };
    let (caller, _) = super::authored_state(checked, state.symbol)?;
    let source = crate::emission::call_source_custody::projected_receivers::source(
        checked,
        caller.symbol,
        state.symbol,
        statement as usize,
        named,
    )?;
    if source.path != argument.path
        || argument.path.iter().any(|segment| {
            !matches!(
                segment,
                checked_trees::CheckedUnitStructuralPathSegment::Field(_)
            )
        })
    {
        return unsupported("view operand substituted its projected path");
    }
    super::borrow_rows::validate_shared_place_argument_at(
        checked,
        call,
        explicit_position.ok_or(LoweringError::Unsupported(
            "view operand lost its observation position",
        ))?,
        source.captured_place(),
    )?;
    Ok(())
}
