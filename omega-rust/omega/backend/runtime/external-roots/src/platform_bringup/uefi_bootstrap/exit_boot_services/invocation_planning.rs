//! Planned and bound exit-boot-services invocations and their errors.

use crate::platform_bringup::uefi_bootstrap::UefiOsHandoffMapAcquired;
use crate::platform_bringup::uefi_bootstrap::exit_boot_services::{
    EXIT_BOOT_SERVICES_FIELD_ALIGNMENT, EXIT_BOOT_SERVICES_FIELD_OFFSET,
    EXIT_BOOT_SERVICES_FIELD_ORDINAL, EXIT_BOOT_SERVICES_FIELD_SIZE,
    LifecycleScopedUefiExitBootServicesProvider, evaluate_exit_boot_services_plan,
    matches_exact_uefi_x64_call_plan, validate_exact_call_shape,
};
use crate::{
    ExternalRootDiagnostic, UefiImageHandleOccurrenceId, UefiMemoryMapKeyId, UefiOsHandoffId,
    UefiPhysicalInvocationId,
};
use calling_conventions::{BoundaryEntryPlan, MachineRegister, ValidatedBoundaryEntryPlan};
use std::ffi::c_void;
use std::num::NonZeroU64;
use target::UefiBootServicesNativeFieldKind;

/// One exact two-operand invocation joined to the live provider carrier. The
/// retained plan fixes RCX/RDX inputs, RAX status, shadow space, and clobbers
/// without performing the firmware call.
#[must_use = "planned UEFI ExitBootServices invocation retains provider and physical custody"]
pub struct PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services> {
    pub(crate) provider: LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services>,
    pub(crate) plan: ValidatedBoundaryEntryPlan,
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
        self.provider.service_identity()
    }
    pub const fn plan(&self) -> &BoundaryEntryPlan {
        self.plan.plan()
    }
    pub fn calling_plan_report_fingerprint(&self) -> u64 {
        self.plan.contract_report_fingerprint()
    }
    pub fn calling_plan_commitment(&self) -> [u8; 32] {
        self.plan.contract_commitment_digest()
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
/// call shape. The evaluated plan fixes the RCX/RDX operand placement and RAX
/// status under the Microsoft-x64 policy; its exactness is replayed here and
/// at every later custody transition.
pub fn prepare_uefi_exit_boot_services_invocation<'system_table, 'boot_services>(
    provider: LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services>,
) -> Result<
    PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
    Box<UefiExitBootServicesInvocationPlanningError<'system_table, 'boot_services>>,
> {
    let plan = match evaluate_exit_boot_services_plan() {
        Ok(plan) => plan,
        Err(diagnostic) => {
            return Err(Box::new(UefiExitBootServicesInvocationPlanningError {
                provider,
                diagnostic,
            }));
        }
    };
    if let Err(diagnostic) = validate_exact_call_shape(plan.plan()) {
        return Err(Box::new(UefiExitBootServicesInvocationPlanningError {
            provider,
            diagnostic,
        }));
    }
    if (
        provider.field.ordinal(),
        provider.field.byte_offset(),
        provider.field.byte_size(),
        provider.field.alignment(),
        provider.field.kind(),
    ) != (
        EXIT_BOOT_SERVICES_FIELD_ORDINAL,
        EXIT_BOOT_SERVICES_FIELD_OFFSET,
        EXIT_BOOT_SERVICES_FIELD_SIZE,
        EXIT_BOOT_SERVICES_FIELD_ALIGNMENT,
        UefiBootServicesNativeFieldKind::FunctionPointer,
    ) {
        return Err(Box::new(UefiExitBootServicesInvocationPlanningError {
            provider,
            diagnostic: ExternalRootDiagnostic(
                "UEFI ExitBootServices provider service row drifted before planning".into(),
            ),
        }));
    }
    Ok(PlannedUefiExitBootServicesInvocation { provider, plan })
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
    pub fn calling_plan_report_fingerprint(&self) -> u64 {
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
    if !matches_exact_uefi_x64_call_plan(&invocation.plan)
        || validate_exact_call_shape(invocation.plan.plan()).is_err()
    {
        return reject(
            invocation,
            "UEFI ExitBootServices operand binding plan drifted from its exact call shape",
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
