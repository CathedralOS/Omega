//! Canonical primitive storage retains its real local and observation identities.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_psi::OperationKind;
use terminal_psi_to_abstract_operations::lower_artifact;

#[test]
fn borrowed_primitive_local_survives_every_abstract_entrance() {
    let source = r#"
        machine reset(value: &mut u64) -> u64 { value = 0; 7 }
        machine enter(value: &mut u64) {
            let mut scratch: u64 = 91;
            let returned: u64 = reset(&mut scratch);
            value = scratch;
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("enter"),
    )
    .expect("primitive local Terminal producer");
    let establishment = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::EstablishPrimitiveLocal { .. }
            )
        })
        .expect("real primitive local establishment")
        .id;
    let semantic_bytes = encode_module(&lowered.semantic_module).expect("canonical semantics");
    let proof_bytes = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("canonical proof");
    let profile = AdmissionProfile::default();
    terminal_verifier::verify_module(&lowered.semantic_module, &lowered.proof_bundle, &profile)
        .expect("valid primitive storage");
    for result in [
        lower_artifact(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &semantic_bytes,
                proof_bytes: &proof_bytes,
                obligation_ledger_bytes: None,
            },
            &profile,
        )
        .map(|admitted| admitted.into_plan()),
        lower_artifact(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &semantic_bytes,
                proof_bytes: &proof_bytes,
                obligation_ledger_bytes: None,
            },
            &profile,
        )
        .map(|admitted| {
            admitted
                .into_optimization_artifact()
                .into_optimization_input()
        })
        .map(|input| input.plan().clone()),
        lower_artifact(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &semantic_bytes,
                proof_bytes: &proof_bytes,
                obligation_ledger_bytes: None,
            },
            &profile,
        )
        .and_then(|admitted| admitted.try_into_native_input(&[]))
        .map(|input| input.plan().clone()),
    ] {
        let plan = result.expect("primitive storage survives abstract admission");
        let operations = plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .collect::<Vec<_>>();
        assert!(operations.iter().any(|operation| matches!(operation,
            abstract_operations::AbstractOperation::EstablishPrimitiveLocal { psi_operation, .. }
            if *psi_operation == establishment)));
        assert!(operations.iter().any(|operation| matches!(
            operation,
            abstract_operations::AbstractOperation::PrimitiveScalarRead { .. }
        )));
        assert!(operations.iter().any(|operation| matches!(
            operation,
            abstract_operations::AbstractOperation::CallStructuralScalar { .. }
        )));
    }
}
