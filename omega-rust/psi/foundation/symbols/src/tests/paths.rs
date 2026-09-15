use super::{main_console_symbols, main_entry_symbols};
use crate::SymbolHandle;

#[test]
fn stores_paths_as_handle_spans() {
    let mut symbols = main_entry_symbols();
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
    let mut symbols = main_entry_symbols();
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
    let symbols = main_entry_symbols();
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
    let symbols = main_console_symbols();
    let write_line = symbols
        .find_descendant_by_path(symbols.root(), ["main", "console", "write_line"])
        .expect("write_line should resolve");

    assert_eq!(
        symbols.display_path(write_line, "::"),
        "main::console::write_line"
    );
    assert_eq!(symbols.display_path(SymbolHandle::invalid(), "::"), "");
}
