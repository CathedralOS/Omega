/*
    Proof-fact membership and expression membership used to answer one
    spelling differently. A domain and a case that share the spelling are
    one contested membership: the fact does not bind the domain.
*/

use crate::resolution::frontend_fixture::Lexer;
use crate::resolution::{ResolutionRequest, resolve};
use crate::source_resolved::domain::ProofFact;
use symbols::SymbolHandle;

pub(super) fn resolve_result(
    source_text: &str,
) -> Result<crate::source_resolved::SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source_text).tokenize().expect("tokens");
    let mut sources = crate::source_inputs::SourceMap::default();
    let source_id = sources
        .add("domain-lookup/main.omg".into(), source_text.to_owned())
        .source_id;
    let syntax =
        crate::resolution::frontend_fixture::parse_syntax_trees_with_id(source_id, &tokens)
            .expect("parse");
    resolve(ResolutionRequest {
        syntax,
        sources: Some(std::sync::Arc::new(sources)),
        top_level_bindings: Vec::new(),
        retained_base: None,
    })
}

fn resolve_source(source_text: &str) -> crate::source_resolved::SymbolResolvedTrees {
    resolve_result(source_text).unwrap_or_else(|errors| panic!("resolve failed: {errors:?}"))
}

fn fact_domain(program: &crate::source_resolved::SymbolResolvedTrees) -> SymbolHandle {
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

#[test]
fn explicit_domain_application_resolves_to_the_same_declared_type_equation() {
    let source = "
        domain<const Bound: u64> u64::AtMost<Bound> requires self <= Bound;
        machine ordinary<const Bound: u64>() -> u64 { Bound }
        machine capacity<Length, const Capacity: u64>() -> u64
        requires Length == u64::AtMost::<Capacity>
        { Capacity }
    ";
    let definition = |source: &str| {
        let resolved = resolve_source(source);
        let mut equations = resolved.tables.declarations.declared_type_equations.iter();
        let (_, equation) = equations
            .next()
            .expect("the machine declares the type equation");
        assert!(equations.next().is_none());
        assert_eq!(equation.parameter.display_name(), "Length");
        assert!(matches!(
            equation.definition,
            crate::source_resolved::types::TypeReference::Constrained(_)
                | crate::source_resolved::types::TypeReference::Generic(_)
        ));
        equation.definition.display_name()
    };
    assert_eq!(
        definition(source),
        definition(&source.replace("AtMost::<", "AtMost<"))
    );
    let wrong_role = source.replace("Length == u64::AtMost::<", "Length == ordinary::<")
        + "machine use_closed_type() -> u64 { capacity::<u64>() }";
    let rejected = |source: &str| {
        let errors =
            resolve_result(source).expect_err("a machine cannot supply the type constructor");
        errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
    };
    let expected = rejected(&wrong_role.replace("ordinary::<", "ordinary<"));
    assert!(!expected.is_empty());
    assert_eq!(rejected(&wrong_role), expected);
}
