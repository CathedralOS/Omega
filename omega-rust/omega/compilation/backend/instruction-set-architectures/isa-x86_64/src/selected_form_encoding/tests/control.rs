//! Control-form tests: near/short returns and branch encodings —
//! displacement bounds, flag effects, and noncanonical-form rejection.

use super::*;

#[test]
fn near_return_is_exact_and_separates_abi_result_custody_from_encoded_effects() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let rax = physical.model().view_named("rax").unwrap().id;
    let rbx = physical.model().view_named("rbx").unwrap().id;
    let rsp = physical.model().view_named("rsp").unwrap();
    let rip = physical.model().view_named("rip").unwrap();
    let kind = SelectedInstructionKind::ReturnScalar;
    let alternative = alternative(MachineAlternativeFamily::ReturnScalar, 0);
    let encoded = encode_x86_64_selected_form(&physical, kind, alternative, &[rax]).unwrap();

    assert_eq!(encoded.bytes(), [0xc3]);
    assert!(encoded.footprint().register_reads.is_empty());
    assert!(encoded.footprint().register_writes.is_empty());
    assert_eq!(encoded.footprint().encoded.external_operand_reads, []);
    assert_eq!(encoded.footprint().encoded.external_operand_writes, []);
    assert_eq!(encoded.footprint().encoded.implicit_unit_uses, rsp.units);
    let mut expected_defs = rsp.units.clone();
    expected_defs.extend(&rip.units);
    expected_defs.sort_unstable();
    expected_defs.dedup();
    assert_eq!(
        encoded.footprint().encoded.implicit_unit_defs,
        expected_defs
    );
    assert_eq!(
        encoded.footprint().encoded.memory,
        MachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer: rsp.id,
            byte_count: 8,
        }
    );
    assert_eq!(
        encoded.footprint().encoded.stack,
        MachineEncodedStackEffect::PopBytesV1 {
            stack_pointer: rsp.id,
            byte_count: 8,
        }
    );
    assert_eq!(
        encoded.footprint().encoded.trap,
        MachineEncodedTrapBehavior::MayArchitecturalFaultV1
    );
    assert_eq!(
        encoded.footprint().encoded.control,
        MachineEncodedControlEffect::ReturnFromActivationStackV1
    );
    assert!(encode_x86_64_selected_form(&physical, kind, alternative, &[rbx]).is_err());
    assert!(
        validate_x86_64_selected_form_encoding(&physical, kind, alternative, &[rax], &[0xc2, 0, 0])
            .is_err()
    );
    assert!(
        validate_x86_64_selected_form_encoding(&physical, kind, alternative, &[rax], &[0xc3, 0xc3])
            .is_err()
    );
}

#[test]
fn unit_return_is_a_distinct_zero_operand_near_return() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::ReturnUnit;
    let return_alternative = alternative(MachineAlternativeFamily::ReturnUnit, 0);
    let encoded = encode_x86_64_selected_form(&physical, kind, return_alternative, &[]).unwrap();

    assert_eq!(encoded.bytes(), [0xc3]);
    assert!(encoded.footprint().register_reads.is_empty());
    assert!(encoded.footprint().register_writes.is_empty());
    assert_eq!(
        encoded.footprint().encoded.control,
        MachineEncodedControlEffect::ReturnFromActivationStackV1
    );
    assert!(
        encode_x86_64_selected_form(
            &physical,
            kind,
            alternative(MachineAlternativeFamily::ReturnScalar, 0),
            &[]
        )
        .is_err()
    );
}

#[test]
fn near_nonzero_branch_has_exact_end_relative_displacement_and_effects() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let alternative = alternative(MachineAlternativeFamily::ConditionalBranchNonZero, 0);
    for displacement in [i64::from(i32::MIN), -6, 0, 6, i64::from(i32::MAX)] {
        let encoded =
            encode_x86_64_selected_nonzero_branch_form(&physical, alternative, displacement)
                .unwrap();
        assert_eq!(&encoded.bytes()[..2], [0x0f, 0x85]);
        assert_eq!(
            i32::from_le_bytes(encoded.bytes()[2..].try_into().unwrap()),
            displacement as i32
        );
        assert!(encoded.footprint().register_reads.is_empty());
        assert!(encoded.footprint().register_writes.is_empty());
        assert_eq!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::ConditionalRelativeBranchV1
        );
    }
    assert!(
        encode_x86_64_selected_nonzero_branch_form(&physical, alternative, i64::from(i32::MAX) + 1)
            .is_err()
    );
    assert!(
        validate_x86_64_selected_nonzero_branch_form(
            &physical,
            alternative,
            0,
            &[0x0f, 0x84, 0, 0, 0, 0]
        )
        .is_err()
    );
    assert!(
        validate_x86_64_selected_nonzero_branch_form(
            &physical,
            alternative,
            0,
            &[0x0f, 0x85, 0, 0, 0, 0, 0]
        )
        .is_err()
    );
}

#[test]
fn short_nonzero_branch_has_exact_signed_rel8_bounds_and_near_footprint() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let alternative = alternative(MachineAlternativeFamily::ConditionalBranchNonZero, 0);
    let near = encode_x86_64_selected_nonzero_branch_form(&physical, alternative, 0).unwrap();

    for (displacement, encoded_displacement) in [(-128, 0x80), (127, 0x7f)] {
        let encoded =
            encode_x86_64_selected_short_nonzero_branch_form(&physical, alternative, displacement)
                .unwrap();
        assert_eq!(encoded.bytes(), [0x75, encoded_displacement]);
        assert_eq!(encoded.footprint(), near.footprint());
    }

    for displacement in [-129, 128] {
        assert_eq!(
            encode_x86_64_selected_short_nonzero_branch_form(&physical, alternative, displacement,),
            Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
        );
        assert_eq!(
            validate_x86_64_selected_short_nonzero_branch_form(
                &physical,
                alternative,
                displacement,
                &[0x75, 0],
            ),
            Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
        );
    }
}

#[test]
fn short_nonzero_branch_validation_rejects_every_noncanonical_form() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let canonical_alternative = alternative(MachineAlternativeFamily::ConditionalBranchNonZero, 0);

    for bytes in [&[0x74, 0][..], &[0x75][..], &[0x75, 0, 0][..]] {
        assert_eq!(
            validate_x86_64_selected_short_nonzero_branch_form(
                &physical,
                canonical_alternative,
                0,
                bytes,
            ),
            Err(X86_64SelectedFormEncodingError::MalformedEncoding)
        );
    }
    assert_eq!(
        validate_x86_64_selected_short_nonzero_branch_form(
            &physical,
            canonical_alternative,
            0,
            &[0x75, 1],
        ),
        Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
    );

    let wrong_alternative = alternative(MachineAlternativeFamily::ConditionalBranchNonZero, 1);
    assert_eq!(
        encode_x86_64_selected_short_nonzero_branch_form(&physical, wrong_alternative, 0,),
        Err(X86_64SelectedFormEncodingError::AlternativeMismatch)
    );
    assert_eq!(
        validate_x86_64_selected_short_nonzero_branch_form(
            &physical,
            alternative(MachineAlternativeFamily::ReturnScalar, 0),
            0,
            &[0x75, 0],
        ),
        Err(X86_64SelectedFormEncodingError::AlternativeMismatch)
    );

    let mut forged = x86_64_physical_register_model();
    forged.views[0].name = "forged.rax".into();
    let forged = validate_physical_register_model(forged).unwrap();
    assert_eq!(
        encode_x86_64_selected_short_nonzero_branch_form(&forged, canonical_alternative, 0,),
        Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel)
    );
}

#[test]
fn near_u64_less_than_branch_is_exact_jb_rel32_with_flag_control_effects() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let rflags = physical.model().view_named("rflags").unwrap();
    let rip = physical.model().view_named("rip").unwrap();
    let alternative = alternative(MachineAlternativeFamily::ConditionalBranchU64LessThan, 0);
    for displacement in [i64::from(i32::MIN), -6, 0, 6, i64::from(i32::MAX)] {
        let encoded =
            encode_x86_64_selected_u64_less_than_branch_form(&physical, alternative, displacement)
                .unwrap();
        assert_eq!(&encoded.bytes()[..2], [0x0f, 0x82]);
        assert_eq!(
            i32::from_le_bytes(encoded.bytes()[2..].try_into().unwrap()),
            displacement as i32
        );
        assert!(encoded.footprint().register_reads.is_empty());
        assert!(encoded.footprint().register_writes.is_empty());
        assert_eq!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::ConditionalRelativeBranchV1
        );
        assert!(rflags.units.iter().all(|unit| {
            encoded
                .footprint()
                .encoded
                .implicit_unit_uses
                .contains(unit)
        }));
        assert!(rip.units.iter().all(|unit| {
            encoded
                .footprint()
                .encoded
                .implicit_unit_uses
                .contains(unit)
        }));
        assert_eq!(encoded.footprint().encoded.implicit_unit_defs, rip.units);
    }
    assert_eq!(
        encode_x86_64_selected_u64_less_than_branch_form(
            &physical,
            alternative,
            i64::from(i32::MAX) + 1,
        ),
        Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)
    );
    for bytes in [
        &[0x0f, 0x83, 0, 0, 0, 0][..],
        &[0x0f, 0x82, 0, 0, 0][..],
        &[0x0f, 0x82, 0, 0, 0, 0, 0][..],
    ] {
        assert_eq!(
            validate_x86_64_selected_u64_less_than_branch_form(&physical, alternative, 0, bytes,),
            Err(X86_64SelectedFormEncodingError::MalformedEncoding)
        );
    }
}

#[test]
fn short_u64_less_than_branch_is_exact_jb_rel8_and_rejects_near_forms() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let near_alternative = alternative(MachineAlternativeFamily::ConditionalBranchU64LessThan, 0);
    let short_alternative = alternative(MachineAlternativeFamily::ConditionalBranchU64LessThan, 1);
    let near =
        encode_x86_64_selected_u64_less_than_branch_form(&physical, near_alternative, 0).unwrap();
    for (displacement, encoded_displacement) in [(-128, 0x80), (127, 0x7f)] {
        let encoded = encode_x86_64_selected_u64_less_than_branch_form(
            &physical,
            short_alternative,
            displacement,
        )
        .unwrap();
        assert_eq!(encoded.bytes(), [0x72, encoded_displacement]);
        assert_eq!(encoded.footprint(), near.footprint());
    }
    for displacement in [-129, 128] {
        assert_eq!(
            encode_x86_64_selected_u64_less_than_branch_form(
                &physical,
                short_alternative,
                displacement,
            ),
            Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
        );
    }
    for bytes in [
        &[0x73, 0][..],
        &[0x72][..],
        &[0x72, 0, 0][..],
        &[0x0f, 0x82, 0, 0, 0, 0][..],
    ] {
        assert_eq!(
            validate_x86_64_selected_u64_less_than_branch_form(
                &physical,
                short_alternative,
                0,
                bytes,
            ),
            Err(X86_64SelectedFormEncodingError::MalformedEncoding)
        );
    }
    assert_eq!(
        validate_x86_64_selected_u64_less_than_branch_form(
            &physical,
            short_alternative,
            0,
            &[0x72, 1],
        ),
        Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
    );
}

#[test]
fn signed_less_than_branch_uses_exact_near_and_short_jl_forms() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let near = alternative(MachineAlternativeFamily::ConditionalBranchI64LessThan, 0);
    let short = alternative(MachineAlternativeFamily::ConditionalBranchI64LessThan, 1);

    let encoded = encode_x86_64_selected_i64_less_than_branch_form(&physical, near, -6).unwrap();
    assert_eq!(encoded.bytes(), [0x0f, 0x8c, 0xfa, 0xff, 0xff, 0xff]);
    assert_eq!(
        encoded.footprint().encoded.control,
        MachineEncodedControlEffect::ConditionalRelativeBranchV1
    );
    let encoded = encode_x86_64_selected_i64_less_than_branch_form(&physical, short, -2).unwrap();
    assert_eq!(encoded.bytes(), [0x7c, 0xfe]);
    assert_eq!(
        validate_x86_64_selected_i64_less_than_branch_form(&physical, short, 0, &[0x72, 0],),
        Err(X86_64SelectedFormEncodingError::MalformedEncoding)
    );
    assert_eq!(
        encode_x86_64_selected_i64_less_than_branch_form(&physical, short, 128),
        Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
    );
}
