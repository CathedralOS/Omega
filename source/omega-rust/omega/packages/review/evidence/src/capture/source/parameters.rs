use super::{
    contracts::project_contract_source_locations,
    invocations::project_signature_invocation_source_locations,
    service_reach::project_signature_service_reach_source_locations,
    suspension::project_signature_operational_source_locations,
};
use crate::capture::source::ProjectedNestedSourceLocation;
use crate::capture::source::locations::project_nested_declaration_source_location;
use crate::record::PackageReviewSourceLocationRole;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(crate) fn collect_type_parameter_source_locations(
    compilation: &CheckedCompilation,
    parameters: &[psi_typed_trees::data::TypeParameter],
    locations: &mut Vec<ProjectedNestedSourceLocation>,
) -> Result<(), Vec<Diagnostic>> {
    for parameter in parameters {
        let psi_typed_trees::data::TypeParameterKind::Machine {
            contract: psi_typed_trees::data::MachineParameterContract::Structural(signature),
        } = &parameter.kind
        else {
            continue;
        };
        collect_callable_parameter_source_locations(
            compilation,
            compilation.state_signature_parameters(signature),
            "structural machine parameter contract value parameter",
            locations,
        )?;
        locations.extend(project_contract_source_locations(
            compilation,
            compilation.state_signature_contracts(signature),
        )?);
        locations.extend(project_signature_invocation_source_locations(
            compilation,
            signature,
        )?);
        locations.extend(project_signature_service_reach_source_locations(
            compilation,
            parameter.symbol,
            signature,
        )?);
        locations.extend(project_signature_operational_source_locations(
            compilation,
            parameter.symbol,
            signature,
        )?);
        collect_type_parameter_source_locations(
            compilation,
            compilation.state_signature_type_parameters(signature),
            locations,
        )?;
    }
    Ok(())
}

pub(crate) fn collect_callable_parameter_source_locations(
    compilation: &CheckedCompilation,
    parameters: &[psi_typed_trees::signature::StateParameter],
    subject: &str,
    locations: &mut Vec<ProjectedNestedSourceLocation>,
) -> Result<(), Vec<Diagnostic>> {
    for parameter in parameters {
        locations.push(project_nested_declaration_source_location(
            compilation,
            parameter.symbol,
            PackageReviewSourceLocationRole::CallableParameter,
            subject,
        )?);
    }
    Ok(())
}
