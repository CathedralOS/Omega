//! Check which defaults settlement predicts before the first preparation.
//! Settlement confirms a prediction on the lowered walk; these tests pin what
//! the prediction itself reads from the typed source.
use super::{ChosenDefaultLayout, predict_default_layouts};
use crate::compile_product::provider_declarations::{
    ProviderPlanningContext, ProviderSelection, TargetProviderDefaults, retain_provider_defaults,
};
use std::sync::Arc;

const TARGETS: [&str; 2] = ["linux_x86_64", "windows_x86_64"];

// `Native` supplies `Service`, and its body calls `Host`, which `NativeHost`
// supplies: a target reaches `NativeHost` only through `Native`'s body. No
// entry reaches `Spare`. The defaults name `Host` before `Service`.
const PROVIDERS: &str = "public boundary trait Service {\n    machine go() reaches none crashes none;\n}\npublic boundary trait Host {\n    machine put() reaches none crashes none;\n}\npublic boundary trait Spare {\n    machine idle() reaches none crashes none;\n}\npublic Native satisfies Service {\n    machine go() reaches none crashes none {\n        Host::put();\n    }\n}\npublic NativeHost satisfies Host {\n    machine put() reaches none crashes none {}\n}\npublic SpareHost satisfies Spare {\n    machine idle() reaches none crashes none {}\n}\n";
const DEFAULTS: &str = "data Defaults {}\nmachine Defaults::provider_defaults(defaults: @mutable Defaults) {\n    Defaults::select_provider::<Host, NativeHost>(defaults);\n    Defaults::select_provider::<Service, Native>(defaults);\n    Defaults::select_provider::<Spare, SpareHost>(defaults);\n}\n";
// A second default for `Service`, so the two compete on every target.
const COMPETING: &str = "public Other satisfies Service {\n    machine go() reaches none crashes none {}\n}\ndata OtherDefaults {}\nmachine OtherDefaults::provider_defaults(defaults: @mutable OtherDefaults) {\n    OtherDefaults::select_provider::<Service, Other>(defaults);\n}\n";
const REACHES: &str = "machine main() {\n    Service::go();\n}\n";

struct Fixture {
    typed: crate::source_state::TypedTrees,
    entry: symbols::SymbolHandle,
    defaults: Vec<TargetProviderDefaults>,
}

fn package() -> semantic_vocabulary::PackageKeyIdentity {
    semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap()
}

fn typed(supplied: &str, program: &str) -> Fixture {
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
                source_provenance::DependencyScope::Product,
            )
            .source_id;
        roots.push(id);
    }
    let primary = roots[0];
    let parsed = crate::ParsedStageSources::parse(Arc::new(sources), roots, Vec::new())
        .expect("parse the fixture");
    let request = parsed
        .into_request(
            TARGETS[0],
            source_provenance::DependencyScope::Product,
            primary,
            Default::default(),
        )
        .expect("request the fixture");
    let typed = crate::test_support::type_authored_source_for_test(
        request,
        &mut crate::CompileTimings::default(),
    )
    .expect("type the fixture");
    let defaults = retain_provider_defaults(ProviderPlanningContext::new(
        &typed,
        source_provenance::DependencyScope::Product,
    ))
    .expect("harvest source defaults");
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .expect("source main")
        .symbol;
    Fixture {
        typed,
        entry,
        defaults,
    }
}

fn predict(fixture: &Fixture, build: &[ProviderSelection]) -> Vec<(String, String)> {
    names(&predict_default_layouts(
        &fixture.typed,
        &[fixture.entry],
        &TARGETS,
        build,
        &[],
        &fixture.defaults,
    ))
}

fn names(layouts: &[ChosenDefaultLayout]) -> Vec<(String, String)> {
    layouts
        .iter()
        .map(|layout| {
            (
                layout.target.clone(),
                layout.selection.provider_type.authored_path.clone(),
            )
        })
        .collect()
}

fn on_every_target(providers: &[&str]) -> Vec<(String, String)> {
    TARGETS
        .iter()
        .flat_map(|target| {
            providers
                .iter()
                .map(|provider| ((*target).to_owned(), (*provider).to_owned()))
        })
        .collect()
}

fn default_named<'a>(fixture: &'a Fixture, provider: &str) -> &'a ProviderSelection {
    fixture
        .defaults
        .iter()
        .flat_map(|row| &row.selections)
        .find(|selection| selection.provider_type.authored_path == provider)
        .expect("the fixture declares this default")
}

/// The prediction follows `Native`'s body to `Host`, as settlement does only
/// after a second preparation, and lists each target's defaults in the order
/// the source declares them, not the order it found them.
#[test]
fn a_default_reached_only_through_another_defaults_body_is_predicted() {
    let fixture = typed(&format!("{PROVIDERS}{DEFAULTS}"), REACHES);
    assert_eq!(
        predict(&fixture, &[]),
        on_every_target(&["NativeHost", "Native"])
    );
}

/// The same source with an entry that calls nothing predicts no default, so
/// no default's layout is prepared before a target chooses it.
#[test]
fn an_entry_that_reaches_no_requirement_predicts_nothing() {
    let fixture = typed(&format!("{PROVIDERS}{DEFAULTS}"), "machine main() {\n}\n");
    assert_eq!(predict(&fixture, &[]), Vec::new());
}

/// A Build selection outranks the default for its slot, so that default is
/// not predicted. The prediction still follows the selected provider's body.
#[test]
fn a_build_selection_outranks_the_default_but_its_body_is_followed() {
    let fixture = typed(&format!("{PROVIDERS}{DEFAULTS}"), REACHES);
    let build = default_named(&fixture, "Native").clone();
    assert_eq!(
        predict(&fixture, std::slice::from_ref(&build)),
        on_every_target(&["NativeHost"])
    );
}

/// Two defaults for one slot compete, so settlement chooses neither and the
/// prediction follows neither body.
#[test]
fn competing_defaults_predict_neither_provider() {
    let fixture = typed(&format!("{PROVIDERS}{DEFAULTS}{COMPETING}"), REACHES);
    assert_eq!(predict(&fixture, &[]), Vec::new());
}

/// A default keyed to one target is predicted only there.
#[test]
fn a_target_keyed_default_is_predicted_only_on_its_target() {
    let mut fixture = typed(&format!("{PROVIDERS}{DEFAULTS}"), REACHES);
    for row in &mut fixture.defaults {
        row.target = Some(TARGETS[1].to_owned());
    }
    assert_eq!(
        predict(&fixture, &[]),
        vec![
            (TARGETS[1].to_owned(), "NativeHost".to_owned()),
            (TARGETS[1].to_owned(), "Native".to_owned()),
        ]
    );
}
