//! Fixtures shared by the symbol-table tests: entry, console and running
//! inventory symbol tables built from sourced declarations.

mod filtered_lookup;
mod hierarchy;
mod module_namespaces;
mod package_scope_separation;
mod paths;
mod provenance;
mod source_scoped_bindings;

use crate::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder};
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceMap, SourceOrigin, SourceSpan, Span};
use std::path::PathBuf;
use std::sync::Arc;

fn main_entry_symbols() -> SymbolTable {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let children = builder.insert_children(
        root,
        [(SymbolKind::Machine, SymbolNameRef::Borrowed("main"))],
    );
    let mut children = SymbolTableBuilder::child_handles(children);
    let main = children.next().expect("main should be present");
    builder.insert_children(
        main,
        [(SymbolKind::State, SymbolNameRef::Borrowed("entry"))],
    );

    builder.finish()
}

fn sourced_symbol_table<const N: usize>(
    metadata: [(SourceOrigin, Option<PackageKeyIdentity>); N],
) -> (SymbolTable, Vec<SymbolHandle>) {
    let mut sources = SourceMap::default();
    let source_spans = metadata.map(|(origin, package_identity)| {
        let source_id = sources
            .add_with_metadata(
                PathBuf::from(format!("source-{}.omg", sources.len())),
                String::from("machine"),
                PathBuf::from("package"),
                package_identity,
                origin,
            )
            .source_id;
        SourceSpan::new(source_id, Span::new(0, 7))
    });
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let children = builder.insert_children(
        root,
        source_spans.map(|source_span| (SymbolKind::Machine, SymbolNameRef::Source(source_span))),
    );
    let authored = SymbolTableBuilder::child_handles(children).collect();

    (builder.finish(), authored)
}

fn main_console_symbols() -> SymbolTable {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let children = builder.insert_children(
        root,
        [(SymbolKind::Machine, SymbolNameRef::Borrowed("main"))],
    );
    let mut children = SymbolTableBuilder::child_handles(children);
    let main = children.next().expect("main should be present");
    let children = builder.insert_children(
        main,
        [(SymbolKind::Object, SymbolNameRef::Borrowed("console"))],
    );
    let mut children = SymbolTableBuilder::child_handles(children);
    let console = children.next().expect("console should be present");
    builder.insert_children(
        console,
        [(SymbolKind::State, SymbolNameRef::Borrowed("write_line"))],
    );

    builder.finish()
}

fn main_running_inventory_symbols() -> SymbolTable {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let children = builder.insert_children(
        root,
        [
            (SymbolKind::Machine, SymbolNameRef::Borrowed("main")),
            (SymbolKind::Data, SymbolNameRef::Borrowed("Inventory")),
        ],
    );
    let mut children = SymbolTableBuilder::child_handles(children);
    let main = children.next().expect("main should be present");
    builder.insert_children(
        main,
        [
            (SymbolKind::State, SymbolNameRef::Borrowed("entry")),
            (SymbolKind::State, SymbolNameRef::Borrowed("running")),
        ],
    );

    builder.finish()
}
