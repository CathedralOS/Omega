use sha2::{Digest, Sha256};
use target::{Architecture, NativeTarget, ObjectFormat};

use super::{
    PhysicalRegisterModel, PreservationConvention, RegisterConstraintCatalog,
    RegisterConstraintFamily, RegisterInstructionConstraint, RegisterOperandAccess,
    RegisterReservationProfile, RegisterUnitId, RegisterUnitKind, RegisterWriteSemantics,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};

const IDENTITY_WIDTH: usize = 32;

fn domain_digest(domain: &[u8], canonical: &[u8]) -> [u8; IDENTITY_WIDTH] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(
        u64::try_from(canonical.len())
            .expect("canonical register identity input length fits u64")
            .to_le_bytes(),
    );
    digest.update(canonical);
    digest.finalize().into()
}

macro_rules! identity {
    ($name:ident, $domain:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; IDENTITY_WIDTH]);

        impl $name {
            fn from_canonical_bytes(canonical: &[u8]) -> Self {
                Self(domain_digest($domain, canonical))
            }

            pub const fn from_bytes(bytes: [u8; IDENTITY_WIDTH]) -> Self {
                Self(bytes)
            }

            pub const fn bytes(self) -> [u8; IDENTITY_WIDTH] {
                self.0
            }
        }
    };
}

identity!(
    PhysicalRegisterModelIdentity,
    b"omega.physical-register-model-identity.v1\0"
);
identity!(
    RegisterConstraintCatalogIdentity,
    b"omega.register-constraint-catalog-identity.v1\0"
);
identity!(
    RegisterReservationProfileIdentity,
    b"omega.register-reservation-profile-identity.v1\0"
);
identity!(
    TargetRegisterEnvironmentIdentity,
    b"omega.target-register-environment-identity.v8\0"
);

pub(super) fn physical_register_model_identity(
    model: &PhysicalRegisterModel,
) -> PhysicalRegisterModelIdentity {
    let mut bytes = Vec::new();
    architecture(&mut bytes, model.architecture);
    length(&mut bytes, model.units.len());
    for unit in &model.units {
        u16_value(&mut bytes, unit.id.0);
        string(&mut bytes, &unit.name);
        u16_value(&mut bytes, unit.bits);
        byte(&mut bytes, unit_kind(unit.kind));
    }
    length(&mut bytes, model.views.len());
    for view in &model.views {
        u16_value(&mut bytes, view.id.0);
        string(&mut bytes, &view.name);
        u16_value(&mut bytes, view.class.0);
        unit_ids(&mut bytes, &view.units);
        unit_ids(&mut bytes, &view.write_units);
        u16_value(&mut bytes, view.bits);
        byte(&mut bytes, write_semantics(view.write_semantics));
        boolean(&mut bytes, view.allocatable);
    }
    length(&mut bytes, model.classes.len());
    for class in &model.classes {
        u16_value(&mut bytes, class.id.0);
        string(&mut bytes, &class.name);
        length(&mut bytes, class.views.len());
        for view in &class.views {
            u16_value(&mut bytes, view.0);
        }
    }
    length(&mut bytes, model.conventions.len());
    for convention in &model.conventions {
        preservation_convention(&mut bytes, convention);
    }
    length(&mut bytes, model.reservations.len());
    for reservation in &model.reservations {
        string(&mut bytes, &reservation.name);
        byte(&mut bytes, reservation_reason(reservation.reason));
        unit_ids(&mut bytes, &reservation.units);
    }
    PhysicalRegisterModelIdentity::from_canonical_bytes(&bytes)
}

pub(super) fn register_constraint_catalog_identity(
    physical: PhysicalRegisterModelIdentity,
    catalog: &RegisterConstraintCatalog,
) -> RegisterConstraintCatalogIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&physical.bytes());
    architecture(&mut bytes, catalog.architecture);
    length(&mut bytes, catalog.required.len());
    for key in &catalog.required {
        constraint_key(&mut bytes, key.family, key.variant);
    }
    length(&mut bytes, catalog.constraints.len());
    for constraint in &catalog.constraints {
        instruction_constraint(&mut bytes, constraint);
    }
    RegisterConstraintCatalogIdentity::from_canonical_bytes(&bytes)
}

pub(super) fn register_reservation_profile_identity(
    target: NativeTarget,
    physical: PhysicalRegisterModelIdentity,
    profile: &RegisterReservationProfile,
    reserved_units: &[RegisterUnitId],
) -> RegisterReservationProfileIdentity {
    let mut bytes = Vec::new();
    native_target(&mut bytes, target);
    bytes.extend_from_slice(&physical.bytes());
    string(&mut bytes, &profile.name);
    length(&mut bytes, profile.active_overlays.len());
    for overlay in &profile.active_overlays {
        string(&mut bytes, overlay);
    }
    unit_ids(&mut bytes, reserved_units);
    RegisterReservationProfileIdentity::from_canonical_bytes(&bytes)
}

pub fn target_register_environment_identity(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: super::TargetRegisterEnvironmentConstraintKeys,
) -> TargetRegisterEnvironmentIdentity {
    let mut bytes = Vec::new();
    native_target(&mut bytes, target);
    bytes.extend_from_slice(&physical.identity().bytes());
    bytes.extend_from_slice(&constraints.identity().bytes());
    bytes.extend_from_slice(&reservations.identity().bytes());
    optional_constraint_key(&mut bytes, selected_keys.structural_unit_call);
    optional_constraint_key(&mut bytes, selected_keys.call_i64_2_u64_to_u64);
    for key in [
        selected_keys.materialize_i64,
        selected_keys.copy_i64,
        selected_keys.add_i64,
        selected_keys.add_i64_immediate,
        selected_keys.subtract_i64,
        selected_keys.subtract_i64_immediate,
        selected_keys.compare_i64_zero,
        selected_keys.conditional_branch,
        selected_keys.return_i64,
        selected_keys.return_unit,
        selected_keys.compare_i64,
    ] {
        constraint_key(&mut bytes, key.family, key.variant);
    }
    TargetRegisterEnvironmentIdentity::from_canonical_bytes(&bytes)
}

fn native_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    architecture(bytes, target.architecture);
    object_format(bytes, target.object_format);
    u64_value(
        bytes,
        u64::try_from(target.pointer_size).expect("target pointer size fits u64"),
    );
    u64_value(
        bytes,
        u64::try_from(target.pointer_alignment).expect("target pointer alignment fits u64"),
    );
}

fn preservation_convention(bytes: &mut Vec<u8>, convention: &PreservationConvention) {
    string(bytes, &convention.name);
    length(bytes, convention.argument_views.len());
    for view in &convention.argument_views {
        u16_value(bytes, view.0);
    }
    length(bytes, convention.result_views.len());
    for view in &convention.result_views {
        u16_value(bytes, view.0);
    }
    unit_ids(bytes, &convention.caller_saved);
    unit_ids(bytes, &convention.callee_saved);
    unit_ids(bytes, &convention.fixed);
    u16_value(bytes, convention.stack_alignment);
    u16_value(bytes, convention.red_zone_bytes);
}

fn instruction_constraint(bytes: &mut Vec<u8>, constraint: &RegisterInstructionConstraint) {
    u16_value(bytes, constraint.id.0);
    constraint_key(bytes, constraint.key.family, constraint.key.variant);
    length(bytes, constraint.operands.len());
    for operand in &constraint.operands {
        u16_value(bytes, operand.operand);
        byte(bytes, operand_access(operand.access));
        u16_value(bytes, operand.class.0);
        optional_u16(bytes, operand.fixed_view.map(|view| view.0));
        optional_u16(bytes, operand.tied_to);
        boolean(bytes, operand.early_clobber);
    }
    unit_ids(bytes, &constraint.implicit_uses);
    unit_ids(bytes, &constraint.implicit_defs);
    unit_ids(bytes, &constraint.clobbers);
}

fn constraint_key(bytes: &mut Vec<u8>, family: RegisterConstraintFamily, variant: u32) {
    byte(bytes, constraint_family(family));
    bytes.extend_from_slice(&variant.to_le_bytes());
}

fn optional_constraint_key(bytes: &mut Vec<u8>, key: Option<super::RegisterConstraintKey>) {
    match key {
        None => byte(bytes, 0),
        Some(key) => {
            byte(bytes, 1);
            constraint_key(bytes, key.family, key.variant);
        }
    }
}

fn unit_ids(bytes: &mut Vec<u8>, values: &[RegisterUnitId]) {
    length(bytes, values.len());
    for value in values {
        u16_value(bytes, value.0);
    }
}

fn optional_u16(bytes: &mut Vec<u8>, value: Option<u16>) {
    match value {
        None => byte(bytes, 0),
        Some(value) => {
            byte(bytes, 1);
            u16_value(bytes, value);
        }
    }
}

fn string(bytes: &mut Vec<u8>, value: &str) {
    length(bytes, value.len());
    bytes.extend_from_slice(value.as_bytes());
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    u64_value(
        bytes,
        u64::try_from(value).expect("canonical register identity length fits u64"),
    );
}

fn u64_value(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn u16_value(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn boolean(bytes: &mut Vec<u8>, value: bool) {
    byte(bytes, u8::from(value));
}

fn byte(bytes: &mut Vec<u8>, value: u8) {
    bytes.push(value);
}

const fn architecture_value(value: Architecture) -> u8 {
    match value {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => 1,
    }
}

fn architecture(bytes: &mut Vec<u8>, value: Architecture) {
    byte(bytes, architecture_value(value));
}

fn object_format(bytes: &mut Vec<u8>, value: ObjectFormat) {
    byte(
        bytes,
        match value {
            ObjectFormat::Elf => 0,
            ObjectFormat::MachO => 1,
            ObjectFormat::Coff => 2,
        },
    );
}

const fn unit_kind(value: RegisterUnitKind) -> u8 {
    match value {
        RegisterUnitKind::IntegerLane => 0,
        RegisterUnitKind::VectorLane => 1,
        RegisterUnitKind::Flags => 2,
        RegisterUnitKind::StackPointer => 3,
        RegisterUnitKind::InstructionPointer => 4,
        RegisterUnitKind::Zero => 5,
        RegisterUnitKind::FloatingControl => 6,
    }
}

const fn write_semantics(value: RegisterWriteSemantics) -> u8 {
    match value {
        RegisterWriteSemantics::ExactView => 0,
        RegisterWriteSemantics::PreservesUnwritten => 1,
        RegisterWriteSemantics::ZeroExtendsParent => 2,
        RegisterWriteSemantics::ZeroExtendsWithinUnit => 3,
        RegisterWriteSemantics::Discards => 4,
        RegisterWriteSemantics::InstructionDefined => 5,
    }
}

const fn reservation_reason(value: super::ReservationReason) -> u8 {
    match value {
        super::ReservationReason::Architectural => 0,
        super::ReservationReason::StackPointer => 1,
        super::ReservationReason::FramePointer => 2,
        super::ReservationReason::Platform => 3,
        super::ReservationReason::Dispatch => 4,
        super::ReservationReason::Metering => 5,
        super::ReservationReason::Syscall => 6,
        super::ReservationReason::InlineAssembly => 7,
        super::ReservationReason::Backend => 8,
    }
}

const fn constraint_family(value: RegisterConstraintFamily) -> u8 {
    match value {
        RegisterConstraintFamily::Call => 0,
        RegisterConstraintFamily::Return => 1,
        RegisterConstraintFamily::SystemCall => 2,
        RegisterConstraintFamily::InlineAssembly => 3,
        RegisterConstraintFamily::Instruction => 4,
    }
}

const fn operand_access(value: RegisterOperandAccess) -> u8 {
    match value {
        RegisterOperandAccess::Use => 0,
        RegisterOperandAccess::Def => 1,
        RegisterOperandAccess::UseDef => 2,
    }
}
