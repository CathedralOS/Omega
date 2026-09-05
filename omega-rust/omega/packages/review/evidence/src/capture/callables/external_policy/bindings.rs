use super::super::external_supply::{
    project_evaluated_binding, project_external_binding, validate_external_binding_payload,
};
use super::rejected;
use crate::record::PackageReviewExternalBinding;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::MachineSupplyMode;
use psi_typed_trees::machine::{Machine, TraitConformance};

pub(super) fn project(
    compilation: &CheckedCompilation,
    machine: &Machine,
    conformance: &TraitConformance,
) -> Result<PackageReviewExternalBinding, Vec<Diagnostic>> {
    let MachineSupplyMode::ExternalRealization { binding, mechanism } = machine.supply_mode else {
        return Err(rejected(
            "requested machine is not external executable supply",
        ));
    };
    if machine.body_is_present || conformance.external_binding_source_span.is_none() {
        return Err(rejected(
            "external leaf has a body or lacks authored via custody",
        ));
    }
    match (binding, mechanism) {
        (Some(binding), Some(mechanism))
            if conformance.external_binding == Some(binding)
                && !conformance.via_expression.is_valid() =>
        {
            let identity = compilation
                .external_bindings
                .identity(binding)
                .ok_or_else(|| rejected("external leaf lost its exact binding-table identity"))?;
            if identity.mechanism() != mechanism {
                return Err(rejected(
                    "external mechanism differs from its exact binding",
                ));
            }
            validate_external_binding_payload(compilation, machine, identity)?;
            Ok(project_external_binding(identity))
        }
        (None, None)
            if conformance.external_binding.is_none() && conformance.via_expression.is_valid() =>
        {
            let row = compilation
                .evaluated_via_bindings()
                .exact(
                    machine.symbol,
                    conformance.symbol,
                    conformance.requirement_symbol,
                )
                .ok_or_else(|| rejected("external leaf lost its exact evaluated via row"))?;
            if row.via_expression() != conformance.via_expression {
                return Err(rejected(
                    "external leaf changed its evaluated via expression",
                ));
            }
            project_evaluated_binding(compilation, row)
        }
        _ => Err(rejected(
            "external leaf mixes legacy and evaluated supply carriers",
        )),
    }
}
