/*
We emit function-fragment objects (`emit_fragment_object`) and compose them
with compiler-private callback objects and admitted program-entry bindings
(`publish_fragment_object`). We verify that the resulting `ObjectArtifact`
carries the exact hosted receiver or UEFI physical entry binding required by the
admitted `ValidatedNativeProgramEntrySettlement` before returning a
`PublishedFragmentObject`.
*/

use diagnostics::Diagnostic;

use crate::native_artifact_error;
use terminal_psi_to_abstract_operations::ValidatedNativeProgramEntrySettlement;

pub struct FragmentObjectPublicationRequest<'request> {
    pub program_entry: Option<&'request ValidatedNativeProgramEntrySettlement>,
    pub private_functions: &'request [image_emission::PrivateFunctionObject],
}

pub struct FragmentObjectProjection {
    source: std::sync::Arc<object_file::StagedOptimizedRelocationFreeObjectContainer>,
    object: image_emission::ObjectArtifact,
}

impl FragmentObjectProjection {
    pub fn with_semantic_source(
        mut self,
        code: terminal_codec::RetainedTerminalCode,
        coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
    ) -> Result<Self, &'static str> {
        self.object = self.object.with_semantic_source(code, coverage)?;
        Ok(self)
    }

    pub fn object(&self) -> &image_emission::ObjectArtifact {
        &self.object
    }

    pub fn entry_call_plan(&self) -> Result<&calling_conventions::CallPlan, Vec<Diagnostic>> {
        let targeted = self
            .source
            .source()
            .source()
            .source()
            .source()
            .optimized_target()
            .target_operations();
        targeted
            .functions
            .iter()
            .find(|function| function.machine == self.object.entry())
            .map(|function| &function.graph.call_plan)
            .ok_or_else(|| {
                native_artifact_error(
                    "entry transport",
                    "emitted entry has no retained target function",
                )
            })
    }

    pub fn into_object(self) -> image_emission::ObjectArtifact {
        self.object
    }
}

pub struct PublishedFragmentObject {
    pub object: image_emission::ObjectArtifact,
}

pub fn publish_fragment_object(
    projection: FragmentObjectProjection,
    request: FragmentObjectPublicationRequest<'_>,
) -> Result<PublishedFragmentObject, Vec<Diagnostic>> {
    let FragmentObjectProjection { object, .. } = projection;
    let mut object =
        image_emission::compose_private_function_objects(object, request.private_functions)
            .map_err(|error| native_artifact_error("private object composition", error))?;
    if let Some(entry) = request.program_entry {
        if entry.access_view_establishments().len() != entry.private_entry_ram().len()
            || entry
                .access_view_establishments()
                .iter()
                .zip(entry.private_entry_ram())
                .any(|(loan, recipe)| {
                    loan.input != *recipe.input()
                        || loan.referent.structural_type != recipe.schema()
                        || !loan.referent.path.is_empty()
                        || !loan.referent.qualifications.is_empty()
                })
            || !entry.private_entry_ram().is_empty()
                && !([
                    target::NativeTarget::linux_x64(),
                    target::NativeTarget::linux_arm64(),
                    target::NativeTarget::macos_arm64(),
                ]
                .contains(&object.target())
                    || object.target() == target::NativeTarget::windows_x64()
                        && entry.source().target_slot().owner == target::TargetProfile::WindowsX64)
        {
            return Err(native_artifact_error(
                "ProgramEntry access-view custody",
                "bound access-view establishments require the hosted entry boundary to lend each referent; this bridge does not carry them yet",
            ));
        }
        if entry.source().target_slot().owner == target::TargetProfile::UefiX64 {
            let adapter = entry.physical_adapter().ok_or_else(|| {
                native_artifact_error(
                    "physical program entry",
                    "semantic entry requires an explicitly selected checked physical adapter",
                )
            })?;
            if object.entry() != adapter.machine() {
                return Err(native_artifact_error(
                    "physical program entry",
                    "emitted root is not the selected physical adapter",
                ));
            }
            let physical = entry.physical_calling_plan().ok_or_else(|| {
                native_artifact_error("physical program entry", "missing physical calling plan")
            })?;
            let contract = entry
                .storage_entry()
                .and_then(|storage| storage.physical_contract())
                .ok_or_else(|| {
                    native_artifact_error(
                        "physical program entry",
                        "missing selected physical entry contract",
                    )
                })?;
            image_emission::bind_physical_entry(
                &mut object,
                entry.checked_entry().terminal_entry(),
                physical.plan(),
                adapter.parameter_formals(),
                contract,
            )
            .map_err(|diagnostic| vec![diagnostic])?;
        } else {
            let contract = entry
                .storage_entry()
                .and_then(|storage| storage.physical_contract())
                .ok_or_else(|| {
                    native_artifact_error(
                        "ProgramEntry receiver provisioning",
                        "missing exact hosted physical contract",
                    )
                })?;
            let demand =
                image_emission::derive_stack_demand(&object, object.entry()).map_err(|error| {
                    native_artifact_error("ProgramEntry receiver stack demand", error)
                })?;
            image_emission::bind_hosted_receiver(
                &mut object,
                entry.source(),
                entry.checked_entry(),
                contract,
                &demand,
                entry.private_entry_ram(),
            )
            .map_err(|diagnostic| vec![diagnostic])?;
        }
    }
    image_emission::require_installed_root_provision(&object)
        .map_err(|error| native_artifact_error("installed physical stack", error))?;
    validate_emitted_receiver_binding(&object, request.program_entry)?;
    Ok(PublishedFragmentObject { object })
}

pub fn emit_fragment_object(
    emission: machine_emission::StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<FragmentObjectProjection, Vec<Diagnostic>> {
    let plan = emission.optimized_target().optimized().shared_program();
    let emitted = machine_emission::stage_optimized_function_fragment_emission(emission)
        .map_err(|error| native_artifact_error("function-fragment emission", error))?;
    let applied = machine_emission::stage_function_fragment_frame_application(emitted)
        .map_err(|error| native_artifact_error("function-fragment frame application", error))?;
    let text = machine_emission::stage_optimized_fixed_frame_text_section(applied)
        .map_err(|error| native_artifact_error("framed text placement", error))?;
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(text)
            .map_err(|error| native_artifact_error("fragment object placement", error))?,
    );
    let object = image_emission::build_function_fragment_object_artifact(
        image_emission::FunctionFragmentObjectArtifactRequest {
            replay: std::sync::Arc::clone(&source),
            dynamic_dispatch: &plan.dynamic_dispatch,
        },
    )
    .map_err(|error| native_artifact_error("fragment object publication", error))?;
    Ok(FragmentObjectProjection { source, object })
}

fn validate_emitted_receiver_binding(
    object: &image_emission::ObjectArtifact,
    admitted: Option<&ValidatedNativeProgramEntrySettlement>,
) -> Result<(), Vec<Diagnostic>> {
    let binding = object.hosted_receiver_binding();
    let Some(settlement) = admitted else {
        return if binding.is_none() && object.physical_entry_binding().is_none() {
            Ok(())
        } else {
            Err(native_artifact_error(
                "ProgramEntry receiver provisioning",
                "emitted object carries a hosted receiver binding that admission never granted",
            ))
        };
    };
    if settlement.source().target_slot().owner == target::TargetProfile::UefiX64 {
        return if binding.is_none()
            && object.physical_entry_binding().is_some_and(|binding| {
                binding.semantic_entry() == settlement.checked_entry().terminal_entry()
                    && settlement
                        .storage_entry()
                        .and_then(|storage| storage.physical_contract())
                        == Some(binding.requirements().contract())
                    && settlement.physical_calling_plan().map(|plan| plan.plan())
                        == Some(binding.plan())
                    && settlement
                        .storage_entry()
                        .and_then(|storage| storage.physical_contract())
                        .and_then(|contract| contract.guaranteed_entry_stack())
                        == Some(binding.arrival_stack())
            })
            && settlement
                .physical_adapter()
                .is_some_and(|adapter| adapter.machine() == object.entry())
        {
            Ok(())
        } else {
            Err(native_artifact_error(
                "ProgramEntry receiver provisioning",
                "admitted UEFI physical adapter did not reach the emitted entry",
            ))
        };
    }
    let Some(binding) = binding else {
        return Err(native_artifact_error(
            "ProgramEntry receiver provisioning",
            "admitted receiver provisioning did not reach the emitted object",
        ));
    };
    let admitted_contract = settlement
        .storage_entry()
        .and_then(|storage| storage.physical_contract());
    if binding.source() != settlement.source()
        || admitted_contract != Some(binding.physical_contract())
    {
        return Err(native_artifact_error(
            "ProgramEntry receiver provisioning",
            "emitted hosted receiver binding does not carry the admitted source signature and physical contract",
        ));
    }
    Ok(())
}
