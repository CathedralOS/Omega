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
use verified_optimization_input::ValidatedNativeProgramEntrySettlement;

pub struct FragmentObjectPublicationRequest<'request> {
    pub runtime: Option<
        verified_optimization_input::runtime_provisions::NativeRuntimePlan<
            object_file::ForeignAdapterObject,
        >,
    >,
    pub program_entry: Option<&'request ValidatedNativeProgramEntrySettlement>,
    pub private_functions: &'request [image_emission::PrivateFunctionObject],
    pub physical_entry: Option<image_emission::EncodedPhysicalEntry>,
    pub hosted_entry: Option<image_emission::EncodedHostedProgramEntry>,
}

pub struct FragmentObjectProjection {
    object: image_emission::ObjectArtifact,
}

impl FragmentObjectProjection {
    pub fn object(&self) -> &image_emission::ObjectArtifact {
        &self.object
    }

    pub fn entry_call_plan(&self) -> Result<&calling_conventions::CallPlan, Vec<Diagnostic>> {
        self.object.entry_call_plan().ok_or_else(|| {
            native_artifact_error(
                "entry transport",
                "emitted entry has no current calling contract",
            )
        })
    }

    pub fn into_object(self) -> image_emission::ObjectArtifact {
        self.object
    }
}

/// The root's runtime adapter objects remain attached when the driver settles the image request.
/// ```compile_fail,E0616
/// fn drop_runtime(mut object: native_artifact::PublishedFragmentObject) {
///     object.runtime = None;
/// }
/// ```
pub struct PublishedFragmentObject {
    pub(crate) object: image_emission::ObjectArtifact,
    pub(crate) runtime: Option<
        verified_optimization_input::runtime_provisions::NativeRuntimePlan<
            object_file::ForeignAdapterObject,
        >,
    >,
}

impl PublishedFragmentObject {
    pub fn object(&self) -> &image_emission::ObjectArtifact {
        &self.object
    }
}

pub fn publish_fragment_object(
    projection: FragmentObjectProjection,
    request: FragmentObjectPublicationRequest<'_>,
) -> Result<PublishedFragmentObject, Vec<Diagnostic>> {
    let FragmentObjectProjection { object, .. } = projection;
    let mut object =
        image_emission::compose_private_function_objects(object, request.private_functions)
            .map_err(|error| native_artifact_error("private object composition", error))?;
    if let Some(hosted_entry) = request.hosted_entry {
        image_emission::bind_hosted_program_entry(&mut object, hosted_entry)
            .map_err(|error| vec![error])?;
    }
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
            let planned = request.physical_entry.ok_or_else(|| {
                native_artifact_error(
                    "physical program entry",
                    "physical entry was not planned and encoded with its bodies",
                )
            })?;
            image_emission::bind_encoded_physical_entry(&mut object, planned)
                .map_err(|diagnostic| vec![diagnostic])?;
        } else if object.hosted_receiver_binding().is_none() {
            return Err(native_artifact_error(
                "ProgramEntry receiver provisioning",
                "hosted entry was not planned and encoded with its bodies",
            ));
        }
    }
    image_emission::require_installed_root_provision(&object)
        .map_err(|error| native_artifact_error("installed physical stack", error))?;
    validate_emitted_receiver_binding(&object, request.program_entry)?;
    if let Some(entry) = request.program_entry
        && request.runtime.as_ref().is_none_or(|runtime| {
            runtime.entry() != entry.checked_entry().terminal_entry()
                || runtime.source_signature_identity()
                    != entry.checked_entry().source_signature_identity()
                || runtime.target_slot() != entry.source().target_slot()
                || runtime.private_entry_ram() != entry.private_entry_ram()
        })
    {
        return Err(native_artifact_error(
            "runtime provisions",
            "runtime entry or private RAM differs from the checked object entry",
        ));
    }
    let runtime = request
        .runtime
        .map(|plan| plan.with_borrowed_storage_layouts(object.borrowed_storage_layouts().clone()));
    Ok(PublishedFragmentObject { object, runtime })
}

/// Publish a completed body from its stage 16 object alone.
///
/// ```compile_fail,E0308
/// fn substitute_view(view: object_file::ObjectPublication) {
///     let _ = native_artifact::emit_fragment_object(view);
/// }
/// ```
pub fn emit_fragment_object(
    issued: object_file::PublishedFunctionObject,
) -> Result<FragmentObjectProjection, Vec<Diagnostic>> {
    let (publication, replay) = issued.into_parts();
    let mut object = image_emission::build_function_fragment_object_artifact(publication)
        .map_err(|error| native_artifact_error("fragment object publication", error))?;
    if let Some(replay) = replay {
        object
            .attach_issued_replay(replay)
            .map_err(|error| vec![error])?;
    }
    Ok(FragmentObjectProjection { object })
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
