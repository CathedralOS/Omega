//! Bootstrap adapter composition and invocation readiness.

use crate::platform_bringup::uefi_bootstrap::{
    UefiApplicationBootstrapAdapterInvocationReadiness,
    UefiApplicationBootstrapSameStackBudgetPlan, UefiApplicationFirmwareLedger,
    UefiApplicationPhysicalArrival,
};
use crate::{ExternalRootDiagnostic, UefiApplicationBootstrapLedgerId, UefiPhysicalInvocationId};
use calling_conventions::{CallSignature, CallingPolicy, validate_boundary_entry_plan};
use program_entry_plan::{
    OptimizedProgramStoragePhysicalEntryDisposition, OptimizedProgramStorageSemanticEntryContract,
    ProgramEntrySourceReceiverSignature, ProgramEntrySourceResultSignature,
    ProgramEntrySourceSignatureIdentity, ProgramStorageEntryRootRole,
    replayed_uefi_x64_physical_calling_plan,
};

/// Exact address-free composition of the target-fixed UEFI physical arrival
/// and the build-selected semantic `ProgramStorageEntry::enter` continuation.
///
/// This is the first retained target-runtime adapter carrier. It owns the
/// private physical-arrival custody, the exact four-term same-stack plan, and
/// the independently checked semantic entry contract. It does not claim that
/// a physical shell was emitted or invoked, that the other three stack
/// contributors have acquired derivation evidence, or that either semantic
/// root exists yet.
#[must_use = "UEFI bootstrap adapter composition retains physical and semantic entry custody"]
pub struct UefiApplicationBootstrapAdapterComposition<'occurrence> {
    pub(super) readiness: UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,
    same_stack_budget: UefiApplicationBootstrapSameStackBudgetPlan,
    semantic_entry: OptimizedProgramStorageSemanticEntryContract,
    semantic_calling_plan_commitment: [u8; 32],
}

impl std::fmt::Debug for UefiApplicationBootstrapAdapterComposition<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiApplicationBootstrapAdapterComposition")
            .field("ledger", &self.ledger_id())
            .field("physical_invocation", &self.physical_invocation())
            .field(
                "physical_requirement_identity",
                &self.physical_requirement_identity(),
            )
            .field(
                "semantic_requirement_identity",
                &self.semantic_requirement_identity(),
            )
            .field(
                "semantic_source_signature_identity",
                &self.semantic_entry.source_signature_identity(),
            )
            .field(
                "required_entry_stack_bytes",
                &self.same_stack_budget.required_entry_stack_bytes(),
            )
            .finish_non_exhaustive()
    }
}

impl UefiApplicationBootstrapAdapterComposition<'_> {
    pub const fn ledger_id(&self) -> UefiApplicationBootstrapLedgerId {
        self.readiness.ledger_id()
    }

    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.readiness.physical_invocation()
    }

    pub fn physical_requirement_identity(&self) -> &str {
        self.readiness.physical_requirement_identity()
    }

    pub fn semantic_requirement_identity(&self) -> &str {
        self.semantic_entry.requirement_identity()
    }

    pub const fn semantic_source_signature_identity(&self) -> ProgramEntrySourceSignatureIdentity {
        self.semantic_entry.source_signature_identity()
    }

    pub const fn same_stack_budget(&self) -> &UefiApplicationBootstrapSameStackBudgetPlan {
        &self.same_stack_budget
    }

    pub const fn semantic_calling_plan_commitment(&self) -> &[u8; 32] {
        &self.semantic_calling_plan_commitment
    }
}

/// Recoverable adapter-composition rejection. No failure drops either the
/// physical arrival, stack plan, or semantic entry contract.
#[derive(Debug)]
#[must_use = "UEFI adapter-composition rejection retains every composition input"]
pub struct UefiApplicationBootstrapAdapterCompositionError<'occurrence> {
    readiness: UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,
    same_stack_budget: UefiApplicationBootstrapSameStackBudgetPlan,
    semantic_entry: OptimizedProgramStorageSemanticEntryContract,
    diagnostic: ExternalRootDiagnostic,
}

impl<'occurrence> UefiApplicationBootstrapAdapterCompositionError<'occurrence> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,
        UefiApplicationBootstrapSameStackBudgetPlan,
        OptimizedProgramStorageSemanticEntryContract,
        ExternalRootDiagnostic,
    ) {
        (
            self.readiness,
            self.same_stack_budget,
            self.semantic_entry,
            self.diagnostic,
        )
    }
}

impl std::fmt::Display for UefiApplicationBootstrapAdapterCompositionError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiApplicationBootstrapAdapterCompositionError<'_> {}

/// Consume physical readiness, its exact same-stack plan, and one checked
/// semantic entry into the first address-free UEFI bootstrap-adapter carrier.
///
/// The join independently validates the semantic call plan and exact root
/// order, then requires its paired physical contract to be byte-for-byte the
/// contract already retained beneath the private firmware-ledger readiness.
pub fn compose_uefi_application_bootstrap_adapter<'occurrence>(
    readiness: UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,
    same_stack_budget: UefiApplicationBootstrapSameStackBudgetPlan,
    semantic_entry: OptimizedProgramStorageSemanticEntryContract,
) -> Result<
    UefiApplicationBootstrapAdapterComposition<'occurrence>,
    Box<UefiApplicationBootstrapAdapterCompositionError<'occurrence>>,
> {
    let reject = |readiness, same_stack_budget, semantic_entry, message: &'static str| {
        Err(Box::new(UefiApplicationBootstrapAdapterCompositionError {
            readiness,
            same_stack_budget,
            semantic_entry,
            diagnostic: ExternalRootDiagnostic(message.into()),
        }))
    };

    if !same_stack_budget.matches_exact_adapter_readiness(&readiness) {
        return reject(
            readiness,
            same_stack_budget,
            semantic_entry,
            "UEFI bootstrap adapter stack plan belongs to different physical readiness custody",
        );
    }
    if semantic_entry.target() != target::NativeTarget::uefi_x64()
        || semantic_entry.target_slot() != readiness.arrival.physical_contract.target_slot()
        || semantic_entry.physical_disposition()
            != OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1
        || semantic_entry.physical_contract() != &readiness.arrival.physical_contract
        || !semantic_entry
            .physical_contract()
            .matches_exact_uefi_x64_physical_contract()
    {
        return reject(
            readiness,
            same_stack_budget,
            semantic_entry,
            "UEFI bootstrap adapter semantic entry does not retain the exact physical arrival contract",
        );
    }
    if semantic_entry.physical_contract().requirement_identity()
        == semantic_entry.requirement_identity()
        || semantic_entry.source_signature().receiver()
            != &ProgramEntrySourceReceiverSignature::Free
        || semantic_entry.source_signature().result() != ProgramEntrySourceResultSignature::Unit
    {
        return reject(
            readiness,
            same_stack_budget,
            semantic_entry,
            "UEFI bootstrap adapter conflates its physical and semantic entry surfaces",
        );
    }
    let [image, initial_storage] = semantic_entry.roots();
    if image.role() != ProgramStorageEntryRootRole::Image
        || image.parameter_index() != 0
        || initial_storage.role() != ProgramStorageEntryRootRole::InitialStorage
        || initial_storage.parameter_index() != 1
    {
        return reject(
            readiness,
            same_stack_budget,
            semantic_entry,
            "UEFI bootstrap adapter semantic roots are not exact Image then InitialStorage",
        );
    }
    let semantic_signature = CallSignature {
        parameters: vec![image.shape(), initial_storage.shape()],
        result: None,
    };
    let semantic_plan = match validate_boundary_entry_plan(
        semantic_entry.semantic_boundary_entry_plan().clone(),
        &semantic_signature,
    ) {
        Ok(plan) => plan,
        Err(_) => {
            return reject(
                readiness,
                same_stack_budget,
                semantic_entry,
                "UEFI bootstrap adapter semantic calling plan failed independent replay",
            );
        }
    };
    let Some(generated_adapter_live_frame_demand) =
        same_stack_budget.generated_adapter_live_frame_demand()
    else {
        return reject(
            readiness,
            same_stack_budget,
            semantic_entry,
            "UEFI bootstrap adapter composition lacks exact generated live-frame evidence",
        );
    };
    if semantic_plan.plan().call.policy != CallingPolicy::MicrosoftX64
        || semantic_plan.plan().call.parameters.len() != 2
        || semantic_plan.plan().call.result.is_some()
        || semantic_plan.contract_report_fingerprint()
            != semantic_entry.semantic_calling_plan_report_fingerprint()
        || semantic_plan.contract_report_fingerprint()
            == readiness
                .arrival
                .physical_contract
                .calling_plan_report_fingerprint()
        || generated_adapter_live_frame_demand.semantic_boundary_commitment()
            != semantic_plan.contract_commitment_digest()
        || generated_adapter_live_frame_demand.bytes()
            != same_stack_budget.components.live_adapter_frames_wcsu_bytes
        || generated_adapter_live_frame_demand.alignment()
            != u64::from(semantic_plan.plan().call.stack_alignment)
    {
        return reject(
            readiness,
            same_stack_budget,
            semantic_entry,
            "UEFI bootstrap adapter semantic and physical ABI identities are not distinct and exact",
        );
    }

    Ok(UefiApplicationBootstrapAdapterComposition {
        readiness,
        same_stack_budget,
        semantic_entry,
        semantic_calling_plan_commitment: semantic_plan.contract_commitment_digest(),
    })
}

/// Recoverable readiness rejection retaining the complete physical arrival.
#[derive(Debug)]
#[must_use = "UEFI adapter-readiness rejection retains physical-arrival custody"]
pub struct UefiApplicationBootstrapAdapterReadinessError<'occurrence> {
    arrival: UefiApplicationPhysicalArrival<'occurrence>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'occurrence> UefiApplicationBootstrapAdapterReadinessError<'occurrence> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        UefiApplicationPhysicalArrival<'occurrence>,
        ExternalRootDiagnostic,
    ) {
        (self.arrival, self.diagnostic)
    }
}

impl std::fmt::Display for UefiApplicationBootstrapAdapterReadinessError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiApplicationBootstrapAdapterReadinessError<'_> {}

/// Consume the exact physical-arrival join before any provider projection can
/// enter target-runtime adapter composition. This is readiness evidence only:
/// target-owned environment/stack evidence and a generated native invocation
/// remain required before `UefiPhysicalEntry::enter` is realized.
pub fn prepare_uefi_application_bootstrap_adapter_invocation<'occurrence>(
    ledger: &UefiApplicationFirmwareLedger<'occurrence>,
    arrival: UefiApplicationPhysicalArrival<'occurrence>,
) -> Result<
    UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,
    Box<UefiApplicationBootstrapAdapterReadinessError<'occurrence>>,
> {
    if !ledger.matches_image_handle(&arrival.image_handle)
        || !ledger.matches_provenance(&arrival.system_table.provenance)
        || !ledger.matches_lease(&arrival.system_table.phase_lease)
    {
        return Err(Box::new(UefiApplicationBootstrapAdapterReadinessError {
            arrival,
            diagnostic: ExternalRootDiagnostic(
                "UEFI adapter readiness belongs to a different or inactive physical invocation"
                    .into(),
            ),
        }));
    }
    if !arrival
        .physical_contract
        .matches_exact_uefi_x64_physical_contract()
    {
        return Err(Box::new(
            UefiApplicationBootstrapAdapterReadinessError {
                arrival,
                diagnostic: ExternalRootDiagnostic(
                    "UEFI adapter readiness does not retain the exact target-owned physical entry contract"
                        .into(),
                ),
            },
        ));
    }
    let Some(replayed) =
        replayed_uefi_x64_physical_calling_plan(arrival.physical_contract.boundary_entry_plan())
    else {
        return Err(Box::new(UefiApplicationBootstrapAdapterReadinessError {
            arrival,
            diagnostic: ExternalRootDiagnostic(
                "UEFI adapter readiness physical calling-plan replay drifted".into(),
            ),
        }));
    };
    if replayed.contract_report_fingerprint()
        != arrival.physical_contract.calling_plan_report_fingerprint()
    {
        return Err(Box::new(UefiApplicationBootstrapAdapterReadinessError {
            arrival,
            diagnostic: ExternalRootDiagnostic(
                "UEFI adapter readiness physical calling-plan replay drifted".into(),
            ),
        }));
    }
    Ok(UefiApplicationBootstrapAdapterInvocationReadiness {
        arrival,
        physical_calling_plan_commitment: replayed.contract_commitment_digest(),
    })
}
