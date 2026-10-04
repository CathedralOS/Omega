//! Rejoin provider requirements to the original receiver field at each call.

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::CheckedProviderAttachmentRequirementPlan;
use checked_trees::CheckedTrees;
use checked_trees::CheckedUnitEffectOperationPlan;
use checked_trees::CheckedUnitStructuralPathSegment;
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
    // The authored field spells the provider as a `&'a mut <boundary trait>`
    // borrow; the requirement names the provider itself, so the comparison
    // unwraps the carrier's mutable reference (and any qualifications) first.
    let mut carrier = field.type_reference;
    loop {
        match checked.type_reference_table.type_reference(carrier) {
            checked_trees::types::TypeReferenceNode::Constrained {
                base_type: inner, ..
            } => carrier = *inner,
            checked_trees::types::TypeReferenceNode::Reference {
                referee: inner,
                access: language_core::ReferenceAccess::Mutable,
                ..
            } => carrier = *inner,
            _ => break,
        }
    }
    if field.relevance.is_erased()
        || checked.normalized_type_identity(carrier).as_str() != requirement.provider_type_identity
    {
        return unsupported("provider field call changed its authored carrier");
    }
    if let Some(receiver) = parameter_receiver {
        let attachment_identity = if owner.attached_data_application.is_valid() {
            checked.normalized_type_identity(owner.attached_data_application)
        } else if let Some(application) = attachment.generic_instance {
            checked.normalized_type_identity(application)
        } else {
            checked
                .normalized_nominal_type_identity(attachment.symbol)
                .ok_or(LoweringError::Unsupported(
                    "binding receiver has no normalized attachment owner",
                ))?
        };
        if receiver.parameter.type_identity != attachment_identity.as_str()
            || !matches!(receiver.path.as_slice(), [CheckedUnitStructuralPathSegment::Field(name)]
                if name == &requirement.field_identity)
        {
            return unsupported("binding receiver changed its exact attachment field");
        }
        return Ok(true);
    }
    let receiver = checked
        .state_parameters(source)
        .iter()
        .find(|parameter| parameter.is_self)
        .ok_or(LoweringError::Unsupported(
            "provider field call has no authored receiver",
        ))?;
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
            let projected = crate::emission::call_source_custody::projected_receivers::source(
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
    Ok(true)
}
