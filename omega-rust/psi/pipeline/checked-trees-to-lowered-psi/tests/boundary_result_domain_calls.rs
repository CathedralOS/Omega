//! Boundary `ensures result in <domain>` clauses on an invoked requirement:
//! a clause authorized by the domain's `established by` route is admitted
//! through the exact-qualification gate, folds the domain onto the structural
//! result qualification at lowering, and survives codec + independent
//! verification; a clause naming a domain that does not authorize this
//! requirement is still refused.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|errors| panic!("{source}: {errors:#?}"))
}

fn program(domain_declaration: &str) -> String {
    format!(
        r#"
        machine identity16(value: u16) -> u16
        requires 0u16 == 0u16
        ensures result == value
        {{ value }}
        pub data Token {{ flag: bool; }}
        {domain_declaration}
        boundary trait Factory {{
            machine create(first: u16, second: u16) -> Token reaches Factory
            ensures result in Token::Held;
        }}
        boundary trait Host {{ machine finish(value: Token) reaches Host; }}
        data Main {{}}
        machine Main::main() reaches Factory + Host {{
            let result: Token = Factory::create(identity16(5u16), 7u16);
            Host::finish(result);
        }}
        "#
    )
}

fn lowered(domain_declaration: &str) -> lowered_psi::LoweredPsi {
    checked_trees_to_lowered_psi::lower_machine(
        &checked(&program(domain_declaration)),
        "Main::main",
    )
    .expect("lower_machine")
}

#[test]
fn authorized_result_domain_qualifies_the_boundary_result() {
    let lowered = lowered("domain Token::Held established by Factory::create;");
    let module = &lowered.semantic_module;
    let held = module
        .structural_domains
        .iter()
        .find(|declaration| declaration.identity.contains("Held"))
        .expect("Token::Held structural domain declaration");
    let boundary = module
        .boundary_machines
        .iter()
        .find(|boundary| boundary.identity.contains("create"))
        .expect("Factory::create boundary declaration");
    assert!(
        matches!(
            &boundary.result,
            terminal_psi::BoundaryMachineResult::Structural(declaration)
                if declaration.qualifications.contains(&held.id)
        ),
        "boundary result carries the authorized domain: {boundary:#?}"
    );
    let call_result = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::BoundaryCall { .. }
            )
            .then(|| operation.result.structural())
            .flatten()
        })
        .expect("the create boundary call emits a structural result");
    assert!(
        call_result.qualifications.contains(&held.id),
        "the call result occurrence carries the authorized domain"
    );

    let bytes = terminal_codec::encode_module(module).expect("encode");
    let decoded = terminal_codec::decode_module(&bytes).expect("decode");
    assert_eq!(&decoded, module);
    terminal_verifier::validate_module(&decoded).expect("independent verification");
}

#[test]
fn unauthorized_result_domain_still_rejects_the_call() {
    let checked = checked(&program("domain Token::Held;"));
    assert!(matches!(
        checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main"),
        Err(checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan { .. })
    ));
}
