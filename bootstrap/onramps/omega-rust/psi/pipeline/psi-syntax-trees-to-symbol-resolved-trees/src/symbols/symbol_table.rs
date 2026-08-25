mod children;
mod names;

use std::sync::Arc;

use psi_source::SourceMap;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbols::{
    SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder, builtin_function_symbols,
    builtin_type_symbols,
};

use crate::symbols::symbol_table::children::{
    insert_builtin_type_symbol_children, insert_conformance_symbol_children,
    insert_data_symbol_children, insert_domain_symbol_children, insert_machine_symbol_children,
    insert_operator_symbol_children, insert_proposition_symbol_children,
    insert_trait_symbol_children,
};
use crate::symbols::symbol_table::names::{operator_symbol_name, symbol_seed};

pub(super) fn build_symbol_table(
    program: &SymbolResolvedTrees,
    sources: Option<Arc<SourceMap>>,
    const_declarations: &[crate::lowerer::PendingConstDeclaration],
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
                .chain(program.conformances.iter().filter_map(|conformance| {
                    conformance
                        .alias
                        .as_ref()
                        .map(|alias| symbol_seed(SymbolKind::Conformance, alias, has_sources))
                }))
                .chain(
                    program.machines.iter().map(|machine| {
                        symbol_seed(SymbolKind::Machine, &machine.name, has_sources)
                    }),
                )
                .chain(program.propositions.iter().map(|proposition| {
                    symbol_seed(SymbolKind::Proposition, &proposition.name, has_sources)
                }))
                .chain(
                    root_operator_names
                        .iter()
                        .map(|name| (SymbolKind::Operator, SymbolNameRef::Borrowed(name.as_str()))),
                )
                .chain(program.traits.iter().map(|trait_definition| {
                    symbol_seed(SymbolKind::Trait, &trait_definition.name, has_sources)
                }))
                .chain(program.wire_schemas.iter().map(|wire_schema| {
                    symbol_seed(SymbolKind::WireSchema, &wire_schema.name, has_sources)
                }))
                .chain(const_declarations.iter().map(|declaration| {
                    if has_sources {
                        (
                            SymbolKind::Const,
                            SymbolNameRef::OwnedSource {
                                value: declaration.semantic_name.as_str(),
                                source_span: declaration.source_span,
                            },
                        )
                    } else {
                        (
                            SymbolKind::Const,
                            SymbolNameRef::Borrowed(declaration.semantic_name.as_str()),
                        )
                    }
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
    for conformance in &program.conformances {
        if conformance.alias.is_some() {
            if let Some(conformance_symbol) = root_children.next() {
                insert_conformance_symbol_children(
                    &mut builder,
                    program,
                    conformance_symbol,
                    conformance,
                    has_sources,
                );
            }
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
    for proposition in &program.propositions {
        if let Some(proposition_symbol) = root_children.next() {
            insert_proposition_symbol_children(
                &mut builder,
                program,
                proposition_symbol,
                proposition,
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
