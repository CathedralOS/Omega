use crate::resolution::{ResolutionRequest, resolve};
use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};

#[test]
fn const_collision_walks_separate_base_from_current_activation_extension() {
    let base_source = r#"
        const Clash: u64 = 1;
        const Choice::Ready: u64 = 2;
    "#;
    let extension_source = r#"
        data Clash {}
        data Choice { case Ready; }
    "#;
    let mut sources = SourceMap::default();
    let base_source_id = sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let extension_source_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let base_tokens = Lexer::new(base_source).tokenize().expect("tokenize base");
    let mut syntax = parse_syntax_trees_with_id(base_source_id, &base_tokens).expect("parse base");
    let extension_tokens = Lexer::new(extension_source)
        .tokenize()
        .expect("tokenize extension");
    let extension_syntax = parse_syntax_trees_with_id(extension_source_id, &extension_tokens)
        .expect("parse extension");
    syntax.extend_from(&extension_syntax);

    resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("free-shadow and case collisions across strata remain separate");
}

#[test]
fn const_declaration_collisions_still_reject_within_the_extension_stratum() {
    for (left, right, expected) in [
        (
            "const Limits::VALUE: u64 = 1;",
            "const Limits::VALUE: u64 = 2;",
            "duplicate const `Limits::VALUE`",
        ),
        (
            "const Clash: u64 = 1;",
            "data Clash {}",
            "free-floating const `Clash` collides with a declaration in the same namespace",
        ),
        (
            "const Choice::Ready: u64 = 1;",
            "data Choice { case Ready; }",
            "collides with the case `Ready`",
        ),
    ] {
        let mut sources = SourceMap::default();
        let left_id = sources
            .add_with_metadata_and_resolution_stratum(
                PathBuf::from("generated-left.omg"),
                left.to_owned(),
                PathBuf::from("."),
                None,
                SourceOrigin::User,
                SourceResolutionStratum::CurrentActivationExtension,
            )
            .source_id;
        let right_id = sources
            .add_with_metadata_and_resolution_stratum(
                PathBuf::from("generated-right.omg"),
                right.to_owned(),
                PathBuf::from("."),
                None,
                SourceOrigin::User,
                SourceResolutionStratum::CurrentActivationExtension,
            )
            .source_id;
        let left_tokens = Lexer::new(left)
            .tokenize()
            .expect("tokenize left extension");
        let mut syntax =
            parse_syntax_trees_with_id(left_id, &left_tokens).expect("parse left extension");
        let right_tokens = Lexer::new(right)
            .tokenize()
            .expect("tokenize right extension");
        let right_syntax =
            parse_syntax_trees_with_id(right_id, &right_tokens).expect("parse right extension");
        syntax.extend_from(&right_syntax);

        let diagnostics = resolve(ResolutionRequest {
            syntax: &syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
        })
        .expect_err("same-extension-stratum const collision must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "expected `{expected}`, got {diagnostics:?}"
        );
    }
}

#[test]
fn authored_memberships_and_struct_literals_ignore_extension_first_declarations() {
    let base = r#"
        data Token { value: u32; case Ready; }
        domain Token::Live;
        domain Token::Accepted
        requires
            self in Token::Live;
            self in Token::Ready | Token::Ready;
        machine make() -> Token
        ensures result in Token::Ready
        { Token { value: 1 } }
    "#;
    let extension = r#"
        data Token { value: u32; case Ready; }
        domain Token::Live;
        domain Token::Accepted;
    "#;
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("base.omg"), base.to_owned())
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
    let extension_tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize extension");
    let mut syntax =
        parse_syntax_trees_with_id(extension_id, &extension_tokens).expect("parse extension first");
    let base_tokens = Lexer::new(base).tokenize().expect("tokenize base");
    let base_syntax = parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse base");
    syntax.extend_from(&base_syntax);

    let program = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("base membership and literal lookup must ignore extension declarations");
    let source_of = |symbol| {
        program
            .symbols
            .symbol_provenance_source_span(symbol)
            .expect("source-backed symbol")
            .source_id
    };
    let literal = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            symbol_resolved_trees::expression::ExpressionNode::StructLiteral(literal) => {
                Some(literal)
            }
            _ => None,
        })
        .expect("authored struct literal");
    assert_eq!(source_of(literal.type_symbol), base_id);

    let accepted = program
        .domain_definitions
        .iter()
        .find(|domain| {
            domain.name.source_span().source_id == base_id
                && domain.name.as_str() == "Token::Accepted"
        })
        .expect("base Token::Accepted");
    let memberships = program
        .proof_facts(accepted.facts)
        .iter()
        .filter_map(|fact| match fact {
            symbol_resolved_trees::domain::ProofFact::Membership(membership) => Some(membership),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(memberships.len(), 1);
    assert!(memberships.iter().any(|membership| {
        membership.domain_symbol.is_valid() && source_of(membership.domain_symbol) == base_id
    }));
    let expression_memberships = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
                Some(membership)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        expression_memberships.iter().any(|membership| {
            membership.case_type_symbol.is_valid()
                && source_of(membership.case_type_symbol) == base_id
                && membership.case_symbol.is_valid()
        }),
        "memberships={expression_memberships:#?}"
    );
}

#[test]
fn authored_trait_machine_conformance_and_dynamic_assignments_ignore_extension_first() {
    let base = r#"
        trait Parent {}
        trait Child: Parent {}
        trait Bounded<Element, Evidence: Element satisfies Parent> {}
        trait Service { machine run(); }
        trait Marker {}
        data Item {}
        Primary: Item satisfies Marker {}
        machine provider() satisfies Service::run {}
        machine generic<Element, Evidence: Element satisfies Parent>() {}
        machine selected<Element>() where Element satisfies Item::Primary {}
        machine erase<'item>(item: &'item Item) -> &'item dyn Item::Primary {
            item as &dyn Item::Primary
        }
    "#;
    let extension = r#"
        trait Parent {}
        trait Child: Parent {}
        trait Bounded<Element, Evidence: Element satisfies Parent> {}
        trait Service { machine run(); }
        trait Marker {}
        data Item {}
        Primary: Item satisfies Marker {}
    "#;
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("base.omg"), base.to_owned())
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
    let extension_tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize extension");
    let mut syntax =
        parse_syntax_trees_with_id(extension_id, &extension_tokens).expect("parse extension first");
    let base_tokens = Lexer::new(base).tokenize().expect("tokenize base");
    let base_syntax = parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse base");
    syntax.extend_from(&base_syntax);

    let program = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("authored assignments must ignore extension declarations");
    let source_of = |symbol| {
        program
            .symbols
            .symbol_provenance_source_span(symbol)
            .expect("source-backed symbol")
            .source_id
    };
    let base_trait = |name: &str| {
        program
            .traits
            .iter()
            .find(|definition| {
                definition.name.as_str() == name
                    && definition.name.source_span().source_id == base_id
            })
            .expect("base trait")
    };
    let child = base_trait("Child");
    assert!(
        program
            .trait_requirements(child.requires)
            .iter()
            .all(|requirement| source_of(requirement.symbol) == base_id)
    );
    let bounded = base_trait("Bounded");
    assert!(
        bounded
            .conformance_bounds
            .iter()
            .all(|bound| source_of(bound.carrier) == base_id)
    );

    for machine_name in ["generic", "selected"] {
        let machine = program
            .machines
            .iter()
            .find(|machine| {
                machine.name.as_str() == machine_name
                    && machine.name.source_span().source_id == base_id
            })
            .expect("base bounded machine");
        assert!(
            machine
                .conformance_bounds
                .iter()
                .all(|bound| source_of(bound.carrier) == base_id)
        );
    }
    let provider = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "provider")
        .expect("base provider");
    assert!(
        program
            .machine_trait_conformances(provider.satisfies)
            .iter()
            .all(|satisfaction| source_of(satisfaction.symbol) == base_id)
    );

    let primary = program
        .conformances
        .iter()
        .find(|conformance| conformance.trait_name.source_span().source_id == base_id)
        .expect("base Primary conformance");
    assert_eq!(source_of(primary.carrier_symbol), base_id);
    assert_eq!(source_of(primary.trait_symbol), base_id);

    let erase = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "erase")
        .expect("base erase");
    let entry = program.machine_state(program.machine_state_handles(erase.states)[0]);
    let Some(symbol_resolved_trees::types::TypeReference::Reference(reference)) =
        &entry.return_type
    else {
        panic!("erase return remains a reference")
    };
    let symbol_resolved_trees::types::TypeReference::DynamicTrait {
        symbol,
        conformance,
        ..
    } = program.child_type_reference(reference.referee)
    else {
        panic!("erase referee remains dynamic")
    };
    assert_eq!(source_of(*symbol), base_id);
    assert_eq!(source_of(conformance.expect("named conformance")), base_id);
}

#[test]
fn trait_slot_catalogs_and_evidence_seeding_use_exact_trait_identity() {
    let base = r#"
        proposition Proven();
        trait Parent { machine base_requirement(); }
        trait Slot<proposition Evidence>: Parent
        where proposition Evidence();
        { machine base_slot(); }
        trait BaseChild: Slot<Proven> {}
        machine base_use<Element, Proof: Element satisfies Slot<Proven>>() {}
    "#;
    let extension = r#"
        boundary trait Service { machine run(); }
        trait Parent { machine extension_requirement(); }
        trait Slot<machine Evidence>: Parent { machine extension_slot(); }
        trait ExtensionChild: Slot<Service::run> {}
        machine extension_use<Element, Proof: Element satisfies Slot<Service::run>>() {}
    "#;
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("base.omg"), base.to_owned())
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
    let extension_tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize extension");
    let mut syntax =
        parse_syntax_trees_with_id(extension_id, &extension_tokens).expect("parse extension first");
    let base_tokens = Lexer::new(base).tokenize().expect("tokenize base");
    let base_syntax = parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse base");
    syntax.extend_from(&base_syntax);

    let program = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("same-named trait catalogs must retain exact stratum identity");
    let argument_kind = |arguments| {
        let [symbol_resolved_trees::types::TypeReference::Named { symbol, .. }] =
            program.child_type_references(arguments)
        else {
            panic!("one named static trait argument")
        };
        program.symbols.get(*symbol).kind
    };

    for (name, source_id, expected_kind) in [
        ("BaseChild", base_id, symbols::SymbolKind::Proposition),
        ("ExtensionChild", extension_id, symbols::SymbolKind::State),
    ] {
        let child = program
            .traits
            .iter()
            .find(|definition| {
                definition.name.as_str() == name
                    && definition.name.source_span().source_id == source_id
            })
            .expect("exact child trait");
        let [requirement] = program.trait_requirements(child.requires) else {
            panic!("one parameterized trait parent")
        };
        assert_eq!(argument_kind(requirement.arguments), expected_kind);
    }

    for (name, source_id, expected_kind, own_requirement, foreign_requirement) in [
        (
            "base_use",
            base_id,
            symbols::SymbolKind::Proposition,
            "base_requirement",
            "extension_requirement",
        ),
        (
            "extension_use",
            extension_id,
            symbols::SymbolKind::State,
            "extension_requirement",
            "base_requirement",
        ),
    ] {
        let machine = program
            .machines
            .iter()
            .find(|machine| {
                machine.name.as_str() == name && machine.name.source_span().source_id == source_id
            })
            .expect("exact bounded machine");
        let [bound] = machine.conformance_bounds.as_slice() else {
            panic!("one conformance bound")
        };
        let exact_slot = program
            .traits
            .iter()
            .find(|definition| {
                definition.name.as_str() == "Slot"
                    && definition.name.source_span().source_id == source_id
            })
            .expect("exact Slot trait");
        assert_eq!(
            bound.carrier, exact_slot.symbol,
            "machine={name} bound must retain exact Slot identity"
        );
        assert_eq!(
            argument_kind(bound.arguments),
            expected_kind,
            "machine={name}, carrier={} {:?}, arguments={:?}",
            program.symbols.name(bound.carrier),
            program.symbols.get(bound.carrier).kind,
            program.child_type_references(bound.arguments),
        );
        let binder = bound.binder.expect("evidence binder symbol");
        let requirement_names = program
            .symbols
            .child_handles(binder)
            .into_iter()
            .flatten()
            .map(|symbol| program.symbols.name(symbol))
            .collect::<Vec<_>>();
        assert!(requirement_names.contains(&own_requirement));
        assert!(!requirement_names.contains(&foreign_requirement));
    }
}

#[test]
fn retains_const_declaration_visibility_after_value_substitution() {
    let source = r#"
        pub const PUBLIC_SIZE: u64 = 4;
        const Buffer::PRIVATE_SIZE: u64 = 2;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize const visibility");
    let syntax = parse_syntax_trees(&tokens).expect("parse const visibility");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve const visibility");

    assert_eq!(program.const_declarations.len(), 2);
    assert!(program.const_declarations[0].is_public);
    assert!(!program.const_declarations[1].is_public);
    assert!(
        program
            .const_declarations
            .iter()
            .all(|declaration| declaration.symbol.is_valid())
    );
    let snapshot = program.snapshot_json().expect("resolved const snapshot");
    assert!(snapshot.contains("\"name\":\"PUBLIC_SIZE\""));
    assert!(snapshot.contains("\"is_public\":true"));
}

#[test]
fn public_const_requires_canonical_compatibility_value_but_private_const_v0_does_not() {
    let private_source = r#"const LABEL: string = "private";"#;
    let tokens = Lexer::new(private_source)
        .tokenize()
        .expect("tokenize private string const");
    let syntax = parse_syntax_trees(&tokens).expect("parse private string const");
    resolve(ResolutionRequest::new(&syntax)).expect("private const-v0 behavior remains unchanged");

    let public_source = r#"pub const LABEL: string = "public";"#;
    let tokens = Lexer::new(public_source)
        .tokenize()
        .expect("tokenize public string const");
    let syntax = parse_syntax_trees(&tokens).expect("parse public string const");
    let diagnostics = resolve(ResolutionRequest::new(&syntax))
        .expect_err("unsupported public const identity must reject rather than emit a weak row");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("public const `LABEL` has no canonical declaration identity")
            && diagnostic.message.contains("not eligible as a const index")
    }));
}

#[test]
fn lowers_dungeon_style_machine_program() {
    let source = r#"
    data Inventory {
        gold: u32;
    }

    pub machine Inventory::clear(&mut self, inventory: &mut Inventory) {
        inventory.gold = 0;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");

    assert_eq!(program.data_definitions.len(), 1);
    assert_eq!(program.machines.len(), 1);
    assert!(program.machines[0].symbol.is_valid());
    assert_eq!(
        program
            .machine_state_handles(program.machines[0].states)
            .len(),
        1
    );
    let state = program.machine_state_handles(program.machines[0].states)[0];
    assert!(program.machine_state(state).symbol.is_valid());
    assert!(
        program
            .symbols
            .find_child_by_name(program.symbols.root(), "u32")
            .is_some()
    );
}

#[test]
fn normalizes_service_rows_from_resolved_boundary_trait_symbols() {
    let source = r#"
    boundary trait Readable {
    }

    boundary trait Filesystem: Readable {
        machine inspect() reaches Readable;
    }

    trait Policy {
    }

    machine backup() reaches Filesystem {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");

    let readable = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Readable")
        .expect("Readable boundary trait");
    let filesystem = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Filesystem")
        .expect("Filesystem boundary trait");
    let readable_id = program
        .service_reaches
        .id_for_symbol(readable.symbol)
        .expect("Readable service id");
    let filesystem_id = program
        .service_reaches
        .id_for_symbol(filesystem.symbol)
        .expect("Filesystem service id");
    assert!(
        program
            .service_reaches
            .id_for_symbol(
                program
                    .traits
                    .iter()
                    .find(|definition| definition.name.as_str() == "Policy")
                    .expect("ordinary policy trait")
                    .symbol,
            )
            .is_none(),
        "ordinary traits must not mint service identities",
    );

    let backup = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "backup")
        .expect("backup machine");
    let mut backup_services = vec![readable_id, filesystem_id];
    backup_services.sort_by_key(|service| service.0);
    assert_eq!(
        program
            .service_reach_rows
            .services(backup.service_reach_row),
        backup_services,
        "authored service rows include normalized boundary-parent closure",
    );

    let inspect = program
        .trait_machine_signatures(filesystem.machines)
        .first()
        .expect("Filesystem::inspect signature");
    assert_eq!(
        program
            .service_reach_rows
            .services(inspect.service_reach_row),
        &[readable_id],
    );
}

#[test]
fn authored_service_reaches_and_invokes_obey_resolution_strata() {
    let base = r#"
        boundary trait Shared {}
        boundary trait BaseOnly {}
        machine base_reach() reaches Shared {}
        machine base_invoke() invokes Shared; {}
    "#;
    let extension = r#"
        boundary trait Shared {}
        machine extension_reach() reaches Shared {}
        machine extension_invoke() invokes Shared; {}
        machine extension_reads_base() reaches BaseOnly invokes BaseOnly; {}
    "#;
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("main.omg"), base.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/services.omg"),
            extension.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let extension_tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize extension");
    let mut syntax =
        parse_syntax_trees_with_id(extension_id, &extension_tokens).expect("parse extension first");
    let base_tokens = Lexer::new(base).tokenize().expect("tokenize base");
    syntax.extend_from(
        &parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse authored base"),
    );
    let program = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve extension-first service rows");

    let service_from = |name: &str, source_id| {
        let definition = program
            .traits
            .iter()
            .find(|definition| {
                definition.name.as_str() == name
                    && program
                        .symbols
                        .symbol_provenance_source_span(definition.symbol)
                        .is_some_and(|span| span.source_id == source_id)
            })
            .expect("source-specific boundary service");
        program
            .service_reaches
            .id_for_symbol(definition.symbol)
            .expect("boundary service id")
    };
    let base_shared = service_from("Shared", base_id);
    let extension_shared = service_from("Shared", extension_id);
    let base_only = service_from("BaseOnly", base_id);

    let machine_services = |name: &str| {
        let machine = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("service-using machine");
        program
            .service_reach_rows
            .services(machine.service_reach_row)
    };
    assert_eq!(machine_services("base_reach"), &[base_shared]);
    assert_eq!(machine_services("base_invoke"), &[base_shared]);
    assert_eq!(machine_services("extension_reach"), &[extension_shared]);
    assert_eq!(machine_services("extension_invoke"), &[extension_shared]);
    assert_eq!(machine_services("extension_reads_base"), &[base_only]);
}

#[test]
fn authored_base_service_names_cannot_resolve_extension_only_declarations() {
    let extension = "boundary trait GeneratedOnly {}";
    let hidden_reach = "machine authored() reaches GeneratedOnly {}";
    let mut reach_sources = SourceMap::default();
    let extension_id = reach_sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/services.omg"),
            extension.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let base_id = reach_sources
        .add(PathBuf::from("main.omg"), hidden_reach.to_owned())
        .source_id;
    let extension_tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize extension");
    let mut reach_syntax =
        parse_syntax_trees_with_id(extension_id, &extension_tokens).expect("parse extension first");
    let base_tokens = Lexer::new(hidden_reach).tokenize().expect("tokenize base");
    reach_syntax.extend_from(
        &parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse authored base"),
    );
    let diagnostic = resolve(ResolutionRequest {
        syntax: &reach_syntax,
        sources: Some(Arc::new(reach_sources)),
        top_level_bindings: Vec::new(),
    })
    .expect_err("Base reaches must not resolve an extension-only service");
    assert!(
        diagnostic[0]
            .message
            .contains("machine `authored` declares unknown boundary service `GeneratedOnly`")
    );

    let hidden_invoke = "machine authored() invokes GeneratedOnly; {}";
    let mut invoke_sources = SourceMap::default();
    let extension_id = invoke_sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/services.omg"),
            extension.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let base_id = invoke_sources
        .add(PathBuf::from("main.omg"), hidden_invoke.to_owned())
        .source_id;
    let extension_tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize extension");
    let mut invoke_syntax =
        parse_syntax_trees_with_id(extension_id, &extension_tokens).expect("parse extension first");
    let base_tokens = Lexer::new(hidden_invoke).tokenize().expect("tokenize base");
    invoke_syntax.extend_from(
        &parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse authored base"),
    );
    let program = resolve(ResolutionRequest {
        syntax: &invoke_syntax,
        sources: Some(Arc::new(invoke_sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("unresolved invokes remains absent from the normalized row");
    let authored = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "authored")
        .expect("authored machine");
    assert!(
        program
            .service_reach_rows
            .services(authored.service_reach_row)
            .is_empty()
    );
}

#[test]
fn rejects_unknown_machine_service_reach_before_resolved_trees() {
    let source = r#"
        machine work()
        reaches MissingService
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = resolve(ResolutionRequest::new(&syntax_trees))
        .expect_err("unknown machine service reach must not enter resolved trees");

    assert!(
        diagnostic[0]
            .message
            .contains("machine `work` declares unknown boundary service `MissingService`")
    );
}

#[test]
fn rejects_ordinary_trait_in_machine_service_reach_before_resolved_trees() {
    let source = r#"
        trait Policy {
        }

        machine work()
        reaches Policy
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = resolve(ResolutionRequest::new(&syntax_trees))
        .expect_err("ordinary traits must not enter a service row");

    assert!(
        diagnostic[0]
            .message
            .contains("machine `work` declares unknown boundary service `Policy`")
    );
}

#[test]
fn rejects_unknown_machine_parameter_service_reach_before_resolved_trees() {
    let source = r#"
        machine invoke<machine F>()
        where machine F() reaches MissingService;
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = resolve(ResolutionRequest::new(&syntax_trees))
        .expect_err("unknown machine-parameter reach must not enter resolved trees");

    assert!(diagnostic[0].message.contains(
        "machine-parameter requirement `F` state `F` declares unknown boundary service `MissingService`"
    ));
}

#[test]
fn rejects_authored_service_reach_on_external_realization_before_resolved_trees() {
    let source = r#"
        boundary trait Process {
            machine exit(code: i32)
            reaches Process;
        }

        machine exit_leaf(code: i32)
        satisfies Process::exit
        via ForeignBinding::Syscall(60)
        reaches Process;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = resolve(ResolutionRequest::new(&syntax_trees))
        .expect_err("external realization must derive rather than repeat service reach");

    assert!(
        diagnostic[0]
            .message
            .contains("repeats an authored `reaches` row")
    );
}
