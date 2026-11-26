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
    let core = std::path::Path::new("core");
    sources.add_with_metadata(
        core.join("foundation.omg"),
        std::str::from_utf8(bundled_declarations::bytes(
            language_core::bundled_files::BundledFile::CoreFoundation,
        ))
        .unwrap()
        .into(),
        core.into(),
        None,
        source_admission::SourceOrigin::Toolchain,
    );
    let imports = crate::resolution::fixture_local_and_core_imports(&syntax, &sources, core);
    resolve(ResolutionRequest {
        syntax,
        sources: Some(std::sync::Arc::new(sources)),
        top_level_bindings: imports,
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
fn domain_index_names_resolve_in_their_declared_type_or_const_role() {
    use crate::source_resolved::types::TypeReference;

    let program = resolve_source(
        "data Index {}
         domain<T, Frame, const Index: u64> T::Calibrated<Frame, Index>;
         domain<const Index: u64> u32::At<Index>;",
    );
    assert_eq!(program.domain_definitions.len(), 2);
    for domain in &program.domain_definitions {
        let parameters = program.data_type_parameters(domain.type_parameters);
        let arguments = program.child_type_references(domain.index_arguments);
        let indices = match domain.name.as_str().rsplit("::").next().unwrap() {
            "Calibrated" => &parameters[1..],
            "At" => parameters,
            other => panic!("unexpected domain {other}"),
        };
        assert_eq!(arguments.len(), indices.len());
        for (argument, binder) in arguments.iter().zip(indices) {
            assert!(
                matches!(argument, TypeReference::Named { symbol, .. } if symbol.is_valid() && *symbol == binder.symbol),
                "{argument:?} must select the binder {binder:?}",
            );
        }
    }
}

#[test]
fn a_refinement_supplies_its_own_binders_in_its_parents_declared_roles() {
    use crate::source_resolved::types::TypeReference;

    let source = "data N {} data OnlyAType {}
         domain<Frame, const N: u8> u32::At<Frame, N>;
         domain<Frame, const N: u8> u32::At<Frame, N>::Child<Frame, N>;";
    for (source, valid_const) in [
        (source.to_owned(), true),
        (
            source.replace("At<Frame, N>::Child", "At<Frame, OnlyAType>::Child"),
            false,
        ),
    ] {
        let program = resolve_source(&source);
        let mut domains = program.domain_definitions.iter();
        let parent = domains.next().unwrap();
        let child = domains.next().unwrap();
        let [ProofFact::Membership(fact)] = program.proof_facts(child.facts) else {
            panic!("a refinement has one parent fact");
        };
        assert_eq!(fact.domain_symbol, parent.symbol);
        let child_binders = program.data_type_parameters(child.type_parameters);
        let [
            TypeReference::Named { symbol: frame, .. },
            TypeReference::Named { symbol: index, .. },
        ] = program.child_type_references(fact.domain_arguments)
        else {
            panic!("the parent application takes a type and a const");
        };
        assert_eq!(*frame, child_binders[0].symbol);
        if valid_const {
            assert_eq!(*index, child_binders[1].symbol);
            assert_ne!(
                *index,
                program.data_type_parameters(parent.type_parameters)[1].symbol
            );
        } else {
            assert!(!index.is_valid());
        }
    }
}

#[test]
fn a_domain_application_selects_its_const_in_signatures_and_casts() {
    use crate::source_resolved::expression::ExpressionNode;
    use crate::source_resolved::types::{TypeConstraint, TypeReference};

    let program = resolve_source(
        "data Survey {}
         const Survey::LENGTH: u64 = 7;
         domain<T, const N: u64> T::Scale<N>;
         machine length(value: u32) -> u32 in Scale<Survey::LENGTH> {
             value as +Scale<Survey::LENGTH>
         }",
    );
    let constant = program.const_declarations.iter().next().unwrap().symbol;
    let assert_selected = |arguments| {
        let [TypeReference::Named { symbol, .. }] = program.child_type_references(arguments) else {
            panic!("expected one named index")
        };
        assert_eq!(*symbol, constant);
    };
    let mut constraints = program
        .tables
        .types
        .constraints
        .iter()
        .filter_map(|(_, constraint)| {
            if let TypeConstraint::Domain(domain) = constraint {
                Some(domain.arguments)
            } else {
                None
            }
        });
    assert_selected(
        constraints
            .next()
            .expect("the return type has an indexed domain"),
    );
    let mut casts = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, node)| {
            if let ExpressionNode::Cast(cast) = node {
                Some(cast.semantic_domain_arguments)
            } else {
                None
            }
        });
    assert_selected(casts.next().expect("the body adds an indexed domain"));
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

#[test]
fn a_domain_carrier_forwards_its_const_binder_through_a_type_alias() {
    use crate::source_resolved::types::{TypeConstraint, TypeReference};

    let program = resolve_source(
        r#"
            data Word { bits: u32; }
            domain<const N: u8> Word::Slot<N>;
            data Slotted<const N: u8> = Word in Slot<N>;
            domain<const N: u8> Slotted<N>::Ready<N>;
        "#,
    );
    let alias = program.type_aliases.iter().next().expect("Slotted");
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Slotted::Ready")
        .expect("Slotted::Ready");
    let binder = program.data_type_parameters(domain.type_parameters)[0].symbol;
    assert!(binder.is_valid());
    assert_ne!(
        binder,
        program.data_type_parameters(alias.type_parameters)[0].symbol
    );

    let TypeReference::AliasUse {
        application,
        expansion,
        ..
    } = &domain.target_type
    else {
        panic!("expected the applied carrier alias");
    };
    let TypeReference::Generic(applied) = application.as_ref() else {
        panic!("expected Slotted<N>");
    };
    let TypeReference::Constrained(expanded) = expansion.as_ref() else {
        panic!("expected Word in Slot<N>");
    };
    let [TypeConstraint::Domain(slot)] = program
        .tables
        .types
        .constraints
        .span_or_empty(expanded.constraints)
    else {
        panic!("expected Slot<N>");
    };
    for arguments in [applied.arguments, slot.arguments] {
        let [TypeReference::Named { symbol, .. }] = program.child_type_references(arguments) else {
            panic!("expected the forwarded const argument");
        };
        assert_eq!(*symbol, binder);
    }
}
