//! Dynamic section tests.

use super::{
    Candidate, ElfDynamicSectionContents, ElfDynamicSymbol, ElfSysvHash,
    PlannedElfDynamicLinkInputs, SHN_UNDEF, STB_GLOBAL, STT_FUNC, VER_NEED_CURRENT, build_gnu_hash,
    derive_contents, elf_hash, gnu_hash, plan_elf_dynamic_sections, validate_candidate,
    validate_contents, validate_gnu_hash_reachability, validate_hash_reachability,
};
use crate::image::{
    FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
    FinalImageSection, FinalImageSymbol,
};
use crate::image_elf::plan_elf_dynamic_link_inputs;
use crate::object_file::{RelocationKind, SymbolKind};
use arena::Handle;
use target::{
    ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_elf_interpreter_plan,
    normalize_foreign_locator,
};

#[derive(Clone, Copy)]
struct ImportFixture {
    object: &'static [u8],
    symbol: &'static [u8],
    version: &'static [u8],
}

const IMPORTS: [ImportFixture; 4] = [
    ImportFixture {
        object: b"libalpha\xff.so.1",
        symbol: b"alpha\xfe",
        version: b"OMEGA_1\xfd",
    },
    ImportFixture {
        object: b"libalpha\xff.so.1",
        symbol: b"beta",
        version: b"OMEGA_1\xfd",
    },
    ImportFixture {
        object: b"libalpha\xff.so.1",
        symbol: b"alpha\xfe",
        version: b"OMEGA_2",
    },
    ImportFixture {
        object: b"libbeta.so.2",
        symbol: b"gamma",
        version: b"OMEGA_1\xfd",
    },
];

fn interpreter_path(target: TargetProfile) -> &'static [u8] {
    match target {
        TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2",
        TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1",
        _ => unreachable!("dynamic section fixture uses a Linux target"),
    }
}

fn input(target: TargetProfile, imports: &[ImportFixture]) -> PlannedElfDynamicLinkInputs {
    input_with_interpreter(target, imports, interpreter_path(target))
}

fn input_with_interpreter(
    target: TargetProfile,
    imports: &[ImportFixture],
    interpreter_path: &[u8],
) -> PlannedElfDynamicLinkInputs {
    let native_target = target.native_target();
    let mut image = FinalImage::with_capacity(
        native_target,
        FinalImageMemory {
            text: vec![0; 64],
            ..FinalImageMemory::default()
        },
        Handle::invalid(),
        imports.len(),
        imports.len(),
        imports.len(),
    );
    for (index, fixture) in imports.iter().enumerate() {
        let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: format!("__omega_dynamic_import_{index}"),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle,
            import: FinalImageImportPlan::Normalized(
                normalize_foreign_locator(
                    ForeignLocatorCandidate::ElfVersioned {
                        object: fixture.object.to_vec(),
                        symbol: fixture.symbol.to_vec(),
                        version: fixture.version.to_vec(),
                    },
                    target,
                )
                .expect("valid ELF fixture locator"),
            ),
        });
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section: FinalImageSection::Text,
                offset: index * 8,
                byte_width: 4,
                symbol_handle,
                addend: 0,
                kind: match native_target {
                    target if target == NativeTarget::linux_arm64() => {
                        RelocationKind::Aarch64Branch26
                    }
                    _ => RelocationKind::X86_64Relative32,
                },
            });
    }
    let interpreter = normalize_elf_interpreter_plan(interpreter_path.to_vec(), target)
        .expect("valid ELF fixture interpreter");
    plan_elf_dynamic_link_inputs(image, interpreter).expect("valid dynamic-link preflight")
}

fn candidate(target: TargetProfile) -> Candidate {
    let inputs = input(target, &IMPORTS);
    let contents = derive_contents(&inputs).expect("derived dynamic sections");
    Candidate { inputs, contents }
}

fn string_at(dynstr: &[u8], offset: u32) -> &[u8] {
    let suffix = &dynstr[offset as usize..];
    let end = suffix
        .iter()
        .position(|byte| *byte == 0)
        .expect("NUL-terminated dynamic string");
    &suffix[..end]
}

#[test]
fn both_linux_targets_plan_complete_address_free_table_relationships() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let plan = plan_elf_dynamic_sections(input(target, &IMPORTS))
            .expect("validated address-free dynamic sections");
        let contents = &plan.contents;

        assert_eq!(
            contents.interpreter,
            [interpreter_path(target), &[0]].concat()
        );
        assert_eq!(contents.dynstr.first(), Some(&0));
        assert_eq!(contents.dynstr.last(), Some(&0));
        assert_eq!(
            contents.dynstr,
            b"\0OMEGA_1\xfd\0OMEGA_2\0alpha\xfe\0beta\0gamma\0libalpha\xff.so.1\0libbeta.so.2\0"
        );
        assert_eq!(contents.dynsym.len(), IMPORTS.len() + 1);
        assert_eq!(contents.dynsym[0], ElfDynamicSymbol::default());
        assert_eq!(
            contents
                .dynsym
                .iter()
                .map(|symbol| symbol.name)
                .collect::<Vec<_>>(),
            [0, 18, 18, 25, 30],
        );
        assert!(contents.dynsym[1..].iter().all(|symbol| {
            symbol.info == (STB_GLOBAL << 4) | STT_FUNC
                && symbol.other == 0
                && symbol.section_index == SHN_UNDEF
                && symbol.value == 0
                && symbol.size == 0
                && !string_at(&contents.dynstr, symbol.name).is_empty()
        }));
        assert_eq!(contents.versym.len(), contents.dynsym.len());
        assert_eq!(contents.versym[0], 0);
        assert!(contents.versym[1..].iter().all(|version| *version >= 2));
        assert_eq!(
            contents.sysv_hash.chain_count as usize,
            contents.dynsym.len()
        );
        assert_eq!(contents.gnu_hash.bucket_count, 1);
        assert_eq!(contents.gnu_hash.symbol_offset, 1);
        assert_eq!(contents.gnu_hash.bloom_count, 1);
        assert_eq!(contents.gnu_hash.bloom_shift, 5);
        assert_eq!(contents.gnu_hash.bloom, [0x0000_0102_0090_2200]);
        assert_eq!(contents.gnu_hash.buckets, [1]);
        assert_eq!(
            contents.gnu_hash.chains,
            [0xf204_f288, 0xf204_f288, 0x7c94_89a0, 0x0f7d_eae9]
        );
        assert_eq!(contents.needed.len(), 2);
        assert_eq!(contents.needed, [36, 51]);
        assert_eq!(contents.verneed.len(), 2);
        assert_eq!(plan.required_version_count(), 3);
        assert!(contents.verneed.iter().all(|need| {
            need.version == VER_NEED_CURRENT
                && need.count as usize == need.auxiliaries.len()
                && string_at(&contents.dynstr, need.file).starts_with(b"lib")
                && need.auxiliaries.iter().all(|auxiliary| {
                    auxiliary.hash == elf_hash(string_at(&contents.dynstr, auxiliary.name))
                        && auxiliary.flags == 0
                        && auxiliary.other >= 2
                })
        }));
        validate_contents(plan.inputs(), contents).expect("independent table replay");
        assert_ne!(
            plan.non_authoritative_content_compatibility_fingerprint(),
            0
        );
    }
}

#[test]
fn canonical_table_contents_and_identity_ignore_import_insertion_order() {
    let forward =
        plan_elf_dynamic_sections(input(TargetProfile::LinuxX64, &IMPORTS)).expect("forward plan");
    let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
    let reverse = plan_elf_dynamic_sections(input(TargetProfile::LinuxX64, &reverse_imports))
        .expect("reverse plan");

    assert_eq!(forward.contents.interpreter, reverse.contents.interpreter);
    assert_eq!(forward.contents.dynstr, reverse.contents.dynstr);
    assert_eq!(forward.contents.dynsym, reverse.contents.dynsym);
    assert_eq!(forward.contents.sysv_hash, reverse.contents.sysv_hash);
    assert_eq!(forward.contents.gnu_hash, reverse.contents.gnu_hash);
    assert_eq!(forward.contents.versym, reverse.contents.versym);
    assert_eq!(forward.contents.verneed, reverse.contents.verneed);
    assert_eq!(forward.contents.needed, reverse.contents.needed);
    assert_eq!(
        forward.non_authoritative_content_compatibility_fingerprint(),
        reverse.non_authoritative_content_compatibility_fingerprint()
    );
}

#[test]
fn content_identity_binds_profile_interpreter_and_exact_table_coordinates() {
    let baseline = plan_elf_dynamic_sections(input(TargetProfile::LinuxX64, &IMPORTS))
        .expect("baseline plan")
        .non_authoritative_content_compatibility_fingerprint();
    let changed_interpreter = plan_elf_dynamic_sections(input_with_interpreter(
        TargetProfile::LinuxX64,
        &IMPORTS,
        b"/another/ld-linux-x86-64.so.2",
    ))
    .expect("changed-interpreter plan")
    .non_authoritative_content_compatibility_fingerprint();
    let changed_profile = plan_elf_dynamic_sections(input(TargetProfile::LinuxArm64, &IMPORTS))
        .expect("changed-profile plan")
        .non_authoritative_content_compatibility_fingerprint();
    let mut changed_imports = IMPORTS;
    changed_imports[0] = ImportFixture {
        object: b"libalpha\xff.so.1",
        symbol: b"alpha_changed\xfe",
        version: b"OMEGA_1\xfd",
    };
    let changed_coordinate =
        plan_elf_dynamic_sections(input(TargetProfile::LinuxX64, &changed_imports))
            .expect("changed-coordinate plan")
            .non_authoritative_content_compatibility_fingerprint();

    assert_ne!(baseline, changed_interpreter);
    assert_ne!(baseline, changed_profile);
    assert_ne!(baseline, changed_coordinate);
}

#[test]
fn shared_objects_versions_and_strings_deduplicate_without_losing_import_rows() {
    let plan = plan_elf_dynamic_sections(input(TargetProfile::LinuxX64, &IMPORTS))
        .expect("validated plan");
    let contents = &plan.contents;
    let alpha_version_indexes = contents
        .bindings
        .iter()
        .filter_map(|binding| {
            let symbol = &contents.dynsym[binding.dynamic_symbol_index as usize];
            (string_at(&contents.dynstr, symbol.name) == b"alpha\xfe")
                .then_some(binding.version_index)
        })
        .collect::<Vec<_>>();

    assert_eq!(contents.bindings.len(), IMPORTS.len());
    assert_eq!(alpha_version_indexes.len(), 2);
    assert_ne!(alpha_version_indexes[0], alpha_version_indexes[1]);
    assert_eq!(contents.verneed[0].auxiliaries.len(), 2);
    assert_eq!(contents.verneed[1].auxiliaries.len(), 1);
    assert_eq!(contents.needed.len(), 2);
}

#[test]
fn independent_validation_rejects_every_cross_table_corruption() {
    let mut corruptions: Vec<Box<dyn Fn(&mut ElfDynamicSectionContents)>> = vec![
        Box::new(|contents| {
            contents.interpreter.pop();
        }),
        Box::new(|contents| contents.dynstr[0] = 1),
        Box::new(|contents| {
            contents.dynstr.pop();
        }),
        Box::new(|contents| contents.dynstr[1] ^= 1),
        Box::new(|contents| contents.dynsym[1].name = 0),
        Box::new(|contents| contents.sysv_hash.buckets[0] = u32::MAX),
        Box::new(|contents| contents.sysv_hash.chains[1] = u32::MAX),
        Box::new(|contents| contents.gnu_hash.bucket_count = 0),
        Box::new(|contents| contents.gnu_hash.symbol_offset = 0),
        Box::new(|contents| contents.gnu_hash.bloom_count = 0),
        Box::new(|contents| contents.gnu_hash.bloom_shift += 1),
        Box::new(|contents| contents.gnu_hash.bloom[0] ^= 1u64 << 9),
        Box::new(|contents| contents.gnu_hash.buckets[0] = u32::MAX),
        Box::new(|contents| contents.gnu_hash.chains[0] ^= 2),
        Box::new(|contents| contents.gnu_hash.chains[3] &= !1),
        Box::new(|contents| contents.versym[1] = 1),
        Box::new(|contents| contents.verneed[0].count += 1),
        Box::new(|contents| contents.verneed[0].auxiliary_offset = 0),
        Box::new(|contents| contents.verneed[0].next_offset = 0),
        Box::new(|contents| contents.verneed[0].auxiliaries[0].hash ^= 1),
        Box::new(|contents| contents.verneed[0].auxiliaries[0].next_offset = 0),
        Box::new(|contents| contents.needed[0] = 0),
        Box::new(|contents| contents.bindings[0].dynamic_symbol_index += 1),
        Box::new(|contents| contents.non_authoritative_content_compatibility_fingerprint ^= 1),
    ];

    for corrupt in corruptions.drain(..) {
        let mut candidate = candidate(TargetProfile::LinuxX64);
        corrupt(&mut candidate.contents);
        let error = validate_candidate(candidate)
            .expect_err("corrupt candidate must reject before sealing");
        assert_eq!(
            error.candidate.inputs.interpreter().target(),
            TargetProfile::LinuxX64,
            "validation rejection retains exact input custody",
        );
    }
}

#[test]
fn system_v_hash_matches_known_generic_abi_vectors() {
    assert_eq!(elf_hash(b""), 0x0000_0000);
    assert_eq!(elf_hash(b"exit"), 0x0006_cf04);
    assert_eq!(elf_hash(b"printf"), 0x0779_05a6);
    assert_eq!(elf_hash(b"memcpy"), 0x073c_3a79);
}

#[test]
fn gnu_hash_matches_known_vectors_and_exact_canonical_chain() {
    assert_eq!(gnu_hash(b""), 0x0000_1505);
    assert_eq!(gnu_hash(b"exit"), 0x7c96_7e3f);
    assert_eq!(gnu_hash(b"printf"), 0x156b_2bb8);
    assert_eq!(gnu_hash(b"memcpy"), 0x0d82_7590);

    let hash = build_gnu_hash(&[
        b"alpha\xfe".as_slice(),
        b"alpha\xfe".as_slice(),
        b"beta".as_slice(),
        b"gamma".as_slice(),
    ])
    .expect("canonical GNU hash");
    assert_eq!(hash.bloom, [0x0000_0102_0090_2200]);
    assert_eq!(hash.buckets, [1]);
    assert_eq!(
        hash.chains,
        [0xf204_f288, 0xf204_f288, 0x7c94_89a0, 0x0f7d_eae9]
    );
    validate_gnu_hash_reachability(
        &hash,
        &[
            b"alpha\xfe".as_slice(),
            b"alpha\xfe".as_slice(),
            b"beta".as_slice(),
            b"gamma".as_slice(),
        ],
    )
    .expect("exact GNU hash reachability");
}

#[test]
fn malformed_hash_counts_reject_without_panicking() {
    let mut zero_buckets = candidate(TargetProfile::LinuxX64);
    zero_buckets.contents.sysv_hash.bucket_count = 0;
    zero_buckets.contents.sysv_hash.buckets.clear();
    assert!(validate_candidate(zero_buckets).is_err());

    let mut mismatched_buckets = candidate(TargetProfile::LinuxX64);
    mismatched_buckets.contents.sysv_hash.bucket_count += 1;
    assert!(validate_candidate(mismatched_buckets).is_err());

    let mut mismatched_chains = candidate(TargetProfile::LinuxX64);
    mismatched_chains.contents.sysv_hash.chain_count = 0;
    mismatched_chains.contents.sysv_hash.chains.clear();
    assert!(validate_candidate(mismatched_chains).is_err());

    let names = [b"symbol".as_slice()];
    for malformed in [
        ElfSysvHash {
            bucket_count: 0,
            chain_count: 2,
            buckets: Vec::new(),
            chains: vec![0, 0],
        },
        ElfSysvHash {
            bucket_count: 1,
            chain_count: 0,
            buckets: vec![1],
            chains: Vec::new(),
        },
        ElfSysvHash {
            bucket_count: 1,
            chain_count: 2,
            buckets: vec![u32::MAX],
            chains: vec![0, 0],
        },
    ] {
        assert!(validate_hash_reachability(&malformed, &names).is_err());
    }
    let cyclic = ElfSysvHash {
        bucket_count: 1,
        chain_count: 3,
        buckets: vec![1],
        chains: vec![0, 1, 0],
    };
    assert!(
        validate_hash_reachability(&cyclic, &[b"first".as_slice(), b"second".as_slice()],).is_err()
    );

    let names = [b"symbol".as_slice()];
    let mut malformed_gnu = build_gnu_hash(&names).expect("valid GNU hash");
    malformed_gnu.bloom.clear();
    assert!(validate_gnu_hash_reachability(&malformed_gnu, &names).is_err());
    let mut malformed_gnu = build_gnu_hash(&names).expect("valid GNU hash");
    malformed_gnu.buckets[0] = u32::MAX;
    assert!(validate_gnu_hash_reachability(&malformed_gnu, &names).is_err());
    let mut malformed_gnu = build_gnu_hash(&names).expect("valid GNU hash");
    malformed_gnu.chains[0] &= !1;
    assert!(validate_gnu_hash_reachability(&malformed_gnu, &names).is_err());
}
