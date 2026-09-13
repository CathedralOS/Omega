//! The selected value is an existing owner, not a reconstructed constructor.

use super::{membership, produce_source};

#[test]
fn terminal_owned_selection_retains_the_unselected_owner_until_completion() {
    let artifact = produce_source(
        "choose",
        "data Choice { case Empty; case Some; }
         machine choose(selected: bool) -> bool {
             let result: Choice = match selected {
                 true -> Choice::Some, false -> Choice::Empty
             };
             result in Choice::Some
         }",
    );
    let mut module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let machine = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let mut sources = Vec::new();
    let mut construction = Vec::new();
    for block in &mut machine.blocks {
        if let Some(place) = block.operations.iter().find_map(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::EstablishScalarCase { .. }
            )
            .then(|| operation.result.structural().unwrap().place)
        }) {
            sources.push(place);
            construction.append(&mut block.operations);
        }
    }
    assert_eq!(sources.len(), 2);
    let remaining = semantic_vocabulary::PlaceId::new(
        machine
            .structural_places
            .iter()
            .map(|place| place.id.get())
            .max()
            .unwrap()
            + 1,
    )
    .unwrap();
    let join = machine
        .blocks
        .iter_mut()
        .find(|block| !block.structural_parameters.is_empty())
        .unwrap();
    let result = join.structural_parameters[0].clone();
    join.structural_parameters[0].position = 1;
    join.structural_parameters.insert(
        0,
        terminal_psi::StructuralParameterDeclaration {
            place: remaining,
            position: 0,
            ..result.clone()
        },
    );
    let result_declaration = machine
        .structural_places
        .iter_mut()
        .find(|place| place.id == result.place)
        .unwrap();
    result_declaration.kind = semantic_vocabulary::StructuralPlaceKind::BlockParameter {
        block: join.id,
        position: 1,
    };
    machine
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: remaining,
            kind: semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                block: join.id,
                position: 0,
            },
        });
    let cleanup_actions = machine
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            terminal_psi::Terminator::Return {
                cleanup_actions, ..
            } => Some(cleanup_actions),
            _ => None,
        })
        .expect("membership completion owns cleanup");
    assert_eq!(cleanup_actions.len(), 1);
    cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
        remaining,
    ));
    machine
        .blocks
        .iter_mut()
        .find(|block| block.id == machine.entry)
        .unwrap()
        .operations
        .extend(construction);
    let mut selected_edges = 0;
    for block in &mut machine.blocks {
        if let terminal_psi::Terminator::Jump {
            structural_arguments,
            trivial_affine_discards,
            ..
        } = &mut block.terminator
            && let [selected] = structural_arguments.as_slice()
            && sources.contains(&selected.place)
        {
            assert!(trivial_affine_discards.is_empty());
            let unselected = sources
                .iter()
                .copied()
                .find(|source| *source != selected.place)
                .unwrap();
            structural_arguments.insert(
                0,
                terminal_psi::StructuralArgument {
                    place: unselected,
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::Owned,
                },
            );
            selected_edges += 1;
        }
    }
    assert_eq!(selected_edges, 2);
    let artifact = membership::canonical(&module);
    membership::execute(
        &artifact,
        "#include <stdbool.h>\nextern bool omega_entry(bool selected);\nint main(void) { return omega_entry(true) && !omega_entry(false) ? 0 : 1; }",
    );
    for duplicate_transfer in [false, true] {
        let mut changed = module.clone();
        let machine = changed
            .machines
            .iter_mut()
            .find(|machine| machine.id == changed.entry)
            .unwrap();
        let edge = machine
            .blocks
            .iter_mut()
            .find_map(|block| match &mut block.terminator {
                terminal_psi::Terminator::Jump {
                    structural_arguments,
                    trivial_affine_discards,
                    ..
                } if structural_arguments.len() == 2 => {
                    Some((structural_arguments, trivial_affine_discards))
                }
                _ => None,
            })
            .unwrap();
        if duplicate_transfer {
            edge.0[1].place = edge.0[0].place;
        } else {
            edge.1.push(edge.0[0].place);
        }
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &terminal_verifier::ProofBundle::default(),
                &super::AdmissionProfile::default(),
            )
            .is_err(),
            "a duplicated transfer or simultaneously transferred/disposed source must reject"
        );
    }
}

#[test]
fn owned_match_transfers_existing_locals_through_native_execution() {
    let source = include_str!("../../../omega/pass/expressions/owned_match_values/main.omg");
    for (source, oracle) in [
        (
            source.to_owned(),
            "extern bool omega_entry(bool selected);\nint main(void) { return omega_entry(true) && !omega_entry(false) ? 0 : 1; }",
        ),
        (
            source.replace("false -> right", "false -> left"),
            "extern bool omega_entry(bool selected);\nint main(void) { return omega_entry(true) && omega_entry(false) ? 0 : 1; }",
        ),
        (
            source
                .replace("selected: bool", "selected: bool, inner: bool")
                .replace(
                    "true -> left",
                    "true -> match inner { true -> left, false -> right }",
                ),
            "extern bool omega_entry(bool selected, bool inner);\nint main(void) { return omega_entry(true, true) && !omega_entry(true, false) && !omega_entry(false, true) && !omega_entry(false, false) ? 0 : 1; }",
        ),
    ] {
        let artifact = produce_source("choose", &source);
        membership::execute(&artifact, &format!("#include <stdbool.h>\n{oracle}"));
    }
}

#[test]
fn owned_match_mixed_fresh_and_existing_arms_execute_natively() {
    let artifact = produce_source(
        "choose",
        include_str!("../../../omega/pass/expressions/owned_match_mixed_values/main.omg"),
    );
    membership::execute(
        &artifact,
        "#include <stdbool.h>\nextern bool omega_entry(bool selected);\nint main(void) { return omega_entry(true) && !omega_entry(false) ? 0 : 1; }",
    );
}

#[test]
fn owned_match_preserves_the_selected_sources_full_width_payload() {
    let artifact = produce_source(
        "choose",
        "data Choice { case Some(value: u64); }
         machine choose(selected: bool, first: u64, second: u64) -> Choice {
             let left: Choice = Choice::Some { value: first };
             let right: Choice = Choice::Some { value: second };
             let spare: Choice = Choice::Some { value: first };
             match selected { true -> left, false -> right }
         }",
    );
    assert_return_cleanup_replay(&artifact);
    membership::execute(
        &artifact,
        "#include <stdbool.h>\n#include <stdint.h>\nstruct choice { uint32_t tag; uint32_t padding; uint64_t value; };\nextern struct choice omega_entry(bool selected, uint64_t first, uint64_t second);\nint main(void) { const uint64_t first = UINT64_C(0x123456789abcdef0); const uint64_t second = UINT64_MAX; struct choice left = omega_entry(true, first, second); struct choice right = omega_entry(false, first, second); return left.tag == 0 && right.tag == 0 && left.value == first && right.value == second ? 0 : 1; }",
    );
}

fn assert_return_cleanup_replay(artifact: &super::CanonicalTerminalArtifact) {
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &super::AdmissionProfile::default(),
    )
    .expect("selected return enters Omega with verified custody");
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(verified.unit()).unwrap();
    for native in [
        super::NativeTarget::linux_x64(),
        super::NativeTarget::linux_arm64(),
        super::NativeTarget::macos_arm64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            verified.input().plan(),
            native,
        )
        .unwrap();
        target_operations_to_selected_instructions::legalize_target_operations(
            &target,
            verified.input().plan(),
            verified.unit(),
        )
        .unwrap_or_else(|error| panic!("legalize retained owners on {native:?}: {error:?}"));
        for mutation in 0..4 {
            let mut changed = target.clone();
            let (source, cleanup) = changed
                .functions
                .iter_mut()
                .flat_map(|function| &mut function.graph.blocks)
                .find_map(|block| match &mut block.terminator {
                    target_operations::TargetControlTerminator::ReturnStructural {
                        source: target_operations::TargetStructuralReturnSource::Home(home),
                        cleanup_actions,
                        ..
                    } => Some((home.place(), cleanup_actions)),
                    _ => None,
                })
                .unwrap();
            assert_eq!(cleanup.len(), 2);
            match mutation {
                0 => cleanup.clear(),
                1 => cleanup[0] = terminal_psi::TerminalAffineCleanupAction::DiscardRoot(source),
                2 => cleanup.push(cleanup[0].clone()),
                _ => cleanup.swap(0, 1),
            }
            assert!(
                target_operations_to_selected_instructions::legalize_target_operations(
                    &changed,
                    verified.input().plan(),
                    verified.unit()
                )
                .is_err(),
                "target cleanup substitution {mutation} on {native:?}"
            );
        }
    }
    for mutation in 0..4 {
        let mut unit = verified.unit().clone();
        let operation = unit
            .functions
            .iter_mut()
            .flat_map(|function| &mut function.blocks)
            .flat_map(|block| &mut block.nodes)
            .find_map(|node| match &mut node.operation {
                abstract_operations::AbstractOperation::ReturnStructural {
                    source,
                    trivial_affine_discards,
                    ..
                } => Some((*source, trivial_affine_discards)),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            operation.1.len(),
            2,
            "unselected owner and untouched newer local remain owed at return"
        );
        match mutation {
            0 => operation.1.clear(),
            1 => operation.1[0] = operation.0,
            2 => operation.1.push(operation.1[0]),
            _ => operation.1.swap(0, 1),
        }
        unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
        assert!(matches!(optimization_unit_semantics::validate_psi_optimization_unit(&unit),
            Err(optimization_unit_semantics::OptimizationUnitValidationError::CurrentCleanupMismatch { .. })),
            "ordinary structural return must retain exact residual disposal, mutation {mutation}");
    }
}

#[test]
fn owned_selection_transports_an_untouched_record_with_its_own_type() {
    let declarations = "data Choice { case Empty; case Some(value: u32); }
        data Record { payload: u64; }";
    let candidates = "let left: Choice = Choice::Some { value: 37 };
        let right: Choice = Choice::Empty;";
    let record = "let retained: Record = Record { payload: payload };";
    for prefix in [
        format!("{record} {candidates}"),
        format!("{candidates} {record}"),
    ] {
        let artifact = produce_source(
            "choose",
            &format!(
                "{declarations}
            machine choose(selected: bool, payload: u64) -> Record {{
                {prefix}
                let result: Choice = match selected {{ true -> left, false -> right }};
                retained
            }}"
            ),
        );
        assert_return_cleanup_replay(&artifact);
        membership::execute(
            &artifact,
            "#include <stdbool.h>\n#include <stdint.h>\nextern uint64_t omega_entry(bool selected, uint64_t payload);\nint main(void) { return omega_entry(true, UINT64_MAX) == UINT64_MAX && omega_entry(false, UINT64_C(0x123456789abcdef0)) == UINT64_C(0x123456789abcdef0) ? 0 : 1; }",
        );
    }
}
