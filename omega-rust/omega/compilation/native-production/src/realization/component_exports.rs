//! Separately emitted checked component entries and their ordinary body closure.
//!
//! Every export uses its producing component's own proposal and package
//! acceptance. Its object never enters the consumer's private-body linker.
//! Container publication and installed entry/resource/era admission remain later
//! consuming operations; emitted bytes alone are not an installed component.

use calling_conventions::{BoundaryCallingApplication, BoundaryNativeParameterOrigin};
use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::provider_admission::{
    SourceEvaluatedImportSettlement, TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
    realization_error,
};

use crate::component_inputs::ComponentNativeInputs;

pub struct ComponentExportRequest<'input> {
    pub accepted_package_permissions: &'input TerminalAuthorityPermissionPolicy,
    pub terminal_authority_policy: &'input TerminalAuthorityPolicy,
    pub receiving_permissions: &'input Option<TerminalAuthorityPermissionPolicy>,
    pub imports: &'input [SourceEvaluatedImportSettlement<'input>],
}

/// A symbolic export and its exact separately emitted closure. This is not a
/// callback identity, loader mapping, installed address or runtime era.
pub struct ComponentNativeExport<'input> {
    pub component: &'input component_description::VerifiedComponent,
    pub candidate: &'input terminal_psi::ProviderCandidateConformance,
    pub calling_application: &'input BoundaryCallingApplication,
    pub body: image_emission::ObjectArtifact,
    pub entry_bridge: Option<machine_emission::native_entry_bridge::NativeEntryBridge>,
}

/// Complete semantic/native export relationship after shared-container emission.
/// Still not a loaded occurrence or installed entry capability.
pub struct ComponentExportImage<'input> {
    pub component: &'input component_description::VerifiedComponent,
    pub candidate: &'input terminal_psi::ProviderCandidateConformance,
    pub calling_application: &'input BoundaryCallingApplication,
    pub image: image_emission::ComponentSharedImage,
}

impl<'input> ComponentNativeExport<'input> {
    pub fn emit_shared_image(self) -> Result<ComponentExportImage<'input>, Vec<Diagnostic>> {
        let Self {
            component,
            candidate,
            calling_application,
            body,
            entry_bridge,
        } = self;
        let function = image_emission::prepare_component_function_image(
            body,
            entry_bridge,
            calling_application.commitment,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let image = image_emission::emit_component_shared_image(function)
            .map_err(|error| vec![error.diagnostic.clone()])?;
        Ok(ComponentExportImage {
            component,
            candidate,
            calling_application,
            image,
        })
    }

    /// Transfer the separately emitted body and its actual ABI bridge into
    /// container preparation. The complete application supplies the slot key;
    /// no caller-authored symbol or local machine ID can choose another slot.
    pub fn into_function_image(self) -> Result<image_emission::ComponentFunctionImage, Diagnostic> {
        image_emission::prepare_component_function_image(
            self.body,
            self.entry_bridge,
            self.calling_application.commitment,
        )
    }
}

pub fn emit_component_exports<'input>(
    inputs: &'input ComponentNativeInputs<'input>,
    request: ComponentExportRequest<'_>,
) -> Result<Vec<ComponentNativeExport<'input>>, Vec<Diagnostic>> {
    let artifact = inputs.component.artifact();
    let proposal = inputs.proposal;
    let mut exports = Vec::with_capacity(inputs.exports.len());
    for export in &inputs.exports {
        let entry = terminal_psi::TargetMachineCallable::Machine(export.candidate.candidate);
        let (_, execution) = proposal.prepare_native_callable_execution(artifact, entry)?;
        let selections = proposal.post_terminal_optimizations().selections();
        let prepared = proposal.prepare_native_providers(
            artifact,
            terminal_psi_to_abstract_operations::RetainedNativeProviderInputs {
                execution: &execution,
                optimization_selections: selections,
                accepted_package_permissions: request.accepted_package_permissions,
                imports: request.imports,
            },
        )?;
        let settlements = prepared.program_settlements();
        let admission = prepared.admission_request(
            &settlements,
            request.terminal_authority_policy,
            request.receiving_permissions,
        );
        let projection = super::callable_objects::emit_callable_object(
            admission,
            selections,
            crate::request::native_optimization_budget(),
            artifact,
            entry,
        )?;
        let boundary = inputs
            .component
            .module()
            .boundary_machine(export.candidate.boundary)
            .ok_or_else(|| {
                realization_error(
                    "component entry ABI",
                    "export lost its requirement declaration",
                )
            })?;
        let body = inputs
            .component
            .module()
            .machine(export.candidate.candidate)
            .ok_or_else(|| {
                realization_error("component entry ABI", "export lost its checked body")
            })?;
        if !boundary.has_valid_parameter_order()
            || boundary.scalar_parameters.len() != body.parameters.len()
            || boundary.structural_parameters.len() != body.structural_parameters.len()
        {
            return Err(realization_error(
                "component entry ABI",
                "export body and requirement formal lanes differ",
            ));
        }
        let application = export.calling_application;
        let native_parameters = application
            .signature
            .native_parameters
            .span(application.signature.native_parameter_rows)
            .ok_or_else(|| {
                realization_error(
                    "component entry ABI",
                    "native parameter roster has stale storage",
                )
            })?;
        let parameter_formals = boundary
            .parameter_positions(terminal_psi::BoundaryParameterKind::Scalar)
            .chain(boundary.parameter_positions(terminal_psi::BoundaryParameterKind::Structural));
        let mut ordinals = Vec::new();
        for formal in parameter_formals {
            let formal = u32::try_from(formal)
                .map_err(|_| realization_error("component entry ABI", "formal index overflows"))?;
            let mut matching = native_parameters.iter().filter(|row| {
                row.origin
                    == BoundaryNativeParameterOrigin::SemanticFormal {
                        formal_ordinal: formal,
                    }
            });
            let parameter = matching.next().ok_or_else(|| {
                realization_error(
                    "component entry ABI",
                    "export formal has no native parameter",
                )
            })?;
            if matching.next().is_some() {
                return Err(realization_error(
                    "component entry ABI",
                    "export formal has competing native parameters",
                ));
            }
            ordinals.push(parameter.native_ordinal);
        }
        if native_parameters.len() != ordinals.len() {
            return Err(realization_error(
                "component entry ABI",
                "component export requires an independently retained private callback companion",
            ));
        }
        let body = projection.into_object();
        if !body.object().layout.function_symbols.is_empty() {
            return Err(realization_error(
                "component export closure",
                "component export retains private callback bodies requiring their own publication custody",
            ));
        }
        let entry_bridge = image_emission::create_callable_entry_transport(
            &body,
            &application.plan.plan().call,
            &ordinals,
        )
        .map_err(|error| realization_error("component entry ABI", error))?;
        exports.push(ComponentNativeExport {
            component: inputs.component,
            candidate: export.candidate,
            calling_application: application,
            body,
            entry_bridge,
        });
    }
    Ok(exports)
}
