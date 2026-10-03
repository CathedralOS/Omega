//! Source correspondence for primitive reference locals. Storage and view types
//! remain distinct: a recast never substitutes the backing declaration.
use super::*;
use checked_trees::{CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan};

/// Reconstruct a shared primitive loan at its exact authored declaration.
/// Immutable scalar ingress has one value and one backing home. Mutable locals
/// already own physical storage; mutable SSA formals require storage coherence
/// across writes before they can use this addressable ingress contract.
pub fn primitive_local_source(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    statement: usize,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let StatementNode::LocalData(local) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement)?
    else {
        return None;
    };
    if !matches!(
        program
            .type_reference_table
            .type_reference(local.type_reference),
        TypeReferenceNode::Reference { .. }
    ) {
        return None;
    }
    let (referent, access) = shared_place_borrowed_parts(program, local.type_reference)?;
    if local.is_mutable
        || !local.symbol.is_valid()
        || !matches!(
            program.type_reference_table.type_reference(referent),
            TypeReferenceNode::Named { .. }
        )
        || program.primitive_type_reference(referent).is_none()
    {
        return None;
    }
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(local.initial_value)
    else {
        return None;
    };
    if borrow.access != language_semantics::ReferenceAccess::Shared {
        return None;
    }
    let (target, recast) = match program.expression_table.expression(borrow.target) {
        ExpressionNode::Cast(cast)
            if cast.form == language_core::cast_form::CastForm::RecastShared =>
        {
            if program.normalized_type_identity(cast.target_type)
                != program.normalized_type_identity(referent)
            {
                return None;
            }
            (cast.value, true)
        }
        _ => (borrow.target, false),
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(target) else {
        return None;
    };
    if !path.symbol.is_valid()
        || path.symbol != path.head_symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    let (source, storage) = if let Some((index, parameter)) = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            !parameter.is_const
                && !parameter.relevance.is_erased()
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
        })
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == path.symbol)
    {
        if parameter.is_mutable {
            return None;
        }
        (
            CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
                parameter_index: u32::try_from(index).ok()?,
            },
            parameter.type_reference,
        )
    } else {
        let (ordinal, backing) = program.state_local_declaration(state, path.symbol)?;
        if ordinal >= statement || !backing.is_mutable {
            return None;
        }
        (
            CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
                symbol: backing.symbol,
            },
            backing.type_reference,
        )
    };
    if !matches!(
        program.type_reference_table.type_reference(storage),
        TypeReferenceNode::Named { .. }
    ) || program.primitive_type_reference(storage).is_none()
        || (!recast
            && program.normalized_type_identity(storage)
                != program.normalized_type_identity(referent))
    {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source,
        path: Vec::new(),
        type_identity: program.normalized_type_identity(storage).into_string(),
        access,
    })
}
