//! Provider policy is normalized while its actual source owner is available.
//! These controls keep the original missing/default/conflict/override/target
//! oracles, then exercise the canonical plans and artifact-bound proposal.

use super::{PreparedProviderChoices, prepare_with_source_defaults};
use build_time_evaluation::provider_declarations::{
    ProviderPlanningContext, ProviderSelection, TargetProviderDefaults, retain_provider_defaults,
};
use psi_compiler::{AcceptedTerminalSource, TerminalSourceCompilation};
use std::sync::Arc;

const TARGET: &str = "linux_x86_64";
const SLOT: &str = "public boundary trait Service {\n    machine go() reaches none crashes none;\n}\ndata Native {}\ndata Other {}\n";
const ORIGINAL_NATIVE_DEFAULT: &str = "machine Native::provider_defaults(defaults: &mut Native) {\n    Native::select_provider<Service, Native>(defaults);\n}\n";

// A positive provider now supplies its real checked operations. Separate data
// stores the default declaration; a data type is not a whole provider value.
const NAMED_SLOT: &str = "public boundary trait Service {\n    machine go() reaches none crashes none;\n}\npublic Native satisfies Service {\n    machine go() reaches none crashes none {}\n}\npublic Other satisfies Service {\n    machine go() reaches none crashes none {}\n}\ndata NativeDefaults {}\ndata OtherDefaults {}\n";
const NATIVE_DEFAULT: &str = "machine NativeDefaults::provider_defaults(defaults: &mut NativeDefaults) {\n    NativeDefaults::select_provider<Service, Native>(defaults);\n}\n";
const OTHER_DEFAULT: &str = "machine OtherDefaults::provider_defaults(defaults: &mut OtherDefaults) {\n    OtherDefaults::select_provider<Service, Other>(defaults);\n}\n";
const REACHES: &str = "machine main() {\n    Service::go();\n}\n";
const REACHES_NOTHING: &str = "machine main() {\n}\n";

fn package() -> semantic_vocabulary::PackageKeyIdentity {
    semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap()
}
fn profile() -> target::TargetProfile {
    target::TargetProfile::from_canonical_target_name(TARGET).expect("a hosted target")
}
fn first(diagnostics: Vec<diagnostics::Diagnostic>) -> String {
    diagnostics[0].message.clone()
}

struct Fixture {
    source: Arc<typed_trees::FrozenTypedTrees>,
    entry: symbols::SymbolHandle,
    defaults: Vec<TargetProviderDefaults>,
}

/// Use the ordinary source pipeline and the original package/source custody.
/// Defaults are harvested before judgment, never recovered from a publication.
fn typed(supplied: &str, program: &str) -> Fixture {
    let mut sources = source::SourceMap::default();
    let mut syntax = None;
    for (path, text, origin) in [
        ("program/main.omg", program, source::SourceOrigin::User),
        (
            "<target-package>",
            supplied,
            source::SourceOrigin::Toolchain,
        ),
    ] {
        let id = sources
            .add_checked_instance(
                path.into(),
                text,
                "program".into(),
                Some(package()),
                origin,
                source::SourceResolutionStratum::Base,
                source::DependencyScope::Product,
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(text)
            .tokenize()
            .expect("lex source");
        match &mut syntax {
            None => {
                syntax = Some(
                    tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens)
                        .expect("parse source"),
                )
            }
            Some(syntax) => {
                tokens_to_syntax_trees::parse(syntax, id, &tokens).expect("parse source")
            }
        }
    }
    let mut request =
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax.unwrap());
    request.sources = Some(Arc::new(sources));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).expect("resolve source");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type source");
    let defaults = retain_provider_defaults(ProviderPlanningContext::new(
        &typed,
        source::DependencyScope::Product,
    ))
    .expect("harvest source defaults");
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .expect("source main")
        .symbol;
    Fixture {
        source: Arc::new(typed.into()),
        entry,
        defaults,
    }
}

fn prepare(
    fixture: &Fixture,
    build: &[ProviderSelection],
    defaults: Vec<TargetProviderDefaults>,
) -> Result<PreparedProviderChoices, String> {
    prepare_with_source_defaults(&fixture.source, &[profile()], build, &[], defaults).map_err(first)
}

fn judge(fixture: &Fixture, choices: &PreparedProviderChoices) -> AcceptedTerminalSource {
    AcceptedTerminalSource::judge(
        Arc::clone(&fixture.source),
        TerminalSourceCompilation {
            entries: &[fixture.entry],
            subjects: &[Some(package())],
            purpose: source::DependencyScope::Product,
            selected: &choices.declaration_roots,
            calling: Some(structure_elaboration::CallingPolicySelections {
                source: &fixture.source,
                applications: &choices.calling_applications,
                entries: &[],
            }),
            providers: Some(
                structure_elaboration::provider_selections::ProviderSelections {
                    source: &fixture.source,
                    choices: &choices.selections,
                },
            ),
            bindings: &[],
        },
        terminal_codec::TerminalPublication::Portable,
    )
    .unwrap_or_else(|refused| panic!("the source route must accept: {refused:?}"))
}

/// These source fixtures declare no native leaves, calling policies or package
/// authority grants. Their empty auxiliary rosters are observed properties of
/// the accepted program; the service map and its checked bodies remain real.
fn execution(
    accepted: &AcceptedTerminalSource,
) -> Result<selected_dispatch::SelectedExecution, String> {
    use provider_planning::target_supply::{
        TargetSupplyExecutionInputs, TargetSupplyRealizationRequest, realize_target_supply,
    };
    let module = accepted.module();
    assert!(module.external_providers.is_empty());
    assert!(module.calling_policy_requests.is_empty());
    let entry = accepted
        .entry()
        .expect("fixture main is an executable entry");
    let reached = accepted
        .boundary_calls_reached(entry, &[])
        .map_err(|error| error.message)?;
    let realized = realize_target_supply(TargetSupplyRealizationRequest::from_execution_inputs(
        module,
        TARGET,
        &reached,
        TargetSupplyExecutionInputs {
            calling_applications: Vec::new(),
            external_bindings: Vec::new(),
            evaluated_native_bindings: Vec::new(),
            evaluated_binding_identities: terminal_psi::TerminalIdentityTable::default(),
            terminal_authority_permissions: Vec::new(),
            terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable::default(
            ),
        },
    ))?;
    selected_dispatch::SelectedExecution::from_realization(Some(profile()), realized).map_err(first)
}

fn produced(accepted: &AcceptedTerminalSource) -> Arc<terminal_codec::CanonicalTerminalArtifact> {
    let program = accepted
        .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
        .expect("accepted source publishes")
        .into_shared_portable()
        .expect("portable publication");
    Arc::clone(program.artifact())
}
fn check_conditions(
    artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
    proposal: &backend_plan::TerminalTargetProviderProposal,
) -> Result<(), String> {
    build_settings::scalar_completion::complete(
        artifact,
        TARGET,
        &[],
        std::slice::from_ref(proposal),
        &mut Default::default(),
    )
    .map(drop)
}
fn selected_provider(fixture: &Fixture, choices: &PreparedProviderChoices, name: &str) {
    let [choice] = choices.selections.as_slice() else {
        panic!("one exact chosen provider")
    };
    let declared = fixture
        .source
        .conformances()
        .iter()
        .find(|declaration| fixture.source.symbols.name(declaration.symbol) == name)
        .expect("the source declares this named map");
    assert_eq!(
        choice.provider,
        fixture.source.implementation_value_head(declared.symbol)
    );
    assert_eq!(choice.target, TARGET);
}

#[test]
fn a_module_that_reaches_nothing_checks_against_its_settled_execution() {
    let fixture = typed(&format!("{NAMED_SLOT}{NATIVE_DEFAULT}"), REACHES_NOTHING);
    let choices = prepare(&fixture, &[], fixture.defaults.clone()).unwrap();
    let accepted = judge(&fixture, &choices);
    let execution = execution(&accepted).expect("no executable provider demand");
    assert_eq!(execution.target_profile(), Some(profile()));
    assert!(execution.plans().plans().is_empty());
    let artifact = produced(&accepted);
    let proposal = execution.provider_proposal(
        artifact.view(),
        optimization_core::OptimizationSelections::default().identity(),
    );
    assert_eq!(proposal.target, TARGET);
    assert_eq!(
        proposal
            .selection
            .as_ref()
            .map(|selection| selection.plans.len()),
        Ok(0)
    );
    assert_eq!(check_conditions(&artifact, &proposal), Ok(()));

    let other = typed(
        &format!("{NAMED_SLOT}{NATIVE_DEFAULT}"),
        "machine helper() {\n}\nmachine main() {\n    helper();\n}\n",
    );
    let other_choices = prepare(&other, &[], other.defaults.clone()).unwrap();
    let other = judge(&other, &other_choices);
    assert_eq!(
        check_conditions(&produced(&other), &proposal),
        Err("scalar application roots belong to another artifact".to_owned())
    );
}

#[test]
fn a_reached_requirement_forms_no_selected_execution_without_its_plan() {
    // Preserve the original data-only provider as the negative. A data type
    // has no complete named operation record, so no checked plan can form.
    let fixture = typed(&format!("{SLOT}{ORIGINAL_NATIVE_DEFAULT}"), REACHES);
    let refusal = prepare(&fixture, &[], fixture.defaults.clone())
        .err()
        .expect("bodyless data supplies no provider record");
    assert_eq!(
        refusal,
        "provider position is not a complete named implementation record"
    );
}

#[test]
fn a_module_that_reaches_no_boundary_requirement_has_nothing_to_settle() {
    let fixture = typed(&format!("{NAMED_SLOT}{NATIVE_DEFAULT}"), REACHES_NOTHING);
    assert_eq!(fixture.defaults.len(), 1);
    let choices = prepare(&fixture, &[], fixture.defaults.clone()).unwrap();
    selected_provider(&fixture, &choices, "Native");
    let accepted = judge(&fixture, &choices);
    assert!(
        !accepted.module().provider_candidates.is_empty(),
        "retained conformance is real"
    );
    assert!(
        accepted
            .boundary_calls_reached(accepted.entry().unwrap(), &[])
            .unwrap()
            .is_empty()
    );
    assert!(execution(&accepted).unwrap().plans().plans().is_empty());
}

#[test]
fn a_reached_requirement_settles_to_its_one_declared_default() {
    let fixture = typed(&format!("{NAMED_SLOT}{NATIVE_DEFAULT}"), REACHES);
    let choices = prepare(&fixture, &[], fixture.defaults.clone()).unwrap();
    selected_provider(&fixture, &choices, "Native");
    let accepted = judge(&fixture, &choices);
    let execution = execution(&accepted).expect("the complete checked provider map forms its plan");
    let [plan] = execution.plans().plans() else {
        panic!("one reached provider plan")
    };
    let [choice] = accepted.module().provider_selections.as_slice() else {
        panic!("one canonical source choice")
    };
    assert_eq!(
        accepted.module().identities.text(choice.provider_identity),
        Some(plan.provider_type.as_str())
    );
    let declarations = execution.realization().provider_declarations();
    let [declaration] = declarations.as_slice() else {
        panic!("one exact native declaration")
    };
    let candidates = backend_plan::selected_provider_candidate_rows(
        plan,
        declaration.provider_declaration.as_deref().unwrap(),
        accepted.module(),
    )
    .unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].candidate_positions.len(), 1);
    let artifact = produced(&accepted);
    let proposal = execution.provider_proposal(
        artifact.view(),
        optimization_core::OptimizationSelections::default().identity(),
    );
    assert_eq!(check_conditions(&artifact, &proposal), Ok(()));
}

#[test]
fn a_reached_requirement_with_no_provider_is_rejected() {
    let fixture = typed(SLOT, REACHES);
    let choices = prepare(&fixture, &[], fixture.defaults.clone()).unwrap();
    let accepted = judge(&fixture, &choices);
    let refusal = execution(&accepted)
        .err()
        .expect("a reached requirement needs chosen supply");
    assert!(
        refusal.contains("no normalized chosen provider"),
        "{refusal}"
    );
}

#[test]
fn a_reached_requirement_with_two_defaults_is_rejected_as_a_conflict() {
    let fixture = typed(
        &format!("{NAMED_SLOT}{NATIVE_DEFAULT}{OTHER_DEFAULT}"),
        REACHES,
    );
    let refusal = prepare(&fixture, &[], fixture.defaults.clone())
        .err()
        .expect("source defaults conflict");
    assert!(refusal.contains("ConflictingDefaults"), "{refusal}");
    assert!(refusal.contains(TARGET), "{refusal}");
}

#[test]
fn supply_is_ranked_by_authority_and_keyed_by_target() {
    let fixture = typed(
        &format!("{NAMED_SLOT}{NATIVE_DEFAULT}{OTHER_DEFAULT}"),
        REACHES,
    );
    let mut defaults = fixture.defaults.clone();
    assert_eq!(defaults.len(), 1);
    let other = defaults[0].selections.pop().expect("the second default");
    assert_eq!(other.provider_type.authored_path, "Other");
    let choices = prepare(&fixture, std::slice::from_ref(&other), defaults.clone()).unwrap();
    selected_provider(&fixture, &choices, "Other");
    let accepted = judge(&fixture, &choices);
    assert_eq!(execution(&accepted).unwrap().plans().plans().len(), 1);

    defaults[0].target = Some("windows_x86_64".to_owned());
    let choices = prepare(&fixture, &[], defaults).unwrap();
    assert!(
        choices.selections.is_empty(),
        "another target's default supplies no Linux row"
    );
    let accepted = judge(&fixture, &choices);
    assert!(
        execution(&accepted).is_err(),
        "reached Linux requirement remains unsupplied"
    );
}
