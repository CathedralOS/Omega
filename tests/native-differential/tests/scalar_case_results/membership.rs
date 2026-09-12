//! Source observation and source-free consumers retain real source-produced constructors.
use super::*;
use semantic_vocabulary::{
    OperationId, PlaceId, ScalarType, StructuralCaseId, StructuralPlaceKind, ValueId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralMultiplicity,
    TerminalMachineResult, Terminator, ValueDeclaration,
};

fn boolean(identity: u64) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(identity).unwrap(),
        scalar_type: ScalarType::Boolean,
        qualifications: Default::default(),
    }
}

fn canonical(module: &terminal_psi::TerminalModule) -> CanonicalTerminalArtifact {
    let proof = terminal_verifier::ProofBundle::default();
    terminal_verifier::verify_module(module, &proof, &AdmissionProfile::default()).unwrap();
    let record =
        terminal_codec::build_identity_optimization_execution_record(module, &proof).unwrap();
    let artifact = CanonicalTerminalArtifact::from_parts(module, &proof, &record, None).unwrap();
    let artifact = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    terminal_verifier::verify_module(
        &terminal_codec::decode_module(artifact.semantic_bytes()).unwrap(),
        &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    artifact
}

fn constructor_observations(
    observed_ordinal: usize,
    return_owner: bool,
) -> CanonicalTerminalArtifact {
    let original = produce("choose");
    let mut module = terminal_codec::decode_module(original.semantic_bytes()).unwrap();
    let machine = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let owner = machine.result.structural().unwrap().structural_type;
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == owner)
        .unwrap();
    let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
        unreachable!()
    };
    let case = cases[observed_ordinal].id;
    let mut next_operation = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .map(|operation| operation.id.get())
        .max()
        .unwrap()
        + 1;
    let mut next_value = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| operation.result.scalar())
        .map(|result| result.id.get())
        .chain(
            machine
                .parameters
                .iter()
                .map(|parameter| parameter.id.get()),
        )
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.parameters)
                .map(|parameter| parameter.id.get()),
        )
        .max()
        .unwrap()
        + 100;
    if !return_owner {
        machine.result = TerminalMachineResult::Scalar(boolean(next_value));
        next_value += 1;
        machine
            .structural_places
            .retain(|place| place.kind != StructuralPlaceKind::Result);
    }
    for block in &mut machine.blocks {
        let Terminator::ReturnStructural { source, edge, .. } = block.terminator else {
            continue;
        };
        for _ in 0..2 {
            block.operations.push(Operation {
                id: OperationId::new(next_operation).unwrap(),
                result: OperationResult::Scalar(boolean(next_value)),
                kind: OperationKind::StructuralCaseMembership { source, case },
            });
            next_operation += 1;
            next_value += 1;
        }
        if !return_owner {
            let result = block
                .operations
                .iter()
                .find_map(|operation| {
                    operation
                        .result
                        .structural()
                        .filter(|result| result.place == source)
                })
                .unwrap();
            block.terminator = Terminator::Return {
                edge,
                value: ValueId::new(next_value - 1).unwrap(),
                cleanup_actions: if result.multiplicity == StructuralMultiplicity::Affine {
                    vec![terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                        source,
                    )]
                } else {
                    Vec::new()
                },
            };
        }
    }
    canonical(&module)
}

fn parameter_observations(access: StructuralAccess) -> CanonicalTerminalArtifact {
    let original = produce("choose");
    let mut module = terminal_codec::decode_module(original.semantic_bytes()).unwrap();
    let machine = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let owner = machine.result.structural().unwrap().structural_type;
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == owner)
        .unwrap();
    let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
        unreachable!()
    };
    let case = cases[1].id;
    let source = PlaceId::new(1).unwrap();
    machine.parameters.clear();
    machine.structural_parameters = vec![terminal_psi::StructuralParameterDeclaration {
        place: source,
        position: 0,
        is_self: false,
        structural_type: owner,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    machine.structural_places = vec![terminal_psi::StructuralPlaceDeclaration {
        id: source,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    machine.result = TerminalMachineResult::Scalar(boolean(3));
    machine.blocks.retain(|block| block.id == machine.entry);
    let block = &mut machine.blocks[0];
    block.parameters.clear();
    block.structural_parameters.clear();
    block.operations = (1..=2)
        .map(|identity| Operation {
            id: OperationId::new(identity).unwrap(),
            result: OperationResult::Scalar(boolean(identity)),
            kind: OperationKind::StructuralCaseMembership { source, case },
        })
        .collect();
    block.terminator = Terminator::Return {
        edge: semantic_vocabulary::EdgeId::new(1).unwrap(),
        value: ValueId::new(2).unwrap(),
        cleanup_actions: Vec::new(),
    };
    canonical(&module)
}

fn execute(artifact: &CanonicalTerminalArtifact, driver: &str) {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let (image, offset) = publish(artifact, NativeTarget::host());
        native_function::assert_c_text(&image.output().final_text_bytes, offset, driver);
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        let _ = (artifact, driver);
        eprintln!("SKIP: case membership runtime requires Linux x64/ARM64 or macOS ARM64");
    }
}

#[test]
fn source_borrowed_case_membership_reaches_native_execution() {
    let artifact = produce_source(
        "observe",
        "data Choice { case Empty; case Some(value: u32); }
         machine observe(choice: &Choice) -> bool { choice in Choice::Empty }",
    );
    execute(
        &artifact,
        "#include <stdint.h>\n#include <stdbool.h>\nstruct choice { uint32_t tag; uint32_t value; };\nextern bool omega_entry(const struct choice *);\nint main(void) { for (uint32_t tag=0; tag<2; ++tag) { struct choice value = {tag, UINT32_MAX}; for (unsigned repeat=0; repeat<3; ++repeat) { if (omega_entry(&value) != (tag == 0)) return 1; if (value.tag != tag || value.value != UINT32_MAX) return 2; } } return 0; }",
    );
}

#[test]
fn native_case_membership_observes_every_tag_and_preserves_returned_payload() {
    for ordinal in 0..4 {
        let artifact = constructor_observations(ordinal, false);
        let driver = format!(
            "#include <stdint.h>\n#include <stdbool.h>\nextern bool omega_entry(uint64_t, uint64_t);\nint main(void) {{ for (uint64_t tag=0; tag<4; ++tag) {{ if (omega_entry(tag, UINT64_MAX) != (tag == {ordinal})) return 1; }} return 0; }}"
        );
        execute(&artifact, &driver);
    }
    execute(&constructor_observations(1, true), include_str!("choose.c"));
}

#[test]
fn native_case_membership_reads_owned_and_readable_parameter_storage() {
    for access in [
        StructuralAccess::Owned,
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let borrowed = access != StructuralAccess::Owned;
        let signature = if borrowed {
            "const struct outcome *"
        } else {
            "struct outcome"
        };
        let argument = if borrowed { "&value" } else { "value" };
        let driver = format!(
            "#include <stdint.h>\n#include <stdbool.h>\nstruct outcome {{ uint32_t tag; uint32_t padding; uint64_t count; }};\nextern bool omega_entry({signature});\nint main(void) {{ for (uint32_t tag=0; tag<4; ++tag) {{ struct outcome value = {{tag, UINT32_MAX, UINT64_MAX}}; if (omega_entry({argument}) != (tag == 1)) return 1; if (value.tag != tag || value.count != UINT64_MAX) return 2; }} return 0; }}"
        );
        execute(&parameter_observations(access), &driver);
    }
}

#[test]
fn native_case_membership_replay_rejects_source_case_result_and_access_substitution() {
    let artifact = parameter_observations(StructuralAccess::SharedBorrow);
    let selections = OptimizationSelections::new([]).unwrap();
    let optimized = optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    let compiled = abstract_operations_to_target_operations::lower_optimized_to_target_operations(
        optimized,
        NativeTarget::macos_arm64(),
    )
    .unwrap();
    let legalized = target_operations_to_selected_instructions::legalize_target_operations(
        compiled.target_operations(),
        compiled.optimized().plan(),
        compiled.optimized(),
    )
    .unwrap();
    for mutation in 0..6 {
        let mut proposed = legalized.plan().clone();
        let function = &mut proposed.scalar_functions[0];
        let row = &mut function.blocks[0].instructions[0];
        let legalized_operations::LegalizedScalarInstructionKind::StructuralCaseMembership {
            source,
            case,
            case_tag,
        } = &mut row.kind
        else {
            unreachable!()
        };
        match mutation {
            0 => *source = PlaceId::new(999_999).unwrap(),
            1 => *case = StructuralCaseId::new(999_999).unwrap(),
            2 => *case_tag = 0,
            3 => {
                row.result.as_mut().unwrap().scalar_type = ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        32,
                    )
                    .unwrap(),
                )
            }
            4 => {
                function.structural.as_mut().unwrap().parameters[0]
                    .semantic
                    .access = StructuralAccess::WriteOnlyBorrow
            }
            _ => {
                function.structural.as_mut().unwrap().parameters[0]
                    .semantic
                    .structural_type = semantic_vocabulary::StructuralTypeId::new(999_999).unwrap()
            }
        }
        assert!(
            target_operations_to_selected_instructions::validate_legalized_operations(
                compiled.target_operations(),
                compiled.optimized().plan(),
                compiled.optimized(),
                proposed,
            )
            .is_err(),
            "membership substitution {mutation}"
        );
    }
}

#[test]
fn native_case_membership_unit_rejects_wrong_owner_write_only_and_unavailable_subject() {
    use abstract_operations::AbstractOperation as O;
    let selections = OptimizationSelections::new([]).unwrap();
    let artifact = parameter_observations(StructuralAccess::SharedBorrow);
    let optimized = optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    let validate = |plan: &abstract_operations::AbstractOperationPlan| {
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            plan,
            optimized.unit().fuel_schedule,
        )
        .unwrap();
        optimization_unit_semantics::validate_psi_optimization_unit(&unit)
    };
    let mut baseline = optimized.plan().clone();
    let owner = baseline.functions[0].structural_parameters[0].structural_type;
    let mut foreign = baseline
        .structural_types
        .iter()
        .find(|declaration| declaration.id == owner)
        .unwrap()
        .clone();
    foreign.id = semantic_vocabulary::StructuralTypeId::new(999_999).unwrap();
    foreign.identity = "membership.foreign-owner".into();
    let terminal_psi::StructuralTypeShape::Sum { cases } = &mut foreign.shape else {
        unreachable!()
    };
    for (ordinal, case) in cases.iter_mut().enumerate() {
        case.id = StructuralCaseId::new(999_000 + ordinal as u64).unwrap();
        case.fields.clear();
    }
    let foreign_case = cases[1].id;
    // Keep the same case spelling under a different nominal owner. Unknown IDs
    // alone would not establish that lookup uses owner identity.
    baseline.structural_types.make_mut().push(foreign);
    validate(&baseline).unwrap();
    for mutation in 0..4 {
        let mut proposed = baseline.clone();
        let function = &mut proposed.functions[0];
        if mutation == 0 {
            function.structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
        } else {
            let O::StructuralCaseMembership {
                source,
                case,
                result,
                ..
            } = &mut function.operations[0]
            else {
                unreachable!()
            };
            match mutation {
                1 => *case = foreign_case,
                2 => *source = PlaceId::new(999_999).unwrap(),
                _ => {
                    result.scalar_type = ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            32,
                        )
                        .unwrap(),
                    )
                }
            }
        }
        assert!(
            validate(&proposed).is_err(),
            "unit membership mutation {mutation}"
        );
    }

    let artifact = constructor_observations(1, false);
    let optimized = optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    let baseline = optimized.plan();
    validate(baseline).unwrap();
    for mutation in 0..5 {
        let mut proposed = baseline.clone();
        let function = &mut proposed.functions[0];
        let constructors = function
            .operations
            .iter()
            .enumerate()
            .filter_map(|(index, operation)| match operation {
                O::EstablishScalarCase { result, .. } => Some((index, result.place)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let observed = function
            .operations
            .iter()
            .position(|operation| matches!(operation, O::StructuralCaseMembership { .. }))
            .unwrap();
        match mutation {
            0 => function.operations.swap(constructors[0].0, observed),
            1 => {
                let O::StructuralCaseMembership { source, .. } = &mut function.operations[observed]
                else {
                    unreachable!()
                };
                *source = constructors[1].1;
            }
            _ => {
                let cleanup = function
                    .operations
                    .iter_mut()
                    .find_map(|operation| match operation {
                        O::Return {
                            cleanup_actions, ..
                        } if !cleanup_actions.is_empty() => Some(cleanup_actions),
                        _ => None,
                    })
                    .unwrap();
                match mutation {
                    2 => cleanup.clear(),
                    3 => cleanup.push(cleanup[0].clone()),
                    _ => {
                        cleanup[0] = terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                            constructors[1].1,
                        )
                    }
                }
            }
        }
        assert!(
            validate(&proposed).is_err(),
            "constructor availability/cleanup mutation {mutation}"
        );
    }
}

#[test]
fn native_case_membership_selection_replays_tag_read_comparison_and_fuel() {
    use selected_instructions::SelectedInstructionKind as I;
    use target_operations_to_selected_instructions::{
        selection_constraints, stage_optimized_instruction_selection,
        validate_selected_instructions,
    };
    let selections = OptimizationSelections::new([]).unwrap();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let artifact = parameter_observations(StructuralAccess::SharedBorrow);
        let optimized = optimize_artifact_sections(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&selections),
        )
        .unwrap();
        let compiled =
            abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                optimized, target,
            )
            .unwrap();
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let staged = stage_optimized_instruction_selection(compiled, environment).unwrap();
        let environment = staged.register_environment();
        let constraints = selection_constraints(staged.legalized(), environment);
        let validate = |raw| {
            validate_selected_instructions(
                staged.legalized(),
                &constraints,
                environment.physical(),
                environment.constraints(),
                raw,
            )
        };
        validate(staged.selected().plan().clone()).unwrap();
        for row in staged.legalized().plan().scalar_functions[0].blocks[0].instructions.iter()
            .filter(|row| matches!(row.kind, legalized_operations::LegalizedScalarInstructionKind::StructuralCaseMembership { .. })) {
            assert!(!row.fuel.is_empty());
            let charged = staged.selected().plan().functions[0].blocks.iter().flat_map(|block| &block.instructions)
                .filter(|instruction| instruction.provenance.operations.contains(&row.operation))
                .flat_map(|instruction| instruction.provenance.fuel.iter().cloned()).collect::<Vec<_>>();
            assert_eq!(charged, row.fuel, "one charge per observation");
        }
        for mutation in 0..6 {
            let mut proposed = staged.selected().plan().clone();
            let instructions = proposed.functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions);
            match mutation {
                0 | 1 | 4 | 5 => {
                    let read = instructions
                        .into_iter()
                        .find(|instruction| {
                            matches!(instruction.kind, I::Load32 { .. })
                                && !instruction.provenance.operations.is_empty()
                        })
                        .unwrap();
                    match mutation {
                        0 => read.kind = I::Load32 { byte_offset: 4 },
                        1 => read.operands.swap(0, 1),
                        4 => read.provenance.fuel.clear(),
                        _ => read.provenance.operations.clear(),
                    }
                }
                2 => {
                    let constant = instructions
                        .into_iter()
                        .find(|instruction| matches!(instruction.kind, I::MaterializeI64 { .. }))
                        .unwrap();
                    constant.kind = I::MaterializeI64 {
                        value: semantic_vocabulary::IntegerValue::Unsigned(0),
                    };
                }
                _ => {
                    let comparison = instructions
                        .into_iter()
                        .find(|instruction| matches!(instruction.kind, I::MaterializeBooleanEqual))
                        .unwrap();
                    comparison.kind = I::MaterializeBooleanU64LessThan;
                }
            }
            assert!(
                validate(proposed).is_err(),
                "selected membership mutation {mutation} on {target:?}"
            );
        }
    }
}
