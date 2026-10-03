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
use crate::expression_preparation::source_custody::structural_places::walk_exact_place;
use checked_trees::CheckedUnitStructuralArgumentPlan;

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
    let (checked_path, root) = walk_exact_place(checked, machine, authored, borrow.target, true)?;
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
    // A shared borrow moves nothing, so every structural place root that
    // `validate_place_root` establishes — a parameter or a uniquely-established
    // prior local, whatever it carries — is admissible to observe through.
    // The selection's custody stays with the root's existing retainer: an
    // owned home, the root's own borrow loan, or an element view's loan.
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
    let (checked_path, root) = walk_exact_place(checked, machine, authored, expression, false)?;
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
    // Copying does not consume the source loan. A slice's literal selector is
    // an exact coordinate too; emission must retain its live-length obligation
    // rather than treating the coordinate as a fixed-array bounds proof.
    let (checked_path, root) = walk_exact_place(checked, machine, authored, expression, true)?;
    // A bare whole-root name walks to the empty path — the copied leaf is
    // then the root's own declared `Unrestricted` type, which the leaf/type
    // agreement below already pins exactly.
    if checked_path != argument.path {
        return unsupported("copied place path does not match its projected path");
    }
    let leaf =
        validation::expression_result_type_reference(&checked.typed, machine, authored, expression)
            .ok_or(LoweringError::Unsupported(
                "copied place lost its leaf type",
            ))?;
    let Some(unwrapped) = validation::unwrapped_type_reference(&checked.typed, reference) else {
        return unsupported("copied place lost its leaf type");
    };
    // Storage identities may name the unwrapped carrier: a `&[T]` view uses
    // its referee and an arithmetic-policy-qualified value uses its layout.
    // Independently require the source and destination declared carriers to
    // match modulo `Constrained` shells; equal storage alone cannot erase
    // policy or confer ownership, and which qualifications a declaration
    // asserts was decided by the checker's weakening law upstream.
    if checked.normalized_type_identity(leaf).as_str() != argument.type_identity
        && checked.normalized_type_identity(unwrapped).as_str() != argument.type_identity
        || !validation::declared_carriers_match(&checked.typed, leaf, reference)
    {
        return unsupported("copied place does not project the result type");
    }
    // A `&[T]` view leaf carries its `Unrestricted` multiplicity on the
    // reference shell — the borrowed view copies whole while the bare
    // `Slice` rung it unwraps to stays affine. Every other leaf's
    // multiplicity already lives on its unwrapped rung.
    if checked.typed.type_multiplicity(unwrapped) != language_semantics::Multiplicity::Unrestricted
        && checked.typed.type_multiplicity(reference)
            != language_semantics::Multiplicity::Unrestricted
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
    // An authored `self` name retains the durable machine symbol while the
    // retained source plan names the `is_self` parameter's own symbol.
    // Resolve through the parameter row so the source match sees the same
    // identity both sides describe.
    let root_symbol = if name.symbol == machine.symbol {
        let mut self_parameters = checked
            .state_parameters(authored)
            .iter()
            .filter(|parameter| parameter.is_self);
        match self_parameters.next() {
            Some(parameter) if self_parameters.next().is_none() => parameter.symbol,
            _ => name.symbol,
        }
    } else {
        name.symbol
    };
    if !super::owned_selection::source_plan_matches(checked, authored.symbol, root_symbol, source) {
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
        // A declaration whose complete type admits zero establishes its own
        // storage without an initializer expression. The emitted producer and
        // its dominance are still required when this exact local is read.
        let established = local.initial_value.is_valid()
            || validation::zeroed_structural_parts(&checked.typed, local.type_reference);
        if locals.next().is_some() || index >= statement as usize || !established {
            return unsupported("borrowed selection root is not uniquely established");
        }
    }
    Ok(())
}
