//! Recorded Product selections retain the source occurrence that authorized each operand.
//! Parsing and typing produce the requests; these tests supply completed host
//! events without executing another Build activation.

use super::provider_declarations::{
    CompositionMode, ProviderPlanningContext, ProviderSelection,
    collect_product_provider_selections,
};
use language_semantics::declaration_selection::{
    BuildOperation, ExecutedProductSelection, ProductDeclarationRequest, ProductTarget,
};
use semantic_vocabulary::PackageKeyIdentity;
use source::{DependencyScope, SourceOrigin};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

const PRELUDE: &str = r#"
public data Build { marker: u64; }
public boundary trait BuildBindingOperations {
    machine prepare(builder: &Build) -> u64 reaches none crashes none;
    machine select(activation: u64) reaches none crashes none;
    machine select_provider(activation: u64, target: u64, independent: bool) reaches none crashes none;
}
public machine build_selection_receiver<'activation>(builder: &'activation mut Build) -> &'activation mut Build {
    builder
}
public data BuildTarget [copy] { case LinuxX86_64; case MacosArm64; }
public const linux_x86_64: BuildTarget = BuildTarget::LinuxX86_64;
public const macos_arm64: BuildTarget = BuildTarget::MacosArm64;
public data CompositionMode [copy] { case Fused; case Independent; }
public machine build_selection_target(target: BuildTarget) -> u64 {
    match target {
        BuildTarget::LinuxX86_64 -> 1u64
        BuildTarget::MacosArm64 -> 2u64
    }
}
public machine build_selection_independent(mode: CompositionMode) -> bool {
    match mode {
        CompositionMode::Fused -> false
        CompositionMode::Independent -> true
    }
}
"#;

const PRODUCT: &str = r#"
public boundary trait Service { machine tick() reaches none crashes none; }
public First satisfies Service { machine tick() reaches none crashes none {} }
public Second satisfies Service { machine tick() reaches none crashes none {} }
data NotSlot {}
data RuntimeDescriptor {}
"#;

const TWO_CHOICES: &str = r#"
machine build(builder: &mut Build) { first(builder); second(builder); }
machine first(builder: &mut Build) {
    Build::select_provider<Service, First>(builder, linux_x86_64);
}
machine second(builder: &mut Build) {
    Build::select_provider<Service, Second>(builder, macos_arm64, CompositionMode::Independent);
}
machine unrelated(builder: &mut Build) {}
"#;

fn package(byte: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([byte; 32]).unwrap()
}

struct Fixture {
    typed: TypedTrees,
    activation: SymbolHandle,
    declared: Vec<Arc<ProductDeclarationRequest>>,
    authors: Vec<SymbolHandle>,
}

impl Fixture {
    fn new(build: &str, product: &str, dependency: Option<&str>) -> Self {
        Self::for_operation(
            build,
            product,
            dependency,
            BuildOperation::ProviderSelection,
        )
    }

    fn for_operation(
        build: &str,
        product: &str,
        dependency: Option<&str>,
        operation: BuildOperation,
    ) -> Self {
        let mut sources = source::SourceMap::default();
        let mut syntax = None;
        let mut ids = Vec::new();
        let mut inputs = vec![
            (
                "root/build.omg",
                build,
                "root",
                package(7),
                SourceOrigin::User,
                DependencyScope::Build,
            ),
            (
                "<build-prelude>",
                PRELUDE,
                "root",
                package(7),
                SourceOrigin::Toolchain,
                DependencyScope::Build,
            ),
            // Authored Build calls resolve in the toolchain code module.
            (
                "<build-prelude:Build>",
                "module Build;\n",
                "root",
                package(7),
                SourceOrigin::Toolchain,
                DependencyScope::Build,
            ),
            (
                "root/main.omg",
                product,
                "root",
                package(7),
                SourceOrigin::User,
                DependencyScope::Product,
            ),
        ];
        if let Some(dependency) = dependency {
            inputs.push((
                "dep/main.omg",
                dependency,
                "dep",
                package(8),
                SourceOrigin::User,
                DependencyScope::Product,
            ));
            sources.retain_product_dependency_scope([(package(7), "dep", package(8))]);
        }
        for (path, text, root, owner, origin, scope) in inputs {
            let id = sources
                .add_checked_instance(
                    path.into(),
                    text,
                    root.into(),
                    Some(owner),
                    origin,
                    source::SourceResolutionStratum::Base,
                    scope,
                )
                .source_id;
            ids.push(id);
            let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
            match &mut syntax {
                None => {
                    syntax = Some(
                        tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap(),
                    )
                }
                Some(syntax) => {
                    tokens_to_syntax_trees::parse(syntax, id, &tokens).unwrap();
                }
            }
        }
        let mut request =
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax.unwrap());
        request.sources = Some(Arc::new(sources));
        request.top_level_bindings = vec![
            symbols::SourceScopedTopLevelBinding::new(ids[0], ids[1], "Build"),
            symbols::SourceScopedTopLevelBinding::module_import(ids[0], ids[2], "Build", 0),
        ];
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let activation = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "build")
            .unwrap()
            .symbol;
        let mut occurrences =
            typed_trees::product_operand::collect_product_selection_occurrences(&typed);
        occurrences.retain(|occurrence| occurrence.request.operation == operation);
        occurrences.sort_by_key(|occurrence| occurrence.request.source_span.span.start);
        let mut declared: Vec<Arc<ProductDeclarationRequest>> = Vec::new();
        let mut authors = Vec::new();
        // Preparation and commit share one source request. Their two protocol
        // calls do not invent two authored declarations or two host events.
        for occurrence in occurrences {
            if declared
                .iter()
                .any(|request| typed.same_product_request_source(request, &occurrence.request))
            {
                continue;
            }
            declared.push(occurrence.request);
            authors.push(typed.symbols.authored_origin(occurrence.machine));
        }
        Self {
            typed,
            activation,
            declared,
            authors,
        }
    }

    fn events(&self, values: &[(ProductTarget, CompositionMode)]) -> Vec<ExecutedProductSelection> {
        assert_eq!(self.declared.len(), values.len());
        self.declared
            .iter()
            .zip(&self.authors)
            .zip(values)
            .map(
                |((request, author), (target, composition))| ExecutedProductSelection {
                    static_environment: None,
                    target: Some(*target),
                    request: Arc::clone(request),
                    selecting_machine: self.typed.symbols.symbol_source_span(*author),
                    composition: Some(*composition),
                },
            )
            .collect()
    }

    fn collect(
        &self,
        events: &[ExecutedProductSelection],
    ) -> Result<Vec<ProviderSelection>, Vec<diagnostics::Diagnostic>> {
        collect_product_provider_selections(
            ProviderPlanningContext::new(&self.typed, DependencyScope::Product),
            Some(self.activation),
            &self.declared,
            events,
        )
    }
}

#[test]
fn a_runnable_product_batch_requires_a_named_implementation() {
    let pass = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/fixtures/pass/build/product_declaration_batch/main.omg"
    ));
    let fail = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/fixtures/fail/build/product_declaration_batch/main.omg"
    ));
    let expected = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/fixtures/fail/build/product_declaration_batch/expected.txt"
    ))
    .trim();
    assert_eq!(
        pass, fail,
        "only the selected provider changes in the Build twin"
    );
    for (program, provider) in [(pass, "SelectedLookup"), (fail, "UnrelatedLookup")] {
        // The harness supplies completed Build events. The kept Build files
        // bind the same declarations for all four hosted targets in CLI runs.
        let build = format!(
            "machine build(builder: &mut Build) {{ Build::select_provider<LookupService, {provider}>(builder, linux_x86_64); }}"
        );
        let fixture = Fixture::new(&build, program, None);
        let events = fixture.events(&[(ProductTarget::LinuxX86_64, CompositionMode::Fused)]);
        let selected = fixture.collect(&events);
        if provider == "SelectedLookup" {
            assert_eq!(selected.unwrap().len(), 1);
        } else {
            assert_eq!(selected.unwrap_err()[0].message, expected);
        }
    }
}

#[test]
fn a_provider_batch_keeps_the_source_of_applied_type_arguments() {
    use typed_trees::product_operand::{ProductApplicationDemand, ProductSourceLookups};

    // The selected trait and implementation must apply to the same exact data
    // type, even when both data declarations have the same empty storage.
    let product = "public data Tag {} public data Other {}\n\
        public boundary trait Service<T> { machine tick() reaches none crashes none; }\n\
        public First<T> satisfies Service<T> { machine tick() reaches none crashes none {} }";
    for actual in ["Tag", "Other"] {
        let build = format!(
            "machine build(builder: &mut Build) {{ Build::select_provider<Service<Tag>, First<{actual}>>(builder, linux_x86_64); }}"
        );
        let mut fixture = Fixture::new(&build, product, None);
        let source = fixture.typed.clone();
        let lookups = ProductSourceLookups::new(&source);
        for (request, author) in fixture.declared.iter().zip(&fixture.authors) {
            for (position, operand) in request.operands.iter().enumerate() {
                typed_trees_to_typed_trees::prepare_product_operand_application(
                    &mut fixture.typed,
                    &lookups,
                    operand,
                    request.operation.product_operand_role(position).unwrap(),
                    *author,
                    None,
                    ProductApplicationDemand::FormationOnly,
                )
                .unwrap();
            }
        }
        let events = fixture.events(&[(ProductTarget::LinuxX86_64, CompositionMode::Fused)]);
        let selected = fixture.collect(&events);
        if actual == "Tag" {
            assert_eq!(selected.unwrap().len(), 1);
        } else {
            assert!(
                selected.unwrap_err()[0]
                    .message
                    .contains("does not satisfy the selected application")
            );
        }
    }
}

#[test]
fn a_service_audit_needs_an_event_from_its_exact_author() {
    let fixture = Fixture::for_operation(
        r#"
machine build(builder: &mut Build) { configure(builder); }
machine configure(builder: &mut Build) { Build::audit_service<Unused>(builder); }
machine unselected(builder: &mut Build) { Build::audit_service<Untaken>(builder); }
"#,
        "public boundary trait Unused {} public boundary trait Untaken {}",
        None,
        BuildOperation::ServiceAudit,
    );
    let [request, untaken] = fixture.declared.as_slice() else {
        panic!("two valid authored audit requests");
    };
    assert_eq!(request.operands[0].path, "Unused");
    assert_eq!(untaken.operands[0].path, "Untaken");
    let formations = fixture
        .declared
        .iter()
        .cloned()
        .zip(fixture.authors.iter().copied())
        .collect::<Vec<_>>();
    let admit = |events: &[ExecutedProductSelection]| {
        super::product_declarations::admit(
            &fixture.typed,
            Some(fixture.activation),
            &fixture.declared,
            events,
            &formations,
        )
    };
    assert!(admit(&[]).unwrap().service_audits.is_empty());

    let mut event = ExecutedProductSelection {
        static_environment: None,
        target: None,
        request: Arc::clone(request),
        selecting_machine: fixture.typed.symbols.symbol_source_span(fixture.authors[0]),
        composition: None,
    };
    let admitted = admit(&[event.clone(), event.clone()]).unwrap();
    let [audit] = admitted.service_audits.as_slice() else {
        panic!("only the executed registration survives and duplicates collapse");
    };
    let selected = audit
        .declaration
        .rejoin(
            &fixture.typed.symbols,
            fixture
                .typed
                .traits()
                .iter()
                .map(|declaration| declaration.symbol),
        )
        .unwrap();
    assert_eq!(fixture.typed.symbols.name(selected), "Unused");

    event.selecting_machine = fixture.typed.symbols.symbol_source_span(fixture.authors[1]);
    let Err(errors) = admit(&[event]) else {
        panic!("an event with a different valid author cannot register this audit");
    };
    assert_eq!(
        errors[0].message,
        "executed service audit has no matching source occurrence"
    );
    assert_eq!(errors[0].source_span, Some(request.source_span));
}

#[test]
fn recorded_targets_composition_and_helper_authors_follow_event_order() {
    let fixture = Fixture::new(TWO_CHOICES, PRODUCT, None);
    let mut events = fixture.events(&[
        (ProductTarget::LinuxX86_64, CompositionMode::Fused),
        (ProductTarget::MacosArm64, CompositionMode::Independent),
    ]);
    events.reverse();
    let selected = fixture.collect(&events).unwrap();
    assert_eq!(selected.len(), 2);
    let activation = fixture
        .typed
        .symbols
        .symbol_source_span(fixture.activation)
        .unwrap();
    for (ordinal, (selection, event)) in selected.iter().zip(&events).enumerate() {
        assert_eq!(selection.target, event.target);
        assert_eq!(Some(selection.composition_mode), event.composition);
        assert_eq!(selection.origin.event_ordinal(), Some(ordinal as u64));
        assert_eq!(selection.origin.activation().span(), activation.span);
        assert_eq!(
            selection.origin.author().span(),
            event.selecting_machine.unwrap().span
        );
        assert_ne!(selection.origin.author().span(), activation.span);
        assert_eq!(
            selection.origin.occurrence(),
            event.request.source_span.span
        );
        assert_eq!(selection.subject.authored_path(), "Service");
    }
    assert_eq!(selected[0].provider_type.authored_path, "Second");
    assert_eq!(selected[1].provider_type.authored_path, "First");
}

#[test]
fn unexecuted_provider_formations_still_validate_both_operands() {
    for (slot, provider, invalid_operand) in [
        ("NotSlot", "First", "NotSlot"),
        ("Service", "MissingProvider", "MissingProvider"),
    ] {
        let build = format!(
            "machine build(builder: &mut Build) {{ Build::select_provider<{slot}, {provider}>(builder, linux_x86_64); }}"
        );
        let fixture = Fixture::new(&build, PRODUCT, None);
        assert_eq!(fixture.declared.len(), 1);
        let refused = fixture.collect(&[]).unwrap_err();
        assert_eq!(
            refused[0].message,
            format!(
                "cannot select provider operand `{invalid_operand}` here; it must name one visible declaration of the required kind"
            )
        );
    }
}

#[test]
fn a_runtime_data_declaration_is_not_a_static_provider_record() {
    let fixture = Fixture::new(
        "machine build(builder: &mut Build) { Build::select_provider<Service, RuntimeDescriptor>(builder, linux_x86_64); }",
        PRODUCT,
        None,
    );
    let events = fixture.events(&[(ProductTarget::LinuxX86_64, CompositionMode::Fused)]);
    let refused = fixture.collect(&events).unwrap_err();
    assert_eq!(
        refused[0].message,
        "provider selection `RuntimeDescriptor` is not a static operation record from a named implementation; a data type is not a provider value"
    );
}

#[test]
fn a_product_dependency_does_not_expose_its_private_provider() {
    let fixture = Fixture::new(
        "machine build(builder: &mut Build) { Build::select_provider<dep::ForeignService, dep::Hidden>(builder, linux_x86_64); }",
        PRODUCT,
        Some(
            "public boundary trait ForeignService { machine tick() reaches none crashes none; }\nHidden satisfies ForeignService { machine tick() reaches none crashes none {} }",
        ),
    );
    let events = fixture.events(&[(ProductTarget::LinuxX86_64, CompositionMode::Fused)]);
    let refused = fixture.collect(&events).unwrap_err();
    assert_eq!(
        refused[0].message,
        "cannot select provider operand `dep::Hidden` here; it must name one visible declaration of the required kind"
    );
}

#[test]
fn provider_events_cannot_replace_their_declared_occurrence_or_lexical_author() {
    let fixture = Fixture::new(TWO_CHOICES, PRODUCT, None);
    let events = fixture.events(&[
        (ProductTarget::LinuxX86_64, CompositionMode::Fused),
        (ProductTarget::MacosArm64, CompositionMode::Independent),
    ]);
    let orphan = collect_product_provider_selections(
        ProviderPlanningContext::new(&fixture.typed, DependencyScope::Product),
        Some(fixture.activation),
        &[],
        &events,
    );
    assert!(orphan.is_err(), "an event requires its declared request");
    let unrelated = fixture
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "unrelated")
        .unwrap()
        .symbol;
    let mut changed = events.clone();
    changed[0].selecting_machine = fixture.typed.symbols.symbol_source_span(unrelated);
    assert!(
        fixture.collect(&changed).is_err(),
        "a sibling cannot supply another helper's selection occurrence"
    );
    let mut missing_composition = events;
    missing_composition[0].composition = None;
    let refused = fixture.collect(&missing_composition).unwrap_err();
    assert!(
        refused
            .iter()
            .any(|diagnostic| diagnostic.message.contains("evaluated composition mode"))
    );
}

/// A package hands its choices to its consumers as source anchors. A
/// consumer that types the same source finds the same selections again,
/// with the target, composition mode and origin each was made with.
#[test]
fn retained_provider_choices_rejoin_the_same_declarations_in_another_typing() {
    use super::provider_declarations::RetainedProviderSelection;
    let package = Fixture::new(TWO_CHOICES, PRODUCT, None);
    let selected = package
        .collect(&package.events(&[
            (ProductTarget::LinuxX86_64, CompositionMode::Fused),
            (ProductTarget::MacosArm64, CompositionMode::Independent),
        ]))
        .unwrap();
    let retained = selected
        .iter()
        .map(|selection| RetainedProviderSelection::capture(&package.typed, selection).unwrap())
        .collect::<Vec<_>>();
    let mut consumer = Fixture::new(TWO_CHOICES, PRODUCT, None).typed;
    let mut candidates = symbols::SourceSpanCandidates::default();
    let mut rejoined = Vec::new();
    for choice in &retained {
        let selection = choice
            .rejoin(
                &mut consumer,
                DependencyScope::Product,
                &rejoined,
                &mut candidates,
            )
            .unwrap_or_else(|refusal| panic!("{}", refusal.message));
        rejoined.push(selection);
    }
    for (original, rejoined) in selected.iter().zip(&rejoined) {
        assert_eq!(rejoined.target, original.target);
        assert_eq!(rejoined.composition_mode, original.composition_mode);
        assert_eq!(rejoined.origin, original.origin);
        assert_eq!(
            rejoined.subject.canonical_path(),
            original.subject.canonical_path()
        );
        assert_eq!(
            rejoined.provider_type.canonical_path,
            original.provider_type.canonical_path
        );
    }
    assert_eq!(rejoined[0].provider_type.canonical_path, "First");
    assert_eq!(rejoined[1].provider_type.canonical_path, "Second");
}

/// The twin: the consumer's source declares the providers one line further
/// down. An anchor names a place in source, so nothing there is the
/// declaration that was chosen, and the same name elsewhere is not taken
/// for it.
#[test]
fn a_retained_provider_choice_is_not_rejoined_by_name_alone() {
    use super::provider_declarations::RetainedProviderSelection;
    let package = Fixture::new(TWO_CHOICES, PRODUCT, None);
    let selected = package
        .collect(&package.events(&[
            (ProductTarget::LinuxX86_64, CompositionMode::Fused),
            (ProductTarget::MacosArm64, CompositionMode::Independent),
        ]))
        .unwrap();
    let retained = RetainedProviderSelection::capture(&package.typed, &selected[0]).unwrap();
    let moved = format!("\ndata Padding {{}}{PRODUCT}");
    let mut consumer = Fixture::new(TWO_CHOICES, &moved, None).typed;
    assert!(
        retained
            .rejoin(
                &mut consumer,
                DependencyScope::Product,
                &[],
                &mut symbols::SourceSpanCandidates::default(),
            )
            .is_err()
    );
}
