mod children;
mod names;

use std::sync::Arc;

use source::SourceMap;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{
    SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder, builtin_function_symbols,
    builtin_type_symbols,
};

use crate::symbols::symbol_table::children::{
    insert_builtin_type_symbol_children, insert_conformance_symbol_children,
    insert_data_symbol_children, insert_domain_symbol_children, insert_machine_symbol_children,
    insert_measure_symbol_children, insert_operator_symbol_children,
    insert_proposition_symbol_children, insert_trait_symbol_children,
};
use crate::symbols::symbol_table::names::{
    measure_symbol_name, measure_symbol_seed, operator_symbol_name, operator_symbol_seed,
    symbol_seed,
};

pub(super) fn extend_symbol_table(
    program: &mut SymbolResolvedTrees,
    sources: Arc<SourceMap>,
    source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    roots: crate::lowerer::RootWatermarks,
    const_declarations: &[crate::lowerer::PendingConstDeclaration],
) {
    let has_sources = true;
    let mut extension = std::mem::take(&mut program.symbols)
        .begin_extension(Some(sources), source_scoped_top_level_bindings);

    for index in roots.domain_definitions..program.domain_definitions.len() {
        let definition = program.domain_definitions[index].clone();
        let symbol = extension.insert_top_level([symbol_seed(
            SymbolKind::Domain,
            &definition.name,
            has_sources,
        )])[0];
        insert_domain_symbol_children(&mut extension, program, symbol, &definition, has_sources);
        program.domain_definitions[index].symbol = symbol;
    }
    for index in roots.data_definitions..program.data_definitions.len() {
        let definition = program.data_definitions[index].clone();
        let symbol = extension.insert_top_level([symbol_seed(
            SymbolKind::Data,
            &definition.name,
            has_sources,
        )])[0];
        insert_data_symbol_children(&mut extension, program, symbol, &definition, has_sources);
        program.data_definitions[index].symbol = symbol;
    }
    for index in roots.conformances..program.conformances.len() {
        let conformance = program.conformances[index].clone();
        let Some(alias) = &conformance.alias else {
            continue;
        };
        let symbol =
            extension.insert_top_level([symbol_seed(SymbolKind::Conformance, alias, has_sources)])
                [0];
        insert_conformance_symbol_children(
            &mut extension,
            program,
            symbol,
            &conformance,
            has_sources,
        );
        program.conformances[index].symbol = symbol;
    }
    for index in roots.machines..program.machines.len() {
        let machine = program.machines[index].clone();
        let symbol = extension.insert_top_level([symbol_seed(
            SymbolKind::Machine,
            &machine.name,
            has_sources,
        )])[0];
        program.machines[index].symbol = symbol;
    }
    for index in roots.propositions..program.propositions.len() {
        let proposition = program.propositions[index].clone();
        let symbol = extension.insert_top_level([symbol_seed(
            SymbolKind::Proposition,
            &proposition.name,
            has_sources,
        )])[0];
        insert_proposition_symbol_children(
            &mut extension,
            program,
            symbol,
            &proposition,
            has_sources,
        );
        program.propositions[index].symbol = symbol;
    }
    for index in roots.operators..program.operators.len() {
        let operator = program.operators[index].clone();
        let name = operator_symbol_name(program, &operator);
        let symbol = extension.insert_top_level([operator_symbol_seed(
            program,
            &operator,
            &name,
            has_sources,
        )])[0];
        insert_operator_symbol_children(&mut extension, program, symbol, &operator, has_sources);
        program.operators[index].symbol = symbol;
    }
    for index in roots.measures..program.measures.len() {
        let measure = program.measures[index].clone();
        let name = measure_symbol_name(program, &measure);
        let symbol = extension.insert_top_level([measure_symbol_seed(
            program,
            &measure,
            &name,
            has_sources,
        )])[0];
        insert_measure_symbol_children(&mut extension, symbol, &measure, has_sources);
        program.measures[index].symbol = symbol;
    }
    for index in roots.traits..program.traits.len() {
        let definition = program.traits[index].clone();
        let symbol = extension.insert_top_level([symbol_seed(
            SymbolKind::Trait,
            &definition.name,
            has_sources,
        )])[0];
        insert_trait_symbol_children(&mut extension, program, symbol, &definition, has_sources);
        program.traits[index].symbol = symbol;
    }
    for index in roots.wire_schemas..program.wire_schemas.len() {
        let schema = program.wire_schemas[index].clone();
        let symbol = extension.insert_top_level([symbol_seed(
            SymbolKind::WireSchema,
            &schema.name,
            has_sources,
        )])[0];
        program.wire_schemas[index].symbol = symbol;
    }
    for (offset, declaration) in const_declarations
        .iter()
        .enumerate()
        .skip(roots.const_declarations)
    {
        let symbol = extension.insert_top_level([(
            SymbolKind::Const,
            SymbolNameRef::OwnedSource {
                value: declaration.semantic_name.as_str(),
                source_span: declaration.source_span,
            },
        )])[0];
        program.const_declarations[offset].symbol = symbol;
    }

    program.symbols = extension.finish();
}

pub(super) fn build_symbol_table(
    program: &SymbolResolvedTrees,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    const_declarations: &[crate::lowerer::PendingConstDeclaration],
    has_authored_modules: bool,
) -> SymbolTable {
    let has_sources = sources.is_some() || has_authored_modules;
    let root_operator_names = program
        .operators
        .iter()
        .map(|operator| operator_symbol_name(program, operator))
        .collect::<Vec<_>>();
    let measure_names = program
        .measures
        .iter()
        .map(|measure| measure_symbol_name(program, measure))
        .collect::<Vec<_>>();
    let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
        sources,
        source_scoped_top_level_bindings,
    );
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let root_children = builder.insert_children(
        root,
        builtin_type_symbols()
            .into_iter()
            .chain(builtin_function_symbols())
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
                program
                    .machines
                    .iter()
                    .map(|machine| symbol_seed(SymbolKind::Machine, &machine.name, has_sources)),
            )
            .chain(program.propositions.iter().map(|proposition| {
                symbol_seed(SymbolKind::Proposition, &proposition.name, has_sources)
            }))
            .chain(
                root_operator_names
                    .iter()
                    .zip(program.operators.iter())
                    .map(|(name, operator)| {
                        operator_symbol_seed(program, operator, name, has_sources)
                    }),
            )
            .chain(
                measure_names
                    .iter()
                    .zip(program.measures.iter())
                    .map(|(name, measure)| {
                        measure_symbol_seed(program, measure, name, has_sources)
                    }),
            )
            .chain(program.traits.iter().map(|trait_definition| {
                symbol_seed(SymbolKind::Trait, &trait_definition.name, has_sources)
            }))
            .chain(program.wire_schemas.iter().map(|wire_schema| {
                symbol_seed(SymbolKind::WireSchema, &wire_schema.name, has_sources)
            }))
            .chain(const_declarations.iter().map(|declaration| {
                // Exact declaration joins also run without a source-map owner.
                // Retaining parser coordinates grants no package authority.
                (
                    SymbolKind::Const,
                    SymbolNameRef::OwnedSource {
                        value: declaration.semantic_name.as_str(),
                        source_span: declaration.source_span,
                    },
                )
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
        if conformance.alias.is_some()
            && let Some(conformance_symbol) = root_children.next()
        {
            insert_conformance_symbol_children(
                &mut builder,
                program,
                conformance_symbol,
                conformance,
                has_sources,
            );
        }
    }
    // Machine children require namespace-aware attachment selection. Their root
    // headers already exist; publish children after the namespace is installed.
    for _ in &program.machines {
        let _ = root_children.next();
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
    for measure in &program.measures {
        if let Some(measure_symbol) = root_children.next() {
            insert_measure_symbol_children(&mut builder, measure_symbol, measure, has_sources);
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

/// Machine children and the later assignment pass must use the same selected
/// attachment. Inherited field slots cannot be guessed from a qualified name:
/// omitting them would shift the state's actual symbol slot during assignment.
pub(super) fn insert_selected_machine_children(
    program: &SymbolResolvedTrees,
    table: SymbolTable,
    sources: Option<Arc<SourceMap>>,
    first_machine: usize,
    has_sources: bool,
) -> SymbolTable {
    let roots = || table.child_handles(table.root()).into_iter().flatten();
    let selections = roots()
        .filter(|handle| table.get(*handle).kind == SymbolKind::Machine)
        .zip(program.machines.iter())
        .skip(first_machine)
        .map(|(symbol, machine)| {
            let selected = machine.attached_data.as_ref().and_then(|attached| {
                table.find_top_level_by_name_and_kinds_from_source(
                    attached.as_str(),
                    &[SymbolKind::Data],
                    attached.source_span(),
                )
            });
            let owner = selected.and_then(|selected| {
                roots()
                    .filter(|handle| table.get(*handle).kind == SymbolKind::Data)
                    .zip(program.data_definitions.iter())
                    .find_map(|(handle, definition)| (handle == selected).then_some(definition))
            });
            (symbol, machine, owner)
        })
        .collect::<Vec<_>>();
    let mut extension = table.begin_extension(sources.clone(), Vec::new());
    for (symbol, machine, owner) in selections {
        insert_machine_symbol_children(
            &mut extension,
            program,
            symbol,
            machine,
            owner,
            has_sources,
            sources.as_deref(),
        );
    }
    extension.finish()
}
