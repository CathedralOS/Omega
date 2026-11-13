// Separately emitted checked component entries and their ordinary body closure.
// Every export uses its producing component's own proposal and package
// acceptance, verifies its formal parameter lanes against the boundary calling
// application, and emits a shared component image keyed by the component
// version and calling application commitment (`component_export_slot`).

use calling_conventions::{BoundaryCallingApplication, BoundaryNativeParameterOrigin};
use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::ComponentNativeInputs;
use terminal_psi_to_abstract_operations::admit_prepared_native_realization;
use terminal_psi_to_abstract_operations::provider_admission::{
    AdmittedNativeCallableAuthority, SourceEvaluatedImportSettlement,
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy, realization_error,
};

pub struct ComponentExportRequest<'input> {
    pub accepted_package_permissions: &'input TerminalAuthorityPermissionPolicy,
    pub terminal_authority_policy: &'input TerminalAuthorityPolicy,
    pub receiving_permissions: &'input Option<TerminalAuthorityPermissionPolicy>,
    pub imports: &'input [SourceEvaluatedImportSettlement<'input>],
}

pub struct ComponentNativeExport<'input> {
    component: &'input component_description::VerifiedComponent,
    selected_provider_plans: &'input effects::SelectedProviderPlanFacts,
    candidate: &'input terminal_psi::ProviderCandidateConformance,
    calling_application: &'input BoundaryCallingApplication,
    body: image_emission::ObjectArtifact,
    entry_bridge: image_emission::CallableEntryTransport,
    runtime: native_artifact::TerminalCallableRuntimeProvisions,
    authority: AdmittedNativeCallableAuthority,
}

pub struct ComponentExportImage<'input> {
    component: &'input component_description::VerifiedComponent,
    selected_provider_plans: &'input effects::SelectedProviderPlanFacts,
    candidate: &'input terminal_psi::ProviderCandidateConformance,
    calling_application: &'input BoundaryCallingApplication,
    image: image_emission::ComponentSharedImage,
    runtime: native_artifact::TerminalCallableRuntimeProvisions,
    authority: AdmittedNativeCallableAuthority,
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

    pub const fn authority(&self) -> &AdmittedNativeCallableAuthority {
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
    works: Vec<terminal_psi_to_abstract_operations::PreparedNativeWork<'input>>,
    scalars: Option<terminal_psi_to_abstract_operations::NativeScalarApplications<'_>>,
    request: ComponentExportRequest<'_>,
) -> Result<Vec<ComponentNativeExport<'input>>, Vec<Diagnostic>> {
    let artifact = inputs.component().artifact();
    let proposal = inputs.proposal();
    let mut exports = Vec::with_capacity(inputs.exports().len());
    if works.len() != inputs.exports().len() {
        return Err(vec![Diagnostic::error(
            "component prepared work and export rosters differ",
        )]);
    }
    for (export, work) in inputs.exports().iter().zip(works) {
        let entry = terminal_psi::TargetMachineCallable::Machine(export.candidate.candidate);
        let (input, execution) = work.lower(scalars.as_ref().map(|scalars| {
            terminal_psi_to_abstract_operations::NativeScalarApplications {
                completed: scalars.completed,
                provider_declarations: scalars.provider_declarations,
            }
        }))?;
        let selections = proposal.post_terminal_optimizations().selections();
        let prepared = terminal_psi_to_abstract_operations::prepare_native_providers(
            proposal,
            artifact,
            terminal_psi_to_abstract_operations::RetainedNativeProviderInputs {
                execution: &execution,
                optimization_selections: selections,
                accepted_package_permissions: request.accepted_package_permissions,
                imports: request.imports,
            },
        )?;
        let settlements = prepared.program_settlements();
        let mut admission = prepared.admission_request(
            &settlements,
            request.terminal_authority_policy,
            request.receiving_permissions,
            &[],
        );
        admission.entry = entry;
        let admitted = admit_prepared_native_realization(input, artifact, &admission)?;
        let projection = super::callable_objects::emit_callable_object(
            admitted,
            admission,
            selections,
            crate::request::native_optimization_budget(),
            artifact,
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
            application.plan.plan(),
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
