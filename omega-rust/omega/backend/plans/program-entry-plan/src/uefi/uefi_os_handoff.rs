//! Exact planning-only UEFI Boot Services OS-handoff invocation.
//!
//! The nonreturning custody transfer out of Boot Services is one bounded
//! `GetMemoryMap`/`ExitBootServices` cycle, not two independent calls: each
//! attempt acquires the freshest firmware map beneath live pending-exit
//! custody, binds the pending `ExitBootServices` invocation to that exact map
//! key, and applies the admitted result. A stale key retires the map and
//! returns custody for the next attempt; the last stale key resolves to the
//! target-authored exhaustion status; `EFI_SUCCESS` consumes Boot Services and
//! transfers control to the loaded OS entry without returning.
//!
//! This plan binds the two target-owned service-table rows that cycle drives,
//! their Microsoft-x64 argument and result placements, the per-leg closed
//! status catalog, and the custody role each status plays. It owns no runtime
//! pointer, provider occurrence, map buffer, custody ledger, call edge,
//! emitted bytes, root, or execution evidence: those live in the
//! `uefi_bootstrap` runtime edge this plan describes.
//!
//! The attempt limit and the exhaustion status are explicit deployment inputs,
//! not target policy: the runtime ledger admits them as a `NonZeroU32` bound
//! and a [`uefi_os_handoff_exhaustion_requires_error`]-satisfying status, so
//! they are retained by the ledger rather than by this canonical plan.

use calling_conventions::{
    BoundaryEntryPlan, CallSignature, CallingPolicy, EntryControl, MachineRegister, ValueLocation,
    ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use diagnostics::Diagnostic;
use target::{
    TargetProfile, UefiBootServicesNativeField, UefiBootServicesNativeFieldKind,
    UefiBootServicesNativeFieldLayout, plan_uefi_boot_services_native_layout,
};

pub const UEFI_GET_MEMORY_MAP_SERVICE_IDENTITY: &str = "EFI_BOOT_SERVICES.GetMemoryMap";
pub const UEFI_EXIT_BOOT_SERVICES_SERVICE_IDENTITY: &str = "EFI_BOOT_SERVICES.ExitBootServices";

pub const UEFI_OS_HANDOFF_MEMORY_MAP_SIZE_TYPE_IDENTITY: &str = "*mut UINTN";
pub const UEFI_OS_HANDOFF_MEMORY_MAP_TYPE_IDENTITY: &str = "*mut EFI_MEMORY_DESCRIPTOR";
pub const UEFI_OS_HANDOFF_MAP_KEY_POINTER_TYPE_IDENTITY: &str = "*mut UINTN";
pub const UEFI_OS_HANDOFF_DESCRIPTOR_SIZE_TYPE_IDENTITY: &str = "*mut UINTN";
pub const UEFI_OS_HANDOFF_DESCRIPTOR_VERSION_TYPE_IDENTITY: &str = "*mut UINT32";
pub const UEFI_OS_HANDOFF_IMAGE_HANDLE_TYPE_IDENTITY: &str = "EFI_HANDLE";
pub const UEFI_OS_HANDOFF_MAP_KEY_TYPE_IDENTITY: &str = "UINTN";
pub const UEFI_OS_HANDOFF_STATUS_TYPE_IDENTITY: &str = "EFI_STATUS";

const GET_MEMORY_MAP_FIELD_ORDINAL: u8 = 9;
const GET_MEMORY_MAP_FIELD_OFFSET: u32 = 56;
const EXIT_BOOT_SERVICES_FIELD_ORDINAL: u8 = 31;
const EXIT_BOOT_SERVICES_FIELD_OFFSET: u32 = 232;
const SERVICE_FIELD_BYTES: u32 = 8;
const SERVICE_FIELD_ALIGNMENT: u32 = 8;

const EFI_STATUS_ERROR_BIT: u64 = 1_u64 << 63;
const EFI_SUCCESS_CODE: u64 = 0;
const EFI_INVALID_PARAMETER_CODE: u64 = EFI_STATUS_ERROR_BIT | 2;
const EFI_BUFFER_TOO_SMALL_CODE: u64 = EFI_STATUS_ERROR_BIT | 5;

/// Whether a deployment-supplied exhaustion status is a target-authored UEFI
/// error. The bounded retry loop must never report success or a warning after
/// every attempt was spent on a stale key, so the status the invocation's
/// firmware return reports on exhaustion has to carry the UEFI error bit.
pub const fn uefi_os_handoff_exhaustion_requires_error(exhaustion_status: u64) -> bool {
    exhaustion_status & EFI_STATUS_ERROR_BIT != 0
}

/// The custody role one firmware status plays inside the bounded handoff
/// cycle. A status code outside a leg's closed row set is not admitted by that
/// leg: the cycle rejects it and returns the custody live at that stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UefiOsHandoffStatusRole {
    /// `GetMemoryMap` sealed the freshest snapshot/key pair into acquisition
    /// evidence; the cycle binds the pending exit to that exact key.
    MapAcquired = 1,
    /// `GetMemoryMap` reported the bound map buffer too small. The cycle grows
    /// it to the firmware-reported requirement and reacquires without spending
    /// a handoff attempt.
    GrowMapBuffer,
    /// `ExitBootServices` rejected the bound map key as stale. The spent bound
    /// operands are consumed but provider custody returns intact for the next
    /// acquisition, so one attempt is spent and the loop may reacquire.
    RetryStaleMapKey,
    /// `ExitBootServices` succeeded: boot-scoped services are consumed and
    /// control transfers to the loaded OS entry. This leg never returns.
    TransferNonReturning,
    /// The status is a member of the leg's closed table but carries no
    /// custody-advancing role; the cycle rejects it and returns the custody
    /// live at that stage.
    Reject,
}

/// One closed status-catalog row for a handoff leg: the exact firmware code
/// and the custody role it plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UefiOsHandoffStatusRow {
    role: UefiOsHandoffStatusRole,
    code: u64,
}

impl UefiOsHandoffStatusRow {
    pub const fn role(self) -> UefiOsHandoffStatusRole {
        self.role
    }
    pub const fn code(self) -> u64 {
        self.code
    }
}

const GET_MEMORY_MAP_STATUS_ROWS: [UefiOsHandoffStatusRow; 3] = [
    UefiOsHandoffStatusRow {
        role: UefiOsHandoffStatusRole::MapAcquired,
        code: EFI_SUCCESS_CODE,
    },
    UefiOsHandoffStatusRow {
        role: UefiOsHandoffStatusRole::GrowMapBuffer,
        code: EFI_BUFFER_TOO_SMALL_CODE,
    },
    UefiOsHandoffStatusRow {
        role: UefiOsHandoffStatusRole::Reject,
        code: EFI_INVALID_PARAMETER_CODE,
    },
];

const EXIT_BOOT_SERVICES_STATUS_ROWS: [UefiOsHandoffStatusRow; 2] = [
    UefiOsHandoffStatusRow {
        role: UefiOsHandoffStatusRole::TransferNonReturning,
        code: EFI_SUCCESS_CODE,
    },
    UefiOsHandoffStatusRow {
        role: UefiOsHandoffStatusRole::RetryStaleMapKey,
        code: EFI_INVALID_PARAMETER_CODE,
    },
];

/// Exact planning-only contract for one Boot Services call leg of the cycle.
/// The retained field, signature identities, evaluated Microsoft-x64 plan, and
/// closed status table let the generated custody transfer replay the leg
/// without re-deriving target knowledge.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "UEFI OS-handoff leg plan retains the exact target provider schema"]
pub struct UefiOsHandoffLegPlan {
    service_identity: &'static str,
    service_field: UefiBootServicesNativeFieldLayout,
    parameter_type_identities: &'static [&'static str],
    result_type_identity: &'static str,
    boundary_entry_plan: BoundaryEntryPlan,
    calling_plan_report_fingerprint: u64,
    calling_plan_commitment: [u8; 32],
    status_rows: &'static [UefiOsHandoffStatusRow],
}

impl UefiOsHandoffLegPlan {
    pub const fn service_identity(&self) -> &'static str {
        self.service_identity
    }
    pub const fn service_field(&self) -> UefiBootServicesNativeFieldLayout {
        self.service_field
    }
    pub const fn parameter_type_identities(&self) -> &'static [&'static str] {
        self.parameter_type_identities
    }
    pub const fn result_type_identity(&self) -> &'static str {
        self.result_type_identity
    }
    pub const fn boundary_entry_plan(&self) -> &BoundaryEntryPlan {
        &self.boundary_entry_plan
    }
    pub const fn calling_plan_report_fingerprint(&self) -> u64 {
        self.calling_plan_report_fingerprint
    }
    pub const fn calling_plan_commitment(&self) -> &[u8; 32] {
        &self.calling_plan_commitment
    }
    pub const fn status_rows(&self) -> &'static [UefiOsHandoffStatusRow] {
        self.status_rows
    }

    /// The custody role a firmware status code drives on this leg, or `None`
    /// when the code is outside the leg's closed table and must reject.
    pub fn status_role(&self, code: u64) -> Option<UefiOsHandoffStatusRole> {
        self.status_rows
            .iter()
            .find(|row| row.code == code)
            .map(|row| row.role)
    }

    /// Structural replay against the same service row of a freshly derived
    /// canonical target plan. A leg whose row is neither `GetMemoryMap` nor
    /// `ExitBootServices` has no canonical counterpart and never replays; the
    /// runtime edge additionally checks that the row is the one its provider
    /// sealed, so an exact leg of the other row still rejects there.
    pub fn matches_exact_uefi_x64_plan(&self) -> bool {
        plan_uefi_os_handoff_invocation(TargetProfile::UefiX64).is_ok_and(|expected| {
            match self.service_field.field() {
                UefiBootServicesNativeField::GetMemoryMap => self == &expected.get_memory_map,
                UefiBootServicesNativeField::ExitBootServices => {
                    self == &expected.exit_boot_services
                }
                _ => false,
            }
        })
    }
}

/// The complete bounded OS-handoff invocation contract. `get_memory_map` runs
/// first to seal the freshest map key; `exit_boot_services` is bound to that
/// exact key and, on `EFI_SUCCESS`, consumes Boot Services and transfers
/// control to the loaded OS entry without returning. The plan is canonical for
/// the target: the attempt limit and exhaustion status are deployment inputs
/// the runtime ledger admits separately, so this artifact binds the structural
/// contract they constrain rather than their values.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "UEFI OS-handoff invocation plan retains the exact bounded transfer contract"]
pub struct UefiOsHandoffInvocationPlan {
    profile: TargetProfile,
    get_memory_map: UefiOsHandoffLegPlan,
    exit_boot_services: UefiOsHandoffLegPlan,
}

impl UefiOsHandoffInvocationPlan {
    pub const fn profile(&self) -> TargetProfile {
        self.profile
    }
    /// The `GetMemoryMap` leg that seals the freshest map key under live
    /// pending-exit custody, growing the bound buffer on `GrowMapBuffer`
    /// without spending a handoff attempt.
    pub const fn get_memory_map(&self) -> &UefiOsHandoffLegPlan {
        &self.get_memory_map
    }
    /// The `ExitBootServices` leg bound to the just-acquired map key. Its
    /// `TransferNonReturning` row is the only custody-consuming outcome.
    pub const fn exit_boot_services(&self) -> &UefiOsHandoffLegPlan {
        &self.exit_boot_services
    }

    /// Structural replay against a freshly derived canonical target plan.
    pub fn matches_exact_uefi_x64_plan(&self) -> bool {
        plan_uefi_os_handoff_invocation(TargetProfile::UefiX64)
            .is_ok_and(|expected| self == &expected)
    }
}

#[derive(Debug)]
#[must_use = "UEFI OS-handoff plan rejection retains the requested profile"]
pub struct UefiOsHandoffInvocationPlanError {
    profile: TargetProfile,
    diagnostic: Diagnostic,
}
impl UefiOsHandoffInvocationPlanError {
    pub const fn profile(&self) -> TargetProfile {
        self.profile
    }
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
    pub fn into_parts(self) -> (TargetProfile, Diagnostic) {
        (self.profile, self.diagnostic)
    }
}
impl std::fmt::Display for UefiOsHandoffInvocationPlanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}
impl std::error::Error for UefiOsHandoffInvocationPlanError {}

/// Derive and validate the canonical bounded OS-handoff invocation plan for
/// `profile`. The plan is exact for `TargetProfile::UefiX64` only; every other
/// profile rejects before any service row is bound.
pub fn plan_uefi_os_handoff_invocation(
    profile: TargetProfile,
) -> Result<UefiOsHandoffInvocationPlan, Box<UefiOsHandoffInvocationPlanError>> {
    let candidate = derive_candidate(profile).map_err(|diagnostic| {
        Box::new(UefiOsHandoffInvocationPlanError {
            profile,
            diagnostic,
        })
    })?;
    validate_candidate(&candidate).map_err(|diagnostic| {
        Box::new(UefiOsHandoffInvocationPlanError {
            profile,
            diagnostic,
        })
    })?;
    Ok(candidate)
}

fn derive_candidate(profile: TargetProfile) -> Result<UefiOsHandoffInvocationPlan, Diagnostic> {
    let layout =
        plan_uefi_boot_services_native_layout(profile).map_err(|error| error.into_parts().1)?;
    let get_memory_map_field = layout
        .field_layout(UefiBootServicesNativeField::GetMemoryMap)
        .ok_or_else(|| Diagnostic::error("UEFI Boot Services layout has no GetMemoryMap field"))?;
    let exit_boot_services_field = layout
        .field_layout(UefiBootServicesNativeField::ExitBootServices)
        .ok_or_else(|| {
            Diagnostic::error("UEFI Boot Services layout has no ExitBootServices field")
        })?;
    Ok(UefiOsHandoffInvocationPlan {
        profile,
        get_memory_map: derive_leg(
            UEFI_GET_MEMORY_MAP_SERVICE_IDENTITY,
            get_memory_map_field,
            &GET_MEMORY_MAP_PARAMETER_TYPE_IDENTITIES,
            GET_MEMORY_MAP_STATUS_ROWS.as_slice(),
            5,
        )?,
        exit_boot_services: derive_leg(
            UEFI_EXIT_BOOT_SERVICES_SERVICE_IDENTITY,
            exit_boot_services_field,
            &EXIT_BOOT_SERVICES_PARAMETER_TYPE_IDENTITIES,
            EXIT_BOOT_SERVICES_STATUS_ROWS.as_slice(),
            2,
        )?,
    })
}

static GET_MEMORY_MAP_PARAMETER_TYPE_IDENTITIES: [&str; 5] = [
    UEFI_OS_HANDOFF_MEMORY_MAP_SIZE_TYPE_IDENTITY,
    UEFI_OS_HANDOFF_MEMORY_MAP_TYPE_IDENTITY,
    UEFI_OS_HANDOFF_MAP_KEY_POINTER_TYPE_IDENTITY,
    UEFI_OS_HANDOFF_DESCRIPTOR_SIZE_TYPE_IDENTITY,
    UEFI_OS_HANDOFF_DESCRIPTOR_VERSION_TYPE_IDENTITY,
];

static EXIT_BOOT_SERVICES_PARAMETER_TYPE_IDENTITIES: [&str; 2] = [
    UEFI_OS_HANDOFF_IMAGE_HANDLE_TYPE_IDENTITY,
    UEFI_OS_HANDOFF_MAP_KEY_TYPE_IDENTITY,
];

fn derive_leg(
    service_identity: &'static str,
    service_field: UefiBootServicesNativeFieldLayout,
    parameter_type_identities: &'static [&'static str],
    status_rows: &'static [UefiOsHandoffStatusRow],
    parameter_count: usize,
) -> Result<UefiOsHandoffLegPlan, Diagnostic> {
    let word = ValueShape::integer(8, 8);
    let validated = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![word; parameter_count],
            result: Some(word),
        },
    )
    .map_err(|error| {
        Diagnostic::error(format!("{service_identity} calling plan rejected: {error}"))
    })?;
    Ok(UefiOsHandoffLegPlan {
        service_identity,
        service_field,
        parameter_type_identities,
        result_type_identity: UEFI_OS_HANDOFF_STATUS_TYPE_IDENTITY,
        boundary_entry_plan: validated.plan().clone(),
        calling_plan_report_fingerprint: validated.contract_report_fingerprint(),
        calling_plan_commitment: validated.contract_commitment_digest(),
        status_rows,
    })
}

fn validate_candidate(plan: &UefiOsHandoffInvocationPlan) -> Result<(), Diagnostic> {
    require(
        plan.profile == TargetProfile::UefiX64,
        "UEFI OS-handoff invocation belongs only to UEFI x64",
    )?;
    validate_leg(
        &plan.get_memory_map,
        UEFI_GET_MEMORY_MAP_SERVICE_IDENTITY,
        UefiBootServicesNativeField::GetMemoryMap,
        GET_MEMORY_MAP_FIELD_ORDINAL,
        GET_MEMORY_MAP_FIELD_OFFSET,
        &GET_MEMORY_MAP_PARAMETER_TYPE_IDENTITIES,
        &GET_MEMORY_MAP_STATUS_ROWS,
        &[
            MachineRegister::X86Rcx,
            MachineRegister::X86Rdx,
            MachineRegister::X86R8,
            MachineRegister::X86R9,
        ],
        Some(32),
    )?;
    validate_leg(
        &plan.exit_boot_services,
        UEFI_EXIT_BOOT_SERVICES_SERVICE_IDENTITY,
        UefiBootServicesNativeField::ExitBootServices,
        EXIT_BOOT_SERVICES_FIELD_ORDINAL,
        EXIT_BOOT_SERVICES_FIELD_OFFSET,
        &EXIT_BOOT_SERVICES_PARAMETER_TYPE_IDENTITIES,
        &EXIT_BOOT_SERVICES_STATUS_ROWS,
        &[MachineRegister::X86Rcx, MachineRegister::X86Rdx],
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_leg(
    leg: &UefiOsHandoffLegPlan,
    service_identity: &'static str,
    field: UefiBootServicesNativeField,
    ordinal: u8,
    byte_offset: u32,
    parameter_type_identities: &[&'static str],
    status_rows: &[UefiOsHandoffStatusRow],
    expected_registers: &[MachineRegister],
    stack_parameter_offset: Option<u32>,
) -> Result<(), Diagnostic> {
    require(
        leg.service_identity == service_identity,
        "UEFI OS-handoff leg service identity drifted",
    )?;
    require(
        (
            leg.service_field.field(),
            leg.service_field.ordinal(),
            leg.service_field.byte_offset(),
            leg.service_field.byte_size(),
            leg.service_field.alignment(),
            leg.service_field.kind(),
        ) == (
            field,
            ordinal,
            byte_offset,
            SERVICE_FIELD_BYTES,
            SERVICE_FIELD_ALIGNMENT,
            UefiBootServicesNativeFieldKind::FunctionPointer,
        ),
        "UEFI OS-handoff leg service-table row drifted",
    )?;
    require(
        leg.parameter_type_identities == parameter_type_identities
            && leg.result_type_identity == UEFI_OS_HANDOFF_STATUS_TYPE_IDENTITY,
        "UEFI OS-handoff leg signature drifted",
    )?;
    require(
        leg.status_rows == status_rows,
        "UEFI OS-handoff leg closed status table drifted",
    )?;
    let word = ValueShape::integer(8, 8);
    let expected = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![
                word;
                expected_registers.len()
                    + usize::from(stack_parameter_offset.is_some())
            ],
            result: Some(word),
        },
    )
    .map_err(|error| {
        Diagnostic::error(format!("{service_identity} calling plan rejected: {error}"))
    })?;
    require(
        leg.boundary_entry_plan == *expected.plan()
            && leg.calling_plan_report_fingerprint == expected.contract_report_fingerprint()
            && leg.calling_plan_commitment == expected.contract_commitment_digest(),
        "UEFI OS-handoff leg calling plan drifted",
    )?;
    validate_exact_call_shape(leg, expected_registers, stack_parameter_offset)
}

fn validate_exact_call_shape(
    leg: &UefiOsHandoffLegPlan,
    expected_registers: &[MachineRegister],
    stack_parameter_offset: Option<u32>,
) -> Result<(), Diagnostic> {
    let call = &leg.boundary_entry_plan.call;
    require(
        call.policy == CallingPolicy::MicrosoftX64
            && call.parameters.len()
                == expected_registers.len() + usize::from(stack_parameter_offset.is_some())
            && call.shadow_bytes == 32
            && call.stack_alignment == 16
            && call.entry_control == EntryControl::CallReturn,
        "UEFI OS-handoff leg call frame drifted",
    )?;
    for (placement, expected_register) in call.parameters.iter().zip(expected_registers) {
        require(
            matches!(placement.locations.as_slice(), [ValueLocation::Register { register, value_byte_offset: 0, byte_size: 8 }] if *register == *expected_register),
            "UEFI OS-handoff leg input register placement drifted",
        )?;
    }
    if let Some(stack_offset) = stack_parameter_offset {
        let last = call
            .parameters
            .last()
            .expect("a stacked operand implies a nonempty parameter list");
        require(
            matches!(
                last.locations.as_slice(),
                [ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset: 0,
                    byte_size: 8,
                    alignment: 8,
                }] if *stack_byte_offset == stack_offset
            ),
            "UEFI OS-handoff leg stacked operand missed its slot",
        )?;
    }
    require(
        matches!(
            call.result
                .as_ref()
                .map(|placement| placement.locations.as_slice()),
            Some([ValueLocation::Register {
                register: MachineRegister::X86Rax,
                value_byte_offset: 0,
                byte_size: 8
            }])
        ),
        "UEFI OS-handoff leg result register placement drifted",
    )
}

fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
}

#[cfg(test)]
mod tests {
    use super::{
        EFI_BUFFER_TOO_SMALL_CODE, EFI_INVALID_PARAMETER_CODE, EFI_STATUS_ERROR_BIT,
        EFI_SUCCESS_CODE, TargetProfile, UefiBootServicesNativeField, UefiOsHandoffInvocationPlan,
        UefiOsHandoffStatusRole, plan_uefi_boot_services_native_layout,
        plan_uefi_os_handoff_invocation, uefi_os_handoff_exhaustion_requires_error,
        validate_candidate,
    };

    fn exact_plan() -> UefiOsHandoffInvocationPlan {
        plan_uefi_os_handoff_invocation(TargetProfile::UefiX64).unwrap()
    }

    #[test]
    fn exact_plan_binds_both_service_rows_abi_and_status_roles() {
        let plan = exact_plan();
        assert!(plan.matches_exact_uefi_x64_plan());
        assert_eq!(plan.profile(), TargetProfile::UefiX64);

        let map = plan.get_memory_map();
        assert!(map.matches_exact_uefi_x64_plan());
        assert_eq!(map.service_identity(), "EFI_BOOT_SERVICES.GetMemoryMap");
        assert_eq!(map.service_field().byte_offset(), 56);
        assert_eq!(map.service_field().ordinal(), 9);
        assert_eq!(map.parameter_type_identities().len(), 5);
        assert_eq!(
            map.status_role(EFI_SUCCESS_CODE),
            Some(UefiOsHandoffStatusRole::MapAcquired)
        );
        assert_eq!(
            map.status_role(EFI_BUFFER_TOO_SMALL_CODE),
            Some(UefiOsHandoffStatusRole::GrowMapBuffer)
        );
        assert_eq!(
            map.status_role(EFI_INVALID_PARAMETER_CODE),
            Some(UefiOsHandoffStatusRole::Reject)
        );

        let exit = plan.exit_boot_services();
        assert!(exit.matches_exact_uefi_x64_plan());
        assert_eq!(
            exit.service_identity(),
            "EFI_BOOT_SERVICES.ExitBootServices"
        );
        assert_eq!(exit.service_field().byte_offset(), 232);
        assert_eq!(exit.service_field().ordinal(), 31);
        assert_eq!(exit.parameter_type_identities().len(), 2);
        assert_eq!(
            exit.status_role(EFI_SUCCESS_CODE),
            Some(UefiOsHandoffStatusRole::TransferNonReturning)
        );
        assert_eq!(
            exit.status_role(EFI_INVALID_PARAMETER_CODE),
            Some(UefiOsHandoffStatusRole::RetryStaleMapKey)
        );

        assert_ne!(map.calling_plan_report_fingerprint(), 0);
        assert_ne!(map.calling_plan_commitment(), &[0; 32]);
        assert_ne!(exit.calling_plan_report_fingerprint(), 0);
        assert_ne!(exit.calling_plan_commitment(), &[0; 32]);
    }

    #[test]
    fn non_uefi_profile_and_unclassified_statuses_reject() {
        assert!(plan_uefi_os_handoff_invocation(TargetProfile::WindowsX64).is_err());
        // A code outside a leg's closed table has no custody role and rejects.
        let plan = exact_plan();
        assert_eq!(plan.get_memory_map().status_role(0x1234), None);
        assert_eq!(plan.exit_boot_services().status_role(0x1234), None);
        // The exhaustion status the deployment supplies must be a target error.
        assert!(!uefi_os_handoff_exhaustion_requires_error(EFI_SUCCESS_CODE));
        assert!(!uefi_os_handoff_exhaustion_requires_error(1));
        assert!(uefi_os_handoff_exhaustion_requires_error(
            EFI_STATUS_ERROR_BIT | 0x8000_0001
        ));
    }

    #[test]
    fn every_leg_axis_rejects_on_drift() {
        let exact = exact_plan();
        let mutations: Vec<Box<dyn Fn(&mut UefiOsHandoffInvocationPlan)>> = vec![
            Box::new(|plan| {
                plan.get_memory_map.service_identity = "EFI_BOOT_SERVICES.OpenProtocol"
            }),
            Box::new(|plan| {
                plan.get_memory_map.service_field =
                    plan_uefi_boot_services_native_layout(TargetProfile::UefiX64)
                        .unwrap()
                        .field_layout(UefiBootServicesNativeField::OpenProtocol)
                        .unwrap()
            }),
            Box::new(|plan| {
                plan.exit_boot_services.service_field =
                    plan_uefi_boot_services_native_layout(TargetProfile::UefiX64)
                        .unwrap()
                        .field_layout(UefiBootServicesNativeField::HandleProtocol)
                        .unwrap()
            }),
            Box::new(|plan| {
                plan.exit_boot_services
                    .boundary_entry_plan
                    .call
                    .parameters
                    .swap(0, 1)
            }),
            Box::new(|plan| plan.get_memory_map.calling_plan_report_fingerprint ^= 1),
            Box::new(|plan| plan.exit_boot_services.calling_plan_commitment[0] ^= 1),
            Box::new(|plan| plan.get_memory_map.status_rows = &[]),
        ];
        for (index, mutate) in mutations.into_iter().enumerate() {
            let mut candidate = exact.clone();
            mutate(&mut candidate);
            assert!(
                validate_candidate(&candidate).is_err(),
                "mutation {index} was not rejected by validate_candidate"
            );
            assert!(
                !candidate.matches_exact_uefi_x64_plan(),
                "mutation {index} still replayed as exact"
            );
            assert!(
                !candidate.get_memory_map().matches_exact_uefi_x64_plan()
                    || !candidate.exit_boot_services().matches_exact_uefi_x64_plan(),
                "mutation {index} left both legs replaying as exact"
            );
        }
    }
}
