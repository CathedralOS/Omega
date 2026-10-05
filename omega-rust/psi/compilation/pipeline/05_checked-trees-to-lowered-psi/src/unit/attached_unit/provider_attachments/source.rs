//! Rejoin provider requirements to the original receiver field at each call.

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::CheckedProviderAttachmentRequirementPlan;
use checked_trees::CheckedTrees;
use checked_trees::CheckedUnitEffectOperationPlan;
use checked_trees::data::DataMember;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;
use symbols::SymbolHandle;

/// Replay a boundary occurrence and report whether it consumes attachment
/// coverage. A Fused binding through an ordinary context still names its exact
/// attachment field: these roots witness specialization, not runtime self
/// storage. Source/shape/erasure replay precedes boundary deduplication.
pub(in crate::unit::attached_unit) fn validate_call_source(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    requirements: &[CheckedProviderAttachmentRequirementPlan],
) -> Result<bool, LoweringError> {
    let Some(planned) = operation.call().filter(|planned| planned.is_boundary()) else {
        return Ok(false);
    };
    let (coordinate, boundary) = (planned.coordinate, planned.target.machine);
    // Operators and external leaves own their source replay elsewhere; only
    // boundary-trait signatures can consume an attached provider field.
    if !checked.traits().iter().any(|definition| {
        definition.is_boundary
            && checked
                .trait_machine_signatures(definition)
                .iter()
                .any(|signature| signature.symbol == boundary)
    }) {
        return Ok(false);
    }
    let authored = crate::call_sites::locate(checked, state, coordinate)?;
    let parameter_receiver =
        crate::emission::call_source_custody::validate_parameter_binding_receiver(
            checked, machine, state, parameters, coordinate, &authored,
        )?;
    let Some(requirement) = requirements
        .iter()
        .find(|requirement| requirement.boundary == boundary)
    else {
        // Complete-set validation separately rejects missing required roots.
        return Ok(true);
    };
    let (owner, source) = crate::declarations::state_declaration(checked, state)?;
    if owner.symbol != machine {
        return unsupported("provider call lost its authored machine");
    }
    let attachment = checked
        .data_definitions()
        .iter()
        .find(|data| data.symbol == owner.attached_data_symbol)
        .ok_or(LoweringError::Unsupported(
            "provider field call lost its attachment owner",
        ))?;
    if !checked.data_members(attachment).iter().any(|member| {
        matches!(member, DataMember::Field(field) if field.name.as_str() == requirement.field_identity)
    }) {
        return unsupported("provider field call names no authored attachment field");
    }
    if parameter_receiver {
        if !owner.attached_data_application.is_valid()
            && attachment.generic_instance.is_none()
            && checked
                .normalized_nominal_type_identity(attachment.symbol)
                .is_none()
        {
            return unsupported("binding receiver has no normalized attachment owner");
        }
        return Ok(true);
    }
    if !checked
        .state_parameters(source)
        .iter()
        .any(|parameter| parameter.is_self)
    {
        return unsupported("provider field call has no authored receiver");
    }
    match authored.source_site {
        Some(checked_trees::NominalMachineUseSite::Statement(_)) => {
            let Some(StatementNode::Call(_)) = checked
                .statement_table
                .statements(source.statement_nodes)
                .get(coordinate.statement_index as usize)
            else {
                return unsupported("provider field call lost its authored statement");
            };
        }
        Some(checked_trees::NominalMachineUseSite::Expression(expression)) => {
            let ExpressionNode::Call(call) = checked.expression_table.expression(expression) else {
                return unsupported("provider field call lost its authored expression");
            };
            crate::emission::call_source_custody::projected_receivers::source(
                checked,
                machine,
                state,
                coordinate.statement_index as usize,
                call.receiver,
            )?;
        }
        None => return unsupported("provider field call has no authored occurrence"),
    }
    Ok(true)
}
