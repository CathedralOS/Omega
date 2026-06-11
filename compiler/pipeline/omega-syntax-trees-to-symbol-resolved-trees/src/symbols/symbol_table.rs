mod children;
mod names;

use std::sync::Arc;

use omega_core::source::SourceMap;
use omega_core::symbols::{
    SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder, builtin_function_symbols,
    builtin_type_symbols,
};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use crate::symbols::symbol_table::children::{
    insert_builtin_type_symbol_children, insert_data_symbol_children,
    insert_domain_symbol_children, insert_machine_symbol_children, insert_operator_symbol_children,
    insert_platform_symbol_children, insert_trait_symbol_children,
};
use crate::symbols::symbol_table::names::{operator_symbol_name, symbol_seed};

pub(super) fn build_symbol_table(
    program: &SymbolResolvedTrees,
    sources: Option<Arc<SourceMap>>,
) -> SymbolTable {
    let has_sources = sources.is_some();
    let root_operator_names = program
        .operators
        .iter()
        .map(|operator| operator_symbol_name(program, operator))
        .collect::<Vec<_>>();
    let mut builder = SymbolTableBuilder::with_sources(sources);
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let root_children =
        builder.insert_children(
            root,
            builtin_type_symbols()
                .into_iter()
                .chain(builtin_function_symbols())
                .chain(program.invariant_definitions.iter().map(|invariant| {
                    symbol_seed(SymbolKind::Invariant, &invariant.name, has_sources)
                }))
                .chain(
                    program
                        .domain_definitions
                        .iter()
                        .map(|domain| symbol_seed(SymbolKind::Domain, &domain.name, has_sources)),
                )
                .chain(
                    program
                        .data_definitions
                        .iter()
                        .map(|data| symbol_seed(SymbolKind::Data, &data.name, has_sources)),
                )
                .chain(
                    program.machines.iter().map(|machine| {
                        symbol_seed(SymbolKind::Machine, &machine.name, has_sources)
                    }),
                )
                .chain(
                    root_operator_names
                        .iter()
                        .map(|name| (SymbolKind::Operator, SymbolNameRef::Borrowed(name.as_str()))),
                )
                .chain(
                    program.platforms.iter().map(|platform| {
                        symbol_seed(SymbolKind::Platform, &platform.name, has_sources)
                    }),
                )
                .chain(program.traits.iter().map(|trait_definition| {
                    symbol_seed(SymbolKind::Trait, &trait_definition.name, has_sources)
                }))
                .chain(program.wire_schemas.iter().map(|wire_schema| {
                    symbol_seed(SymbolKind::WireSchema, &wire_schema.name, has_sources)
                })),
        );
    let mut root_children = SymbolTableBuilder::child_handles(root_children);

    for builtin_type in builtin_type_symbols() {
        if let Some(builtin_symbol) = root_children.next() {
            insert_builtin_type_symbol_children(&mut builder, builtin_symbol, builtin_type);
        }
    }
    for _ in 0..builtin_function_symbols().len() {
        let _ = root_children.next();
    }
    for _ in &program.invariant_definitions {
        let _ = root_children.next();
    }
    for domain in &program.domain_definitions {
        if let Some(domain_symbol) = root_children.next() {
            insert_domain_symbol_children(
                &mut builder,
                program,
                domain_symbol,
                domain,
                has_sources,
            );
        }
    }
    for data_definition in &program.data_definitions {
        if let Some(data_symbol) = root_children.next() {
            insert_data_symbol_children(
                &mut builder,
                program,
                data_symbol,
                data_definition,
                has_sources,
            );
        }
    }
    for machine in &program.machines {
        if let Some(machine_symbol) = root_children.next() {
            insert_machine_symbol_children(
                &mut builder,
                program,
                machine_symbol,
                machine,
                has_sources,
            );
        }
    }
    for operator in &program.operators {
        if let Some(operator_symbol) = root_children.next() {
            insert_operator_symbol_children(
                &mut builder,
                program,
                operator_symbol,
                operator,
                has_sources,
            );
        }
    }
    for platform in &program.platforms {
        if let Some(platform_symbol) = root_children.next() {
            insert_platform_symbol_children(
                &mut builder,
                program,
                platform_symbol,
                platform,
                has_sources,
            );
        }
    }
    for trait_definition in &program.traits {
        if let Some(trait_symbol) = root_children.next() {
            insert_trait_symbol_children(
                &mut builder,
                program,
                trait_symbol,
                trait_definition,
                has_sources,
            );
        }
    }

    builder.finish()
}
