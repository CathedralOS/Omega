use omega_register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::MachineEncodedEffects;

use crate::{
    X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint, x86_64_physical_register_model,
};

/// Target-decoded meaning of the canonical dependency-breaking x86-64 zero
/// idiom. Keeping this separate from the baseline `MaterializeI64` form makes
/// the added RFLAGS clobber explicit to a later post-allocation rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64DecodedXorZeroI64Materialization {
    destination: RegisterViewId,
    value_bits: u64,
    footprint: X86_64SelectedFormFootprint,
}

impl X86_64DecodedXorZeroI64Materialization {
    pub const fn destination(&self) -> RegisterViewId {
        self.destination
    }

    pub const fn value_bits(&self) -> u64 {
        self.value_bits
    }

    pub const fn footprint(&self) -> &X86_64SelectedFormFootprint {
        &self.footprint
    }
}

/// Canonical three-byte `xor r64, r64` bytes plus their independently decoded
/// destination, zero value, and complete machine-state footprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64XorZeroI64Materialization {
    bytes: Vec<u8>,
    decoded: X86_64DecodedXorZeroI64Materialization,
}

impl ValidatedX86_64XorZeroI64Materialization {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn decoded(&self) -> &X86_64DecodedXorZeroI64Materialization {
        &self.decoded
    }

    pub const fn destination(&self) -> RegisterViewId {
        self.decoded.destination()
    }

    pub const fn value_bits(&self) -> u64 {
        self.decoded.value_bits()
    }

    pub const fn footprint(&self) -> &X86_64SelectedFormFootprint {
        self.decoded.footprint()
    }
}

/// Encode the unique three-byte dependency-breaking realization of an exact
/// i64 zero in one canonical allocatable 64-bit GPR. This is an optimizer form,
/// not a second always-applicable selected-instruction catalog alternative.
pub fn encode_x86_64_xor_zero_i64_materialization(
    physical: &ValidatedPhysicalRegisterModel,
    destination: RegisterViewId,
) -> Result<ValidatedX86_64XorZeroI64Materialization, X86_64SelectedFormEncodingError> {
    let register = validate_destination(physical, destination)?;
    let bytes = [rex(register, register), 0x31, modrm(register, register)];
    validate_x86_64_xor_zero_i64_materialization(physical, destination, &bytes)
}

/// Decode exactly one canonical `REX.W + 31 /r` dependency-breaking zero
/// idiom. The ModRM register and r/m fields must name the same allocatable
/// 64-bit GPR; alternate prefixes, noncanonical REX bits, and suffixes fail.
pub fn decode_x86_64_xor_zero_i64_materialization(
    physical: &ValidatedPhysicalRegisterModel,
    bytes: &[u8],
) -> Result<X86_64DecodedXorZeroI64Materialization, X86_64SelectedFormEncodingError> {
    validate_canonical_physical_model(physical)?;
    let [rex_byte, opcode, modrm_byte] = bytes else {
        return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
    };
    if *opcode != 0x31 || modrm_byte >> 6 != 3 {
        return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
    }
    let register = ((modrm_byte >> 3) & 7) | (((rex_byte >> 2) & 1) << 3);
    let rm = (modrm_byte & 7) | ((rex_byte & 1) << 3);
    if register != rm
        || *rex_byte != rex(register, register)
        || *modrm_byte != modrm(register, register)
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let destination = canonical_gpr64_view(physical, register)?;
    Ok(X86_64DecodedXorZeroI64Materialization {
        destination,
        value_bits: 0,
        footprint: footprint(destination),
    })
}

/// Validate canonical XOR-zero bytes against the post-allocation destination
/// selected by the caller. Destination and zero semantics come from the byte
/// decoder rather than from the request.
pub fn validate_x86_64_xor_zero_i64_materialization(
    physical: &ValidatedPhysicalRegisterModel,
    destination: RegisterViewId,
    bytes: &[u8],
) -> Result<ValidatedX86_64XorZeroI64Materialization, X86_64SelectedFormEncodingError> {
    validate_destination(physical, destination)?;
    let decoded = decode_x86_64_xor_zero_i64_materialization(physical, bytes)?;
    if decoded.destination != destination || decoded.value_bits != 0 {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64XorZeroI64Materialization {
        bytes: bytes.to_vec(),
        decoded,
    })
}

fn validate_canonical_physical_model(
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), X86_64SelectedFormEncodingError> {
    if physical.model() != &x86_64_physical_register_model() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    Ok(())
}

fn validate_destination(
    physical: &ValidatedPhysicalRegisterModel,
    destination: RegisterViewId,
) -> Result<u8, X86_64SelectedFormEncodingError> {
    validate_canonical_physical_model(physical)?;
    GPR64_NAMES
        .iter()
        .enumerate()
        .find_map(|(register, name)| {
            physical
                .model()
                .view_named(name)
                .filter(|view| view.id == destination && view.bits == 64 && view.allocatable)
                .map(|_| register as u8)
        })
        .filter(|register| *register != 4)
        .ok_or(X86_64SelectedFormEncodingError::UnknownOrNonGpr64View(
            destination,
        ))
}

fn canonical_gpr64_view(
    physical: &ValidatedPhysicalRegisterModel,
    register: u8,
) -> Result<RegisterViewId, X86_64SelectedFormEncodingError> {
    let view = GPR64_NAMES
        .get(usize::from(register))
        .and_then(|name| physical.model().view_named(name))
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    if register == 4 || view.bits != 64 || !view.allocatable {
        return Err(X86_64SelectedFormEncodingError::UnknownOrNonGpr64View(
            view.id,
        ));
    }
    Ok(view.id)
}

fn footprint(destination: RegisterViewId) -> X86_64SelectedFormFootprint {
    let physical = x86_64_physical_register_model();
    let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![], vec![0]);
    encoded.implicit_unit_clobbers = physical.view_named("rflags").unwrap().units.clone();
    X86_64SelectedFormFootprint {
        register_reads: vec![],
        register_writes: vec![destination],
        writes_rflags: true,
        encoded,
    }
}

const GPR64_NAMES: [&str; 16] = [
    "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13",
    "r14", "r15",
];

fn rex(register: u8, rm: u8) -> u8 {
    0x48 | ((register >> 3) << 2) | (rm >> 3)
}

fn modrm(register: u8, rm: u8) -> u8 {
    0xc0 | ((register & 7) << 3) | (rm & 7)
}

#[cfg(test)]
mod tests {
    use omega_register_model::{RegisterViewId, validate_physical_register_model};
    use omega_selected_instructions::{
        MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior,
    };

    use super::*;

    #[test]
    fn round_trips_every_low_and_high_gpr_with_exact_zero_effects() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let rflags = physical.model().view_named("rflags").unwrap();

        for (name, expected) in [
            ("rax", [0x48, 0x31, 0xc0]),
            ("rcx", [0x48, 0x31, 0xc9]),
            ("rdx", [0x48, 0x31, 0xd2]),
            ("rbx", [0x48, 0x31, 0xdb]),
            ("rbp", [0x48, 0x31, 0xed]),
            ("rsi", [0x48, 0x31, 0xf6]),
            ("rdi", [0x48, 0x31, 0xff]),
            ("r8", [0x4d, 0x31, 0xc0]),
            ("r9", [0x4d, 0x31, 0xc9]),
            ("r10", [0x4d, 0x31, 0xd2]),
            ("r11", [0x4d, 0x31, 0xdb]),
            ("r12", [0x4d, 0x31, 0xe4]),
            ("r13", [0x4d, 0x31, 0xed]),
            ("r14", [0x4d, 0x31, 0xf6]),
            ("r15", [0x4d, 0x31, 0xff]),
        ] {
            let destination = physical.model().view_named(name).unwrap().id;
            let encoded = encode_x86_64_xor_zero_i64_materialization(&physical, destination)
                .expect("canonical GPR must encode");
            assert_eq!(encoded.bytes(), expected);
            assert_eq!(encoded.destination(), destination);
            assert_eq!(encoded.value_bits(), 0);

            let decoded =
                decode_x86_64_xor_zero_i64_materialization(&physical, encoded.bytes()).unwrap();
            assert_eq!(decoded.destination(), destination);
            assert_eq!(decoded.value_bits(), 0);
            assert_eq!(decoded, *encoded.decoded());
            assert_eq!(
                validate_x86_64_xor_zero_i64_materialization(
                    &physical,
                    destination,
                    encoded.bytes(),
                )
                .unwrap(),
                encoded
            );

            let footprint = encoded.footprint();
            assert!(footprint.register_reads.is_empty());
            assert_eq!(footprint.register_writes, [destination]);
            assert!(footprint.writes_rflags);
            assert!(footprint.encoded.external_operand_reads.is_empty());
            assert_eq!(footprint.encoded.external_operand_writes, [0]);
            assert!(footprint.encoded.implicit_unit_uses.is_empty());
            assert!(footprint.encoded.implicit_unit_defs.is_empty());
            assert_eq!(footprint.encoded.implicit_unit_clobbers, rflags.units);
            assert_eq!(footprint.encoded.memory, MachineEncodedMemoryEffect::NoneV1);
            assert_eq!(
                footprint.encoded.stack,
                MachineEncodedStackEffect::UnchangedV1
            );
            assert_eq!(footprint.encoded.trap, MachineEncodedTrapBehavior::NeverV1);
            assert_eq!(
                footprint.encoded.control,
                MachineEncodedControlEffect::FallThroughV1
            );
        }
    }

    #[test]
    fn decoder_rejects_rex_opcode_modrm_and_framing_corruption() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        for bytes in [
            &[][..],
            &[0x31, 0xc0][..],
            &[0x48, 0x31][..],
            &[0x48, 0x31, 0xc0, 0x90][..],
            &[0x40, 0x31, 0xc0][..],
            &[0x4a, 0x31, 0xc0][..],
            &[0x49, 0x31, 0xc0][..],
            &[0x4c, 0x31, 0xc0][..],
            &[0x48, 0x30, 0xc0][..],
            &[0x48, 0x29, 0xc0][..],
            &[0x48, 0x31, 0x00][..],
            &[0x48, 0x31, 0xc1][..],
        ] {
            assert!(
                decode_x86_64_xor_zero_i64_materialization(&physical, bytes).is_err(),
                "noncanonical bytes were accepted: {bytes:02x?}"
            );
        }
    }

    #[test]
    fn validation_rejects_destination_view_and_model_corruption() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let rax = physical.model().view_named("rax").unwrap().id;
        let rbx = physical.model().view_named("rbx").unwrap().id;
        let rsp = physical.model().view_named("rsp").unwrap().id;
        let eax = physical.model().view_named("eax").unwrap().id;
        let rax_bytes = encode_x86_64_xor_zero_i64_materialization(&physical, rax)
            .unwrap()
            .bytes()
            .to_vec();

        assert_eq!(
            validate_x86_64_xor_zero_i64_materialization(&physical, rbx, &rax_bytes),
            Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
        );
        for view in [rsp, eax, RegisterViewId(u16::MAX)] {
            assert_eq!(
                encode_x86_64_xor_zero_i64_materialization(&physical, view),
                Err(X86_64SelectedFormEncodingError::UnknownOrNonGpr64View(view))
            );
        }
        assert_eq!(
            decode_x86_64_xor_zero_i64_materialization(&physical, &[0x48, 0x31, 0xe4]),
            Err(X86_64SelectedFormEncodingError::UnknownOrNonGpr64View(rsp))
        );

        let mut forged = x86_64_physical_register_model();
        forged.views[0].name = "forged.rax".into();
        let forged = validate_physical_register_model(forged).unwrap();
        assert_eq!(
            encode_x86_64_xor_zero_i64_materialization(&forged, rax),
            Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel)
        );
        assert_eq!(
            decode_x86_64_xor_zero_i64_materialization(&forged, &rax_bytes),
            Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel)
        );
        assert_eq!(
            validate_x86_64_xor_zero_i64_materialization(&forged, rax, &rax_bytes),
            Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel)
        );
    }
}
