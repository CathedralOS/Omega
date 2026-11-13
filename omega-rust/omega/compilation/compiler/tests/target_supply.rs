//! Target preparation and provider settlement decide from the accepted
//! Terminal module what a target must supply. Each test types a program from
//! source through the ordinary lexer, parser, resolver and typer, has the
//! lowered-form checker judge it, harvests the provider defaults it declares
//! and runs the two stages on that accepted module. The last tests carry the
//! settled supply on into the selected execution a product reads and check
//! the produced program's target-local conditions against it.

use build_time_evaluation::provider_declarations::{
    ProviderPlanningContext, ProviderSelection, TargetProviderDefaults, retain_provider_defaults,
};
use provider_planning::target_supply::{
    PreparedTargetSupply, ReachedBoundaryRequirement, SettledTargetSupply, SupplyAuthority,
    TargetSupplyRequest,
};
use psi_compiler::{AcceptedTerminalSource, TerminalSourceCompilation};
use std::sync::Arc;

const TARGET: &str = "linux_x86_64";

/// A boundary trait with one requirement and two data types that can be
/// named as its provider.
const SLOT: &str = "public boundary trait Service {\n    machine go() reaches none crashes none;\n}\ndata Native {}\ndata Other {}\n";
const NATIVE_DEFAULT: &str = "machine Native::provider_defaults(defaults: &mut Native) {\n    Native::select_provider<Service, Native>(defaults);\n}\n";
const OTHER_DEFAULT: &str = "machine Other::provider_defaults(defaults: &mut Other) {\n    Other::select_provider<Service, Other>(defaults);\n}\n";
const REACHES: &str = "machine main() {\n    Service::go();\n}\n";
const REACHES_NOTHING: &str = "machine main() {\n}\n";

fn package() -> semantic_vocabulary::PackageKeyIdentity {
    semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap()
}

/// The accepted module of `program`'s `main` and the defaults `supplied`
/// declares. `supplied` stands for the target package: it is a toolchain
/// source of the same compilation, so the checker judges the program's own
/// declarations and reads the boundary trait only where the program calls
/// it.
fn accepted(
    supplied: &str,
    program: &str,
) -> (AcceptedTerminalSource, Vec<TargetProviderDefaults>) {
    let mut sources = source::SourceMap::default();
    let mut syntax = None;
    for (path, text, package, origin) in [
        (
            "program/main.omg",
            program,
            package(),
            source::SourceOrigin::User,
        ),
        (
            "<target-package>",
            supplied,
            package(),
            source::SourceOrigin::Toolchain,
        ),
    ] {
        let id = sources
            .add_checked_instance(
                path.into(),
                text,
                "program".into(),
                Some(package),
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
                );
            }
            Some(syntax) => {
                tokens_to_syntax_trees::parse(syntax, id, &tokens).expect("parse source");
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
    .expect("harvest provider defaults");
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .expect("the program declares main")
        .symbol;
    let frozen: typed_trees::FrozenTypedTrees = typed.into();
    let accepted = AcceptedTerminalSource::judge(
        Arc::new(frozen),
        TerminalSourceCompilation {
            entries: &[entry],
            subjects: &[Some(package())],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
        },
        terminal_codec::TerminalPublication::Portable,
    )
    .unwrap_or_else(|refused| panic!("the route must accept the program: {refused:?}"));
    (accepted, defaults)
}

fn prepare(
    accepted: &AcceptedTerminalSource,
    build: &[ProviderSelection],
    defaults: &[TargetProviderDefaults],
) -> Result<PreparedTargetSupply, String> {
    let reached = accepted
        .boundary_requirements_reached()
        .expect("reached requirements rejoin their declarations")
        .into_iter()
        .map(|reached| ReachedBoundaryRequirement {
            boundary: reached.boundary,
            requirement: reached.requirement,
            slot: reached.slot,
            name: reached.name,
        })
        .collect::<Vec<_>>();
    PreparedTargetSupply::prepare(TargetSupplyRequest {
        module: accepted.module(),
        reached: &reached,
        build,
        defaults,
        target: TARGET,
    })
    .map_err(first)
}

fn settle(supplied: &str, program: &str) -> Result<SettledTargetSupply, String> {
    let (accepted, defaults) = accepted(supplied, program);
    prepare(&accepted, &[], &defaults)?.settle().map_err(first)
}

fn first(diagnostics: Vec<diagnostics::Diagnostic>) -> String {
    diagnostics[0].message.clone()
}

fn profile() -> target::TargetProfile {
    target::TargetProfile::from_canonical_target_name(TARGET).expect("a hosted target")
}

/// The Terminal artifact a product of the accepted source is checked on.
fn produced(accepted: &AcceptedTerminalSource) -> Arc<terminal_codec::CanonicalTerminalArtifact> {
    let program = accepted
        .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
        .expect("the accepted source publishes")
        .into_shared_portable()
        .expect("the publication is portable");
    Arc::clone(program.artifact())
}

/// The target-local conditions of `artifact`, checked against `proposal`.
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

/// A program that reaches no boundary requirement settles a selected
/// execution with no plan for its target, and the program's target-local
/// conditions check against the proposal formed from it: nothing but the
/// settled supply and the produced artifact is needed to check the program.
#[test]
fn a_module_that_reaches_nothing_checks_against_its_settled_execution() {
    let (accepted, defaults) = accepted(&format!("{SLOT}{NATIVE_DEFAULT}"), REACHES_NOTHING);
    let settled = prepare(&accepted, &[], &defaults)
        .unwrap()
        .settle()
        .unwrap();
    let execution = selected_dispatch::SelectedExecution::settle(Some(profile()), &settled)
        .expect("a supply that demands nothing settles");
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

    // The proposal belongs to the artifact it was formed from: the same
    // settled execution proposes nothing for another program's artifact.
    let (other, _) = accepted_other();
    assert_eq!(
        check_conditions(&produced(&other), &proposal),
        Err("scalar application roots belong to another artifact".to_owned())
    );
}

/// A second program with a different accepted module.
fn accepted_other() -> (AcceptedTerminalSource, Vec<TargetProviderDefaults>) {
    accepted(
        &format!("{SLOT}{NATIVE_DEFAULT}"),
        "machine helper() {\n}\nmachine main() {\n    helper();\n}\n",
    )
}

/// The twin reaches the requirement, so its supply is chosen and no selected
/// execution can be formed for the target: the only constructor refuses with
/// the plan that cannot be formed, and no product can read a custody that
/// was never settled.
#[test]
fn a_reached_requirement_forms_no_selected_execution_without_its_plan() {
    let (accepted, defaults) = accepted(&format!("{SLOT}{NATIVE_DEFAULT}"), REACHES);
    let settled = prepare(&accepted, &[], &defaults)
        .unwrap()
        .settle()
        .unwrap();
    assert_eq!(
        selected_dispatch::SelectedExecution::settle(Some(profile()), &settled)
            .map(|execution| execution.plans().plans().len())
            .map_err(first),
        Err(
            "no product route yet: provider settlement for `linux_x86_64`: `Service::go` is supplied by `Native`, and no provider or calling plan can be formed for a selected provider on this route"
                .to_owned()
        )
    );
}

/// A program whose entry reaches no boundary requirement demands nothing:
/// preparation finds no requirement on its module even though a default is
/// declared, and settlement forms no plan.
#[test]
fn a_module_that_reaches_no_boundary_requirement_has_nothing_to_settle() {
    let (accepted, defaults) = accepted(&format!("{SLOT}{NATIVE_DEFAULT}"), REACHES_NOTHING);
    assert_eq!(defaults.len(), 1, "the default is declared");
    assert!(accepted.module().boundary_machines.is_empty());
    let prepared = prepare(&accepted, &[], &defaults).unwrap();
    assert!(prepared.demands_nothing());
    let settled = prepared.settle().unwrap();
    assert_eq!(settled.plans(), Ok(Vec::new()));
}

/// The twin reaches the requirement. Preparation pairs it with the declared
/// default, settlement chooses that default, and the plan of the chosen
/// provider is refused by the requirement's and the provider's names.
#[test]
fn a_reached_requirement_settles_to_its_one_declared_default() {
    let (accepted, defaults) = accepted(&format!("{SLOT}{NATIVE_DEFAULT}"), REACHES);
    let prepared = prepare(&accepted, &[], &defaults).unwrap();
    let [requirement] = prepared.requirements() else {
        panic!("one requirement is reached: {prepared:?}");
    };
    assert_eq!(requirement.reached.name, "Service::go");
    assert_eq!(requirement.supplies.len(), 1);
    assert_eq!(
        requirement.supplies[0].authority,
        SupplyAuthority::TargetDefault
    );
    let settled = prepared.settle().unwrap();
    assert_eq!(
        settled.requirements()[0]
            .supply
            .selection
            .provider_type
            .authored_path,
        "Native"
    );
    assert_eq!(
        settled.plans().map_err(first),
        Err(
            "no product route yet: provider settlement for `linux_x86_64`: `Service::go` is supplied by `Native`, and no provider or calling plan can be formed for a selected provider on this route"
                .to_owned()
        )
    );
}

/// A reached requirement nobody supplies rejects the program, naming the
/// requirement and the target.
#[test]
fn a_reached_requirement_with_no_provider_is_rejected() {
    assert_eq!(
        settle(SLOT, REACHES).unwrap_err(),
        "boundary requirement `Service::go` has no provider on target `linux_x86_64`: the Build selects none and the target package declares no default"
    );
}

/// Two defaults for one slot on one target conflict; neither is chosen by
/// the order they were declared in.
#[test]
fn a_reached_requirement_with_two_defaults_is_rejected_as_a_conflict() {
    assert_eq!(
        settle(&format!("{SLOT}{NATIVE_DEFAULT}{OTHER_DEFAULT}"), REACHES).unwrap_err(),
        "boundary requirement `Service::go` has conflicting providers on target `linux_x86_64`: the target package declares `Native` and `Other`"
    );
}

/// A default keyed to another target is no supply for this one, and a Build
/// selection outranks the target's default instead of conflicting with it.
/// A selection is one value whichever authority declared it, so the Build's
/// here is the selection the second default states.
#[test]
fn supply_is_ranked_by_authority_and_keyed_by_target() {
    let (accepted, mut defaults) =
        accepted(&format!("{SLOT}{NATIVE_DEFAULT}{OTHER_DEFAULT}"), REACHES);
    assert_eq!(defaults.len(), 1, "untargeted defaults share one row");
    let other = defaults[0].selections.pop().expect("the second default");
    assert_eq!(other.provider_type.authored_path, "Other");
    let settled = prepare(&accepted, std::slice::from_ref(&other), &defaults)
        .unwrap()
        .settle()
        .unwrap();
    assert_eq!(
        settled.requirements()[0].supply.authority,
        SupplyAuthority::Build
    );
    assert_eq!(
        settled.requirements()[0]
            .supply
            .selection
            .provider_type
            .authored_path,
        "Other"
    );

    defaults[0].target = Some("windows_x86_64".to_owned());
    assert_eq!(
        prepare(&accepted, &[], &defaults)
            .unwrap()
            .settle()
            .map_err(first)
            .unwrap_err(),
        "boundary requirement `Service::go` has no provider on target `linux_x86_64`: the Build selects none and the target package declares no default"
    );
}
