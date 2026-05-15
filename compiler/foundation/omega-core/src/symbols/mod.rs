mod definition;
mod kind;
mod name;
mod path;
mod symbol;
mod table;

pub use definition::{SymbolDefinition, builtin_type_symbol_definitions};
pub use kind::SymbolKind;
pub use name::{SymbolName, SymbolNameRef, SymbolNameStorageKind};
pub use path::SymbolPath;
pub use symbol::{Symbol, SymbolHandle, SymbolNameHandle, SymbolSpan};
pub use table::{SymbolNameStorageCounts, SymbolTable};

#[cfg(test)]
mod tests {
    use super::{SymbolDefinition, SymbolHandle, SymbolKind, SymbolTable};

    #[test]
    fn invalid_symbol_resolves_to_dummy() {
        let symbols = SymbolTable::new();
        let invalid = SymbolHandle::invalid();

        assert_eq!(symbols.get(invalid).kind, SymbolKind::Unknown);
        assert_eq!(symbols.name(invalid), "");
    }

    #[test]
    fn stores_symbols_with_parent_handles() {
        let symbols = SymbolTable::from_definition(SymbolDefinition::with_children(
            SymbolKind::Root,
            "root",
            [SymbolDefinition::with_children(
                SymbolKind::Machine,
                "main",
                [SymbolDefinition::named(SymbolKind::State, "entry")],
            )],
        ));
        let root = symbols.root();
        let machine = symbols
            .find_child_by_name(root, "main")
            .expect("main should resolve");
        let state = symbols
            .find_child_by_name(machine, "entry")
            .expect("entry should resolve");

        assert_eq!(symbols.get(machine).parent, root);
        assert_eq!(symbols.get(state).parent, machine);
        assert_eq!(symbols.get(root).children.count(), 1);
        assert_eq!(symbols.get(machine).children.count(), 1);
        assert_eq!(symbols.name(state), "entry");
    }

    #[test]
    fn stores_paths_as_handle_spans() {
        let mut symbols = SymbolTable::from_definition(SymbolDefinition::with_children(
            SymbolKind::Root,
            "root",
            [SymbolDefinition::with_children(
                SymbolKind::Machine,
                "main",
                [SymbolDefinition::named(SymbolKind::State, "entry")],
            )],
        ));
        let root = symbols.root();
        let machine = symbols
            .find_child_by_name(root, "main")
            .expect("main should resolve");
        let state = symbols
            .find_child_by_name(machine, "entry")
            .expect("entry should resolve");
        let path = symbols.path_from_members(root, [machine, state]);

        assert_eq!(path.root, root);
        assert_eq!(symbols.path_members(path), &[machine, state]);
    }

    #[test]
    fn resolves_child_paths_by_sibling_walk() {
        let mut symbols = SymbolTable::from_definition(SymbolDefinition::with_children(
            SymbolKind::Root,
            "root",
            [SymbolDefinition::with_children(
                SymbolKind::Machine,
                "main",
                [SymbolDefinition::named(SymbolKind::State, "entry")],
            )],
        ));
        let root = symbols.root();
        let path = symbols.resolve_child_path(root, ["main", "entry"]);
        let names = symbols
            .path_members(path)
            .iter()
            .map(|symbol| symbols.name(*symbol))
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["main", "entry"]);
        let missing_path = symbols.resolve_child_path(root, ["main", "missing"]);
        assert!(symbols.path_members(missing_path).is_empty());
    }

    #[test]
    fn resolves_descendant_without_storing_path_members() {
        let symbols = SymbolTable::from_definition(SymbolDefinition::with_children(
            SymbolKind::Root,
            "root",
            [SymbolDefinition::with_children(
                SymbolKind::Machine,
                "main",
                [SymbolDefinition::named(SymbolKind::State, "entry")],
            )],
        ));
        let root = symbols.root();
        let entry = symbols
            .find_descendant_by_path(root, ["main", "entry"])
            .expect("entry should resolve");

        assert_eq!(symbols.name(entry), "entry");
        assert_eq!(
            symbols.find_descendant_by_path(root, ["main", "missing"]),
            None
        );
        assert_eq!(symbols.path_member_arena().len(), 0);
    }

    #[test]
    fn formats_symbol_display_path_from_parent_chain() {
        let symbols = SymbolTable::from_definition(SymbolDefinition::with_children(
            SymbolKind::Root,
            "root",
            [SymbolDefinition::with_children(
                SymbolKind::Machine,
                "main",
                [SymbolDefinition::with_children(
                    SymbolKind::Object,
                    "console",
                    [SymbolDefinition::named(SymbolKind::State, "write_line")],
                )],
            )],
        ));
        let write_line = symbols
            .find_descendant_by_path(symbols.root(), ["main", "console", "write_line"])
            .expect("write_line should resolve");

        assert_eq!(
            symbols.display_path(write_line, "::"),
            "main::console::write_line"
        );
        assert_eq!(symbols.display_path(SymbolHandle::invalid(), "::"), "");
    }

    #[test]
    fn child_ranges_are_exact_per_parent() {
        let symbols = SymbolTable::from_definition(SymbolDefinition::with_children(
            SymbolKind::Root,
            "root",
            [
                SymbolDefinition::with_children(
                    SymbolKind::Machine,
                    "main",
                    [
                        SymbolDefinition::named(SymbolKind::State, "entry"),
                        SymbolDefinition::named(SymbolKind::State, "running"),
                    ],
                ),
                SymbolDefinition::named(SymbolKind::Data, "Inventory"),
            ],
        ));
        let root = symbols.root();
        let root_children = symbols
            .child_handles(root)
            .expect("root children should resolve")
            .map(|child| symbols.name(child).to_owned())
            .collect::<Vec<_>>();
        let main = symbols
            .find_child_by_name(root, "main")
            .expect("main should resolve");
        let main_children = symbols
            .child_handles(main)
            .expect("main children should resolve")
            .map(|child| symbols.name(child).to_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            root_children,
            vec!["main".to_owned(), "Inventory".to_owned()]
        );
        assert_eq!(
            main_children,
            vec!["entry".to_owned(), "running".to_owned()]
        );
    }
}
