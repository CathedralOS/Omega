/*
    Proof-fact membership and expression membership used to answer one
    spelling differently. A domain and a case that share the spelling are
    one contested membership: the fact does not bind the domain.
*/

use crate::frontend_fixture::Lexer;
use crate::{ResolutionRequest, resolve};
use symbol_resolved_trees::domain::ProofFact;
use symbols::SymbolHandle;

pub(super) fn resolve_result(
    source_text: &str,
) -> Result<symbol_resolved_trees::SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source_text).tokenize().expect("tokens");
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add("domain-lookup/main.omg".into(), source_text.to_owned())
        .source_id;
    let syntax =
        crate::frontend_fixture::parse_syntax_trees_with_id(source_id, &tokens).expect("parse");
    resolve(ResolutionRequest {
        syntax,
        sources: Some(std::sync::Arc::new(sources)),
        top_level_bindings: Vec::new(),
        retained_base: None,
    })
}

fn resolve_source(source_text: &str) -> symbol_resolved_trees::SymbolResolvedTrees {
    resolve_result(source_text).unwrap_or_else(|errors| panic!("resolve failed: {errors:?}"))
}

fn fact_domain(program: &symbol_resolved_trees::SymbolResolvedTrees) -> SymbolHandle {
    let data = program
        .data_definitions
        .iter()
        .next()
        .expect("one data declaration");
    let [ProofFact::Membership(membership)] = program.proof_facts(data.requires_facts) else {
        panic!(
            "expected one membership fact, got {:?}",
            program.proof_facts(data.requires_facts)
        );
    };
    membership.domain_symbol
}

#[test]
fn proof_fact_membership_rejects_a_domain_competing_with_a_case() {
    let errors = resolve_result(
        r#"
            data Player
                requires self in Player::Valid
            {
                case Valid;
            }
            domain Player::Valid;
            "#,
    )
    .expect_err("a domain competing with a case rejects");
    assert!(
        errors.iter().any(|error| {
            error
                .message
                .contains("ambiguous membership `Player::Valid`")
        }),
        "{errors:?}"
    );
}

#[test]
fn proof_fact_membership_keeps_a_domain_when_no_case_competes() {
    let program = resolve_source(
        r#"
            data Player
                requires self in Player::Valid
            {
                case Other;
            }
            domain Player::Valid;
            "#,
    );
    let domain = fact_domain(&program);
    assert_eq!(program.symbols.display_path(domain, "::"), "Player::Valid",);
}

#[test]
fn proof_expression_membership_rejects_a_domain_competing_with_a_case() {
    let errors = resolve_result(
        r#"
            data Player
                requires (self in Player::Valid) || false
            {
                case Valid;
            }
            domain Player::Valid;
            "#,
    )
    .expect_err("a domain inside a proof expression competing with a case rejects");
    assert!(
        errors.iter().any(|error| {
            error
                .message
                .contains("ambiguous membership `Player::Valid`")
        }),
        "{errors:?}"
    );
}
