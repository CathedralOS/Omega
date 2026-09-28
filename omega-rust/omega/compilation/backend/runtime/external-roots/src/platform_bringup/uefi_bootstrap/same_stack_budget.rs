//! Same-stack demand components and the bootstrap budget plan.

use crate::platform_bringup::uefi_bootstrap::UefiApplicationBootstrapAdapterInvocationReadiness;
use crate::{
    ExternalRootDiagnostic, GeneratedProgramStorageAdapterLiveFrameDemand,
    UefiApplicationBootstrapLedgerId, UefiBootServicesPhaseLeaseId, UefiFirmwareSessionId,
    UefiImageHandleOccurrenceId, UefiPhysicalInvocationId, UefiSystemTableOccurrenceId,
};
use program_entry_plan::UEFI_X64_PHYSICAL_CALLING_PLAN_COMMITMENT;

/// Exact numeric inputs to same-stack UEFI bootstrap planning.
///
/// These coordinates are not WCSU derivation evidence. Later compiler/runtime
/// producers must bind each value to the generated shell, checked adapter,
/// closed continuation/provider graph, and target reserve that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UefiApplicationBootstrapSameStackDemandComponents {
    generated_shell_wcsu_bytes: u64,
    pub(super) live_adapter_frames_wcsu_bytes: u64,
    maximum_nested_continuation_provider_wcsu_bytes: u64,
    target_reserve_bytes: u64,
}

impl UefiApplicationBootstrapSameStackDemandComponents {
    pub const fn new(
        generated_shell_wcsu_bytes: u64,
        live_adapter_frames_wcsu_bytes: u64,
        maximum_nested_continuation_provider_wcsu_bytes: u64,
        target_reserve_bytes: u64,
    ) -> Self {
        Self {
            generated_shell_wcsu_bytes,
            live_adapter_frames_wcsu_bytes,
            maximum_nested_continuation_provider_wcsu_bytes,
            target_reserve_bytes,
        }
    }

    pub const fn generated_shell_wcsu_bytes(self) -> u64 {
        self.generated_shell_wcsu_bytes
    }

    pub const fn live_adapter_frames_wcsu_bytes(self) -> u64 {
        self.live_adapter_frames_wcsu_bytes
    }

    pub const fn maximum_nested_continuation_provider_wcsu_bytes(self) -> u64 {
        self.maximum_nested_continuation_provider_wcsu_bytes
    }

    pub const fn target_reserve_bytes(self) -> u64 {
        self.target_reserve_bytes
    }
}

/// Planning result for the settled UEFI same-stack inequality.
///
/// The result binds the complete four-term demand to one physical-arrival
/// readiness and the exact target-owned numeric guarantee. The stronger
/// constructor additionally retains exact generated-wrapper evidence for the
/// live-adapter term; the other three terms remain unauthenticated numeric
/// inputs. Neither form is runtime stack/environment admission.
#[must_use = "UEFI same-stack budget plan must be retained for later adapter composition"]
pub struct UefiApplicationBootstrapSameStackBudgetPlan {
    readiness_authority: u64,
    ledger: UefiApplicationBootstrapLedgerId,
    session: UefiFirmwareSessionId,
    invocation: UefiPhysicalInvocationId,
    image_handle_occurrence: UefiImageHandleOccurrenceId,
    system_table_occurrence: UefiSystemTableOccurrenceId,
    phase_lease: UefiBootServicesPhaseLeaseId,
    phase_generation: u64,
    physical_calling_plan_commitment: [u8; 32],
    target_entry_stack_guarantee: target::TargetEntryStackGuarantee,
    pub(super) components: UefiApplicationBootstrapSameStackDemandComponents,
    generated_adapter_live_frame_demand: Option<GeneratedProgramStorageAdapterLiveFrameDemand>,
    required_entry_stack_bytes: u64,
}

impl std::fmt::Debug for UefiApplicationBootstrapSameStackBudgetPlan {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiApplicationBootstrapSameStackBudgetPlan")
            .field("ledger", &self.ledger)
            .field("session", &self.session)
            .field("invocation", &self.invocation)
            .field("image_handle_occurrence", &self.image_handle_occurrence)
            .field("system_table_occurrence", &self.system_table_occurrence)
            .field("phase_lease", &self.phase_lease)
            .field("components", &self.components)
            .field(
                "required_entry_stack_bytes",
                &self.required_entry_stack_bytes,
            )
            .field(
                "guaranteed_entry_stack_bytes",
                &self
                    .target_entry_stack_guarantee
                    .guaranteed_available_bytes(),
            )
            .finish_non_exhaustive()
    }
}

impl UefiApplicationBootstrapSameStackBudgetPlan {
    pub const fn ledger_id(&self) -> UefiApplicationBootstrapLedgerId {
        self.ledger
    }

    pub const fn firmware_session(&self) -> UefiFirmwareSessionId {
        self.session
    }

    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.invocation
    }

    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.image_handle_occurrence
    }

    pub const fn system_table_occurrence(&self) -> UefiSystemTableOccurrenceId {
        self.system_table_occurrence
    }

    pub const fn phase_lease_id(&self) -> UefiBootServicesPhaseLeaseId {
        self.phase_lease
    }

    pub const fn physical_calling_plan_commitment(&self) -> &[u8; 32] {
        &self.physical_calling_plan_commitment
    }

    pub const fn target_entry_stack_guarantee(&self) -> &target::TargetEntryStackGuarantee {
        &self.target_entry_stack_guarantee
    }

    pub const fn components(&self) -> UefiApplicationBootstrapSameStackDemandComponents {
        self.components
    }

    pub const fn required_entry_stack_bytes(&self) -> u64 {
        self.required_entry_stack_bytes
    }

    pub const fn generated_adapter_live_frame_demand(
        &self,
    ) -> Option<&GeneratedProgramStorageAdapterLiveFrameDemand> {
        self.generated_adapter_live_frame_demand.as_ref()
    }

    pub const fn remaining_entry_stack_bytes(&self) -> u64 {
        self.target_entry_stack_guarantee
            .guaranteed_available_bytes()
            - self.required_entry_stack_bytes
    }

    /// Rejoin the planning result to the exact still-live adapter-readiness
    /// custody. Public report coordinates alone cannot substitute a readiness
    /// minted by another private firmware ledger.
    pub fn matches_exact_adapter_readiness(
        &self,
        readiness: &UefiApplicationBootstrapAdapterInvocationReadiness<'_>,
    ) -> bool {
        self.readiness_authority == readiness.arrival.image_handle.authority
            && self.ledger == readiness.ledger_id()
            && self.session == readiness.firmware_session()
            && self.invocation == readiness.physical_invocation()
            && self.image_handle_occurrence == readiness.image_handle_occurrence()
            && self.system_table_occurrence == readiness.system_table_occurrence()
            && self.phase_lease == readiness.arrival.system_table.phase_lease.lease
            && self.phase_generation == readiness.arrival.system_table.phase_lease.generation
            && self.physical_calling_plan_commitment == readiness.physical_calling_plan_commitment
            && readiness.arrival.physical_contract.guaranteed_entry_stack()
                == Some(&self.target_entry_stack_guarantee)
    }
}

/// Replay the exact UEFI physical plan and check the settled same-stack
/// bootstrap inequality:
///
/// `shell + live adapter frames + max nested continuation/provider + reserve
/// <= target guarantee`.
///
/// Every contributor must be explicit and nonzero. This closes the numeric
/// planning relation only; component provenance and runtime firmware
/// conformance remain required before invocation admission.
pub fn plan_uefi_application_bootstrap_same_stack_budget(
    readiness: &UefiApplicationBootstrapAdapterInvocationReadiness<'_>,
    components: UefiApplicationBootstrapSameStackDemandComponents,
) -> Result<UefiApplicationBootstrapSameStackBudgetPlan, ExternalRootDiagnostic> {
    plan_uefi_application_bootstrap_same_stack_budget_inner(readiness, components, None)
}

/// Plan the same four-term inequality while deriving the live adapter-frame
/// term from exact installed generated-wrapper evidence. The remaining three
/// terms deliberately stay explicit numeric planning inputs until their own
/// producer joins land.
pub fn plan_uefi_application_bootstrap_same_stack_budget_with_generated_adapter(
    readiness: &UefiApplicationBootstrapAdapterInvocationReadiness<'_>,
    generated_shell_wcsu_bytes: u64,
    generated_adapter_live_frame_demand: GeneratedProgramStorageAdapterLiveFrameDemand,
    maximum_nested_continuation_provider_wcsu_bytes: u64,
    target_reserve_bytes: u64,
) -> Result<UefiApplicationBootstrapSameStackBudgetPlan, ExternalRootDiagnostic> {
    let components = UefiApplicationBootstrapSameStackDemandComponents::new(
        generated_shell_wcsu_bytes,
        generated_adapter_live_frame_demand.bytes(),
        maximum_nested_continuation_provider_wcsu_bytes,
        target_reserve_bytes,
    );
    plan_uefi_application_bootstrap_same_stack_budget_inner(
        readiness,
        components,
        Some(generated_adapter_live_frame_demand),
    )
}

fn plan_uefi_application_bootstrap_same_stack_budget_inner(
    readiness: &UefiApplicationBootstrapAdapterInvocationReadiness<'_>,
    components: UefiApplicationBootstrapSameStackDemandComponents,
    generated_adapter_live_frame_demand: Option<GeneratedProgramStorageAdapterLiveFrameDemand>,
) -> Result<UefiApplicationBootstrapSameStackBudgetPlan, ExternalRootDiagnostic> {
    if !readiness
        .arrival
        .physical_contract
        .matches_exact_uefi_x64_physical_contract()
    {
        return Err(ExternalRootDiagnostic(
            "UEFI same-stack planning requires the exact target-owned physical contract".into(),
        ));
    }
    if readiness.physical_calling_plan_commitment != UEFI_X64_PHYSICAL_CALLING_PLAN_COMMITMENT {
        return Err(ExternalRootDiagnostic(
            "UEFI same-stack planning physical calling-plan commitment drifted".into(),
        ));
    }
    let Some(guarantee) = readiness.arrival.physical_contract.guaranteed_entry_stack() else {
        return Err(ExternalRootDiagnostic(
            "UEFI same-stack planning lost the exact contract's numeric stack guarantee".into(),
        ));
    };
    if !guarantee.matches_exact_uefi_x64_entry_stack_guarantee()
        || Some(guarantee.application())
            != readiness
                .arrival
                .physical_contract
                .guaranteed_entry_stack_application()
        || guarantee.required_alignment()
            != u64::from(
                readiness
                    .arrival
                    .physical_contract
                    .boundary_entry_plan()
                    .call
                    .stack_alignment,
            )
    {
        return Err(ExternalRootDiagnostic(
            "UEFI same-stack planning target guarantee did not replay the exact physical contract"
                .into(),
        ));
    }

    let contributions = [
        (
            "generated shell WCSU",
            components.generated_shell_wcsu_bytes,
        ),
        (
            "live adapter-frame WCSU",
            components.live_adapter_frames_wcsu_bytes,
        ),
        (
            "maximum nested continuation/provider WCSU",
            components.maximum_nested_continuation_provider_wcsu_bytes,
        ),
        ("explicit target reserve", components.target_reserve_bytes),
    ];
    let mut required_entry_stack_bytes = 0_u64;
    for (name, bytes) in contributions {
        if bytes == 0 {
            return Err(ExternalRootDiagnostic(format!(
                "UEFI same-stack planning omitted {name}"
            )));
        }
        required_entry_stack_bytes =
            required_entry_stack_bytes
                .checked_add(bytes)
                .ok_or_else(|| {
                    ExternalRootDiagnostic(
                        "UEFI same-stack planning demand addition overflowed".into(),
                    )
                })?;
    }
    if required_entry_stack_bytes > guarantee.guaranteed_available_bytes() {
        return Err(ExternalRootDiagnostic(format!(
            "UEFI same-stack bootstrap requires {required_entry_stack_bytes} bytes but the selected target guarantees only {} bytes",
            guarantee.guaranteed_available_bytes(),
        )));
    }

    Ok(UefiApplicationBootstrapSameStackBudgetPlan {
        readiness_authority: readiness.arrival.image_handle.authority,
        ledger: readiness.ledger_id(),
        session: readiness.firmware_session(),
        invocation: readiness.physical_invocation(),
        image_handle_occurrence: readiness.image_handle_occurrence(),
        system_table_occurrence: readiness.system_table_occurrence(),
        phase_lease: readiness.arrival.system_table.phase_lease.lease,
        phase_generation: readiness.arrival.system_table.phase_lease.generation,
        physical_calling_plan_commitment: readiness.physical_calling_plan_commitment,
        target_entry_stack_guarantee: guarantee.clone(),
        components,
        generated_adapter_live_frame_demand,
        required_entry_stack_bytes,
    })
}
