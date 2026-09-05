//! Exact planning-only UEFI `EFI_BOOT_SERVICES.HandleProtocol` invocation.
//!
//! The plan binds the target-owned service-table row, Loaded Image GUID,
//! Microsoft-x64 argument/result placements, and closed status table. It owns
//! no runtime pointer, provider occurrence, call edge, emitted bytes, root, or
//! execution evidence.

use calling_conventions::{
    BoundaryEntryPlan, CallSignature, CallingPolicy, MachineRegister, ValueLocation, ValueShape,
    evaluate_ordinary_boundary_entry_plan,
};
use diagnostics::Diagnostic;
use target::{
    TargetProfile, UEFI_LOADED_IMAGE_PROTOCOL_GUID, UefiBootServicesNativeField,
    UefiBootServicesNativeFieldKind, UefiBootServicesNativeFieldLayout, UefiProtocolGuid,
    plan_uefi_boot_services_native_layout,
};

pub const UEFI_HANDLE_PROTOCOL_SERVICE_IDENTITY: &str = "EFI_BOOT_SERVICES.HandleProtocol";
pub const UEFI_HANDLE_PROTOCOL_HANDLE_TYPE_IDENTITY: &str = "EFI_HANDLE";
pub const UEFI_HANDLE_PROTOCOL_GUID_POINTER_TYPE_IDENTITY: &str = "*const EFI_GUID";
pub const UEFI_HANDLE_PROTOCOL_INTERFACE_OUT_TYPE_IDENTITY: &str = "*mut *mut void";
pub const UEFI_HANDLE_PROTOCOL_STATUS_TYPE_IDENTITY: &str = "EFI_STATUS";

const HANDLE_PROTOCOL_FIELD_ORDINAL: u8 = 21;
const HANDLE_PROTOCOL_FIELD_OFFSET: u32 = 152;
const STATUS_ERROR_BIT: u64 = 1_u64 << 63;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UefiHandleProtocolStatus {
    Success = 1,
    InvalidParameter = 2,
    Unsupported = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UefiHandleProtocolStatusRow {
    status: UefiHandleProtocolStatus,
    code: u64,
}

impl UefiHandleProtocolStatusRow {
    pub const fn status(self) -> UefiHandleProtocolStatus {
        self.status
    }
    pub const fn code(self) -> u64 {
        self.code
    }
}

const STATUS_ROWS: [UefiHandleProtocolStatusRow; 3] = [
    UefiHandleProtocolStatusRow {
        status: UefiHandleProtocolStatus::Success,
        code: 0,
    },
    UefiHandleProtocolStatusRow {
        status: UefiHandleProtocolStatus::InvalidParameter,
        code: STATUS_ERROR_BIT | 2,
    },
    UefiHandleProtocolStatusRow {
        status: UefiHandleProtocolStatus::Unsupported,
        code: STATUS_ERROR_BIT | 3,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "UEFI HandleProtocol invocation plan retains exact target provider schema"]
pub struct UefiHandleProtocolInvocationPlan {
    profile: TargetProfile,
    service_identity: &'static str,
    service_field: UefiBootServicesNativeFieldLayout,
    protocol: UefiProtocolGuid,
    parameter_type_identities: [&'static str; 3],
    result_type_identity: &'static str,
    boundary_entry_plan: BoundaryEntryPlan,
    calling_plan_report_fingerprint: u64,
    calling_plan_commitment: [u8; 32],
    status_rows: [UefiHandleProtocolStatusRow; 3],
}

impl UefiHandleProtocolInvocationPlan {
    pub const fn profile(&self) -> TargetProfile {
        self.profile
    }
    pub const fn service_identity(&self) -> &'static str {
        self.service_identity
    }
    pub const fn service_field(&self) -> UefiBootServicesNativeFieldLayout {
        self.service_field
    }
    pub const fn protocol(&self) -> UefiProtocolGuid {
        self.protocol
    }
    pub const fn parameter_type_identities(&self) -> &[&'static str; 3] {
        &self.parameter_type_identities
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
    pub const fn status_rows(&self) -> &[UefiHandleProtocolStatusRow; 3] {
        &self.status_rows
    }

    pub fn status_code(&self, status: UefiHandleProtocolStatus) -> u64 {
        self.status_rows
            .iter()
            .find(|row| row.status == status)
            .expect("closed status catalog")
            .code
    }

    /// Structural replay against a freshly derived closed target plan.
    pub fn matches_exact_uefi_x64_plan(&self) -> bool {
        plan_uefi_handle_protocol_invocation(TargetProfile::UefiX64)
            .is_ok_and(|expected| self == &expected)
    }
}

#[derive(Debug)]
#[must_use = "UEFI HandleProtocol plan rejection retains requested profile"]
pub struct UefiHandleProtocolInvocationPlanError {
    profile: TargetProfile,
    diagnostic: Diagnostic,
}
impl UefiHandleProtocolInvocationPlanError {
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
impl std::fmt::Display for UefiHandleProtocolInvocationPlanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}
impl std::error::Error for UefiHandleProtocolInvocationPlanError {}

pub fn plan_uefi_handle_protocol_invocation(
    profile: TargetProfile,
) -> Result<UefiHandleProtocolInvocationPlan, Box<UefiHandleProtocolInvocationPlanError>> {
    let candidate = derive_candidate(profile).map_err(|diagnostic| {
        Box::new(UefiHandleProtocolInvocationPlanError {
            profile,
            diagnostic,
        })
    })?;
    validate_candidate(&candidate).map_err(|diagnostic| {
        Box::new(UefiHandleProtocolInvocationPlanError {
            profile,
            diagnostic,
        })
    })?;
    Ok(candidate)
}

fn derive_candidate(
    profile: TargetProfile,
) -> Result<UefiHandleProtocolInvocationPlan, Diagnostic> {
    let layout =
        plan_uefi_boot_services_native_layout(profile).map_err(|error| error.into_parts().1)?;
    let service_field = layout
        .field_layout(UefiBootServicesNativeField::HandleProtocol)
        .ok_or_else(|| {
            Diagnostic::error("UEFI Boot Services layout has no HandleProtocol field")
        })?;
    let word = ValueShape::integer(8, 8);
    let validated = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![word, word, word],
            result: Some(word),
        },
    )
    .map_err(|error| {
        Diagnostic::error(format!(
            "UEFI HandleProtocol calling plan rejected: {error}"
        ))
    })?;
    Ok(UefiHandleProtocolInvocationPlan {
        profile,
        service_identity: UEFI_HANDLE_PROTOCOL_SERVICE_IDENTITY,
        service_field,
        protocol: UEFI_LOADED_IMAGE_PROTOCOL_GUID,
        parameter_type_identities: [
            UEFI_HANDLE_PROTOCOL_HANDLE_TYPE_IDENTITY,
            UEFI_HANDLE_PROTOCOL_GUID_POINTER_TYPE_IDENTITY,
            UEFI_HANDLE_PROTOCOL_INTERFACE_OUT_TYPE_IDENTITY,
        ],
        result_type_identity: UEFI_HANDLE_PROTOCOL_STATUS_TYPE_IDENTITY,
        boundary_entry_plan: validated.plan().clone(),
        calling_plan_report_fingerprint: validated.contract_report_fingerprint(),
        calling_plan_commitment: validated.contract_commitment_digest(),
        status_rows: STATUS_ROWS,
    })
}

fn validate_candidate(plan: &UefiHandleProtocolInvocationPlan) -> Result<(), Diagnostic> {
    require(
        plan.profile == TargetProfile::UefiX64,
        "HandleProtocol invocation belongs only to UEFI x64",
    )?;
    let expected = derive_candidate(TargetProfile::UefiX64)?;
    require(
        plan.service_identity == UEFI_HANDLE_PROTOCOL_SERVICE_IDENTITY,
        "HandleProtocol invocation service identity drifted",
    )?;
    require(
        (
            plan.service_field.field(),
            plan.service_field.ordinal(),
            plan.service_field.byte_offset(),
            plan.service_field.byte_size(),
            plan.service_field.alignment(),
            plan.service_field.kind(),
        ) == (
            UefiBootServicesNativeField::HandleProtocol,
            HANDLE_PROTOCOL_FIELD_ORDINAL,
            HANDLE_PROTOCOL_FIELD_OFFSET,
            8,
            8,
            UefiBootServicesNativeFieldKind::FunctionPointer,
        ),
        "HandleProtocol invocation service-table row drifted",
    )?;
    require(
        plan.protocol == UEFI_LOADED_IMAGE_PROTOCOL_GUID,
        "HandleProtocol invocation GUID drifted",
    )?;
    require(
        plan.parameter_type_identities == expected.parameter_type_identities
            && plan.result_type_identity == UEFI_HANDLE_PROTOCOL_STATUS_TYPE_IDENTITY,
        "HandleProtocol invocation signature drifted",
    )?;
    require(
        plan.boundary_entry_plan == expected.boundary_entry_plan
            && plan.calling_plan_report_fingerprint == expected.calling_plan_report_fingerprint
            && plan.calling_plan_commitment == expected.calling_plan_commitment,
        "HandleProtocol Microsoft-x64 plan drifted",
    )?;
    require(
        plan.status_rows == STATUS_ROWS,
        "HandleProtocol closed status table drifted",
    )?;
    validate_exact_locations(plan)
}

fn validate_exact_locations(plan: &UefiHandleProtocolInvocationPlan) -> Result<(), Diagnostic> {
    let call = &plan.boundary_entry_plan.call;
    let expected_registers = [
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86R8,
    ];
    require(
        call.policy == CallingPolicy::MicrosoftX64
            && call.parameters.len() == 3
            && call.shadow_bytes == 32
            && call.stack_alignment == 16,
        "HandleProtocol call frame drifted",
    )?;
    for (placement, expected_register) in call.parameters.iter().zip(expected_registers) {
        require(
            matches!(placement.locations.as_slice(), [ValueLocation::Register { register, value_byte_offset: 0, byte_size: 8 }] if *register == expected_register),
            "HandleProtocol input register placement drifted",
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
        "HandleProtocol result register placement drifted",
    )
}

fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_plan_binds_service_guid_signature_abi_and_statuses() {
        let plan = plan_uefi_handle_protocol_invocation(TargetProfile::UefiX64).unwrap();
        assert!(plan.matches_exact_uefi_x64_plan());
        assert_eq!(plan.service_field().byte_offset(), 152);
        assert_eq!(plan.protocol(), UEFI_LOADED_IMAGE_PROTOCOL_GUID);
        assert_eq!(plan.parameter_type_identities().len(), 3);
        assert_eq!(plan.status_code(UefiHandleProtocolStatus::Success), 0);
        assert_eq!(
            plan.status_code(UefiHandleProtocolStatus::InvalidParameter),
            STATUS_ERROR_BIT | 2
        );
        assert_eq!(
            plan.status_code(UefiHandleProtocolStatus::Unsupported),
            STATUS_ERROR_BIT | 3
        );
        assert_ne!(plan.calling_plan_report_fingerprint(), 0);
        assert_ne!(plan.calling_plan_commitment(), &[0; 32]);
        validate_exact_locations(&plan).unwrap();
    }

    #[test]
    fn non_uefi_profile_and_every_plan_axis_reject() {
        assert!(plan_uefi_handle_protocol_invocation(TargetProfile::WindowsX64).is_err());
        let exact = plan_uefi_handle_protocol_invocation(TargetProfile::UefiX64).unwrap();
        let mutations: Vec<Box<dyn Fn(&mut UefiHandleProtocolInvocationPlan)>> = vec![
            Box::new(|plan| plan.service_identity = "EFI_BOOT_SERVICES.OpenProtocol"),
            Box::new(|plan| {
                plan.service_field = plan_uefi_boot_services_native_layout(TargetProfile::UefiX64)
                    .unwrap()
                    .field_layout(UefiBootServicesNativeField::OpenProtocol)
                    .unwrap()
            }),
            Box::new(|plan| plan.protocol.data1 ^= 1),
            Box::new(|plan| plan.parameter_type_identities.swap(0, 1)),
            Box::new(|plan| plan.result_type_identity = "Unit"),
            Box::new(|plan| plan.boundary_entry_plan.call.parameters.swap(0, 1)),
            Box::new(|plan| plan.calling_plan_report_fingerprint ^= 1),
            Box::new(|plan| plan.calling_plan_commitment[0] ^= 1),
            Box::new(|plan| plan.status_rows.swap(1, 2)),
        ];
        for mutate in mutations {
            let mut candidate = exact.clone();
            mutate(&mut candidate);
            assert!(validate_candidate(&candidate).is_err());
            assert!(!candidate.matches_exact_uefi_x64_plan());
        }
    }
}
