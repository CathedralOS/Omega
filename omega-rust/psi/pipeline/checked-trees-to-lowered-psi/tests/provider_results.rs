use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{decode_module, encode_module, encode_proof_bundle};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    ProviderInstallationSelection, TerminalExecution, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalStructuralValue, admit_provider_installation_from_artifact,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    pub data Token { value: u64; }
    pub data Pair { left: Token; right: Token; }
    boundary trait Factory {
        machine forward(value: Pair) -> Pair reaches Factory;
    }
    data Provider {}
    machine Provider::forward(value: Pair) -> Pair satisfies Factory::forward { value }
    data Alternative {}
    machine Alternative::forward(value: Pair) -> Pair satisfies Factory::forward { value }
    data Sink {}
    machine Sink::take(token: Token) {}
    data Root {}
    machine Root::enter(value: Pair) reaches Factory {
        let result: Pair = Factory::forward(value);
        Sink::take(result.right);
    }
"#;

#[test]
fn authored_affine_provider_returns_into_partial_result_cleanup() {
    for attached in [true, false] {
        for (fields, calls, residuals) in [
            ("left: Token; right: Token;", "Sink::take(result.right);", 1),
            (
                "left: Token; right: Token;",
                "Sink::take(result.right); Sink::take(result.left);",
                0,
            ),
            (
                "row: [Token; 3]; tail: Token;",
                "Sink::take(result.row[1]);",
                3,
            ),
        ] {
            let mut source = SOURCE
                .replace("left: Token; right: Token;", fields)
                .replace("Sink::take(result.right);", calls);
            if !attached {
                source = source.replace("Root::enter", "enter");
            }
            let checked = checked_source(&source);
            let entry = if attached { "Root::enter" } else { "enter" };
            assert_eq!(
                checked
                    .facts
                    .flow
                    .terminal_partial_affine_unit_cleanups
                    .machines
                    .len(),
                1,
                "the caller must have checked partial-result cleanup"
            );
            let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, entry)
                .expect("lower provider result and partial cleanup");
            assert_eq!(lowered.semantic_module.provider_candidates.len(), 2);
            let caller = lowered
                .semantic_module
                .machines
                .iter()
                .find(|machine| machine.id == lowered.semantic_module.entry)
                .unwrap();
            let result = caller.blocks[0].operations[0].result.structural().unwrap();
            assert_ne!(result.place, caller.structural_parameters[0].place);
            match &caller.blocks[0].terminator {
                terminal_psi::Terminator::ReturnUnitPartialAffine {
                    trivial_affine_discards,
                    residual_affine_discards,
                    ..
                } => {
                    assert!(trivial_affine_discards.is_empty());
                    assert_eq!(residual_affine_discards.len(), residuals);
                    assert!(
                        residual_affine_discards
                            .iter()
                            .all(|discard| discard.place == result.place)
                    );
                }
                terminal_psi::Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } => {
                    assert_eq!(residuals, 0);
                    assert!(trivial_affine_discards.is_empty());
                }
                _ => panic!("Unit cleanup"),
            }
            execute_candidates(&lowered.semantic_module, &lowered.proof_bundle);
        }
    }
}

fn checked_source(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn execute_candidates(
    module: &terminal_psi::TerminalModule,
    proof: &terminal_verifier::ProofBundle,
) {
    let semantic = encode_module(module).expect("encode");
    assert_eq!(decode_module(&semantic).expect("decode"), *module);
    let proof = encode_proof_bundle(proof).expect("proof");
    let profile = proof_admission::AdmissionProfile::default();
    for candidate in &module.provider_candidates {
        assert!(matches!(
            candidate.candidate_identity.as_str(),
            "Provider::forward" | "Alternative::forward"
        ));
        let installation = admit_provider_installation_from_artifact(
            &semantic,
            &proof,
            &profile,
            &[ProviderInstallationSelection {
                boundary: candidate.boundary,
                provider_identity: candidate.provider_identity.clone(),
                candidate: candidate.candidate,
            }],
        )
        .expect("install authored provider");
        let argument = TerminalStructuralValue {
            opaque_identity: 50,
            structural_type: candidate.signature.parameters[0].structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        for incremental in [false, true] {
            let mut execution = TerminalExecution::start_artifact_with_provider_installation(
                &semantic,
                &proof,
                &profile,
                &[],
                std::slice::from_ref(&argument),
                &installation,
            )
            .expect("start");
            let mut fuel = if incremental {
                TerminalFuelMeter::with_allowance(0)
            } else {
                TerminalFuelMeter::unbounded()
            };
            let mut completed = false;
            for _ in 0..32 {
                match execution.resume(&mut fuel).unwrap() {
                    TerminalExecutionStatus::Complete(result) => {
                        assert_eq!(result, TerminalExecutionResult::Unit);
                        completed = true;
                        break;
                    }
                    TerminalExecutionStatus::SponsorExhausted(_) => {
                        assert!(incremental);
                        fuel.replenish(1).unwrap();
                    }
                    other => panic!("unexpected {other:?}"),
                }
            }
            assert!(completed);
            assert!(execution.effects().is_empty());
            assert!(execution.live_affine_frontier().next().is_none());
        }
    }
}

#[test]
fn authored_affine_provider_requires_one_exact_checked_return_plan() {
    let checked = checked_source(SOURCE);
    let plans = &checked
        .facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_machines;
    assert_eq!(plans.len(), 2);
    for mutation in 0..3 {
        let mut malformed = checked.clone();
        let plans = &mut malformed
            .facts
            .flow
            .terminal_structural_returns
            .claim_free_affine_machines;
        match mutation {
            0 => {
                plans.remove(0);
            }
            1 => {
                plans.push(plans[0].clone());
            }
            2 => {
                plans[0].result.type_identity = "Token".into();
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&malformed, "Root::enter").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn boundary_result_cleanup_does_not_forget_an_untransferred_input() {
    let checked = checked_source(
        r#"
        pub data Token { value: u64; }
        pub data Pair { left: Token; right: Token; }
        boundary trait Factory { machine create() -> Pair reaches Factory; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(value: Pair) reaches Factory {
            let result: Pair = Factory::create();
            Sink::take(result.right);
        }
    "#,
    );
    // Both the original input and the result complement need cleanup. The
    // single-root plan must not silently retain only the result's leftovers.
    assert!(
        checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .machines
            .is_empty()
    );
    assert!(checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").is_err());
}

#[test]
fn retained_affine_provider_rejoins_its_authored_return_source() {
    let checked = checked_source(SOURCE);
    let plan = &checked
        .facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_machines[0];
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == plan.machine)
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let [checked_trees::statement::StatementNode::Expression(expression)] =
        checked.statement_table.statements(state.statement_nodes)
    else {
        panic!("identity body")
    };
    for replace_head in [false, true] {
        let mut changed = checked.clone();
        let checked_trees::expression::ExpressionNode::Name(path) =
            changed.typed.expression_table.expression_mut(*expression)
        else {
            panic!("returned name")
        };
        if replace_head {
            path.head_symbol = machine.symbol;
        } else {
            path.symbol = machine.symbol;
        }
        assert!(checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err());
    }
}
