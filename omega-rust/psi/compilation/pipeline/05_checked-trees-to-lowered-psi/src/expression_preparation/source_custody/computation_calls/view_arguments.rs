//! Readable view operands retain their declared owner and exact call
//! occurrence. A shared or mutable view actual loans an existing caller place whose carrier is
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

/// The formal's exact readable slice referent and access.
pub(super) fn readable_view_referent(
    checked: &CheckedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, ReferenceAccess)> {
    // Constraints can qualify either side of the reference shell. Rejoin the
    // slice carrier through the common classifier; qualifier admission remains
    // a separate obligation on the exact argument and formal.
    let view = validation::slice_view_shell(&checked.typed, type_reference, &[])?;
    matches!(
        view.access,
        ReferenceAccess::Shared | ReferenceAccess::Mutable
    )
    .then_some((view.slice, view.access))
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
    let Some((referee, target_access)) = readable_view_referent(checked, target.type_reference)
    else {
        return unsupported("view operand requires a readable slice formal");
    };
    let (expected_access, expected_kind) = match target_access {
        ReferenceAccess::Shared => (
            CheckedStructuralAccess::SharedBorrow,
            checked_trees::BorrowAccessKind::Read,
        ),
        ReferenceAccess::Mutable => (
            CheckedStructuralAccess::MutableBorrow,
            checked_trees::BorrowAccessKind::Mutable,
        ),
        ReferenceAccess::WriteOnly => return unsupported("view operand requires readable access"),
    };
    if argument.access != expected_access {
        return unsupported("view operand changed its formal custody");
    }
    if checked.normalized_type_identity(referee).as_str() != argument.type_identity {
        return unsupported("view operand changed its declared referent type");
    }
    if let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } = &argument.source
    {
        if target_access != ReferenceAccess::Shared || !is_byte_slice(checked, referee) {
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
        ExpressionNode::Borrow(borrow) if borrow.access == target_access => borrow.target,
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
    // The authored endpoint and retained storage owner must agree independently:
    // another same-typed descriptor cannot supply this call occurrence.
    let parameters = checked.state_parameters(state);
    let source_reference = match argument.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            if symbol != source.root
                || parameters
                    .iter()
                    .any(|parameter| parameter.symbol == symbol)
            {
                return unsupported("view operand substituted its local owner");
            }
            let local = super::preceding_local(
                checked,
                state,
                symbol,
                statement as usize,
                "view operand lost its local declaration",
                "view operand lost its prior establishment",
            )?;
            if local.is_mutable
                && matches!(
                    checked
                        .type_reference_table
                        .type_reference(local.type_reference),
                    TypeReferenceNode::Reference { .. }
                )
            {
                return unsupported("view operand uses a rebindable local carrier");
            }
            local.type_reference
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let parameter = super::structural_parameter_at(
                checked,
                parameters,
                parameter_index,
                "view operand lost its source parameter",
            )?;
            if parameter.is_const
                || parameter.symbol != source.root
                || !super::parameter_binding_is_unique(checked, state, source.root)
            {
                return unsupported("view operand substituted its parameter owner");
            }
            parameter.type_reference
        }
        _ => return unsupported("view operand has no retained source owner"),
    };
    // Whole view carriers retain their declared access. Projected shared view
    // fields keep the ordinary exact place-type replay below; they never grant
    // exclusive access merely because the containing record is mutable.
    let endpoint_reference = if argument.path.is_empty() {
        source_reference
    } else {
        validation::declared_place_type_raw(&checked.typed, caller, Some(state), named).ok_or(
            LoweringError::Unsupported("view operand lost its projected declaration"),
        )?
    };
    let Some((source_referee, source_access)) = readable_view_referent(checked, endpoint_reference)
    else {
        return unsupported("view operand has no readable view carrier declaration");
    };
    if checked.normalized_type_identity(source_referee) != checked.normalized_type_identity(referee)
        || (target_access == ReferenceAccess::Mutable && source_access != ReferenceAccess::Mutable)
    {
        return unsupported("view operand widens its declared carrier access or type");
    }
    super::borrow_rows::validate_place_argument_at(
        checked,
        call,
        explicit_position.ok_or(LoweringError::Unsupported(
            "view operand lost its observation position",
        ))?,
        source.captured_place(),
        expected_kind,
    )?;
    Ok(())
}
