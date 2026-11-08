// Conditional toolchain `<build-prelude>` selection (`prepare_build_prelude`).
//
// Injects `build_declarations::build_prelude_source()` only when the selected `build`
// entry source declares a `build` machine without declaring its own `Build` data type.
// When another file in the program declares a product-local `Build` data type, we
// also emit a `SourceScopedTopLevelBinding` alias (`alias_for`) so `build.omg` binds
// to the toolchain `Build` declaration while the product's own `Build` retains its
// separate identity.

use super::SourceStorage;

pub(crate) struct BuildPrelude {
    pub source: String,                      // Toolchain build-prelude source text.
    pub alias_for: Option<source::SourceId>, // Optional `build.omg` `SourceId` requiring a scoped `Build` alias.
}

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
