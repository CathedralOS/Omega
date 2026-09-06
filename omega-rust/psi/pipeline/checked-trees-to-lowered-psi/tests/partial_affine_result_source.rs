use checked_trees::{
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment,
};
use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalStructuralValue,
};
use terminal_psi::{OperationKind, OperationResult, StructuralPathSegment, Terminator};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data Token { value: u64; }
    data Pair { left: Token; right: Token; }
    data Sink {}
    machine Sink::take(token: Token) {}
    data Root {}
    machine Root::forward(value: Pair) -> Pair { value }
    machine Root::enter(value: Pair) {
        let result: Pair = Root::forward(value);
        Sink::take(result.right);
    }
"#;

#[test]
fn authored_result_projection_retains_its_untransferred_remainder() {
    let checked = checked(SOURCE);
    let _artifact = terminal_production::produce_terminal_artifact(&checked, "Root::enter")
        .expect("projected call result and its residual cleanup publish");
}

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn checked(source: &str) -> checked_trees::CheckedTrees {
    lower_typed_trees(typed(source)).unwrap_or_else(|errors| panic!("{source}\n{errors:#?}"))
}

fn source(boundary: bool, nested: bool, body: &str) -> String {
    let mut source = SOURCE.replace("Sink::take(result.right);", body);
    if nested {
        source = source
            .replace(
                "data Pair { left: Token; right: Token; }",
                "data Pair { left: Token; grid: [[Token; 3]; 2]; tail: Token; }",
            )
            .replace(
                "data Sink {}",
                "data Sink {} machine Sink::take_row(row: [Token; 3]) {}",
            );
    }
    if boundary {
        source = source
            .replace("data Token", "pub data Token")
            .replace("data Pair", "pub data Pair")
            .replace(
                "machine Root::enter(value: Pair)",
                "machine Root::enter() reaches Factory",
            )
            .replace(
                "let result: Pair = Root::forward(value);",
                "let result: Pair = Factory::create();",
            );
        source.push_str("boundary trait Factory { machine create() -> Pair reaches Factory; }");
    }
    source
}

fn path(parts: &[&str]) -> Vec<StructuralPathSegment> {
    parts
        .iter()
        .map(|part| match part.parse::<u64>() {
            Ok(index) => StructuralPathSegment::FixedIndex(index),
            Err(_) => StructuralPathSegment::Field((*part).into()),
        })
        .collect()
}

#[derive(Default)]
struct Factory {
    calls: usize,
}

impl TerminalEffectHandler for Factory {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        panic!("structural producer response required")
    }
    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            result: terminal_psi::BoundaryMachineResult::Structural(result),
            ..
        } = effect
        else {
            panic!("only source factory boundary runs")
        };
        assert!(arguments.is_empty());
        assert!(structural_arguments.is_empty());
        self.calls += 1;
        Ok(TerminalEffectResult::Structural(TerminalStructuralValue {
            opaque_identity: 123,
            structural_type: result.structural_type,
            qualifications: result.qualifications.clone(),
            path: Vec::new(),
        }))
    }
}

fn assert_source(
    source: &str,
    boundary: bool,
    moved: &[Vec<StructuralPathSegment>],
    residuals: &[Vec<StructuralPathSegment>],
) {
    let checked = checked(source);
    let root = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap();
    let state = &checked.machine_states(root)[0];
    let locals = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local) => Some(local),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(locals.len(), 1);
    assert_eq!(locals[0].name.as_str(), "result");
    assert!(!locals[0].is_mutable);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("source result residuals lower");
    let module = &lowered.semantic_module;
    let semantic = encode_module(module).unwrap();
    assert_eq!(decode_module(&semantic).unwrap(), *module);
    let proof = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    assert_eq!(decode_proof_bundle(&proof).unwrap(), lowered.proof_bundle);
    let published =
        terminal_production::produce_terminal_artifact(&checked, "Root::enter").unwrap();
    assert_eq!(decode_module(published.semantic_bytes()).unwrap(), *module);
    let verified = terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
    if boundary {
        let mut missing_reach = module.clone();
        let entry = missing_reach
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        assert!(!entry.published_service_ceiling.is_empty());
        entry.published_service_ceiling.clear();
        assert!(
            terminal_verifier::verify_module(
                &missing_reach,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err(),
            "result cleanup cannot erase its producer's declared service reach"
        );
    }
    let certificate =
        terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, module.entry).unwrap();
    terminal_fixed_fuel::validate_fixed_entry_fuel(&verified, &certificate).unwrap();
    let fuel = 2 * moved.len() as u64 + if boundary { 2 } else { 3 };
    assert_eq!(certificate.ceiling_units(), fuel);
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(caller.blocks.len(), 1);
    let block = &caller.blocks[0];
    assert_eq!(block.operations.len(), moved.len() + 1);
    let OperationResult::Structural(result) = &block.operations[0].result else {
        panic!("exact producer result")
    };
    assert_eq!(
        matches!(block.operations[0].kind, OperationKind::BoundaryCall { .. }),
        boundary
    );
    if !boundary {
        assert!(matches!(
            block.operations[0].kind,
            OperationKind::CallStructuralWithScalarArguments { .. }
        ));
        assert_ne!(result.place, caller.structural_parameters[0].place);
    }
    let actual_moves = block.operations[1..]
        .iter()
        .map(|operation| {
            let OperationKind::CallUnit {
                structural_arguments,
                ..
            } = &operation.kind
            else {
                panic!("projected Unit disposer")
            };
            assert_eq!(structural_arguments.len(), 1);
            assert_eq!(structural_arguments[0].place, result.place);
            structural_arguments[0].path.clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual_moves, moved,
        "authored disposal order survives result binding"
    );
    let cleanup = match &block.terminator {
        Terminator::ReturnUnitPartialAffine {
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } => {
            assert!(trivial_affine_discards.is_empty());
            assert!(
                residual_affine_discards
                    .iter()
                    .all(|discard| discard.place == result.place)
            );
            assert_eq!(
                residual_affine_discards
                    .iter()
                    .map(|discard| discard.path.clone())
                    .collect::<Vec<_>>(),
                residuals
            );
            residual_affine_discards.clone()
        }
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } => {
            assert!(residuals.is_empty());
            assert!(trivial_affine_discards.is_empty());
            Vec::new()
        }
        _ => panic!("normal exact residual return"),
    };
    let expected_frontier = cleanup
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let arguments = if boundary {
        Vec::new()
    } else {
        vec![TerminalStructuralValue {
            opaque_identity: 123,
            structural_type: caller.structural_parameters[0].structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }]
    };
    let mut reference = None;
    for incremental in [false, true] {
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &arguments,
        )
        .unwrap();
        let mut factory = Factory::default();
        let mut meter = if incremental {
            TerminalFuelMeter::with_allowance(0)
        } else {
            TerminalFuelMeter::unbounded()
        };
        let mut complete = false;
        let mut paused_return = false;
        let mut call_order = Vec::new();
        for _ in 0..256 {
            match execution
                .resume_with_effect_handler(&mut meter, &mut factory)
                .unwrap()
            {
                TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
                    assert!(incremental);
                    if exhaustion.site == FuelChargeSite::Edge(block.terminator.edge()) {
                        assert_eq!(
                            execution
                                .live_affine_frontier()
                                .cloned()
                                .collect::<std::collections::BTreeSet<_>>(),
                            expected_frontier
                        );
                        assert!(
                            meter
                                .usage()
                                .at(FuelChargeSite::Edge(block.terminator.edge()))
                                .is_none()
                        );
                        paused_return = true;
                    }
                    if let FuelChargeSite::Operation(operation) = exhaustion.site
                        && block.operations[1..]
                            .iter()
                            .any(|candidate| candidate.id == operation)
                    {
                        call_order.push(operation);
                    }
                    meter.replenish(1).unwrap();
                }
                TerminalExecutionStatus::Complete(value) => {
                    assert_eq!(value, TerminalExecutionResult::Unit);
                    complete = true;
                    break;
                }
                status => panic!("unexpected {status:?}"),
            }
        }
        assert!(complete);
        assert_eq!(paused_return, incremental);
        if incremental {
            assert_eq!(
                call_order,
                block.operations[1..]
                    .iter()
                    .map(|operation| operation.id)
                    .collect::<Vec<_>>()
            );
        }
        assert_eq!(factory.calls, usize::from(boundary));
        assert!(execution.live_affine_frontier().next().is_none());
        assert_eq!(meter.usage().total_units(), fuel);
        if let Some((effects, usage)) = &reference {
            assert_eq!(execution.effects(), effects);
            assert_eq!(meter.usage(), usage);
        } else {
            reference = Some((execution.effects().to_vec(), meter.usage().clone()));
        }
    }
}

#[test]
fn ordinary_and_boundary_results_support_partial_and_complete_field_moves() {
    for boundary in [false, true] {
        assert_source(
            &source(boundary, false, "Sink::take(result.right);"),
            boundary,
            &[path(&["right"])],
            &[path(&["left"])],
        );
        assert_source(
            &source(
                boundary,
                false,
                "Sink::take(result.right); Sink::take(result.left);",
            ),
            boundary,
            &[path(&["right"]), path(&["left"])],
            &[],
        );
    }
}

#[test]
fn projected_result_rows_and_leaves_keep_maximal_reverse_residuals() {
    for boundary in [false, true] {
        assert_source(
            &source(
                boundary,
                true,
                "Sink::take(result.grid[1][1]); Sink::take(result.left);",
            ),
            boundary,
            &[path(&["grid", "1", "1"]), path(&["left"])],
            &[
                path(&["tail"]),
                path(&["grid", "1", "2"]),
                path(&["grid", "1", "0"]),
                path(&["grid", "0"]),
            ],
        );
        assert_source(
            &source(
                boundary,
                true,
                "Sink::take_row(result.grid[1]); Sink::take(result.grid[0][1]);",
            ),
            boundary,
            &[path(&["grid", "1"]), path(&["grid", "0", "1"])],
            &[
                path(&["tail"]),
                path(&["grid", "0", "2"]),
                path(&["grid", "0", "0"]),
                path(&["left"]),
            ],
        );
    }
}

#[test]
fn checked_result_root_paths_and_complement_rejoin_authored_custody() {
    for boundary in [false, true] {
        let original = checked(&source(boundary, false, "Sink::take(result.right);"));
        checked_trees_to_lowered_psi::lower_machine(&original, "Root::enter")
            .expect("valid control before mutations");
        let machine = original
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Root::enter")
            .unwrap()
            .symbol;
        for mutation in 0..6 {
            let mut changed = original.clone();
            let plan = changed
                .facts
                .flow
                .terminal_partial_affine_unit_cleanups
                .machines
                .iter_mut()
                .find(|plan| plan.machine.machine == machine)
                .unwrap();
            match mutation {
                0 => {
                    plan.residual_affine_discards[0].source =
                        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
                }
                1 => {
                    plan.residual_affine_discards[0].source =
                        CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: 1,
                        }
                }
                2 => {
                    plan.residual_affine_discards.pop();
                }
                3 => plan.residual_affine_discards[0]
                    .type_identity
                    .push_str("::wrong"),
                4 => {
                    let operation = plan
                        .machine
                        .operations
                        .iter_mut()
                        .find(|operation| {
                            matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
                        })
                        .unwrap();
                    let CheckedUnitEffectOperationPlan::CallUnit {
                        structural_arguments,
                        ..
                    } = operation
                    else {
                        unreachable!()
                    };
                    structural_arguments[0].path =
                        vec![CheckedUnitStructuralPathSegment::Field("left".into())];
                    plan.residual_affine_discards[0].path =
                        vec![CheckedUnitStructuralPathSegment::Field("right".into())];
                }
                5 => {
                    let operation = plan
                        .machine
                        .operations
                        .iter_mut()
                        .find(|operation| {
                            matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
                        })
                        .unwrap();
                    let CheckedUnitEffectOperationPlan::CallUnit {
                        structural_arguments,
                        ..
                    } = operation
                    else {
                        unreachable!()
                    };
                    structural_arguments[0].source =
                        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 };
                }
                _ => unreachable!(),
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err(),
                "boundary={boundary} mutation={mutation}"
            );
        }
        let mut changed = checked(&source(boundary, true, "Sink::take(result.grid[1][1]);"));
        checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter")
            .expect("valid multi-residual control");
        let plan = changed
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .machines
            .iter_mut()
            .find(|plan| !plan.residual_affine_discards.is_empty())
            .unwrap();
        plan.residual_affine_discards.reverse();
        assert!(checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err());
    }
}

#[test]
fn source_result_paths_cannot_be_used_after_their_owned_move() {
    for boundary in [false, true] {
        for body in [
            "Sink::take(result.right); Sink::take(result.right);",
            "Sink::take(result.right); let again: Pair = Root::forward(result);",
            "let again: Pair = Root::forward(result); Sink::take(result.left);",
        ] {
            let source = source(boundary, false, body);
            if let Ok(checked) = lower_typed_trees(typed(&source)) {
                assert!(
                    terminal_production::produce_terminal_artifact(&checked, "Root::enter")
                        .is_err(),
                    "moved source unexpectedly produced an executable artifact: {source}"
                );
            }
        }
    }
}
