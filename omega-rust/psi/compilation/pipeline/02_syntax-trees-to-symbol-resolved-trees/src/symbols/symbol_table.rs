mod children;
mod names;

use std::sync::Arc;

use source::SourceMap;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{
    FixedCoreDeclaration, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder,
    builtin_function_symbols, builtin_type_symbols, fixed_core_declaration_symbols,
};

use crate::symbols::symbol_table::children::{
    insert_builtin_type_symbol_children, insert_conformance_symbol_children,
    insert_data_symbol_children, insert_domain_symbol_children, insert_machine_symbol_children,
    insert_mathematical_symbol_children, insert_measure_symbol_children,
    insert_operator_symbol_children, insert_proposition_symbol_children,
    insert_trait_symbol_children,
};
use crate::symbols::symbol_table::names::{
    machine_symbol_seed, measure_symbol_name, measure_symbol_seed, operator_symbol_name,
    operator_symbol_seed, symbol_seed,
};

pub(super) fn extend_symbol_table(
    program: &mut SymbolResolvedTrees,
    sources: Arc<SourceMap>,
    source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    roots: crate::resolution::lowerer::RootWatermarks,
    const_declarations: &[crate::resolution::lowerer::PendingConstDeclaration],
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
        let symbol = extension.insert_top_level([machine_symbol_seed(&machine, has_sources)])[0];
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
    for index in roots.mathematical_definitions..program.mathematical_definitions.len() {
        let definition = program.mathematical_definitions[index].clone();
        let symbol = extension.insert_top_level([symbol_seed(
            SymbolKind::MathematicalDefinition,
            &definition.name,
            has_sources,
        )])[0];
        insert_mathematical_symbol_children(
            &mut extension,
            program,
            symbol,
            &definition,
            has_sources,
        );
        program.mathematical_definitions[index].symbol = symbol;
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
    const_declarations: &[crate::resolution::lowerer::PendingConstDeclaration],
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
            .chain(fixed_core_declaration_symbols())
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
                    .map(|machine| machine_symbol_seed(machine, has_sources)),
            )
            .chain(program.propositions.iter().map(|proposition| {
                symbol_seed(SymbolKind::Proposition, &proposition.name, has_sources)
            }))
            .chain(program.mathematical_definitions.iter().map(|definition| {
                symbol_seed(
                    SymbolKind::MathematicalDefinition,
                    &definition.name,
                    has_sources,
                )
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
    // The fixed `core::*` mathematical declarations own their root slots
    // positionally (see `FixedCoreDeclaration::ordinal`); they carry no
    // children.
    for _ in 0..FixedCoreDeclaration::COUNT {
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
    for definition in &program.mathematical_definitions {
        if let Some(definition_symbol) = root_children.next() {
            insert_mathematical_symbol_children(
                &mut builder,
                program,
                definition_symbol,
                definition,
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

/// Add the selected declaration cohort's own children after root identities
/// exist. Carrier qualification supplies neither field aliases nor storage.
pub(super) fn insert_selected_machine_children(
    program: &SymbolResolvedTrees,
    table: SymbolTable,
    sources: Option<Arc<SourceMap>>,
    first_machine: usize,
    has_sources: bool,
) -> SymbolTable {
    let selections = table
        .child_handles(table.root())
        .into_iter()
        .flatten()
        .filter(|handle| table.get(*handle).kind == SymbolKind::Machine)
        .zip(program.machines.iter())
        .skip(first_machine)
        .collect::<Vec<_>>();
    let mut extension = table.begin_extension(sources.clone(), Vec::new());
    for (symbol, machine) in selections {
        insert_machine_symbol_children(
            &mut extension,
            program,
            symbol,
            machine,
            has_sources,
            sources.as_deref(),
        );
    }
    extension.finish()
}

/// Append implementation-owned children after the whole declaration cohort
/// has identities. A later completion uses this same operation with a selected
/// cohort: existing state/parameter/local handles are never assigned again.
pub(super) fn append_body_symbols(
    program: &mut SymbolResolvedTrees,
    selected_machines: &[symbols::SymbolHandle],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    if selected_machines.iter().any(|selected| {
        !selected.is_valid()
            || !program
                .machines
                .iter()
                .any(|machine| machine.symbol == *selected)
    }) {
        return Err(vec![diagnostics::Diagnostic::error(
            "body symbol completion requires an existing machine declaration",
        )]);
    }
    let has_sources = program.symbols.source_files().next().is_some();
    let mut new_states = Vec::new();
    let declarations = &mut program.tables.declarations;
    for machine in program.roots.machines.iter() {
        if !selected_machines.contains(&machine.symbol) {
            continue;
        }
        for state_handle in declarations
            .machine_state_handles
            .span_or_empty(machine.states)
        {
            let state = declarations.machine_states.get_mut(*state_handle);
            if !state.symbol.is_valid() {
                let inserted = program.symbols.append_children(
                    machine.symbol,
                    [symbol_seed(SymbolKind::State, &state.name, has_sources)],
                );
                state.symbol = inserted.start();
                new_states.push((machine.type_parameters, *state_handle));
            }
            let parameters = declarations
                .state_parameters
                .span_mut_or_empty(state.parameters);
            let inserted = program.symbols.append_children(
                state.symbol,
                parameters
                    .iter()
                    .filter(|parameter| !parameter.symbol.is_valid())
                    .map(|parameter| {
                        symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)
                    }),
            );
            for (parameter, symbol) in parameters
                .iter_mut()
                .filter(|parameter| !parameter.symbol.is_valid())
                .zip(SymbolTableBuilder::child_handles(inserted))
            {
                parameter.symbol = symbol;
            }
            let statements = declarations
                .state_statements
                .span_mut_or_empty(state.statements);
            append_local_symbols(&mut program.symbols, state.symbol, statements);
            let graphs: Vec<_> = statements
                .iter()
                .filter_map(|statement| {
                    if let symbol_resolved_trees::statement::Statement::Assembly(handle) = statement
                    {
                        Some(*handle)
                    } else {
                        None
                    }
                })
                .collect();
            for handle in graphs {
                let graph = declarations.assembly_graphs.get(handle);
                for block in declarations.assembly_blocks.span_or_empty(graph.blocks) {
                    append_local_symbols(
                        &mut program.symbols,
                        state.symbol,
                        declarations
                            .state_statements
                            .span_mut_or_empty(block.instructions),
                    );
                }
            }
        }
    }
    // All synthetic headers now exist, including forward state references.
    // Only their signatures need this assignment; authored headers retain the
    // exact declarations and type handles established by the first phase.
    for (type_parameters, state_handle) in new_states {
        let state = declarations.machine_states.get_mut(state_handle);
        let type_parameters = declarations
            .data_type_parameters
            .span_or_empty(type_parameters);
        for parameter in declarations
            .state_parameters
            .span_mut_or_empty(state.parameters)
        {
            super::type_references::assign_type_reference_symbol_with_locals_and_constraints(
                &program.symbols,
                &mut declarations.child_type_references,
                &program.tables.types.constraints,
                type_parameters,
                &mut parameter.type_reference,
            );
        }
        if let Some(return_type) = &mut state.return_type {
            super::type_references::assign_type_reference_symbol_with_locals_and_constraints(
                &program.symbols,
                &mut declarations.child_type_references,
                &program.tables.types.constraints,
                type_parameters,
                return_type,
            );
        }
    }
    Ok(())
}

/// The same append-only declaration allocation serves ordinary bodies and
/// one-shot lexical input preparation. Already indexed locals keep identity.
pub(super) fn append_local_symbols(
    symbols: &mut SymbolTable,
    state: symbols::SymbolHandle,
    statements: &mut [symbol_resolved_trees::statement::Statement],
) {
    use symbol_resolved_trees::statement::Statement;
    let has_sources = symbols.source_files().next().is_some();
    let inserted = symbols.append_children(
        state,
        statements.iter().filter_map(|statement| match statement {
            Statement::LocalData(local) if !local.symbol.is_valid() => {
                Some(symbol_seed(SymbolKind::Local, &local.name, has_sources))
            }
            _ => None,
        }),
    );
    let locals = statements
        .iter_mut()
        .filter_map(|statement| match statement {
            Statement::LocalData(local) if !local.symbol.is_valid() => Some(local),
            _ => None,
        });
    for (local, symbol) in locals.zip(SymbolTableBuilder::child_handles(inserted)) {
        local.symbol = symbol;
    }
}
