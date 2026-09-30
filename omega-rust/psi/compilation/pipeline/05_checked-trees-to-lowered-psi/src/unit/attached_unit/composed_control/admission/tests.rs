use checked_trees::CheckedTrees;
use source::{SourceMap, SourceOrigin};
use std::path::PathBuf;

const CORE_SERVICE_OMG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/core/binding.omg"
));

/// The same-leaf twins' shape: `left` and `right` each declare `Reader`,
/// `Runner` and a composed `Runner::run`, and only the resolved symbols tell
/// the two `Runner`s apart. The states report through one `shared::Sink`, as
/// the twins report through std's one `Console`.
fn same_leaf_runners() -> CheckedTrees {
    const SHARED: &str = "module shared; pub boundary trait Sink { machine emit(value: i32); }";
    let module_source = |module: &str, expected: u32| {
        format!(
            "module {module};
             use shared::Sink;
             pub boundary trait Reader {{ machine read() -> i32; }}
             pub data Runner {{ reader: Binding<Reader>; sink: Binding<Sink>; }}
             pub machine Runner::run(&mut self) reaches Reader, Sink
             invokes Reader;
             invokes Sink;
             {{
                 let value: i32 = self.reader.read();
                 transition value == {expected} {{ true -> deliver(value) _ -> wrong(value) }}
                 state deliver(&mut self, observed: i32) {{ self.sink.emit(observed); }}
                 state wrong(&mut self, observed: i32) {{ self.sink.emit(65); }}
             }}"
        )
    };
    let left = module_source("left", 11);
    let right = module_source("right", 37);
    let mut sources = SourceMap::default();
    let service = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/binding.omg"),
            CORE_SERVICE_OMG.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let shared = sources
        .add("shared.omg".into(), SHARED.to_owned())
        .source_id;
    let left_id = sources.add("left.omg".into(), left.clone()).source_id;
    let right_id = sources.add("right.omg".into(), right.clone()).source_id;
    let mut typed = crate::front_end::typed_program_from_source_map(
        sources,
        &[
            (service, CORE_SERVICE_OMG),
            (shared, SHARED),
            (left_id, left.as_str()),
            (right_id, right.as_str()),
        ],
    );
    let authorizations = typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .map(
            |definition| typed_trees::typed_trees::FusedServiceErasureAuthorization {
                requirement: definition.symbol,
            },
        )
        .collect();
    typed
        .bind_fused_service_erasures(authorizations)
        .expect("fixture authorization");
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("both modules' runners check")
}

#[test]
fn composed_attachment_is_the_resolved_owner_not_a_same_leaf_sibling() {
    let checked = same_leaf_runners();
    for module in ["left", "right"] {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| {
                checked
                    .symbols
                    .display_path(machine.attached_data_symbol, "::")
                    == format!("{module}::Runner")
            })
            .unwrap_or_else(|| panic!("missing `{module}::Runner::run`"));
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .find(|plan| plan.machine == machine.symbol)
            .unwrap_or_else(|| panic!("`{module}::Runner::run` has a composed Unit plan"));
        let attachment = super::exact_attachment(&checked, plan)
            .unwrap_or_else(|error| panic!("`{module}::Runner` attachment: {error:?}"))
            .expect("an attached machine keeps its attachment");
        assert_eq!(
            attachment.identity,
            format!("named(name({module}::Runner))")
        );

        let lowered = crate::lower_machine(
            &checked,
            crate::TerminalMachineSelection::Symbol(machine.symbol),
        )
        .unwrap_or_else(|error| panic!("`{module}::Runner::run` lowers: {error:?}"));
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("`{module}::Runner::run` verifies: {error:?}"));
    }
}
