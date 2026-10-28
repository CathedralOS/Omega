//! Dynamic section byte tests.

use super::{
    Candidate, ValidatedElfDynamicSectionPlan, decode_dynsym, decode_gnu_hash, decode_sysv_hash,
    decode_verneed, decode_versym, encode_payloads,
    non_authoritative_payload_compatibility_fingerprint, read_u16, read_u32,
    serialize_elf_dynamic_sections, validate_candidate, validate_payloads,
};
use crate::bytes::name_at;
use crate::plan_elf_dynamic_sections;
use crate::test_support::{SINGLE_SITE_IMPORTS as IMPORTS, SingleSiteImportFixture};
use target::TargetProfile;

fn interpreter_path(target: TargetProfile) -> &'static [u8] {
    crate::test_support::import_fixture_interpreter_path(target)
}

fn structural_plan(
    target: TargetProfile,
    imports: &[SingleSiteImportFixture],
) -> ValidatedElfDynamicSectionPlan {
    structural_plan_with_interpreter(target, imports, interpreter_path(target))
}

fn structural_plan_with_interpreter(
    target: TargetProfile,
    imports: &[SingleSiteImportFixture],
    interpreter_path: &[u8],
) -> ValidatedElfDynamicSectionPlan {
    let inputs = crate::test_support::single_site_import_inputs(
        target,
        imports,
        "__omega_payload_import_",
        64,
        interpreter_path,
        "valid payload fixture locator",
        "valid payload fixture interpreter",
    );
    plan_elf_dynamic_sections(inputs).expect("valid structural dynamic sections")
}

fn candidate(target: TargetProfile) -> Candidate {
    let plan = structural_plan(target, &IMPORTS);
    let payloads = encode_payloads(plan.contents()).expect("encoded payloads");
    let non_authoritative_payload_compatibility_fingerprint =
        non_authoritative_payload_compatibility_fingerprint(&plan, &payloads);
    Candidate {
        plan,
        payloads,
        non_authoritative_payload_compatibility_fingerprint,
    }
}

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| u32::from_le_bytes(*word))
        .collect()
}

#[test]
fn both_linux_targets_serialize_exact_elf64_lsb_payloads() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let serialized = serialize_elf_dynamic_sections(structural_plan(target, &IMPORTS))
            .expect("validated serialized payloads");
        let payloads = &serialized.payloads;

        assert_eq!(
            payloads.interpreter,
            [interpreter_path(target), &[0]].concat()
        );
        assert_eq!(payloads.dynstr.len(), 64);
        assert_eq!(
            payloads.dynstr,
            b"\0OMEGA_1\xfd\0OMEGA_2\0alpha\xfe\0beta\0gamma\0libalpha\xff.so.1\0libbeta.so.2\0"
        );
        assert_eq!(payloads.dynsym.len(), 120);
        assert_eq!(&payloads.dynsym[..24], &[0; 24]);
        assert_eq!(
            &payloads.dynsym[24..48],
            &[
                18, 0, 0, 0, 0x12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]
        );
        assert_eq!(payloads.sysv_hash.len(), 44);
        assert_eq!(
            words(&payloads.sysv_hash),
            [4, 5, 0, 3, 1, 0, 0, 2, 0, 4, 0]
        );
        assert_eq!(payloads.gnu_hash.len(), 44);
        assert_eq!(
            words(&payloads.gnu_hash),
            [
                1,
                1,
                1,
                5,
                0x0090_2200,
                0x0000_0102,
                1,
                0xf204_f288,
                0xf204_f288,
                0x7c94_89a0,
                0x0f7d_eae9,
            ]
        );
        assert_eq!(payloads.versym, [0, 0, 2, 0, 3, 0, 2, 0, 4, 0]);
        assert_eq!(payloads.verneed.len(), 80);
        assert_eq!(read_u16(&payloads.verneed, 0, "version").unwrap(), 1);
        assert_eq!(read_u16(&payloads.verneed, 2, "count").unwrap(), 2);
        assert_eq!(read_u32(&payloads.verneed, 4, "file").unwrap(), 36);
        assert_eq!(read_u32(&payloads.verneed, 8, "aux").unwrap(), 16);
        assert_eq!(read_u32(&payloads.verneed, 12, "next").unwrap(), 48);
        assert_eq!(read_u16(&payloads.verneed, 22, "other").unwrap(), 2);
        assert_eq!(read_u32(&payloads.verneed, 24, "name").unwrap(), 1);
        assert_eq!(read_u32(&payloads.verneed, 28, "next").unwrap(), 16);
        assert_eq!(read_u16(&payloads.verneed, 38, "other").unwrap(), 3);
        assert_eq!(read_u32(&payloads.verneed, 44, "next").unwrap(), 0);
        assert_eq!(read_u32(&payloads.verneed, 52, "file").unwrap(), 51);
        assert_eq!(read_u16(&payloads.verneed, 70, "other").unwrap(), 4);
        assert_eq!(serialized.dynamic_string_byte_count(), 64);
        assert_eq!(serialized.dynamic_symbol_byte_count(), 120);
        assert_eq!(serialized.system_v_hash_byte_count(), 44);
        assert_eq!(serialized.gnu_hash_byte_count(), 44);
        assert_eq!(serialized.symbol_version_byte_count(), 10);
        assert_eq!(serialized.version_requirement_byte_count(), 80);
        assert_ne!(
            serialized.non_authoritative_payload_compatibility_fingerprint(),
            0
        );
        validate_payloads(serialized.plan(), payloads).expect("independent byte replay");
    }
}

#[test]
fn serialized_payloads_and_identity_ignore_import_insertion_order() {
    let forward =
        serialize_elf_dynamic_sections(structural_plan(TargetProfile::LinuxX64, &IMPORTS))
            .expect("forward payloads");
    let reversed = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
    let reverse =
        serialize_elf_dynamic_sections(structural_plan(TargetProfile::LinuxX64, &reversed))
            .expect("reverse payloads");

    assert_eq!(forward.payloads, reverse.payloads);
    assert_eq!(
        forward.non_authoritative_payload_compatibility_fingerprint(),
        reverse.non_authoritative_payload_compatibility_fingerprint()
    );
}

#[test]
fn payload_identity_binds_profile_and_exact_serialized_coordinates() {
    let baseline =
        serialize_elf_dynamic_sections(structural_plan(TargetProfile::LinuxX64, &IMPORTS))
            .expect("baseline payloads")
            .non_authoritative_payload_compatibility_fingerprint();
    let changed_profile =
        serialize_elf_dynamic_sections(structural_plan(TargetProfile::LinuxArm64, &IMPORTS))
            .expect("changed-profile payloads")
            .non_authoritative_payload_compatibility_fingerprint();
    let changed_interpreter = serialize_elf_dynamic_sections(structural_plan_with_interpreter(
        TargetProfile::LinuxX64,
        &IMPORTS,
        b"/another/ld-linux-x86-64.so.2",
    ))
    .expect("changed-interpreter payloads")
    .non_authoritative_payload_compatibility_fingerprint();
    let mut changed_imports = IMPORTS;
    changed_imports[0] = SingleSiteImportFixture {
        object: b"libalpha\xff.so.1",
        symbol: b"changed_alpha\xfe",
        version: b"OMEGA_1\xfd",
    };
    let changed_coordinate =
        serialize_elf_dynamic_sections(structural_plan(TargetProfile::LinuxX64, &changed_imports))
            .expect("changed-coordinate payloads")
            .non_authoritative_payload_compatibility_fingerprint();

    assert_ne!(baseline, changed_profile);
    assert_ne!(baseline, changed_interpreter);
    assert_ne!(baseline, changed_coordinate);
}

#[test]
fn independent_decoder_rejects_every_payload_and_identity_corruption() {
    let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
        Box::new(|candidate| {
            candidate.payloads.interpreter.pop();
        }),
        Box::new(|candidate| candidate.payloads.interpreter.push(0)),
        Box::new(|candidate| candidate.payloads.dynstr[0] = 1),
        Box::new(|candidate| {
            candidate.payloads.dynstr.pop();
        }),
        Box::new(|candidate| {
            candidate.payloads.dynsym.pop();
        }),
        Box::new(|candidate| candidate.payloads.dynsym.push(0)),
        Box::new(|candidate| candidate.payloads.dynsym[24..28].reverse()),
        Box::new(|candidate| candidate.payloads.dynsym[24] = 0),
        Box::new(|candidate| {
            candidate.payloads.sysv_hash.pop();
        }),
        Box::new(|candidate| candidate.payloads.sysv_hash.push(0)),
        Box::new(|candidate| candidate.payloads.sysv_hash[..4].fill(0)),
        Box::new(|candidate| candidate.payloads.sysv_hash[4..8].fill(0xff)),
        Box::new(|candidate| candidate.payloads.sysv_hash[8..12].fill(0xff)),
        Box::new(|candidate| {
            candidate.payloads.sysv_hash[28..32].copy_from_slice(&1u32.to_le_bytes())
        }),
        Box::new(|candidate| {
            candidate.payloads.gnu_hash.pop();
        }),
        Box::new(|candidate| candidate.payloads.gnu_hash.push(0)),
        Box::new(|candidate| candidate.payloads.gnu_hash[..4].fill(0)),
        Box::new(|candidate| candidate.payloads.gnu_hash[4..8].fill(0)),
        Box::new(|candidate| candidate.payloads.gnu_hash[8..12].fill(0xff)),
        Box::new(|candidate| candidate.payloads.gnu_hash[12..16].fill(0xff)),
        Box::new(|candidate| candidate.payloads.gnu_hash[16] ^= 1),
        Box::new(|candidate| candidate.payloads.gnu_hash[24..28].fill(0xff)),
        Box::new(|candidate| candidate.payloads.gnu_hash[28] ^= 2),
        Box::new(|candidate| candidate.payloads.gnu_hash[40] &= !1),
        Box::new(|candidate| {
            candidate.payloads.versym.pop();
        }),
        Box::new(|candidate| candidate.payloads.versym.push(0)),
        Box::new(|candidate| candidate.payloads.versym[2] = 1),
        Box::new(|candidate| {
            candidate.payloads.verneed.pop();
        }),
        Box::new(|candidate| candidate.payloads.verneed.push(0)),
        Box::new(|candidate| candidate.payloads.verneed[2..4].fill(0xff)),
        Box::new(|candidate| candidate.payloads.verneed[8..12].fill(0)),
        Box::new(|candidate| candidate.payloads.verneed[12..16].fill(0)),
        Box::new(|candidate| candidate.payloads.verneed[28..32].fill(0)),
        Box::new(|candidate| candidate.non_authoritative_payload_compatibility_fingerprint ^= 1),
    ];

    for corrupt in corruptions {
        let mut candidate = candidate(TargetProfile::LinuxX64);
        corrupt(&mut candidate);
        let error = validate_candidate(candidate)
            .expect_err("corrupt bytes must reject before sealing payloads");
        assert_eq!(
            error.candidate.plan.inputs().interpreter().target(),
            TargetProfile::LinuxX64,
            "decoder rejection retains exact structural-plan custody",
        );
    }
}

#[test]
fn adversarial_lengths_counts_indexes_and_offsets_reject_without_panicking() {
    for bytes in [Vec::new(), vec![0; 7], vec![0xff; 8], vec![0; 12]] {
        assert!(decode_sysv_hash(&bytes).is_err());
    }
    let out_of_range_hash = [1u32, 2, u32::MAX, 0, 0].map(u32::to_le_bytes).concat();
    assert!(decode_sysv_hash(&out_of_range_hash).is_err());
    let cyclic_hash = [1u32, 3, 1, 0, 1, 0].map(u32::to_le_bytes).concat();
    assert!(decode_sysv_hash(&cyclic_hash).is_err());

    for bytes in [
        Vec::new(),
        vec![0; 15],
        vec![0xff; 16],
        vec![0; 20],
        vec![0; 44],
    ] {
        assert!(decode_gnu_hash(&bytes, 5).is_err());
    }
    let mut out_of_range_gnu = candidate(TargetProfile::LinuxX64).payloads.gnu_hash;
    out_of_range_gnu[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode_gnu_hash(&out_of_range_gnu, 5).is_err());
    let mut unterminated_gnu = candidate(TargetProfile::LinuxX64).payloads.gnu_hash;
    unterminated_gnu[40] &= !1;
    assert!(decode_gnu_hash(&unterminated_gnu, 5).is_err());

    for bytes in [Vec::new(), vec![0; 15], vec![0xff; 16], vec![0; 32]] {
        assert!(decode_verneed(&bytes, 1).is_err());
    }
    let mut cyclic_auxiliary = candidate(TargetProfile::LinuxX64).payloads.verneed;
    cyclic_auxiliary[28..32].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode_verneed(&cyclic_auxiliary, 2).is_err());

    assert!(decode_dynsym(&[0; 23], 1).is_err());
    assert!(decode_versym(&[0], 1).is_err());
    assert!(name_at(&[0], u32::MAX, "ELF dynamic string reference").is_err());
}
