//! Planned and bound get-memory-map invocations and their errors.

use crate::platform_bringup::uefi_bootstrap::PlannedUefiExitBootServicesInvocation;
use crate::platform_bringup::uefi_bootstrap::get_memory_map::{
    LifecycleScopedUefiGetMemoryMapProvider, UefiMemoryMapBuffer,
};
use crate::{ExternalRootDiagnostic, UefiImageHandleOccurrenceId, UefiPhysicalInvocationId};
use calling_conventions::MachineRegister;
use program_entry_plan::{UefiOsHandoffLegPlan, plan_uefi_os_handoff_invocation};
use std::ffi::c_void;
use std::num::NonZeroU64;
use target::TargetProfile;

/// One exact five-operand acquisition plan joined to the live provider
/// carrier. The retained `GetMemoryMap` leg plan fixes the RCX/RDX/R8/R9
/// operand registers, the stack-resident descriptor-version slot, RAX
/// status, shadow space, clobbers, and the closed status table without
/// performing the firmware call.
#[must_use = "planned UEFI GetMemoryMap invocation retains provider and pending-exit custody"]
pub struct PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services> {
    pub(crate) provider:
        LifecycleScopedUefiGetMemoryMapProvider<'pending_exit, 'system_table, 'boot_services>,
    pub(super) plan: UefiOsHandoffLegPlan,
}

impl std::fmt::Debug for PlannedUefiGetMemoryMapInvocation<'_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlannedUefiGetMemoryMapInvocation")
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

impl PlannedUefiGetMemoryMapInvocation<'_, '_, '_> {
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

    /// Replay that the retained leg is still the canonical `GetMemoryMap`
    /// leg for this provider's sealed service row and that the borrowed
    /// pending exit still retains its own exact leg. Every custody
    /// transition after preparation gates on this before touching an
    /// operand.
    pub(super) fn retains_exact_plan(&self) -> bool {
        self.plan.matches_exact_uefi_x64_plan()
            && self.plan.service_field() == self.provider.field
            && self.provider.pending_exit.retains_exact_plan()
    }
}

impl<'pending_exit, 'system_table, 'boot_services>
    PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services>
{
    pub fn into_pending_exit_invocation(
        self,
    ) -> &'pending_exit PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services> {
        self.provider.pending_exit
    }
}

#[derive(Debug)]
#[must_use = "UEFI GetMemoryMap planning rejection retains provider custody"]
pub struct UefiGetMemoryMapInvocationPlanningError<'pending_exit, 'system_table, 'boot_services> {
    provider: LifecycleScopedUefiGetMemoryMapProvider<'pending_exit, 'system_table, 'boot_services>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'pending_exit, 'system_table, 'boot_services>
    UefiGetMemoryMapInvocationPlanningError<'pending_exit, 'system_table, 'boot_services>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        LifecycleScopedUefiGetMemoryMapProvider<'pending_exit, 'system_table, 'boot_services>,
        ExternalRootDiagnostic,
    ) {
        (self.provider, self.diagnostic)
    }
}

impl std::fmt::Display for UefiGetMemoryMapInvocationPlanningError<'_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiGetMemoryMapInvocationPlanningError<'_, '_, '_> {}

/// Consume the live provider into the exact target-authored GetMemoryMap call
/// shape. The `GetMemoryMap` leg of the planned OS-handoff invocation fixes
/// the four register operands, the stack-resident descriptor-version operand,
/// RAX status, and the closed status table under the Microsoft-x64 policy;
/// the leg must replay exact and name the provider's sealed service row, the
/// borrowed pending exit must still retain its own exact leg, and every later
/// custody transition replays the same gate.
pub fn prepare_uefi_get_memory_map_invocation<'pending_exit, 'system_table, 'boot_services>(
    provider: LifecycleScopedUefiGetMemoryMapProvider<'pending_exit, 'system_table, 'boot_services>,
) -> Result<
    PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services>,
    Box<UefiGetMemoryMapInvocationPlanningError<'pending_exit, 'system_table, 'boot_services>>,
> {
    if !provider.pending_exit.retains_exact_plan() {
        return Err(Box::new(UefiGetMemoryMapInvocationPlanningError {
            provider,
            diagnostic: ExternalRootDiagnostic(
                "UEFI GetMemoryMap provider lost its exact pending exit plan before planning"
                    .into(),
            ),
        }));
    }
    let plan = match plan_uefi_os_handoff_invocation(TargetProfile::UefiX64) {
        Ok(plan) => plan,
        Err(error) => {
            return Err(Box::new(UefiGetMemoryMapInvocationPlanningError {
                provider,
                diagnostic: ExternalRootDiagnostic(format!(
                    "UEFI OS-handoff invocation plan rejected: {}",
                    error.diagnostic()
                )),
            }));
        }
    };
    let leg = plan.get_memory_map().clone();
    if !plan.matches_exact_uefi_x64_plan() || leg.service_field() != provider.field {
        return Err(Box::new(UefiGetMemoryMapInvocationPlanningError {
            provider,
            diagnostic: ExternalRootDiagnostic(
                "UEFI GetMemoryMap invocation plan drifted from its lifecycle provider".into(),
            ),
        }));
    }
    Ok(PlannedUefiGetMemoryMapInvocation {
        provider,
        plan: leg,
    })
}

/// Concrete, still-uninvoked operands for one exact map acquisition: the
/// private service address and the exclusively borrowed buffer/cell storage.
/// Public observations are limited to identities and ABI destinations.
#[must_use = "bound UEFI GetMemoryMap operands retain provider and buffer custody"]
pub struct BoundUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services, 'buffer> {
    pub(crate) invocation:
        PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services>,
    pub(super) buffer: &'buffer mut UefiMemoryMapBuffer,
    pub(super) _service: NonZeroU64,
}

impl std::fmt::Debug for BoundUefiGetMemoryMapInvocation<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BoundUefiGetMemoryMapInvocation")
            .field("physical_invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("service_identity", &self.service_identity())
            .field("buffer_capacity", &self.buffer.capacity())
            .field("argument_destinations", &self.argument_destinations())
            .finish_non_exhaustive()
    }
}

impl BoundUefiGetMemoryMapInvocation<'_, '_, '_, '_> {
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.invocation.physical_invocation()
    }
    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.invocation.image_handle_occurrence()
    }
    pub const fn service_identity(&self) -> &'static str {
        self.invocation.service_identity()
    }
    /// The four register destinations; the fifth operand occupies the first
    /// stack slot above the shadow area and has no register destination.
    pub const fn argument_destinations(&self) -> [MachineRegister; 4] {
        [
            MachineRegister::X86Rcx,
            MachineRegister::X86Rdx,
            MachineRegister::X86R8,
            MachineRegister::X86R9,
        ]
    }
    pub const fn calling_plan_report_fingerprint(&self) -> u64 {
        self.invocation.calling_plan_report_fingerprint()
    }
}

impl<'pending_exit, 'system_table, 'boot_services, 'buffer>
    BoundUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services, 'buffer>
{
    /// End acquisition custody before execution: the pending-exit borrow and
    /// the buffer return to the caller unchanged.
    pub fn into_pending_exit_invocation(
        self,
    ) -> (
        &'pending_exit PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services>,
        &'buffer mut UefiMemoryMapBuffer,
    ) {
        (self.invocation.provider.pending_exit, self.buffer)
    }
}

#[derive(Debug)]
#[must_use = "UEFI GetMemoryMap operand rejection retains provider and buffer custody"]
pub struct UefiGetMemoryMapInvocationBindingError<
    'pending_exit,
    'system_table,
    'boot_services,
    'buffer,
> {
    invocation: PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services>,
    buffer: &'buffer mut UefiMemoryMapBuffer,
    diagnostic: ExternalRootDiagnostic,
}

impl<'pending_exit, 'system_table, 'boot_services, 'buffer>
    UefiGetMemoryMapInvocationBindingError<'pending_exit, 'system_table, 'boot_services, 'buffer>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services>,
        &'buffer mut UefiMemoryMapBuffer,
        ExternalRootDiagnostic,
    ) {
        (self.invocation, self.buffer, self.diagnostic)
    }
}

impl std::fmt::Display for UefiGetMemoryMapInvocationBindingError<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiGetMemoryMapInvocationBindingError<'_, '_, '_, '_> {}

/// Bind one custody-scoped buffer to the retained five-operand plan. Binding
/// re-arms the call cells: the in-out size cell takes the buffer capacity and
/// every output cell clears, so outputs read back after execution can only
/// name this attempt. A drifted plan or service row rejects without consuming
/// either input.
pub fn bind_uefi_get_memory_map_invocation<
    'pending_exit,
    'system_table,
    'boot_services,
    'buffer,
>(
    invocation: PlannedUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services>,
    buffer: &'buffer mut UefiMemoryMapBuffer,
) -> Result<
    BoundUefiGetMemoryMapInvocation<'pending_exit, 'system_table, 'boot_services, 'buffer>,
    Box<
        UefiGetMemoryMapInvocationBindingError<
            'pending_exit,
            'system_table,
            'boot_services,
            'buffer,
        >,
    >,
> {
    let reject = |invocation, buffer, message: &'static str| {
        Err(Box::new(UefiGetMemoryMapInvocationBindingError {
            invocation,
            buffer,
            diagnostic: ExternalRootDiagnostic(message.into()),
        }))
    };
    if !invocation.retains_exact_plan() {
        return reject(
            invocation,
            buffer,
            "UEFI GetMemoryMap operand binding plan drifted from its lifecycle provider",
        );
    }
    // Re-arm the in-out and output cells so every post-call value is sealed
    // by this exact attempt; stale outputs from an earlier call cannot
    // masquerade as fresh acquisition evidence.
    buffer.map_size = buffer.map.len();
    buffer.map_key = 0;
    buffer.descriptor_size = 0;
    buffer.descriptor_version = 0;
    buffer.occupied = 0;
    Ok(BoundUefiGetMemoryMapInvocation {
        _service: invocation.provider.get_memory_map,
        invocation,
        buffer,
    })
}

pub(crate) type UefiGetMemoryMapFunction = unsafe extern "efiapi" fn(
    memory_map_size: *mut usize,
    memory_map: *mut c_void,
    map_key: *mut usize,
    descriptor_size: *mut usize,
    descriptor_version: *mut u32,
) -> u64;
