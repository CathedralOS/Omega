use super::validate_repeated_normalized_domain_identities;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use facts::{Fact, FactOrigin, FactPayload, FactPlan, FactRef};
use source::SourceId;
use source_files_to_tokens::Lexer;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;

fn typed(sources: &[&str]) -> TypedTrees {
    let tokens = Lexer::new(sources[0])
        .tokenize()
        .expect("tokenize first source");
    let mut syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse first source");
    for (source_ordinal, source) in sources.iter().enumerate().skip(1) {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize domain source");
        tokens_to_syntax_trees::parse_syntax_trees_into_with_id(
            &mut syntax,
            SourceId(source_ordinal),
            &tokens,
        )
        .expect("parse domain source");
    }
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve domain declarations");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type domain declarations")
}

fn diagnostics(program: &TypedTrees) -> Vec<Diagnostic> {
    let mut facts = FactPlan::default();
    for domain in program.domain_definitions() {
        let mut references = HandleSpan::empty();
        for fact in program.proof_facts(domain) {
            let ProofFact::Expression(expression) = fact else {
                panic!("these fixtures contain only Boolean domain facts");
            };
            let fact = facts.append_fact(Fact {
                origin: FactOrigin::DomainDefinition {
                    domain_symbol: domain.symbol,
                },
                payload: FactPayload::BooleanExpression(*expression),
                ..Fact::default()
            });
            facts.refs.append_to_span(&mut references, FactRef { fact });
        }
        facts.append_symbol_set(domain.symbol, references);
    }
    let mut diagnostics = Vec::new();
    validate_repeated_normalized_domain_identities(program, &facts, &mut diagnostics);
    diagnostics
}

fn repeated_capacity_domains(second_fact: &str) -> TypedTrees {
    typed(&[&format!(
        "domain [u8; 8]::Utf8 requires true;
         domain [u8; 16]::Utf8 requires {second_fact};"
    )])
}

#[test]
fn repeated_capacity_specializations_keep_equal_normalized_facts() {
    let program = repeated_capacity_domains("true");
    let [first, second] = program.domain_definitions() else {
        panic!("two capacity specializations");
    };
    assert_eq!(first.name, second.name);
    assert_eq!(first.semantic_id, second.semantic_id);
    assert_ne!(first.symbol, second.symbol);
    let diagnostics = diagnostics(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn repeated_capacity_specializations_reject_different_normalized_facts() {
    let diagnostics = diagnostics(&repeated_capacity_domains("false"));
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("declared more than once with different normalized semantics")),
        "{diagnostics:?}"
    );
}

#[test]
fn sibling_sources_in_one_module_keep_the_same_consistency_group() {
    let program = typed(&[
        "module codecs; domain [u8; 8]::Utf8 requires true;",
        "module codecs; domain [u8; 16]::Utf8 requires false;",
    ]);
    let first = program.domain_definitions()[0].symbol;
    let second = program.domain_definitions()[1].symbol;
    assert_eq!(
        program.symbols.symbol_module(first),
        program.symbols.symbol_module(second)
    );
    let diagnostics = diagnostics(&program);
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("declared more than once with different normalized semantics")),
        "{diagnostics:?}"
    );
}

#[test]
fn same_owner_identity_corruption_does_not_split_the_validation_group() {
    let mut program = repeated_capacity_domains("true");
    let changed = program
        .semantic_domains
        .intern("corrupted-capacity-identity");
    let second_symbol = program.domain_definitions()[1].symbol;
    let (handle, _) = program
        .tables
        .domain_definitions
        .iter()
        .find(|(_, domain)| domain.symbol == second_symbol)
        .expect("second capacity declaration");
    program
        .tables
        .domain_definitions
        .get_mut(handle)
        .semantic_id = changed;
    let diagnostics = diagnostics(&program);
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("declared more than once with different normalized semantics")),
        "{diagnostics:?}"
    );
}

#[test]
fn different_module_owners_cannot_share_a_tampered_semantic_identity() {
    let mut program = typed(&[
        "module first; domain<const N: u64> u64::Gate<N> requires true;",
        "module second; domain<const N: u64> u64::Gate<N> requires true;",
    ]);
    let first_symbol = program.domain_definitions()[0].symbol;
    let second_symbol = program.domain_definitions()[1].symbol;
    assert_ne!(
        program.symbols.symbol_module(first_symbol),
        program.symbols.symbol_module(second_symbol)
    );
    assert_ne!(
        program.domain_definitions()[0].semantic_id,
        program.domain_definitions()[1].semantic_id
    );
    assert!(diagnostics(&program).is_empty());
    let first_identity = program.domain_definitions()[0].semantic_id;
    let (handle, _) = program
        .tables
        .domain_definitions
        .iter()
        .find(|(_, domain)| domain.symbol == second_symbol)
        .expect("second module declaration");
    program
        .tables
        .domain_definitions
        .get_mut(handle)
        .semantic_id = first_identity;
    // Equal predicates cannot authorize unrelated declaration owners to share
    // the identity that downstream qualification implication consumes.
    let diagnostics = diagnostics(&program);
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("shares a normalized semantic identity with a distinct declaration owner")),
        "{diagnostics:?}"
    );
}
