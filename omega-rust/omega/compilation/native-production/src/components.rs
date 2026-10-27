//! Component image production and its exact receiver-call/runtime custody.

use crate::realization::{ComponentExportImage, ComponentExportRequest, ComponentNativeCall};
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderPlan;
use std::sync::Arc;
use terminal_psi::TerminalIdentityTable;
use terminal_psi_to_abstract_operations::provider_admission::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
};
use terminal_psi_to_abstract_operations::{AdmittedNativeComponentCall, ComponentNativeInputs};

/// One independently selected component and its complete receiving plan.
/// Package acceptance is separate from the receiver policy on the root request.
/// These inputs select work; the emitted image must still pass native admission.
pub struct ComponentCompilation<'input> {
    pub input: ComponentNativeInputs<'input>,
    pub plan: &'input ProviderPlan,
    pub receiving_identities: &'input TerminalIdentityTable,
    pub accepted_package_permissions: &'input TerminalAuthorityPermissionPolicy,
}

/// Keep each emitted export beside its exact call and selecting plan. Root
/// realization borrows these owners; successful publication consumes them.
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

    pub(crate) fn into_product(
        self,
    ) -> Result<Arc<native_artifact::NativeComponentProduct>, Vec<Diagnostic>> {
        self.image
            .into_product(&self.call)
            .map(Arc::new)
            .map_err(|reason| vec![Diagnostic::error(reason)])
    }
}

pub(crate) fn emit_components<'input>(
    components: &[ComponentCompilation<'input>],
    receiver: terminal_codec::TerminalArtifactView<'_>,
    terminal_authority_policy: &TerminalAuthorityPolicy,
    receiving_permissions: &Option<TerminalAuthorityPermissionPolicy>,
) -> Result<Vec<EmittedComponent<'input>>, Vec<Diagnostic>> {
    let mut emitted = Vec::new();
    for component in components {
        let calls = component
            .input
            .admit_native_calls(receiver)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let exports = crate::realization::emit_component_exports(
            &component.input,
            ComponentExportRequest {
                accepted_package_permissions: component.accepted_package_permissions,
                terminal_authority_policy,
                receiving_permissions,
                imports: &[],
            },
        )?;
        // Check before pairing: a shorter roster must not silently truncate
        // either side, nor borrow another component's export to fill the gap.
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
