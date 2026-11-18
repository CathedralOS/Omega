// Separately emitted checked component entries and their ordinary body closure.
// Every export uses its producing component's own proposal and package
// acceptance, verifies its formal parameter lanes against the boundary calling
// application, and emits a shared component image keyed by the component
// version and calling application commitment (`component_export_slot`).

use calling_conventions::{BoundaryCallingApplication, BoundaryNativeParameterOrigin};
use diagnostics::Diagnostic;
use native_program_planning::ComponentNativeInputs;
use native_program_planning::admit_prepared_native_realization;
use provider_admission::provider_admission::{
    AdmittedNativeCallableAuthority, SourceEvaluatedImportSettlement,
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy, realization_error,
};

pub struct ComponentExportRequest<'input> {
    pub retain_native_stage_inputs: bool,
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
    runtime: native_artifact::TerminalCallableRuntimeProvisions<
        super::auxiliary_native::EncodedForeignAdapter,
    >,
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
        call: &native_program_planning::AdmittedNativeComponentCall<'_>,
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
            runtime: runtime.map_adapters(super::auxiliary_native::emit),
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
    executions: Vec<native_program_planning::PreparedNativeExecution>,
    scalars: Option<native_program_planning::NativeScalarApplications<'_>>,
    request: ComponentExportRequest<'_>,
) -> Result<Vec<ComponentNativeExport<'input>>, Vec<Diagnostic>> {
    let artifact = inputs.component().artifact();
    let proposal = inputs.proposal();
    let mut planned = Vec::with_capacity(inputs.exports().len());
    if executions.len() != inputs.exports().len() {
        return Err(vec![Diagnostic::error(
            "component prepared execution and export rosters differ",
        )]);
    }
    for (export, execution) in inputs.exports().iter().zip(executions) {
        let entry = terminal_psi::TargetMachineCallable::Machine(export.candidate.candidate);
        let (input, execution) = native_program_planning::lower_native_execution(
            proposal,
            execution,
            &[],
            scalars.as_ref().map(
                |scalars| native_program_planning::NativeScalarApplications {
                    completed: scalars.completed,
                },
            ),
        )?;
        let selections = proposal.post_terminal_optimizations().selections();
        let prepared = native_program_planning::prepare_native_providers(
            proposal,
            artifact,
            native_program_planning::RetainedNativeProviderInputs {
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
        let target = native_program_planning::callable_planning::plan_callable(
            admitted,
            admission,
            selections,
            crate::request::native_optimization_budget(),
            artifact,
            request.retain_native_stage_inputs,
        )?;
        let physical =
            super::callable_objects::prepare_callable_physical(target, None, Vec::new())?;
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
        if !physical.body.private_references.is_empty() {
            return Err(realization_error(
                "component export closure",
                "component export retains private callback bodies requiring their own publication custody",
            ));
        }
        let entry_plan = physical_planning::callable_entry::plan_callable_entry_transport(
            &physical.body.frame,
            &physical.body.installations,
            application.plan.plan(),
            &ordinals,
        )
        .map_err(|error| realization_error("component entry ABI", error))?;
        planned.push((export, physical, entry_plan));
    }
    // Close every export, entry transport and foreign helper before encoding
    // the component. No export can discover more source or bodies afterward.
    let handlers = installation_binding::RetainedHandlerMeasurements::default();
    let mut bound = Vec::with_capacity(planned.len());
    for (export, physical, entry_plan) in planned {
        bound.push((export, physical.bind(&handlers)?, entry_plan));
    }
    let mut encoded = Vec::with_capacity(bound.len());
    for (export, physical, entry_plan) in bound {
        let body = physical.encode()?;
        let entry = native_encoding::callable_entry::encode_callable_entry_transport(entry_plan)
            .map_err(|error| {
                realization_error(
                    "component entry ABI",
                    format!("native entry transport: {error}"),
                )
            })?;
        encoded.push((export, body, entry));
    }
    let mut layouts = Vec::with_capacity(encoded.len());
    for (export, body, entry) in encoded {
        layouts.push((export, body.layout()?, entry));
    }
    let mut exports = Vec::with_capacity(layouts.len());
    for (export, layout, entry_bridge) in layouts {
        let application = export.calling_application;
        let emitted = layout.construct_object()?;
        let body = emitted.projection.into_object();
        let runtime = emitted.runtime;
        let authority = emitted.authority;
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
