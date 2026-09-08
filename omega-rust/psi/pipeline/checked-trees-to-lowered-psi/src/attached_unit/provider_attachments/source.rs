//! Rejoin provider requirements to the original receiver field at each call.

use crate::{CheckedTrees, LoweringError, unsupported};
use checked_trees::CheckedProviderAttachmentRequirementPlan;
use checked_trees::CheckedUnitEffectOperationPlan;
use checked_trees::CheckedUnitStructuralPathSegment;
use checked_trees::data::DataMember;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;
use symbols::SymbolHandle;

pub(in crate::attached_unit) fn validate_call_source(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    requirements: &[CheckedProviderAttachmentRequirementPlan],
) -> Result<(), LoweringError> {
    let (coordinate, boundary) = match operation {
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            target_machine,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate,
            target_machine,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            target_machine,
            ..
        } => (coordinate, target_machine),
        _ => return Ok(()),
    };
    let Some(requirement) = requirements
        .iter()
        .find(|requirement| requirement.boundary == *boundary)
    else {
        // Complete-set validation separately rejects missing required roots.
        return Ok(());
    };
    let (owner, source) = crate::scalar_source_custody::authored_state(checked, state)?;
    if owner.symbol != machine {
        return unsupported("provider call lost its authored machine");
    }
    let receiver = checked
        .state_parameters(source)
        .iter()
        .find(|parameter| parameter.is_self)
        .ok_or(LoweringError::Unsupported(
            "provider field call has no authored receiver",
        ))?;
    let attachment = checked
        .data_definitions()
        .iter()
        .find(|data| data.symbol == owner.attached_data_symbol)
        .ok_or(LoweringError::Unsupported(
            "provider field call lost its attachment owner",
        ))?;
    let field = checked
        .data_members(attachment)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == requirement.field_identity => {
                Some(field)
            }
            _ => None,
        })
        .ok_or(LoweringError::Unsupported(
            "provider field call names no authored attachment field",
        ))?;
    if field.relevance.is_erased()
        || checked
            .normalized_type_identity(field.type_reference)
            .as_str()
            != requirement.provider_type_identity
    {
        return unsupported("provider field call changed its authored carrier");
    }
    let authored =
        crate::call_source_custody::authored::locate_source(checked, state, *coordinate)?;
    match authored.source_site {
        Some(checked_trees::NominalMachineUseSite::Statement(_)) => {
            let Some(StatementNode::Call(call)) = checked
                .statement_table
                .statements(source.statement_nodes)
                .get(coordinate.statement_index as usize)
            else {
                return unsupported("provider field call lost its authored statement");
            };
            let members = checked.statement_table.name_path_members(call.receiver);
            if members.len() != 2
                || members[0].as_str() != receiver.name.as_str()
                || members[1].as_str() != field.name.as_str()
                // Statement source resolution represents lexical self by its
                // machine; executable places use the state's receiver instead.
                || call.receiver_root_symbol != machine
                || call.receiver_symbol != field.symbol
            {
                return unsupported(
                    "provider field call changed its authored receiver root or field",
                );
            }
        }
        Some(checked_trees::NominalMachineUseSite::Expression(expression)) => {
            let ExpressionNode::Call(call) = checked.expression_table.expression(expression) else {
                return unsupported("provider field call lost its authored expression");
            };
            let projected = crate::call_source_custody::projected_receivers::source(
                checked,
                machine,
                state,
                coordinate.statement_index as usize,
                call.receiver,
            )?;
            if projected.root != receiver.symbol
                || projected.stamp != field.symbol
                || projected.path
                    != [CheckedUnitStructuralPathSegment::Field(
                        requirement.field_identity.clone(),
                    )]
            {
                return unsupported(
                    "provider expression changed its authored receiver root or field",
                );
            }
        }
        None => return unsupported("provider field call has no authored occurrence"),
    }
    Ok(())
}
