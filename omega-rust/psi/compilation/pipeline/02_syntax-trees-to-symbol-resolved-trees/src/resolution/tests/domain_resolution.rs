use crate::resolution::{ResolutionRequest, resolve};
use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};

mod semantic_identity_tests {
    use crate::resolution::{ResolutionRequest, resolve};
    use language_semantics::SemanticDomainTable;
    use semantic_vocabulary::PackageKeyIdentity;
    use source::{DependencyScope, SourceMap, SourceOrigin, SourceResolutionStratum};
    use source_files_to_tokens::Lexer;
    use std::path::PathBuf;
    use std::sync::Arc;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use tokens_to_syntax_trees::parse_syntax_trees_with_id;

    fn add_domain_source(
        sources: &mut SourceMap,
        root: &str,
        filename: &str,
        text: &str,
        package_marker: u8,
        scope: DependencyScope,
    ) {
        sources.add_checked_instance(
            PathBuf::from(root).join(filename),
            text.to_owned(),
            PathBuf::from(root),
            PackageKeyIdentity::from_digest([package_marker; 32]),
            SourceOrigin::User,
            SourceResolutionStratum::Base,
            scope,
        );
    }

    fn resolve_domain_sources(sources: SourceMap) -> SymbolResolvedTrees {
        let mut syntax = syntax_trees::SyntaxTrees::default();
        for file in sources.files() {
            let tokens = Lexer::new(&file.source)
                .tokenize()
                .expect("tokenize domain source");
            let parsed =
                parse_syntax_trees_with_id(file.source_id, &tokens).expect("parse domain source");
            syntax.extend_from(&parsed);
        }
        resolve(ResolutionRequest {
            syntax: &syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
        })
        .expect("resolve owned domain declarations")
    }

    fn canonical_names(program: &SymbolResolvedTrees) -> Vec<String> {
        program
            .domain_definitions
            .iter()
            .map(|domain| {
                program
                    .semantic_domains
                    .name(domain.semantic_id)
                    .expect("retained canonical domain identity")
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn managed_build_and_product_domains_keep_distinct_checked_identities() {
        let mut sources = SourceMap::default();
        for scope in [DependencyScope::Build, DependencyScope::Product] {
            add_domain_source(
                &mut sources,
                "shared",
                "codec.omg",
                "module codec; domain [u8; 8]::Utf8;",
                1,
                scope,
            );
        }
        let program = resolve_domain_sources(sources);
        assert_eq!(program.domain_definitions.len(), 2);
        assert_ne!(
            program.domain_definitions[0].semantic_id,
            program.domain_definitions[1].semantic_id
        );
        let names = canonical_names(&program);
        assert_ne!(names[0], names[1]);
    }

    #[test]
    fn managed_authored_policy_names_do_not_acquire_builtin_identity() {
        let mut sources = SourceMap::default();
        add_domain_source(
            &mut sources,
            "policies",
            "main.omg",
            "domain [u8; 8]::Wrapping; domain [u8; 8]::Saturating; domain [u8; 8]::Trapping;",
            1,
            DependencyScope::Product,
        );
        let program = resolve_domain_sources(sources);
        assert_eq!(program.domain_definitions.len(), 3);
        for (name, builtin) in [
            ("Wrapping", SemanticDomainTable::WRAPPING),
            ("Saturating", SemanticDomainTable::SATURATING),
            ("Trapping", SemanticDomainTable::TRAPPING),
        ] {
            assert_eq!(program.semantic_domains.lookup(name), Some(builtin));
            let authored = program
                .domain_definitions
                .iter()
                .find(|domain| domain.name.as_str().rsplit("::").next() == Some(name))
                .expect("authored lookalike");
            assert_ne!(authored.semantic_id, builtin);
            assert!(authored.semantic_roles.is_empty());
        }
    }

    #[test]
    fn mapped_domain_rejects_missing_source_custody() {
        let tokens = Lexer::new("domain [u8; 8]::Utf8;")
            .tokenize()
            .expect("tokenize domain");
        let syntax =
            parse_syntax_trees_with_id(source::SourceId(0), &tokens).expect("parse domain");
        let diagnostics = resolve(ResolutionRequest {
            syntax: &syntax,
            sources: Some(Arc::new(SourceMap::default())),
            top_level_bindings: Vec::new(),
        })
        .expect_err("a mapped declaration cannot fall back to source-free identity");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "domain declaration is missing its checked source owner"
        }));
    }

    #[test]
    fn contract_membership_prefers_the_occurrence_package_over_foreign_leaves() {
        let mut sources = SourceMap::default();
        add_domain_source(
            &mut sources,
            "application",
            "main.omg",
            "domain [u8; 8]::Utf8\nrequires\n    valid_utf8(self);\n\ndomain [u8; 16]::Utf8\nrequires\n    valid_utf8(self);\n\nmachine fill(line: &mut [u8; 8])\nensures\n    line in Utf8\n{\n}\n",
            1,
            DependencyScope::Product,
        );
        // A dependency publishes the same leaf under its own package identity;
        // root-scoped declarations still reach the application's authored
        // spelling, so the occurrence's own package must outrank them.
        add_domain_source(
            &mut sources,
            "library",
            "calling.omg",
            "domain [u8; 256]::Utf8\nrequires\n    valid_utf8(self);\n",
            2,
            DependencyScope::Product,
        );
        let program = resolve_domain_sources(sources);
        assert_eq!(program.domain_definitions.len(), 3);
        let own = program
            .domain_definitions
            .iter()
            .filter(|domain| {
                program
                    .symbols
                    .same_symbol_source_package(program.domain_definitions[0].symbol, domain.symbol)
            })
            .collect::<Vec<_>>();
        assert_eq!(own.len(), 2, "the application owns both authored domains");

        let machine = program.machines.first().expect("fill machine");
        let contract = program
            .machine_contracts(machine)
            .iter()
            .find(|contract| {
                contract.kind == symbol_resolved_trees::signature::SignatureContractKind::Ensures
            })
            .expect("fill should retain its ensures contract");
        let [symbol_resolved_trees::domain::ProofFact::Membership(membership)] =
            program.proof_facts(contract.facts)
        else {
            panic!("ensures should contain one domain membership")
        };
        assert!(membership.domain_symbol.is_valid());
        assert_eq!(
            membership.domain_symbol, program.domain_definitions[0].symbol,
            "the leaf spelling selects the application's own declaration, not the foreign one"
        );
    }
}

#[test]
fn infers_top_level_operator_home_from_qualified_operands() {
    let source = r#"
    domain i32::Degrees;

    operator + add(left: i32 in Degrees, right: i32 in Degrees) -> i32 in Degrees;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "i32::Degrees")
        .expect("domain should lower");
    let operator = program
        .operator_definitions(domain.operators)
        .first()
        .expect("qualified operands should supply one semantic home");

    assert!(program.operators.is_empty());
    assert_eq!(
        domain.semantic_roles.denotation_dimension,
        Some(domain.semantic_id)
    );
    assert_eq!(
        program
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["add"]
    );
}

#[test]
fn rejects_ambiguous_inferred_domain_operator_home() {
    let source = r#"
    domain i32::Degrees;
    domain i32::Radians;

    operator + add(left: i32 in Degrees, right: i32 in Radians) -> i32;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = resolve(ResolutionRequest::new(&syntax_trees))
        .expect_err("competing operand domains must not infer an operator home");
    assert!(
        diagnostic[0]
            .message
            .contains("has more than one possible domain home")
    );
}

#[test]
fn normalizes_authored_exact_machine_route() {
    use language_semantics::DomainEstablishmentRoute;

    let source = r#"
    data Token { value: u64; }

    domain Token::Stamped
    established by Token::stamp;

    machine Token::stamp(value: u64) -> Token
    ensures result in Token::Stamped
    {
        Token { value: value }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");

    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Stamped")
        .expect("stamped domain");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Token::stamp")
        .expect("stamp machine");
    assert_eq!(
        domain.establishment_routes,
        [DomainEstablishmentRoute::ExactMachine {
            machine: machine.symbol,
        }]
    );
}

#[test]
fn exact_machine_route_rejects_a_machine_without_domain_authority() {
    let source = r#"
    data Token { value: u64; }

    domain Token::Stamped
    established by Token::stamp;

    machine Token::stamp(value: u64) -> Token
    {
        Token { value: value }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostics = resolve(ResolutionRequest::new(&syntax_trees))
        .expect_err("a machine route must authorize the domain on its exact result");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not name the domain on its exact result")),
        "unexpected domain-establishment diagnostics: {diagnostics:?}"
    );
}

#[test]
fn establishment_route_rejects_ambiguity_across_declaration_kinds() {
    let source = r#"
    data Token { value: u64; }

    domain Token::Stamped
    established by Issuer::issue;

    trait Issuer {
        machine issue(value: u64) -> Token in Stamped;
    }

    machine Issuer::issue(value: u64) -> Token
    ensures result in Token::Stamped
    {
        Token { value: value }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostics = resolve(ResolutionRequest::new(&syntax_trees))
        .expect_err("a route naming both a requirement and a machine must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("ambiguous across declaration kinds")),
        "unexpected domain-establishment diagnostics: {diagnostics:?}"
    );
}

#[test]
fn authored_establishment_route_cannot_use_an_extension_only_domain_constraint() {
    let base = r#"
        data Token { value: u64; }
        domain Token::Issued
        established by Issuer::issue;
        trait Issuer {
            machine issue(value: u64) -> Token in Later;
        }
    "#;
    let extension = "domain Token::Later = Token::Issued;";
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("main.omg"), base.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            extension.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let base_tokens = Lexer::new(base).tokenize().expect("tokenize base");
    let mut syntax = parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse base source");
    let extension_tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize extension");
    let extension_syntax = parse_syntax_trees_with_id(extension_id, &extension_tokens)
        .expect("parse extension source");
    syntax.extend_from(&extension_syntax);

    let diagnostics = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect_err("an authored route must not gain authority from a hidden domain alias");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not name the domain on its exact result")),
        "unexpected domain-establishment diagnostics: {diagnostics:?}"
    );
}

#[test]
fn signature_free_route_still_rejects_a_contested_same_package_leaf() {
    // Scoping narrows the pool to the occurrence's dependency scope; it does
    // not weaken the exact-selection contract. Two same-package declarations
    // competing for one leaf remain ambiguous.
    let first = "pub boundary trait Shared {}";
    let second = "pub boundary trait Shared {}";
    let user = r#"
        data Token { value: u64; }
        domain Token::Issued
        established by Shared::issue;
    "#;
    let mut sources = SourceMap::default();
    let mut syntax = syntax_trees::SyntaxTrees::default();
    for (index, text) in [first, second, user].into_iter().enumerate() {
        let source_id = sources
            .add_with_metadata(
                PathBuf::from(format!("package/{index}.omg")),
                text.to_owned(),
                PathBuf::from("package"),
                None,
                SourceOrigin::User,
            )
            .source_id;
        let tokens = Lexer::new(text).tokenize().expect("tokenize");
        tokens_to_syntax_trees::parse(&mut syntax, source_id, &tokens).expect("parse");
    }
    let diagnostics = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect_err("same-package leaf collision still rejects");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not resolve to one exact trait")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}
