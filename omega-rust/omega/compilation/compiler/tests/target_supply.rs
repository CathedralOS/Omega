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
use psi_compiler::AcceptedTerminalSource;
use std::sync::Arc;

const TARGET: &str = "linux_x86_64";

/// A boundary trait with one requirement, and the data types whose
/// `provider_defaults` machines declare a target's defaults. A provider is
/// a named implementation, never a data type (#3002).
const SLOT: &str = "public boundary trait Service {\n    machine go() reaches none crashes none;\n}\ndata NativeDefaults {}\ndata OtherDefaults {}\n";
/// Two named implementations of `Service`, each a provider a default can
/// name.
const PROVIDERS: &str = "Native satisfies Service {\n    machine go() reaches none crashes none {\n    }\n}\nOther satisfies Service {\n    machine go() reaches none crashes none {\n    }\n}\n";
const NATIVE_DEFAULT: &str = "machine NativeDefaults::provider_defaults(defaults: &mut NativeDefaults) {\n    NativeDefaults::select_provider<Service, Native>(defaults);\n}\n";
const OTHER_DEFAULT: &str = "machine OtherDefaults::provider_defaults(defaults: &mut OtherDefaults) {\n    OtherDefaults::select_provider<Service, Other>(defaults);\n}\n";
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
) -> (Arc<AcceptedTerminalSource>, Vec<TargetProviderDefaults>) {
    let (source, defaults, roster) = judged(supplied, program);
    (
        roster.judge(source).expect("the exact roster accepts"),
        defaults,
    )
}

/// The same, with the roster the judgment accepted.
fn judged(
    supplied: &str,
    program: &str,
) -> (
    Arc<typed_trees::FrozenTypedTrees>,
    Vec<TargetProviderDefaults>,
    compilation_products::ProductRoster,
) {
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
    let roster = compilation_products::ProductRoster {
        entries: vec![entry],
        subject: Some(package()),
        ..Default::default()
    };
    (Arc::new(frozen), defaults, roster)
}

/// What a product of one target reads after its source is checked.
#[derive(Debug)]
struct Supplied {
    supply: PreparedTargetSupply,
    roster: compilation_products::ProductRoster,
    publication: Arc<compilation_products::ProductTerminalPublication>,
    rejudged: bool,
}

impl Supplied {
    fn accepted(&self) -> Option<&AcceptedTerminalSource> {
        self.rejudged.then(|| {
            self.publication
                .accepted()
                .expect("the actual publication accepted")
        })
    }
}

/// Target preparation for `program` as a product runs it: every provider a
/// target chooses is judged as a root of the Product's one module, and the
/// demand of `TARGET` is walked through the providers it chooses.
fn supplied_target(supplied: &str, program: &str) -> Result<Supplied, String> {
    supplied_on(supplied, program, TARGET, |_| {})
}

/// The same for `target`, after `key` has keyed the harvested defaults to
/// targets: the sources of these tests declare untargeted defaults only.
fn supplied_on(
    supplied: &str,
    program: &str,
    target: &str,
    key: impl FnOnce(&mut Vec<TargetProviderDefaults>),
) -> Result<Supplied, String> {
    let (source, mut defaults, roster) = judged(supplied, program);
    key(&mut defaults);
    let original = Arc::new(
        compilation_products::ProductTerminalPublication::judge(
            source,
            roster,
            optimization_core::OptimizationSelections::default(),
        )
        .map_err(first)?,
    );
    let product = compiler::judge_supplied_product(compiler::ProductSupplySource {
        publication: Arc::clone(&original),
        build: &[],
        defaults: &defaults,
    })
    .map_err(first)?;
    let supply = compiler::prepare_supplied_target(compiler::TargetSupplySource {
        publication: &product,
        build: &[],
        defaults: &defaults,
        target: Some(target),
    })
    .map_err(first)?;
    let rejudged = !Arc::ptr_eq(&original, &product);
    Ok(Supplied {
        supply,
        roster: product.roster().map_err(first)?.clone(),
        publication: product,
        rejudged,
    })
}

/// An accepted module cannot authenticate an extra supply that its roster
/// never asked the checker to judge.
#[test]
fn a_publication_judges_its_exact_roster_including_predeclared_supplies() {
    let declarations = format!(
        "{SLOT}Native satisfies Service {{ machine go() reaches none crashes Trap {{ }} }}{NATIVE_DEFAULT}"
    );
    let (source, defaults, mut roster) = judged(&declarations, REACHES);
    let original = compilation_products::ProductTerminalPublication::judge(
        Arc::clone(&source),
        roster.clone(),
        optimization_core::OptimizationSelections::default(),
    )
    .unwrap();
    let accepted = original.accepted().unwrap();
    assert_eq!(accepted.module().boundary_machines.len(), 1);
    assert_eq!(accepted.module().machines.len(), 1);
    let (_, requirement) = source
        .trait_machine_signatures
        .iter()
        .find(|(_, row)| row.name.as_str() == "go")
        .unwrap();
    let native = source
        .conformances()
        .iter()
        .find(|row| {
            row.alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "Native")
        })
        .unwrap();
    roster.supplies.push(psi_compiler::TerminalSourceSupply {
        slot: requirement.symbol,
        provider: native.symbol,
    });
    let error = compilation_products::ProductTerminalPublication::judge(
        source,
        roster,
        optimization_core::OptimizationSelections::default(),
    )
    .unwrap_err();
    let message = diagnostics::join_messages(&error);
    assert!(message.contains("crash"), "{message}");
    // The global queue must mint B's own publication too: A cannot stand in
    // for the newly chosen provider's body and refinement obligations.
    let error = compiler::judge_supplied_product(compiler::ProductSupplySource {
        publication: Arc::new(original),
        build: &[],
        defaults: &defaults,
    })
    .unwrap_err();
    let message = diagnostics::join_messages(&error);
    assert!(message.contains("crash"), "{message}");
}

/// A second boundary trait whose provider reaches a third.
const DEEPER: &str = "public boundary trait Deeper {\n    machine down() reaches none crashes none;\n}\npublic boundary trait Chained {\n    machine go() reaches Deeper crashes none;\n}\nChaining satisfies Chained {\n    machine go() reaches Deeper crashes none {\n        Deeper::down();\n    }\n}\nBottom satisfies Deeper {\n    machine down() reaches none crashes none {\n    }\n}\nmachine NativeDefaults::provider_defaults(defaults: &mut NativeDefaults) {\n    NativeDefaults::select_provider<Chained, Chaining>(defaults);\n    NativeDefaults::select_provider<Deeper, Bottom>(defaults);\n}\n";

/// The chosen provider of a reached requirement becomes a root of the same
/// compilation: the module judged for the target holds the provider's
/// machine beside the entry's, and the roster names the supply. A program
/// that reaches nothing is not judged again.
#[test]
fn a_chosen_provider_is_judged_as_a_root_of_the_same_compilation() {
    let target = supplied_target(&format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}"), REACHES)
        .expect("the provider refines the requirement");
    assert_eq!(target.roster.supplies.len(), 1);
    let supplied = target.accepted().expect("the chosen provider was judged");
    assert_eq!(supplied.module().machines.len(), 2);
    assert_eq!(target.supply.requirements().len(), 1);

    let unreached = supplied_target(
        &format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}"),
        REACHES_NOTHING,
    )
    .unwrap();
    assert!(unreached.roster.supplies.is_empty());
    assert!(unreached.accepted().is_none());
}

/// A provider that reaches a boundary requirement of its own has that
/// requirement's chosen provider judged too: the supply is closed under
/// what the chosen providers reach.
#[test]
fn a_provider_that_reaches_a_requirement_has_its_supply_judged_too() {
    let target = supplied_target(
        &format!("{SLOT}{DEEPER}"),
        "machine main() {\n    Chained::go();\n}\n",
    )
    .expect("both providers refine their requirements");
    assert_eq!(target.roster.supplies.len(), 2);
    assert_eq!(target.supply.requirements().len(), 2);
    let supplied = target.accepted().expect("the chosen providers were judged");
    assert_eq!(supplied.module().machines.len(), 3);
}

/// The twins: a chosen provider that implements another trait, so it gives
/// the requirement no machine, and one whose machine may crash where the
/// requirement says `crashes none`, stop the program. Unchosen, the same
/// declarations stop nothing.
#[test]
fn a_chosen_provider_that_does_not_refine_its_requirement_stops_the_program() {
    let stray = format!(
        "{SLOT}public boundary trait Unrelated {{\n    machine go() reaches none crashes none;\n}}\nNative satisfies Unrelated {{\n    machine go() reaches none crashes none {{\n    }}\n}}\n{NATIVE_DEFAULT}"
    );
    assert_eq!(
        supplied_target(&stray, REACHES).unwrap_err(),
        "not supported yet: declaration provider that does not give a supplied requirement exactly one machine; the checker does not support this construct yet, so the program is refused, with no claim that it is wrong"
    );
    let crashing = format!(
        "{SLOT}{NATIVE_DEFAULT}Native satisfies Service {{\n    machine go() reaches none crashes Trap {{\n    }}\n}}\n"
    );
    let refused = supplied_target(&crashing, REACHES).unwrap_err();
    assert!(refused.contains("crash"), "{refused}");
    assert!(
        supplied_target(&crashing, REACHES_NOTHING)
            .unwrap()
            .accepted()
            .is_none()
    );
}

/// Two targets choose different providers for one requirement, and each
/// provider calls a raw requirement of its own operating system. Both
/// providers are judged in the Product's one module, which therefore
/// declares both raw requirements.
const PER_TARGET: &str = "public boundary trait LinuxRaw {\n    machine call() reaches none crashes none;\n}\npublic boundary trait WindowsRaw {\n    machine call() reaches none crashes none;\n}\nNative satisfies Service {\n    machine go() reaches LinuxRaw crashes none {\n        LinuxRaw::call();\n    }\n}\nOther satisfies Service {\n    machine go() reaches WindowsRaw crashes none {\n        WindowsRaw::call();\n    }\n}\nLeaf satisfies LinuxRaw {\n    machine call() reaches none crashes none {\n    }\n}\ndata LeafDefaults {}\nmachine LeafDefaults::provider_defaults(defaults: &mut LeafDefaults) {\n    LeafDefaults::select_provider<LinuxRaw, Leaf>(defaults);\n}\n";
const REACHES_BOTH_OS: &str = "public boundary trait Service {\n    machine go() reaches LinuxRaw, WindowsRaw crashes none;\n}\ndata NativeDefaults {}\ndata OtherDefaults {}\n";

/// Keys `Native` and the raw Linux leaf to Linux and `Other` to Windows.
fn key_per_target(defaults: &mut Vec<TargetProviderDefaults>) {
    let untargeted = std::mem::take(defaults);
    for selection in untargeted.into_iter().flat_map(|row| row.selections) {
        let target = match selection.provider_type.authored_path.as_str() {
            "Other" => "windows_x86_64",
            _ => "linux_x86_64",
        };
        defaults.push(TargetProviderDefaults {
            target: Some(target.to_owned()),
            machines: Vec::new(),
            selections: vec![selection],
        });
    }
}

/// A target is asked only for what its entries reach through the providers
/// it chose. Linux chooses `Native`, whose raw requirement has a provider,
/// so Linux settles two requirements and is not asked for the Windows
/// provider's raw requirement, although the one module declares it. The
/// twin is Windows: it chooses `Other`, whose raw requirement nobody
/// supplies, and is refused by that requirement's name and no other.
#[test]
fn a_target_demands_what_its_own_chosen_providers_reach() {
    let declarations = format!("{REACHES_BOTH_OS}{NATIVE_DEFAULT}{OTHER_DEFAULT}{PER_TARGET}");
    let linux = supplied_on(&declarations, REACHES, "linux_x86_64", key_per_target)
        .expect("every requirement Linux reaches has a provider");
    let module = linux.accepted().expect("the chosen providers were judged");
    assert_eq!(
        module.module().boundary_machines.len(),
        3,
        "the one module declares both targets' raw requirements"
    );
    assert_eq!(linux.roster.supplies.len(), 3, "both targets' providers");
    let mut demanded = linux
        .supply
        .requirements()
        .iter()
        .map(|requirement| requirement.reached.name.as_str())
        .collect::<Vec<_>>();
    demanded.sort_unstable();
    assert_eq!(demanded, ["LinuxRaw::call", "Service::go"]);
    assert!(linux.supply.settle().is_ok());

    let windows = supplied_on(&declarations, REACHES, "windows_x86_64", key_per_target)
        .expect("preparation reports the demand; settlement rejects it");
    assert_eq!(
        windows.supply.settle().map_err(first).unwrap_err(),
        "boundary requirement `WindowsRaw::call` has no provider on target `windows_x86_64`: the Build selects none and the target package declares no default"
    );
}

/// The walk continues only into a provider the module holds. Read from the
/// first judgment, which holds no provider, a requirement whose provider
/// was chosen is refused by name instead of being settled over a provider
/// nobody judged.
#[test]
fn a_chosen_provider_the_module_does_not_hold_is_refused() {
    let (source, defaults, roster) = judged(&format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}"), REACHES);
    let publication = compilation_products::ProductTerminalPublication::judge(
        source,
        roster,
        optimization_core::OptimizationSelections::default(),
    )
    .unwrap();
    assert_eq!(
        compiler::prepare_supplied_target(compiler::TargetSupplySource {
            publication: &publication,
            build: &[],
            defaults: &defaults,
            target: Some(TARGET),
        })
        .map_err(first)
        .unwrap_err(),
        "no product route yet: target preparation for `linux_x86_64`: `Service::go` is supplied by `Native`, which the accepted module does not hold as a judged provider"
    );
}

fn prepare(
    accepted: &AcceptedTerminalSource,
    build: &[ProviderSelection],
    defaults: &[TargetProviderDefaults],
) -> Result<PreparedTargetSupply, String> {
    prepare_for_target(accepted, build, defaults, Some(TARGET))
}

fn prepare_for_target(
    accepted: &AcceptedTerminalSource,
    build: &[ProviderSelection],
    defaults: &[TargetProviderDefaults],
    target: Option<&str>,
) -> Result<PreparedTargetSupply, String> {
    let declared = accepted
        .boundary_requirements_declared()
        .expect("declared requirements rejoin their declarations")
        .into_iter()
        .map(|declared| ReachedBoundaryRequirement {
            boundary: declared.boundary,
            requirement: declared.requirement,
            slot: declared.slot,
            name: declared.name,
        })
        .collect::<Vec<_>>();
    let roots = accepted.entry().into_iter().collect::<Vec<_>>();
    PreparedTargetSupply::prepare(TargetSupplyRequest {
        module: accepted.module(),
        declared: &declared,
        roots: &roots,
        judged: &[],
        build,
        defaults,
        target,
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
    let (accepted, defaults) = accepted(
        &format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}"),
        REACHES_NOTHING,
    );
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
    let proposal = proposal.expect("selected deployment retains its provider proposal");
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
fn accepted_other() -> (Arc<AcceptedTerminalSource>, Vec<TargetProviderDefaults>) {
    accepted(
        &format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}"),
        "machine helper() {\n}\nmachine main() {\n    helper();\n}\n",
    )
}

/// The twin reaches the requirement, so its supply is chosen and no selected
/// execution can be formed for the target: the only constructor refuses with
/// the plan that cannot be formed, and no product can read a custody that
/// was never settled.
#[test]
fn a_reached_requirement_forms_no_selected_execution_without_its_plan() {
    let (accepted, defaults) = accepted(&format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}"), REACHES);
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
    let (accepted, defaults) = accepted(
        &format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}"),
        REACHES_NOTHING,
    );
    assert_eq!(defaults.len(), 1, "the default is declared");
    assert!(accepted.module().boundary_machines.is_empty());
    let prepared = prepare(&accepted, &[], &defaults).unwrap();
    assert!(prepared.demands_nothing());
    let settled = prepared.settle().unwrap();
    assert_eq!(settled.plans(), Ok(Vec::new()));
}

#[test]
fn unscoped_supply_keeps_pure_programs_targetless_and_refuses_provider_demands() {
    let (accepted, defaults) = accepted(
        &format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}"),
        REACHES_NOTHING,
    );
    let prepared = prepare_for_target(&accepted, &[], &defaults, None).unwrap();
    assert_eq!(prepared.target(), None);
    let settled = prepared.settle().unwrap();
    let execution = selected_dispatch::SelectedExecution::settle(None, &settled).unwrap();
    assert_eq!(execution.target_profile(), None);
    let artifact = produced(&accepted);
    assert!(
        execution
            .provider_proposal(
                artifact.view(),
                optimization_core::OptimizationSelections::default().identity()
            )
            .is_none()
    );
    assert!(selected_dispatch::SelectedExecution::settle(Some(profile()), &settled).is_err());

    let (reached, defaults) =
        self::accepted(&format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}"), REACHES);
    let error = prepare_for_target(&reached, &[], &defaults, None).unwrap_err();
    assert!(
        error.contains("requires an explicit deployment target"),
        "{error}"
    );
}

/// The twin reaches the requirement. Preparation pairs it with the declared
/// default, settlement chooses that default, and the plan of the chosen
/// provider is refused by the requirement's and the provider's names.
#[test]
fn a_reached_requirement_settles_to_its_one_declared_default() {
    let (accepted, defaults) = accepted(&format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}"), REACHES);
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
        settle(
            &format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}{OTHER_DEFAULT}"),
            REACHES
        )
        .unwrap_err(),
        "boundary requirement `Service::go` has conflicting providers on target `linux_x86_64`: the target package declares `Native` and `Other`"
    );
}

/// A default keyed to another target is no supply for this one, and a Build
/// selection outranks the target's default instead of conflicting with it.
/// A selection is one value whichever authority declared it, so the Build's
/// here is the selection the second default states.
#[test]
fn supply_is_ranked_by_authority_and_keyed_by_target() {
    let (accepted, mut defaults) = accepted(
        &format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}{OTHER_DEFAULT}"),
        REACHES,
    );
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
