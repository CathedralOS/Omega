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

use terminal_psi_to_abstract_operations::ComponentNativeInputs;

pub struct ComponentExportRequest<'input> {
    pub accepted_package_permissions: &'input TerminalAuthorityPermissionPolicy,
    pub terminal_authority_policy: &'input TerminalAuthorityPolicy,
    pub receiving_permissions: &'input Option<TerminalAuthorityPermissionPolicy>,
    pub imports: &'input [SourceEvaluatedImportSettlement<'input>],
}

/// A symbolic export and its exact separately emitted closure. This is not a
/// callback identity, loader mapping, installed address or runtime era.
pub struct ComponentNativeExport<'input> {
    component: &'input component_description::VerifiedComponent,
    selected_provider_plans: &'input effects::SelectedProviderPlanFacts,
    candidate: &'input terminal_psi::ProviderCandidateConformance,
    calling_application: &'input BoundaryCallingApplication,
    body: image_emission::ObjectArtifact,
    entry_bridge: Option<machine_emission::native_entry_bridge::NativeEntryBridge>,
    runtime: native_artifact::TerminalCallableRuntimeProvisions,
    authority:
        terminal_psi_to_abstract_operations::provider_admission::AdmittedNativeCallableAuthority,
}

/// Complete semantic/native export relationship after shared-container emission.
/// Still not a loaded occurrence or installed entry capability.
pub struct ComponentExportImage<'input> {
    component: &'input component_description::VerifiedComponent,
    selected_provider_plans: &'input effects::SelectedProviderPlanFacts,
    candidate: &'input terminal_psi::ProviderCandidateConformance,
    calling_application: &'input BoundaryCallingApplication,
    image: image_emission::ComponentSharedImage,
    runtime: native_artifact::TerminalCallableRuntimeProvisions,
    authority:
        terminal_psi_to_abstract_operations::provider_admission::AdmittedNativeCallableAuthority,
}

impl<'input> ComponentExportImage<'input> {
    pub fn into_product(
        self,
        call: &terminal_psi_to_abstract_operations::AdmittedNativeComponentCall<'_>,
    ) -> Result<native_artifact::NativeComponentProduct, String> {
        if self.component.artifact() != call.component().artifact()
            || self.candidate.candidate != call.selection().candidate()
            || self.calling_application != call.application()
        {
            return Err("component product conversion changed its checked export owner".into());
        }
        native_artifact::NativeComponentProduct::from_emitted(
            call,
            self.image,
            self.runtime,
            self.authority,
            self.selected_provider_plans,
        )
    }

    pub const fn component(&self) -> &'input component_description::VerifiedComponent {
        self.component
    }
    pub const fn candidate(&self) -> &'input terminal_psi::ProviderCandidateConformance {
        self.candidate
    }
    pub const fn calling_application(&self) -> &'input BoundaryCallingApplication {
        self.calling_application
    }
    pub const fn image(&self) -> &image_emission::ComponentSharedImage {
        &self.image
    }
    pub const fn authority(
        &self,
    ) -> &terminal_psi_to_abstract_operations::provider_admission::AdmittedNativeCallableAuthority
    {
        &self.authority
    }
}

impl<'input> ComponentNativeExport<'input> {
    pub fn emit_shared_image(self) -> Result<ComponentExportImage<'input>, Vec<Diagnostic>> {
        let Self {
            component,
            selected_provider_plans,
            candidate,
            calling_application,
            body,
            entry_bridge,
            runtime,
            authority,
        } = self;
        let function = image_emission::prepare_component_function_image(
            body,
            entry_bridge,
            component_export_slot(component.subject(), &calling_application.commitment),
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let image = image_emission::emit_component_shared_image(function)
            .map_err(|error| vec![error.diagnostic.clone()])?;
        Ok(ComponentExportImage {
            component,
            selected_provider_plans,
            candidate,
            calling_application,
            image,
            runtime,
            authority,
        })
    }
}

/// Physical linkage identifies one component version as well as its complete
/// calling application. Equal ABI contracts from another historical component
/// must not become interchangeable ELF symbols under loader interposition.
/// This digest names linkage only; selection and installation remain separate.
pub fn component_export_slot(
    component: terminal_psi::TerminalPsiIdentity,
    application: &[u8; 32],
) -> [u8; 32] {
    target::component_export_slot(
        component.vocabulary_marker.get(),
        component.program_fingerprint.as_bytes(),
        application,
    )
}

pub fn emit_component_exports<'input>(
    inputs: &ComponentNativeInputs<'input>,
    request: ComponentExportRequest<'_>,
) -> Result<Vec<ComponentNativeExport<'input>>, Vec<Diagnostic>> {
    let artifact = inputs.component().artifact();
    let proposal = inputs.proposal();
    let mut exports = Vec::with_capacity(inputs.exports().len());
    for export in inputs.exports() {
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
        let runtime = projection.runtime;
        let authority = projection.authority;
        let projection = projection.projection;
        let boundary = inputs
            .component()
            .module()
            .boundary_machine(export.candidate.boundary)
            .ok_or_else(|| {
                realization_error(
                    "component entry ABI",
                    "export lost its requirement declaration",
                )
            })?;
        let body = inputs
            .component()
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
            component: inputs.component(),
            selected_provider_plans: &proposal.providers().selection,
            candidate: export.candidate,
            calling_application: application,
            body,
            entry_bridge,
            runtime,
            authority,
        });
    }
    Ok(exports)
}
