//! Scalar-affine cleanup integration tests by lowering and custody boundary.
//!
//! Exact behavior descends through payloadless-case fencing, jump cleanup,
//! proof-bearing scalar cleanup, and structural-return custody. Typed fixture
//! identities remain in one explicit support leaf.

#[path = "scalar_affine_cleanup/payloadless_case.rs"]
mod payloadless_case;
#[path = "scalar_affine_cleanup/scalar_cleanup_proofs.rs"]
mod scalar_cleanup_proofs;
#[path = "scalar_affine_cleanup/structural_return.rs"]
mod structural_return;
#[path = "scalar_affine_cleanup/support.rs"]
mod support;

#[test]
fn named_and_legacy_cleanup_receivers_cross_the_runtime_firewall() {
    for (subject, receiver) in [("token: &mut Token", "token"), ("&mut self", "self")] {
        let source = format!(
            r#"
            data Helper {{}}
            machine Helper::note(value: bool) {{}}
            data Token {{ ready: bool; }}
            machine Token::drop({subject}) requires {receiver}.ready {{
                Helper::note({receiver}.ready);
                {receiver}.ready = false;
            }}
            data Root {{}}
            machine Root::enter(token: Token) requires token.ready {{}}
        "#
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .unwrap();
        let lowered = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            checked_trees_to_lowered_psi::TerminalMachineSelection::Name("Root::enter"),
        )
        .unwrap();
        let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
        let proof =
            terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
                .unwrap();
        let admitted = terminal_psi_to_abstract_operations::lower_artifact(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &semantic,
                proof_bytes: &proof,
                obligation_ledger_bytes: None,
            },
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();
        let plan = admitted.plan();
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .unwrap();
        let abstract_operations::AbstractOperation::ReturnUnit {
            cleanup_actions, ..
        } = caller.operations.last().unwrap()
        else {
            panic!("source owns one normal cleanup return")
        };
        let [terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup)] =
            cleanup_actions.as_slice()
        else {
            panic!("one root owns one invocation")
        };
        let hook = plan
            .functions
            .iter()
            .find(|function| function.machine == cleanup.cleanup_machine)
            .unwrap();
        assert_eq!(
            cleanup.cleanup_receiver,
            Some(hook.structural_parameters[0].place)
        );
        assert!(
            cleanup.requirement_obligations.is_empty(),
            "proof IDs do not enter runtime custody"
        );
        let input = admitted
            .into_optimization_artifact()
            .into_optimization_input();
        let unit = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
            input,
            terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        )
        .unwrap();
        optimization_unit_semantics::validate_psi_optimization_unit(unit.unit())
            .expect("runtime ownership settles the exact borrowed hook and consumed root");
    }
}
