// Independent component image preparation, emission, and receiver-call custody.
// `prepare_components` admits each component's native calls against the
// receiver artifact, prepares callable execution and keeps the completed scalar
// values its exports read;
// `emit_components` emits shared component images and verifies that the emitted
// export roster matches the admitted call roster 1-for-1.

use verified_optimization_input::{AdmittedNativeComponentCall, ComponentNativeInputs, PreparedNativeExecution};
use crate::pipeline::realization::{
    ComponentExportImage, ComponentExportRequest, ComponentNativeCall,
};
use diagnostics::Diagnostic;
use provider_plans::ProviderPlan;
use native_artifact::NativeComponentProduct;
use provider_admission::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
};
use std::sync::Arc;
use terminal_psi::TerminalIdentityTable;

pub struct ComponentCompilation<'input> {
    pub input: ComponentNativeInputs<'input>,
    pub plan: &'input ProviderPlan,
    pub receiving_identities: &'input TerminalIdentityTable,
    pub accepted_package_permissions: &'input TerminalAuthorityPermissionPolicy,
    /// Completed scalar values read by the exports.
    pub scalars: Option<&'input psi_scalar_applications::AdmittedScalarApplications>,
}

pub(crate) struct PreparedComponent<'input> {
    compilation: &'input ComponentCompilation<'input>,
    pub(crate) calls: Vec<AdmittedNativeComponentCall<'input>>,
    pub(crate) scalars: Option<&'input psi_scalar_applications::AdmittedScalarApplications>,
    exports: Vec<PreparedNativeExecution>,
}

pub(crate) fn prepare_components<'input>(
    components: &'input [ComponentCompilation<'input>],
    receiver: terminal_codec::TerminalArtifactView<'_>,
) -> Result<Vec<PreparedComponent<'input>>, Vec<Diagnostic>> {
    let mut prepared = Vec::with_capacity(components.len());
    for compilation in components {
        let input = &compilation.input;
        let calls = input
            .admit_native_calls(receiver)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let artifact = input.component().shared_artifact();
        let proposal = input.proposal();
        let exports = input
            .exports()
            .iter()
            .map(|export| {
                proposal
                    .prepare_native_callable_execution(
                        input.authorization(),
                        terminal_psi::TargetMachineCallable::Machine(export.candidate.candidate),
                    )?
                    .with_operation_evidence(artifact, input.evidence().cloned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut demand = Vec::new();
        for execution in &exports {
            demand.extend(execution.scalar_application_demand(proposal.target_profile())?);
        }
        let scalars = crate::pipeline::prepare::required_scalars::required_scalars(
            artifact,
            &demand,
            compilation.scalars,
        )?;
        prepared.push(PreparedComponent {
            compilation,
            calls,
            scalars,
            exports,
        });
    }
    Ok(prepared)
}

pub(crate) struct CompiledComponent {
    pub product: Arc<NativeComponentProduct>,
    pub originals: Option<Box<published_object_verification::RetainedPhysicalInputs>>,
}

pub(crate) struct EmittedComponent<'input> {
    image: ComponentExportImage<'input>,
    call: AdmittedNativeComponentCall<'input>,
    plan: &'input ProviderPlan,
    receiving_identities: &'input TerminalIdentityTable,
}

impl EmittedComponent<'_> {
    pub(crate) fn admit_call(&self) -> Result<ComponentNativeCall<'_>, Vec<Diagnostic>> {
        ComponentNativeCall::join(
            &self.image,
            &self.call,
            self.plan,
            self.receiving_identities,
        )
    }

    pub(crate) fn into_product(mut self) -> Result<CompiledComponent, Vec<Diagnostic>> {
        let originals = self.image.stage_originals.take();
        self.image
            .into_product(&self.call)
            .map(|product| CompiledComponent {
                product: Arc::new(product),
                originals,
            })
            .map_err(|reason| vec![Diagnostic::error(reason)])
    }
}

pub(crate) fn emit_components<'input>(
    components: Vec<PreparedComponent<'input>>,
    terminal_authority_policy: &TerminalAuthorityPolicy,
    receiving_permissions: &Option<TerminalAuthorityPermissionPolicy>,
    retain_native_stage_inputs: bool,
) -> Result<Vec<EmittedComponent<'input>>, Vec<Diagnostic>> {
    let mut emitted = Vec::new();
    for component in components {
        let PreparedComponent {
            compilation: component,
            calls,
            scalars,
            exports: work,
        } = component;
        let exports = crate::pipeline::realization::emit_component_exports(
            &component.input,
            work,
            scalars.map(|completed| artifact_lowering::NativeScalarApplications { completed }),
            ComponentExportRequest {
                retain_native_stage_inputs,
                accepted_package_permissions: component.accepted_package_permissions,
                terminal_authority_policy,
                receiving_permissions,
                imports: &[],
            },
        )?;
        if exports.len() != calls.len() {
            return Err(vec![Diagnostic::error(
                "component export image and import selection rosters differ",
            )]);
        }
        for (export, call) in exports.into_iter().zip(calls) {
            emitted.push(EmittedComponent {
                image: export.emit_shared_image()?,
                call,
                plan: component.plan,
                receiving_identities: component.receiving_identities,
            });
        }
    }
    Ok(emitted)
}
