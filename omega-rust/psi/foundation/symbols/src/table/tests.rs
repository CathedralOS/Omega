//! Symbol-table unit tests, extracted from `table` so the storage and
//! lookup machinery stays the file's headline.

use super::*;

#[cfg(test)]
mod builtin_function_identity_tests {
    use super::{BuiltinFunction, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder};
    use crate::{builtin_function_symbols, builtin_type_symbols};

    fn builtin_symbol_table(
        function_symbols: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'static>)>,
    ) -> SymbolTable {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        builder.insert_children(
            root,
            builtin_type_symbols().into_iter().chain(function_symbols),
        );
        builder.finish()
    }

    #[test]
    fn every_installed_builtin_function_round_trips_by_exact_slot() {
        let symbols = builtin_symbol_table(builtin_function_symbols());

        for (function, _) in BuiltinFunction::ALL {
            let symbol = symbols
                .builtin_function_symbol(function)
                .expect("compiler builtin function must occupy its fixed root slot");
            assert_eq!(symbols.builtin_function_for_symbol(symbol), Some(function));
        }
    }
}

#[cfg(test)]
mod source_name_storage_tests {
    use super::{Arc, SourceMap, SourceSpan, SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use source::Span;

    #[test]
    fn builder_and_extension_names_borrow_retained_source_bytes() {
        let mut sources = SourceMap::default();
        let source_id = sources.add("main.omg".into(), "Ready".into()).source_id;
        let source_span = SourceSpan::new(source_id, Span::new(0, 5));
        let sources = Arc::new(sources);
        let expected_pointer = sources.text_at(source_span).as_ptr();
        let seed = SymbolNameRef::OwnedSource {
            value: "Ready",
            source_span,
        };
        let mut builder = SymbolTableBuilder::with_sources(Some(sources.clone()));
        let root = builder.insert_root(SymbolKind::Root, seed);
        let child = SymbolTableBuilder::child_handles(
            builder.insert_children(root, [(SymbolKind::Machine, seed)]),
        )
        .next()
        .expect("authored child");
        let mut extension = builder.finish().begin_extension(Some(sources), Vec::new());
        let additional = extension.insert_top_level([(SymbolKind::Machine, seed)])[0];
        let additional_child = SymbolTableBuilder::child_handles(
            extension.insert_children(additional, [(SymbolKind::State, seed)]),
        )
        .next()
        .expect("extension child");
        // Dropping the map must not lose the retained spelling or its source allocation.
        let table = extension
            .finish()
            .begin_extension(None, Vec::new())
            .finish();
        for symbol in [root, child, additional, additional_child] {
            assert_eq!(table.name(symbol), "Ready");
            assert_eq!(table.name(symbol).as_ptr(), expected_pointer);
            assert_eq!(table.symbol_source_span(symbol), Some(source_span));
        }
    }
}
