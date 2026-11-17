//! Target preparation and provider settlement decide from the accepted
//! Terminal module what a target must supply. Each test types a program from
//! source through the ordinary lexer, parser, resolver and typer, has the
//! lowered-form checker judge it, harvests the provider defaults it declares
//! and runs the two stages on that accepted module. The last tests carry the
//! settled supply on into the selected execution a product reads and check
//! the produced program's target-local conditions against it.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

use build_time_evaluation::provider_declarations::{
    ProviderPlanningContext, ProviderSelection, TargetProviderDefaults, retain_provider_defaults,
};
use provider_planning::target_supply::{
    PreparedTargetSupply, ReachedBoundaryRequirement, SettledTargetSupply, SupplyAuthority,
    SupplyPlanning, TargetSupplyRequest,
};
use source_compiler::AcceptedTerminalSource;
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

/// An accepted fixture keeps the same publication used by provider execution.
struct AcceptedFixture {
    publication: Arc<products::ProductTerminalPublication>,
}

impl std::ops::Deref for AcceptedFixture {
    type Target = AcceptedTerminalSource;

    fn deref(&self) -> &Self::Target {
        self.publication
            .accepted()
            .expect("the fixture publication accepted")
    }
}

/// The accepted module of `program`'s `main` and the defaults `supplied`
/// declares. `supplied` stands for the target package: it is a toolchain
/// source of the same compilation, so the checker judges the program's own
/// declarations and reads the boundary trait only where the program calls
/// it.
fn accepted(supplied: &str, program: &str) -> (AcceptedFixture, Vec<TargetProviderDefaults>) {
    let (source, defaults, roster) = judged(supplied, program);
    let publication = products::ProductTerminalPublication::judge(
        source,
        roster,
        optimization_core::OptimizationSelections::default(),
    )
    .expect("the exact roster accepts");
    (
        AcceptedFixture {
            publication: Arc::new(publication),
        },
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
    products::ProductRoster,
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
    let roster = products::ProductRoster {
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
    roster: products::ProductRoster,
    publication: Arc<products::ProductTerminalPublication>,
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
    supplied_product(source, defaults, roster, target, &[])
}

/// Judge the actual source supply, bind its chosen target's normalization to
/// that same frozen owner, and rejudge the same roster before execution uses it.
/// Refused provider bodies remain the supply judgment's original diagnostic.
fn supplied_product(
    source: Arc<typed_trees::FrozenTypedTrees>,
    defaults: Vec<TargetProviderDefaults>,
    roster: products::ProductRoster,
    target: &str,
    native_bindings: &[provider_planning::native_bindings::NativeBuildSupply],
) -> Result<Supplied, String> {
    let original = Arc::new(
        products::ProductTerminalPublication::judge(
            Arc::clone(&source),
            roster,
            optimization_core::OptimizationSelections::default(),
        )
        .map_err(first)?,
    );
    let mut product = match compiler::judge_supplied_product(compiler::ProductSupplySource {
        publication: Arc::clone(&original),
        build: &[],
        native_bindings,
        defaults: &defaults,
        layout_deferred: &[],
        layout_prepared: &[],
    })
    .map_err(first)?
    {
        compiler::SupplyJudgment::Supplied(product) => product,
        compiler::SupplyJudgment::PrepareLayouts(layouts) => {
            panic!("no fixture layout was deferred: {layouts:?}")
        }
    };
    let mut roster = product.roster().map_err(first)?.clone();
    let prepared = compiler::prepare_supplied_target(compiler::TargetSupplySource {
        publication: &product,
        build: &[],
        native_bindings,
        defaults: &defaults,
        target: Some(target),
    })
    .map_err(first)?;
    let mut chosen = Vec::new();
    for requirement in prepared.requirements() {
        if let Some(selection) = requirement.chosen().and_then(|supply| supply.checked())
            && !chosen.contains(selection)
        {
            chosen.push(selection.clone());
        }
    }
    if !chosen.is_empty()
        && !roster
            .normalization
            .providers
            .iter()
            .any(|row| row.target == target)
    {
        let profile = target::TargetProfile::from_canonical_target_name(target)
            .map_err(|diagnostic| diagnostic.message)?;
        let choices = compiler::prepare_provider_choices(&source, &[profile], &chosen, &[], &[])
            .map_err(first)?;
        roster.selected.extend(choices.declaration_roots);
        let normalization = Arc::make_mut(&mut roster.normalization);
        normalization.providers.extend(choices.selections);
        for application in choices.calling_applications {
            if !normalization.calling.applications.contains(&application) {
                normalization.calling.applications.push(application);
            }
        }
        product = Arc::new(
            products::ProductTerminalPublication::judge(
                Arc::clone(&source),
                roster,
                optimization_core::OptimizationSelections::default(),
            )
            .map_err(first)?,
        );
    }
    let supply = compiler::prepare_supplied_target(compiler::TargetSupplySource {
        publication: &product,
        build: &[],
        native_bindings,
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
    let original = products::ProductTerminalPublication::judge(
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
    roster.supplies.push(source_compiler::TerminalSourceSupply {
        slot: requirement.symbol,
        provider: native.symbol,
    });
    let error = products::ProductTerminalPublication::judge(
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
        native_bindings: &[],
        defaults: &defaults,
        layout_deferred: &[],
        layout_prepared: &[],
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
        "not supported yet: declaration provider does not give the supplied requirement exactly one machine; the checker does not support this construct yet, so the program is refused, with no claim that it is wrong"
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
    let publication = products::ProductTerminalPublication::judge(
        source,
        roster,
        optimization_core::OptimizationSelections::default(),
    )
    .unwrap();
    assert_eq!(
        compiler::prepare_supplied_target(compiler::TargetSupplySource {
            publication: &publication,
            build: &[],
            native_bindings: &[],
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
    prepare_for_target(accepted, build, &[], defaults, Some(TARGET))
}

fn prepare_for_target(
    accepted: &AcceptedTerminalSource,
    build: &[ProviderSelection],
    native_bindings: &[provider_planning::native_bindings::NativeBuildSupply],
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
        native_bindings,
        build,
        defaults,
        target,
    })
    .map_err(first)
}

/// ProcessExit's checked external completion still needs a selected provider.
/// Preparing its real declaration must retain that demand, while settlement
/// without any supply must refuse rather than invent a returning realization.
#[test]
fn process_exit_retains_its_external_demand_and_requires_supply() {
    let frontier = unbuilt_frontier::bound_fixture_frontier(
        "tests/fixtures/run/calls/published_process_exit",
        Some("entry_start"),
        &[],
    );
    let roster = products::ProductRoster {
        entries: frontier.entries,
        subject: frontier.subject,
        selected: frontier.selected,
        bindings: frontier.bindings,
        ..Default::default()
    };
    let typed = Arc::new(frontier.typed);
    let accepted = roster
        .judge(Arc::clone(&typed))
        .expect("the actual ProcessExit program is accepted");
    let supply =
        prepare(&accepted, &[], &[]).expect("external completion is an ordinary provider demand");
    let [requirement] = supply.requirements() else {
        panic!("exactly the reached ProcessExit requirement: {supply:?}");
    };
    let boundary = accepted
        .module()
        .boundary_machine(requirement.reached.boundary)
        .expect("the demand keeps its exact checked declaration");
    assert_eq!(
        boundary.completion,
        semantic_vocabulary::BoundaryCompletion::External(
            semantic_vocabulary::ExternalCompletionKind::ProcessExit,
        )
    );
    let projected = provider_planning::hosted_requirement_schema(
        accepted.module(),
        language_core::sealed_intrinsics::HostedOperation::ProcessExit,
        boundary.id,
    )
    .expect("the admitted canonical declaration retains its complete schema");
    let source_schema = accepted
        .slot_schema(requirement.reached.slot)
        .expect("the actual source owns the complete Core schema");
    // Source review uses display paths and ordinary type identities. Native
    // planning uses canonical service applications and package-qualified types.
    // Build the full expected contract from the independent source owner in
    // that coordinate system; copying the projected header would hide a lost
    // nominal Never result or a changed public row.
    let source_owner = typed
        .trait_definition_by_symbol(requirement.reached.slot)
        .unwrap();
    let source_requirement = typed
        .trait_machine_signature_by_symbol(source_owner, requirement.reached.requirement)
        .unwrap();
    let service_application = |symbol| {
        let declaration = typed.trait_definition_by_symbol(symbol).unwrap();
        let scope = typed_trees::trait_definition::AppliedTraitScope::abstract_scope(declaration);
        typed
            .type_identity_text(scope.portable_application_identity(&typed).unwrap())
            .unwrap()
    };
    let mut expected = source_schema.as_ref().clone();
    expected.trait_name = expected
        .identities
        .intern(&service_application(source_owner.symbol));
    assert_eq!(expected.receiver_applications.len(), 1);
    expected.receiver_applications[0].trait_name = expected.trait_name;
    let [method] = expected.methods.as_mut_slice() else {
        panic!("the genuine Core ProcessExit contract has exactly one member");
    };
    method.requirement_owner = service_application(source_owner.symbol);
    method.parameter_type_identities = typed
        .state_signature_parameters(source_requirement)
        .iter()
        .map(|parameter| {
            expected
                .identities
                .intern(&typed.package_qualified_type_identity_text(parameter.type_reference))
        })
        .collect();
    method.result_type_identity = expected
        .identities
        .intern(&typed.package_qualified_type_identity_text(source_requirement.return_type));
    // Distinct source instances can display the same service name. Follow this
    // declaration's exact authored row, never a whole-program name lookup.
    let mut reached = Vec::new();
    for service in typed
        .service_reach_rows
        .services(source_requirement.service_reach_row)
    {
        typed.service_reaches.extend_closure(*service, &mut reached);
    }
    reached.sort_unstable_by_key(|service| service.0);
    reached.dedup();
    let definitions = reached
        .iter()
        .map(|service| typed.service_reaches.definition(*service).unwrap())
        .collect::<Vec<_>>();
    let mut displayed = definitions
        .iter()
        .map(|declaration| declaration.name.clone())
        .collect::<Vec<_>>();
    displayed.sort_unstable();
    displayed.dedup();
    assert_eq!(method.service_reach, displayed);
    method.service_reach = definitions
        .iter()
        .map(|declaration| service_application(declaration.symbol))
        .collect();
    method.service_reach.sort_unstable();
    method.service_reach.dedup();
    assert!(method.synchronous_invocations.is_empty());
    assert_eq!(
        projected.declaration.as_ref(),
        &expected,
        "native projection must retain nominal result, progress and all public rows",
    );
    assert!(requirement.supplies.is_empty());
    let missing = supply.settle().expect_err("no provider was selected");
    assert_eq!(missing.len(), 1);
    assert!(missing[0].message.contains("ProcessExit::exit_process"));
    assert!(missing[0].message.contains("has no provider on target"));
}

fn settle(supplied: &str, program: &str) -> Result<SettledTargetSupply, String> {
    let (accepted, defaults) = accepted(supplied, program);
    prepare(&accepted, &[], &defaults)?.settle().map_err(first)
}

fn first(diagnostics: Vec<diagnostics::Diagnostic>) -> String {
    diagnostics[0].message.clone()
}

fn native_supply(
    accepted: &AcceptedTerminalSource,
    target: language_semantics::declaration_selection::ProductTarget,
    supplier: semantic_vocabulary::PackageKeyIdentity,
) -> provider_planning::native_bindings::NativeBuildSupply {
    use evaluation_accounting::{
        BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
        TerminalStorageScheduleIdentity,
    };
    let requirement = accepted
        .boundary_requirements_declared()
        .unwrap()
        .into_iter()
        .find(|requirement| requirement.name == "Service::go")
        .unwrap();
    let measurement = EvaluationMeasurementIdentity::TerminalPsi {
        work_schedule: semantic_vocabulary::WorkScheduleIdentity::new(1).unwrap(),
        storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
    };
    let sponsor = BuildEvaluationSponsor::new_with_measurement(
        BuildEvaluationSponsorLimits::new(100, 100, 8, 8, 100, 100, 100, 100).unwrap(),
        measurement,
    )
    .unwrap();
    let usage = *sponsor
        .begin_invocation(measurement, 100)
        .unwrap()
        .snapshot()
        .unwrap()
        .record();
    provider_planning::native_bindings::NativeBuildSupply {
        selection: build_time_evaluation::native_bindings::NativeBindingSelection {
            application: None,
            subject: requirement.slot,
            requirement: requirement.requirement,
            target,
            locator: semantic_vocabulary::NativeBindingLocator::Syscall { number: 39 },
            producer: language_semantics::declaration_selection::NativeBindingProducer {
                package: supplier,
                declaration_path: "build".into(),
                callable_identity: "build::bind_service".into(),
            },
            source_occurrence_digest: [5; 32],
            supplier_package: supplier,
            source_span: Default::default(),
        },
        evaluation: Arc::new(provider_planning::native_bindings::NativeBuildEvaluation {
            terminal_code: terminal_psi::TerminalPsiIdentity {
                vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([7; 32]),
            },
            usage,
            observation_digest: [9; 32],
        }),
    }
}

#[test]
fn a_native_binding_supplies_only_its_reached_requirement() {
    use language_semantics::declaration_selection::ProductTarget;
    let (source, defaults, roster) = judged(&format!("{WHOLE_SLOT}{NATIVE_DEFAULT}"), REACHES);
    let original = Arc::new(
        products::ProductTerminalPublication::judge(
            source,
            roster,
            optimization_core::OptimizationSelections::default(),
        )
        .unwrap(),
    );
    let native = native_supply(
        original.accepted().unwrap(),
        ProductTarget::LinuxX86_64,
        package(),
    );
    let publication = supplied_product(
        Arc::clone(original.accepted().unwrap().source()),
        defaults.clone(),
        original.roster().unwrap().clone(),
        "windows_x86_64",
        std::slice::from_ref(&native),
    )
    .expect("the actual checked neighbor and native supply share one judged source")
    .publication;
    assert!(
        publication
            .roster()
            .unwrap()
            .declaration_metadata
            .is_empty(),
        "the common boundary publisher needs no native-only metadata nomination"
    );
    let supplies = &publication.roster().unwrap().supplies;
    let [checked] = supplies.as_slice() else {
        panic!("the other targets choose one shared checked default")
    };
    assert_eq!(checked.slot, native.selection.subject);
    assert_eq!(
        checked.provider,
        defaults[0].selections[0].provider_type.symbol
    );
    let accepted = publication.accepted().unwrap();
    assert!(
        accepted
            .module()
            .calling_signatures
            .iter()
            .any(|signature| {
                signature.application.is_ok()
                    && accepted
                        .module()
                        .boundary_machine(signature.boundary)
                        .is_some()
            }),
        "the native boundary retains its ABI through the shared publication"
    );
    let other = compiler::prepare_supplied_target(compiler::TargetSupplySource {
        publication: &publication,
        build: &[],
        native_bindings: std::slice::from_ref(&native),
        defaults: &defaults,
        target: Some("windows_x86_64"),
    })
    .unwrap()
    .settle()
    .unwrap();
    assert_eq!(other.requirements().len(), 1);
    assert_eq!(
        other.requirements()[0].supply.authority,
        SupplyAuthority::TargetDefault
    );
    assert_eq!(
        other.requirements()[0].supply.checked(),
        Some(&defaults[0].selections[0])
    );
    assert_eq!(
        other.requirements()[0].reached.requirement,
        native.selection.requirement
    );
    assert!(
        other
            .native_bindings(accepted.module())
            .unwrap()
            .rows
            .is_empty()
    );
    let other_plans = plans(&publication, &other).unwrap();
    let [other_plan] = other_plans.as_slice() else {
        panic!("the other target retains its exact checked default plan")
    };
    assert_eq!(other_plan.provider_type, "Native");
    assert_eq!(other_plan.target, "windows_x86_64");

    let prepared = prepare_for_target(
        accepted,
        &[],
        std::slice::from_ref(&native),
        &defaults,
        Some(TARGET),
    )
    .unwrap();
    assert_eq!(
        prepared.requirements().len(),
        1,
        "the uncalled sibling owes no native binding"
    );
    let settled = prepared.settle().unwrap();
    assert_eq!(
        settled.requirements()[0].supply.authority,
        SupplyAuthority::NativeBinding
    );
    assert!(
        plans(&publication, &settled).unwrap().is_empty(),
        "a native leaf has no checked-provider plan"
    );
    let normalized = settled.native_bindings(accepted.module()).unwrap();
    let [row] = normalized.rows.as_slice() else {
        panic!("the chosen leaf retains one receipt")
    };
    let boundary = accepted.module().boundary_machine(row.boundary).unwrap();
    let abi = accepted
        .module()
        .calling_abi_identity(row.boundary)
        .unwrap();
    let module = accepted.module();
    assert_eq!(module.calling_signatures.len(), 2);
    assert_eq!(module.calling_abi_signatures.len(), 2);
    for declared in &module.boundary_machines {
        let application = module
            .calling_signatures
            .iter()
            .find(|signature| signature.boundary == declared.id)
            .unwrap();
        let calling = module
            .calling_abi_signature(*application.application.as_ref().unwrap())
            .unwrap();
        assert_eq!(
            module.identities.text(calling.requirement_identity),
            module
                .identities
                .text(declared.declaration.as_ref().unwrap().source.callable)
        );
    }
    let calling = module.calling_abi_signature(abi.id()).unwrap();
    assert!(
        calling.storage.is_ok(),
        "the canonical calling declaration retains its storage"
    );
    let callable = boundary.declaration.as_ref().unwrap().source.callable;
    let origin = row
        .binding
        .receipt()
        .build_origin()
        .expect("the evaluation keeps its Build origin");
    assert_eq!(
        normalized.identities.text(origin.requirement_identity),
        accepted.module().identities.text(callable)
    );
}

#[test]
fn native_binding_targets_and_supplier_conflicts_are_enforced() {
    use language_semantics::declaration_selection::ProductTarget;
    let (accepted, _) = accepted(WHOLE_SLOT, REACHES);
    let native = native_supply(&accepted, ProductTarget::LinuxArm64, package());
    let missing = prepare_for_target(&accepted, &[], &[native], &[], Some(TARGET))
        .unwrap()
        .settle()
        .unwrap_err();
    assert!(first(missing).contains("has no provider"));
    let native = native_supply(&accepted, ProductTarget::LinuxX86_64, package());
    let mut other = native.clone();
    let other_supplier = semantic_vocabulary::PackageKeyIdentity::from_digest([2; 32]).unwrap();
    other.selection.supplier_package = other_supplier;
    other.selection.producer.package = other_supplier;
    let conflict = prepare_for_target(&accepted, &[], &[native, other], &[], Some(TARGET))
        .unwrap()
        .settle()
        .unwrap_err();
    assert!(first(conflict).contains("conflicting providers"));
}

#[test]
fn an_explicit_checked_selection_overrides_a_native_binding() {
    use language_semantics::declaration_selection::ProductTarget;
    let (accepted, defaults) = accepted(&format!("{WHOLE_SLOT}{NATIVE_DEFAULT}"), REACHES);
    let explicit = defaults[0].selections[0].clone();
    let native = native_supply(&accepted, ProductTarget::LinuxX86_64, package());
    let settled = prepare_for_target(
        &accepted,
        std::slice::from_ref(&explicit),
        &[native],
        &defaults,
        Some(TARGET),
    )
    .unwrap()
    .settle()
    .unwrap();
    assert_eq!(
        settled.requirements()[0].supply.authority,
        SupplyAuthority::Build
    );
    assert_eq!(settled.requirements()[0].supply.checked(), Some(&explicit));
    assert!(
        settled
            .native_bindings(accepted.module())
            .unwrap()
            .rows
            .is_empty()
    );
}

fn profile() -> target::TargetProfile {
    target::TargetProfile::from_canonical_target_name(TARGET).expect("a hosted target")
}

/// The provider plans of `settled`, formed over the module and slot schemas
/// of `accepted` as a product of the target forms them.
fn plans(
    publication: &products::ProductTerminalPublication,
    settled: &SettledTargetSupply,
) -> Result<Vec<effects::provider_plan::ProviderPlan>, String> {
    if settled.requirements().is_empty() {
        return settled
            .realize_no_demand()
            .map(|realized| realized.provider_declarations().to_vec())
            .map_err(first);
    }
    let accepted = publication.accepted().map_err(first)?;
    let schemas = compiler::settled_slot_schemas(accepted, settled);
    settled
        .plans(SupplyPlanning {
            module: accepted.module(),
            schemas: &schemas,
            execution: execution_inputs(publication, settled)?,
            installation_requirements: &accepted.installation_reach_requirements(),
        })
        .map_err(first)
}

/// The selected execution a product of `profile` forms from `settled`.
fn execution(
    profile: Option<target::TargetProfile>,
    publication: &products::ProductTerminalPublication,
    settled: &SettledTargetSupply,
) -> Result<selected_dispatch::SelectedExecution, String> {
    if settled.requirements().is_empty() {
        return selected_dispatch::SelectedExecution::from_realization(
            profile,
            settled.realize_no_demand().map_err(first)?,
        )
        .map_err(first);
    }
    let accepted = publication.accepted().map_err(first)?;
    let schemas = compiler::settled_slot_schemas(accepted, settled);
    selected_dispatch::SelectedExecution::settle(
        profile,
        settled,
        SupplyPlanning {
            module: accepted.module(),
            schemas: &schemas,
            execution: execution_inputs(publication, settled)?,
            installation_requirements: &accepted.installation_reach_requirements(),
        },
    )
    .map_err(first)
}

/// Seal the fixture's actual source consumption, then use the product's one
/// publication/calling/native/permission producer. No Build or imported package
/// was executed by this fixture, so source sealing records that exact absence.
fn execution_inputs(
    publication: &products::ProductTerminalPublication,
    settled: &SettledTargetSupply,
) -> Result<provider_planning::target_supply::TargetSupplyExecutionInputs, String> {
    let accepted = publication.accepted().map_err(first)?;
    let source = accepted.source();
    let sources = package_compilation::BuildSourceCustody {
        source_file_count: source.symbols.source_files().count(),
        build_execution_profile: None,
        generated_source_custody: Vec::new(),
        base_source_consumption_commitment: None,
    }
    .seal(source, None)
    .map_err(first)?;
    let target = settled
        .target()
        .ok_or_else(|| "nonempty provider supply lost its deployment target".to_owned())?;
    let profile =
        target::TargetProfile::from_canonical_target_name(target).map_err(|error| error.message)?;
    let mut reached = settled
        .requirements()
        .iter()
        .map(|requirement| requirement.reached.boundary)
        .collect::<Vec<_>>();
    reached.sort_unstable();
    reached.dedup();
    compiler::prepare_provider_execution_inputs(
        publication,
        &sources,
        &[], // This fixture executes no Build or dependency-native supplies.
        profile,
        &reached,
        &mut source_compiler::UnmeasuredProduction,
    )
    .map_err(first)
}

/// The Terminal artifact a product of the accepted source is checked on.
fn produced(accepted: &AcceptedTerminalSource) -> Arc<terminal_codec::CanonicalTerminalArtifact> {
    let program = accepted
        .produce(
            Default::default(),
            &mut source_compiler::UnmeasuredProduction,
        )
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
    let execution = execution(Some(profile()), &accepted.publication, &settled)
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
fn accepted_other() -> (AcceptedFixture, Vec<TargetProviderDefaults>) {
    accepted(
        &format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}"),
        "machine helper() {\n}\nmachine main() {\n    helper();\n}\n",
    )
}

/// The twin reaches the requirement, so its supply is chosen, but it is read
/// from the first judgment, which never judged the chosen provider. No
/// selected execution can be formed for the target: the only constructor
/// refuses with the plan that cannot be formed, and no product can read a
/// custody that was never settled.
#[test]
fn a_reached_requirement_forms_no_selected_execution_without_its_judged_provider() {
    let (accepted, defaults) = accepted(&format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}"), REACHES);
    let settled = prepare(&accepted, &[], &defaults)
        .unwrap()
        .settle()
        .unwrap();
    assert_eq!(
        execution(Some(profile()), &accepted.publication, &settled)
            .map(|execution| execution.plans().plans().len()),
        Err(
            "provider settlement for `linux_x86_64`: `Service::go` is supplied by `Native`, which the accepted module does not hold as a judged provider"
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
    assert_eq!(plans(&accepted.publication, &settled), Ok(Vec::new()));
}

#[test]
fn unscoped_supply_keeps_pure_programs_targetless_and_refuses_provider_demands() {
    let (accepted, defaults) = accepted(
        &format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}"),
        REACHES_NOTHING,
    );
    let prepared = prepare_for_target(&accepted, &[], &[], &defaults, None).unwrap();
    assert_eq!(prepared.target(), None);
    let settled = prepared.settle().unwrap();
    let execution = execution(None, &accepted.publication, &settled).unwrap();
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
    assert!(self::execution(Some(profile()), &accepted.publication, &settled).is_err());

    let (reached, defaults) =
        self::accepted(&format!("{SLOT}{PROVIDERS}{NATIVE_DEFAULT}"), REACHES);
    let error = prepare_for_target(&reached, &[], &[], &defaults, None).unwrap_err();
    assert!(
        error.contains("requires an explicit deployment target"),
        "{error}"
    );
}

/// The twin reaches the requirement. Preparation pairs it with the declared
/// default and settlement chooses that default. Read from the first
/// judgment, which holds no provider, the plan of the chosen provider is
/// refused by the requirement's and the provider's names.
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
            .checked()
            .unwrap()
            .provider_type
            .authored_path,
        "Native"
    );
    assert_eq!(
        plans(&accepted.publication, &settled),
        Err(
            "provider settlement for `linux_x86_64`: `Service::go` is supplied by `Native`, which the accepted module does not hold as a judged provider"
                .to_owned()
        )
    );
}

/// A boundary trait with two requirements, and a provider of both. The
/// programs below reach only `go`.
const WHOLE_SLOT: &str = "public boundary trait Service {\n    machine go() reaches none crashes none;\n    machine stop() reaches none crashes none;\n}\ndata NativeDefaults {}\nNative satisfies Service {\n    machine go() reaches none crashes none {\n    }\n    machine stop() reaches none crashes none {\n    }\n}\n";

/// The settled supply of `WHOLE_SLOT`'s provider for a program that reaches
/// `go`, and the product whose module judged the provider.
fn whole_slot() -> (
    Arc<products::ProductTerminalPublication>,
    SettledTargetSupply,
) {
    let (source, defaults, roster) = judged(&format!("{WHOLE_SLOT}{NATIVE_DEFAULT}"), REACHES);
    let target = supplied_product(source, defaults, roster, TARGET, &[])
        .expect("the provider refines both requirements");
    assert!(target.rejudged, "the chosen provider was judged");
    let settled = target.supply.settle().expect("one default is chosen");
    (target.publication, settled)
}

/// A chosen provider forms one plan for its whole slot. The program reaches
/// only `go`, but the plan binds `stop` too, because a plan covers the
/// slot's complete schema: the module declares both requirements and states
/// the provider's candidate for each. The product's proposal joins the
/// plan's rows to those candidates and installs the two machines the
/// checker judged for the provider, and the produced program's target-local
/// conditions check against it.
#[test]
fn a_chosen_provider_forms_one_plan_for_its_whole_slot() {
    let (publication, settled) = whole_slot();
    let accepted = publication
        .accepted()
        .expect("the product publication accepted");
    assert_eq!(settled.requirements().len(), 1, "only `go` is reached");
    let module = accepted.module();
    assert_eq!(module.boundary_machines.len(), 2);
    assert_eq!(module.provider_candidates.len(), 2);

    let execution = execution(Some(profile()), &publication, &settled)
        .expect("the chosen provider's plan covers its slot");
    let [plan] = execution.plans().plans() else {
        panic!("one provider is chosen: {:?}", execution.plans().plans());
    };
    let [choice] = module.provider_selections.as_slice() else {
        panic!("the accepted source selected exactly one whole-slot provider");
    };
    assert_eq!(choice.target, TARGET);
    let provider_identity = module
        .identities
        .text(choice.provider_identity)
        .expect("the accepted choice retains its canonical provider identity");
    assert_eq!(plan.provider_type, provider_identity);
    assert_eq!(
        plan.application_identity(),
        choice.provider_application.map(|application| {
            module
                .identities
                .text(application)
                .expect("the accepted choice retains its exact provider application")
        })
    );
    assert_eq!(plan.provider_type_package_identity, choice.provider_package);
    assert!(module.provider_candidates.iter().all(|candidate| {
        candidate.provider_identity == choice.provider_identity
            && candidate.provider_declaration.identity() == choice.provider_declaration
    }));
    assert_eq!(
        settled.requirements()[0]
            .supply
            .checked()
            .expect("the chosen whole-slot supply is checked")
            .provider_type
            .authored_path,
        "Native"
    );
    assert_eq!(plan.target, TARGET);
    for candidate in &module.provider_candidates {
        assert_eq!(
            plan.provider_declaration.as_deref(),
            module
                .identities
                .text(candidate.provider_declaration.identity()),
        );
    }
    assert_eq!(plan.row_numeric_realizations, vec![None; plan.rows.len()]);
    let mut methods = plan
        .rows
        .iter()
        .map(|row| row.method.as_str())
        .collect::<Vec<_>>();
    methods.sort_unstable();
    assert_eq!(methods, ["go", "stop"]);

    let artifact = produced(accepted);
    let proposal = execution
        .provider_proposal(
            artifact.view(),
            optimization_core::OptimizationSelections::default().identity(),
        )
        .expect("selected deployment retains its provider proposal");
    let selection = proposal
        .selection
        .as_ref()
        .expect("the plan's rows join the artifact's candidates");
    assert_eq!(selection.plans, execution.plans().plans());
    let mut installed = selection
        .checked_candidates
        .iter()
        .map(|installed| installed.candidate)
        .collect::<Vec<_>>();
    installed.sort_unstable();
    let mut judged = accepted
        .supplied_requirements()
        .iter()
        .map(|supplied| supplied.machine)
        .collect::<Vec<_>>();
    judged.sort_unstable();
    assert_eq!(installed, judged);
    assert_eq!(check_conditions(&artifact, &proposal), Ok(()));
}

/// A supplied slot needs ABI declarations for its uncalled members too.
/// Their signatures remain distinct from the reached member's signature.
#[test]
fn a_whole_slot_keeps_each_members_exact_calling_declaration() {
    let supplied = format!(
        "{}{NATIVE_DEFAULT}",
        WHOLE_SLOT.replace("stop()", "stop(bytes: &[u8])")
    );
    let target = supplied_target(&supplied, REACHES).expect("both supplied bodies check");
    assert_eq!(target.supply.requirements().len(), 1, "only go is demanded");
    let accepted = target.publication.accepted().unwrap();
    let module = accepted.module();
    assert_eq!(module.calling_signatures.len(), 2);
    assert_eq!(module.calling_abi_signatures.len(), 2);
    for boundary in &module.boundary_machines {
        let declaration = boundary.declaration.as_ref().unwrap();
        let signature = module
            .calling_signatures
            .iter()
            .find(|row| row.boundary == boundary.id)
            .unwrap();
        let abi = module
            .calling_abi_signature(*signature.application.as_ref().unwrap())
            .unwrap();
        assert_eq!(
            module.identities.text(abi.requirement_identity),
            module.identities.text(declaration.source.callable),
            "each ABI belongs to its exact requirement"
        );
    }
    let settled = target.supply.settle().unwrap();
    let execution = execution(Some(profile()), &target.publication, &settled).unwrap();
    let plan = &execution.plans().plans()[0];
    let program = provider_planning::calling_policy_plans::CallingProgram {
        artifact: produced(accepted),
        evaluator: build_settings::CALLING_POLICY_EVALUATOR,
    };
    let applications = provider_planning::calling_policy_plans::component_calling_applications(
        profile(),
        &program,
        &[plan],
        &[],
    )
    .expect("every member has a materializable declaration application");
    assert_eq!(applications.len(), 2);

    let mut changed = plan.clone();
    let sibling = module
        .calling_abi_signatures
        .iter()
        .find(|abi| {
            module.identities.text(abi.requirement_identity)
                != Some(
                    changed
                        .identities
                        .text(changed.rows[0].declared_requirement_identity),
                )
        })
        .unwrap();
    changed.rows[0].calling_signature = Some(sibling.id);
    let error = provider_planning::calling_policy_plans::component_calling_applications(
        profile(),
        &program,
        &[&changed],
        &[],
    )
    .expect_err("one member cannot borrow its sibling's declaration ABI");
    assert!(
        diagnostics::join_messages(&error)
            .contains("selected closed method lost its exact published declaration ABI")
    );
}

/// Local checked calls keep the sum's actual tag and every payload. That
/// private layout does not authorize passing the same type to foreign code.
#[test]
fn a_local_checked_provider_keeps_case_results_without_a_foreign_abi() {
    let supplied = format!(
        r#"
public data Outcome {{
    case Empty;
    case Count(value: u64);
    case Byte(value: u8);
}}
public boundary trait Service {{
    machine go() -> Outcome reaches none crashes none;
    machine stop() -> Outcome reaches none crashes none;
}}
Native satisfies Service {{
    machine go() -> Outcome reaches none crashes none {{ Outcome::Count {{ value: 7u64 }} }}
    machine stop() -> Outcome reaches none crashes none {{ Outcome::Byte {{ value: 9u8 }} }}
}}
data NativeDefaults {{}}
{NATIVE_DEFAULT}
"#
    );
    let target = supplied_target(
        &supplied,
        "machine main() { let result: Outcome = Service::go(); }",
    )
    .expect("the local provider and its uncalled member check");
    assert_eq!(target.supply.requirements().len(), 1);
    let accepted = target.publication.accepted().unwrap();
    let settled = target.supply.settle().unwrap();
    let execution = execution(Some(profile()), &target.publication, &settled).unwrap();
    let artifact = produced(accepted);
    let proposal = execution
        .provider_proposal(
            artifact.view(),
            optimization_core::OptimizationSelections::default().identity(),
        )
        .unwrap();
    let selection = proposal.selection.as_ref().unwrap();
    let installed = selection
        .admit(artifact.view(), TARGET)
        .expect("all local members have exact checked installations");
    assert_eq!(installed.providers().count(), 2);
    assert_eq!(check_conditions(&artifact, &proposal), Ok(()));

    let module = artifact.semantic_module();
    let terminal_psi::BoundaryMachineResult::Structural(result) =
        &module.boundary_machines[0].result
    else {
        panic!("the result keeps its logical case type")
    };
    let geometry =
        calling_conventions::terminal_layout::NativeGeometry::new(&module.structural_types)
            .unwrap();
    let layout = geometry.sum_layout(result.structural_type).unwrap();
    assert_eq!(
        layout.tag_shape,
        calling_conventions::ValueShape::integer(4, 4)
    );
    assert_eq!(
        layout.shape,
        calling_conventions::ValueShape::integer(16, 8)
    );
    assert_eq!(layout.cases.len(), 3);
    assert!(layout.cases[0].fields.is_empty());
    assert_eq!(
        layout.cases[1].fields[0].shape,
        calling_conventions::ValueShape::integer(8, 8)
    );
    assert_eq!(
        layout.cases[2].fields[0].shape,
        calling_conventions::ValueShape::integer(1, 1)
    );
    assert_eq!(layout.cases[1].fields[0].byte_offset, 8);
    assert_eq!(layout.cases[2].fields[0].byte_offset, 8);

    let no_extents = |_| panic!("this closed case type has no evaluated extent");
    let mut foreign = layout::TerminalBoundaryRecipes::new(module, TARGET, &no_extents);
    let refused = foreign.value(result.structural_type).unwrap_err();
    assert!(
        refused.contains("case data is not yet classifiable as a boundary value"),
        "{refused}"
    );

    let mut changed = selection.clone();
    changed.checked_candidates[0].candidate = changed.checked_candidates[1].candidate;
    let refused = changed.admit(artifact.view(), TARGET).unwrap_err();
    assert!(
        refused.contains("complete canonical selected-provider roster"),
        "{refused}"
    );
}

/// The twins of the plan. A module that states no candidate of the provider
/// for one requirement of the slot forms no plan, and the refusal names that
/// requirement although the program never reaches it. A plan whose adapter
/// rows name each other's machines is refused by the artifact's candidates:
/// each row must name the machine stated for its own requirement.
#[test]
fn a_plan_is_refused_by_the_requirement_it_does_not_bind() {
    let (publication, settled) = whole_slot();
    let accepted = publication
        .accepted()
        .expect("the product publication accepted");
    let schemas = compiler::settled_slot_schemas(accepted, &settled);
    let reached = settled.requirements()[0].reached.boundary;
    let mut missing = accepted.module().clone();
    missing
        .provider_candidates
        .retain(|candidate| candidate.boundary == reached);
    let refused = settled
        .plans(SupplyPlanning {
            module: &missing,
            schemas: &schemas,
            execution: execution_inputs(&publication, &settled)
                .expect("retain execution inputs from the actual accepted publication"),
            installation_requirements: &accepted.installation_reach_requirements(),
        })
        .map_err(first)
        .unwrap_err();
    assert!(
        refused.contains("`Native` supplies `Service::stop` with no checked machine"),
        "{refused}"
    );

    let mut crossed = plans(&publication, &settled).expect("the plan covers its slot");
    let rows = &mut crossed[0].rows;
    let machines = rows
        .iter()
        .map(|row| match row.binding {
            effects::provider_plan::ProviderBinding::CheckedAdapter {
                machine_identity, ..
            } => machine_identity,
            _ => panic!("a checked provider's rows are checked adapters"),
        })
        .collect::<Vec<_>>();
    for (row, machine) in rows.iter_mut().zip(machines.into_iter().rev()) {
        if let effects::provider_plan::ProviderBinding::CheckedAdapter {
            machine_identity, ..
        } = &mut row.binding
        {
            *machine_identity = machine;
        }
    }
    let artifact = produced(accepted);
    let refused =
        backend_plan::TerminalTargetProviderSelection::from_plans(crossed, artifact.view())
            .unwrap_err();
    assert!(
        refused.contains("no emitted exact declaration candidate"),
        "{refused}"
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
            .checked()
            .unwrap()
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

/// Real dependency Build execution supplies the raw Console write binding.
/// The receiver must keep its complete checked method and reject a locator
/// receipt or Calling identity associated with any other requirement.
#[test]
fn a_completed_plain_binding_keeps_its_receipt_and_calling_identity() {
    let workspace = fixture_package_inputs::repo_root()
        .join("build")
        .join(format!(
            "omega-plain-boundary-native-receipts-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&workspace);
    std::fs::create_dir_all(&workspace).unwrap();
    let main = workspace.join("main.omg");
    std::fs::write(
        &main,
        r#"
use omega_language_std::Console;
machine main() reaches Console crashes none { Console::write_byte(65); }
"#,
    )
    .unwrap();
    let standard = fixture_package_inputs::bundled_standard_library_root();
    let standard = standard
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    std::fs::write(
        workspace.join("build.omg"),
        format!(
            r#"
machine build(builder: &mut Build) {{
    Build::application(builder, "plain_boundary_native_receipts");
    let standard: DependencyRegistration = Build::dependency(builder,
        dependency::from_source(Source::Path {{ location: "{standard}" }}));
    Build::bind(builder, windows_x86_64::ProgramEntry, main);
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    Build::bind(builder, linux_arm64::ProgramEntry, main);
    Build::bind(builder, macos_arm64::ProgramEntry, main);
}}
"#
        ),
    )
    .unwrap();
    let profile = target::TargetProfile::host_if_supported().expect("hosted owner control");
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let mut prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
                &main, Some(profile.target_name()),
            )?;
            let checked = fixture_package_inputs::check_fixture(
                &mut prepared, profile.target_name(), Default::default(),
            )?;
            let accepted = checked.terminal_publication().accepted()?;
            let module = accepted.module();
            let realization = checked.execution().realization();
            let mut leaves = realization.direct_boundary_leaves().iter().filter(|leaf|
                matches!(&leaf.execution, backend_plan::DirectBoundaryExecution::Foreign(Ok(binding))
                    if binding.method == "write"));
            let leaf = leaves.next().expect("the actual Console wrapper reaches its raw write");
            assert!(leaves.next().is_none(), "one target has one exact raw write");
            let mut boundaries = module.boundary_machines.iter().filter(|boundary|
                module.identities.text(boundary.identity)
                    == Some(leaf.satisfied_requirement_identity.as_str()));
            let boundary = boundaries.next().expect("the original raw requirement is retained");
            assert!(boundaries.next().is_none());
            let schema = backend_plan::DirectBoundaryLeaf::declared_schema(module, boundary.id)
                .expect("the raw requirement retains its complete public schema");
            assert_eq!(leaf.satisfied_schema.declaration.as_ref(), schema.surface.as_ref());
            let formals = boundary.scalar_contract_parameters().unwrap();
            let count = formals.last().expect("the raw count is the last scalar formal");
            let count = semantic_vocabulary::ScalarTerm::value(count.id, count.scalar_type);
            fn bounds_count_at_four(
                clause: &semantic_vocabulary::Proposition,
                count: &semantic_vocabulary::ScalarTerm,
            ) -> bool {
                match clause {
                    semantic_vocabulary::Proposition::LessOrEqual(left, right) =>
                        left == count && right.integer_value().is_some_and(|(_, value)| matches!(value,
                            semantic_vocabulary::IntegerValue::Unsigned(4)
                                | semantic_vocabulary::IntegerValue::Signed(4))),
                    semantic_vocabulary::Proposition::Conjunction(clauses) =>
                        clauses.iter().any(|clause| bounds_count_at_four(clause, count)),
                    _ => false,
                }
            }
            assert!(boundary.scalar_requires.iter().any(|clause| bounds_count_at_four(clause, &count)),
                "the actual count <= 4 requirement must remain on the checked boundary");
            leaf.validate_plain_execution(module, realization.evaluated_native_bindings(),
                realization.evaluated_binding_identities()).unwrap();

            let source = accepted.closure().elaboration.boundaries.get(boundary.id)
                .expect("the binding has its exact source correspondence");
            let supply = checked.build().config.native_bindings.iter().find(|supply|
                supply.selection.requirement == *source
                    && supply.selection.target.canonical_name() == profile.target_name())
                .expect("the dependency Build produced the actual selected locator");
            let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
            let wrong = provider_planning::native_bindings::normalize_native_binding(
                supply, "wrong-requirement-for-the-same-completed-locator", &mut identities,
            ).unwrap();
            let refusal = leaf.validate_plain_execution(module, &[wrong], &identities.build())
                .expect_err("a completed locator for another requirement is not this supply");
            assert!(refusal.contains("unique completed Build locator receipt"), "{refusal}");

            let mut wrong = leaf.clone();
            let backend_plan::DirectBoundaryExecution::Foreign(Ok(binding)) = &mut wrong.execution else {
                panic!("the selected plain leaf is foreign")
            };
            binding.calling_applications.first_mut().expect("actual Calling result")
                .abi.requirement_identity.push_str("::wrong");
            let refusal = wrong.validate_plain_execution(module,
                realization.evaluated_native_bindings(), realization.evaluated_binding_identities())
                .expect_err("an unrelated Calling ABI cannot execute this requirement");
            assert!(refusal.contains("public declaration ABI"), "{refusal}");
            Ok::<(), Vec<diagnostics::Diagnostic>>(())
        }).unwrap().join().unwrap()
    }).unwrap_or_else(|refused| panic!("completed plain binding: {refused:?}"));
    std::fs::remove_dir_all(workspace).unwrap();
}
