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

    let (rebased, _) = carrier
        .rebase_authored_selections_for_typed_continuation(&destination)
        .expect("rebase exact extension suffix")
        .into_typing_continuation_parts();
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
    .map(|seeded| seeded.trees().clone())
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
    .map(|seeded| seeded.trees().clone())
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
