use super::validate_repeated_normalized_domain_identities;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use facts::{Fact, FactOrigin, FactPayload, FactPlan, FactRef};
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;

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

#[test]
fn domain_predicate_places_use_the_carrier_type_under_logical_operators() {
    for (carrier, fields, predicate, accepted) in [
        ("Player", "health: i32;", "self.health", false),
        ("Player", "health: i32;", "!self.health", false),
        ("Player", "health: i32;", "self.health && true", false),
        ("Player", "health: i32;", "false || self.health", false),
        ("Player", "health: i32;", "self.health > 0", true),
        (
            "Player",
            "health: i32;",
            "self.health > 0 && self.health < 0",
            true,
        ),
        ("Player", "healthy: bool;", "self.healthy", true),
        ("Player", "healthy: bool;", "!self.healthy && true", true),
        ("Player", "flags: [i32; 2];", "self.flags[0]", false),
        ("Player", "flags: [bool; 2];", "self.flags[0]", true),
        ("i32", "", "self", false),
        ("bool", "", "self", true),
    ] {
        let program = crate::front_end::typed_program_from_texts(&[&format!(
            "data Player {{ {fields} }} domain {carrier}::Selected requires {predicate};"
        )]);
        let fact_plan = crate::build_definition_fact_plan(&program);
        let domain = &program.domain_definitions()[0];
        let mut diagnostics = Vec::new();
        super::validate_domain_fact_payloads(
            &program,
            &fact_plan,
            domain,
            &mut diagnostics,
            super::ProofFactOwner::Domain(domain.name.as_str()),
        );
        assert_eq!(
            diagnostics.is_empty(),
            accepted,
            "{carrier} with {fields}: {predicate}: {diagnostics:?}"
        );
    }
}

#[test]
fn sibling_sources_in_one_module_keep_the_same_consistency_group() {
    let program = crate::front_end::typed_program_from_texts(&[
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
