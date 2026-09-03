//! Exact target-runtime execution of the retained UEFI `HandleProtocol` call.
//!
//! The unsafe edge is deliberately narrow: it consumes the non-clone bound
//! operand carrier, invokes only its private service address through the UEFI
//! ABI, and seals the returned status and exact output slot. A separate safe
//! transition derives Loaded Image geometry through the target-owned native
//! layout. Neither carrier exposes the returned interface pointer.

use std::ffi::c_void;

use omega_program_entry_plan::UefiHandleProtocolStatus;
use omega_target::{
    TargetProfile, UEFI_LOADED_IMAGE_PROTOCOL_GUID, UefiProtocolGuid,
    plan_uefi_loaded_image_native_layout, validate_uefi_loaded_image_occurrence,
};

use super::{BoundUefiHandleProtocolInvocation, LifecycleScopedUefiLoadedImageCorrespondence};
use crate::{ExternalRootDiagnostic, UefiImageHandleOccurrenceId, UefiPhysicalInvocationId};

type UefiHandleProtocolFunction = unsafe extern "efiapi" fn(
    handle: *mut c_void,
    protocol: *const UefiProtocolGuid,
    interface: *mut *mut c_void,
) -> u64;

/// Opaque pointer-sized storage passed as the exact `VOID **` output operand.
/// Callers can construct only an empty slot and can observe only whether an
/// execution populated it; the returned address remains private.
#[repr(transparent)]
#[must_use = "UEFI HandleProtocol output storage remains borrowed through provider execution"]
pub struct UefiHandleProtocolInterfaceOutputSlot(pub(super) *mut c_void);

impl UefiHandleProtocolInterfaceOutputSlot {
    pub const fn empty() -> Self {
        Self(std::ptr::null_mut())
    }

    pub const fn is_empty(&self) -> bool {
        self.0.is_null()
    }
}

impl Default for UefiHandleProtocolInterfaceOutputSlot {
    fn default() -> Self {
        Self::empty()
    }
}

impl std::fmt::Debug for UefiHandleProtocolInterfaceOutputSlot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiHandleProtocolInterfaceOutputSlot")
            .field("is_empty", &self.is_empty())
            .finish_non_exhaustive()
    }
}

/// Closed interpretation of the exact returned `EFI_STATUS`. Unknown values
/// remain observable as unsupported execution results but cannot establish
/// Loaded Image correspondence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UefiHandleProtocolExecutionStatus {
    Success,
    InvalidParameter,
    Unsupported,
    Unknown,
}

/// Sealed evidence that the exact retained service was invoked once with its
/// retained operands. The raw output value remains private.
#[must_use = "executed UEFI HandleProtocol custody must be admitted or released"]
pub struct ExecutedUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output> {
    pub(super) invocation:
        BoundUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output>,
    status_code: u64,
    interface_output: *mut c_void,
}

impl std::fmt::Debug for ExecutedUefiHandleProtocolInvocation<'_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExecutedUefiHandleProtocolInvocation")
            .field("physical_invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("status_code", &self.status_code())
            .field("status", &self.status())
            .field("interface_output_is_null", &self.interface_output_is_null())
            .finish_non_exhaustive()
    }
}

impl ExecutedUefiHandleProtocolInvocation<'_, '_, '_> {
    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.invocation.physical_invocation()
    }

    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.invocation.image_handle_occurrence()
    }

    pub const fn status_code(&self) -> u64 {
        self.status_code
    }

    pub fn status(&self) -> UefiHandleProtocolExecutionStatus {
        let plan = &self.invocation.invocation.plan;
        if self.status_code == plan.status_code(UefiHandleProtocolStatus::Success) {
            UefiHandleProtocolExecutionStatus::Success
        } else if self.status_code == plan.status_code(UefiHandleProtocolStatus::InvalidParameter) {
            UefiHandleProtocolExecutionStatus::InvalidParameter
        } else if self.status_code == plan.status_code(UefiHandleProtocolStatus::Unsupported) {
            UefiHandleProtocolExecutionStatus::Unsupported
        } else {
            UefiHandleProtocolExecutionStatus::Unknown
        }
    }

    pub const fn interface_output_is_null(&self) -> bool {
        self.interface_output.is_null()
    }
}

#[derive(Debug)]
#[must_use = "UEFI HandleProtocol execution rejection retains bound provider custody"]
pub struct UefiHandleProtocolExecutionError<'system_table, 'boot_services, 'output> {
    invocation: BoundUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'system_table, 'boot_services, 'output>
    UefiHandleProtocolExecutionError<'system_table, 'boot_services, 'output>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        BoundUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output>,
        ExternalRootDiagnostic,
    ) {
        (self.invocation, self.diagnostic)
    }
}

impl std::fmt::Display for UefiHandleProtocolExecutionError<'_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiHandleProtocolExecutionError<'_, '_, '_> {}

/// Invoke the exact retained UEFI `HandleProtocol` service once.
///
/// # Safety
///
/// The target-runtime caller must establish that the retained numeric service
/// and handle values came from the live UEFI physical invocation and remain
/// callable/readable for this operation. If the service returns success, it
/// must initialize the retained output slot with a readable
/// `EFI_LOADED_IMAGE_PROTOCOL` occurrence of at least the exact target-owned
/// prefix size, valid until the returned execution carrier is consumed or
/// released. This is the sole host-language unsafe premise; the resulting
/// receipt cannot be constructed directly.
pub unsafe fn execute_uefi_loaded_image_handle_protocol<'system_table, 'boot_services, 'output>(
    invocation: BoundUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output>,
) -> Result<
    ExecutedUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output>,
    Box<UefiHandleProtocolExecutionError<'system_table, 'boot_services, 'output>>,
> {
    if !invocation.invocation.plan.matches_exact_uefi_x64_plan()
        || invocation.protocol != &UEFI_LOADED_IMAGE_PROTOCOL_GUID
        || !invocation.interface_output.is_empty()
    {
        return Err(Box::new(UefiHandleProtocolExecutionError {
            invocation,
            diagnostic: ExternalRootDiagnostic(
                "UEFI HandleProtocol execution operands drifted after binding".into(),
            ),
        }));
    }
    let Ok(service_address) = usize::try_from(invocation._service.get()) else {
        return Err(Box::new(UefiHandleProtocolExecutionError {
            invocation,
            diagnostic: ExternalRootDiagnostic(
                "UEFI HandleProtocol service address does not fit the runtime pointer carrier"
                    .into(),
            ),
        }));
    };
    let Ok(handle_address) = usize::try_from(invocation._handle.get()) else {
        return Err(Box::new(UefiHandleProtocolExecutionError {
            invocation,
            diagnostic: ExternalRootDiagnostic(
                "UEFI image handle does not fit the runtime pointer carrier".into(),
            ),
        }));
    };
    // SAFETY: the function-pointer validity and exact EFI ABI are the explicit
    // caller obligations above. The bound carrier owns the only route to these
    // private numeric operands and retains the output borrow across the call.
    let service: UefiHandleProtocolFunction = unsafe { std::mem::transmute(service_address) };
    // SAFETY: upheld by the same target-runtime contract. The output address is
    // the exact opaque slot borrowed into the bound invocation.
    let status_code = unsafe {
        service(
            handle_address as *mut c_void,
            invocation.protocol,
            &mut invocation.interface_output.0,
        )
    };
    let interface_output = invocation.interface_output.0;
    Ok(ExecutedUefiHandleProtocolInvocation {
        invocation,
        status_code,
        interface_output,
    })
}

#[derive(Debug)]
#[must_use = "UEFI HandleProtocol result rejection retains exact execution custody"]
pub struct UefiHandleProtocolLoadedImageCallError<'system_table, 'boot_services, 'output> {
    execution: ExecutedUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'system_table, 'boot_services, 'output>
    UefiHandleProtocolLoadedImageCallError<'system_table, 'boot_services, 'output>
{
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub const fn status_code(&self) -> u64 {
        self.execution.status_code()
    }

    pub fn status(&self) -> UefiHandleProtocolExecutionStatus {
        self.execution.status()
    }

    pub fn into_parts(
        self,
    ) -> (
        ExecutedUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output>,
        ExternalRootDiagnostic,
    ) {
        (self.execution, self.diagnostic)
    }
}

impl std::fmt::Display for UefiHandleProtocolLoadedImageCallError<'_, '_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiHandleProtocolLoadedImageCallError<'_, '_, '_> {}

/// Consume one exact execution receipt and derive Loaded Image geometry only
/// through the target-owned native layout. Caller-authored status or geometry
/// values cannot enter this transition.
pub fn admit_uefi_loaded_image_handle_protocol_execution<'system_table, 'boot_services, 'output>(
    execution: ExecutedUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output>,
) -> Result<
    LifecycleScopedUefiLoadedImageCorrespondence<'system_table, 'boot_services, 'output>,
    Box<UefiHandleProtocolLoadedImageCallError<'system_table, 'boot_services, 'output>>,
> {
    if execution.invocation.interface_output.0 != execution.interface_output {
        return reject(
            execution,
            "UEFI HandleProtocol output slot drifted after provider execution",
        );
    }
    match execution.status() {
        UefiHandleProtocolExecutionStatus::Success => {}
        UefiHandleProtocolExecutionStatus::InvalidParameter => {
            return reject(execution, "UEFI HandleProtocol returned InvalidParameter");
        }
        UefiHandleProtocolExecutionStatus::Unsupported => {
            return reject(execution, "UEFI HandleProtocol returned Unsupported");
        }
        UefiHandleProtocolExecutionStatus::Unknown => {
            return reject(
                execution,
                "UEFI HandleProtocol returned a status outside its closed target table",
            );
        }
    }
    if execution.interface_output.is_null() {
        return reject(
            execution,
            "UEFI HandleProtocol success returned a null Loaded Image interface",
        );
    }
    let layout = match plan_uefi_loaded_image_native_layout(TargetProfile::UefiX64) {
        Ok(layout) => layout,
        Err(error) => {
            return reject(
                execution,
                format!(
                    "UEFI Loaded Image target layout rejected: {}",
                    error.diagnostic()
                ),
            );
        }
    };
    if !(execution.interface_output as usize).is_multiple_of(layout.alignment() as usize) {
        return reject(
            execution,
            "UEFI Loaded Image interface is not aligned for its exact target layout",
        );
    }
    // SAFETY: only the unsafe execution edge can construct `execution`; its
    // success contract requires this exact readable prefix to remain live
    // until the receipt is consumed or released.
    let bytes = unsafe {
        std::slice::from_raw_parts(
            execution.interface_output.cast::<u8>(),
            layout.byte_size() as usize,
        )
    };
    let geometry = match validate_uefi_loaded_image_occurrence(layout, bytes) {
        Ok(geometry) => geometry,
        Err(error) => {
            return reject(
                execution,
                format!(
                    "UEFI Loaded Image occurrence rejected: {}",
                    error.diagnostic()
                ),
            );
        }
    };
    Ok(LifecycleScopedUefiLoadedImageCorrespondence {
        execution,
        geometry,
    })
}

fn reject<'system_table, 'boot_services, 'output>(
    execution: ExecutedUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output>,
    message: impl Into<String>,
) -> Result<
    LifecycleScopedUefiLoadedImageCorrespondence<'system_table, 'boot_services, 'output>,
    Box<UefiHandleProtocolLoadedImageCallError<'system_table, 'boot_services, 'output>>,
> {
    Err(Box::new(UefiHandleProtocolLoadedImageCallError {
        execution,
        diagnostic: ExternalRootDiagnostic(message.into()),
    }))
}
