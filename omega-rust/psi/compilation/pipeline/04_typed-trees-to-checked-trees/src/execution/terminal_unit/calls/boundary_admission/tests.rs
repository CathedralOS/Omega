use super::provider_attachment_receiver_matches;
use crate::semantic::calls::find_call_site;
use source::{SourceMap, SourceOrigin};
use std::path::PathBuf;
use typed_trees::TypedTrees;

const CORE_SERVICE_OMG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/core/binding.omg"
));

/// Two modules that each declare `Reader` and `Runner` with a `reader` field.
/// Only the resolved symbols tell `left::Runner` from `right::Runner`; their
/// fields carry different `Reader` requirements.
fn same_leaf_attachments() -> TypedTrees {
    let module_source = |module: &str| {
        format!(
            "module {module};
             pub boundary trait Reader {{ machine read() -> i32; }}
             pub data Runner {{ reader: Binding<Reader>; }}
             pub machine Runner::run(&mut self) reaches Reader
             invokes Reader;
             {{
                 let value: i32 = self.reader.read();
             }}"
        )
    };
    let left = module_source("left");
    let right = module_source("right");
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
    let left_id = sources.add("left.omg".into(), left.clone()).source_id;
    let right_id = sources.add("right.omg".into(), right.clone()).source_id;
    crate::tests::front_end::typed_program_from_source_map(
        sources,
        &[
            (service, CORE_SERVICE_OMG),
            (left_id, left.as_str()),
            (right_id, right.as_str()),
        ],
    )
}

fn requirement(program: &TypedTrees, path: &str) -> symbols::SymbolHandle {
    program
        .traits()
        .iter()
        .find(|definition| program.symbols.display_path(definition.symbol, "::") == path)
        .unwrap_or_else(|| panic!("missing boundary `{path}`"))
        .symbol
}

#[test]
fn provider_receivers_read_the_resolved_attachment_not_a_same_leaf_sibling() {
    let program = same_leaf_attachments();
    for (module, sibling) in [("left", "right"), ("right", "left")] {
        let machine = program
            .machines()
            .iter()
            .find(|machine| {
                program
                    .symbols
                    .display_path(machine.attached_data_symbol, "::")
                    == format!("{module}::Runner")
            })
            .unwrap_or_else(|| panic!("missing `{module}::Runner::run`"));
        let state = program
            .machine_states(machine)
            .first()
            .expect("the entry state");
        let call_site = find_call_site(&program, machine.symbol, state.symbol, 0, 0)
            .expect("the `self.reader.read()` call site");
        assert!(
            provider_attachment_receiver_matches(
                &program,
                machine,
                &call_site,
                requirement(&program, &format!("{module}::Reader")),
            ),
            "`{module}::Runner::run` routes through its own `reader` field"
        );
        assert!(
            !provider_attachment_receiver_matches(
                &program,
                machine,
                &call_site,
                requirement(&program, &format!("{sibling}::Reader")),
            ),
            "`{module}::Runner::run` must not answer to `{sibling}::Reader`"
        );
    }
}
