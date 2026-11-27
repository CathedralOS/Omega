//! Compile each selected supplier root under its package's providers, then publish its callable image.
mod settlement;
pub(crate) use settlement::ComponentCall;

use crate::driver::inputs::components::VerifiedSuppliers;
use diagnostics::Diagnostic;
use provider_admission::realization_error;
use provider_realization::published_application::{
    NativeApplicationSettings, bind_component_export,
};
use std::sync::Arc;
use verified_optimization_input::{
    ComponentImport, RetainedTerminalArtifact, TerminalNativeRealizationProposal,
};

pub(crate) struct Export<'a> {
    import: &'a ComponentImport,
    index: usize,
    plan: &'a build_providers::plans::ProviderPlan,
    retained: RetainedTerminalArtifact,
    permissions: provider_admission::TerminalAuthorityPermissionPolicy,
}

pub(crate) fn bind<'a>(
    receiver: &'a TerminalNativeRealizationProposal,
    execution: &provider_realization::native_execution::PreparedNativeExecution,
    suppliers: &VerifiedSuppliers,
) -> Result<Vec<Export<'a>>, Vec<Diagnostic>> {
    let mut exports = Vec::new();
    for import in receiver.component_imports() {
        let plan = receiver
            .providers()
            .selection
            .plans()
            .iter()
            .find(|plan| plan.identity_digest() == import.plan())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "component import lost its selected provider plan",
                )]
            })?;
        let supplier = plan
            .provider_type_package_identity
            .and_then(|package| suppliers.get(&package))
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "component import lost its supplier Product",
                )]
            })?;
        if supplier.component != *import.component() {
            return Err(vec![Diagnostic::error(
                "component import differs from its verified supplier",
            )]);
        }
        for (index, selected) in import.calls().iter().enumerate() {
            if !execution
                .scope()
                .reach()
                .contains_boundary(selected.receiving_boundary())
            {
                continue;
            }
            let (retained, permissions) = bind_component_export(
                &supplier.selection,
                selected,
                NativeApplicationSettings {
                    application: Default::default(),
                    optimizations: &supplier.optimizations,
                    x86_scalar_fma_provider: supplier.x86_scalar_fma_provider,
                },
                &supplier.exclusions,
            )?;
            exports.push(Export {
                import,
                index,
                plan,
                retained,
                permissions,
            });
        }
    }
    Ok(exports)
}

/// Physical images and optional replay inputs from all reached exports in this construction.
pub(crate) struct Emission<'request> {
    pub suppliers: &'request VerifiedSuppliers,
    pub isolated_execution_grant: Option<&'request provider_admission::IsolatedExecutionGrant>,
    pub authority: &'request provider_admission::TerminalAuthorityPolicy,
    pub permissions: &'request Option<provider_admission::TerminalAuthorityPermissionPolicy>,
    pub scalar_receipts: &'request mut Vec<native_scalar_inputs::EvaluationReceipt>,
    pub retain_stage_inputs: bool,
    pub products: Vec<Arc<native_artifact::NativeComponentProduct>>,
    pub originals: Vec<(
        [u8; 32],
        Box<published_object_verification::RetainedPhysicalInputs>,
    )>,
}

impl Emission<'_> {
    fn record(
        &mut self,
        product: &Arc<native_artifact::NativeComponentProduct>,
        originals: Option<Box<published_object_verification::RetainedPhysicalInputs>>,
    ) -> Result<(), Vec<Diagnostic>> {
        let slot = *product.locator().slot_commitment();
        let previous = self
            .products
            .iter()
            .find(|other| *other.locator().slot_commitment() == slot);
        if previous.is_some_and(|other| {
            other.locator() != product.locator() || other.description() != product.description()
        }) {
            return Err(realization_error(
                "component emission",
                "a shared export has inconsistent physical products",
            ));
        }
        if let Some(originals) = originals {
            self.originals.push((slot, originals));
        }
        self.products.push(Arc::clone(product));
        Ok(())
    }
}

pub(crate) fn emit<'a>(
    emission: &mut Emission<'_>,
    receiver: &TerminalNativeRealizationProposal,
    exports: &'a [Export<'a>],
) -> Result<Vec<ComponentCall<'a>>, Vec<Diagnostic>> {
    let mut calls = Vec::new();
    for export in exports {
        let retained = &export.retained;
        let artifact = retained.shared_artifact();
        let proposal = retained.native_realization_proposal();
        let call = export
            .import
            .native_call(
                export.index,
                retained,
                export.plan,
                &receiver.boundary_applications().calling,
            )
            .map_err(|why| vec![Diagnostic::error(why)])?;
        // A diamond can reach the same receiving slot and provider export more than once.
        // Rejoin its checked occurrence before compiling any of its children again.
        if let Some(product) = emission.products.iter().find(|product| {
            product.selection() == call.selection()
                && product.application() == call.application()
                && product.artifact().manifest().identity() == artifact.manifest().identity()
        }) {
            calls.push(ComponentCall::join(
                Arc::clone(product),
                call,
                export.plan,
                receiver.providers().selection.identities(),
            )?);
            continue;
        }
        let execution = provider_realization::native_execution::prepare_program(
            proposal,
            retained.authorization(),
        )?;
        let scalars = crate::driver::inputs::scalars::complete(
            artifact,
            proposal,
            execution.scope().reach().scalar_application_demand(),
            emission.scalar_receipts,
        )?;
        let selected = proposal.post_terminal_optimizations().selections();
        let (abstract_input, prepared) = artifact_lowering::lower_native_root(
            proposal,
            artifact_lowering::RetainedNativeProviderInputs {
                execution: &execution,
                optimization_selections: selected,
                accepted_package_permissions: &export.permissions,
            },
            &[],
            scalars
                .as_ref()
                .map(|completed| artifact_lowering::NativeScalarApplications { completed }),
        )?;
        let dependencies = bind(proposal, &execution, emission.suppliers)?;
        let children = emit(emission, proposal, &dependencies)?;
        let mut settlements = prepared.program_settlements();
        settlements.extend(children.iter().map(ComponentCall::settlement));
        let admission = prepared.admission_request(
            &settlements,
            emission.authority,
            emission.permissions,
            emission.isolated_execution_grant,
            &[],
        );
        let admitted = artifact_lowering::admit_prepared_native_realization(
            abstract_input,
            artifact,
            &admission,
        )?;
        let (target, evidence) = super::plan_callables::plan_callable(
            admitted,
            admission,
            selected,
            crate::driver::request::native_optimization_budget(),
            artifact,
            emission.retain_stage_inputs,
        )?;
        let physical =
            super::callable_objects::prepare_callable_physical(target, None, Vec::new())?;
        if !physical.body.stage.program().private_references.is_empty() {
            return Err(realization_error(
                "component export",
                "private callback references need their separately compiled bodies",
            ));
        }
        let inbound = export_ordinals(&call)?;
        let entry = entry_planning::callable_entry::plan_callable_entry_transport(
            &physical.body.stage.program().frame,
            physical.body.stage.installations().for_entry_planning(),
            call.application().plan.plan(),
            &inbound,
        )
        .map_err(|reason| realization_error("component entry ABI", reason))?;
        let entry = installation_binding::entries::bind_callable_entry(entry);
        let entry = native_encoding::callable_entry::encode_callable_entry_transport(entry)
            .map_err(|reason| realization_error("component entry encoding", reason))?;
        let entry = placement::entry_layout::resolve_callable_entry(entry)
            .map_err(|reason| realization_error("component entry layout", reason))?;
        let object = physical
            .bind(&Default::default())?
            .encode()?
            .layout()?
            .construct_object()?;
        let slot = target::component_export_slot(
            artifact.view().identity().vocabulary_marker.get(),
            artifact.view().identity().program_fingerprint.as_bytes(),
            &call.application().commitment,
        );
        let function = image_emission::prepare_component_function_image(
            object.projection.into_object(),
            object_file::CallableEntryTransport::from_layout(entry),
            slot,
        )
        .map_err(|why| vec![why])?;
        let image = image_emission::emit_component_shared_image(function)
            .map_err(|why| vec![why.diagnostic.clone()])?;
        let runtime = evidence
            .runtime
            .map_adapters(crate::driver::runtime::foreign_adapters::compile)
            .map_adapters(native_artifact::emit_runtime_adapter);
        let product = native_artifact::NativeComponentProduct::from_emitted(
            &call,
            image,
            runtime,
            evidence.authority,
            execution.selection(),
        )
        .map_err(|why| vec![Diagnostic::error(why)])?;
        let joined = ComponentCall::join(
            Arc::new(product),
            call,
            export.plan,
            receiver.providers().selection.identities(),
        )?;
        emission.record(&joined.product, object.stage_originals)?;
        calls.push(joined);
    }
    Ok(calls)
}

fn export_ordinals(
    call: &verified_optimization_input::AdmittedNativeComponentCall<'_>,
) -> Result<Vec<u32>, Vec<Diagnostic>> {
    let selected = call.selection();
    let module = call.component().module();
    let boundary = module
        .boundary_machine(selected.provider_boundary())
        .ok_or_else(|| {
            realization_error("component entry ABI", "missing requirement declaration")
        })?;
    let body = module.machine(selected.candidate()).ok_or_else(|| {
        realization_error("component entry ABI", "missing selected supplier body")
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
    let projection = call
        .application()
        .call_projection()
        .map_err(|why| realization_error("component entry ABI", why))?;
    let mut ordinals = std::collections::BTreeMap::new();
    for parameter in projection.parameters() {
        let calling_conventions::BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } =
            parameter.origin
        else {
            return Err(realization_error(
                "component entry ABI",
                "private callback formals require component-owned thunks",
            ));
        };
        ordinals.insert(formal_ordinal as usize, parameter.native_ordinal);
    }
    if ordinals.len() != boundary.parameter_order.len() {
        return Err(realization_error(
            "component entry ABI",
            "export formal roster differs from its calling application",
        ));
    }
    boundary
        .parameter_positions(terminal_psi::BoundaryParameterKind::Scalar)
        .chain(boundary.parameter_positions(terminal_psi::BoundaryParameterKind::Structural))
        .map(|formal| {
            ordinals.get(&formal).copied().ok_or_else(|| {
                realization_error(
                    "component entry ABI",
                    "export formal has no native parameter",
                )
            })
        })
        .collect()
}
