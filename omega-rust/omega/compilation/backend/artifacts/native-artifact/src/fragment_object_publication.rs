//! Publish machine fragments as an object with entry custody and physical evidence.

use diagnostics::Diagnostic;

use crate::NativePhysicalEvidenceScope;
use terminal_psi_to_abstract_operations::ValidatedNativeProgramEntrySettlement;

/// Entry custody and companion functions for one emitted object.
pub struct FragmentObjectPublicationRequest<'request> {
    /// The canonical Terminal artifact the emitted object seals. The semantic
    /// wrapper route re-stages the fragment container under this exact
    /// artifact so its custody replay binds the same canonical bytes the
    /// realized product carries.
    pub terminal: &'request terminal_codec::CanonicalTerminalArtifact,
    /// The validated admission custody the binder must satisfy exactly. This
    /// is the replayed settlement, not the caller-supplied declaration: its
    /// source signature, storage contract, and Fused establishment roster were
    /// already joined against the canonical artifact before reaching emission.
    pub program_entry: Option<&'request ValidatedNativeProgramEntrySettlement>,
    pub boundary_application_coverage:
        Option<&'request boundary_applications::TerminalBoundaryApplicationCoverage>,
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
    // Physical construction already retains the admitted abstract program.
    // Share that allocation before consuming emission; do not clone the plan
    // or accept independently supplied publication identities. The publication
    // validator still replays the emitted object and checks this exact join.
    let FragmentObjectProjection { source, object } = projection;
    let optimized = source
        .source()
        .source()
        .source()
        .source()
        .optimized_target()
        .optimized();
    let plan = optimized.shared_program();
    let validation = optimized.validation();
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
                && object.target() != target::NativeTarget::linux_x64()
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
            image_emission::bind_physical_entry(
                &mut object,
                physical.plan(),
                adapter.parameter_formals(),
            )
            .map_err(|diagnostic| vec![diagnostic])?;
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
            let demand = image_emission::derive_stack_demand(&object, object.entry())
                .map_err(|error| crate::native_artifact_error("physical entry stack", error))?;
            if !guarantee.admits_demand(demand.ceiling_bytes(), u64::from(demand.stack_alignment()))
            {
                return Err(crate::native_artifact_error(
                    "physical entry stack",
                    format!(
                        "source bootstrap exceeds the target's arrival stack guarantee: demand {} bytes aligned to {}, reserve {} bytes, available {} bytes aligned to {}",
                        demand.ceiling_bytes(),
                        demand.stack_alignment(),
                        guarantee.admission_reserve_bytes(),
                        guarantee.guaranteed_available_bytes(),
                        guarantee.required_alignment(),
                    ),
                ));
            }
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
            // The checked eligibility decides whether the receiver's nominal
            // cleanup must occupy its hosted extent through completion. The
            // binding records that occupancy so the extent is tracked through
            // the installation ledger rather than silently dropping it.
            let cleanup_occupancy =
                entry
                    .checked_entry()
                    .receiver_eligibility()
                    .is_some_and(|eligibility| {
                        eligibility.cleanup()
                        == terminal_psi::CheckedProgramEntryReceiverCleanup::OccupiesHostedExtent
                    });
            let determined_evidence = entry
                .checked_entry()
                .receiver_eligibility()
                .map(|eligibility| eligibility.determined_evidence_fields())
                .unwrap_or_default();
            image_emission::bind_hosted_receiver(
                &mut object,
                entry.source(),
                entry.checked_entry(),
                contract,
                entry.fused_service_establishments(),
                determined_evidence,
                &demand,
                cleanup_occupancy,
                entry.private_entry_ram(),
            )
            .map_err(|diagnostic| vec![diagnostic])?;
        }
    }
    let physical_evidence_scope = match request.boundary_application_coverage {
        Some(coverage) => NativePhysicalEvidenceScope::from_validated_fragment_publication(
            &plan,
            validation.psi(),
            validation.identity(),
            validation.final_unit(),
            coverage,
            &source,
            &object,
        )
        .map_err(|error| {
            crate::native_artifact_error("fragment physical-evidence projection", error)
        })?,
        None => NativePhysicalEvidenceScope::Unavailable,
    };
    validate_emitted_receiver_binding(&object, request.program_entry)?;
    Ok(PublishedFragmentObject {
        object,
        physical_evidence_scope,
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
            implementation_table_addresses: &source
                .source()
                .text_section()
                .unresolved_implementation_table_addresses,
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
        // with already established arguments. The image owns only ABI transport.
        return if binding.is_none()
            && object.physical_entry_binding().is_some_and(|binding| {
                settlement.physical_calling_plan().map(|plan| plan.plan()) == Some(binding.plan())
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
    let admitted_occupancy = settlement
        .checked_entry()
        .receiver_eligibility()
        .is_some_and(|eligibility| {
            eligibility.cleanup()
                == terminal_psi::CheckedProgramEntryReceiverCleanup::OccupiesHostedExtent
        });
    if binding.cleanup_occupancy() != admitted_occupancy {
        return Err(crate::native_artifact_error(
            "ProgramEntry receiver provisioning",
            "emitted hosted receiver binding does not track the admitted cleanup occupancy",
        ));
    }
    Ok(())
}
