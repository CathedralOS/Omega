use super::{
    Lowerer, lower_syntax_extension_against_resolved_base,
    lower_syntax_extension_with_authored_selection_frontier, lower_syntax_trees,
    lower_syntax_trees_with_sources, lower_syntax_trees_with_sources_and_top_level_bindings,
};
use psi_source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use psi_source_files_to_tokens::Lexer;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees_with_id;
use std::path::PathBuf;
use std::sync::Arc;

mod state_local_resolution;

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
    let base = lower_syntax_trees_with_sources(&base_syntax, Arc::new(sources.clone()))
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
            psi_source::SourceSpan::new(base_id, psi_source::Span::new(0, 1)),
            psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::MemberAccess,
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

    let carrier = lower_syntax_extension_with_authored_selection_frontier(
        base,
        &extension_syntax,
        Arc::new(sources),
        Vec::new(),
    )
    .expect("resolve seeded extension");
    let unrebased = carrier.trees();
    assert_eq!(
        unrebased.authored_declaration_selections().len(),
        base_selection_count + 1
    );
    let unrebased_extension =
        unrebased.authored_declaration_selections().as_slice()[base_selection_count];
    assert_eq!(unrebased_extension.source_span().source_id, extension_id);
    let psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target) =
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
            psi_symbol_resolved_trees::statement::StatementNode::Call(call) => Some(call),
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
    let psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target) =
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
            psi_symbol_resolved_trees::statement::StatementNode::Call(call) => Some(call),
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
    let base = lower_syntax_trees_with_sources(&base_syntax, Arc::new(sources.clone()))
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
    let program = lower_syntax_extension_against_resolved_base(
        base,
        &extension_syntax,
        Arc::new(sources),
        Vec::new(),
    )
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
            let psi_symbol_resolved_trees::data::DataMember::Field(field) = member else {
                panic!("Generated has fields only")
            };
            let psi_symbol_resolved_trees::types::TypeReference::Named { symbol, .. } =
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
    let base = lower_syntax_trees_with_sources(&base_syntax, Arc::new(sources.clone()))
        .expect("resolve base");
    let left_tokens = Lexer::new(left_source).tokenize().expect("tokenize left");
    let mut extension = parse_syntax_trees_with_id(left_id, &left_tokens).expect("parse left once");
    let right_tokens = Lexer::new(right_source).tokenize().expect("tokenize right");
    extension.extend_from(
        &parse_syntax_trees_with_id(right_id, &right_tokens).expect("parse right once"),
    );

    let diagnostics = lower_syntax_extension_against_resolved_base(
        base,
        &extension,
        Arc::new(sources),
        Vec::new(),
    )
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
    let base = lower_syntax_trees_with_sources(&base_syntax, Arc::new(sources.clone()))
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

    let program = lower_syntax_extension_against_resolved_base(
        base,
        &extension_syntax,
        Arc::new(sources),
        Vec::new(),
    )
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
    let program = lower_syntax_trees_with_sources_and_top_level_bindings(
        &syntax,
        Arc::new(sources),
        vec![psi_symbols::SourceScopedTopLevelBinding::new(
            provider_source_id,
            requirement_source_id,
            "Carrier::operation",
        )],
    )
    .expect("resolve explicit requirement satisfaction");
    let requirements = program
        .machines
        .iter()
        .filter(|machine| {
            machine.name.as_str() == "Carrier::operation"
                && matches!(
                    machine.supply_mode,
                    psi_language_semantics::MachineSupplyMode::TopLevelRequirement
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
        psi_symbols::SymbolKind::Machine
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
    let program = lower_syntax_trees(&syntax).expect("resolve trait requirement satisfaction");
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
        psi_symbols::SymbolKind::Trait
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
        let diagnostics = lower_syntax_trees(&syntax)
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
    let program = lower_syntax_trees(&syntax).expect("resolve trait machine requirement parameter");
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
        psi_symbol_resolved_trees::data::TypeParameterKind::Machine {
            contract:
                psi_symbol_resolved_trees::data::MachineParameterContract::RequirementIdentity
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
    let program = lower_syntax_trees(&syntax).expect("resolve private callback slot");
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
    let psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name } = argument else {
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

    let program = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
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
    let [psi_symbol_resolved_trees::types::TypeReference::Named { symbol, .. }] =
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

    let program = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
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

    let program = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
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
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
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
        let diagnostics = lower_syntax_trees(&syntax).expect_err("invalid slot must fail closed");
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
    let program = lower_syntax_trees(&syntax).expect("resolve conformance");
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
    let program = lower_syntax_trees(&syntax).expect("resolve data");
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
    let program = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
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
    let [psi_symbol_resolved_trees::wire::WireMember::Field(header)] =
        program.wire_members(message.members)
    else {
        panic!("Message should retain one wire field")
    };
    let psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name } =
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
    let program = lower_syntax_trees(&syntax).expect("resolve public machine");
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
        psi_language_semantics::MachineSupplyMode::CheckedBody
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
    let program = lower_syntax_trees(&syntax).expect("resolve const parameters");
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
        let psi_symbol_resolved_trees::data::TypeParameterKind::Const { type_reference } =
            &parameter.kind
        else {
            panic!("const parameter")
        };
        let psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name } =
            type_reference
        else {
            panic!("named const carrier")
        };
        assert_eq!(name.as_str(), "u64");
        assert!(symbol.is_valid());
        assert_eq!(
            program.symbols.get(*symbol).kind,
            psi_symbols::SymbolKind::BuiltinType
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
    let program = lower_syntax_trees(&syntax).expect("resolve provider selection");
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
            psi_symbol_resolved_trees::statement::StatementNode::Call(call)
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
        psi_symbols::SymbolKind::Trait
    );
    assert_eq!(
        program.symbols.get(provider.symbol).kind,
        psi_symbols::SymbolKind::Data
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
    let program = lower_syntax_trees(&syntax).expect("resolve top-level provider selection");
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
            psi_symbol_resolved_trees::statement::StatementNode::Call(call)
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
        psi_symbols::SymbolKind::Machine
    );
    assert_eq!(
        program.symbols.get(provider.symbol).kind,
        psi_symbols::SymbolKind::Data
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

    let program = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
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
            psi_symbol_resolved_trees::statement::StatementNode::Call(call) => Some(call),
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
    let program = lower_syntax_trees(&syntax).expect("resolve");
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
    let psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name } = source_argument
    else {
        panic!("Source should remain a named reference");
    };
    assert_eq!(name.as_str(), "Source");
    assert_eq!(*symbol, parameters[0].symbol);

    let psi_symbol_resolved_trees::data::TypeParameterKind::Machine { contract } =
        &parameters[2].kind
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
    let psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name } =
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
    let program = lower_syntax_trees(&syntax).expect("resolve nominal requirement");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "register")
        .expect("register machine");
    let parameter = program
        .machine_type_parameters(machine)
        .first()
        .expect("Selected parameter");
    let psi_symbol_resolved_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind
    else {
        panic!("Selected should be a machine parameter");
    };
    let psi_symbol_resolved_trees::data::MachineParameterContract::Nominal {
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
    let psi_symbol_resolved_trees::data::MachineParameterContractView::Nominal {
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
    let diagnostic = lower_syntax_trees(&syntax).expect_err("overload must reject");

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
        let diagnostic = lower_syntax_trees(&syntax).expect_err("unknown path must reject");
        assert!(
            psi_diagnostics::format_diagnostics(&diagnostic).contains(expected),
            "unexpected diagnostic for {path}: {diagnostic:?}"
        );
    }
}

#[test]
fn nominal_machine_parameter_view_rejects_mismatched_trait_requirement_pair() {
    let source = r#"
        trait First { machine call(value: u32) -> u64; }
        trait Second { machine call(value: u32) -> u64; }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax).expect("resolve traits");
    let first = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "First")
        .expect("First trait");
    let second = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Second")
        .expect("Second trait");
    let second_requirement = program
        .trait_machine_signatures(second.machines)
        .first()
        .expect("Second::call");
    let mismatched = psi_symbol_resolved_trees::data::MachineParameterContract::Nominal {
        trait_definition: first.symbol,
        requirement: second_requirement.symbol,
        authored_path: Vec::new(),
    };

    assert!(
        program
            .machine_parameter_contract_view(&mismatched)
            .is_none()
    );
}

#[test]
fn resolves_explicit_conformance_binder_as_proof_static_machine_child() {
    let source = r#"
        trait Ranked {}

        machine sort<Element, Order: Element satisfies Ranked>(
            values: &mut [Element]
        ) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax).expect("resolve");
    let machine = program.machines.iter().next().expect("machine");
    let [bound] = machine.conformance_bounds.as_slice() else {
        panic!("one explicit conformance binder");
    };
    let binder = bound.binder.expect("binder symbol");
    assert!(binder.is_valid());
    assert_eq!(program.symbols.name(binder), "Order");
    assert_eq!(program.symbols.get(binder).parent, machine.symbol);
    assert_eq!(
        program.symbols.get(binder).kind,
        psi_symbols::SymbolKind::ConformanceParameter
    );
    let parameters = program.machine_type_parameters(machine);
    assert_eq!(parameters.len(), 1);
    assert_eq!(bound.subject, parameters[0].symbol);
    assert!(bound.carrier.is_valid());
    assert_eq!(program.symbols.name(bound.carrier), "Ranked");
}

#[test]
fn resolves_explicit_conformance_binder_as_proof_static_trait_child() {
    let source = r#"
        trait Ranked<Metric> {}

        trait Ordering<Element, Order: Element satisfies Ranked<u32>> {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax).expect("resolve");
    let ordering = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Ordering")
        .expect("Ordering trait");
    let [bound] = ordering.conformance_bounds.as_slice() else {
        panic!("one explicit conformance binder");
    };
    let binder = bound.binder.expect("binder symbol");
    assert!(binder.is_valid());
    assert_eq!(program.symbols.name(binder), "Order");
    assert_eq!(program.symbols.get(binder).parent, ordering.symbol);
    assert_eq!(
        program.symbols.get(binder).kind,
        psi_symbols::SymbolKind::ConformanceParameter
    );
    let parameters = program.trait_type_parameters(ordering);
    assert_eq!(parameters.len(), 1);
    assert_eq!(bound.subject, parameters[0].symbol);
    assert!(bound.carrier.is_valid());
    assert_eq!(program.symbols.name(bound.carrier), "Ranked");
    assert_eq!(program.child_type_references(bound.arguments).len(), 1);
}

#[test]
fn retains_callable_conformance_bound_declarations_with_owner_exposure() {
    let source = r#"
        trait Ranked {}
        data Card {}
        PowerOrder: Card satisfies Ranked;

        pub machine rank<Element, Evidence: Element satisfies Ranked>(value: &Element) {}

        machine rank_power<Element>(value: &Element)
        where Element satisfies Card::PowerOrder
        {}
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize conformance-bound custody");
    let syntax = parse_syntax_trees(&tokens).expect("parse conformance-bound custody");
    let program = lower_syntax_trees(&syntax).expect("resolve conformance-bound custody");
    let ranked = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Ranked")
        .expect("Ranked trait")
        .symbol;
    let card = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Card")
        .expect("Card data")
        .symbol;
    let power_order = program
        .conformances
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|alias| alias.as_str() == "PowerOrder")
        })
        .expect("PowerOrder conformance")
        .symbol;
    let selections = program.authored_declaration_selections();

    assert!(selections.iter().any(|selection| {
        selection.kind()
            == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::TypeReference
            && selection.exposure()
                == psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface
            && matches!(
                selection.target(),
                psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if target.selected_symbol() == ranked
            )
    }));
    assert!(selections.iter().any(|selection| {
        selection.kind() == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::TypeReference
            && selection.exposure()
                == psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation
            && matches!(
                selection.target(),
                psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if target.selected_symbol() == card
            )
    }));
    assert!(selections.iter().any(|selection| {
        selection.kind() == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::Conformance
            && selection.exposure()
                == psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation
            && matches!(
                selection.target(),
                psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if target.selected_symbol() == power_order
            )
    }));
}

#[test]
fn resolves_every_selected_conformance_bound_application_lane() {
    let source = r#"
        trait Encodes<Output> {}
        data Card {}
        data Message {}
        machine rank(value: &Card) -> u64 { 0 }

        FullEncoding<'scope, Element, Output, const Rank: u64, machine TieBreak>:
            Element satisfies Encodes<Output>
        where machine TieBreak(value: &Element) -> u64;
        {}

        machine inspect<'view, Element>(value: &'view Element)
        where Element satisfies Card::FullEncoding<'view, Card, Message, 7, rank>
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax).expect("resolve");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .expect("inspect machine");
    let [bound] = machine.conformance_bounds.as_slice() else {
        panic!("one selected conformance bound");
    };
    let selected = bound
        .selected_conformance
        .as_ref()
        .expect("selected conformance");
    assert!(matches!(
        program.symbols.get(selected.symbol).kind,
        psi_symbols::SymbolKind::Conformance
    ));
    let application = selected.application.as_ref().expect("complete application");
    assert_eq!(application.lifetime_arguments[0].as_str(), "view");
    assert!(matches!(
        program.symbols.get(application.arguments[0].symbol).kind,
        psi_symbols::SymbolKind::Data
    ));
    assert!(matches!(
        program.symbols.get(application.arguments[1].symbol).kind,
        psi_symbols::SymbolKind::Data
    ));
    assert!(application.arguments[2].const_literal.is_some());
    assert!(matches!(
        program.symbols.get(application.arguments[3].symbol).kind,
        psi_symbols::SymbolKind::State
    ));
    for expected in [
        application.arguments[0].symbol,
        application.arguments[1].symbol,
        application.arguments[3].symbol,
    ] {
        assert!(program.authored_declaration_selections().iter().any(|selection| {
            selection.exposure()
                == psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation
                && matches!(
                    selection.target(),
                    psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                        if target.selected_symbol() == expected
                )
        }));
    }
}

#[test]
fn lowers_closed_conformance_rows_to_exact_machine_states() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
            machine Self::rank_value(&self) -> u32;
        }
        data Card { }
        machine Card::stable_rank_value(&self) -> u32 { }

        PowerOrder: Card satisfies Ranked {
            machine before(&self, other: &Card) -> bool { }
            Ranked::rank_value = Card::stable_rank_value;
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program = lower_syntax_trees(&syntax_trees).expect("closed rows should normalize");
    let conformances = program.conformances.iter().collect::<Vec<_>>();
    let [conformance] = conformances.as_slice() else {
        panic!("one conformance");
    };
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| {
        row.declaring_trait.is_valid()
            && row.requirement.is_valid()
            && row.realization_machine.is_valid()
            && row.realization_state.is_valid()
    }));
    let before = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "before")
        .expect("inline row");
    assert_eq!(before.realization_name.as_str(), "Card::PowerOrder::before");
    let rank = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "rank_value")
        .expect("reference row");
    assert_eq!(rank.realization_name.as_str(), "Card::stable_rank_value");
}

#[test]
fn retains_named_conformance_visibility_and_snapshot_identity() {
    let source = r#"
        trait Shape {}
        data Circle {}
        pub PublicCircle: Circle satisfies Shape;
        PrivateCircle: Circle satisfies Shape;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax_trees).expect("resolve");
    let conformances = program.conformances.iter().collect::<Vec<_>>();

    assert_eq!(conformances.len(), 2);
    assert!(conformances[0].is_public);
    assert!(!conformances[1].is_public);
    let snapshot = program
        .snapshot_json()
        .expect("resolved conformance snapshot");
    assert!(snapshot.contains("\"name\":\"PublicCircle\""));
    assert!(snapshot.contains("\"is_public\":true"));
    assert!(snapshot.contains("\"trait_name\":\"Shape\""));
}

#[test]
fn lowers_subjectless_conformance_to_package_symbol_and_closed_rows() {
    let source = r#"
        trait Evidence {
            machine witness(value: i32);
        }

        ConcreteEvidence: satisfies Evidence {
            machine witness(value: i32) { }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("subjectless block should parse");
    let program = lower_syntax_trees(&syntax_trees).expect("subjectless rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    assert!(matches!(
        conformance.subject,
        psi_symbol_resolved_trees::trait_definition::ConformanceSubject::Subjectless
    ));
    assert!(conformance.symbol.is_valid());
    assert_eq!(program.symbols.name(conformance.symbol), "ConcreteEvidence");
    assert_eq!(
        program.symbols.get(conformance.symbol).parent,
        program.symbols.root()
    );
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    let [row] = rows.as_slice() else {
        panic!("one normalized evidence row");
    };
    assert!(row.requirement.is_valid());
    assert!(row.realization_machine.is_valid());
    assert!(row.realization_state.is_valid());
    assert_eq!(row.realization_name.as_str(), "ConcreteEvidence::witness");
    let realization = program
        .machines
        .iter()
        .find(|machine| machine.symbol == row.realization_machine)
        .expect("inline realization machine");
    assert!(realization.attached_data.is_none());
}

#[test]
fn subjectless_inline_calls_route_through_the_same_closed_map() {
    let source = r#"
        trait Evidence {
            machine first(value: i32);
            machine second(value: i32);
        }

        ConcreteEvidence: satisfies Evidence {
            machine first(value: i32) { second(value); }
            machine second(value: i32) { }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("subjectless block should parse");
    let program = lower_syntax_trees(&syntax_trees).expect("subjectless rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    let first = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "first")
        .expect("first row");
    let second = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "second")
        .expect("second row");
    let first_machine = program
        .machines
        .iter()
        .find(|machine| machine.symbol == first.realization_machine)
        .expect("first realization");
    let first_state = program.machine_state(
        *program
            .machine_state_handles(first_machine.states)
            .first()
            .expect("first realization state"),
    );
    let call = program
        .tables
        .bodies
        .statements
        .statements(first_state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            psi_symbol_resolved_trees::statement::StatementNode::Call(call) => Some(call),
            _ => None,
        })
        .expect("first calls second");
    assert_eq!(call.target_symbol, second.realization_state);
}

#[test]
fn closed_conformance_blocks_never_fall_back_to_ambient_attached_machines() {
    let source = r#"
        trait Ranked { machine Self::before(&self, other: &Self) -> bool; }
        data Card { }
        machine Card::before(&self, other: &Card) -> bool { }
        PowerOrder: Card satisfies Ranked { }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("closed map must ignore the ambient attached look-alike");
    assert!(
        diagnostic[0]
            .message
            .contains("is incomplete: missing `Ranked::before`")
    );
}

#[test]
fn closed_conformance_retains_trait_default_selection_rows() {
    let source = r#"
        trait Ranked { machine Self::fallback(&self) { } }
        data Card { }
        PowerOrder: Card satisfies Ranked { }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program = lower_syntax_trees(&syntax_trees)
        .expect("the selected trait-default template should cover the row");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].source,
        psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
    );
    assert!(rows[0].realization_machine.is_valid());
    assert!(rows[0].realization_state.is_valid());
    assert_eq!(
        rows[0].realization_name.as_str(),
        "Card::PowerOrder::Ranked::fallback"
    );
    let realization = program
        .machines
        .iter()
        .find(|machine| machine.symbol == rows[0].realization_machine)
        .expect("selected default realization");
    let [requirement] = program.machine_trait_conformances(realization.satisfies) else {
        panic!("default realization retains one exact requirement edge");
    };
    assert_eq!(requirement.symbol, rows[0].declaring_trait);
    assert_eq!(
        requirement.requirement.as_ref().map(|name| name.as_str()),
        Some("fallback")
    );
}

#[test]
fn inherited_trait_default_applications_partition_shared_authored_calls() {
    let source = r#"
        trait Resettable {
            machine set(&mut self, value: i32);
            machine reset(&mut self) { self.set(30); }
        }
        trait Counter { requires Resettable; }

        data Left { value: i32; }
        LeftCounter: Left satisfies Counter;
        machine Left::set(&mut self, value: i32) { self.value = value; }

        data Right { value: i32; }
        RightCounter: Right satisfies Counter;
        machine Right::set(&mut self, value: i32) { self.value = value; }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("source should parse");
    let program = lower_syntax_trees(&syntax_trees)
        .expect("each inherited default application should own its routed call");

    let applications = ["Left::reset", "Right::reset"].map(|name| {
        let machine = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("synthesized attached default");
        let partition = machine
            .compiler_selection_partition
            .expect("default application partition");
        let [requirement] = program.machine_trait_conformances(machine.satisfies) else {
            panic!("synthesized default retains one requirement edge");
        };
        assert_eq!(
            requirement.requirement.as_ref().map(|name| name.as_str()),
            Some("reset")
        );
        let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
        let call = program
            .state_statements(state.statements)
            .iter()
            .find_map(|statement| match statement {
                psi_symbol_resolved_trees::statement::Statement::Call(call) => Some(call),
                _ => None,
            })
            .expect("default body call");
        let occurrence = call
            .authored_call_selection
            .expect("routed call selection occurrence");
        let selection = *program
            .authored_declaration_selections()
            .get(occurrence)
            .expect("selection ledger row");
        assert_eq!(selection.compiler_partition(), Some(partition));
        (partition, selection)
    });

    assert_ne!(applications[0].0, applications[1].0);
    assert_eq!(
        applications[0].1.source_span(),
        applications[1].1.source_span()
    );
    assert_ne!(applications[0].1.target(), applications[1].1.target());
}

#[test]
fn closed_conformance_retains_every_same_named_default_overload() {
    let source = r#"
        trait Converter {
            machine Self::convert(&self, value: i32) -> i32 { value }
            machine Self::convert(&self, value: i32) -> i32 in Saturating { value }
        }
        data Item { }
        Primary: Item satisfies Converter { }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program = lower_syntax_trees(&syntax_trees)
        .expect("same-named default overloads retain exact declaration identities");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| {
        row.source
            == psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
            && row.requirement.is_valid()
            && row.realization_state.is_valid()
    }));
    assert_ne!(rows[0].requirement, rows[1].requirement);
    assert_ne!(rows[0].realization_state, rows[1].realization_state);
}

#[test]
fn closed_conformance_matches_inline_members_to_result_overloads() {
    let source = r#"
        trait Converter {
            machine Self::convert(&self, value: i32) -> i32 { value }
            machine Self::convert(&self, value: i32) -> i32 in Saturating { value }
        }
        data Item { }
        Primary: Item satisfies Converter {
            machine convert(&self, value: i32) -> i32 in Saturating { value }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program = lower_syntax_trees(&syntax_trees)
        .expect("the inline member's complete signature should select one overload");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter()
            .filter(|row| {
                row.source
                    == psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::Inline
            })
            .count(),
        1
    );
    assert_eq!(
        rows.iter()
            .filter(|row| {
                row.source
                    == psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
            })
            .count(),
        1
    );
    assert_eq!(
        program
            .machines
            .iter()
            .filter(|machine| {
                machine
                    .name
                    .as_str()
                    .contains("Primary::Converter::convert")
            })
            .count(),
        1,
        "the overridden Saturating default candidate must not remain executable"
    );
}

#[test]
fn trait_default_calls_route_through_the_same_closed_map() {
    let source = r#"
        trait Pair {
            machine Self::first(&self) { self.second(); }
            machine Self::second(&self);
        }
        data Card { }
        machine Card::second(&self) { }
        Selected: Card satisfies Pair {
            machine second(&self) { }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program = lower_syntax_trees(&syntax_trees).expect("closed rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    let first = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "first")
        .expect("default first row");
    let second = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "second")
        .expect("inline second row");
    assert_eq!(
        first.source,
        psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
    );
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.symbol == first.realization_machine)
        .expect("instantiated default machine");
    let state = program
        .machine_state_handles(machine.states)
        .first()
        .map(|handle| program.machine_state(*handle))
        .expect("instantiated default state");
    let [psi_symbol_resolved_trees::statement::StatementNode::Call(call)] = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
    else {
        panic!("one default-body call");
    };
    assert_eq!(call.target_symbol, second.realization_state);
}

#[test]
fn trait_default_synthesis_is_idempotent_across_orchestration_and_lowering() {
    let source = r#"
        trait Ranked { machine Self::fallback(&self) { } }
        data Card { }
        Selected: Card satisfies Ranked { }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let mut syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    crate::synthesize_trait_defaults(&mut syntax_trees)
        .expect("orchestration may synthesize before resolution");
    let program = lower_syntax_trees(&syntax_trees)
        .expect("resolution's mandatory synthesis pass must not duplicate the row");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(
        program
            .machines
            .iter()
            .filter(|machine| machine.name.as_str() == "Card::Selected::Ranked::fallback")
            .count(),
        1
    );
}

#[test]
fn inherited_same_name_defaults_keep_distinct_exact_rows() {
    let source = r#"
        trait Left { machine Self::fallback(&self) { } }
        trait Right { machine Self::fallback(&self) { } }
        trait Both: Left + Right { }
        data Card { }
        Selected: Card satisfies Both { }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program = lower_syntax_trees(&syntax_trees).expect("exact defaults should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| {
        row.source
            == psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
            && row.realization_machine.is_valid()
            && row.realization_state.is_valid()
    }));
    assert_ne!(rows[0].declaring_trait, rows[1].declaring_trait);
    assert_ne!(rows[0].realization_name, rows[1].realization_name);
}

#[test]
fn inherited_requirement_collisions_require_trait_qualified_rows() {
    let ambiguous = r#"
        trait LeftOrder { machine Self::before(&self, other: &Self); }
        trait RightOrder { machine Self::before(&self, other: &Self); }
        trait BothOrders: LeftOrder + RightOrder { }
        data Card { }
        Selected: Card satisfies BothOrders {
            machine before(&self, other: &Card) { }
        }
    "#;
    let tokens = Lexer::new(ambiguous)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("a short row name must not choose one inherited declaration");
    assert!(
        diagnostic[0]
            .message
            .contains("is ambiguous across inherited traits")
    );

    let qualified = r#"
        trait LeftOrder { machine Self::before(&self, other: &Self); }
        trait RightOrder { machine Self::before(&self, other: &Self); }
        trait BothOrders: LeftOrder + RightOrder { }
        data Card { }
        machine Card::left_before(&self, other: &Card) { }
        machine Card::right_before(&self, other: &Card) { }
        Selected: Card satisfies BothOrders {
            LeftOrder::before = Card::left_before;
            RightOrder::before = Card::right_before;
        }
    "#;
    let tokens = Lexer::new(qualified)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("qualified rows should parse");
    let program = lower_syntax_trees(&syntax_trees).expect("qualified rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].declaring_trait, rows[1].declaring_trait);
}

#[test]
fn inline_conformance_member_calls_route_through_the_same_closed_map() {
    let source = r#"
        trait Pair {
            machine Self::first(&self, other: &Self);
            machine Self::second(&self);
        }
        data Card { }
        machine Card::second(&self) { }
        Selected: Card satisfies Pair {
            machine first(&self, other: &Card) { other.second(); }
            machine second(&self) { }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program = lower_syntax_trees(&syntax_trees).expect("closed rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    let first = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "first")
        .expect("first row");
    let second = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "second")
        .expect("second row");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.symbol == first.realization_machine)
        .expect("inline first machine");
    let state_handles = program.machine_state_handles(machine.states);
    let state = state_handles
        .first()
        .map(|handle| program.machine_state(*handle))
        .expect("inline first state");
    let [psi_symbol_resolved_trees::statement::StatementNode::Call(call)] = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
    else {
        panic!("one call statement");
    };
    assert_eq!(
        call.target_symbol,
        second.realization_state,
        "the ambient Card::second look-alike must not supply the closed row; receiver={:?}, starts_at_self={}, target={}",
        program
            .tables
            .bodies
            .statements
            .name_path_members(call.receiver),
        call.receiver_starts_at_self,
        call.target
    );
}

#[test]
fn inline_conformance_value_calls_route_through_the_same_closed_map() {
    let source = r#"
        trait Pair {
            machine Self::first(&self) -> i32;
            machine Self::second(&self) -> i32;
        }
        data Card { }
        machine Card::second(&self) -> i32 { transition { _ -> (1) } }
        Selected: Card satisfies Pair {
            machine first(&self) -> i32 {
                transition { _ -> (self.second()) }
            }
            machine second(&self) -> i32 { transition { _ -> (2) } }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program = lower_syntax_trees(&syntax_trees).expect("closed rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    let first = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "first")
        .expect("first row");
    let second = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "second")
        .expect("second row");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.symbol == first.realization_machine)
        .expect("inline first machine");
    let state = program
        .machine_state_handles(machine.states)
        .first()
        .map(|handle| program.machine_state(*handle))
        .expect("inline first state");
    let Some(psi_symbol_resolved_trees::statement::StatementNode::LocalData(local)) = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .first()
    else {
        panic!("value-call normalization should retain its hoisted initializer");
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) = program
        .tables
        .bodies
        .expressions
        .expression(local.initial_value)
    else {
        panic!("hoisted value call");
    };
    assert_eq!(call.target_symbol, second.realization_state);
}

#[test]
fn inline_conformance_calls_preserve_a_foreign_receiver_method() {
    let source = r#"
        data Other { }
        machine Other::second(&self) { }
        trait Pair {
            machine Self::first(&self, other: &Other);
            machine Self::second(&self);
        }
        data Card { }
        Selected: Card satisfies Pair {
            machine first(&self, other: &Other) { other.second(); }
            machine second(&self) { }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program = lower_syntax_trees(&syntax_trees).expect("closed rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    let first = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "first")
        .expect("first row");
    let second = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "second")
        .expect("second row");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.symbol == first.realization_machine)
        .expect("inline first machine");
    let state = program
        .machine_state_handles(machine.states)
        .first()
        .map(|handle| program.machine_state(*handle))
        .expect("inline first state");
    let [psi_symbol_resolved_trees::statement::StatementNode::Call(call)] = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
    else {
        panic!("one call statement");
    };
    assert!(call.target_symbol.is_valid());
    assert_ne!(call.target_symbol, second.realization_state);
    assert_eq!(program.symbols.name(call.target_symbol), "second");
}

#[test]
fn proposition_parameter_signatures_receive_distinct_symbols() {
    let source = r#"
        trait Reflexive<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
            machine prove(value: C) ensures Relation(value, value);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");

    let trait_definition = &program.traits[0];
    let [carrier, relation] = program.trait_type_parameters(trait_definition) else {
        panic!("trait should retain its carrier and proposition parameters");
    };
    assert_eq!(
        program.symbols.get(relation.symbol).kind,
        psi_symbols::SymbolKind::PropositionParameter
    );
    let psi_symbol_resolved_trees::data::TypeParameterKind::Proposition { contract } =
        &relation.kind
    else {
        panic!("Relation should retain a proposition signature");
    };
    let [left, right] = program.state_parameters(contract.parameters) else {
        panic!("Relation should retain two value parameters");
    };
    assert!(left.symbol.is_valid() && right.symbol.is_valid());
    for parameter in [left, right] {
        let psi_symbol_resolved_trees::types::TypeReference::Named { symbol, .. } =
            &parameter.type_reference
        else {
            panic!("relation parameter should retain C");
        };
        assert_eq!(*symbol, carrier.symbol);
    }

    let [signature] = program.trait_machine_signatures(trait_definition.machines) else {
        panic!("trait should retain one proof signature");
    };
    let [contract] = program.signature_contracts(signature.contracts) else {
        panic!("proof signature should retain one ensures contract");
    };
    let [psi_symbol_resolved_trees::domain::ProofFact::Expression(expression)] =
        program.proof_facts(contract.facts)
    else {
        panic!("resolved proof fact should remain an expression");
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("ensures should remain a proposition-family call");
    };
    assert_eq!(call.target_symbol, relation.symbol);
}

#[test]
fn proposition_declarations_resolve_as_a_distinct_proof_category() {
    let source = r#"
        pub proposition related(left: i32, right: i32);
        proposition witnessed<machine Generator>(value: i32) evidence i32;
        proposition reflexive(value: i32) = related(value, value);
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");

    assert_eq!(program.propositions.len(), 3);
    assert!(program.propositions[0].is_public);
    assert!(!program.propositions[1].is_public);
    assert!(
        program
            .snapshot_json()
            .expect("resolved proposition snapshot")
            .contains("\"is_public\":true")
    );
    assert_eq!(program.machines.len(), 0);
    assert!(
        program
            .propositions
            .iter()
            .all(|item| item.symbol.is_valid())
    );
    assert!(
        program
            .propositions
            .iter()
            .all(|item| program.symbols.get(item.symbol).kind
                == psi_symbols::SymbolKind::Proposition)
    );

    let witnessed = &program.propositions[1];
    let [generator] = program
        .tables
        .declarations
        .proposition_binders
        .span_or_empty(witnessed.binders)
    else {
        panic!("witnessed proposition should retain one binder");
    };
    assert!(matches!(
        generator.kind,
        psi_symbol_resolved_trees::proposition::PropositionBinderKind::Machine
    ));
    assert_eq!(
        program.symbols.get(generator.symbol).kind,
        psi_symbols::SymbolKind::PropositionMachineParameter
    );
    let psi_symbol_resolved_trees::proposition::PropositionBody::Witness { evidence } =
        &witnessed.body
    else {
        panic!("witness evidence should remain distinct from a body");
    };
    assert!(matches!(
        evidence,
        psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name }
            if symbol.is_valid() && name.as_str() == "i32"
    ));

    let psi_symbol_resolved_trees::proposition::PropositionBody::Transparent { proposition } =
        program.propositions[2].body
    else {
        panic!("transparent proposition should retain its source expansion");
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(proposition)
    else {
        panic!("transparent expansion should remain a proposition call");
    };
    assert_eq!(call.target_symbol, program.propositions[0].symbol);
    for argument in program
        .tables
        .bodies
        .expressions
        .expression_handles(call.arguments)
    {
        let psi_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
            program.tables.bodies.expressions.expression(*argument)
        else {
            panic!("alias arguments should remain parameter names");
        };
        assert!(path.symbol.is_valid());
        assert_eq!(
            program.symbols.get(path.symbol).kind,
            psi_symbols::SymbolKind::Parameter
        );
    }
}

#[test]
fn transparent_proposition_zero_value_target_resolves_in_its_binder_scope() {
    let source = r#"
        data Optional<Element> { case #0 None; }
        proposition zero_reflexive<Item>() =
            zero_value<Optional<Item>>() == zero_value<Optional<Item>>();
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax).expect("resolve");
    let optional = program.data_definitions.first().expect("Optional data");
    let proposition = program.propositions.first().expect("zero proposition");
    let [binder] = program
        .tables
        .declarations
        .proposition_binders
        .span_or_empty(proposition.binders)
    else {
        panic!("one proposition type binder")
    };

    let targets = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            psi_symbol_resolved_trees::expression::ExpressionNode::ZeroValue(target) => {
                Some(*target)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 2);
    for target in targets {
        let psi_symbol_resolved_trees::types::TypeReference::Generic(target) =
            program.child_type_reference(target)
        else {
            panic!("zero-value target should remain a generic type")
        };
        assert_eq!(target.base_symbol, optional.symbol);
        let [argument] = program.child_type_references(target.arguments) else {
            panic!("one exact zero-value type argument")
        };
        assert!(matches!(
            argument,
            psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name }
                if *symbol == binder.symbol && name.as_str() == "Item"
        ));
    }
}

#[test]
fn retains_exact_expression_selection_symbols() {
    let source = r#"
        data Token {
            value: u32;
            case Issued(code: u32);
        }
        machine path() -> u32 { Token::Issued::code }
        machine record() -> Token { Token { value: 1 } }
        machine issue() -> Token { Token::Issued { value: 1, code: 2 } }
        machine is_issued(token: Token) -> bool { token in Token::Issued }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize exact selections");
    let syntax = parse_syntax_trees(&tokens).expect("parse exact selections");
    let program = lower_syntax_trees(&syntax).expect("resolve exact selections");
    let expressions = &program.tables.bodies.expressions;

    let path = expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            psi_symbol_resolved_trees::expression::ExpressionNode::Name(path)
                if expressions.name_path_members(path.members).len() == 3 =>
            {
                Some(path)
            }
            _ => None,
        })
        .expect("three-segment case payload path");
    let path_symbols = expressions.name_path_member_symbols(path.member_symbols);
    assert_eq!(path_symbols.len(), 3);
    assert!(path_symbols.iter().all(|symbol| symbol.is_valid()));
    assert_eq!(
        program.symbols.get(path_symbols[0]).kind,
        psi_symbols::SymbolKind::Data
    );
    assert_eq!(
        program.symbols.get(path_symbols[1]).kind,
        psi_symbols::SymbolKind::Variant
    );
    assert_eq!(
        program.symbols.get(path_symbols[2]).kind,
        psi_symbols::SymbolKind::Field
    );

    let literals = expressions
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            psi_symbol_resolved_trees::expression::ExpressionNode::StructLiteral(literal) => {
                Some(literal)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(literals.len(), 2);
    for literal in literals {
        assert!(literal.type_symbol.is_valid());
        assert_eq!(
            literal.case_name.is_some(),
            literal.case_symbol.is_some_and(|symbol| symbol.is_valid())
        );
        assert!(
            expressions
                .struct_fields(literal.fields)
                .iter()
                .all(|field| field.field_symbol.is_valid())
        );
    }

    let membership = expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            psi_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
                Some(membership)
            }
            _ => None,
        })
        .expect("case membership");
    assert!(!membership.domain_symbol.is_valid());
    assert!(membership.case_type_symbol.is_valid());
    assert!(membership.case_symbol.is_valid());
    assert_eq!(
        program.symbols.get(membership.case_type_symbol).kind,
        psi_symbols::SymbolKind::Data
    );
    assert_eq!(
        program.symbols.get(membership.case_symbol).kind,
        psi_symbols::SymbolKind::Variant
    );

    let selections = program.authored_declaration_selections();
    assert!(!selections.is_empty());
    assert!(selections.iter().all(|selection| {
        selection.exposure()
            == psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation
    }));
    for required_kind in [
        psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::StaticPathSegment,
        psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::StructLiteralType,
        psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::StructLiteralCase,
        psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::StructLiteralField,
        psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::CaseReference,
        psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::CaseMembership,
    ] {
        assert!(
            selections
                .iter()
                .any(|selection| selection.kind() == required_kind),
            "missing authored selection kind {required_kind:?}"
        );
    }
    assert!(expressions.iter_expressions().any(|(expression, _)| {
        expressions
            .authored_selection_occurrences(expression)
            .next()
            .is_some()
    }));
}

#[test]
fn outcome_specific_ensures_normalizes_only_against_declared_result_sum() {
    let source = r#"
        data Outcome { case Success; case Failure; }
        machine choose() -> Outcome
        ensures Outcome::Success -> { true; }
        { Outcome::Success }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize guarded guarantee");
    let syntax = parse_syntax_trees(&tokens).expect("parse guarded guarantee");
    let program = lower_syntax_trees(&syntax).expect("resolve guarded guarantee");
    let outcome = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "Outcome")
        .expect("Outcome data");
    let success = program
        .data_members(outcome.members)
        .iter()
        .find_map(|member| match member {
            psi_symbol_resolved_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == "Success" =>
            {
                Some(variant)
            }
            _ => None,
        })
        .expect("Success case");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "choose")
        .expect("choose machine");
    let [contract] = program.machine_contracts(machine) else {
        panic!("one guarded guarantee row")
    };
    assert_eq!(
        contract.kind,
        psi_symbol_resolved_trees::signature::SignatureContractKind::EnsuresForResultCase {
            result_data: outcome.symbol,
            result_case: success.symbol,
        }
    );
}

#[test]
fn authored_outcome_specific_contract_uses_the_exact_base_result_sum() {
    let base = r#"
        data Outcome { case Success; }
        machine choose() -> Outcome
        ensures Outcome::Success -> { true; }
        { Outcome::Success }
    "#;
    let extension = "data Outcome { case Success; case GeneratedOnly; }";
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

    let program = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("authored outcome contract must retain the base result sum");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "choose")
        .expect("choose machine");
    let [contract] = program.machine_contracts(machine) else {
        panic!("one outcome-specific contract")
    };
    let psi_symbol_resolved_trees::signature::SignatureContractKind::EnsuresForResultCase {
        result_data,
        result_case,
    } = contract.kind
    else {
        panic!("outcome-specific contract kind")
    };
    for symbol in [result_data, result_case] {
        assert_eq!(
            program
                .symbols
                .symbol_provenance_source_span(symbol)
                .expect("source-backed result declaration")
                .source_id,
            base_id
        );
    }
}

#[test]
fn outcome_specific_ensures_rejects_non_sum_and_foreign_cases() {
    for (source, expected) in [
        (
            "data Record {} machine choose() -> Record ensures Record::Success -> { true; } { Record {} }",
            "requires a sum result",
        ),
        (
            "data Outcome { case Success; } machine choose() -> Outcome ensures Outcome::Missing -> { true; } { Outcome::Success }",
            "unknown case `Missing`",
        ),
        (
            "data Outcome { case Success; } data Other { case Success; } machine choose() -> Outcome ensures Other::Success -> { true; } { Outcome::Success }",
            "does not belong to declared result sum `Outcome`",
        ),
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize invalid guarded guarantee");
        let syntax = parse_syntax_trees(&tokens).expect("parse invalid guarded guarantee");
        let diagnostics = lower_syntax_trees(&syntax).expect_err("guard resolution must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "expected {expected:?}, got: {diagnostics:?}"
        );
    }
}

#[test]
fn captures_resolved_calls_and_late_checked_operators_in_private_bodies() {
    let source = r#"
        machine identity(value: u32) -> u32 { value }
        machine calculate(value: u32) -> u32 { identity(value) + 1 }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize authored selections");
    let syntax = parse_syntax_trees(&tokens).expect("parse authored selections");
    let program = lower_syntax_trees(&syntax).expect("resolve authored selections");
    let selections = program.authored_declaration_selections();
    assert!(selections.iter().any(|selection| {
        selection.kind() == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::Call
            && matches!(
                selection.target(),
                psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(_)
            )
    }));
    assert!(selections.iter().any(|selection| {
        selection.kind()
            == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::Operator
            && selection.target()
                == psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::LateBound(
                    psi_symbol_resolved_trees::AuthoredDeclarationSelectionLateBinding::CheckedOperator,
                )
    }));
}

#[test]
fn guard_hoist_copies_share_one_authored_call_occurrence() {
    let source = r#"
        data Main { }
        machine Main::value(&mut self) -> bool { true }
        machine Main::done(&mut self) { }
        machine Main::run(&mut self) {
            let seed: bool = true;
            transition self.value() == seed {
                true -> done()
                false -> done()
            }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize guard hoist");
    let syntax = parse_syntax_trees(&tokens).expect("parse guard hoist");
    let program = lower_syntax_trees(&syntax).expect("resolve guard hoist");
    let target_start = source
        .find("self.value() == seed")
        .expect("guard call source")
        + "self.".len();
    let selections = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind() == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::Call
                && selection.source_span().span.start == target_start
        })
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        panic!("one authored occurrence must own every compiler copy: {selections:?}");
    };
    assert!(matches!(
        selection.target(),
        psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(_)
    ));
    assert!(
        program
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .any(|(expression, _)| program
                .tables
                .bodies
                .expressions
                .authored_selection_occurrences(expression)
                .any(|occurrence| occurrence == selection.occurrence_id()))
    );
}

#[test]
fn const_specialization_copies_share_the_authored_member_declaration() {
    let source = r#"
        data FixedBuffer<const N: u64> { items: [i32; N]; }
        machine FixedBuffer::first(&self) -> i32 { self.items[0] }
        data Main { small: FixedBuffer<2>; large: FixedBuffer<4>; }
        machine Main::small(&self) -> i32 { self.small.first() }
        machine Main::large(&self) -> i32 { self.large.first() }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize const copies");
    let syntax = parse_syntax_trees(&tokens).expect("parse const copies");
    let program = lower_syntax_trees(&syntax).expect("resolve const copies");
    let items_start = source.find("self.items").expect("member source") + "self.".len();
    let selections = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind()
                == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::MemberAccess
                && selection.source_span().span.start == items_start
        })
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        panic!("one source member must own every specialized copy: {selections:?}");
    };
    let psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target) =
        selection.target()
    else {
        panic!("specialized member selection must resolve exactly")
    };
    assert_eq!(
        program.symbols.display_path(target.selected_symbol(), "::"),
        "FixedBuffer::first::items"
    );
}

#[test]
fn distinguishes_public_contract_expressions_from_public_machine_bodies() {
    let source = r#"
        machine helper() -> bool { true }
        pub machine api() -> bool
        requires helper()
        {
            helper()
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize expression exposure");
    let syntax = parse_syntax_trees(&tokens).expect("parse expression exposure");
    let program = lower_syntax_trees(&syntax).expect("resolve expression exposure");
    let call_exposures = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind() == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::Call
        })
        .map(|selection| selection.exposure())
        .collect::<Vec<_>>();

    assert!(call_exposures.contains(
        &psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface
    ));
    assert!(call_exposures.contains(
        &psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation
    ));
}

#[test]
fn retains_authored_expression_exposure_for_embedded_type_lowering() {
    let source = r#"
        data Marker {}
        pub proposition public_zero() =
            zero_value<Marker>() == zero_value<Marker>();
        proposition private_zero() =
            zero_value<Marker>() == zero_value<Marker>();
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize expression type exposure");
    let syntax = parse_syntax_trees(&tokens).expect("parse expression type exposure");
    let program = lower_syntax_trees(&syntax).expect("resolve expression type exposure");
    let mut exposures = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter(|&(_, node)| {
            matches!(
                node,
                psi_symbol_resolved_trees::expression::ExpressionNode::ZeroValue(_)
            )
        })
        .map(|(expression, _)| {
            program
                .tables
                .bodies
                .expressions
                .authored_expression_exposure(expression)
                .expect("authored zero-value expression exposure")
        })
        .collect::<Vec<_>>();
    exposures.sort_by_key(|exposure| match exposure {
        psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation => 0,
        psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface => 1,
    });
    assert_eq!(
        exposures,
        [
            psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface,
            psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface,
        ]
    );
}

#[test]
fn qualification_cast_domains_retain_exact_expression_custody() {
    let source = r#"
        domain u16::Tagged;
        pub machine api(value: u8)
        requires (value as u16 in Tagged) == 1
        { }
        machine helper(value: u8)
        requires (value as u16 in Tagged) == 1
        { }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize qualification casts");
    let syntax = parse_syntax_trees(&tokens).expect("parse qualification casts");
    let program = lower_syntax_trees(&syntax).expect("resolve qualification casts");

    let casts = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(expression, node)| match node {
            psi_symbol_resolved_trees::expression::ExpressionNode::Cast(cast)
                if !cast.semantic_domain.is_empty() =>
            {
                Some(expression)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(casts.len(), 2);

    let mut exposures = casts
        .into_iter()
        .map(|cast| {
            let occurrences = program
                .tables
                .bodies
                .expressions
                .authored_selection_occurrences(cast)
                .collect::<Vec<_>>();
            let [occurrence] = occurrences.as_slice() else {
                panic!("each qualification cast must retain one exact authored selection")
            };
            let selection = program
                .authored_declaration_selections()
                .get(*occurrence)
                .expect("qualification-cast occurrence must rejoin its selection");
            assert_eq!(
                selection.kind(),
                psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::DomainMembership
            );
            assert_eq!(
                selection.target(),
                psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::LateBound(
                    psi_symbol_resolved_trees::AuthoredDeclarationSelectionLateBinding::CheckedDomainMembership,
                )
            );
            let source_span = selection.source_span();
            assert!(source_span.span.start < source_span.span.end);
            assert_eq!(source_span.span.end - source_span.span.start, "Tagged".len());
            selection.exposure()
        })
        .collect::<Vec<_>>();
    exposures.sort_by_key(|exposure| match exposure {
        psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation => 0,
        psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface => 1,
    });
    assert_eq!(
        exposures,
        [
            psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface,
        ]
    );
}

#[test]
fn retains_nested_unary_operator_custody_in_public_propositions() {
    let source = "pub proposition inverted(value: u8, expected: u8) = ~value == expected;";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize public unary proposition");
    let syntax = parse_syntax_trees(&tokens).expect("parse public unary proposition");
    let program = lower_syntax_trees(&syntax).expect("resolve public unary proposition");
    let proposition = program
        .propositions
        .iter()
        .find(|proposition| proposition.name.as_str() == "inverted")
        .expect("inverted proposition");
    let psi_symbol_resolved_trees::proposition::PropositionBody::Transparent { proposition } =
        proposition.body
    else {
        panic!("inverted proposition must remain transparent")
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Binary(binary) =
        program.tables.bodies.expressions.expression(proposition)
    else {
        panic!("inverted proposition must retain its binary root")
    };
    let unary = binary.left;
    assert!(matches!(
        program.tables.bodies.expressions.expression(unary),
        psi_symbol_resolved_trees::expression::ExpressionNode::Unary(_)
    ));
    let occurrences = program
        .tables
        .bodies
        .expressions
        .authored_selection_occurrences(unary)
        .collect::<Vec<_>>();
    let [occurrence] = occurrences.as_slice() else {
        panic!("nested unary operator must retain one exact authored selection")
    };
    let selection = program
        .authored_declaration_selections()
        .get(*occurrence)
        .expect("nested unary occurrence must rejoin its selection");
    assert_eq!(
        selection.kind(),
        psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::Operator
    );
    assert_eq!(
        selection.exposure(),
        psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface
    );
}

#[test]
fn retains_exact_establishment_route_declarations_with_domain_exposure() {
    let source = r#"
        data Ticket { }
        pub trait Issues {
            machine issue() -> Ticket in Ticket::Issued;
            machine hide() -> Ticket in Ticket::Internal;
        }
        pub domain Ticket::Issued
        established by Issues::issue, Issues::issue;
        domain Ticket::Internal
        established by Issues::hide;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize establishment selections");
    let syntax = parse_syntax_trees(&tokens).expect("parse establishment selections");
    let program = lower_syntax_trees(&syntax).expect("resolve establishment selections");
    let issues = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Issues")
        .expect("Issues trait");
    let requirements = program.trait_machine_signatures(issues.machines);
    let issue = requirements
        .iter()
        .find(|requirement| requirement.name.as_str() == "issue")
        .expect("issue requirement");
    let hide = requirements
        .iter()
        .find(|requirement| requirement.name.as_str() == "hide")
        .expect("hide requirement");
    let rows = program
        .authored_declaration_selections()
        .iter()
        .filter_map(|selection| match selection.target() {
            psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                if [issues.symbol, issue.symbol, hide.symbol]
                    .contains(&target.selected_symbol()) =>
            {
                Some((
                    selection.kind(),
                    selection.exposure(),
                    target.selected_symbol(),
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    let issued = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Ticket::Issued")
        .expect("Issued domain");
    assert_eq!(
        issued.establishment_routes.len(),
        1,
        "semantic alternatives should deduplicate"
    );
    assert_eq!(rows.len(), 6, "rows={rows:#?}");
    assert_eq!(
        rows.iter()
            .filter(|(kind, exposure, symbol)| {
                *kind
                    == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::TypeReference
                    && *exposure
                        == psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface
                    && *symbol == issues.symbol
            })
            .count(),
        2
    );
    assert_eq!(
        rows.iter()
            .filter(|(kind, exposure, symbol)| {
                *kind
                    == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::StaticPathSegment
                    && *exposure
                        == psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface
                    && *symbol == issue.symbol
            })
            .count(),
        2
    );
    assert_eq!(
        rows.iter()
            .filter(|(_, exposure, _)| {
                *exposure
                    == psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation
            })
            .count(),
        2
    );
}

#[test]
fn distinguishes_boundary_contract_expressions_from_boundary_adapter_bodies() {
    let source = r#"
        machine helper() -> bool { true }
        boundary machine api() -> bool
        requires helper()
        {
            helper()
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize boundary expression exposure");
    let syntax = parse_syntax_trees(&tokens).expect("parse boundary expression exposure");
    let program = lower_syntax_trees(&syntax).expect("resolve boundary expression exposure");
    let call_exposures = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind() == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::Call
        })
        .map(|selection| selection.exposure())
        .collect::<Vec<_>>();

    assert!(call_exposures.contains(
        &psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface
    ));
    assert!(call_exposures.contains(
        &psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation
    ));
}

#[test]
fn captures_expression_static_type_and_machine_arguments() {
    let source = r#"
        data Card { }
        machine chosen(value: &Card) -> bool { true }
        machine apply<T, machine Selected>(value: &T) -> bool
        where machine Selected(value: &T) -> bool
        {
            Selected(value)
        }
        machine caller(value: &Card) -> bool {
            apply<Card, chosen>(value)
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize static arguments");
    let syntax = parse_syntax_trees(&tokens).expect("parse static arguments");
    let program = lower_syntax_trees(&syntax).expect("resolve static arguments");
    let selected_kinds = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind()
                == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::StaticArgument
        })
        .filter_map(|selection| match selection.target() {
            psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target) => {
                Some(program.symbols.get(target.selected_symbol()).kind)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(selected_kinds.contains(&psi_symbols::SymbolKind::Data));
    assert!(selected_kinds.contains(&psi_symbols::SymbolKind::State));
}

#[test]
fn static_and_proof_static_arguments_obey_current_activation_resolution_strata() {
    let base = r#"
        trait Marker {}
        data Box<Element> {}
        data Item {}
        const Limits::VALUE: u64 = 1;
        Marked: Item satisfies Marker;
        machine Item::work() {}

        data BaseBox<Element> {}
        data BaseItem {}
        const BaseLimits::VALUE: u64 = 2;
        BaseMarked: BaseItem satisfies Marker;
        machine BaseItem::work() {}

        machine sink<A, B, C, D>() {}
        proposition prove<A, B, C, D>();
        machine base_call() {
            sink<Box<Item>, Limits::VALUE, Marked, Item::work>();
        }
        machine base_hidden_call() {
            sink<ExtensionBox<ExtensionItem>, ExtensionLimits::VALUE, ExtensionMarked, ExtensionItem::work>();
        }
        proposition base_proof() =
            prove<Box<Item>, Limits::VALUE, Marked, Item::work>();
        proposition base_hidden_proof() =
            prove<ExtensionBox<ExtensionItem>, ExtensionLimits::VALUE, ExtensionMarked, ExtensionItem::work>();
    "#;
    let extension = r#"
        data Box<Element> {}
        data Item {}
        const Limits::VALUE: u64 = 3;
        Marked: Item satisfies Marker;
        machine Item::work() {}

        data ExtensionBox<Element> {}
        data ExtensionItem {}
        const ExtensionLimits::VALUE: u64 = 4;
        ExtensionMarked: ExtensionItem satisfies Marker;
        machine ExtensionItem::work() {}

        machine extension_call() {
            sink<Box<Item>, Limits::VALUE, Marked, Item::work>();
        }
        machine extension_reads_base() {
            sink<BaseBox<BaseItem>, BaseLimits::VALUE, BaseMarked, BaseItem::work>();
        }
        proposition extension_proof() =
            prove<Box<Item>, Limits::VALUE, Marked, Item::work>();
    "#;
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("main.omg"), base.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/extension.omg"),
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
    let program = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("resolve extension-first static arguments");

    fn statement_arguments<'a>(
        program: &'a psi_symbol_resolved_trees::SymbolResolvedTrees,
        machine_name: &str,
    ) -> &'a [psi_symbol_resolved_trees::expression::StaticMachineArgument] {
        let machine = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .expect("caller machine");
        let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
        program
            .tables
            .bodies
            .statements
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                psi_symbol_resolved_trees::statement::StatementNode::Call(call)
                    if call.target.as_str() == "sink" =>
                {
                    Some(call.machine_arguments.as_ref())
                }
                _ => None,
            })
            .expect("sink call")
    }

    fn proof_arguments<'a>(
        program: &'a psi_symbol_resolved_trees::SymbolResolvedTrees,
        proposition_name: &str,
    ) -> &'a [psi_symbol_resolved_trees::expression::StaticMachineArgument] {
        let proposition = program
            .propositions
            .iter()
            .find(|proposition| proposition.name.as_str() == proposition_name)
            .expect("transparent proposition");
        let psi_symbol_resolved_trees::proposition::PropositionBody::Transparent { proposition } =
            proposition.body
        else {
            panic!("transparent proposition body")
        };
        let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
            program.tables.bodies.expressions.expression(proposition)
        else {
            panic!("proposition call")
        };
        &call.machine_arguments
    }

    fn assert_argument_sources(
        program: &psi_symbol_resolved_trees::SymbolResolvedTrees,
        arguments: &[psi_symbol_resolved_trees::expression::StaticMachineArgument],
        expected: psi_source::SourceId,
    ) {
        assert_eq!(arguments.len(), 4);
        let nested = &arguments[0]
            .application
            .as_ref()
            .expect("nested data application")
            .arguments[0];
        for argument in arguments.iter().chain(std::iter::once(nested)) {
            assert!(
                argument.symbol.is_valid(),
                "argument={argument:#?}, spans={:?}",
                argument
                    .path
                    .iter()
                    .map(|name| name.source_span())
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                program
                    .symbols
                    .symbol_provenance_source_span(argument.symbol)
                    .expect("selected static declaration provenance")
                    .source_id,
                expected,
                "argument={argument:#?}",
            );
        }
    }

    assert_argument_sources(
        &program,
        statement_arguments(&program, "base_call"),
        base_id,
    );
    assert_argument_sources(&program, proof_arguments(&program, "base_proof"), base_id);
    assert_argument_sources(
        &program,
        statement_arguments(&program, "extension_call"),
        extension_id,
    );
    assert_argument_sources(
        &program,
        proof_arguments(&program, "extension_proof"),
        extension_id,
    );
    assert_argument_sources(
        &program,
        statement_arguments(&program, "extension_reads_base"),
        base_id,
    );

    for arguments in [
        statement_arguments(&program, "base_hidden_call"),
        proof_arguments(&program, "base_hidden_proof"),
    ] {
        assert!(arguments.iter().all(|argument| !argument.symbol.is_valid()));
        let nested = &arguments[0]
            .application
            .as_ref()
            .expect("nested hidden data application")
            .arguments[0];
        assert!(!nested.symbol.is_valid());
    }
}

#[test]
fn resolves_named_const_static_arguments_with_exact_authored_custody() {
    let source = r#"
        pub const Limits::LIMIT: u64 = 7;
        pub machine constant<const Value: u64>() -> u64 { 0 }
        boundary machine trusted_constant() -> u64
        ensures result == constant<Limits::LIMIT>();
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize named const static argument");
    let syntax = parse_syntax_trees(&tokens).expect("parse named const static argument");
    let program = lower_syntax_trees(&syntax).expect("resolve named const static argument");
    let selections = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind()
                == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::StaticArgument
                && matches!(
                    selection.target(),
                    psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                        if program.symbols.get(target.selected_symbol()).kind
                            == psi_symbols::SymbolKind::Const
                )
        })
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        panic!("one exact named const static-argument selection: {selections:#?}")
    };
    assert_eq!(
        selection.exposure(),
        psi_symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface
    );
}

#[test]
fn captures_statement_calls_and_their_explicit_conformance_arguments() {
    let source = r#"
        trait Marker { }
        data Good { }
        GoodMarker: Good satisfies Marker;

        machine effect() { }
        machine consume<Element, Evidence: Element satisfies Marker>(value: Element) { }
        machine caller(value: Good) {
            effect();
            consume<Good, GoodMarker>(value);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize statement selections");
    let syntax = parse_syntax_trees(&tokens).expect("parse statement selections");
    let program = lower_syntax_trees(&syntax).expect("resolve statement selections");
    let selected = program
        .conformances
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|alias| alias.as_str() == "GoodMarker")
        })
        .expect("GoodMarker conformance")
        .symbol;
    let selected_type = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "Good")
        .expect("Good data")
        .symbol;
    let selections = program.authored_declaration_selections();
    let statement_calls = program
        .machines
        .iter()
        .flat_map(|machine| program.machine_state_handles(machine.states))
        .flat_map(|state| {
            program
                .tables
                .bodies
                .statements
                .statements(program.machine_state(*state).statement_nodes)
        })
        .filter_map(|statement| match statement {
            psi_symbol_resolved_trees::statement::StatementNode::Call(call) => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(statement_calls.len(), 2, "calls={statement_calls:#?}");
    assert!(
        statement_calls.iter().all(|call| {
            call.target.is_source_backed()
                && call.operational_acknowledgement.origin
                    == psi_language_semantics::CallOperationalAcknowledgementOrigin::Source
        }),
        "calls={statement_calls:#?}"
    );

    assert!(
        selections.iter().any(|selection| {
            selection.kind() == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::Call
                && matches!(
                    selection.target(),
                    psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(_)
                )
        }),
        "selections={selections:#?}"
    );
    assert!(selections.iter().any(|selection| {
        selection.kind()
            == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::StaticArgument
            && matches!(
                selection.target(),
                psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if target.selected_symbol() == selected_type
            )
    }));
    assert!(selections.iter().any(|selection| {
        selection.kind() == psi_symbol_resolved_trees::AuthoredDeclarationSelectionKind::Conformance
            && matches!(
                selection.target(),
                psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if target.selected_symbol() == selected
            )
    }));
}

#[test]
fn substituted_const_retains_authored_declaration_selection_custody() {
    let source = r#"
        const ROOT_SIZE: u64 = 4;
        machine selected_size() -> u64 { ROOT_SIZE }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize const selection");
    let syntax = parse_syntax_trees(&tokens).expect("parse const selection");
    let program = lower_syntax_trees(&syntax).expect("resolve const selection");
    let selection = program
        .authored_declaration_selections()
        .iter()
        .find(|selection| {
            matches!(
                selection.target(),
                psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if program.symbols.get(target.selected_symbol()).kind
                        == psi_symbols::SymbolKind::Const
            )
        })
        .expect("retained const selection");
    let occurrence = program
        .authored_declaration_selections()
        .iter()
        .position(|candidate| candidate == selection)
        .expect("const selection ordinal");

    assert!(
        program
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .any(|(expression, _)| program
                .tables
                .bodies
                .expressions
                .authored_selection_occurrences(expression)
                .any(|attached| attached.ordinal() == occurrence as u64))
    );
}

#[test]
fn const_substitution_obeys_current_activation_resolution_strata() {
    let base_source = r#"
        const Limits::VALUE: u64 = 1;
        const Limits::BASE_ONLY: u64 = 3;
        machine authored_value() -> u64 { Limits::VALUE }
    "#;
    let extension_source = r#"
        const Limits::VALUE: u64 = 2;
        const Generated::ONLY: u64 = 4;
        machine extension_reads_base() -> u64 { Limits::BASE_ONLY }
    "#;
    let second_extension_source =
        "machine second_extension_reads_first() -> u64 { Generated::ONLY }";
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
    let second_extension_source_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated-second.omg"),
            second_extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;

    // Deliberately put the extension first: source order must not let its
    // same-named const retarget the authored occurrence.
    let extension_tokens = Lexer::new(extension_source)
        .tokenize()
        .expect("tokenize extension");
    let mut syntax = parse_syntax_trees_with_id(extension_source_id, &extension_tokens)
        .expect("parse extension");
    let base_tokens = Lexer::new(base_source).tokenize().expect("tokenize base");
    let base_syntax = parse_syntax_trees_with_id(base_source_id, &base_tokens).expect("parse base");
    syntax.extend_from(&base_syntax);
    let second_extension_tokens = Lexer::new(second_extension_source)
        .tokenize()
        .expect("tokenize second extension");
    let second_extension_syntax =
        parse_syntax_trees_with_id(second_extension_source_id, &second_extension_tokens)
            .expect("parse second extension");
    syntax.extend_from(&second_extension_syntax);

    let program = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("cross-stratum duplicate consts remain separate");
    let const_targets = program
        .authored_declaration_selections()
        .iter()
        .filter_map(|selection| {
            let psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target) =
                selection.target()
            else {
                return None;
            };
            (program.symbols.get(target.selected_symbol()).kind == psi_symbols::SymbolKind::Const)
                .then(|| {
                    (
                        selection.source_span().source_id,
                        program
                            .symbols
                            .symbol_source_span(target.selected_symbol())
                            .expect("source-backed const")
                            .source_id,
                    )
                })
        })
        .collect::<Vec<_>>();

    assert_eq!(const_targets.len(), 3, "const targets={const_targets:?}");
    assert!(const_targets.contains(&(base_source_id, base_source_id)));
    assert!(const_targets.contains(&(extension_source_id, base_source_id)));
    assert!(const_targets.contains(&(second_extension_source_id, extension_source_id)));
    assert!(!const_targets.contains(&(base_source_id, extension_source_id)));
}

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

    lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("free-shadow and case collisions across strata remain separate");
}

#[test]
fn const_collision_walks_still_reject_within_the_extension_stratum() {
    for (left, right, expected) in [
        (
            "const Limits::VALUE: u64 = 1;",
            "const Limits::VALUE: u64 = 2;",
            "duplicate const `Limits::VALUE`",
        ),
        (
            "const Clash: u64 = 1;",
            "data Clash {}",
            "free-floating `const Clash` collides with data `Clash`",
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

        let diagnostics = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
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

    let program = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
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
            psi_symbol_resolved_trees::expression::ExpressionNode::StructLiteral(literal) => {
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
            psi_symbol_resolved_trees::domain::ProofFact::Membership(membership) => {
                Some(membership)
            }
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
            psi_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
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

    let program = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
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
    let Some(psi_symbol_resolved_trees::types::TypeReference::Reference(reference)) =
        &entry.return_type
    else {
        panic!("erase return remains a reference")
    };
    let psi_symbol_resolved_trees::types::TypeReference::DynamicTrait {
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

    let program = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("same-named trait catalogs must retain exact stratum identity");
    let argument_kind = |arguments| {
        let [psi_symbol_resolved_trees::types::TypeReference::Named { symbol, .. }] =
            program.child_type_references(arguments)
        else {
            panic!("one named static trait argument")
        };
        program.symbols.get(*symbol).kind
    };

    for (name, source_id, expected_kind) in [
        ("BaseChild", base_id, psi_symbols::SymbolKind::Proposition),
        (
            "ExtensionChild",
            extension_id,
            psi_symbols::SymbolKind::State,
        ),
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
            psi_symbols::SymbolKind::Proposition,
            "base_requirement",
            "extension_requirement",
        ),
        (
            "extension_use",
            extension_id,
            psi_symbols::SymbolKind::State,
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
    let program = lower_syntax_trees(&syntax).expect("resolve const visibility");

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
    lower_syntax_trees(&syntax).expect("private const-v0 behavior remains unchanged");

    let public_source = r#"pub const LABEL: string = "public";"#;
    let tokens = Lexer::new(public_source)
        .tokenize()
        .expect("tokenize public string const");
    let syntax = parse_syntax_trees(&tokens).expect("parse public string const");
    let diagnostics = lower_syntax_trees(&syntax)
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
        gold: u32[exact];
    }

    pub machine Inventory::clear(&mut self, inventory: &mut Inventory) {
        inventory.gold = 0;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

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
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

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
    let program = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
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
    let diagnostic = lower_syntax_trees_with_sources(&reach_syntax, Arc::new(reach_sources))
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
    let program = lower_syntax_trees_with_sources(&invoke_syntax, Arc::new(invoke_sources))
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
    let diagnostic = lower_syntax_trees(&syntax_trees)
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
    let diagnostic = lower_syntax_trees(&syntax_trees)
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
    let diagnostic = lower_syntax_trees(&syntax_trees)
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
        via Binding::Syscall(60)
        reaches Process;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("external realization must derive rather than repeat service reach");

    assert!(
        diagnostic[0]
            .message
            .contains("repeats an authored `reaches` row")
    );
}

#[test]
fn rejects_authored_empty_service_reach_on_external_realization_before_resolved_trees() {
    let source = r#"
        boundary trait Process {
            machine exit(code: i32)
            reaches Process;
        }

        machine exit_leaf(code: i32)
        satisfies Process::exit
        via Binding::Syscall(60)
        reaches;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("an explicit empty external reach must not collapse into omission");

    assert!(
        diagnostic[0]
            .message
            .contains("repeats an authored `reaches` row")
    );
}

#[test]
fn retains_external_realization_mechanism_without_rendering_classification() {
    let source = r#"
        boundary trait Console {
            machine write(value: u8);
        }

        machine write_leaf(value: u8)
        satisfies Console::write
        via Binding::CompilerIntrinsic;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax_trees).expect("resolve external realization");
    let leaf = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "write_leaf")
        .expect("external leaf");

    let psi_language_semantics::MachineSupplyMode::ExternalRealization { binding, mechanism } =
        leaf.supply_mode
    else {
        panic!("bodyless via leaf must retain external supply");
    };
    let binding = binding.expect("bootstrap binding identity");
    assert!(binding.is_valid());
    assert_eq!(
        mechanism,
        Some(psi_language_semantics::ExternalBindingMechanism::CompilerIntrinsic)
    );
    let [conformance] = program.machine_trait_conformances(leaf.satisfies) else {
        panic!("external leaf must retain one exact satisfaction row");
    };
    assert_eq!(conformance.external_binding, Some(binding));
}

#[test]
fn retains_ordinary_via_call_as_resolved_expression_without_fabricated_binding() {
    let source = r#"
        boundary trait Console {
            machine write(value: u8);
        }

        machine binding() -> i32 {
            0
        }

        machine write_leaf(value: u8)
        satisfies Console::write
        via binding();
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax_trees).expect("resolve ordinary via call");
    let leaf = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "write_leaf")
        .expect("external leaf");
    assert_eq!(
        leaf.supply_mode,
        psi_language_semantics::MachineSupplyMode::ExternalRealization {
            binding: None,
            mechanism: None,
        }
    );
    let [conformance] = program.machine_trait_conformances(leaf.satisfies) else {
        panic!("external leaf must retain one exact satisfaction row");
    };
    assert!(conformance.external_binding.is_none());
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) = program
        .tables
        .bodies
        .expressions
        .expression(conformance.via_expression)
    else {
        panic!("ordinary via source must retain its resolved call");
    };
    assert_eq!(call.target.as_str(), "binding");
    assert!(call.target_symbol.is_valid());
}

#[test]
fn keeps_attached_machines_as_distinct_callables() {
    let source = r#"
    pub machine Game::new() {}

    pub machine Game::running() {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    assert_eq!(program.machines.len(), 2);
    assert_eq!(program.machines[0].name.as_str(), "Game::new");
    assert_eq!(
        program.machines[0]
            .attached_data
            .as_ref()
            .map(|name| name.as_str()),
        Some("Game")
    );
    assert_eq!(program.machines[1].name.as_str(), "Game::running");
    assert_eq!(
        program
            .machine_state_handles(program.machines[0].states)
            .len(),
        1
    );
}

#[test]
fn lowers_domain_definitions() {
    let source = r#"
    domain Player::Valid
    requires
        self.health >= 0

    domain Player::Alive
    requires
        self in Player::Valid;
        self.health > 0

    domain Player::Tagged;

    domain Player::Usable =
        Player::Valid & Player::Alive;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    assert_eq!(program.domain_definitions.len(), 4);
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Alive")
        .expect("alive domain should lower");
    assert!(domain.symbol.is_valid());
    assert_eq!(domain.name.as_str(), "Player::Alive");
    let facts = program.proof_facts(domain.facts);
    assert_eq!(facts.len(), 2);
    let psi_symbol_resolved_trees::domain::ProofFact::Membership(membership) = &facts[0] else {
        panic!("first domain fact should be membership")
    };
    assert!(membership.domain_symbol.is_valid());
    assert!(domain.semantic_clause_token_count >= 3);
    assert_eq!(
        domain.predicate_body,
        psi_language_semantics::DomainPredicateBody::Present
    );
    assert!(domain.semantic_roles.is_empty());
    let tagged = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Tagged")
        .expect("tagged domain should lower");
    assert_eq!(
        tagged.predicate_body,
        psi_language_semantics::DomainPredicateBody::Bodyless
    );
    assert_eq!(tagged.semantic_clause_token_count, 0);
    assert!(tagged.semantic_roles.is_empty());
    assert!(
        program
            .symbols
            .find_child_by_name(program.symbols.root(), "Player::Alive")
            .is_some()
    );
    let usable = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Usable")
        .expect("usable alias should lower");
    let alias = usable.alias.as_ref().expect("alias theory");
    assert_eq!(alias.constituents.len(), 2);
    assert!(
        alias
            .constituents
            .iter()
            .all(|constituent| constituent.domain_symbol.is_valid())
    );
    assert!(usable.facts.is_empty(), "aliases are not predicate facts");
}

#[test]
fn resolves_exact_case_symbols_in_domain_proof_expressions() {
    let source = r#"
    data Command {
        case Move(dx: i32);
        case Say(volume: i32);
    }

    domain Command::Interactive
    requires
        self in Command::Move | Command::Say;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let command = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Command")
        .expect("Command data");
    let expected_cases = program
        .data_members(command.members)
        .iter()
        .filter_map(|member| match member {
            psi_symbol_resolved_trees::data::DataMember::Variant(variant) => Some(variant.symbol),
            _ => None,
        })
        .collect::<Vec<_>>();
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Command::Interactive")
        .expect("interactive domain");
    let [psi_symbol_resolved_trees::domain::ProofFact::Expression(expression)] =
        program.proof_facts(domain.facts)
    else {
        panic!("case union should remain one proof expression");
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Binary(union) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("proof expression should remain a case union");
    };

    for (expression, expected_case) in [union.left, union.right].into_iter().zip(expected_cases) {
        let psi_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) =
            program.tables.bodies.expressions.expression(expression)
        else {
            panic!("union operand should remain a case membership");
        };
        assert!(!membership.domain_symbol.is_valid());
        assert_eq!(membership.case_type_symbol, command.symbol);
        assert_eq!(membership.case_symbol, expected_case);
    }
}

#[test]
fn resolves_free_machine_calls_in_domain_predicates() {
    let source = r#"
    boundary machine no_wrap(base: addr, length: u64) -> bool;

    data Region {
        base: addr;
        length: u64;
    }

    domain Region::Valid
    requires
        no_wrap(self.base, self.length);
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Region::Valid")
        .expect("valid domain");
    let [psi_symbol_resolved_trees::domain::ProofFact::Expression(predicate)] =
        program.proof_facts(domain.facts)
    else {
        panic!("one predicate call");
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(*predicate)
    else {
        panic!("predicate should remain a call");
    };
    assert!(call.target_symbol.is_valid());
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "no_wrap")
        .expect("predicate machine");
    assert!(
        program
            .machine_state_handles(machine.states)
            .iter()
            .any(|state| program.machine_state(*state).symbol == call.target_symbol)
    );
}

#[test]
fn resolves_repeated_capacity_specializations_as_one_domain_identity() {
    let source = r#"
    domain [u8; 8]::Utf8
    requires
        valid_utf8(self);

    domain [u8; 16]::Utf8
    requires
        valid_utf8(self);

    data Holder {
        label: [u8; 8] in Utf8;
    }

    machine fill(out: &mut Holder)
    ensures
        out.label in Utf8
    {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    assert_eq!(
        program.domain_definitions[0].semantic_id, program.domain_definitions[1].semantic_id,
        "capacity-specialized declarations with the same normalized predicate should share semantic identity",
    );

    let machine = program.machines.first().expect("fill machine");
    let contract = program
        .machine_contracts(machine)
        .iter()
        .find(|contract| {
            contract.kind == psi_symbol_resolved_trees::signature::SignatureContractKind::Ensures
        })
        .expect("fill should retain its ensures contract");
    let [psi_symbol_resolved_trees::domain::ProofFact::Membership(membership)] =
        program.proof_facts(contract.facts)
    else {
        panic!("ensures should contain one domain membership")
    };
    assert!(membership.domain_symbol.is_valid());
}

#[test]
fn preserves_operator_declarations() {
    let source = r#"
    pub operator Slice::index<T>(items: &[T], index: usize) -> T
    requires
        index < items.len;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    assert_eq!(program.operators.len(), 1);
    let operator = &program.operators[0];
    assert!(operator.is_public);
    assert_eq!(
        program
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["Slice", "index"]
    );
    assert_eq!(
        program.data_type_parameters(operator.type_parameters).len(),
        1
    );
    assert_eq!(program.state_parameters(operator.parameters).len(), 2);
    assert!(operator.symbol.is_valid());
    assert!(operator.return_type.is_some());
    assert_eq!(program.signature_contracts(operator.contracts).len(), 1);
    assert!(operator.token_count > 0);
}

#[test]
fn resolves_operator_const_parameter_carriers() {
    let source = r#"
    pub operator ConstSurface::identity<const Count: u64>(
        value: [u8; Count]
    ) -> [u8; Count];
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let operator = program.operators.first().expect("const-generic operator");
    let [parameter] = program.data_type_parameters(operator.type_parameters) else {
        panic!("one const parameter")
    };
    let psi_symbol_resolved_trees::data::TypeParameterKind::Const {
        type_reference: psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name },
    } = &parameter.kind
    else {
        panic!("const parameter carrier")
    };
    assert_eq!(name.as_str(), "u64");
    assert!(
        symbol.is_valid(),
        "const carrier must retain semantic identity"
    );
}

#[test]
fn preserves_domain_operator_declarations() {
    let source = r#"
    data Quantity {
        value: i32;
    }

    domain Quantity::Additive
    requires
        self.value >= 0;

    operator Quantity::Additive::add(left: Quantity, right: Quantity) -> Quantity;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Quantity::Additive")
        .expect("domain should lower");
    let operators = program.operator_definitions(domain.operators);

    assert_eq!(operators.len(), 1);
    assert_eq!(
        domain.semantic_roles.denotation_dimension,
        Some(domain.semantic_id)
    );
    assert!(domain.semantic_roles.arithmetic_policy.is_none());
    assert!(operators[0].symbol.is_valid());
    assert_eq!(
        program
            .operator_path_members(operators[0].name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["add"]
    );
    assert_eq!(program.proof_facts(domain.facts).len(), 1);
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
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
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
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("competing operand domains must not infer an operator home");
    assert!(
        diagnostic[0]
            .message
            .contains("has more than one possible domain home")
    );
}

#[test]
fn does_not_infer_domain_establishment_from_contract_placement() {
    let source = r#"
    data Token {
        value: u64;
    }

    domain Token::Issued;

    machine Token::issue(value: u64) -> Token
    ensures
        result in Token::Issued
    {
        Token { value: value }
    }

    boundary trait TokenIssuer {
        machine issue(value: u64) -> Token
        ensures
            result in Token::Issued;
    }

    domain Token::Stamped;

    operator Token::Stamped::stamp(value: Token) -> Token
    ensures
        result in Token::Stamped;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    let issued = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Issued")
        .expect("issued domain");
    assert!(issued.establishment_routes.is_empty());

    let stamped = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Stamped")
        .expect("stamped domain");
    assert!(program.operator_definitions(stamped.operators).len() == 1);
    assert!(stamped.establishment_routes.is_empty());
}

#[test]
fn normalizes_authored_checked_and_boundary_requirement_routes() {
    use psi_language_semantics::DomainEstablishmentRoute;

    let source = r#"
    data Token { value: u64; }

    domain Token::Checked
    established by CheckedIssuer::issue;
    domain Token::Admitted
    established by BoundaryIssuer::issue;

    trait CheckedIssuer {
        machine issue(value: u64) -> Token in Checked;
    }
    boundary trait BoundaryIssuer {
        machine issue(value: u64) -> Token
        ensures result in Token::Admitted;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    for (domain_name, trait_name, is_boundary) in [
        ("Token::Checked", "CheckedIssuer", false),
        ("Token::Admitted", "BoundaryIssuer", true),
    ] {
        let domain = program
            .domain_definitions
            .iter()
            .find(|domain| domain.name.as_str() == domain_name)
            .expect("domain");
        let definition = program
            .traits
            .iter()
            .find(|definition| definition.name.as_str() == trait_name)
            .expect("trait");
        let requirement = program
            .trait_machine_signatures(definition.machines)
            .first()
            .expect("requirement");
        let expected = if is_boundary {
            DomainEstablishmentRoute::BoundaryRequirement {
                boundary_trait: definition.symbol,
                requirement: requirement.symbol,
            }
        } else {
            DomainEstablishmentRoute::CheckedRequirement {
                trait_definition: definition.symbol,
                requirement: requirement.symbol,
            }
        };
        assert!(domain.establishment_routes.contains(&expected));
    }
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

    let diagnostics = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect_err("an authored route must not gain authority from a hidden domain alias");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not name the domain on its exact result")),
        "unexpected domain-establishment diagnostics: {diagnostics:?}"
    );
}

#[test]
fn preserves_explicit_progress_profile_classification_during_resolution() {
    let source = r#"
    data SchedulerHandle {}
    domain SchedulerHandle::WeakFair
    satisfies ProgressProfile
    established by SchedulerAdmission::grant;
    boundary trait SchedulerAdmission {
        machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "SchedulerHandle::WeakFair")
        .expect("profile domain");

    assert_eq!(
        domain.classification,
        Some(psi_language_semantics::DomainClassification::ProgressProfile)
    );
    assert!(matches!(
        domain.establishment_routes.as_slice(),
        [psi_language_semantics::DomainEstablishmentRoute::BoundaryRequirement { .. }]
    ));
}

#[test]
fn boundary_requirement_route_accepts_exact_non_self_parameter_domain() {
    use psi_language_semantics::DomainEstablishmentRoute;

    let source = r#"
    data Token { value: u64; }
    domain Token::Pending
    established by BoundaryIngress::enter;
    boundary trait BoundaryIngress {
        machine enter(token: Token in Pending);
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Pending")
        .expect("pending domain");
    let ingress = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "BoundaryIngress")
        .expect("boundary ingress trait");
    let enter = program
        .trait_machine_signatures(ingress.machines)
        .first()
        .expect("entry requirement");
    assert!(
        domain
            .establishment_routes
            .contains(&DomainEstablishmentRoute::BoundaryRequirement {
                boundary_trait: ingress.symbol,
                requirement: enter.symbol,
            })
    );
}

#[test]
fn ordinary_requirement_route_rejects_parameter_domain_as_introduction() {
    let source = r#"
    data Token { value: u64; }
    domain Token::Pending
    established by OrdinaryIngress::enter;
    trait OrdinaryIngress {
        machine enter(token: Token in Pending);
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("an ordinary call must treat its parameter domain as a precondition");
    assert!(diagnostic[0].message.contains(
        "does not name the domain on its exact result or an exact non-self external-root parameter"
    ));
}

#[test]
fn rejects_unresolved_authored_domain_requirement_route() {
    let source = r#"
    data Token { value: u64; }
    domain Token::Issued
    established by MissingIssuer::issue;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees).expect_err("route must resolve exactly");
    assert!(
        diagnostic[0]
            .message
            .contains("does not resolve to one exact trait")
    );
}

#[test]
fn rejects_overloaded_signature_free_domain_requirement_route() {
    let source = r#"
    data Token { value: u64; }
    domain Token::Issued
    established by Issuer::issue;
    trait Issuer {
        machine issue(value: u64) -> Token in Issued;
        machine issue(value: i64) -> Token in Issued;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("a signature-free requirement path must not choose among overloads");
    assert_eq!(diagnostic.len(), 2);
    assert!(diagnostic[0].message.contains("declaring trait `Issuer`"));
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
fn signature_free_overload_reports_one_declaration_and_every_affected_use() {
    let source = r#"
        data Token { value: u64; }
        domain Token::Issued
        established by Issuer::issue;

        trait Issuer {
            machine issue(value: u64) -> Token in Issued;
            machine issue(value: i64) -> Token in Issued;
        }

        machine register<machine Selected>()
        where machine Selected satisfies Issuer::issue;
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let diagnostics = lower_syntax_trees(&syntax).expect_err("overload must reject every use");

    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics[0].message.contains("declaring trait `Issuer`"));
    assert!(diagnostics[1].message.starts_with("domain `Token::Issued`"));
    assert!(
        diagnostics[2]
            .message
            .starts_with("nominal machine parameter `Selected`")
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.source_span.is_some())
    );
    assert!(
        diagnostics[1].source_span.unwrap().span.start
            < diagnostics[2].source_span.unwrap().span.start
    );
}

#[test]
fn authored_signature_free_requirement_ignores_current_activation_extension_overloads() {
    let base = r#"
        data Token { value: u64; }
        domain Token::Issued
        established by Issuer::issue;
        trait Issuer {
            machine issue(value: u64) -> Token in Issued;
        }
        machine register<machine Selected>()
        where machine Selected satisfies Issuer::issue;
        {}
    "#;
    let extension = r#"
        trait Issuer {
            machine issue(value: u64) -> Token in Issued;
            machine issue(value: i64) -> Token in Issued;
        }
    "#;
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("main.omg"), base.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/extension.omg"),
            extension.to_owned(),
            PathBuf::from("."),
            None,
            psi_source::SourceOrigin::User,
            psi_source::SourceResolutionStratum::CurrentActivationExtension,
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

    lower_syntax_trees_with_sources(&syntax, Arc::new(sources)).expect(
        "authored signature-free uses must resolve only against the retained base trait family",
    );
}

#[test]
fn expands_alias_establishment_routes_to_atomic_domains() {
    use psi_language_semantics::DomainEstablishmentRoute;

    let source = r#"
    data Token {
        value: u64;
    }

    domain Token::Issued
    established by TokenIssuer::issue;
    domain Token::Stamped
    established by TokenIssuer::issue;
    domain Token::Ready = Token::Issued & Token::Stamped;

    boundary trait TokenIssuer {
        machine issue(value: u64) -> Token
        ensures
            result in Token::Ready;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let issuer = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "TokenIssuer")
        .expect("boundary trait");
    let requirement = program
        .trait_machine_signatures(issuer.machines)
        .first()
        .expect("issue requirement");
    let route = DomainEstablishmentRoute::BoundaryRequirement {
        boundary_trait: issuer.symbol,
        requirement: requirement.symbol,
    };

    for name in ["Token::Issued", "Token::Stamped"] {
        let atom = program
            .domain_definitions
            .iter()
            .find(|domain| domain.name.as_str() == name)
            .expect("atomic domain");
        assert_eq!(atom.establishment_routes, [route]);
    }
    let alias = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Ready")
        .expect("alias domain");
    assert!(
        alias.establishment_routes.is_empty(),
        "routes belong to normalized atomic facts, not alias spellings"
    );
}

#[test]
fn lowers_machine_contract_clauses() {
    let source = r#"
    machine distinct_indices(i: usize, j: usize)
    requires
        i < j
    ensures
        i != j
    {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let machine = program.machines.first().expect("machine");
    let contracts = program.machine_contracts(machine);

    assert_eq!(contracts.len(), 2);
    assert!(contracts[0].token_count >= 3);
    assert!(contracts[1].token_count >= 3);
    for (contract, keyword) in contracts.iter().zip(["requires", "ensures"]) {
        let span = contract
            .keyword_source_span
            .expect("resolved contract keyword span");
        assert_eq!(&source[span.span.start..span.span.end], keyword);
    }
    assert_eq!(program.proof_facts(contracts[0].facts).len(), 1);
    assert_eq!(program.proof_facts(contracts[1].facts).len(), 1);
}

#[test]
fn lowers_named_contract_evidence_bindings() {
    let source = r#"
    proposition carries(value: i32) evidence i32;
    machine forward(value: i32)
    requires input_proof: carries(value)
    ensures output_proof: carries(value)
    {
    }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax_trees).expect("lower");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let contracts = program.machine_contracts(machine);
    assert_eq!(contracts.len(), 2);
    assert_eq!(
        contracts[0].binding.as_ref().map(|name| name.as_str()),
        Some("input_proof")
    );
    assert_eq!(
        contracts[1].binding.as_ref().map(|name| name.as_str()),
        Some("output_proof")
    );
}

#[test]
fn classifies_evidence_forwarding_out_of_runtime_statements() {
    let source = r#"
    trait Evidence {}
    proposition carries(value: i32) evidence Evidence;
    machine forward(value: i32)
    requires input_proof: carries(value)
    ensures output_proof: carries(value)
    {
        output_proof = input_proof;
    }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax_trees).expect("lower");
    let [forwarding] = program.evidence_forwardings.as_slice() else {
        panic!("one resolved evidence forwarding expected");
    };
    assert!(forwarding.machine_symbol.is_valid());
    assert!(forwarding.state_symbol.is_valid());
    assert_eq!(forwarding.target.as_str(), "output_proof");
    assert_eq!(forwarding.source.as_str(), "input_proof");
    assert_eq!(forwarding.source_conformance, None);
    assert_eq!(program.snapshot().evidence_forwardings.len(), 1);
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.symbol == forwarding.machine_symbol)
        .expect("owner machine");
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    assert!(
        program
            .tables
            .bodies
            .statements
            .statements(state.statement_nodes)
            .is_empty(),
        "erased forwarding must not enter runtime statement spans"
    );
}

#[test]
fn resolves_explicit_evidence_producer_to_exact_subjectless_conformance() {
    let source = r#"
    trait Evidence {}
    proposition carries(value: i32) evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    machine produce(value: i32)
    ensures output_proof: carries(value)
    {
        output_proof = ConcreteEvidence;
    }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax_trees).expect("lower");
    let [assignment] = program.evidence_forwardings.as_slice() else {
        panic!("one resolved evidence assignment expected");
    };
    let producer = program
        .conformances
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|alias| alias.as_str() == "ConcreteEvidence")
        })
        .expect("subjectless producer conformance");
    assert_eq!(assignment.source_conformance, Some(producer.symbol));
    assert_eq!(
        program.snapshot().evidence_forwardings[0].source_conformance,
        Some(producer.symbol.arena_index())
    );
}

#[test]
fn binds_evidence_forwarding_to_attached_machine_with_duplicate_short_name() {
    let source = r#"
    data Left {}
    data Right {}
    trait Evidence {}
    proposition carries(value: i32) evidence Evidence;

    machine Left::forward(value: i32)
    requires incoming: carries(value)
    ensures outgoing: carries(value)
    {
        outgoing = incoming;
    }

    machine Right::forward(value: i32)
    requires incoming: carries(value)
    ensures outgoing: carries(value)
    {
        outgoing = incoming;
    }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax_trees).expect("lower");

    assert_eq!(program.evidence_forwardings.len(), 2);
    for (root_index, forwarding) in program.evidence_forwardings.iter().enumerate() {
        let machine = program
            .machines
            .iter()
            .nth(root_index)
            .expect("parallel root machine");
        assert_eq!(forwarding.machine_root_index, root_index);
        assert_eq!(forwarding.machine_symbol, machine.symbol);
    }
    assert_ne!(
        program.evidence_forwardings[0].machine_symbol,
        program.evidence_forwardings[1].machine_symbol
    );
}

#[test]
fn resolves_generic_calls_inside_machine_contracts() {
    let source = r#"
    data Index {
        case Zero;
        case Next(previous: Index);
    }

    machine generic<machine S>(value: Index) -> Index
    where machine S(index: Index) -> Index;
    {
        value
    }

    machine witness<machine Selected>(value: Index)
    where machine Selected(index: Index) -> Index;
    ensures generic<Selected>(value) == value
    {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let witness = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "witness")
        .expect("witness machine");
    let ensures = program
        .machine_contracts(witness)
        .iter()
        .find(|contract| {
            contract.kind == psi_symbol_resolved_trees::signature::SignatureContractKind::Ensures
        })
        .expect("witness ensures");
    let [psi_symbol_resolved_trees::domain::ProofFact::Expression(expression)] =
        program.proof_facts(ensures.facts)
    else {
        panic!("one expression fact")
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Binary(binary) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("equality expression")
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(binary.left)
    else {
        panic!("generic call on equality left")
    };
    assert!(call.target_symbol.is_valid());
    assert_eq!(call.machine_arguments.len(), 1);
    assert!(call.machine_arguments[0].symbol.is_valid());
}

#[test]
fn lowers_attached_main_state_name_as_main() {
    let source = r#"
    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    assert_eq!(program.machines.len(), 1);
    assert_eq!(program.machines[0].name.as_str(), "Main::main");
    assert_eq!(
        program.machines[0]
            .attached_data
            .as_ref()
            .map(|name| name.as_str()),
        Some("Main")
    );
    let state = program
        .machine_state_handles(program.machines[0].states)
        .first()
        .map(|state| program.machine_state(*state))
        .expect("entry state");
    assert_eq!(state.name.as_str(), "main");
}

#[test]
fn transition_target_prefers_state_over_same_named_attached_field() {
    let source = r#"
    data Main { next: bool; }

    machine Main::main(&mut self) {
        transition { _ -> next() }

        state next(&mut self) {}
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let machine = program.machines.first().expect("main machine");
    let states = program.machine_state_handles(machine.states);
    let entry = program.machine_state(states[0]);
    let next = program.machine_state(states[1]);
    let psi_symbol_resolved_trees::statement::Statement::Transition(transition) =
        &program.state_statements(entry.statements)[0]
    else {
        panic!("main should transition to next");
    };
    let psi_symbol_resolved_trees::statement::TransitionTarget::Named(target) = &transition.target
    else {
        panic!("next should remain a named transition target");
    };

    assert_eq!(target.symbol, next.symbol);
    assert_eq!(
        program.symbols.get(target.symbol).kind,
        psi_symbols::SymbolKind::State
    );
}

#[test]
fn resolves_qualified_attached_machine_tail_transition() {
    let source = r#"
    data Main {}

    machine Main::pack(left: i32, right: i32) -> i32 {
        left + right
    }

    machine Main::issue() -> i32 {
        transition { _ -> Main::pack(1, 2) }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let pack = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Main::pack")
        .expect("pack machine");
    let pack_state = program
        .machine_state_handles(pack.states)
        .first()
        .map(|state| program.machine_state(*state))
        .expect("pack state");
    let issue = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Main::issue")
        .expect("issue machine");
    let issue_state = program
        .machine_state_handles(issue.states)
        .first()
        .map(|state| program.machine_state(*state))
        .expect("issue state");
    let psi_symbol_resolved_trees::statement::Statement::Transition(transition) = program
        .state_statements(issue_state.statements)
        .last()
        .expect("terminal transition")
    else {
        panic!("issue should end in a transition");
    };
    let psi_symbol_resolved_trees::statement::TransitionTarget::Named(target) = &transition.target
    else {
        panic!("qualified tail call should remain a named transition");
    };

    assert_eq!(target.symbol, pack_state.symbol);
    assert!(target.head_symbol.is_valid());
}

#[test]
fn attached_calls_and_qualified_transitions_obey_resolution_strata() {
    let base = r#"
        data Gadget {}
        data BaseOnly {}
        data Leaf {}
        data Inner { leaf: Leaf; }
        data Outer { inner: Inner; }
        machine Gadget::work() -> i32 { 1 }
        machine BaseOnly::work() -> i32 { 3 }
        machine Leaf::work(&self) -> i32 { 5 }
        machine authored_call() -> i32 { Gadget::work() }
        machine authored_hidden_call() -> i32 { Ghost::work() }
        machine Outer::nested_call(&self) -> i32 { self.inner.leaf.work() }
        machine authored_transition() -> i32 {
            transition { _ -> Gadget::work() }
        }
    "#;
    let extension = r#"
        data Gadget {}
        data Ghost {}
        data Leaf {}
        machine Gadget::work() -> i32 { 2 }
        machine Ghost::work() -> i32 { 4 }
        machine Leaf::work(&self) -> i32 { 6 }
        machine extension_call() -> i32 { Gadget::work() }
        machine extension_reads_base() -> i32 { BaseOnly::work() }
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

    let program = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("attached-call resolution must retain stratum boundaries");
    let source_of = |symbol| {
        program
            .symbols
            .symbol_provenance_source_span(symbol)
            .expect("source-backed symbol")
            .source_id
    };
    let calls = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    let base_targets = calls
        .iter()
        .filter(|call| call.target.source_span().source_id == base_id)
        .map(|call| call.target_symbol)
        .collect::<Vec<_>>();
    assert_eq!(
        base_targets
            .iter()
            .filter(|symbol| {
                symbol.is_valid()
                    && program.symbols.name(**symbol) == "work"
                    && source_of(**symbol) == base_id
            })
            .count(),
        2,
        "qualified and nested authored calls must select Base"
    );
    assert!(base_targets.iter().any(|symbol| !symbol.is_valid()));
    assert!(calls.iter().any(|call| {
        call.target.source_span().source_id == extension_id
            && call.target_symbol.is_valid()
            && source_of(call.target_symbol) == extension_id
    }));
    assert!(calls.iter().any(|call| {
        call.target.source_span().source_id == extension_id
            && call.target_symbol.is_valid()
            && source_of(call.target_symbol) == base_id
    }));

    let transition_machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "authored_transition")
        .expect("authored transition machine");
    let transition_state =
        program.machine_state(program.machine_state_handles(transition_machine.states)[0]);
    let psi_symbol_resolved_trees::statement::Statement::Transition(transition) = program
        .state_statements(transition_state.statements)
        .last()
        .expect("terminal transition")
    else {
        panic!("authored transition remains terminal")
    };
    let psi_symbol_resolved_trees::statement::TransitionTarget::Named(target) = &transition.target
    else {
        panic!("authored qualified transition remains named")
    };
    assert_eq!(source_of(target.head_symbol), base_id);
    assert_eq!(source_of(target.symbol), base_id);
}

#[test]
fn resolves_self_parameter_type_to_machine_symbol() {
    let source = r#"
    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let machine = program.machines.first().expect("machine");
    let entry = program
        .machine_state_handles(machine.states)
        .first()
        .map(|state| program.machine_state(*state))
        .expect("entry state");
    let parameter = program
        .state_parameters(entry.parameters)
        .first()
        .expect("self parameter");

    let psi_symbol_resolved_trees::types::TypeReference::Reference(reference) =
        &parameter.type_reference
    else {
        panic!("self parameter should retain its authored reference shell");
    };
    let psi_symbol_resolved_trees::types::TypeReference::SelfType { symbol } =
        program.child_type_reference(reference.referee)
    else {
        panic!("self parameter referee should stay explicit");
    };

    assert_eq!(*symbol, machine.symbol);
}

#[test]
fn source_backed_names_are_used_when_sources_are_available() {
    let source = r#"
    data Inventory {
        gold: u32;
    }
    "#;
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("main.omg"), source.to_owned())
        .source_id;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees =
        parse_syntax_trees_with_id(source_id, &tokens).expect("parse should succeed");
    let program = lower_syntax_trees_with_sources(&syntax_trees, Arc::new(sources))
        .expect("lowering should succeed");
    let counts = program.symbols.name_storage_counts();

    assert!(
        counts.source_names > 0,
        "source identifiers should be stored by source span"
    );
    assert!(
        counts.owned_names == 0,
        "loaded source-backed identifiers should not allocate owned symbol names"
    );
    assert!(
        counts.static_names > 0,
        "builtins and synthetic roots should stay static"
    );
}

#[test]
fn lowerer_keeps_source_free_visibility_checks_permissive() {
    let mut sources = SourceMap::default();
    let declaration = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/extension.omg"),
            "data Extension {}".to_owned(),
            PathBuf::from("."),
            None,
            psi_source::SourceOrigin::User,
            psi_source::SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_span(psi_source::Span::new(5, 14));
    let lowerer = Lowerer::new(Some(Arc::new(sources)), Vec::new());

    assert!(
        lowerer
            .source_reference_can_see_declaration(psi_source::SourceSpan::default(), declaration,)
    );
}

#[test]
fn authored_base_paths_and_receiverless_calls_ignore_extension_first_declarations() {
    let base = r#"
        data Choice {}
        machine pick() -> i32 { 1 }
        proposition selected(value: i32);
        machine authored(value: Choice) -> i32 { pick() }
        machine authored_missing() -> i32 { generated_pick() }
        proposition authored(value: i32) = selected(value);
        proposition authored_missing(value: i32) = generated_selected(value);
    "#;
    let extension = r#"
        data Choice {}
        machine pick() -> i32 { 2 }
        machine generated_pick() -> i32 { 3 }
        proposition selected(value: i32);
        proposition generated_selected(value: i32);
    "#;
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("main.omg"), base.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/extension.omg"),
            extension.to_owned(),
            PathBuf::from("."),
            None,
            psi_source::SourceOrigin::User,
            psi_source::SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let extension_tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize extension");
    let mut syntax = parse_syntax_trees_with_id(extension_id, &extension_tokens)
        .expect("parse extension source");
    let base_tokens = Lexer::new(base).tokenize().expect("tokenize base");
    let base_syntax = parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse base source");
    syntax.extend_from(&base_syntax);

    let program = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("base references must ignore extension-first declarations");
    let authored = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "authored")
        .expect("authored machine");
    let entry = program
        .machine_state_handles(authored.states)
        .first()
        .map(|handle| program.machine_state(*handle))
        .expect("authored entry");
    let parameter = program
        .state_parameters(entry.parameters)
        .first()
        .expect("Choice parameter");
    let psi_symbol_resolved_trees::types::TypeReference::Named { symbol, .. } =
        &parameter.type_reference
    else {
        panic!("Choice parameter remains nominal")
    };
    assert_eq!(
        program
            .symbols
            .symbol_provenance_source_span(*symbol)
            .expect("Choice declaration provenance")
            .source_id,
        base_id
    );
    let hidden_pick_target = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            psi_symbol_resolved_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "generated_pick" =>
            {
                Some(call.target_symbol)
            }
            _ => None,
        })
        .expect("authored receiverless extension-only pick call");
    assert!(!hidden_pick_target.is_valid());
    let pick_target = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            psi_symbol_resolved_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "pick" =>
            {
                Some(call.target_symbol)
            }
            _ => None,
        })
        .expect("authored receiverless pick call");
    assert_eq!(
        program
            .symbols
            .symbol_provenance_source_span(pick_target)
            .expect("pick declaration provenance")
            .source_id,
        base_id
    );
    let authored_proposition = program
        .propositions
        .iter()
        .find(|proposition| proposition.name.as_str() == "authored")
        .expect("authored proposition");
    let psi_symbol_resolved_trees::proposition::PropositionBody::Transparent { proposition } =
        authored_proposition.body
    else {
        panic!("authored proposition stays transparent")
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(proposition)
    else {
        panic!("authored proposition body stays a call")
    };
    assert_eq!(
        program
            .symbols
            .symbol_provenance_source_span(call.target_symbol)
            .expect("selected proposition provenance")
            .source_id,
        base_id
    );
    let authored_missing_proposition = program
        .propositions
        .iter()
        .find(|proposition| proposition.name.as_str() == "authored_missing")
        .expect("authored missing proposition");
    let psi_symbol_resolved_trees::proposition::PropositionBody::Transparent { proposition } =
        authored_missing_proposition.body
    else {
        panic!("authored missing proposition stays transparent")
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(proposition)
    else {
        panic!("authored missing proposition body stays a call")
    };
    assert!(!call.target_symbol.is_valid());
}

#[test]
fn trait_operator_requirement_retains_fixed_token_after_resolution() {
    let source = r#"
        trait Ranked<T> {
            operator < compare(left: T, right: T) -> bool;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax).expect("resolve");
    let trait_definition = program.traits.first().expect("Ranked trait");
    let [requirement] = program.trait_machine_signatures(trait_definition.machines) else {
        panic!("one trait operator requirement expected");
    };

    assert_eq!(
        requirement.spelling.map(|spelling| spelling.symbol()),
        Some("<")
    );
}

#[test]
fn deep_left_associated_boolean_expression_resolves_on_the_default_test_stack() {
    let expression = std::iter::repeat_n("enabled", 128)
        .collect::<Vec<_>>()
        .join(" && ");
    let source =
        format!("data Root {{}} machine Root::measure(enabled: bool) -> bool {{ {expression} }}");
    let tokens = Lexer::new(&source)
        .tokenize()
        .expect("tokenize deep expression");
    let syntax = parse_syntax_trees(&tokens).expect("parse deep expression");

    lower_syntax_trees(&syntax).expect("resolve deep expression on the default test stack");
}
