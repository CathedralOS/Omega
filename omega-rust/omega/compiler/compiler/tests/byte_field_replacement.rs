//! Authored bounded replacement retains live length and caller-owned backing.

use native_realization::{compiler_baseline_request_v1, optimize_artifact_sections};
use optimization_core::OptimizationSelections;
use proof_admission::AdmissionProfile;
use target::NativeTarget;

#[path = "byte_field_replacement/replay.rs"]
mod replay;

#[path = "byte_field_replacement/indexed.rs"]
mod indexed;

// Reuse the host linker/executor without modifying the shared differential owner.
#[path = "../../../../../tests/native-differential/tests/common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

const CASES: [(&str, bool); 4] = [("", false), ("X", false), ("XYZ", false), ("X", true)];

fn replacement(literal: &str, nested: bool) -> lowered_psi::LoweredPsi {
    let (fields, selected) = if nested {
        ("payload: Payload;", "payload.text")
    } else {
        ("text: [u8; 3] in Utf8;", "text")
    };
    let source = format!(
        r#"
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        data Payload {{ text: [u8; 3] in Utf8; sibling: u64; }}
        data Record {{ before: u64; {fields} after: u64; }}
        machine Record::replace(&mut self) {{ self.{selected} = "{literal}"; }}
        "#
    );
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokenize bounded replacement");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse replacement");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve replacement");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type replacement");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("replacement must establish the destination Utf8 predicate");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Record::replace")
        .expect("authored bounded replacement must reach Terminal");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent destination capacity and source content evidence");
    assert!(lowered.semantic_module.machines.iter().any(|machine| {
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| {
                matches!(&operation.kind,
                terminal_psi::OperationKind::StructuralByteSequenceFieldStore { path, .. }
                if path.len() == usize::from(nested))
            })
    }));
    lowered
}

fn publish(
    lowered: &lowered_psi::LoweredPsi,
    target: NativeTarget,
) -> (image_emission::ExecutableImage, usize) {
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    let selections = OptimizationSelections::new([]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .expect("canonical replacement artifact independently verifies");
    let physical =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .expect("bounded byte replacement reaches native physical realization");
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let framed = machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
    let placed = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(placed).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    let entry_offset = object.entry_function().text_offset;
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    (image, entry_offset)
}

/// Hand-authored Terminal exercises the already specified runtime replacement
/// contract. Authored Psi runtime-view assignment remains a separate dependency.
fn runtime_replacement(nested: bool, write_only: bool) -> lowered_psi::LoweredPsi {
    use proof_admission::{
        CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
    };
    use semantic_vocabulary::{
        BlockId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue, ObligationId,
        OperationId, PlaceId, ScalarType, StructuralPlaceKind, ValueId,
    };
    use terminal_psi::{
        Block, ByteSequenceCarrier, Operation, OperationKind, OperationResult, StructuralAccess,
        StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
        StructuralTypeShape, SuccessorEdge, Terminator, ValueDeclaration,
    };

    // The source producer also lacks bounded write-only assignment. Construct
    // that specified Terminal access explicitly; do not claim source coverage.
    let mut lowered = replacement("", nested);
    let view_type = lowered
        .semantic_module
        .structural_types
        .iter()
        .find_map(|declaration| {
            matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
            )
            .then_some(declaration.id)
        })
        .expect("literal retained its byte-view carrier");
    let machine = &mut lowered.semantic_module.machines[0];
    if write_only {
        machine.structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    }
    let (destination, path, field) = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::StructuralByteSequenceFieldStore {
                destination,
                path,
                field,
                ..
            } => Some((*destination, path.clone(), *field)),
            _ => None,
        })
        .expect("source template supplies exact destination declaration");
    let source = PlaceId::new(1001).unwrap();
    let length = ValueId::new(1001).unwrap();
    let capacity = ValueId::new(1002).unwrap();
    let fits = ValueId::new(1003).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    assert!(machine.parameters.is_empty());
    machine
        .structural_places
        .retain(|place| place.id == destination);
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: source,
            position: 1,
            is_self: false,
            structural_type: view_type,
            access: StructuralAccess::SharedBorrow,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: source,
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let successor = |edge, block| SuccessorEdge {
        erased_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(block).unwrap(),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let operation = |identity, value, scalar_type, kind| Operation {
        static_reach_binding: None,
        id: OperationId::new(identity).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value,
            scalar_type,
        }),
        kind,
    };
    machine.entry = BlockId::new(1001).unwrap();
    machine.blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            id: machine.entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![
                operation(
                    1001,
                    length,
                    scalar,
                    OperationKind::ByteSequenceLength { source },
                ),
                operation(
                    1002,
                    capacity,
                    scalar,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(3),
                    },
                ),
                operation(
                    1003,
                    fits,
                    ScalarType::Boolean,
                    OperationKind::IntegerLessOrEqual {
                        left: length,
                        right: capacity,
                    },
                ),
            ],
            terminator: Terminator::Conditional {
                condition: fits,
                when_true: successor(1001, 1002),
                when_false: successor(1002, 1003),
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            id: BlockId::new(1002).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: OperationId::new(1004).unwrap(),
                result: OperationResult::Unit,
                kind: OperationKind::StructuralByteSequenceFieldStore {
                    destination,
                    path,
                    field,
                    source,
                    length,
                    obligation: ObligationId::new(1001).unwrap(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1003).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            id: BlockId::new(1003).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1004).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        },
    ];
    let reconstructed =
        terminal_verifier::reconstruct_terminal_obligations(&lowered.semantic_module).unwrap();
    let [question] = reconstructed.obligations() else {
        panic!("one runtime capacity obligation");
    };
    let axiom = question
        .semantic_axioms
        .iter()
        .position(|axiom| *axiom == question.obligation.proposition)
        .expect("selected true edge establishes the exact declaration-derived capacity bound");
    lowered.proof_bundle = terminal_verifier::ProofBundle {
        evidence: vec![terminal_verifier::ObligationEvidence {
            obligation: question.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1001).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: question.obligation.proposition.clone(),
                    rule: ProofRule::SemanticAxiom { index: axiom },
                },
            }),
        }],
        ..Default::default()
    };
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("runtime view replacement independently proves capacity from its guard");
    let mut wrong_edge = lowered.semantic_module.clone();
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &mut wrong_edge.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    std::mem::swap(when_true, when_false);
    assert!(
        terminal_verifier::verify_module(
            &wrong_edge,
            &lowered.proof_bundle,
            &AdmissionProfile::default()
        )
        .is_err(),
        "false guard cannot reuse the true guard's capacity proof"
    );
    lowered
}

#[test]
fn runtime_view_replacement_publishes_on_four_targets() {
    for (nested, write_only) in [(false, false), (true, true)] {
        let lowered = runtime_replacement(nested, write_only);
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let (image, _) = publish(&lowered, target);
            assert!(!image.output().final_text_bytes.is_empty());
        }
    }
}

#[test]
fn runtime_view_replacement_reads_only_live_bytes_and_preserves_source() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    for (nested, write_only) in [(false, false), (true, true)] {
        let (image, entry_offset) = publish(
            &runtime_replacement(nested, write_only),
            NativeTarget::host(),
        );
        let (fields, selected, sibling_check) = if nested {
            (
                "struct Payload payload;",
                "payload.text",
                "if (record.payload.sibling != saved.payload.sibling) return 4;",
            )
        } else {
            ("struct Text text;", "text", "")
        };
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry_offset,
            &format!(
                r#"
            #include <stdint.h>
            #include <string.h>
            #include <unistd.h>
            #include <sys/mman.h>
            struct Text {{ uint64_t length; uint8_t bytes[3]; }};
            struct Payload {{ struct Text text; uint64_t sibling; }};
            struct Record {{ uint64_t before; {fields} uint64_t after; }};
            struct View {{ const uint8_t *data; uint64_t length; }};
            extern void omega_entry(struct Record *destination, const struct View *source);
            int main(void) {{
                alarm(10);
                long page = sysconf(_SC_PAGESIZE);
                if (page <= 0) return 10;
                uint8_t *backing = mmap(0, (size_t)page * 2, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0);
                if (backing == MAP_FAILED) return 11;
                memcpy(backing + page - 4, "WXYZ", 4);
                if (mprotect(backing, (size_t)page, PROT_READ) || mprotect(backing + page, (size_t)page, PROT_NONE)) return 12;
                const unsigned lengths[] = {{0, 1, 3, 4}};
                for (unsigned trial = 0; trial < 4; ++trial) {{
                    unsigned length = lengths[trial];
                    struct View view = {{length ? backing + page - length : 0, length}};
                    struct Record record;
                    memset(&record, 0xa7, sizeof(record));
                    record.{selected}.length = 3;
                    memcpy(record.{selected}.bytes, "old", 3);
                    const struct Record saved = record;
                    for (unsigned repetition = 0; repetition < 2; ++repetition) {{
                        omega_entry(&record, &view);
                        if (length > 3) {{
                            if (memcmp(&record, &saved, sizeof(record))) return 5;
                        }} else {{
                            if (record.{selected}.length != length) return 1;
                            if (length && memcmp(record.{selected}.bytes, view.data, length)) return 2;
                            if (record.before != saved.before || record.after != saved.after) return 3;
                            {sibling_check}
                            if (memcmp(record.{selected}.bytes + length, saved.{selected}.bytes + length, 3 - length)) return 6;
                        }}
                        if (memcmp(backing + page - 4, "WXYZ", 4)) return 7;
                        if (view.length != length || view.data != (length ? backing + page - length : 0)) return 8;
                    }}
                }}
                return munmap(backing, (size_t)page * 2) ? 13 : 0;
            }}
        "#
            ),
        );
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!(
        "SKIP runtime byte-view guard-page execution: requires Linux x64/AArch64 or macOS AArch64; four-target publication is separate"
    );
}

#[test]
fn bounded_replacement_source_reaches_verified_terminal() {
    for (literal, nested) in CASES {
        replacement(literal, nested);
    }
}

#[test]
fn bounded_replacement_publishes_on_four_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for (literal, nested) in CASES {
            let (image, _) = publish(&replacement(literal, nested), target);
            assert!(!image.output().final_text_bytes.is_empty());
        }
    }
}

#[test]
fn bounded_replacement_mutates_original_field_and_preserves_neighbors() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    for (literal, nested) in CASES {
        let (image, entry_offset) = publish(&replacement(literal, nested), NativeTarget::host());
        let (fields, selected, sibling_check) = if nested {
            (
                "struct Payload payload;",
                "payload.text",
                "if (record.payload.sibling != saved.payload.sibling) return 4;",
            )
        } else {
            ("struct Text text;", "text", "")
        };
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry_offset,
            &format!(
                r#"
                    #include <stdint.h>
                    #include <string.h>
                    #include <unistd.h>
                    struct Text {{ uint64_t length; uint8_t bytes[3]; }};
                    struct Payload {{ struct Text text; uint64_t sibling; }};
                    struct Record {{ uint64_t before; {fields} uint64_t after; }};
                    extern void omega_entry(struct Record *destination);
                    int main(void) {{
                        alarm(10);
                        struct Record record;
                        memset(&record, 0xa7, sizeof(record));
                        record.{selected}.length = 3;
                        memcpy(record.{selected}.bytes, "old", 3);
                        const struct Record saved = record;
                        for (unsigned repetition = 0; repetition < 2; ++repetition) {{
                            omega_entry(&record);
                            if (record.{selected}.length != {length}) return 1;
                            if (memcmp(record.{selected}.bytes, "{literal}", {length})) return 2;
                            if (record.before != saved.before || record.after != saved.after) return 3;
                            {sibling_check}
                        }}
                        return 0;
                    }}
                    "#,
                length = literal.len(),
            ),
        );
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    eprintln!(
        "SKIP replacement runtime: C/text harness supports Linux x64/AArch64 and macOS AArch64; four-target publication is separate"
    );
}
