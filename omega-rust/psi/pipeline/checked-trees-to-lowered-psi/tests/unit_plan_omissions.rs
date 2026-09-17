//! A missing checked Unit plan reports why the checked stage omitted it: the
//! lowering error carries the omission chain from the requested machine to
//! the machine whose own body failed local construction.

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

const UNSUPPORTED_BODY: &str = r#"
    data Token { observed: bool; other: bool; }
    machine Token::drop(&mut self) {}
    data Main {}
    machine Main::main(token: Token, input: u64, enabled: bool) -> bool {
        let staged: bool = token.observed && ((input < 1u64) || enabled);
        staged
    }
"#;

#[test]
fn a_root_without_an_admitted_body_names_its_own_local_construction() {
    let checked = checked(UNSUPPORTED_BODY);
    let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
        .expect_err("the mixed member/comparison body has no Unit plan");
    let checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan {
        machine,
        reason,
        omission,
    } = error
    else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(machine, "Main::main");
    assert_eq!(
        reason,
        "attached Unit closure is missing a checked transitive machine plan"
    );
    assert_eq!(
        omission.as_deref(),
        Some("`Main::main` has no admitted body (local construction stopped at completion)")
    );
}

#[test]
fn a_root_whose_callee_lacks_a_body_names_the_callee_chain() {
    let checked = checked(
        r#"
        data Token { observed: bool; other: bool; }
        machine Token::drop(&mut self) {}
        data Main {}
        machine Main::main(token: Token, input: u64, enabled: bool) {
            Main::relay(token, input, enabled);
        }
        machine Main::relay(token: Token, input: u64, enabled: bool) {
            Main::leaf(token, input, enabled);
        }
        machine Main::leaf(token: Token, input: u64, enabled: bool) {
            let staged: bool = token.observed && ((input < 1u64) || enabled);
        }
    "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let named = |name: &str| {
        checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} is declared"))
            .symbol
    };
    let omission = |name: &str| {
        plans
            .omission_for_machine(named(name))
            .unwrap_or_else(|| panic!("{name} has an omission row"))
            .stage
    };
    assert!(matches!(
        omission("Main::leaf"),
        checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction { .. }
    ));
    assert_eq!(
        omission("Main::relay"),
        checked_trees::CheckedUnitPlanOmissionStage::UnavailableCallee {
            target: named("Main::leaf")
        }
    );
    assert_eq!(
        omission("Main::main"),
        checked_trees::CheckedUnitPlanOmissionStage::UnavailableCallee {
            target: named("Main::relay")
        }
    );
    let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
        .expect_err("the root's transitive callee has no Unit plan");
    let checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan {
        machine,
        omission,
        ..
    } = error
    else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(machine, "Main::main");
    assert_eq!(
        omission.as_deref(),
        Some(
            "`Main::main` calls `Main::relay`, which has no plan; \
             `Main::relay` calls `Main::leaf`, which has no plan; \
             `Main::leaf` has no admitted body (local construction stopped at completion)"
        )
    );
}
