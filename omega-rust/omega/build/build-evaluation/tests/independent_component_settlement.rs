//! Provider settlement closes `Independent` selections against the component
//! descriptions the package inputs attach.
//!
//! The provider plan comes from a real source fixture routed through the
//! build-evaluation entrance (`filter_target_machines` for the target roster,
//! then `settle_checked_providers`). Each realizing component is a canonical
//! Terminal module described the way a dependency's own compilation would
//! describe it and attached to the package inputs as bytes plus the observed
//! subject; settlement verifies those bytes itself before provider planning
//! joins them. The build's `Independent` selection is constructed as the
//! `ProviderSelection` the build machine would have harvested: the frontend
//! harvest of `CompositionMode::Independent` is the attributed regression
//! recorded beside COMPONENT-SUBSTRATE, so it is not on this route yet.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use build_evaluation::target_machines::{
    SelectedTargetMachineDeclarations, filter_target_machines,
};
use build_evaluation::{CheckedProviderSelection, settle_checked_providers};
use component_description::test_support::{
    bare_module, describe_module, module_subject, provider_module,
};
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use package_compilation::{
    BuildDeclarationKind, IndependentComponentDescription, PackageCompilationInputs,
    PackageDependencyBinding, PackageSourceBinding,
};
use provider_planning::{
    CompositionMode, ProviderPlanDerivation, ProviderSelection, ProviderSelectionIdentity,
    ProviderSelectionSubject, derive_satisfies_plans,
};
use semantic_vocabulary::PackageKeyIdentity;
use terminal_psi::TerminalModule;
use typed_trees::TypedTrees;

const SOURCE: &str = r#"
    boundary trait MachineControl {}
    boundary trait PortIo {}

    pub data InterruptAcknowledgement [copy] { token: u64; }
    pub domain InterruptAcknowledgement::Pending;
    pub data LapicCompletion {}

    pub boundary requirement InterruptAcknowledgement::complete(self)
    reaches <= MachineControl + PortIo
    requires self in InterruptAcknowledgement::Pending;

    machine LapicCompletion::complete(
        acknowledgement: InterruptAcknowledgement in Pending
    )
    satisfies InterruptAcknowledgement::complete
    reaches MachineControl
    {
    }
"#;

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

/// Package identities for the fixture graph: the root application and the
/// dependencies whose descriptions a build may attach.
fn identity(seed: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([seed; 32]).expect("nonzero package identity")
}

const ROOT: u8 = 1;
const LAPIC: u8 = 2;
const OTHER: u8 = 3;

/// Physical source roots for the fixture packages. Package inputs
/// canonicalize every root, so the directories must exist; nothing is read
/// from them because provider settlement consumes the typed program.
struct PackageRoots(PathBuf);

impl PackageRoots {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-independent-component-settlement-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        for package in ["root", "lapic", "other"] {
            std::fs::create_dir_all(path.join(package)).expect("create package root");
        }
        Self(path)
    }

    fn binding(&self, seed: u8, name: &str) -> PackageSourceBinding {
        PackageSourceBinding::new(identity(seed), name, self.0.join(name))
    }
}

impl Drop for PackageRoots {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The consumer side of one build: the typed program, its retained target
/// roster, the build's one provider selection, and the package graph whose
/// attachments settlement verifies.
struct Build {
    typed: TypedTrees,
    target_machines: SelectedTargetMachineDeclarations,
    selection: ProviderSelection,
    plan: ProviderPlan,
    roots: PackageRoots,
}

impl Build {
    fn new(mode: CompositionMode) -> Self {
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add(
                PathBuf::from("independent_selection.omg"),
                SOURCE.to_owned(),
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(SOURCE)
            .tokenize()
            .expect("tokenize fixture");
        let mut syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
            .expect("parse fixture");
        let target_machines = filter_target_machines(&mut syntax, None)
            .expect("the fixture declares no target machine");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: &syntax,
                sources: Some(std::sync::Arc::new(sources)),
                top_level_bindings: Vec::new(),
            },
        )
        .expect("resolve fixture");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type fixture");
        let requirement = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "InterruptAcknowledgement::complete")
            .expect("typed requirement");
        let provider = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "LapicCompletion::complete")
            .expect("typed checked provider");
        let provider_type = typed
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "LapicCompletion")
            .expect("nominal provider type");
        let derived = derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None));
        assert_eq!(derived.len(), 1, "one exact checked provider plan");
        let plan = derived[0].plan.clone();
        let selection = ProviderSelection {
            subject: ProviderSelectionSubject::BoundaryRequirement(ProviderSelectionIdentity {
                symbol: requirement.symbol,
                package: typed.symbols.symbol_package_identity(requirement.symbol),
                canonical_path: plan.schema.trait_name.clone(),
                authored_path: "InterruptAcknowledgement::complete".to_owned(),
            }),
            provider_type: ProviderSelectionIdentity {
                symbol: provider_type.symbol,
                package: typed.symbols.symbol_package_identity(provider_type.symbol),
                canonical_path: plan.provider_type.clone(),
                authored_path: "LapicCompletion".to_owned(),
            },
            composition_mode: mode,
            selecting_machine: provider.symbol,
            source_span: typed
                .symbols
                .symbol_provenance_source_span(provider.symbol)
                .expect("the selecting machine retains its authored span"),
        };
        Self {
            typed,
            target_machines,
            selection,
            plan,
            roots: PackageRoots::new(),
        }
    }

    /// The exact coordinates the selected plan expects a component to
    /// realize: requirement identity, provider type, checked adapter machine.
    fn realization(&self) -> (String, String, String) {
        let ProviderBinding::CheckedAdapter {
            machine_identity, ..
        } = &self.plan.rows[0].binding
        else {
            panic!("the fixture selects a checked adapter")
        };
        (
            self.plan.rows[0].requirement_identity.clone(),
            self.plan.provider_type.clone(),
            machine_identity.clone(),
        )
    }

    fn realizing_module(&self) -> TerminalModule {
        let (requirement, provider, machine) = self.realization();
        provider_module(&requirement, &provider, &machine)
    }

    /// The root application depending on `lapic` and `other`, with the given
    /// component descriptions attached.
    fn package_inputs(
        &self,
        descriptions: Vec<IndependentComponentDescription>,
    ) -> PackageCompilationInputs {
        PackageCompilationInputs::new(
            identity(ROOT),
            BuildDeclarationKind::Application,
            vec![
                self.roots.binding(ROOT, "root"),
                self.roots.binding(LAPIC, "lapic"),
                self.roots.binding(OTHER, "other"),
            ],
            vec![
                PackageDependencyBinding::new(identity(ROOT), "lapic", identity(LAPIC)),
                PackageDependencyBinding::new(identity(ROOT), "other", identity(OTHER)),
            ],
        )
        .expect("the fixture package graph is closed")
        .with_independent_component_descriptions(descriptions)
        .expect("every description names a dependency")
    }

    fn settle(
        self,
        descriptions: Vec<IndependentComponentDescription>,
    ) -> Result<CheckedProviderSelection, Vec<diagnostics::Diagnostic>> {
        let package_inputs = self.package_inputs(descriptions);
        let Self {
            mut typed,
            target_machines,
            selection,
            ..
        } = self;
        settle_checked_providers(
            &mut typed,
            target_machines,
            None,
            Some(&package_inputs),
            &[selection],
            &std::collections::BTreeSet::new(),
            &[],
            &[],
        )
    }
}

/// Attach one module's description for a dependency package the way the
/// dependency's own compilation would publish it: the encoded bytes beside
/// the subject that compilation observed.
fn attached(package: u8, module: &TerminalModule) -> IndependentComponentDescription {
    IndependentComponentDescription::new(
        identity(package),
        module_subject(module),
        describe_module(module),
    )
}

fn message_texts(diagnostics: &[diagnostics::Diagnostic]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect()
}

#[test]
fn independent_selection_settles_against_one_attached_verified_description() {
    let build = Build::new(CompositionMode::Independent);
    let plan = build.plan.clone();
    let description = attached(LAPIC, &build.realizing_module());

    let settled = build
        .settle(vec![description])
        .expect("one verified description realizing the plan settles the build");
    assert_eq!(
        settled.selected_provider_plan_facts.plans(),
        std::slice::from_ref(&plan)
    );
    assert_eq!(settled.provider_plans, vec![plan]);
}

#[test]
fn fused_selection_settles_without_any_description() {
    let build = Build::new(CompositionMode::Fused);
    let plan = build.plan.clone();

    let settled = build
        .settle(Vec::new())
        .expect("a fused selection consumes no component");
    assert_eq!(
        settled.selected_provider_plan_facts.plans(),
        std::slice::from_ref(&plan)
    );
}

#[test]
fn independent_selection_rejects_when_no_description_is_attached() {
    let build = Build::new(CompositionMode::Independent);

    let rejected = build
        .settle(Vec::new())
        .err()
        .expect("an independent edge without a verified component never settles");
    let messages = message_texts(&rejected);
    assert!(
        messages.iter().any(|message| {
            message.contains("retains independent composition")
                && message.contains("no verified component description was supplied")
                && message.contains("refusing to treat the edge as fused")
        }),
        "unexpected diagnostics: {messages:#?}"
    );
}

#[test]
fn independent_selection_rejects_several_realizing_descriptions() {
    let build = Build::new(CompositionMode::Independent);
    let module = build.realizing_module();

    let rejected = build
        .settle(vec![attached(LAPIC, &module), attached(OTHER, &module)])
        .err()
        .expect("two components realizing one plan do not name one deployment");
    let messages = message_texts(&rejected);
    assert!(
        messages.iter().any(|message| {
            message.contains("2 verified components realize it")
                && message.contains("exactly one component may deploy an independent edge")
        }),
        "unexpected diagnostics: {messages:#?}"
    );
}

#[test]
fn independent_selection_rejects_an_unmatched_extra_description() {
    let build = Build::new(CompositionMode::Independent);
    let realizing = attached(LAPIC, &build.realizing_module());
    let extra = attached(OTHER, &bare_module());

    let rejected = build
        .settle(vec![realizing, extra])
        .err()
        .expect("a supplied component that realizes no selected plan is not evidence");
    let messages = message_texts(&rejected);
    assert_eq!(
        messages.len(),
        1,
        "only the extra component rejects: {messages:#?}"
    );
    assert!(
        messages[0].contains("realizes no independently selected provider plan"),
        "unexpected diagnostics: {messages:#?}"
    );

    // A fused selection consumes no component either: attaching one leaves
    // it unmatched.
    let fused = Build::new(CompositionMode::Fused);
    let description = attached(LAPIC, &fused.realizing_module());
    let rejected = fused
        .settle(vec![description])
        .err()
        .expect("a fused edge leaves the attached component unmatched");
    assert!(
        message_texts(&rejected)
            .iter()
            .all(|message| message.contains("realizes no independently selected provider plan")),
        "unexpected diagnostics: {rejected:#?}"
    );
}

#[test]
fn independent_selection_rejects_a_mismatched_realization() {
    let build = Build::new(CompositionMode::Independent);
    let (requirement, _, machine) = build.realization();
    let description = attached(
        LAPIC,
        &provider_module(&requirement, "OtherProvider", &machine),
    );

    let rejected = build
        .settle(vec![description])
        .err()
        .expect("a component realizing another provider type does not close the edge");
    let messages = message_texts(&rejected);
    assert!(
        messages.iter().any(|message| {
            message.contains("no verified component realizes it")
                && message.contains("is not realized by selected provider")
                && message.contains("refusing to treat the edge as fused")
        }),
        "unexpected diagnostics: {messages:#?}"
    );
}

#[test]
fn attached_description_for_another_subject_rejects_before_the_join() {
    let build = Build::new(CompositionMode::Independent);
    let module = build.realizing_module();
    // The description bytes are the realizing module's, but the subject the
    // attachment claims was observed is another module's: a substituted
    // description must reject at verification, not join the plan.
    let substituted = IndependentComponentDescription::new(
        identity(LAPIC),
        module_subject(&bare_module()),
        describe_module(&module),
    );

    let rejected = build
        .settle(vec![substituted])
        .err()
        .expect("a description whose subject is not the observed one never verifies");
    let messages = message_texts(&rejected);
    assert_eq!(messages.len(), 1, "unexpected diagnostics: {messages:#?}");
    assert!(
        messages[0].contains("component description attached for dependency package `lapic`")
            && messages[0].contains("failed independent verification")
            && messages[0].contains("is not the admitted subject"),
        "unexpected diagnostics: {messages:#?}"
    );
}

#[test]
fn corrupt_attached_description_rejects_before_the_join() {
    let build = Build::new(CompositionMode::Independent);
    let module = build.realizing_module();
    let mut bytes = describe_module(&module);
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    let corrupt =
        IndependentComponentDescription::new(identity(LAPIC), module_subject(&module), bytes);

    let rejected = build
        .settle(vec![corrupt])
        .err()
        .expect("corrupt description bytes never verify");
    let messages = message_texts(&rejected);
    assert_eq!(messages.len(), 1, "unexpected diagnostics: {messages:#?}");
    assert!(
        messages[0].contains("component description attached for dependency package `lapic`")
            && messages[0].contains("failed independent verification"),
        "unexpected diagnostics: {messages:#?}"
    );
}

#[test]
fn package_inputs_reject_descriptions_outside_the_dependency_graph() {
    let build = Build::new(CompositionMode::Independent);
    let module = build.realizing_module();
    let inputs = PackageCompilationInputs::new(
        identity(ROOT),
        BuildDeclarationKind::Application,
        vec![
            build.roots.binding(ROOT, "root"),
            build.roots.binding(LAPIC, "lapic"),
        ],
        vec![PackageDependencyBinding::new(
            identity(ROOT),
            "lapic",
            identity(LAPIC),
        )],
    )
    .expect("closed package graph");

    let errors = inputs
        .clone()
        .with_independent_component_descriptions(vec![
            attached(ROOT, &module),
            attached(OTHER, &module),
            attached(LAPIC, &module),
            attached(LAPIC, &module),
        ])
        .expect_err("the root, a foreign package, and a duplicate all reject");
    let rendered = errors.iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(rendered.len(), 3, "unexpected errors: {rendered:#?}");
    assert!(
        rendered
            .iter()
            .any(|error| error.contains("cannot attach its own component description")),
        "unexpected errors: {rendered:#?}"
    );
    assert!(
        rendered
            .iter()
            .any(|error| error.contains("component description names foreign package")),
        "unexpected errors: {rendered:#?}"
    );
    assert!(
        rendered
            .iter()
            .any(|error| error.contains("has more than one component description")),
        "unexpected errors: {rendered:#?}"
    );

    // A valid attachment survives the source/target split and rejoin.
    let attached_inputs = inputs
        .with_independent_component_descriptions(vec![attached(LAPIC, &module)])
        .expect("one description for the dependency");
    let (source, target) = attached_inputs.clone().into_parts();
    let rejoined = PackageCompilationInputs::from_parts(source, target)
        .expect("target attachments rejoin their source graph");
    assert_eq!(rejoined, attached_inputs);
    assert_eq!(rejoined.independent_component_descriptions().count(), 1);
}
