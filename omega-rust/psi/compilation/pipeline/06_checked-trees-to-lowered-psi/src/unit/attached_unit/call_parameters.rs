//! Retained call formals connect authored positions to dense runtime slots.
//! The selected checked body owns parameter retention and erasure. Consumers
//! borrow its roster instead of reconstructing either policy from source
//! argument counts; ordinary call admission still owns transfer authority.

use super::bodies::{UnitBody, UnitPlans};
use super::{CheckedTrees, LoweringError, unique_unit_boundary, unsupported};
use crate::checked_facts::flow_facts;
use crate::scalar_lowering::call_closure::callee::CheckedScalarCallee;
use checked_trees::{
    CheckedStructuralScalarParameterPlan, CheckedUnitCallPlan, CheckedUnitCallResult,
    CheckedUnitStructuralParameterPlan,
};

struct ParameterRoster<'checked> {
    scalar: &'checked [CheckedStructuralScalarParameterPlan],
    structural: &'checked [CheckedUnitStructuralParameterPlan],
}

fn parameter_roster<'checked>(
    checked: &'checked CheckedTrees,
    call: &CheckedUnitCallPlan,
) -> Result<ParameterRoster<'checked>, LoweringError> {
    let plans = UnitPlans::published(&checked.facts.flow);
    let (state, scalar, structural) = if call.dynamic_selector().is_some() {
        // A dispatch slot owns the shared formal roster; no single selectable
        // implementation is the call's target body. Its authored selector is
        // retained ahead of the data formals in this same position namespace.
        let target = flow_facts(checked)
            .terminal_unit_effects
            .dynamic_slots
            .iter()
            .find(|target| target.slot == call.target.machine)
            .ok_or(LoweringError::Unsupported(
                "call parameter roster names no checked dynamic slot",
            ))?;
        (
            target.slot,
            target.scalar_parameters.as_slice(),
            target.structural_parameters.as_slice(),
        )
    } else if call.is_boundary()
        || matches!(
            call.route(),
            checked_trees::CheckedUnitCallRoute::ActivationStorage { .. }
        )
    {
        // Compiler-owned activation primitives have checked declaration formals
        // but never enter an implementation frame. Reuse that retained roster;
        // executable provider settlement remains exclusive to Boundary calls.

        let target = unique_unit_boundary(plans, call.target.machine)?;
        (
            target.state,
            target.scalar_parameters.as_slice(),
            target.structural_parameters.as_slice(),
        )
    } else if matches!(call.result(), CheckedUnitCallResult::Scalar(_)) {
        let target = CheckedScalarCallee::find_for_unit_call(checked, call.target.machine)?
            .body()
            .entry()?;
        (
            target.state,
            target.scalar_parameters,
            target.structural_parameters,
        )
    } else {
        let target = UnitBody::find(plans, call.target.machine)?.entry()?;
        (
            target.state,
            target.scalar_parameters,
            target.structural_parameters,
        )
    };
    if state != call.target.state {
        return unsupported("call parameter roster belongs to another entry state");
    }
    Ok(ParameterRoster { scalar, structural })
}

pub(crate) fn structural_parameters<'checked>(
    checked: &'checked CheckedTrees,
    call: &CheckedUnitCallPlan,
) -> Result<&'checked [CheckedUnitStructuralParameterPlan], LoweringError> {
    Ok(parameter_roster(checked, call)?.structural)
}

/// Runtime scalars use the selected body's retained positions, just like the
/// structural lane. Erased primitive formals remain in the authored telescope
/// but never become executable operand slots or evaluation steps.
pub(crate) fn scalar_positions(
    checked: &CheckedTrees,
    call: &CheckedUnitCallPlan,
    signature: &crate::declarations::TargetSignature<'_>,
) -> Result<Vec<usize>, LoweringError> {
    let parameters = parameter_roster(checked, call)?.scalar;
    if parameters.len() != call.scalar_operands().len() {
        return unsupported("call scalar arguments disagree with the retained formals");
    }
    let mut positions = Vec::with_capacity(parameters.len());
    for retained in parameters {
        let position = signature
            .authored_parameter_position(retained.source_position)
            .ok_or(LoweringError::Unsupported(
                "retained scalar formal has no authored position",
            ))? as usize;
        let authored = &signature.parameters[position];
        if authored.relevance.is_erased()
            || checked.primitive_type_reference(authored.type_reference)
                != Some(retained.primitive_type)
            || positions
                .last()
                .is_some_and(|previous| *previous >= position)
        {
            return unsupported("retained scalar formal disagrees with its authored partition");
        }
        positions.push(position);
    }
    Ok(positions)
}

/// Rejoin retained slots to the declaration's parameter list. This only
/// establishes positional identity: callers still replay each operand's source,
/// access, type, and transferred evidence independently.
pub(crate) fn structural_positions(
    checked: &CheckedTrees,
    call: &CheckedUnitCallPlan,
    signature: &crate::declarations::TargetSignature<'_>,
) -> Result<Vec<u32>, LoweringError> {
    let parameters = structural_parameters(checked, call)?;
    if parameters.len() != call.structural_arguments.len() {
        return unsupported("call structural arguments disagree with the retained formals");
    }
    let mut positions = Vec::with_capacity(parameters.len());
    for retained in parameters {
        let position = signature
            .authored_parameter_position(retained.position)
            .ok_or(LoweringError::Unsupported(
                "retained call formal has no authored position",
            ))?;
        let authored = &signature.parameters[position as usize];
        if authored.relevance.is_erased()
            || checked
                .primitive_type_reference(authored.type_reference)
                .is_some()
            || positions
                .last()
                .is_some_and(|previous| *previous >= position)
        {
            return unsupported("retained call formal disagrees with its authored partition");
        }
        positions.push(position);
    }
    // Source/callee admission owns justified omissions (including semantic-only
    // placed loans); this join does not reclassify omitted formals as ABI values.
    Ok(positions)
}
