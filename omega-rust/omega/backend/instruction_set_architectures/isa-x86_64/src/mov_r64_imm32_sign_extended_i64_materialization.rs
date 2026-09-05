use register_model::{
    RegisterUnitId, RegisterViewId, RegisterWriteSemantics, ValidatedPhysicalRegisterModel,
};
use selected_instructions::MachineEncodedEffects;
use semantic_vocabulary::IntegerValue;

use crate::x86_64_physical_register_model;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64MovR64Imm32SignExtendedI64MaterializationFootprint {
    pub register_reads: Vec<RegisterViewId>,
    pub register_writes: Vec<RegisterViewId>,
    pub encoded_write_view: RegisterViewId,
    pub encoded_write_view_units: Vec<RegisterUnitId>,
    pub encoded_write_units: Vec<RegisterUnitId>,
    pub encoded_write_semantics: RegisterWriteSemantics,
    pub writes_rflags: bool,
    pub encoded: MachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64DecodedMovR64Imm32SignExtendedI64Materialization {
    destination: RegisterViewId,
    value_bits: u64,
    footprint: X86_64MovR64Imm32SignExtendedI64MaterializationFootprint,
}

impl X86_64DecodedMovR64Imm32SignExtendedI64Materialization {
    pub const fn destination(&self) -> RegisterViewId {
        self.destination
    }
    pub const fn encoded_write_view(&self) -> RegisterViewId {
        self.destination
    }
    pub const fn value_bits(&self) -> u64 {
        self.value_bits
    }
    pub const fn footprint(&self) -> &X86_64MovR64Imm32SignExtendedI64MaterializationFootprint {
        &self.footprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64MovR64Imm32SignExtendedI64Materialization {
    bytes: Vec<u8>,
    decoded: X86_64DecodedMovR64Imm32SignExtendedI64Materialization,
}

impl ValidatedX86_64MovR64Imm32SignExtendedI64Materialization {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub const fn decoded(&self) -> &X86_64DecodedMovR64Imm32SignExtendedI64Materialization {
        &self.decoded
    }
    pub const fn destination(&self) -> RegisterViewId {
        self.decoded.destination()
    }
    pub const fn encoded_write_view(&self) -> RegisterViewId {
        self.decoded.encoded_write_view()
    }
    pub const fn value_bits(&self) -> u64 {
        self.decoded.value_bits()
    }
    pub const fn footprint(&self) -> &X86_64MovR64Imm32SignExtendedI64MaterializationFootprint {
        self.decoded.footprint()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86_64MovR64Imm32SignExtendedI64MaterializationError {
    NonCanonicalPhysicalModel,
    UnknownOrNonGpr64View(RegisterViewId),
    IntegerOutsideSignExtendedI32Bits,
    MalformedEncoding,
    EncodedFormMismatch,
}

impl std::fmt::Display for X86_64MovR64Imm32SignExtendedI64MaterializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid x86-64 MOV-r64-imm32 sign-extended i64 materialization: {self:?}"
        )
    }
}
impl std::error::Error for X86_64MovR64Imm32SignExtendedI64MaterializationError {}

pub fn encode_x86_64_mov_r64_imm32_sign_extended_i64_materialization(
    physical: &ValidatedPhysicalRegisterModel,
    destination: RegisterViewId,
    value: IntegerValue,
) -> Result<
    ValidatedX86_64MovR64Imm32SignExtendedI64Materialization,
    X86_64MovR64Imm32SignExtendedI64MaterializationError,
> {
    require_canonical_model(physical)?;
    let register = register_number(physical, destination)?;
    let immediate = sign_extended_i32(value)?;
    let bytes = vec![
        if register < 8 { 0x48 } else { 0x49 },
        0xc7,
        0xc0 | (register & 7),
        immediate[0],
        immediate[1],
        immediate[2],
        immediate[3],
    ];
    validate_x86_64_mov_r64_imm32_sign_extended_i64_materialization(
        physical,
        destination,
        value,
        &bytes,
    )
}

pub fn decode_x86_64_mov_r64_imm32_sign_extended_i64_materialization(
    physical: &ValidatedPhysicalRegisterModel,
    bytes: &[u8],
) -> Result<
    X86_64DecodedMovR64Imm32SignExtendedI64Materialization,
    X86_64MovR64Imm32SignExtendedI64MaterializationError,
> {
    require_canonical_model(physical)?;
    let [rex @ (0x48 | 0x49), 0xc7, modrm, a, b, c, d] = bytes else {
        return Err(X86_64MovR64Imm32SignExtendedI64MaterializationError::MalformedEncoding);
    };
    if modrm & 0xf8 != 0xc0 {
        return Err(X86_64MovR64Imm32SignExtendedI64MaterializationError::MalformedEncoding);
    }
    let register = (modrm & 7) + if *rex == 0x49 { 8 } else { 0 };
    if register == 4 {
        return Err(X86_64MovR64Imm32SignExtendedI64MaterializationError::EncodedFormMismatch);
    }
    let name = GPR_NAMES
        .get(usize::from(register))
        .ok_or(X86_64MovR64Imm32SignExtendedI64MaterializationError::MalformedEncoding)?;
    let view = physical
        .model()
        .view_named(name)
        .ok_or(X86_64MovR64Imm32SignExtendedI64MaterializationError::MalformedEncoding)?;
    if view.bits != 64
        || !view.allocatable
        || view.write_semantics != RegisterWriteSemantics::ExactView
        || view.units.len() != 4
        || view.write_units != view.units
    {
        return Err(X86_64MovR64Imm32SignExtendedI64MaterializationError::EncodedFormMismatch);
    }
    let immediate = i32::from_le_bytes([*a, *b, *c, *d]);
    let value_bits = (i64::from(immediate)) as u64;
    let destination = view.id;
    Ok(X86_64DecodedMovR64Imm32SignExtendedI64Materialization {
        destination,
        value_bits,
        footprint: X86_64MovR64Imm32SignExtendedI64MaterializationFootprint {
            register_reads: vec![],
            register_writes: vec![destination],
            encoded_write_view: destination,
            encoded_write_view_units: view.units.clone(),
            encoded_write_units: view.write_units.clone(),
            encoded_write_semantics: view.write_semantics,
            writes_rflags: false,
            encoded: MachineEncodedEffects::fallthrough_v1(vec![], vec![0]),
        },
    })
}

pub fn validate_x86_64_mov_r64_imm32_sign_extended_i64_materialization(
    physical: &ValidatedPhysicalRegisterModel,
    destination: RegisterViewId,
    value: IntegerValue,
    bytes: &[u8],
) -> Result<
    ValidatedX86_64MovR64Imm32SignExtendedI64Materialization,
    X86_64MovR64Imm32SignExtendedI64MaterializationError,
> {
    let expected = i64::from(i32::from_le_bytes(sign_extended_i32(value)?)) as u64;
    let decoded = decode_x86_64_mov_r64_imm32_sign_extended_i64_materialization(physical, bytes)?;
    if decoded.destination != destination || decoded.value_bits != expected {
        return Err(X86_64MovR64Imm32SignExtendedI64MaterializationError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64MovR64Imm32SignExtendedI64Materialization {
        bytes: bytes.to_vec(),
        decoded,
    })
}

fn register_number(
    physical: &ValidatedPhysicalRegisterModel,
    destination: RegisterViewId,
) -> Result<u8, X86_64MovR64Imm32SignExtendedI64MaterializationError> {
    GPR_NAMES
        .iter()
        .enumerate()
        .find_map(|(number, name)| {
            physical
                .model()
                .view_named(name)
                .filter(|view| view.id == destination && view.bits == 64 && view.allocatable)
                .map(|_| number as u8)
        })
        .filter(|number| *number != 4)
        .ok_or(
            X86_64MovR64Imm32SignExtendedI64MaterializationError::UnknownOrNonGpr64View(
                destination,
            ),
        )
}

fn require_canonical_model(
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), X86_64MovR64Imm32SignExtendedI64MaterializationError> {
    if physical.model() != &x86_64_physical_register_model() {
        return Err(
            X86_64MovR64Imm32SignExtendedI64MaterializationError::NonCanonicalPhysicalModel,
        );
    }
    Ok(())
}

fn sign_extended_i32(
    value: IntegerValue,
) -> Result<[u8; 4], X86_64MovR64Imm32SignExtendedI64MaterializationError> {
    let bits = match value {
        IntegerValue::Signed(value) => i64::try_from(value).map(|value| value as u64),
        IntegerValue::Unsigned(value) => u64::try_from(value),
    }
    .map_err(|_| {
        X86_64MovR64Imm32SignExtendedI64MaterializationError::IntegerOutsideSignExtendedI32Bits
    })?;
    let low = bits as u32;
    if (i64::from(low as i32) as u64) != bits {
        return Err(
            X86_64MovR64Imm32SignExtendedI64MaterializationError::IntegerOutsideSignExtendedI32Bits,
        );
    }
    Ok(low.to_le_bytes())
}

const GPR_NAMES: [&str; 16] = [
    "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13",
    "r14", "r15",
];

#[cfg(test)]
mod tests {
    use super::*;
    use register_model::{RegisterViewId, validate_physical_register_model};

    #[test]
    fn all_allocatable_gprs_and_sign_extension_boundaries_round_trip() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let values = [
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(i32::MAX as u128),
            IntegerValue::Unsigned(0xffff_ffff_8000_0000),
            IntegerValue::Unsigned(u64::MAX as u128),
            IntegerValue::Signed(i32::MIN as i128),
            IntegerValue::Signed(-1),
        ];
        for (number, name) in GPR_NAMES.iter().enumerate() {
            if number == 4 {
                continue;
            }
            let destination = physical.model().view_named(name).unwrap().id;
            for value in values {
                let encoded = encode_x86_64_mov_r64_imm32_sign_extended_i64_materialization(
                    &physical,
                    destination,
                    value,
                )
                .unwrap();
                assert_eq!(encoded.bytes().len(), 7);
                assert_eq!(encoded.bytes()[0], if number < 8 { 0x48 } else { 0x49 });
                assert_eq!(&encoded.bytes()[1..3], &[0xc7, 0xc0 | (number as u8 & 7)]);
                assert_eq!(
                    decode_x86_64_mov_r64_imm32_sign_extended_i64_materialization(
                        &physical,
                        encoded.bytes()
                    )
                    .unwrap(),
                    *encoded.decoded()
                );
                assert_eq!(
                    encoded.footprint().encoded_write_semantics,
                    RegisterWriteSemantics::ExactView
                );
            }
        }
    }

    #[test]
    fn rejects_gap_values_bad_destinations_and_byte_corruption() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let rax = physical.model().view_named("rax").unwrap().id;
        for value in [
            0x8000_0000_u128,
            u32::MAX as u128,
            0x1_0000_0000,
            0xffff_ffff_7fff_ffff,
        ] {
            assert_eq!(encode_x86_64_mov_r64_imm32_sign_extended_i64_materialization(&physical, rax, IntegerValue::Unsigned(value)), Err(X86_64MovR64Imm32SignExtendedI64MaterializationError::IntegerOutsideSignExtendedI32Bits));
        }
        for name in ["rsp", "eax", "rflags"] {
            let destination = physical.model().view_named(name).unwrap().id;
            assert!(
                encode_x86_64_mov_r64_imm32_sign_extended_i64_materialization(
                    &physical,
                    destination,
                    IntegerValue::Signed(-1)
                )
                .is_err()
            );
        }
        assert!(
            encode_x86_64_mov_r64_imm32_sign_extended_i64_materialization(
                &physical,
                RegisterViewId(u16::MAX),
                IntegerValue::Signed(-1)
            )
            .is_err()
        );
        let canonical = [0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff];
        for bad in [
            vec![],
            vec![0x40, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff],
            vec![0x48, 0xc6, 0xc0, 0xff, 0xff, 0xff, 0xff],
            vec![0x48, 0xc7, 0xc8, 0xff, 0xff, 0xff, 0xff],
            [canonical.as_slice(), &[0][..]].concat(),
        ] {
            assert!(
                decode_x86_64_mov_r64_imm32_sign_extended_i64_materialization(&physical, &bad)
                    .is_err()
            );
        }
        let rcx = physical.model().view_named("rcx").unwrap().id;
        assert!(
            validate_x86_64_mov_r64_imm32_sign_extended_i64_materialization(
                &physical,
                rcx,
                IntegerValue::Signed(-1),
                &canonical
            )
            .is_err()
        );
        assert!(
            validate_x86_64_mov_r64_imm32_sign_extended_i64_materialization(
                &physical,
                rax,
                IntegerValue::Signed(-2),
                &canonical
            )
            .is_err()
        );

        let mut forged_model = x86_64_physical_register_model();
        let eax_index = usize::from(forged_model.view_named("eax").unwrap().id.0);
        forged_model.views[eax_index].write_semantics = RegisterWriteSemantics::PreservesUnwritten;
        let forged = validate_physical_register_model(forged_model).unwrap();
        assert_eq!(
            decode_x86_64_mov_r64_imm32_sign_extended_i64_materialization(&forged, &canonical),
            Err(X86_64MovR64Imm32SignExtendedI64MaterializationError::NonCanonicalPhysicalModel)
        );
    }
}
