//! Reconstruct a borrowed operand's live storage root, declared endpoint and
//! reference crossings. Scalar and aggregate callers share this address/access
//! join; their formal carrier checks and call occurrence checks remain separate.
use crate::emission::call_source_custody::projected_receivers::ReceiverSource;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::state::State;
use checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{
    CheckedTrees, CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
};
use language_core::ReferenceAccess;

pub(super) struct Source {
    pub(super) place: ReceiverSource,
    pub(super) declared_endpoint: TypeReferenceHandle,
    pub(super) carrier: TypeReferenceHandle,
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
    let (caller, _) = super::authored_state(checked, state.symbol)?;
    let source = match argument.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
        | CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
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
    let root = source.root;
    let whole_source = source.path.is_empty();
    let parameters = checked.state_parameters(state);
    let reference = match argument.source {
        CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } => {
            if symbol != root
                || !source.path.is_empty()
                || parameters
                    .iter()
                    .any(|parameter| parameter.symbol == symbol)
            {
                return unsupported("primitive operand substituted its local storage");
            }
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

        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            if symbol != root
                || parameters
                    .iter()
                    .any(|parameter| parameter.symbol == symbol)
            {
                return unsupported("borrowed operand substituted its local owner");
            }
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
            if source.is_const
                || source.symbol != root
                || !super::parameter_binding_is_unique(checked, state, root)
            {
                return unsupported("borrowed operand substituted its source parameter");
            }
            let reference = match checked.type_reference_table.type_reference(
                validation::peel_declared_constraints(&checked.typed, source.type_reference),
            ) {
                TypeReferenceNode::Reference {
                    access, referee, ..
                } if compatible_access(*access, target_access) => *referee,
                TypeReferenceNode::Named { .. } | TypeReferenceNode::FixedArray { .. }
                    if target_access == ReferenceAccess::Shared
                        || source.is_mutable
                        || source.is_self =>
                {
                    source.type_reference
                }
                _ => return unsupported("borrowed operand widens source access"),
            };
            if source.is_self {
                let TypeReferenceNode::Named { symbol, .. } =
                    checked.type_reference_table.type_reference(reference)
                else {
                    return unsupported("borrowed source self lost its nominal attachment");
                };
                if *symbol != caller.symbol && *symbol != caller.attached_data_symbol {
                    return unsupported("borrowed source self changed its attachment");
                }
                // Projected loans reconstruct the endpoint from the authored
                // place below. Their enclosing attachment need not have an
                // incidentally interned named type; its identity was checked
                // above independently of the producer's argument plan.
                if whole_source {
                    checked
                        .type_reference_table
                        .find_named_type_reference(caller.attached_data_symbol)
                        .ok_or(LoweringError::Unsupported(
                            "borrowed source attachment has no declared type",
                        ))?
                } else {
                    reference
                }
            } else {
                reference
            }
        }
        _ => return unsupported("borrowed operand has no supported source owner"),
    };
    let reference = if whole_source {
        reference
    } else {
        validation::declared_place_type_raw(&checked.typed, caller, Some(state), named).ok_or(
            LoweringError::Unsupported("borrowed projection lost its declared type"),
        )?
    };
    let peeled = validation::peel_declared_constraints(&checked.typed, reference);
    if peeled != reference
        && !validation::has_plain_owned_contents_with_represented_constraints(
            &checked.typed,
            reference,
        )
    {
        return unsupported("borrowed operand has unrepresented source constraints");
    }
    // Reconstruct both the referent type and its address. Each stored shell
    // crossed here contributes a Referent step; the captured authored place
    // remains unchanged for borrow-fact replay. Checking type alone would
    // accept a descriptor address as though it named the backing value.
    let mut carrier = peeled;
    let mut projected_path = source.path.clone();
    while let TypeReferenceNode::Reference {
        access, referee, ..
    } = checked.type_reference_table.type_reference(carrier)
    {
        if !compatible_access(*access, target_access) {
            break;
        }
        projected_path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
        carrier = validation::peel_declared_constraints(&checked.typed, *referee);
    }
    // A primitive-reference local stores its carrier as an ordinary Reference
    // result. Source replay of the bare name has no field crossings, and its
    // declaration was peeled once above; this is that one retained root crossing.
    if let CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } = argument.source
        && let Some((ordinal, _)) = checked.typed.state_local_declaration(state, symbol)
        && validation::reference_result_custody::primitive_local_source(
            &checked.typed,
            state,
            ordinal,
        )
        .is_some()
    {
        projected_path.insert(0, checked_trees::CheckedUnitStructuralPathSegment::Referent);
    }
    if projected_path != argument.path {
        return unsupported("borrowed operand substituted its projected referent path");
    }
    Ok(Source {
        place: source,
        declared_endpoint: reference,
        carrier,
    })
}

fn compatible_access(source: ReferenceAccess, target: ReferenceAccess) -> bool {
    source == ReferenceAccess::Mutable || source == target
}
