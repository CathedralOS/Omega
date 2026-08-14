use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::OperationKind;
use psi_terminal_codec::{decode_module, encode_module};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    boundary trait Signal {
        machine emit();
    }

    data FirstProvider {}
    machine FirstProvider::emit() satisfies Signal::emit {}

    data SecondProvider {}
    machine SecondProvider::emit() satisfies Signal::emit {}

    data Root {}
    machine Root::enter()
    reaches Signal
    {
        Signal::emit();
    }
"#;

#[test]
fn checked_unit_provider_candidates_are_cataloged_without_selection_or_call_rewrite() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("zero-argument Unit provider catalog lowers");

    let module = &lowered.semantic_module;
    assert_eq!(module.boundary_machines.len(), 1);
    assert_eq!(module.provider_candidates.len(), 2);
    assert_eq!(module.machines.len(), 3, "root plus both adapter bodies");
    let boundary = module.boundary_machines[0].id;
    let requirement_identity = &module.provider_candidates[0].requirement_identity;
    assert!(!requirement_identity.is_empty());
    assert!(module.provider_candidates.iter().all(|candidate| {
        candidate.boundary == boundary
            && candidate.requirement_identity == *requirement_identity
            && candidate.signature.parameters.is_empty()
            && candidate.refinement.positional_parameters.is_empty()
            && candidate.refinement.required_domains.is_empty()
            && module
                .machines
                .iter()
                .any(|machine| machine.id == candidate.candidate)
    }));
    assert_eq!(
        module
            .provider_candidates
            .iter()
            .map(|candidate| candidate.provider_identity.as_str())
            .collect::<Vec<_>>(),
        vec!["FirstProvider", "SecondProvider"]
    );
    assert_eq!(
        module
            .provider_candidates
            .iter()
            .map(|candidate| candidate.candidate_identity.as_str())
            .collect::<Vec<_>>(),
        vec!["FirstProvider::emit", "SecondProvider::emit"]
    );
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry machine");
    assert!(matches!(
        entry.blocks[0].operations[0].kind,
        OperationKind::BoundaryCall { boundary: called, .. } if called == boundary
    ));

    let encoded = encode_module(module).expect("encode catalog");
    let decoded = decode_module(&encoded).expect("decode and independently verify catalog shape");
    assert_eq!(decoded, *module);
}
