//! Check how authored defaults and Build selections reach published Psi.
#[path = "../../../../../../../tests/support/provider_choice_programs.rs"]
mod programs;

use crate::compile_product::provider_choices::{
    PreparedProviderChoices, prepare_with_source_defaults,
};
use crate::compile_product::provider_declarations::{
    ProviderPlanningContext, ProviderSelection, TargetProviderDefaults, retain_provider_defaults,
};
use crate::publication::AcceptedTerminalSource;
use crate::test_support::TerminalSourceCompilation;
use std::sync::Arc;

const TARGET: &str = "linux_x86_64";
const SLOT: &str = "public boundary trait Service {\n    machine go() reaches none crashes none;\n}\ndata Native {}\ndata Other {}\n";
const ORIGINAL_NATIVE_DEFAULT: &str = "machine Native::provider_defaults(defaults: @mutable Native) {\n    Native::select_provider::<Service, Native>(defaults);\n}\n";

// The named implementations supply the operations. Separate data declarations select defaults.
const NAMED_SLOT: &str = "public boundary trait Service {\n    machine go() reaches none crashes none;\n}\npublic Native satisfies Service {\n    machine go() reaches none crashes none {}\n}\npublic Other satisfies Service {\n    machine go() reaches none crashes none {}\n}\ndata NativeDefaults {}\ndata OtherDefaults {}\n";
const NATIVE_DEFAULT: &str = "machine NativeDefaults::provider_defaults(defaults: @mutable NativeDefaults) {\n    NativeDefaults::select_provider::<Service, Native>(defaults);\n}\n";
const OTHER_DEFAULT: &str = "machine OtherDefaults::provider_defaults(defaults: @mutable OtherDefaults) {\n    OtherDefaults::select_provider::<Service, Other>(defaults);\n}\n";
const REACHES: &str = "machine main() {\n    Service::go();\n}\n";
const REACHES_NOTHING: &str = "machine main() {\n}\n";

fn package() -> semantic_vocabulary::PackageKeyIdentity {
    semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap()
}

fn first(diagnostics: Vec<diagnostics::Diagnostic>) -> String {
    diagnostics[0].message.clone()
}

struct Fixture {
    source: crate::test_support::preparation::PreparedTypedTrees,
    entry: symbols::SymbolHandle,
    defaults: Vec<TargetProviderDefaults>,
}

fn typed(supplied: &str, program: &str) -> Fixture {
    try_typed(supplied, program).expect("harvest source defaults")
}

fn receiving_source(
    supplied: &str,
    program: &str,
    purpose: source_provenance::DependencyScope,
) -> Result<typed_trees::TypedTrees, Vec<diagnostics::Diagnostic>> {
    let mut sources = crate::representations::source_inputs::SourceMap::default();
    let mut roots = Vec::new();
    for (path, text, origin) in [
        (
            "program/main.omg",
            program,
            source_admission::SourceOrigin::User,
        ),
        (
            "program/provider.omg",
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
                purpose,
            )
            .source_id;
        roots.push(id);
    }
    let primary = roots[0];
    let parsed = crate::ParsedStageSources::parse(Arc::new(sources), roots, Vec::new())?;
    let request = parsed.into_request(TARGET, purpose, primary)?;
    crate::test_support::type_authored_source_for_test(
        request,
        &mut crate::CompileTimings::default(),
    )
}

fn try_typed(supplied: &str, program: &str) -> Result<Fixture, Vec<diagnostics::Diagnostic>> {
    let typed = receiving_source(
        supplied,
        program,
        source_provenance::DependencyScope::Product,
    )?;
    let defaults = retain_provider_defaults(ProviderPlanningContext::new(
        &typed,
        source_provenance::DependencyScope::Product,
    ))?;
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .expect("source main")
        .symbol;
    let prepared = crate::test_support::preparation::prepare_typed_trees(typed, &[], &[])?;
    Ok(Fixture {
        source: prepared,
        entry,
        defaults,
    })
}

fn prepare(
    fixture: &Fixture,
    build: &[ProviderSelection],
    defaults: Vec<TargetProviderDefaults>,
) -> Result<PreparedProviderChoices, String> {
    prepare_with_source_defaults(fixture.source.trees(), &[TARGET], build, &[], &defaults)
        .map_err(first)
}

fn judge(fixture: &Fixture, choices: &PreparedProviderChoices) -> AcceptedTerminalSource {
    crate::test_support::judge_terminal_source(
        fixture.source.clone(),
        TerminalSourceCompilation {
            declaration_metadata: &[],
            supplies: &[],
            entries: &[fixture.entry],
            subjects: &[Some(package())],
            purpose: source_provenance::DependencyScope::Product,
            selected: &choices.declaration_roots,
            calling: Some(crate::test_support::lowering::CallingPolicySelections {
                source: fixture.source.trees(),
                applications: &choices.calling_applications,
                entries: &[],
            }),
            providers: Some(
                crate::test_support::lowering::provider_selections::ProviderSelections {
                    source: fixture.source.trees(),
                    choices: &choices.selections,
                },
            ),
            bindings: &[],
        },
        terminal_codec::TerminalPublication::Portable,
    )
    .unwrap_or_else(|refused| panic!("the source route must accept: {refused:?}"))
}

/// Native planning binds its provider choices to this published program identity.
fn assert_published(accepted: &AcceptedTerminalSource, file_name: &str) {
    let artifact = accepted
        .produce(Default::default(), &mut crate::CompileTimings::default())
        .expect("accepted source publishes")
        .into_portable_publication()
        .expect("portable publication");
    if let Some(directory) = std::env::var_os("OMEGA_PROVIDER_TEST_OUTPUT") {
        let path = std::path::Path::new(&directory)
            .join(file_name)
            .with_extension("new.psi");
        std::fs::write(&path, artifact.to_bytes())
            .unwrap_or_else(|error| panic!("could not write {}: {error}", path.display()));
    }
    assert_eq!(
        artifact.view().identity(),
        programs::open(crate::test_support::fixture_repository::root(), file_name)
            .view()
            .identity()
    );
}

fn assert_unsupplied(accepted: &AcceptedTerminalSource, file_name: &str) {
    let entry = accepted
        .entry()
        .expect("fixture main is an executable entry");
    let reached = accepted
        .boundary_call_reach(entry, &[], Some(TARGET))
        .unwrap()
        .boundaries;
    let closed = accepted
        .close_logical_provider_reach(TARGET, Some(entry), &reached)
        .expect("the source retains the unsupplied requirement");
    assert_eq!(closed.requirements(), reached);
    assert_eq!(reached.len(), 1);
    assert!(closed.selection().providers().is_empty());
    assert_published(accepted, file_name);
}

fn selected_provider(fixture: &Fixture, choices: &PreparedProviderChoices, name: &str) {
    let [choice] = choices.selections.as_slice() else {
        panic!("one exact chosen provider")
    };
    let source = fixture.source.trees();
    let declared = source
        .conformances()
        .iter()
        .find(|declaration| source.symbols.name(declaration.symbol) == name)
        .expect("the source declares this named map");
    assert_eq!(
        choice.provider,
        source.implementation_value_head(declared.symbol)
    );
    assert_eq!(choice.target, TARGET);
}

#[test]
fn changing_main_changes_the_published_program() {
    let fixture = typed(
        &format!("{NAMED_SLOT}{NATIVE_DEFAULT}"),
        "machine helper() {\n}\nmachine main() {\n    helper();\n}\n",
    );
    let choices = prepare(&fixture, &[], fixture.defaults.clone()).unwrap();
    assert_published(&judge(&fixture, &choices), programs::UNREACHED_HELPER);
}

#[test]
fn a_data_type_cannot_supply_a_provider_default() {
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
        !accepted.module().interfaces.provider_candidates.is_empty(),
        "retained conformance is real"
    );
    assert!(
        accepted
            .boundary_call_reach(accepted.entry().unwrap(), &[], Some(TARGET))
            .unwrap()
            .boundaries
            .is_empty()
    );
    assert_published(&accepted, programs::UNREACHED_DEFAULT);
}

#[test]
fn a_reached_requirement_settles_to_its_one_declared_default() {
    let fixture = typed(&format!("{NAMED_SLOT}{NATIVE_DEFAULT}"), REACHES);
    let choices = prepare(&fixture, &[], fixture.defaults.clone()).unwrap();
    selected_provider(&fixture, &choices, "Native");
    let accepted = judge(&fixture, &choices);
    assert_published(&accepted, programs::REACHED_DEFAULT);
}

#[test]
fn a_reached_requirement_is_published_without_a_provider() {
    let fixture = typed(SLOT, REACHES);
    let choices = prepare(&fixture, &[], fixture.defaults.clone()).unwrap();
    let accepted = judge(&fixture, &choices);
    assert_unsupplied(&accepted, programs::MISSING);
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
    assert!(
        refusal.contains("conflicting providers on target")
            && refusal.contains("the target package declares"),
        "{refusal}"
    );
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
    assert_published(&accepted, programs::OVERRIDE);

    defaults[0].target = Some("windows_x86_64".to_owned());
    let choices = prepare(&fixture, &[], defaults).unwrap();
    let [windows] = choices.selections.as_slice() else {
        panic!("the source retains exactly the Windows choice");
    };
    assert_eq!(windows.target, "windows_x86_64");
    let accepted = judge(&fixture, &choices);
    assert_unsupplied(&accepted, programs::FOREIGN_DEFAULT);
}

#[test]
fn interpreted_build_checks_and_executes_the_provider_chosen_for_its_source() {
    use crate::compile_product::provider_declarations::capture_provider_selection;
    use source_provenance::DependencyScope;
    use terminal_interpreter::{
        TerminalExecutionResult, TerminalExecutionStatus, TerminalProgram, TerminalScalarValue,
    };

    let library = format!(
        "public boundary trait Service {{ machine go() -> u32 reaches none crashes none; }}
         public Native satisfies Service {{ machine go() -> u32 {{ 17u32 }} }}
         public Other satisfies Service {{ machine go() -> u32 {{ 23u32 }} }}
         data NativeDefaults {{}}
         data OtherDefaults {{}}
         {NATIVE_DEFAULT}{OTHER_DEFAULT}"
    );
    let main = "machine main() -> u32 { Service::go() }";
    let product = typed(&library, main);
    let mut choices = product
        .defaults
        .iter()
        .flat_map(|group| &group.selections)
        .map(|row| {
            let source = product.source.trees();
            let expected = match source.symbols.name(row.provider_type.symbol) {
                "Native" => 17,
                "Other" => 23,
                name => panic!("unexpected provider {name}"),
            };
            (
                capture_provider_selection(source, row).expect("exact source declarations"),
                expected,
            )
        });
    let (first, first_result) = choices.next().unwrap();
    let (second, second_result) = choices.next().unwrap();
    let receiver = receiving_source(&library, main, DependencyScope::Build).unwrap();
    let entry = receiver
        .machines()
        .iter()
        .find(|m| m.name.as_str() == "main")
        .unwrap()
        .symbol;
    let changed = receiving_source(
        &library.replace("17u32", "18u32"),
        main,
        DependencyScope::Build,
    )
    .unwrap();
    let changed_entry = changed
        .machines()
        .iter()
        .find(|m| m.name.as_str() == "main")
        .unwrap()
        .symbol;
    let selected = crate::InterpretedProviderSelections {
        overrides: vec![first.clone()],
        defaults: vec![],
    };
    let Err(failure) = crate::CheckedBuildInvocation::judge(
        &changed,
        changed_entry,
        &[Some(package())],
        None,
        None,
        &selected,
    ) else {
        panic!("changed provider source was accepted for execution")
    };
    assert!(
        failure
            .diagnostics
            .iter()
            .any(|d| d.message.contains("absent or changed"))
    );
    for (chosen, fallback, expected) in [
        (&first, &second, first_result),
        (&second, &first, second_result),
    ] {
        let providers = crate::InterpretedProviderSelections {
            overrides: vec![chosen.clone()],
            defaults: vec![fallback.clone()],
        };
        let evaluated = crate::CheckedBuildInvocation::judge(
            &receiver,
            entry,
            &[Some(package())],
            None,
            None,
            &providers,
        )
        .unwrap()
        .invocation
        .prepare_publication()
        .unwrap()
        .publish()
        .unwrap()
        .1;
        let artifact = evaluated.view();
        let checked =
            psi_provider_selections::logical_reach::admit_interpreted_providers(artifact).unwrap();
        let program = TerminalProgram::borrow(artifact);
        let supplies = program
            .admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                installation: Some(&checked),
                native_code_layouts: None,
                host_imports: &[],
                external_completions: &[],
                entry: None,
                x86_flags: None,
                memory_ordering: None,
                target_machines: None,
                target_layouts: None,
            })
            .unwrap();
        let mut instance = program
            .instantiate(terminal_interpreter::TerminalInstantiation {
                installation: Some(&supplies),
                ..Default::default()
            })
            .unwrap();
        let result = instance
            .invoke(&mut build_model::work::unsponsored_meter(Some(10_000)))
            .unwrap();
        let TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Unsigned(value),
                ..
            },
        )) = result
        else {
            panic!("expected selected implementation to return: {result:?}");
        };
        assert_eq!(value, expected);
    }
}
