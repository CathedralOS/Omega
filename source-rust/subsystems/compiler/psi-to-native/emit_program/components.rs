//! Emit the admitted component exports and join their receivers to the resulting images.

mod exports;
mod receiver_calls;

use crate::component_inputs::PreparedComponent;
use diagnostics::Diagnostic;
use exports::{ComponentExportImage, ComponentExportRequest, emit_component_exports};
use native_artifact::NativeComponentProduct;
use provider_admission::{TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy};
use provider_plans::ProviderPlan;
use receiver_calls::ComponentNativeCall;
use std::sync::Arc;
use terminal_psi::TerminalIdentityTable;
use verified_optimization_input::AdmittedNativeComponentCall;

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
        let exports = emit_component_exports(
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
