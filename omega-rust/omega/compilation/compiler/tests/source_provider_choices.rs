//! Provider policy is normalized while its actual source owner is available.
//! These controls keep the original missing/default/conflict/override/target
//! oracles, then exercise the canonical plans and artifact-bound proposal.

use source_compiler::evaluation::provider_declarations::{
    ProviderPlanningContext, ProviderSelection, TargetProviderDefaults, retain_provider_defaults,
};
use source_compiler::source_provider_choices::{
    PreparedProviderChoices, prepare_with_source_defaults,
};
use source_compiler::{AcceptedTerminalSource, TerminalSourceCompilation};
use std::sync::Arc;

const TARGET: &str = "linux_x86_64";
const SLOT: &str = "public boundary trait Service {\n    machine go() reaches none crashes none;\n}\ndata Native {}\ndata Other {}\n";
const ORIGINAL_NATIVE_DEFAULT: &str = "machine Native::provider_defaults(defaults: @mut Native) {\n    Native::select_provider<Service, Native>(defaults);\n}\n";

// A positive provider now supplies its real checked operations. Separate data
// stores the default declaration; a data type is not a whole provider value.
const NAMED_SLOT: &str = "public boundary trait Service {\n    machine go() reaches none crashes none;\n}\npublic Native satisfies Service {\n    machine go() reaches none crashes none {}\n}\npublic Other satisfies Service {\n    machine go() reaches none crashes none {}\n}\ndata NativeDefaults {}\ndata OtherDefaults {}\n";
const NATIVE_DEFAULT: &str = "machine NativeDefaults::provider_defaults(defaults: @mut NativeDefaults) {\n    NativeDefaults::select_provider<Service, Native>(defaults);\n}\n";
const OTHER_DEFAULT: &str = "machine OtherDefaults::provider_defaults(defaults: @mut OtherDefaults) {\n    OtherDefaults::select_provider<Service, Other>(defaults);\n}\n";
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
    source: Arc<source_compiler::source_state::FrozenTypedTrees>,
    entry: symbols::SymbolHandle,
    defaults: Vec<TargetProviderDefaults>,
}

/// Use the ordinary source pipeline and the original package/source custody.
/// Defaults are harvested before judgment, never recovered from a publication.
fn typed(supplied: &str, program: &str) -> Fixture {
    try_typed(supplied, program).expect("harvest source defaults")
}

fn try_typed(supplied: &str, program: &str) -> Result<Fixture, Vec<diagnostics::Diagnostic>> {
    let mut sources = source_compiler::source_inputs::SourceMap::default();
    let mut syntax = None;
    for (path, text, origin) in [
        (
            "program/main.omg",
            program,
            source_admission::SourceOrigin::User,
        ),
        (
            "<target-package>",
            supplied,
            source_admission::SourceOrigin::Toolchain,
        ),
    ] {
        let id = sources
            .add_checked_instance(
                path.into(),
                text,
                "program".into(),
                Some(package()),
                origin,
                source_admission::SourceResolutionStratum::Base,
                declaration_contracts::source_identity::DependencyScope::Product,
            )
            .source_id;
        let tokens = crate::frontend_fixture::Lexer::new(text)
            .tokenize()
            .expect("lex source");
        match &mut syntax {
            None => {
                syntax = Some(
                    crate::frontend_fixture::parse_syntax_trees_with_id(id, &tokens)
                        .expect("parse source"),
                )
            }
            Some(syntax) => {
                crate::frontend_fixture::parse(syntax, id, &tokens).expect("parse source");
            }
        }
    }
    let mut request = source_compiler::resolution::ResolutionRequest::new(syntax.unwrap());
    request.sources = Some(Arc::new(sources));
    let resolved = source_compiler::resolution::resolve(request).expect("resolve source");
    let typed =
        source_compiler::typing::lower_symbol_resolved_trees(&resolved).expect("type source");
    let defaults = retain_provider_defaults(ProviderPlanningContext::new(
        &typed,
        declaration_contracts::source_identity::DependencyScope::Product,
    ))?;
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .expect("source main")
        .symbol;
    Ok(Fixture {
        source: Arc::new(typed.into()),
        entry,
        defaults,
    })
}

fn prepare(
    fixture: &Fixture,
    build: &[ProviderSelection],
    defaults: Vec<TargetProviderDefaults>,
) -> Result<PreparedProviderChoices, String> {
    prepare_with_source_defaults(&fixture.source, &[TARGET], build, &[], &defaults).map_err(first)
}

fn judge(fixture: &Fixture, choices: &PreparedProviderChoices) -> AcceptedTerminalSource {
    AcceptedTerminalSource::judge(
        Arc::clone(&fixture.source),
        TerminalSourceCompilation {
            declaration_metadata: &[],
            supplies: &[],

            entries: &[fixture.entry],
            subjects: &[Some(package())],
            purpose: declaration_contracts::source_identity::DependencyScope::Product,
            selected: &choices.declaration_roots,
            calling: Some(source_compiler::lowering::CallingPolicySelections {
                source: &fixture.source,
                applications: &choices.calling_applications,
                entries: &[],
            }),
            providers: Some(
                source_compiler::lowering::provider_selections::ProviderSelections {
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
fn fixture_inputs(
    module: &terminal_psi::TerminalModule,
) -> provider_realization::target_supply::TargetSupplyExecutionInputs {
    assert!(module.external_providers.is_empty());
    assert!(module.calling_policy_requests.is_empty());
    provider_realization::target_supply::TargetSupplyExecutionInputs {
        calling_applications: Vec::new(),
        external_bindings: Vec::new(),
        evaluated_native_bindings: Vec::new(),
        evaluated_binding_identities: Default::default(),
        terminal_authority_permissions: Vec::new(),
        terminal_authority_permission_identities: Default::default(),
    }
}

fn execution(
    accepted: &AcceptedTerminalSource,
) -> Result<provider_realization::SelectedExecution, String> {
    let module = accepted.module();
    let entry = accepted
        .entry()
        .expect("fixture main is an executable entry");
    let reached = accepted
        .boundary_calls_reached(entry, &[], Some(TARGET))
        .map_err(|error| error.message)?;
    let closed = accepted
        .close_logical_provider_reach(TARGET, Some(entry), &reached)
        .map_err(first)?;
    provider_realization::plan_target_execution(
        module,
        profile(),
        closed.selection(),
        closed.bindings(),
        fixture_inputs(module),
    )
    .map_err(|error| match error {
        provider_realization::NativeProgramPlanningError::Supply(error) => error.diagnostic(module),
        provider_realization::NativeProgramPlanningError::Admission(errors) => first(errors),
    })
}

fn produced(accepted: &AcceptedTerminalSource) -> Arc<terminal_codec::CanonicalTerminalArtifact> {
    let program = accepted
        .produce(
            Default::default(),
            &mut source_compiler::UnmeasuredProduction,
        )
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
    let proposal = execution
        .provider_proposal(
            artifact.view(),
            optimization_core::OptimizationSelections::default().identity(),
        )
        .expect("the selected target has an actual provider proposal");
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
fn a_data_type_cannot_supply_a_provider_default() {
    // Preserve the original data-only provider as the negative. A data type
    // has no complete named operation record, so no checked plan can form.
    let refusal = try_typed(&format!("{SLOT}{ORIGINAL_NATIVE_DEFAULT}"), REACHES)
        .err()
        .expect("bodyless data supplies no provider record");
    assert_eq!(refusal.len(), 1);
    assert_eq!(
        refusal[0].message,
        "provider selection `Native` is not a static operation record from a named implementation; a data type is not a provider value"
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
            .boundary_calls_reached(accepted.entry().unwrap(), &[], Some(TARGET))
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
    let [declaration] = declarations else {
        panic!("one exact native declaration")
    };
    let candidates =
        backend_plan::selected_provider_candidate_rows(declaration, accepted.module()).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].candidate_positions.len(), 1);
    let artifact = produced(&accepted);
    let proposal = execution
        .provider_proposal(
            artifact.view(),
            optimization_core::OptimizationSelections::default().identity(),
        )
        .expect("the selected target has an actual provider proposal");
    assert_eq!(check_conditions(&artifact, &proposal), Ok(()));
    // Later native work cannot reinterpret a settlement for another canonical
    // program, even when the same provider and boundary coordinates still exist.
    let closed = accepted
        .close_logical_provider_reach(TARGET, accepted.entry(), &[])
        .unwrap();
    assert_eq!(closed.selection().program(), artifact.view().identity());
    let mut changed = accepted.module().clone();
    changed.provider_selections.clear();
    use provider_realization::{NativeProgramPlanningError, plan_target_execution};
    let Err(NativeProgramPlanningError::Supply(foreign_program)) = plan_target_execution(
        &changed,
        profile(),
        closed.selection(),
        closed.bindings(),
        fixture_inputs(&changed),
    ) else {
        panic!("a selection from another canonical program must refuse");
    };
    assert_eq!(
        foreign_program.diagnostic(&changed),
        "logical provider choices belong to a different accepted program"
    );
    assert!(!closed.bindings().is_empty());
    let Err(NativeProgramPlanningError::Admission(changed_body)) = plan_target_execution(
        accepted.module(),
        profile(),
        closed.selection(),
        &[],
        fixture_inputs(accepted.module()),
    ) else {
        panic!("a different closed body roster must refuse");
    };
    assert_eq!(
        first(changed_body),
        "native planning changed a closed logical provider body"
    );
    let changed_identity = terminal_codec::terminal_psi_identity(&changed).unwrap();
    assert_eq!(
        execution.realization().require_program(changed_identity),
        Err("provider realization belongs to a different accepted program".to_owned())
    );
}

#[test]
fn a_reached_requirement_with_no_provider_is_rejected() {
    let fixture = typed(SLOT, REACHES);
    let choices = prepare(&fixture, &[], fixture.defaults.clone()).unwrap();
    let accepted = judge(&fixture, &choices);
    let refusal = execution(&accepted).expect_err("a reached requirement needs chosen supply");
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

#[path = "../../../../../tests/support/frontend.rs"]
mod frontend_fixture;
