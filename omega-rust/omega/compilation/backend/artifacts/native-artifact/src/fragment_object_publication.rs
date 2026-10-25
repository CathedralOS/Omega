//! Publish machine fragments as an object with admitted entry custody.

use diagnostics::Diagnostic;

use crate::NativePhysicalEvidenceScope;
use terminal_psi_to_abstract_operations::ValidatedNativeProgramEntrySettlement;

/// Entry custody and companion functions for one emitted object.
pub struct FragmentObjectPublicationRequest<'request> {
    /// The validated admission custody the binder must satisfy exactly. This
    /// is the replayed settlement, not the caller-supplied declaration: its
    /// source signature, storage contract, and Fused establishment roster were
    /// already joined against the canonical artifact before reaching emission.
    pub program_entry: Option<&'request ValidatedNativeProgramEntrySettlement>,
    /// The materialized compiler-private callback functions, in placement
    /// order. Each record carries real emitted bytes produced by the shared
    /// physical pipeline inside this same realization.
    pub private_functions: &'request [image_emission::PrivateFunctionObject],
}

/// The ordinary module projection and its original fragment custody.
/// Emitting this before discovering callback bodies prunes dead destinations
/// through the normal optimizer, rather than publishing orphan private code.
pub struct FragmentObjectProjection {
    source: std::sync::Arc<object_file::StagedOptimizedRelocationFreeObjectContainer>,
    object: image_emission::ObjectArtifact,
}

impl FragmentObjectProjection {
    pub fn object(&self) -> &image_emission::ObjectArtifact {
        &self.object
    }

    /// Borrow the entry transport that ordinary instruction selection consumed.
    /// Scalar and mixed ABI summaries describe subsets of signatures; the
    /// retained control graph carries the complete plan, including borrowed
    /// referents, owned aggregates and resultless entries. Reading it does not
    /// reconstruct or replay a plan after emission.
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
                crate::native_artifact_error(
                    "entry transport",
                    "emitted entry has no retained target function",
                )
            })
    }

    pub fn into_object(self) -> image_emission::ObjectArtifact {
        self.object
    }
}

/// One emitted object and the custody retained by its publication checks.
pub struct PublishedFragmentObject {
    pub object: image_emission::ObjectArtifact,
    pub physical_evidence_scope: NativePhysicalEvidenceScope,
}

/// Compose the already emitted modules and bind the admitted entry before
/// capturing publication evidence. No producing stage is replayed here.
pub fn publish_fragment_object(
    projection: FragmentObjectProjection,
    request: FragmentObjectPublicationRequest<'_>,
) -> Result<PublishedFragmentObject, Vec<Diagnostic>> {
    let FragmentObjectProjection { object, .. } = projection;
    let mut object =
        image_emission::compose_private_function_objects(object, request.private_functions)
            .map_err(|error| crate::native_artifact_error("private object composition", error))?;
    // The publication receipt retains the complete immutable object, including
    // entry metadata. Bind before capture; later equality must still reject
    // any replacement, omission or mutation of that checked binding.
    if let Some(entry) = request.program_entry {
        // The settlement is the boundary that would lend each bound
        // placed-view referent; neither the hosted receiver bridge nor the
        // semantic wrapper carries that custody yet, so a nonempty bound set
        // fails closed here rather than publishing an entry whose loans
        // nobody emits.
        if entry.placed_view_establishments().len() != entry.private_entry_ram().len()
            || entry
                .placed_view_establishments()
                .iter()
                .zip(entry.private_entry_ram())
                .any(|(loan, recipe)| {
                    loan.input != *recipe.input()
                        || loan.referent.structural_type != recipe.schema()
                        || !loan.referent.path.is_empty()
                        || !loan.referent.qualifications.is_empty()
                })
            || !entry.private_entry_ram().is_empty()
                && ![
                    target::NativeTarget::linux_x64(),
                    target::NativeTarget::macos_arm64(),
                ]
                .contains(&object.target())
        {
            return Err(crate::native_artifact_error(
                "ProgramEntry placed-view custody",
                "bound placed-view establishments require the hosted entry boundary to lend each referent; this bridge does not carry them yet",
            ));
        }
        if entry.source().target_slot().owner == target::TargetProfile::UefiX64 {
            let adapter = entry.physical_adapter().ok_or_else(|| {
                crate::native_artifact_error(
                    "physical program entry",
                    "semantic entry requires an explicitly selected checked physical adapter",
                )
            })?;
            if object.entry() != adapter.machine() {
                return Err(crate::native_artifact_error(
                    "physical program entry",
                    "emitted root is not the selected physical adapter",
                ));
            }
            let physical = entry.physical_calling_plan().ok_or_else(|| {
                crate::native_artifact_error(
                    "physical program entry",
                    "missing physical calling plan",
                )
            })?;
            let guarantee = entry
                .storage_entry()
                .and_then(|storage| storage.physical_contract())
                .and_then(|contract| contract.guaranteed_entry_stack())
                .ok_or_else(|| {
                    crate::native_artifact_error(
                        "physical program entry",
                        "missing target entry stack guarantee",
                    )
                })?;
            image_emission::bind_physical_entry(
                &mut object,
                physical.plan(),
                adapter.parameter_formals(),
                guarantee,
            )
            .map_err(|diagnostic| vec![diagnostic])?;
        } else {
            let contract = entry
                .storage_entry()
                .and_then(|storage| storage.physical_contract())
                .ok_or_else(|| {
                    crate::native_artifact_error(
                        "ProgramEntry receiver provisioning",
                        "missing exact hosted physical contract",
                    )
                })?;
            let demand =
                image_emission::derive_stack_demand(&object, object.entry()).map_err(|error| {
                    crate::native_artifact_error("ProgramEntry receiver stack demand", error)
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
    validate_emitted_receiver_binding(&object, request.program_entry)?;
    Ok(PublishedFragmentObject {
        object,
        // A normal compile publishes its object, not an independently replayed
        // PCC proof bundle. Retained-artifact verification has separate entry
        // points; adding a native call kind must not extend the retired D41
        // child catalog just to execute it (pipeline.md, verification placement).
        physical_evidence_scope: NativePhysicalEvidenceScope::Unavailable,
    })
}

/// Produce an ordinary module object, without fabricating application entry custody.
/// Root publication and private callback bodies share this emission operation.
pub fn emit_fragment_object(
    emission: machine_emission::StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<FragmentObjectProjection, Vec<Diagnostic>> {
    let plan = emission.optimized_target().optimized().shared_program();
    let emitted = machine_emission::stage_optimized_function_fragment_emission(emission)
        .map_err(|error| crate::native_artifact_error("function-fragment emission", error))?;
    let applied =
        machine_emission::stage_function_fragment_frame_application(emitted).map_err(|error| {
            crate::native_artifact_error("function-fragment frame application", error)
        })?;
    let text = machine_emission::stage_optimized_fixed_frame_text_section(applied)
        .map_err(|error| crate::native_artifact_error("framed text placement", error))?;
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(text)
            .map_err(|error| crate::native_artifact_error("fragment object placement", error))?,
    );
    let object = image_emission::build_function_fragment_object_artifact(
        image_emission::FunctionFragmentObjectArtifactRequest {
            replay: std::sync::Arc::clone(&source),
            dynamic_dispatch: &plan.dynamic_dispatch,
        },
    )
    .map_err(|error| crate::native_artifact_error("fragment object publication", error))?;
    Ok(FragmentObjectProjection { source, object })
}

/// Admission is demand, not construction. The emitted object must carry a
/// hosted receiver binding iff one was admitted, and that binding must be the
/// validated settlement's exact source signature and physical contract — a
/// missing binding is bypassed provisioning, and a substituted one binds a
/// different continuation, receiver, or arrival contract than the checked
/// source selected.
fn validate_emitted_receiver_binding(
    object: &image_emission::ObjectArtifact,
    admitted: Option<&ValidatedNativeProgramEntrySettlement>,
) -> Result<(), Vec<Diagnostic>> {
    let binding = object.hosted_receiver_binding();
    let Some(settlement) = admitted else {
        return if binding.is_none() && object.physical_entry_binding().is_none() {
            Ok(())
        } else {
            Err(crate::native_artifact_error(
                "ProgramEntry receiver provisioning",
                "emitted object carries a hosted receiver binding that admission never granted",
            ))
        };
    };
    if settlement.source().target_slot().owner == target::TargetProfile::UefiX64 {
        // Source owns firmware bootstrap and calls the semantic continuation
        // with already established arguments. The image owns ABI transport and
        // the retained private stack, not firmware policy or root authority.
        return if binding.is_none()
            && object.physical_entry_binding().is_some_and(|binding| {
                settlement.physical_calling_plan().map(|plan| plan.plan()) == Some(binding.plan())
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
            Err(crate::native_artifact_error(
                "ProgramEntry receiver provisioning",
                "admitted UEFI physical adapter did not reach the emitted entry",
            ))
        };
    }
    let Some(binding) = binding else {
        return Err(crate::native_artifact_error(
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
        return Err(crate::native_artifact_error(
            "ProgramEntry receiver provisioning",
            "emitted hosted receiver binding does not carry the admitted source signature and physical contract",
        ));
    }
    Ok(())
}
