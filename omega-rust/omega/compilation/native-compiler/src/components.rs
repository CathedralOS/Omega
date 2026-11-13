// Independent component image preparation, emission, and receiver-call custody.
// `prepare_components` admits each component's native calls against the
// receiver artifact and prepares callable execution + scalar completion demand;
// `emit_components` emits shared component images and verifies that the emitted
// export roster matches the admitted call roster 1-for-1.

use crate::realization::{ComponentExportImage, ComponentExportRequest, ComponentNativeCall};
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderPlan;
use native_artifact::NativeComponentProduct;
use std::sync::Arc;
use terminal_psi::TerminalIdentityTable;
use terminal_psi_to_abstract_operations::provider_admission::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
};
use terminal_psi_to_abstract_operations::{AdmittedNativeComponentCall, ComponentNativeInputs};

pub struct ComponentCompilation<'input> {
    pub input: ComponentNativeInputs<'input>,
    pub plan: &'input ProviderPlan,
    pub receiving_identities: &'input TerminalIdentityTable,
    pub accepted_package_permissions: &'input TerminalAuthorityPermissionPolicy,
}

pub(crate) struct PreparedComponent<'input> {
    compilation: &'input ComponentCompilation<'input>,
    pub(crate) calls: Vec<AdmittedNativeComponentCall<'input>>,
    pub(crate) scalars: crate::NativeScalarCompletion<'input>,
    exports: Vec<terminal_psi_to_abstract_operations::PreparedNativeWork<'input>>,
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
                terminal_psi_to_abstract_operations::prepare_native_callable_execution(
                    proposal,
                    artifact,
                    terminal_psi::TargetMachineCallable::Machine(export.candidate.candidate),
                )?
                .with_evidence(input.evidence().cloned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let scalars = crate::NativeScalarCompletion::new(
            artifact,
            proposal,
            exports
                .iter()
                .flat_map(|work| work.scalar_application_demand().iter().copied()),
        );
        prepared.push(PreparedComponent {
            compilation,
            calls,
            scalars,
            exports,
        });
    }
    Ok(prepared)
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

    pub(crate) fn into_product(self) -> Result<Arc<NativeComponentProduct>, Vec<Diagnostic>> {
        self.image
            .into_product(&self.call)
            .map(Arc::new)
            .map_err(|reason| vec![Diagnostic::error(reason)])
    }
}

pub(crate) fn emit_components<'input>(
    components: Vec<PreparedComponent<'input>>,
    terminal_authority_policy: &TerminalAuthorityPolicy,
    receiving_permissions: &Option<TerminalAuthorityPermissionPolicy>,
) -> Result<Vec<EmittedComponent<'input>>, Vec<Diagnostic>> {
    let mut emitted = Vec::new();
    for component in components {
        let PreparedComponent {
            compilation: component,
            calls,
            scalars,
            exports: work,
        } = component;
        let exports = crate::realization::emit_component_exports(
            &component.input,
            work,
            scalars.native_input()?,
            ComponentExportRequest {
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
