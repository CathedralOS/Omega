use super::*;

#[test]
fn source_continuations_retain_distinct_result_owners_and_ordered_residuals() {
    for boundary in [false, true] {
        for (fields, projection, expected_count) in [
            ("left: Token; right: Token;", "right", 1),
            ("grid: [[Token; 2]; 2]; tail: Token;", "grid[0][1]", 3),
            ("grid: [[Token; 2]; 2]; tail: Token;", "grid[1]", 2),
            ("right: Token;", "right", 0),
        ] {
            let parameters = if boundary {
                ""
            } else {
                "first: Pair, second: Pair"
            };
            let reach = if boundary { "reaches Factory" } else { "" };
            let first = if boundary {
                "Factory::create()"
            } else {
                "Root::forward(first)"
            };
            let second = if boundary {
                "Factory::create()"
            } else {
                "Root::forward(second)"
            };
            let consumer_type = if projection == "grid[1]" {
                "[Token; 2]"
            } else {
                "Token"
            };
            let mut source = format!(
                "pub data Token {{ value: u64; }}
                 pub data Pair {{ {fields} }}
                 data Sink {{}}
                 machine Sink::take(value: {consumer_type}) {{}}
                 machine Sink::done() {{}}
                 data Root {{}}
                 machine Root::forward(value: Pair) -> Pair {{ value }}
                 machine Root::enter({parameters}) {reach} {{
                     Sink::take({first}.{projection});
                     Sink::take({second}.{projection});
                     Sink::done();
                 }}"
            );
            if boundary {
                source.push_str(
                    "boundary trait Factory { machine create() -> Pair reaches Factory; }",
                );
            }
            let tokens = Lexer::new(&source).tokenize().unwrap();
            let syntax = parse_syntax_trees(&tokens).unwrap();
            let resolved = lower_syntax_trees(&syntax).unwrap();
            let typed = lower_symbol_resolved_trees(&resolved).unwrap();
            let checked = lower_typed_trees(typed).unwrap();
            let terminal =
                checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").unwrap();
            let semantic = encode_module(&terminal.semantic_module).unwrap();
            let proof = encode_proof_bundle(&terminal.proof_bundle).unwrap();
            let input = lower_artifact_sections_for_optimization(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
            )
            .unwrap();
            let verified = build_verified_psi_optimization_unit(
                input,
                terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
            )
            .unwrap();
            validate_psi_optimization_unit(verified.unit())
                .unwrap_or_else(|error| panic!("{source}\n{error:?}"));
            let caller = verified
                .unit()
                .functions
                .iter()
                .find(|function| function.machine == terminal.semantic_module.entry)
                .unwrap();
            let terminal_caller = terminal
                .semantic_module
                .machines
                .iter()
                .find(|machine| machine.id == caller.machine)
                .unwrap();
            let mut cleanup_edges = Vec::new();
            for block in &terminal_caller.blocks {
                let terminal_psi::Terminator::Jump {
                    edge,
                    residual_affine_discards: expected,
                    ..
                } = &block.terminator
                else {
                    continue;
                };
                let node = caller.blocks.iter().flat_map(|block| &block.nodes).find(|node| matches!(node.operation, AbstractOperation::Jump { psi_edge, .. } if psi_edge == *edge)).unwrap();
                let AbstractOperation::Jump {
                    residual_affine_discards,
                    ..
                } = &node.operation
                else {
                    unreachable!()
                };
                assert_eq!(residual_affine_discards, expected);
                assert_eq!(node.successors[0].residual_affine_discards, *expected);
                assert_eq!(expected.len(), expected_count);
                cleanup_edges.push((*edge, expected.clone()));
            }
            assert_eq!(cleanup_edges.len(), 2);
            if expected_count != 0 {
                assert_ne!(cleanup_edges[0].1[0].place, cleanup_edges[1].1[0].place);
                for mutation in 0..4 {
                    let mut changed = verified.unit().clone();
                    let caller = changed
                        .functions
                        .iter_mut()
                        .find(|function| function.machine == changed.entry)
                        .unwrap();
                    let node = caller.blocks.iter_mut().flat_map(|block| &mut block.nodes).find(|node| matches!(node.operation, AbstractOperation::Jump { psi_edge, .. } if psi_edge == cleanup_edges[0].0)).unwrap();
                    let AbstractOperation::Jump {
                        residual_affine_discards,
                        ..
                    } = &mut node.operation
                    else {
                        unreachable!()
                    };
                    match mutation {
                        0 => residual_affine_discards.clear(),
                        1 => node.successors[0].residual_affine_discards.clear(),
                        2 => {
                            *residual_affine_discards = cleanup_edges[1].1.clone();
                            node.successors[0].residual_affine_discards =
                                residual_affine_discards.clone();
                        }
                        3 => {
                            residual_affine_discards.push(residual_affine_discards[0].clone());
                            node.successors[0].residual_affine_discards =
                                residual_affine_discards.clone();
                        }
                        _ => unreachable!(),
                    }
                    changed.identity =
                        optimization_unit::recompute_psi_optimization_unit_identity(&changed);
                    assert!(
                        validate_psi_optimization_unit(&changed).is_err(),
                        "mutation {mutation} must reject after identity refresh"
                    );
                }
                if !boundary {
                    for target in [
                        target::NativeTarget::linux_x64(),
                        target::NativeTarget::linux_arm64(),
                    ] {
                        let native =
                            abstract_operations_to_target_operations::lower_to_target_operations(
                                verified.input().plan(),
                                target,
                            );
                        if fields == "left: Token; right: Token;" {
                            native
                                .expect("direct-register results retain both native continuations");
                        } else {
                            assert!(
                                native.is_err(),
                                "wider structural results retain their ABI fence"
                            );
                        }
                    }
                }
            }
        }
    }
}
