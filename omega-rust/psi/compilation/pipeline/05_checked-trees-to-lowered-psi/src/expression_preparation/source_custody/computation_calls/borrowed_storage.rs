//! Reconstruct a borrowed operand's live storage root, declared endpoint and
//! reference crossings. Scalar and aggregate callers share this address/access
//! join; their formal carrier checks and call occurrence checks remain separate.
use crate::emission::call_source_custody::projected_receivers::ReceiverSource;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::state::State;
use checked_trees::types::TypeReferenceNode;
use checked_trees::{
    CheckedTrees, CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
};
use language_core::ReferenceAccess;

pub(super) struct Source {
    pub(super) place: ReceiverSource,
}

pub(super) fn source(
    checked: &CheckedTrees,
    state: &State,
    statement: u32,
    target_access: ReferenceAccess,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
) -> Result<Source, LoweringError> {
    let table = &checked.expression_table;
    if !table.expression_is_valid(expression) {
        return unsupported("borrowed operand has a stale authored expression");
    }
    let named = match table.expression(expression) {
        ExpressionNode::Borrow(borrow) if borrow.access == target_access => borrow.target,
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            expression
        }
        _ => return unsupported("borrowed operand has no named or projected source"),
    };
    if !table.expression_is_valid(named) {
        return unsupported("borrowed operand has a stale source expression");
    }
    let (caller, _) = crate::declarations::state_declaration(checked, state.symbol)?;
    let source = match argument.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
        | CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { .. }
        | CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
        | CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { .. }
        | CheckedUnitStructuralArgumentSourcePlan::Parameter { .. } => {
            crate::emission::call_source_custody::projected_receivers::source(
                checked,
                caller.symbol,
                state.symbol,
                statement as usize,
                named,
            )?
        }
        _ => return unsupported("borrowed operand has no supported source owner"),
    };
    let whole_source = source.path.is_empty();
    let parameters = checked.state_parameters(state);
    let reference = match argument.source {
        CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { parameter_index } => {
            let parameter = parameters
                .iter()
                .filter(|parameter| {
                    !parameter.is_const
                        && !parameter.relevance.is_erased()
                        && checked
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                })
                .nth(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "borrowed operand lost its scalar parameter",
                ))?;
            if !whole_source
                || !argument.path.is_empty()
                || source.captured_place().root_symbol != parameter.symbol
                || (target_access != ReferenceAccess::Shared && !parameter.is_mutable)
            {
                return unsupported("borrowed operand substitutes scalar ingress custody");
            }
            parameter.type_reference
        }
        CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } => {
            let local = super::preceding_local(
                checked,
                state,
                symbol,
                statement as usize,
                "primitive operand lost its local declaration",
                "primitive operand lost its initialized local storage",
            )?;
            if !local.is_mutable
                || checked
                    .primitive_type_reference(local.type_reference)
                    .is_none()
            {
                return unsupported("primitive operand has no mutable scalar storage");
            }
            local.type_reference
        }

        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
        | CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { symbol, .. } => {
            let local = super::preceding_local(
                checked,
                state,
                symbol,
                statement as usize,
                "borrowed operand lost its local declaration",
                "borrowed operand has no exact prior establishment",
            )?;
            // The loan rejoins the current original backing, not an immutable
            // snapshot of the initializer's scalar fields. An established
            // shared-borrow local carries that loan itself, so the carrier the
            // callee observes is the referent it dereferences to — the same
            // rule a shared-borrow parameter follows.
            match checked.type_reference_table.type_reference(
                validation::peel_declared_constraints(&checked.typed, local.type_reference),
            ) {
                TypeReferenceNode::Reference {
                    access, referee, ..
                } if compatible_access(*access, target_access)
                    && (target_access == ReferenceAccess::Shared || !local.is_mutable) =>
                {
                    *referee
                }
                // Lending exclusive receiver custody follows the ordinary
                // receiver lane's mutable-local requirement.
                TypeReferenceNode::Named { .. } | TypeReferenceNode::FixedArray { .. }
                    if target_access == ReferenceAccess::Shared || local.is_mutable =>
                {
                    local.type_reference
                }
                _ => return unsupported("borrowed operand widens source access"),
            }
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let source = super::structural_parameter_at(
                checked,
                parameters,
                parameter_index,
                "borrowed operand lost its structural parameter",
            )?;
            let reference = match checked.type_reference_table.type_reference(
                validation::peel_declared_constraints(&checked.typed, source.type_reference),
            ) {
                TypeReferenceNode::Reference {
                    access, referee, ..
                } if compatible_access(*access, target_access) => *referee,
                TypeReferenceNode::Named { .. } | TypeReferenceNode::FixedArray { .. }
                    if target_access == ReferenceAccess::Shared || source.is_mutable =>
                {
                    source.type_reference
                }
                _ => return unsupported("borrowed operand widens source access"),
            };
            reference
        }
        _ => return unsupported("borrowed operand has no supported source owner"),
    };
    if let Some(identity) = argument.borrowed_root_type_identity()
        && validation::structural_type_identity(&checked.typed, reference, &[], &[]).as_str()
            != identity
    {
        return unsupported("borrowed operand substitutes its checked logical root");
    }
    let reference = if whole_source {
        reference
    } else {
        let reference = checked.facts.resolved_types.place(named);
        if !reference.is_valid() {
            return unsupported("borrowed projection lost its declared type");
        }
        reference
    };
    let peeled = validation::peel_declared_constraints(&checked.typed, reference);
    if peeled != reference
        && !crate::content_classes::plain_owned_with_represented_constraints(checked, reference)
    {
        return unsupported("borrowed operand has unrepresented source constraints");
    }
    Ok(Source { place: source })
}

fn compatible_access(source: ReferenceAccess, target: ReferenceAccess) -> bool {
    source == ReferenceAccess::Mutable || source == target
}
