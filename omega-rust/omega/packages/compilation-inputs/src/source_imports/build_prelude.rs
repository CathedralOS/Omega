//! Select the toolchain Build prelude without capturing source-stage execution.

use super::SourceStorage;

/// Source and the Build occurrence needing a scoped alias. The binding target
/// comes from the admitted prelude, never a prediction of parser order.
pub(crate) struct BuildPrelude {
    pub source: String,
    pub alias_for: Option<source::SourceId>,
}

/// Supply the prelude only for the selected build entry when that source has
/// no authored Build declaration. A product-local Build retains its identity;
/// only the build source receives an alias to the toolchain declaration.
pub(crate) fn prepare_build_prelude(
    source_storage: &SourceStorage,
    build_source_id: Option<source::SourceId>,
) -> Option<BuildPrelude> {
    let mut has_build_machine = false;
    let mut build_source_declares_build_data = false;
    let mut program_declares_build_data = false;
    for (_, file) in source_storage.source_roots.iter() {
        let is_build_file = Some(file.source_id) == build_source_id;
        for root_item in &file.root_items {
            match source_storage.syntax_trees.root_item(*root_item) {
                syntax_trees::item::Item::Machine(machine)
                    if is_build_file
                        && machine.name.as_str(&source_storage.syntax_trees) == "build" =>
                {
                    has_build_machine = true;
                }
                syntax_trees::item::Item::Data(data)
                    if data.name.as_str(&source_storage.syntax_trees) == "Build" =>
                {
                    if is_build_file {
                        build_source_declares_build_data = true;
                    } else {
                        program_declares_build_data = true;
                    }
                }
                _ => {}
            }
        }
    }
    let inject_build_vocabulary = has_build_machine && !build_source_declares_build_data;
    if !inject_build_vocabulary {
        return None;
    }

    Some(BuildPrelude {
        source: build_declarations::build_prelude_source(),
        alias_for: build_source_id.filter(|_| program_declares_build_data),
    })
}
