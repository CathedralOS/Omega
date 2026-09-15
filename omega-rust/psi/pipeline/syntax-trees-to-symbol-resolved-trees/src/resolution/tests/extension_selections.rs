use crate::resolution::{ExtensionRequest, ResolutionRequest, resolve, resolve_extension};
use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};

#[test]
fn seeded_extension_carrier_rebases_selection_suffix_after_later_base_rows() {
    let base_source = "machine helper() {} machine retained() { helper(); }";
    let extension_source = "machine generated() { helper(); }";
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let base_syntax = parse_syntax_trees_with_id(
        base_id,
        &Lexer::new(base_source).tokenize().expect("tokenize base"),
    )
    .expect("parse base");
    let base = resolve(ResolutionRequest {
        syntax: &base_syntax,
        sources: Some(Arc::new(sources.clone())),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve base");
    let helper = base.machines.iter().next().expect("helper");
    let helper_symbol = helper.symbol;
    let helper_state_symbol = base
        .machine_state(base.machine_state_handles(helper.states)[0])
        .symbol;
    let mut destination = base.authored_declaration_selections().clone();
    let base_selection_count = destination.len();
    destination
        .record_resolved(
            source::SourceSpan::new(base_id, source::Span::new(0, 1)),
            symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            symbol_resolved_trees::AuthoredDeclarationSelectionKind::MemberAccess,
            helper_symbol,
        )
        .expect("later-phase-only base selection");
    let extension_syntax = parse_syntax_trees_with_id(
        extension_id,
        &Lexer::new(extension_source)
            .tokenize()
            .expect("tokenize extension"),
    )
    .expect("parse extension");

    let carrier = resolve_extension(ExtensionRequest {
        base,
        syntax: &extension_syntax,
        sources: Arc::new(sources),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve seeded extension");
    let unrebased = carrier.trees();
    assert_eq!(
        unrebased.authored_declaration_selections().len(),
        base_selection_count + 1
    );
    let unrebased_extension =
        unrebased.authored_declaration_selections().as_slice()[base_selection_count];
    assert_eq!(unrebased_extension.source_span().source_id, extension_id);
    let symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target) =
        unrebased_extension.target()
    else {
        panic!("generated helper call is resolved")
    };
    assert_eq!(target.selected_symbol(), helper_state_symbol);
    let generated = unrebased
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "generated")
        .expect("generated machine");
    let state = unrebased.machine_state(unrebased.machine_state_handles(generated.states)[0]);
    let call = unrebased
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            symbol_resolved_trees::statement::StatementNode::Call(call) => Some(call),
            _ => None,
        })
        .expect("unrebased generated helper call");
    assert_eq!(
        call.authored_call_selection,
        Some(unrebased_extension.occurrence_id())
    );

    let rebased = carrier
        .rebase_authored_selections(&destination)
        .expect("rebase exact extension suffix");
    assert_eq!(
        &rebased.authored_declaration_selections().as_slice()[..destination.len()],
        destination.as_slice()
    );
    assert_eq!(
        rebased.authored_declaration_selections().len(),
        destination.len() + 1
    );
    let rebased_extension = rebased.authored_declaration_selections().as_slice()[destination.len()];
    assert_eq!(
        rebased_extension.occurrence_id().ordinal(),
        u64::try_from(destination.len()).expect("selection count")
    );
    assert_eq!(rebased_extension.source_span().source_id, extension_id);
    let symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target) =
        rebased_extension.target()
    else {
        panic!("rebased helper call remains resolved")
    };
    assert_eq!(target.selected_symbol(), helper_state_symbol);
    let shifted = rebased_extension.occurrence_id();
    let generated = rebased
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "generated")
        .expect("generated machine");
    let state = rebased.machine_state(rebased.machine_state_handles(generated.states)[0]);
    let call = rebased
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            symbol_resolved_trees::statement::StatementNode::Call(call) => Some(call),
            _ => None,
        })
        .expect("generated helper call");
    assert_eq!(call.authored_call_selection, Some(shifted));
}

#[test]
fn seeded_extension_preserves_base_identity_and_resolves_base_peers_and_shadowing() {
    let base_source = r#"
        data Shared { base: u32; }
        data Authored { shared: Shared; }
    "#;
    let extension_source = r#"
        data Shared { extension: u64; }
        data Generated { authored: Authored; peer: Peer; shared: Shared; }
        data Peer { value: u32; }
    "#;
    let mut sources = SourceMap::default();
    let base_source_id = sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let extension_source_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/extension.omg"),
            extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let base_tokens = Lexer::new(base_source).tokenize().expect("tokenize base");
    let base_syntax =
        parse_syntax_trees_with_id(base_source_id, &base_tokens).expect("parse base once");
    let base = resolve(ResolutionRequest {
        syntax: &base_syntax,
        sources: Some(Arc::new(sources.clone())),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve retained base");
    let expected_base_roots = base.data_definitions.iter().cloned().collect::<Vec<_>>();
    let expected_base_members = base
        .tables
        .declarations
        .data_members
        .iter()
        .map(|(handle, member)| (handle, member.clone()))
        .collect::<Vec<_>>();
    let expected_base_selections = base.authored_declaration_selections().as_slice().to_vec();
    let base_shared = expected_base_roots[0].symbol;
    let base_authored = expected_base_roots[1].symbol;
    let base_shared_span = base
        .symbols
        .symbol_source_span(base_shared)
        .expect("source-backed base Shared");
    let base_authored_span = base
        .symbols
        .symbol_source_span(base_authored)
        .expect("source-backed base Authored");

    let extension_tokens = Lexer::new(extension_source)
        .tokenize()
        .expect("tokenize extension");
    let extension_syntax = parse_syntax_trees_with_id(extension_source_id, &extension_tokens)
        .expect("parse extension once");
    let program = resolve_extension(ExtensionRequest {
        base,
        syntax: &extension_syntax,
        sources: Arc::new(sources),
        top_level_bindings: Vec::new(),
    })
    .map(|seeded| seeded.into_unrebased_trees())
    .expect("continue resolution from retained base");

    assert_eq!(
        program
            .data_definitions
            .iter()
            .take(expected_base_roots.len())
            .cloned()
            .collect::<Vec<_>>(),
        expected_base_roots,
    );
    assert_eq!(
        program
            .tables
            .declarations
            .data_members
            .iter()
            .take(expected_base_members.len())
            .map(|(handle, member)| (handle, member.clone()))
            .collect::<Vec<_>>(),
        expected_base_members,
    );
    assert_eq!(
        &program.authored_declaration_selections().as_slice()[..expected_base_selections.len()],
        expected_base_selections.as_slice(),
    );
    assert_eq!(
        program.symbols.symbol_source_span(base_shared),
        Some(base_shared_span)
    );
    assert_eq!(
        program.symbols.symbol_source_span(base_authored),
        Some(base_authored_span)
    );

    let extension_shared = program
        .data_definitions
        .iter()
        .find(|definition| {
            definition.name.as_str() == "Shared"
                && definition.name.source_span().source_id == extension_source_id
        })
        .expect("extension Shared");
    let peer = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Peer")
        .expect("extension Peer");
    let generated = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Generated")
        .expect("extension Generated");
    let selected = program
        .data_members(generated.members)
        .iter()
        .map(|member| {
            let symbol_resolved_trees::data::DataMember::Field(field) = member else {
                panic!("Generated has fields only")
            };
            let symbol_resolved_trees::types::TypeReference::Named { symbol, .. } =
                &field.type_reference
            else {
                panic!("Generated fields retain named types")
            };
            *symbol
        })
        .collect::<Vec<_>>();
    assert_eq!(
        selected,
        vec![base_authored, peer.symbol, extension_shared.symbol]
    );
}

#[test]
fn seeded_extension_rejects_duplicates_within_its_own_stratum() {
    let base_source = "data Authored {}";
    let left_source = "const Limits::VALUE: u64 = 1;";
    let right_source = "const Limits::VALUE: u64 = 2;";
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let left_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/left.omg"),
            left_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let right_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/right.omg"),
            right_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let base_tokens = Lexer::new(base_source).tokenize().expect("tokenize base");
    let base_syntax = parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse base once");
    let base = resolve(ResolutionRequest {
        syntax: &base_syntax,
        sources: Some(Arc::new(sources.clone())),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve base");
    let left_tokens = Lexer::new(left_source).tokenize().expect("tokenize left");
    let mut extension = parse_syntax_trees_with_id(left_id, &left_tokens).expect("parse left once");
    let right_tokens = Lexer::new(right_source).tokenize().expect("tokenize right");
    extension.extend_from(
        &parse_syntax_trees_with_id(right_id, &right_tokens).expect("parse right once"),
    );

    let diagnostics = resolve_extension(ExtensionRequest {
        base,
        syntax: &extension,
        sources: Arc::new(sources),
        top_level_bindings: Vec::new(),
    })
    .map(|seeded| seeded.into_unrebased_trees())
    .expect_err("same-stratum duplicate const must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("duplicate const `Limits::VALUE`")
    }));
}

#[test]
fn seeded_extension_retains_base_service_ids_and_authored_reach_provenance() {
    let base_source = r#"
        boundary trait Readable {}
        boundary trait Filesystem: Readable {}
        machine authored() reaches Filesystem {}
    "#;
    let extension_source = r#"
        boundary trait Aardvark {}
        machine generated() reaches Filesystem, Aardvark {}
    "#;
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/reaches.omg"),
            extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let base_tokens = Lexer::new(base_source).tokenize().expect("tokenize base");
    let base_syntax = parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse base once");
    let base = resolve(ResolutionRequest {
        syntax: &base_syntax,
        sources: Some(Arc::new(sources.clone())),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve retained base");
    let filesystem = base
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Filesystem")
        .expect("base Filesystem")
        .symbol;
    let filesystem_id = base
        .service_reaches
        .id_for_symbol(filesystem)
        .expect("base Filesystem service id");
    let authored = base
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "authored")
        .expect("base authored machine");
    let authored_symbol = authored.symbol;
    let authored_row = authored.service_reach_row;
    let authored_provenance = base
        .authored_service_reach_rows_for(authored_symbol)
        .cloned()
        .collect::<Vec<_>>();
    let extension_tokens = Lexer::new(extension_source)
        .tokenize()
        .expect("tokenize extension");
    let extension_syntax =
        parse_syntax_trees_with_id(extension_id, &extension_tokens).expect("parse extension once");

    let program = resolve_extension(ExtensionRequest {
        base,
        syntax: &extension_syntax,
        sources: Arc::new(sources),
        top_level_bindings: Vec::new(),
    })
    .map(|seeded| seeded.into_unrebased_trees())
    .expect("seeded service resolution");
    assert_eq!(
        program.service_reaches.id_for_symbol(filesystem),
        Some(filesystem_id)
    );
    let retained_authored = program
        .machines
        .iter()
        .find(|machine| machine.symbol == authored_symbol)
        .expect("retained authored machine");
    assert_eq!(retained_authored.service_reach_row, authored_row);
    assert_eq!(
        program
            .authored_service_reach_rows_for(authored_symbol)
            .cloned()
            .collect::<Vec<_>>(),
        authored_provenance,
    );
    let generated = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "generated")
        .expect("generated machine");
    let generated_services = program
        .service_reach_rows
        .services(generated.service_reach_row)
        .iter()
        .filter_map(|service| program.service_reaches.definition(*service))
        .map(|definition| definition.name.as_str())
        .collect::<Vec<_>>();
    assert!(generated_services.contains(&"Filesystem"));
    assert!(generated_services.contains(&"Aardvark"));
}

#[test]
fn explicit_top_level_boundary_requirement_satisfaction_resolves_exact_machine_symbol() {
    let provider_source = r#"
        machine Carrier::operation(value: u32) {}
        machine checked_provider(value: u32)
        satisfies Carrier::operation
        {}
    "#;
    let requirement_source = "pub boundary requirement Carrier::operation(value: u32);";
    let mut sources = SourceMap::default();
    let provider_source_id = sources
        .add(PathBuf::from("provider.omg"), provider_source.to_owned())
        .source_id;
    let requirement_source_id = sources
        .add(
            PathBuf::from("requirements.omg"),
            requirement_source.to_owned(),
        )
        .source_id;
    let provider_tokens = Lexer::new(provider_source)
        .tokenize()
        .expect("tokenize provider");
    let mut syntax = parse_syntax_trees_with_id(provider_source_id, &provider_tokens)
        .expect("parse provider source");
    let requirement_tokens = Lexer::new(requirement_source)
        .tokenize()
        .expect("tokenize explicit requirement");
    let requirement_syntax = parse_syntax_trees_with_id(requirement_source_id, &requirement_tokens)
        .expect("parse explicit requirement");
    syntax.extend_from(&requirement_syntax);
    let program = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: vec![symbols::SourceScopedTopLevelBinding::new(
            provider_source_id,
            requirement_source_id,
            "Carrier::operation",
        )],
    })
    .expect("resolve explicit requirement satisfaction");
    let requirements = program
        .machines
        .iter()
        .filter(|machine| {
            machine.name.as_str() == "Carrier::operation"
                && matches!(
                    machine.supply_mode,
                    language_semantics::MachineSupplyMode::TopLevelRequirement
                )
        })
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        panic!("one explicit requirement")
    };
    let provider = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "checked_provider")
        .expect("checked provider");
    let [satisfaction] = program.machine_trait_conformances(provider.satisfies) else {
        panic!("one satisfaction row")
    };

    assert!(requirement.name.is_source_backed());
    assert!(satisfaction.name.is_source_backed());
    assert_eq!(satisfaction.symbol, requirement.symbol);
    assert_eq!(
        program.symbols.get(satisfaction.symbol).kind,
        symbols::SymbolKind::Machine
    );
}

#[test]
fn trait_requirement_satisfaction_retains_trait_symbol_resolution() {
    let source = r#"
        boundary trait Carrier { machine operation(value: u32); }
        machine checked_provider(value: u32) satisfies Carrier::operation {}
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize trait provider");
    let syntax = parse_syntax_trees(&tokens).expect("parse trait requirement");
    let program =
        resolve(ResolutionRequest::new(&syntax)).expect("resolve trait requirement satisfaction");
    let trait_definition = program.traits.first().expect("Carrier trait");
    let provider = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "checked_provider")
        .expect("checked provider");
    let [satisfaction] = program.machine_trait_conformances(provider.satisfies) else {
        panic!("one satisfaction row")
    };

    assert_eq!(satisfaction.symbol, trait_definition.symbol);
    assert_eq!(
        program.symbols.get(satisfaction.symbol).kind,
        symbols::SymbolKind::Trait
    );
}

#[test]
fn top_level_boundary_requirement_satisfaction_rejects_wrong_kind_and_missing_target() {
    for (source, expected) in [
        (
            r#"
                machine Carrier::operation(value: u32) {}
                machine provider(value: u32) satisfies Carrier::operation {}
            "#,
            "is an ordinary machine, not an explicit top-level `boundary requirement`",
        ),
        (
            "machine provider(value: u32) satisfies Missing::operation {}",
            "does not resolve to an exact trait requirement or top-level `boundary requirement`",
        ),
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize invalid target");
        let syntax = parse_syntax_trees(&tokens).expect("parse invalid target");
        let diagnostics = resolve(ResolutionRequest::new(&syntax))
            .expect_err("invalid satisfaction target must fail symbol assignment");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "expected `{expected}`, got {diagnostics:?}"
        );
    }
}

#[test]
fn trait_machine_requirement_identity_reaches_resolved_trees() {
    let tokens = Lexer::new("trait PrivateCallbackSlot<machine Requirement> {}")
        .tokenize()
        .expect("tokenize trait machine requirement parameter");
    let syntax = parse_syntax_trees(&tokens).expect("parse trait machine requirement parameter");
    let program = resolve(ResolutionRequest::new(&syntax))
        .expect("resolve trait machine requirement parameter");
    let trait_definition = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "PrivateCallbackSlot")
        .expect("PrivateCallbackSlot trait");
    let [parameter] = program.trait_type_parameters(trait_definition) else {
        panic!("one resolved trait machine requirement parameter")
    };
    assert!(parameter.symbol.is_valid());
    assert!(matches!(
        parameter.kind,
        symbol_resolved_trees::data::TypeParameterKind::Machine {
            contract: symbol_resolved_trees::data::MachineParameterContract::RequirementIdentity
        }
    ));
}

#[test]
fn trait_machine_requirement_argument_resolves_one_exact_requirement() {
    let source = r#"
        boundary trait WindowProcedure { machine call(value: u32); }
        trait PrivateCallbackSlot<machine Requirement> {}
        data WndClassLayout {}
        WndClassWindowProcedureSlot:
            WndClassLayout satisfies PrivateCallbackSlot<WindowProcedure::call>;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize private callback slot");
    let syntax = parse_syntax_trees(&tokens).expect("parse private callback slot");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve private callback slot");
    let window_procedure = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "WindowProcedure")
        .expect("WindowProcedure trait");
    let requirement = program
        .trait_machine_signatures(window_procedure.machines)
        .first()
        .expect("WindowProcedure::call");
    let conformance = program.conformances.first().expect("slot conformance");
    let [argument] = program.child_type_references(conformance.arguments) else {
        panic!("one slot requirement argument")
    };
    let symbol_resolved_trees::types::TypeReference::Named { symbol, name } = argument else {
        panic!("requirement argument remains a named identity")
    };
    assert_eq!(name.as_str(), "WindowProcedure::call");
    assert!(name.is_source_backed());
    assert_eq!(*symbol, requirement.symbol);
}

#[test]
fn authored_trait_machine_identity_uses_the_exact_base_trait_catalog() {
    let base = r#"
        boundary trait WindowProcedure { machine call(value: u32); }
        trait PrivateCallbackSlot<machine Requirement> {}
        data WndClassLayout {}
        Slot: WndClassLayout satisfies PrivateCallbackSlot<WindowProcedure::call>;
    "#;
    let extension = r#"
        boundary trait WindowProcedure { machine call(value: i64); }
        trait PrivateCallbackSlot<Requirement> {}
        data WndClassLayout {}
    "#;
    let mut sources = SourceMap::default();
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
    let base_id = sources
        .add(PathBuf::from("base.omg"), base.to_owned())
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
    .expect("authored machine identity must use the exact base trait catalog");
    let conformance = program
        .conformances
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "Slot")
        })
        .expect("base Slot conformance");
    let [symbol_resolved_trees::types::TypeReference::Named { symbol, .. }] =
        program.child_type_references(conformance.arguments)
    else {
        panic!("one named machine-identity argument")
    };
    assert_eq!(
        program
            .symbols
            .symbol_provenance_source_span(*symbol)
            .expect("source-backed requirement")
            .source_id,
        base_id
    );
}

#[test]
fn authored_quotient_paths_ignore_extension_first_declarations() {
    let base = r#"
        data Carrier {}
        proposition equivalent(left: Carrier, right: Carrier) = true;
        trait RelationEvidence<C> {}
        Proof: satisfies RelationEvidence<Carrier> {}
        data Q = Carrier % equivalent
        where equivalent satisfies RelationEvidence<Carrier> as Proof;
    "#;
    let extension = r#"
        data Carrier {}
        proposition equivalent(left: Carrier, right: Carrier) = true;
        trait RelationEvidence<C> {}
        Proof: satisfies RelationEvidence<Carrier> {}
    "#;
    let mut sources = SourceMap::default();
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
    let base_id = sources
        .add(PathBuf::from("base.omg"), base.to_owned())
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
    .expect("authored quotient paths must ignore extension declarations");
    let quotient = program
        .data_definitions
        .iter()
        .find(|definition| {
            definition.name.as_str() == "Q" && definition.name.source_span().source_id == base_id
        })
        .and_then(|definition| definition.quotient.as_ref())
        .expect("base quotient metadata");
    let selection = quotient
        .equivalence
        .as_ref()
        .expect("equivalence selection");
    for symbol in [
        quotient.relation_symbol,
        selection.relation_symbol,
        selection.trait_symbol,
        selection.conformance_symbol,
    ] {
        assert_eq!(
            program
                .symbols
                .symbol_provenance_source_span(symbol)
                .expect("source-backed quotient selection")
                .source_id,
            base_id
        );
    }
}

#[test]
fn authored_conformance_result_dispatch_ignores_an_extension_domain_alias() {
    let base = r#"
        domain i32::Left;
        domain i32::Right;
        data Item {}
        trait Pick {
            machine Self::pick(&self) -> i32 in Left;
            machine Self::pick(&self) -> i32 in Right;
        }
        Selected: Item satisfies Pick {
            machine pick(&self) -> i32 in Left { 0 }
            machine pick(&self) -> i32 in Right { 0 }
        }
    "#;
    let extension = "domain i32::Left = i32::Right;";
    let mut sources = SourceMap::default();
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
    let base_id = sources
        .add(PathBuf::from("base.omg"), base.to_owned())
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
    .expect("the extension Left alias must not make both Base overloads dispatch as Right");
    let conformance = program
        .conformances
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "Selected")
        })
        .expect("Selected conformance");
    let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed conformance")
    };
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].requirement, rows[1].requirement);
    assert_ne!(rows[0].realization_state, rows[1].realization_state);
}

#[test]
fn trait_machine_requirement_argument_rejects_non_requirement_and_overload() {
    for (source, expected) in [
        (
            r#"
                trait PrivateCallbackSlot<machine Requirement> {}
                data WndClassLayout {}
                Bad: WndClassLayout satisfies PrivateCallbackSlot<WndClassLayout>;
            "#,
            "expected one exact `Trait::requirement` path",
        ),
        (
            r#"
                boundary trait WindowProcedure {
                    machine call(value: u32);
                    machine call(value: u64);
                }
                trait PrivateCallbackSlot<machine Requirement> {}
                data WndClassLayout {}
                Bad: WndClassLayout satisfies PrivateCallbackSlot<WindowProcedure::call>;
            "#,
            "signature-free references reject overloads",
        ),
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize invalid slot");
        let syntax = parse_syntax_trees(&tokens).expect("parse invalid slot");
        let diagnostics =
            resolve(ResolutionRequest::new(&syntax)).expect_err("invalid slot must fail closed");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "expected `{expected}`, got {diagnostics:?}"
        );
    }
}

#[test]
fn retains_public_conformance_visibility_and_snapshot_shape() {
    let source = "pub trait Ranked {} pub data Card {} pub PowerOrder: Card satisfies Ranked {}";
    let tokens = Lexer::new(source).tokenize().expect("tokenize conformance");
    let syntax = parse_syntax_trees(&tokens).expect("parse conformance");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve conformance");
    let conformance = program.conformances.iter().next().expect("conformance");

    assert!(conformance.is_public);
    assert_eq!(
        conformance.alias.as_ref().map(|name| name.as_str()),
        Some("PowerOrder")
    );
    assert!(conformance.symbol.is_valid());
    let snapshot = program.snapshot();
    assert_eq!(snapshot.roots.conformances.len(), 1);
    assert!(snapshot.roots.conformances[0].is_public);
    assert_eq!(snapshot.roots.conformances[0].name, "PowerOrder");
}

#[test]
fn retains_public_data_trait_and_wire_visibility() {
    let source = r#"
        pub data PublicRecord { value: u32; }
        pub data Packet { #1 value: u32; }
        pub trait PublicTrait {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize data");
    let syntax = parse_syntax_trees(&tokens).expect("parse data");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve data");
    let public = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "PublicRecord")
        .expect("public data");
    let wire_derived = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "Packet")
        .expect("wire-derived data");
    let wire_schema = program
        .wire_schemas
        .iter()
        .find(|schema| schema.name.as_str() == "Packet")
        .expect("wire schema");
    let trait_definition = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "PublicTrait")
        .expect("public trait");

    assert!(public.is_public);
    assert!(wire_derived.is_public);
    assert!(wire_schema.is_public);
    assert!(trait_definition.is_public);
    let snapshot = program.snapshot();
    assert!(
        snapshot
            .roots
            .wire_schemas
            .iter()
            .any(|schema| schema.name == "Packet" && schema.is_public)
    );
    assert!(
        snapshot
            .roots
            .traits
            .iter()
            .any(|definition| definition.name == "PublicTrait" && definition.is_public)
    );
}

#[test]
fn resolves_wire_owned_nested_field_type_identity() {
    let source = r#"
        data Header { #0 value: u32; }
        data Message { #0 header: Header; }
    "#;
    let mut sources = SourceMap::default();
    let source_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("package/main.omg"),
            source.to_owned(),
            PathBuf::from("package"),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::Base,
        )
        .source_id;
    let tokens = Lexer::new(source).tokenize().expect("tokenize wire data");
    let syntax = parse_syntax_trees_with_id(source_id, &tokens).expect("parse wire data");
    let program = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve source-owned wire data");
    let header_symbol = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "Header")
        .expect("wire-derived Header data")
        .symbol;
    let message = program
        .wire_schemas
        .iter()
        .find(|schema| schema.name.as_str() == "Message")
        .expect("Message wire schema");
    let [symbol_resolved_trees::wire::WireMember::Field(header)] =
        program.wire_members(message.members)
    else {
        panic!("Message should retain one wire field")
    };
    let symbol_resolved_trees::types::TypeReference::Named { symbol, name } =
        &header.type_reference
    else {
        panic!("nested wire field should retain its named type")
    };

    assert_eq!(name.as_str(), "Header");
    assert_eq!(*symbol, header_symbol);
}

#[test]
fn retains_public_machine_visibility_in_symbol_resolved_trees() {
    let tokens = Lexer::new("pub machine Package::entry() { }")
        .tokenize()
        .expect("tokenize public machine");
    let syntax = parse_syntax_trees(&tokens).expect("parse public machine");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve public machine");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Package::entry")
        .expect("resolved public machine");

    assert!(machine.is_public);
    assert_eq!(
        machine.attached_data.as_ref().map(|name| name.as_str()),
        Some("Package")
    );
    assert_eq!(
        machine.supply_mode,
        language_semantics::MachineSupplyMode::CheckedBody
    );
}

#[test]
fn resolves_machine_and_trait_const_parameter_carrier_types() {
    let source = r#"
        pub machine measure<const Width: u64>() { }
        pub trait Sized<const Width: u64> { }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize const parameters");
    let syntax = parse_syntax_trees(&tokens).expect("parse const parameters");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve const parameters");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "measure")
        .expect("measure machine");
    let trait_definition = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Sized")
        .expect("Sized trait");
    for parameter in [
        &program.machine_type_parameters(machine)[0],
        &program.trait_type_parameters(trait_definition)[0],
    ] {
        let symbol_resolved_trees::data::TypeParameterKind::Const { type_reference } =
            &parameter.kind
        else {
            panic!("const parameter")
        };
        let symbol_resolved_trees::types::TypeReference::Named { symbol, name } = type_reference
        else {
            panic!("named const carrier")
        };
        assert_eq!(name.as_str(), "u64");
        assert!(symbol.is_valid());
        assert_eq!(
            program.symbols.get(*symbol).kind,
            symbols::SymbolKind::BuiltinType
        );
    }
}

#[test]
fn resolves_provider_selection_type_paths_to_exact_symbols() {
    let source = r#"
        boundary trait Console { machine write(); }
        data ConsoleProvider { }
        machine build(builder: &mut Build) {
            builder.select_provider<Console, ConsoleProvider>();
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize provider selection");
    let syntax = parse_syntax_trees(&tokens).expect("parse provider selection");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve provider selection");
    let build = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("build machine");
    let state = program.machine_state(program.machine_state_handles(build.states)[0]);
    let call = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            symbol_resolved_trees::statement::StatementNode::Call(call)
                if call.target.as_str() == "select_provider" =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("provider-selection statement call");
    let [boundary, provider] = call.machine_arguments.as_ref() else {
        panic!("two retained provider-selection arguments")
    };

    assert_eq!(
        program.symbols.get(boundary.symbol).kind,
        symbols::SymbolKind::Trait
    );
    assert_eq!(
        program.symbols.get(provider.symbol).kind,
        symbols::SymbolKind::Data
    );
    assert_eq!(program.symbols.name(boundary.symbol), "Console");
    assert_eq!(program.symbols.name(provider.symbol), "ConsoleProvider");
}

#[test]
fn resolves_top_level_requirement_provider_selection_to_exact_machine_symbol() {
    let source = r#"
        pub boundary requirement InterruptAcknowledgement::complete();
        data LapicCompletion {}
        machine build(builder: &mut Build) {
            builder.select_provider<InterruptAcknowledgement::complete, LapicCompletion>();
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize top-level provider selection");
    let syntax = parse_syntax_trees(&tokens).expect("parse top-level provider selection");
    let program =
        resolve(ResolutionRequest::new(&syntax)).expect("resolve top-level provider selection");
    let requirement = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "InterruptAcknowledgement::complete")
        .expect("top-level requirement");
    let build = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("build machine");
    let state = program.machine_state(program.machine_state_handles(build.states)[0]);
    let call = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            symbol_resolved_trees::statement::StatementNode::Call(call)
                if call.target.as_str() == "select_provider" =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("provider-selection statement call");
    let [subject, provider] = call.machine_arguments.as_ref() else {
        panic!("two retained provider-selection arguments")
    };
    assert_eq!(subject.symbol, requirement.symbol);
    assert_eq!(
        program.symbols.get(subject.symbol).kind,
        symbols::SymbolKind::Machine
    );
    assert_eq!(
        program.symbols.get(provider.symbol).kind,
        symbols::SymbolKind::Data
    );
}

#[test]
fn authored_build_selection_paths_cannot_fall_back_to_extension_declarations() {
    let base = r#"
        machine build(builder: &mut Build) {
            builder.select_provider<GeneratedBoundary, GeneratedProvider>();
            builder.select_representation<GeneratedOpaque, GeneratedRepresentation>();
        }
    "#;
    let extension = r#"
        boundary trait GeneratedBoundary { machine enter(); }
        data GeneratedProvider {}
        data GeneratedOpaque {}
        trait Shape {}
        GeneratedRepresentation: GeneratedOpaque satisfies Shape {}
    "#;
    let mut sources = SourceMap::default();
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
    let base_id = sources
        .add(PathBuf::from("main.omg"), base.to_owned())
        .source_id;
    let extension_tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize extension");
    let mut syntax = parse_syntax_trees_with_id(extension_id, &extension_tokens)
        .expect("parse extension source");
    let base_tokens = Lexer::new(base).tokenize().expect("tokenize base");
    let base_syntax = parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse base source");
    syntax.extend_from(&base_syntax);

    let program = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolved lowering retains invalid hidden build selections for diagnostics");
    let build = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("build machine");
    let state = program.machine_state(program.machine_state_handles(build.states)[0]);
    let calls = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            symbol_resolved_trees::statement::StatementNode::Call(call) => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    for call in calls {
        assert!(
            call.machine_arguments
                .iter()
                .all(|argument| !argument.symbol.is_valid()),
            "authored `{}` arguments must not resurrect extension declarations: {:?}",
            call.target,
            call.machine_arguments
        );
    }
}

#[test]
fn resolves_name_owned_conformance_telescope_in_its_own_scope() {
    let source = r#"
        trait Converter<Source, Target> {}

        GenericConversion<'scope, Source, const Width: u64, machine Convert>:
            Source satisfies Converter<Source, u64>
        where machine Convert(value: Source) -> u64;
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let conformance = program.conformances.iter().next().expect("one conformance");

    assert_eq!(conformance.lifetime_parameters.len(), 1);
    assert_eq!(conformance.lifetime_parameters[0].as_str(), "scope");
    assert_eq!(
        program.symbols.get(conformance.symbol).parent,
        program.symbols.root(),
        "a conformance name is package-scoped even when its subject is a carrier"
    );
    let parameters = program.data_type_parameters(conformance.type_parameters);
    assert_eq!(parameters.len(), 3);
    assert!(
        parameters
            .iter()
            .all(|parameter| parameter.symbol.is_valid())
    );
    assert!(
        parameters
            .iter()
            .all(|parameter| program.symbols.get(parameter.symbol).parent == conformance.symbol)
    );
    assert_eq!(conformance.carrier_symbol, parameters[0].symbol);
    let converter = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Converter")
        .expect("Converter trait");
    assert_eq!(conformance.trait_symbol, converter.symbol);

    let source_argument = program
        .tables
        .declarations
        .child_type_references
        .span_or_empty(conformance.arguments)
        .first()
        .expect("Source trait argument");
    let symbol_resolved_trees::types::TypeReference::Named { symbol, name } = source_argument
    else {
        panic!("Source should remain a named reference");
    };
    assert_eq!(name.as_str(), "Source");
    assert_eq!(*symbol, parameters[0].symbol);

    let symbol_resolved_trees::data::TypeParameterKind::Machine { contract } = &parameters[2].kind
    else {
        panic!("Convert should be a machine parameter");
    };
    let contract = program
        .machine_parameter_contract_view(contract)
        .expect("structural machine contract")
        .signature();
    let contract_parameter = program
        .state_parameters(contract.parameters)
        .first()
        .expect("Convert value parameter");
    let symbol_resolved_trees::types::TypeReference::Named { symbol, name } =
        &contract_parameter.type_reference
    else {
        panic!("contract parameter should remain named");
    };
    assert_eq!(name.as_str(), "Source");
    assert_eq!(*symbol, parameters[0].symbol);
}

#[test]
fn resolves_forward_declared_nominal_machine_parameter_to_exact_requirement() {
    let source = r#"
        machine register<machine Selected>(value: u32) -> u64
        where machine Selected satisfies WindowProcedure::call;
        {
            Selected(value)
        }

        boundary trait WindowProcedure {
            machine call(value: u32) -> u64;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve nominal requirement");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "register")
        .expect("register machine");
    let parameter = program
        .machine_type_parameters(machine)
        .first()
        .expect("Selected parameter");
    let symbol_resolved_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind
    else {
        panic!("Selected should be a machine parameter");
    };
    let symbol_resolved_trees::data::MachineParameterContract::Nominal {
        trait_definition,
        requirement,
        authored_path,
    } = contract
    else {
        panic!("Selected should retain an exact nominal requirement");
    };
    let trait_definition_row = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "WindowProcedure")
        .expect("WindowProcedure trait");
    let requirement_row = program
        .trait_machine_signatures(trait_definition_row.machines)
        .first()
        .expect("call requirement");

    assert_eq!(*trait_definition, trait_definition_row.symbol);
    assert_eq!(*requirement, requirement_row.symbol);
    assert_eq!(
        authored_path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["WindowProcedure", "call"]
    );
    assert_ne!(parameter.symbol, requirement_row.symbol);
    let symbol_resolved_trees::data::MachineParameterContractView::Nominal {
        trait_definition,
        requirement,
    } = program
        .machine_parameter_contract_view(contract)
        .expect("valid nominal contract view")
    else {
        panic!("nominal view")
    };
    assert_eq!(trait_definition.name.as_str(), "WindowProcedure");
    assert_eq!(requirement.name.as_str(), "call");
    assert_eq!(program.state_parameters(requirement.parameters).len(), 1);
}

#[test]
fn rejects_overloaded_nominal_machine_parameter_requirement() {
    let source = r#"
        trait WindowProcedure {
            machine call(value: u32) -> u64;
            machine call(value: u64) -> u64;
        }

        machine register<machine Selected>()
        where machine Selected satisfies WindowProcedure::call;
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let diagnostic = resolve(ResolutionRequest::new(&syntax)).expect_err("overload must reject");

    assert_eq!(diagnostic.len(), 2);
    assert!(
        diagnostic[0]
            .message
            .contains("declaring trait `WindowProcedure`")
    );
    assert!(diagnostic[0].message.contains("source-compatibility break"));
    assert!(
        diagnostic[1]
            .message
            .contains("does not resolve to one exact trait requirement")
    );
    assert!(
        diagnostic
            .iter()
            .all(|diagnostic| diagnostic.source_span.is_some())
    );
}

#[test]
fn rejects_unknown_nominal_machine_parameter_paths() {
    for (path, expected) in [
        ("MissingTrait::call", "does not resolve to one exact trait"),
        (
            "WindowProcedure::missing",
            "does not resolve to one exact trait requirement",
        ),
    ] {
        let source = format!(
            r#"
                trait WindowProcedure {{
                    machine call(value: u32) -> u64;
                }}

                machine register<machine Selected>()
                where machine Selected satisfies {path};
                {{}}
            "#
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let diagnostic =
            resolve(ResolutionRequest::new(&syntax)).expect_err("unknown path must reject");
        assert!(
            diagnostics::format_diagnostics(&diagnostic).contains(expected),
            "unexpected diagnostic for {path}: {diagnostic:?}"
        );
    }
}
