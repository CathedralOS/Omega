//! Replay custody for a `Reference` selection value carrying a `SharedBorrow`
//! argument. The checked plan recorded the authored `&place` expression's
//! exact root symbol and member/fixed-index path; replay rebuilds that place
//! and requires shared access on the authored borrow, the result type and the
//! argument, the exact path, the projected leaf's identity equal to the
//! result's record referent, and an established structural root place the
//! pipeline can carry -- linear-tolerant, because observing a place moves
//! nothing. There is no owned receipt or transfer to consume: a shared-borrow
//! join is provenance pass-through, so unlike `validate_projection` this leaf
//! leaves owned custody records untouched.

use super::{
    CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, StatementNode, unsupported,
};
use checked_trees::CheckedUnitStructuralArgumentPlan;
use symbols::SymbolHandle;

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    authored: &checked_trees::state::State,
    statement: u32,
    expression: ExpressionHandle,
    reference: checked_trees::types::TypeReferenceHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    let ExpressionNode::Borrow(borrow) = checked.expression_table.expression(expression) else {
        return unsupported("borrowed selection value lost its authored borrow");
    };
    if borrow.access != language_semantics::ReferenceAccess::Shared {
        return unsupported("borrowed selection value changed its authored access");
    }
    let Some(referent) = super::shared_borrow_record_referent(checked, reference) else {
        return unsupported("borrowed selection result lost its record referent");
    };
    if argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
        return unsupported("borrowed selection changed its planned access");
    }
    let (checked_path, root) = walk_exact_place(checked, machine, authored, borrow.target)?;
    if checked_path != argument.path {
        return unsupported("borrowed selection path does not match its projected path");
    }
    let leaf = validation::expression_result_type_reference(
        &checked.typed,
        machine,
        authored,
        borrow.target,
    )
    .ok_or(LoweringError::Unsupported(
        "borrowed selection lost its target type",
    ))?;
    if checked.normalized_type_identity(leaf).as_str() != argument.type_identity
        || checked.normalized_type_identity(leaf) != checked.normalized_type_identity(referent)
    {
        return unsupported("borrowed selection does not project the result referent");
    }
    validate_place_root(
        checked,
        machine,
        authored,
        statement,
        root,
        &argument.source,
    )?;
    let root_reference =
        validation::expression_result_type_reference(&checked.typed, machine, authored, root)
            .ok_or(LoweringError::Unsupported(
                "borrowed selection lost its root type",
            ))?;
    // The root carrier rule matches the referent rule: a shared borrow moves
    // nothing, so a `[linear]` root is an admissible place to observe through.
    if !validation::has_linear_owned_contents(
        &checked.typed,
        validation::unwrapped_type_reference(&checked.typed, root_reference)
            .unwrap_or(root_reference),
    ) {
        return unsupported(
            "borrowed selection root is not a structural place the pipeline can carry",
        );
    }
    Ok(())
}

/// A payloadless scalar-case leaf read through shared-borrowed storage
/// (`self.bitness` on a `&self` receiver): the retained root/path must
/// replay the authored member walk exactly as `validate` replays a `&place`
/// target, the projected leaf must be the result's declared case type, and
/// the source keeps the same root-establishment rule. The leaf's declared
/// case sum must be payloadless — there is no authored spelling that would
/// carry payload fields out of a borrowed case.
pub(super) fn validate_scalar_case_place(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    authored: &checked_trees::state::State,
    statement: u32,
    expression: ExpressionHandle,
    reference: checked_trees::types::TypeReferenceHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    if argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
        return unsupported("scalar case place changed its planned access");
    }
    let (checked_path, root) = walk_exact_place(checked, machine, authored, expression)?;
    if checked_path != argument.path || checked_path.is_empty() {
        return unsupported("scalar case place path does not match its projected path");
    }
    let leaf =
        validation::expression_result_type_reference(&checked.typed, machine, authored, expression)
            .ok_or(LoweringError::Unsupported(
                "scalar case place lost its leaf type",
            ))?;
    if checked.normalized_type_identity(leaf).as_str() != argument.type_identity
        || checked.normalized_type_identity(leaf) != checked.normalized_type_identity(reference)
    {
        return unsupported("scalar case place does not project the result type");
    }
    if !leaf_is_payloadless_case_sum(checked, reference) {
        return unsupported("scalar case place leaf is not a payloadless case sum");
    }
    validate_place_root(
        checked,
        machine,
        authored,
        statement,
        root,
        &argument.source,
    )?;
    Ok(())
}

/// An `Unrestricted` leaf read through shared-borrowed storage
/// (`self.scan_compare_type` on a `&self` receiver): same retained
/// root/path contract as a scalar-case leaf read, but the leaf's contents
/// are copied into a fresh owned place — payload-bearing leaves that a
/// case fan-out cannot reconstruct are admitted here. The leaf must still
/// be `Unrestricted`: an affine subtree can only ride the move contract.
pub(super) fn validate_copied_place(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    authored: &checked_trees::state::State,
    statement: u32,
    expression: ExpressionHandle,
    reference: checked_trees::types::TypeReferenceHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    if argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
        return unsupported("copied place changed its planned access");
    }
    let (checked_path, root) = walk_exact_place(checked, machine, authored, expression)?;
    if checked_path != argument.path || checked_path.is_empty() {
        return unsupported("copied place path does not match its projected path");
    }
    let leaf =
        validation::expression_result_type_reference(&checked.typed, machine, authored, expression)
            .ok_or(LoweringError::Unsupported(
                "copied place lost its leaf type",
            ))?;
    if checked.normalized_type_identity(leaf).as_str() != argument.type_identity
        || checked.normalized_type_identity(leaf) != checked.normalized_type_identity(reference)
    {
        return unsupported("copied place does not project the result type");
    }
    let Some(unwrapped) = validation::unwrapped_type_reference(&checked.typed, reference) else {
        return unsupported("copied place lost its leaf type");
    };
    if checked.typed.type_multiplicity(unwrapped) != language_semantics::Multiplicity::Unrestricted
    {
        return unsupported("copied place leaf is not unrestricted");
    }
    validate_place_root(
        checked,
        machine,
        authored,
        statement,
        root,
        &argument.source,
    )?;
    Ok(())
}

/// The leaf type must be an unrestricted sum whose every case carries no
/// payload field — observing the active case is then the whole read, and no
/// borrowed payload needs a spelling the authored expression never offered.
fn leaf_is_payloadless_case_sum(
    checked: &CheckedTrees,
    reference: checked_trees::types::TypeReferenceHandle,
) -> bool {
    let Some(unwrapped) = validation::unwrapped_type_reference(&checked.typed, reference) else {
        return false;
    };
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(unwrapped)
    else {
        return false;
    };
    let Some(data) = checked
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)
    else {
        return false;
    };
    let members = checked.data_members(data);
    !members.is_empty()
        && members.iter().all(|member| {
            matches!(member, checked_trees::data::DataMember::Variant(variant) if checked.data_payload_fields(variant).is_empty())
        })
}

/// Walk an authored member/indexed place chain to its root name, rebuilding
/// the retained field/fixed-index path against the same declaration and
/// literal-bound rules the checker committed to.
fn walk_exact_place(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    authored: &checked_trees::state::State,
    target: ExpressionHandle,
) -> Result<
    (
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        ExpressionHandle,
    ),
    LoweringError,
> {
    let mut checked_path = Vec::new();
    let mut cursor = target;
    let root = loop {
        match checked.expression_table.expression(cursor) {
            ExpressionNode::Member(member) => {
                if member.case_variant.is_some() {
                    return unsupported("borrowed selection target uses a case member");
                }
                let receiver = validation::declared_place_type_raw(
                    &checked.typed,
                    machine,
                    Some(authored),
                    member.receiver,
                )
                .and_then(|receiver| validation::unwrapped_type_reference(&checked.typed, receiver))
                .ok_or(LoweringError::Unsupported(
                    "borrowed selection lost its receiver type",
                ))?;
                let checked_trees::types::TypeReferenceNode::Named { symbol, .. } =
                    checked.type_reference_table.type_reference(receiver)
                else {
                    return unsupported("borrowed selection receiver is not a named record");
                };
                // A `self` receiver keeps the machine-keyed `Self` alias; its
                // owner is the machine's attached data declaration, and the
                // authored member symbol lives in the receiver-view space, so
                // the field resolves by name there.
                let self_receiver = *symbol == machine.symbol;
                let owner_symbol = if self_receiver {
                    machine.attached_data_symbol
                } else {
                    *symbol
                };
                let owner = checked
                    .data_definitions()
                    .iter()
                    .find(|owner| owner.symbol == owner_symbol)
                    .ok_or(LoweringError::Unsupported(
                        "borrowed selection field owner is absent",
                    ))?;
                let field = validation::exact_data_member_field(
                    &checked.typed,
                    owner,
                    if self_receiver {
                        SymbolHandle::invalid()
                    } else {
                        member.member_symbol
                    },
                    member.member.as_str(),
                    None,
                )
                .ok_or(LoweringError::Unsupported(
                    "borrowed selection lost its member declaration",
                ))?;
                if field.relevance.is_erased() {
                    return unsupported("borrowed selection reads an erased member");
                }
                checked_path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
                ));
                cursor = member.receiver;
            }
            // A literal fixed index is an exact place segment the same way a
            // record field is: the checked planner recorded its ordinal after
            // the same literal-length bound check replayed here.
            ExpressionNode::Indexed(indexed) => {
                let ExpressionNode::Integer(index) =
                    checked.expression_table.expression(indexed.index)
                else {
                    return unsupported(
                        "borrowed selection target indexes through a non-literal ordinal",
                    );
                };
                let index = index
                    .value_bignum()
                    .and_then(|value| value.to_u64())
                    .ok_or(LoweringError::Unsupported(
                        "borrowed selection index exceeds u64",
                    ))?;
                let container = validation::declared_place_type_raw(
                    &checked.typed,
                    machine,
                    Some(authored),
                    indexed.collection,
                )
                .and_then(|container| {
                    validation::unwrapped_type_reference(&checked.typed, container)
                })
                .ok_or(LoweringError::Unsupported(
                    "borrowed selection index has no declared collection",
                ))?;
                let checked_trees::types::TypeReferenceNode::FixedArray {
                    length: checked_trees::types::FixedArrayLength::Literal(length),
                    ..
                } = checked.type_reference_table.type_reference(container)
                else {
                    return unsupported("borrowed selection index has no literal array length");
                };
                if usize::try_from(index)
                    .ok()
                    .is_none_or(|index| index >= *length)
                {
                    return unsupported("borrowed selection index is out of bounds");
                }
                checked_path.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    index,
                ));
                cursor = indexed.collection;
            }
            ExpressionNode::Name(_) => break cursor,
            _ => return unsupported("borrowed selection target is not an exact place"),
        }
    };
    checked_path.reverse();
    Ok((checked_path, root))
}

/// The root of an exact observed place: a uniquely named parameter or an
/// already-established local whose retained source plan still matches the
/// authored name.
fn validate_place_root(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    authored: &checked_trees::state::State,
    statement: u32,
    root: ExpressionHandle,
    source: &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
) -> Result<(), LoweringError> {
    let ExpressionNode::Name(name) = checked.expression_table.expression(root) else {
        return unsupported("borrowed selection target is not an exact place");
    };
    if !name.symbol.is_valid()
        || name.head_symbol != name.symbol
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return unsupported("borrowed selection root is not an exact name");
    }
    if !super::owned_selection::source_plan_matches(checked, authored.symbol, name.symbol, source) {
        return unsupported("borrowed selection source does not match its root");
    }
    // An authored `self` name retains the durable machine symbol; it roots at
    // the is_self parameter rather than at a same-named parameter row.
    if !checked.state_parameters(authored).iter().any(|parameter| {
        parameter.symbol == name.symbol || (parameter.is_self && name.symbol == machine.symbol)
    }) {
        let mut locals = checked
            .statement_table
            .statements(authored.statement_nodes)
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| match candidate {
                StatementNode::LocalData(local) if local.symbol == name.symbol => {
                    Some((index, local))
                }
                _ => None,
            });
        let Some((index, local)) = locals.next() else {
            return unsupported("borrowed selection loses its authored root");
        };
        if locals.next().is_some() || index >= statement as usize || !local.initial_value.is_valid()
        {
            return unsupported("borrowed selection root is not uniquely established");
        }
    }
    Ok(())
}
