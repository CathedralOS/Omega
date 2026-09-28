//! One-field mutations of the wrapper object plan.
//!
//! Independently representable substitutions re-authenticate the plan
//! identity, keep a valid canonical envelope, and still fail the joins the
//! owning stage replays: the retained manifest rejects them
//! (`ManifestMismatch`) and `compose_object` derives a different expected
//! object (the staged `staged.object != expected` → `InvalidObject` check).
//! Fields closed by the representation — the canonical target, section name,
//! alignment, joined machine-code bytes, positional/role-bound symbol rows,
//! bound symbols, the fixed wrapper byte count, every resolved call field, and
//! the relocation count — are rejected at encoding as non-canonical
//! (`InvalidObject`), before any custody decision.
//!
//! The representable legs run through the shared
//! `run_one_field_substitution_matrix` driver over the declared
//! `PlanFieldForTest` inventory; the closed-field encode refusals stay
//! authored as explicit loops below.
use super::super::super::{
    OptimizedProgramStorageSemanticWrapperObjectSymbol,
    OptimizedProgramStorageSemanticWrapperObjectSymbolRole,
};
use super::super::{
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectRecordError,
    decode_optimized_program_storage_semantic_wrapper_object,
    encode_optimized_program_storage_semantic_wrapper_object,
};
use super::super::{child, template};
use super::fixture::{foreign_parts, recompose, staged_parts, three_symbol_parts};
use crate::semantic_wrapper_object::manifest::validate_manifest;
use object_file::{ObjectLocalSymbolId, canonical_private_machine_symbol_name};
use optimization_core::{
    MutationOutcome, OneFieldSubstitutionMatrix, OptimizedObjectArtifactIdentity,
    OptimizedObjectArtifactManifestIdentity, OptimizedProgramStorageSemanticWrapperObjectIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
    run_one_field_substitution_matrix,
};
use semantic_vocabulary::MachineId;
use target::{Architecture, ObjectFormat};
use terminal_psi::SemanticFingerprint;

type PlanMutation = fn(&mut OptimizedProgramStorageSemanticWrapperObjectPlan);

optimization_core::custody_field_inventory! {
    /// One independently representable substitution of the wrapper object
    /// plan; checked by the retained manifest replay (`validate_manifest`)
    /// over the honestly re-encoded container, with independent
    /// recomposition as the joined replay.
    pub enum PlanFieldForTest {
        SourceArtifact,
        SourceArtifactManifest,
        SourceObject,
        SourceObjectContainer,
        SourceSignature,
        PsiProgramFingerprint,
        // Wrapper body byte outside the joined call opcode (offset 80) and
        // displacement field (offsets 81..85).
        WrapperBodyByte,
        // Byte between the next-instruction offset (85) and the return
        // offset (89): covered by length/cursor joins but not by a fixed
        // value.
        WrapperTailByte,
        // The copied child byte at the continuation's section offset.
        ContinuationByte,
        // A canonical-shape-preserving roster substitution: an extra
        // zero-byte `PrivateTerminalFunctionV1` row satisfies every symbol
        // join, so only honest re-identification and replay expose it.
        ZeroByteFunctionRowAppended,
    }
}

/// Substitute exactly one plan field with a fixed alternate and
/// re-authenticate the plan identity.
fn substitute_plan_for_test(
    object: &mut OptimizedProgramStorageSemanticWrapperObjectPlan,
    field: PlanFieldForTest,
    _donor: &OptimizedProgramStorageSemanticWrapperObjectPlan,
) {
    match field {
        PlanFieldForTest::SourceArtifact => {
            object.source_artifact =
                OptimizedObjectArtifactIdentity::from_canonical_bytes(b"mutated-artifact")
        }
        PlanFieldForTest::SourceArtifactManifest => {
            object.source_artifact_manifest =
                OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(
                    b"mutated-artifact-manifest",
                )
        }
        PlanFieldForTest::SourceObject => {
            object.source_object =
                RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"mutated-source-object")
        }
        PlanFieldForTest::SourceObjectContainer => {
            object.source_object_container =
                RelocationFreeObjectContainerIdentity::from_canonical_bytes(
                    b"mutated-source-container",
                )
        }
        PlanFieldForTest::SourceSignature => object.source_signature = [0xa7; 32],
        PlanFieldForTest::PsiProgramFingerprint => {
            object.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xa9; 32])
        }
        PlanFieldForTest::WrapperBodyByte => object.text_bytes[0] ^= 0x40,
        PlanFieldForTest::WrapperTailByte => object.text_bytes[88] ^= 0x40,
        PlanFieldForTest::ContinuationByte => object.text_bytes[90] ^= 0x40,
        PlanFieldForTest::ZeroByteFunctionRowAppended => {
            let machine = MachineId::new(8).unwrap();
            object
                .symbols
                .push(OptimizedProgramStorageSemanticWrapperObjectSymbol {
                    symbol: ObjectLocalSymbolId::new(3).unwrap(),
                    source_function_index: Some(1),
                    machine: Some(machine),
                    name: canonical_private_machine_symbol_name(machine),
                    section_offset: 91,
                    byte_count: 0,
                    role:
                        OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalFunctionV1,
                })
        }
    }
    object.identity = object.recomputed_identity().unwrap();
}

#[test]
fn wrapper_object_plan_rejects_every_representable_one_field_substitution() {
    let (_, _, manifest, _) = staged_parts();
    let (foreign, _, _, _) = foreign_parts();
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "wrapper object plan",
        fields: PlanFieldForTest::INVENTORY,
        honest: &|| staged_parts().0,
        donor: foreign,
        // The containing identity: honest re-authentication must move it.
        custody: &|object| object.identity,
        substitute: &substitute_plan_for_test,
        // A representable substitution still encodes; the retained manifest
        // replay over the honestly re-encoded container rejects it.
        check: &|object| {
            let container =
                encode_optimized_program_storage_semantic_wrapper_object(object, &template())?;
            validate_manifest(object, &container, &manifest).map(|()| object.identity)
        },
        outcome: &|_| {
            MutationOutcome::ExactError(
                OptimizedProgramStorageSemanticWrapperObjectRecordError::ManifestMismatch,
            )
        },
        joined_replay: Some(&|object, field| {
            let container =
                encode_optimized_program_storage_semantic_wrapper_object(object, &template())
                    .unwrap_or_else(|error| {
                        panic!(
                            "representable {field:?} must keep a valid object envelope: {error:?}"
                        )
                    });
            assert_eq!(
                decode_optimized_program_storage_semantic_wrapper_object(&container.bytes),
                Ok(object.clone()),
                "canonical codec must round-trip reauthenticated {field:?}",
            );
            assert_ne!(
                recompose(&child()),
                *object,
                "independent object recomposition must differ after substituting {field:?} \
                 (the owning stage rejects this drift as InvalidObject)",
            );
        }),
    });
}

#[test]
fn wrapper_object_plan_rejects_every_closed_field_at_encoding() {
    let (object, _, _, _) = staged_parts();
    let mutations: [(&str, PlanMutation); 34] = [
        ("target.architecture", |object| {
            object.target.architecture = Architecture::Aarch64
        }),
        ("target.object_format", |object| {
            object.target.object_format = ObjectFormat::Elf
        }),
        ("target.pointer_size", |object| {
            object.target.pointer_size = 4
        }),
        ("target.pointer_alignment", |object| {
            object.target.pointer_alignment = 4
        }),
        ("text_section_name", |object| {
            object.text_section_name = ".omega_bad".into()
        }),
        ("text_section_alignment", |object| {
            object.text_section_alignment = 4
        }),
        ("text_bytes[call opcode]", |object| {
            object.text_bytes[80] = 0x90
        }),
        ("text_bytes[displacement byte]", |object| {
            object.text_bytes[81] ^= 1
        }),
        ("text_bytes.truncate", |object| {
            object.text_bytes.truncate(90)
        }),
        ("text_bytes.extend", |object| object.text_bytes.push(0)),
        ("symbols[0].symbol", |object| {
            object.symbols[0].symbol = ObjectLocalSymbolId::new(9).unwrap()
        }),
        ("symbols[0].source_function_index", |object| {
            object.symbols[0].source_function_index = Some(0)
        }),
        ("symbols[0].machine", |object| {
            object.symbols[0].machine = Some(MachineId::new(8).unwrap())
        }),
        ("symbols[0].name", |object| {
            object.symbols[0].name = "w".into()
        }),
        ("symbols[0].section_offset", |object| {
            object.symbols[0].section_offset = 1
        }),
        ("symbols[0].byte_count", |object| {
            object.symbols[0].byte_count = 89
        }),
        ("symbols[0].role→continuation", |object| {
            object.symbols[0].role =
                OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalContinuationV1
        }),
        ("symbols[0].role→function", |object| {
            object.symbols[0].role =
                OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalFunctionV1
        }),
        ("symbols[1].symbol", |object| {
            object.symbols[1].symbol = ObjectLocalSymbolId::new(9).unwrap()
        }),
        ("symbols[1].source_function_index=None", |object| {
            object.symbols[1].source_function_index = None
        }),
        ("symbols[1].source_function_index=9", |object| {
            object.symbols[1].source_function_index = Some(9)
        }),
        ("symbols[1].machine=None", |object| {
            object.symbols[1].machine = None
        }),
        ("symbols[1].machine", |object| {
            object.symbols[1].machine = Some(MachineId::new(8).unwrap())
        }),
        ("symbols[1].name", |object| {
            object.symbols[1].name = "c".into()
        }),
        ("symbols[1].section_offset", |object| {
            object.symbols[1].section_offset = 91
        }),
        ("symbols[1].byte_count", |object| {
            object.symbols[1].byte_count = 2
        }),
        ("symbols[1].role→wrapper", |object| {
            object.symbols[1].role =
                OptimizedProgramStorageSemanticWrapperObjectSymbolRole::SemanticWrapperV1
        }),
        ("symbols[1].role→function", |object| {
            object.symbols[1].role =
                OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalFunctionV1
        }),
        ("wrapper_symbol", |object| {
            object.wrapper_symbol = ObjectLocalSymbolId::new(9).unwrap()
        }),
        ("continuation_symbol", |object| {
            object.continuation_symbol = ObjectLocalSymbolId::new(9).unwrap()
        }),
        ("wrapper_byte_count", |object| {
            object.wrapper_byte_count = 91
        }),
        ("call_resolution.wrapper_section_offset", |object| {
            object.call_resolution.wrapper_section_offset = 1
        }),
        ("call_resolution.continuation_section_offset", |object| {
            object.call_resolution.continuation_section_offset = 91
        }),
        ("call_resolution.displacement", |object| {
            object.call_resolution.displacement = 6
        }),
    ];

    for (field, mutate) in mutations {
        let mut mutated = object.clone();
        mutate(&mut mutated);
        mutated.identity = mutated.recomputed_identity().unwrap();
        assert_eq!(
            encode_optimized_program_storage_semantic_wrapper_object(&mutated, &template()),
            Err(OptimizedProgramStorageSemanticWrapperObjectRecordError::InvalidObject),
            "non-canonical {field} must be rejected at encoding",
        );
    }
}

/// `psi.vocabulary_marker` and `call_resolution.state` are single-valued in
/// memory (`VocabularyMarker::CURRENT` and `ResolvedInCompositeTextSectionV1`
/// are the only constructible values), so their substitution axes exist only
/// on the wire and are covered by `wire.rs`.
#[test]
fn wrapper_object_plan_rejects_every_closed_axis_at_encoding_extended() {
    let (object, _, _, _) = staged_parts();
    let mutations: [(&str, PlanMutation); 8] = [
        (
            "call_resolution.next_instruction_section_offset",
            |object| object.call_resolution.next_instruction_section_offset = 86,
        ),
        ("relocation_record_count", |object| {
            object.relocation_record_count = 1
        }),
        ("symbols.remove(wrapper)", |object| {
            object.symbols.remove(0);
        }),
        ("symbols.remove(continuation)", |object| {
            object.symbols.remove(1);
        }),
        ("symbols.swap", |object| object.symbols.swap(0, 1)),
        ("symbols.push(duplicate wrapper)", |object| {
            let duplicate = object.symbols[0].clone();
            object.symbols.push(duplicate);
        }),
        ("symbols.clear", |object| object.symbols.clear()),
        ("text_bytes.drop wrapper tail", |object| {
            object.text_bytes.truncate(80)
        }),
    ];

    for (field, mutate) in mutations {
        let mut mutated = object.clone();
        mutate(&mut mutated);
        mutated.identity = mutated.recomputed_identity().unwrap();
        assert_eq!(
            encode_optimized_program_storage_semantic_wrapper_object(&mutated, &template()),
            Err(OptimizedProgramStorageSemanticWrapperObjectRecordError::InvalidObject),
            "non-canonical {field} must be rejected at encoding",
        );
    }
}

#[test]
fn wrapper_object_plan_rejects_terminal_function_row_substitutions() {
    let (object, _, _, _) = three_symbol_parts();
    assert_eq!(object.symbols.len(), 3);
    let mutations: [(&str, PlanMutation); 9] = [
        ("symbols[2].symbol", |object| {
            object.symbols[2].symbol = ObjectLocalSymbolId::new(9).unwrap()
        }),
        ("symbols[2].source_function_index", |object| {
            object.symbols[2].source_function_index = Some(0)
        }),
        ("symbols[2].machine=duplicate", |object| {
            object.symbols[2].machine = Some(MachineId::new(7).unwrap())
        }),
        ("symbols[2].machine=rename mismatch", |object| {
            object.symbols[2].machine = Some(MachineId::new(9).unwrap())
        }),
        ("symbols[2].name", |object| {
            object.symbols[2].name = "f".into()
        }),
        ("symbols[2].section_offset", |object| {
            object.symbols[2].section_offset = 90
        }),
        ("symbols[2].byte_count", |object| {
            object.symbols[2].byte_count = 0
        }),
        ("symbols[2].role→continuation", |object| {
            object.symbols[2].role =
                OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalContinuationV1
        }),
        ("symbols.remove(function)", |object| {
            object.symbols.remove(2);
        }),
    ];

    for (field, mutate) in mutations {
        let mut mutated = object.clone();
        mutate(&mut mutated);
        mutated.identity = mutated.recomputed_identity().unwrap();
        assert_eq!(
            encode_optimized_program_storage_semantic_wrapper_object(&mutated, &template()),
            Err(OptimizedProgramStorageSemanticWrapperObjectRecordError::InvalidObject),
            "non-canonical {field} must be rejected at encoding",
        );
    }
}

#[test]
fn stale_object_identity_rejects_at_encoding() {
    let (object, _, _, _) = staged_parts();

    // Substituting the containing identity without touching content is a stale
    // identity, not a reauthenticated record.
    let mut mutated = object.clone();
    mutated.identity =
        OptimizedProgramStorageSemanticWrapperObjectIdentity::from_canonical_bytes(b"stale-object");
    assert_eq!(
        encode_optimized_program_storage_semantic_wrapper_object(&mutated, &template()),
        Err(OptimizedProgramStorageSemanticWrapperObjectRecordError::InvalidObject),
    );

    // Substituting content while retaining the authentic identity is equally
    // stale: the recomputed identity can no longer match.
    let mut mutated = object.clone();
    mutated.source_artifact =
        OptimizedObjectArtifactIdentity::from_canonical_bytes(b"mutated-artifact");
    assert_eq!(
        encode_optimized_program_storage_semantic_wrapper_object(&mutated, &template()),
        Err(OptimizedProgramStorageSemanticWrapperObjectRecordError::InvalidObject),
    );
}
