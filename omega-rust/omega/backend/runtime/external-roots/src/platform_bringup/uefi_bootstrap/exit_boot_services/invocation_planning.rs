//! Planned and bound exit-boot-services invocations and their errors.

use crate::platform_bringup::uefi_bootstrap::UefiOsHandoffMapAcquired;
use crate::platform_bringup::uefi_bootstrap::exit_boot_services::LifecycleScopedUefiExitBootServicesProvider;
use crate::{
    ExternalRootDiagnostic, UefiImageHandleOccurrenceId, UefiMemoryMapKeyId, UefiOsHandoffId,
    UefiPhysicalInvocationId,
};
use calling_conventions::MachineRegister;
use program_entry_plan::{UefiOsHandoffLegPlan, plan_uefi_os_handoff_invocation};
use std::ffi::c_void;
use std::num::NonZeroU64;
use target::TargetProfile;

/// One exact two-operand invocation joined to the live provider carrier. The
/// retained `ExitBootServices` leg plan fixes RCX/RDX inputs, RAX status,
/// shadow space, clobbers, and the closed status table without performing
/// the firmware call.
#[must_use = "planned UEFI ExitBootServices invocation retains provider and physical custody"]
pub struct PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services> {
    pub(crate) provider: LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services>,
    pub(crate) plan: UefiOsHandoffLegPlan,
}

impl std::fmt::Debug for PlannedUefiExitBootServicesInvocation<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlannedUefiExitBootServicesInvocation")
            .field("physical_invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("service_identity", &self.service_identity())
            .field(
                "calling_plan_report_fingerprint",
                &self.calling_plan_report_fingerprint(),
            )
            .finish_non_exhaustive()
    }
}

impl PlannedUefiExitBootServicesInvocation<'_, '_> {
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.provider.physical_invocation()
    }
    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.provider.image_handle_occurrence()
    }
    pub const fn service_identity(&self) -> &'static str {
        self.plan.service_identity()
    }
    pub const fn plan(&self) -> &UefiOsHandoffLegPlan {
        &self.plan
    }
    pub const fn calling_plan_report_fingerprint(&self) -> u64 {
        self.plan.calling_plan_report_fingerprint()
    }
    pub const fn calling_plan_commitment(&self) -> [u8; 32] {
        *self.plan.calling_plan_commitment()
    }

    /// Replay that the retained leg is still the canonical `ExitBootServices`
    /// leg for this provider's sealed service row. Every custody transition
    /// after preparation gates on this before touching an operand.
    pub(crate) fn retains_exact_plan(&self) -> bool {
        self.plan.matches_exact_uefi_x64_plan() && self.plan.service_field() == self.provider.field
    }
}

#[derive(Debug)]
#[must_use = "UEFI ExitBootServices planning rejection retains provider custody"]
pub struct UefiExitBootServicesInvocationPlanningError<'system_table, 'boot_services> {
    provider: LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'system_table, 'boot_services>
    UefiExitBootServicesInvocationPlanningError<'system_table, 'boot_services>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services>,
        ExternalRootDiagnostic,
    ) {
        (self.provider, self.diagnostic)
    }
}

impl std::fmt::Display for UefiExitBootServicesInvocationPlanningError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiExitBootServicesInvocationPlanningError<'_, '_> {}

/// Consume the live provider into the exact target-authored ExitBootServices
/// call shape. The `ExitBootServices` leg of the planned OS-handoff
/// invocation fixes the RCX/RDX operand placement, RAX status, and closed
/// status table under the Microsoft-x64 policy; the leg must replay exact
/// and name the provider's sealed service row, and every later custody
/// transition replays the same gate.
pub fn prepare_uefi_exit_boot_services_invocation<'system_table, 'boot_services>(
    provider: LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services>,
) -> Result<
    PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    Box<UefiExitBootServicesInvocationPlanningError<'system_table, 'boot_services>>,
> {
    let plan = match plan_uefi_os_handoff_invocation(TargetProfile::UefiX64) {
        Ok(plan) => plan,
        Err(error) => {
            return Err(Box::new(UefiExitBootServicesInvocationPlanningError {
                provider,
                diagnostic: ExternalRootDiagnostic(format!(
                    "UEFI OS-handoff invocation plan rejected: {}",
                    error.diagnostic()
                )),
            }));
        }
    };
    let leg = plan.exit_boot_services().clone();
    if !plan.matches_exact_uefi_x64_plan() || leg.service_field() != provider.field {
        return Err(Box::new(UefiExitBootServicesInvocationPlanningError {
            provider,
            diagnostic: ExternalRootDiagnostic(
                "UEFI ExitBootServices invocation plan drifted from its lifecycle provider".into(),
            ),
        }));
    }
    Ok(PlannedUefiExitBootServicesInvocation {
        provider,
        plan: leg,
    })
}

/// Concrete, still-uninvoked operands for one exact acquired-map
/// `ExitBootServices` attempt.
///
/// The carrier keeps the physical image handle, service function, and map key
/// private. Public observations are limited to identities and ABI
/// destinations; neither firmware operand can be reinterpreted as storage
/// authority. The bound attempt belongs to one exact
/// [`UefiOsHandoffMapAcquired`] of the same physical invocation.
#[must_use = "bound UEFI ExitBootServices operands retain provider and attempt custody"]
pub struct BoundUefiExitBootServicesInvocation<'system_table, 'boot_services> {
    pub(crate) invocation: PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    pub(crate) _handle: NonZeroU64,
    pub(crate) _service: NonZeroU64,
    pub(crate) map_key: UefiMemoryMapKeyId,
    handoff: UefiOsHandoffId,
}

impl std::fmt::Debug for BoundUefiExitBootServicesInvocation<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BoundUefiExitBootServicesInvocation")
            .field("physical_invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("handoff", &self.handoff)
            .field("map_key", &self.map_key)
            .field("argument_destinations", &self.argument_destinations())
            .finish_non_exhaustive()
    }
}

impl BoundUefiExitBootServicesInvocation<'_, '_> {
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.invocation.physical_invocation()
    }
    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.invocation.image_handle_occurrence()
    }
    pub const fn service_identity(&self) -> &'static str {
        self.invocation.service_identity()
    }
    pub const fn handoff_id(&self) -> UefiOsHandoffId {
        self.handoff
    }
    pub const fn map_key(&self) -> UefiMemoryMapKeyId {
        self.map_key
    }
    pub const fn argument_destinations(&self) -> [MachineRegister; 2] {
        [MachineRegister::X86Rcx, MachineRegister::X86Rdx]
    }
    pub const fn calling_plan_report_fingerprint(&self) -> u64 {
        self.invocation.calling_plan_report_fingerprint()
    }
}

#[derive(Debug)]
#[must_use = "UEFI ExitBootServices operand rejection retains provider custody"]
pub struct UefiExitBootServicesInvocationBindingError<'system_table, 'boot_services> {
    invocation: PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'system_table, 'boot_services>
    UefiExitBootServicesInvocationBindingError<'system_table, 'boot_services>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        ExternalRootDiagnostic,
    ) {
        (self.invocation, self.diagnostic)
    }
}

impl std::fmt::Display for UefiExitBootServicesInvocationBindingError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiExitBootServicesInvocationBindingError<'_, '_> {}

/// Bind the exact acquired map and retained physical image-handle value to the
/// RCX/RDX plan. The acquired carrier proves the key under the handoff's
/// current attempt; an acquired map from a different physical invocation, a
/// drifted plan, or address-free image-handle provenance cannot cross this
/// edge and rejects without consuming either input.
pub fn bind_uefi_exit_boot_services_invocation<'system_table, 'boot_services>(
    invocation: PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    acquired: &UefiOsHandoffMapAcquired,
) -> Result<
    BoundUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    Box<UefiExitBootServicesInvocationBindingError<'system_table, 'boot_services>>,
> {
    let reject = |invocation, message: &'static str| {
        Err(Box::new(UefiExitBootServicesInvocationBindingError {
            invocation,
            diagnostic: ExternalRootDiagnostic(message.into()),
        }))
    };
    if !invocation.retains_exact_plan() {
        return reject(
            invocation,
            "UEFI ExitBootServices operand binding plan drifted from its lifecycle provider",
        );
    }
    if acquired.physical_invocation() != invocation.provider.physical_invocation() {
        return reject(
            invocation,
            "UEFI ExitBootServices attempt bound a map acquired under a different physical invocation",
        );
    }
    let Some(handle) = invocation
        .provider
        .projection
        .readiness
        .arrival
        .image_handle
        .opaque_handle
    else {
        return reject(
            invocation,
            "UEFI ExitBootServices operand binding requires the exact physical image-handle value",
        );
    };
    let service = invocation.provider.exit_boot_services;
    Ok(BoundUefiExitBootServicesInvocation {
        invocation,
        _handle: handle,
        _service: service,
        map_key: acquired.map_key(),
        handoff: acquired.handoff_id(),
    })
}

pub(crate) type UefiExitBootServicesFunction =
    unsafe extern "efiapi" fn(image_handle: *mut c_void, map_key: usize) -> u64;
