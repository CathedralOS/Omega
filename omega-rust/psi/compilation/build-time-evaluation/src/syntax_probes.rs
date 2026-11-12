/*
    Temporary frontend normalization, symbol resolution, and seeded typing
    probes for compile-time constant and access-view evaluation.

    We construct isolated syntax and typed probe programs that preserve original
    source and import custody without leaking temporary symbols or placeholder
    layouts into the published program.
*/

use crate::build_time_evaluation::type_references;
use crate::unique::push_unique;
use diagnostics::Diagnostic;
use std::sync::Arc;

mod evaluation {
    use crate::build_time_evaluation::EvaluationDomain;
    use diagnostics::Diagnostic;
    use source::SourceSpan;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_to_seeded_base;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::machine::MachineBodyStatus;
    use typed_trees::references::{DeclarationSelection, collect_selected_program_references};

    pub(crate) fn prepare_evaluation(
        frontier: syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
        probe_sources: &[SourceSpan],
        expression_sources: &[SourceSpan],
        declaration_roots: &[SymbolHandle],
        predecessor: Option<symbol_resolved_trees_to_typed_trees::SeededTypingBase>,
    ) -> Result<(TypedTrees, EvaluationDomain), Vec<Diagnostic>> {
        let preparation = prepare_evaluation_inputs(
            frontier,
            probe_sources,
            expression_sources,
            declaration_roots,
            predecessor,
        )?;
        Ok((preparation.typed, preparation.domain))
    }

    pub(crate) struct PreparedEvaluationInputs {
        pub(crate) typed: TypedTrees,
        pub(crate) domain: EvaluationDomain,
        pub(crate) pending_type_inputs:
            Vec<(SymbolHandle, syntax_trees::types::TypeReferenceHandle)>,
    }

    impl PreparedEvaluationInputs {
        pub(crate) fn domain_for_roots(&self, roots: &[SymbolHandle]) -> EvaluationDomain {
            self.domain_for_inputs(roots, &[])
        }

        pub(crate) fn domain_for_inputs(
            &self,
            roots: &[SymbolHandle],
            expressions: &[SourceSpan],
        ) -> EvaluationDomain {
            Self::domain_for_program(&self.typed, roots, expressions)
        }

        pub(crate) fn domain_for_program(
            program: &TypedTrees,
            roots: &[SymbolHandle],
            expressions: &[SourceSpan],
        ) -> EvaluationDomain {
            let admission = crate::BuildTimeAdmissionPlan::infer(program, None);
            Self::domain_for_admission(program, &admission, roots, expressions)
        }

        pub(crate) fn domain_for_admission(
            program: &TypedTrees,
            admission: &crate::BuildTimeAdmissionPlan,
            roots: &[SymbolHandle],
            expressions: &[SourceSpan],
        ) -> EvaluationDomain {
            let mut execution = roots.to_vec();
            admission.append_execution_dependencies(program, &mut execution);
            let (work, selection) = declaration_dependencies(
                program,
                &admission.operators,
                &execution,
                expressions,
                &[],
            );
            EvaluationDomain::selected(work, selection)
        }
    }

    pub(crate) fn prepare_evaluation_inputs(
        frontier: syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
        probe_sources: &[SourceSpan],
        expression_sources: &[SourceSpan],
        declaration_roots: &[SymbolHandle],
        predecessor: Option<symbol_resolved_trees_to_typed_trees::SeededTypingBase>,
    ) -> Result<PreparedEvaluationInputs, Vec<Diagnostic>> {
        prepare_evaluation_inputs_with_owners(
            frontier,
            probe_sources,
            expression_sources,
            declaration_roots,
            &[],
            predecessor,
        )
    }

    pub(crate) fn prepare_owned_probe_evaluation(
        frontier: syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
        owners: &[SymbolHandle],
        expression_sources: &[SourceSpan],
        predecessor: Option<symbol_resolved_trees_to_typed_trees::SeededTypingBase>,
    ) -> Result<PreparedEvaluationInputs, Vec<Diagnostic>> {
        prepare_evaluation_inputs_with_owners(
            frontier,
            &[],
            expression_sources,
            &[],
            owners,
            predecessor,
        )
    }

    pub(crate) fn prepare_call_free_probe_typing(
        mut frontier: syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
        owners: &[SymbolHandle],
        predecessor: Option<symbol_resolved_trees_to_typed_trees::SeededTypingBase>,
    ) -> Result<TypedTrees, Vec<Diagnostic>> {
        if owners.iter().any(|owner| {
            !frontier
                .trees()
                .machines
                .iter()
                .any(|machine| machine.symbol == *owner)
        }) {
            return Err(vec![Diagnostic::error(
                "evaluation probe lost its exact declaration owner",
            )]);
        }
        loop {
            let demand = frontier.body_demand(owners, &[])?;
            if demand.deferred_machines.is_empty() {
                break;
            }
            frontier = frontier
                .complete_machine_bodies(&demand.deferred_machines)?
                .into_resolved_frontier();
        }
        let typing = if let Some(predecessor) = predecessor {
            let extension = frontier.into_extension()?;
            let (typing, _) = predecessor.append_deferred(extension)?;
            typing
        } else {
            lower_symbol_resolved_trees_to_seeded_base(frontier.shared_trees())
                .map_err(|diagnostic| vec![diagnostic])?
        };
        Ok(typing.into_typed())
    }

    fn prepare_evaluation_inputs_with_owners(
        mut frontier: syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
        probe_sources: &[SourceSpan],
        expression_sources: &[SourceSpan],
        declaration_roots: &[SymbolHandle],
        owners: &[SymbolHandle],
        predecessor: Option<symbol_resolved_trees_to_typed_trees::SeededTypingBase>,
    ) -> Result<PreparedEvaluationInputs, Vec<Diagnostic>> {
        let mut roots = owners.to_vec();
        if owners.iter().any(|owner| {
            !frontier
                .trees()
                .machines
                .iter()
                .any(|machine| machine.symbol == *owner)
        }) {
            return Err(vec![Diagnostic::error(
                "evaluation probe lost its exact declaration owner",
            )]);
        }
        for reference in probe_sources {
            let mut candidates = frontier.trees().machines.iter().filter(|machine| {
                frontier.trees().symbols.symbol_source_span(machine.symbol) == Some(*reference)
            });
            let (Some(machine), None) = (candidates.next(), candidates.next()) else {
                return Err(vec![
                    Diagnostic::error("evaluation probe lost its unique declaration owner")
                        .with_source_span(*reference),
                ]);
            };
            roots.push(machine.symbol);
        }

        let expressions = if expression_sources.is_empty() {
            Vec::new()
        } else {
            frontier
                .trees()
                .tables
                .bodies
                .expressions
                .iter_expressions()
                .filter(|(handle, _)| {
                    expression_sources.contains(
                        &frontier
                            .trees()
                            .tables
                            .bodies
                            .expressions
                            .source_span(*handle),
                    )
                })
                .map(|(handle, _)| handle)
                .collect::<Vec<_>>()
        };
        for reference in expression_sources {
            if !expressions.iter().any(|handle| {
                frontier
                    .trees()
                    .tables
                    .bodies
                    .expressions
                    .source_span(*handle)
                    == *reference
            }) {
                return Err(vec![
                    Diagnostic::error("evaluation endpoint lost its resolved occurrence")
                        .with_source_span(*reference),
                ]);
            }
        }
        let execution = loop {
            let demand = frontier.body_demand(&roots, &expressions)?;
            if demand.deferred_machines.is_empty() {
                break demand.machines;
            }
            frontier = frontier
                .complete_machine_bodies(&demand.deferred_machines)?
                .into_resolved_frontier();
        };
        let mut typing = if let Some(predecessor) = predecessor {
            let extension = frontier.into_extension()?;
            let (typing, rebased) = predecessor.append_deferred(extension)?;
            frontier = rebased;
            typing
        } else {
            lower_symbol_resolved_trees_to_seeded_base(frontier.shared_trees())
                .map_err(|diagnostic| vec![diagnostic])?
        };
        let mut execution = execution;
        validate_declaration_roots(typing.typed(), declaration_roots)?;
        let (work, selection) = loop {
            while !validation::resolve_named_result_overloads(typing.typed_mut())?.is_empty() {}
            typed_trees_to_typed_trees::bind_pre_specialization_authored_selections(
                typing.typed_mut(),
            )
            .map_err(|diagnostic| vec![diagnostic])?;
            let operators = validation::derive_pre_flow_operator_selections(typing.typed());
            crate::BuildTimeAdmissionPlan::infer_execution_dependencies(
                typing.typed(),
                &operators,
                &mut execution,
            );
            let (work, selection) = declaration_dependencies(
                typing.typed(),
                &operators,
                &execution,
                expression_sources,
                declaration_roots,
            );
            let requested = typing
                .typed()
                .machines()
                .iter()
                .filter(|machine| {
                    machine.body_status == MachineBodyStatus::Deferred
                        && selection.contains(machine.symbol)
                })
                .map(|machine| machine.symbol)
                .collect::<Vec<_>>();
            if requested.is_empty() {
                break (work, selection);
            }
            execution.extend(requested.iter().copied());
            frontier = typing.join_prepared_allocations(frontier)?;
            let completion = frontier.complete_machine_bodies(&requested)?;
            (typing, frontier) = typing.complete_bodies(completion)?;
        };
        let typed = typing.into_typed();
        Ok(PreparedEvaluationInputs {
            typed,
            domain: EvaluationDomain::selected(work, selection),
            pending_type_inputs: frontier.into_pending_type_inputs(),
        })
    }

    fn validate_declaration_roots(
        program: &TypedTrees,
        roots: &[SymbolHandle],
    ) -> Result<(), Vec<Diagnostic>> {
        if roots.is_empty() {
            return Ok(());
        }
        let declarations = typed_trees::references::collect_program_declarations(program);
        if roots
            .iter()
            .any(|symbol| !symbol.is_valid() || !declarations.contains(symbol))
        {
            return Err(vec![Diagnostic::error(
                "evaluation declaration root lost its retained owner",
            )]);
        }
        Ok(())
    }

    fn dependency_machine(program: &TypedTrees, dependency: SymbolHandle) -> SymbolHandle {
        if !dependency.is_valid() {
            return SymbolHandle::invalid();
        }
        match program.symbols.get(dependency).kind {
            symbols::SymbolKind::Machine => dependency,
            symbols::SymbolKind::State => program
                .machine_holding_state(dependency)
                .map_or_else(SymbolHandle::invalid, |machine| machine.symbol),
            _ => SymbolHandle::invalid(),
        }
    }

    fn declaration_dependencies(
        program: &TypedTrees,
        operators: &checked_trees::CheckedOperatorFacts,
        execution: &[SymbolHandle],
        expression_sources: &[SourceSpan],
        declaration_roots: &[SymbolHandle],
    ) -> (DeclarationSelection, DeclarationSelection) {
        let declarations = typed_trees::references::collect_program_declarations(program);
        let roster = DeclarationSelection::from_symbols(declarations.iter().copied());
        let mut selection = DeclarationSelection::from_symbols(execution.iter().copied());
        let mut endpoint_owners =
            typed_trees::references::collect_expression_source_owners(program, expression_sources);
        let mut has_endpoint_owners = !expression_sources.is_empty();
        for &symbol in declaration_roots {
            if program.symbols.get(symbol).kind == symbols::SymbolKind::Machine {
                endpoint_owners.include(symbol);
                has_endpoint_owners = true;
            } else {
                selection.include(symbol);
            }
        }
        let endpoint_declarations =
            if has_endpoint_owners && endpoint_owners != DeclarationSelection::from_symbols([]) {
                let endpoint_refs = collect_selected_program_references(program, &endpoint_owners);
                for mut symbol in endpoint_refs.symbols {
                    let body = dependency_machine(program, symbol);
                    while symbol.is_valid() {
                        if roster.contains(symbol)
                            && !endpoint_owners.contains(symbol)
                            && (program.symbols.get(symbol).kind != symbols::SymbolKind::Machine
                                || symbol == body)
                        {
                            selection.include(symbol);
                        }
                        symbol = program.symbols.get(symbol).parent;
                    }
                }
                endpoint_refs.declarations
            } else {
                Vec::new()
            };
        let mut closure = typed_trees::references::ReferenceClosure::new();
        loop {
            let mut changed = false;
            for family in &program.target_machine_families {
                if selection.contains(family.canonical)
                    || family
                        .members
                        .iter()
                        .any(|member| selection.contains(member.machine))
                {
                    if !selection.contains(family.canonical) {
                        selection.include(family.canonical);
                        changed = true;
                    }
                    for member in &family.members {
                        if !selection.contains(member.machine) {
                            selection.include(member.machine);
                            changed = true;
                        }
                    }
                }
            }
            let (mut referenced_symbols, new_expressions) = closure.extend(program, &selection);
            if !new_expressions.is_empty() {
                for (_, operator_use) in operators
                    .uses
                    .iter()
                    .filter(|(_, operator_use)| new_expressions.contains(&operator_use.expression))
                {
                    referenced_symbols.insert(operator_use.selected_operator_symbol);
                    referenced_symbols.extend(
                        operators
                            .candidates
                            .span_or_empty(operator_use.candidates)
                            .iter()
                            .map(|candidate| candidate.operator_symbol),
                    );
                }
                referenced_symbols.extend(
                    operators
                        .named_uses
                        .iter()
                        .filter(|(_, operator_use)| {
                            new_expressions.contains(&operator_use.expression)
                        })
                        .map(|(_, operator_use)| operator_use.selected_operator_symbol),
                );
            }
            for mut symbol in referenced_symbols {
                let body = dependency_machine(program, symbol);
                while symbol.is_valid() {
                    if roster.contains(symbol)
                        && !selection.contains(symbol)
                        && (program.symbols.get(symbol).kind != symbols::SymbolKind::Machine
                            || symbol == body)
                    {
                        selection.include(symbol);
                        changed = true;
                    }
                    symbol = program.symbols.get(symbol).parent;
                }
            }
            if !changed {
                let mut work = selection.clone();
                for symbol in endpoint_declarations {
                    work.include(symbol);
                }
                return (work, selection);
            }
        }
    }
}
pub(crate) use evaluation::PreparedEvaluationInputs;
pub(crate) use evaluation::{
    prepare_call_free_probe_typing, prepare_evaluation, prepare_owned_probe_evaluation,
};

pub(crate) fn retain_pending_const_inputs(
    probe: &mut syntax_trees::SyntaxTrees,
    arguments: &[syntax_trees::types::TypeReferenceHandle],
) {
    use syntax_trees::item::Item;
    use syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};

    if arguments.is_empty() {
        return;
    }
    for &argument in arguments {
        probe
            .type_references
            .retain_pending_const_argument_uses(argument);
    }
    for item in probe.root_item_handles().to_vec() {
        let Item::Const(mut definition) = probe.root_item(item).clone() else {
            continue;
        };
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = type_references(probe)
            .type_reference(definition.type_reference)
            .clone()
        else {
            continue;
        };
        let retained = type_references(probe)
            .constraints(constraints)
            .iter()
            .filter(|constraint| {
                let TypeConstraintNode::Domain(domain) = constraint else {
                    return true;
                };
                !type_references(probe)
                    .type_reference_handles(domain.arguments)
                    .iter()
                    .any(|argument| arguments.contains(argument))
            })
            .cloned()
            .collect::<Vec<_>>();
        if retained.len() == constraints.len() {
            continue;
        }
        definition.type_reference = if retained.is_empty() {
            base_type
        } else {
            let retained = probe.type_references.insert_constraints(retained);
            probe
                .type_references
                .insert_constrained(base_type, retained)
        };
        probe.items.replace_item(item, Item::Const(definition));
    }
}

pub(crate) struct InitializerSelectionInputs {
    pub(crate) authored: syntax_trees::SyntaxTrees,
    pub(crate) probe: syntax_trees::SyntaxTrees,
}

pub(crate) fn initializer_selection_inputs(
    mut authored: syntax_trees::SyntaxTrees,
    context: &crate::BuildTimeSources<'_>,
) -> Result<InitializerSelectionInputs, Vec<Diagnostic>> {
    let arguments = syntax_trees_to_symbol_resolved_trees::pre_resolution::closed_data_const_argument_expressions(
        &mut authored,
        context.sources.clone(),
        context.source_scoped_top_level_bindings.to_vec(),
        context.retained_base(),
        context.declaration_syntax,
    )?;
    let mut probe = authored.clone();
    crate::const_evaluation::const_qualifications::defer_unproved(&mut probe);
    retain_pending_const_inputs(
        &mut probe,
        &arguments
            .iter()
            .map(|(argument, _, _)| *argument)
            .collect::<Vec<_>>(),
    );
    Ok(InitializerSelectionInputs { authored, probe })
}

pub(crate) fn normalize_declared_types(
    syntax: syntax_trees::SyntaxTrees,
    context: &crate::build_time_evaluation::BuildTimeSources<'_>,
) -> Result<syntax_trees::SyntaxTrees, Vec<diagnostics::Diagnostic>> {
    syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_declared_types(
        normalization_request(
            syntax,
            context.sources.clone(),
            context.source_scoped_top_level_bindings,
            context.retained_base(),
            [
                context.declaration_syntax,
                context.retained_declaration_syntax,
            ],
        ),
    )
}

pub(crate) fn normalize_generic_data(
    syntax_trees: syntax_trees::SyntaxTrees,
    sources: Option<Arc<source::SourceMap>>,
    source_scoped_top_level_bindings: &[symbols::SourceScopedTopLevelBinding],
    retained_base: Option<&symbol_resolved_trees::SymbolResolvedTrees>,
    declarations: [Option<&syntax_trees::SyntaxTrees>; 2],
) -> Result<syntax_trees::SyntaxTrees, Vec<Diagnostic>> {
    syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_generic_data(
        normalization_request(
            syntax_trees,
            sources,
            source_scoped_top_level_bindings,
            retained_base,
            declarations,
        ),
    )
}

pub(crate) fn normalize_initializer_probe_data(
    syntax_trees: syntax_trees::SyntaxTrees,
    sources: Option<Arc<source::SourceMap>>,
    source_scoped_top_level_bindings: &[symbols::SourceScopedTopLevelBinding],
    retained_base: Option<&symbol_resolved_trees::SymbolResolvedTrees>,
    declaration_forests: [Option<&syntax_trees::SyntaxTrees>; 2],
    declarations: &[syntax_trees::item::ItemHandle],
) -> Result<syntax_trees::SyntaxTrees, Vec<Diagnostic>> {
    syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_initializer_probe_data(
        normalization_request(
            syntax_trees,
            sources,
            source_scoped_top_level_bindings,
            retained_base,
            declaration_forests,
        ),
        declarations,
    )
}

pub(crate) fn extend_normalization_owners(
    syntax: &syntax_trees::SyntaxTrees,
    declarations: &mut Vec<syntax_trees::item::ItemHandle>,
    owners: &[source::SourceSpan],
) -> Result<(), Vec<Diagnostic>> {
    let mut pending = syntax.root_item_handles().to_vec();
    let mut lexical_items = Vec::new();
    while let Some(item) = pending.pop() {
        lexical_items.push(item);
        match syntax.root_item(item) {
            syntax_trees::item::Item::Machine(machine) => {
                pending.extend(machine.nested_machines.handles());
            }
            syntax_trees::item::Item::Conformance(conformance) => {
                for member in syntax.items.conformance_members(conformance.members) {
                    use syntax_trees::item::ConformanceMember;
                    let machine = match member {
                        ConformanceMember::Machine(machine)
                        | ConformanceMember::Synthesized(machine)
                        | ConformanceMember::TraitDefault { machine, .. } => machine,
                        ConformanceMember::Selection { .. } => continue,
                    };
                    pending.extend(machine.nested_machines.handles());
                }
            }
            _ => {}
        }
    }
    for owner in owners {
        let matches = lexical_items
            .iter()
            .copied()
            .filter(|item| {
                let name = match syntax.root_item(*item) {
                    syntax_trees::item::Item::Const(definition) => &definition.name,
                    syntax_trees::item::Item::Data(definition) => &definition.name,
                    syntax_trees::item::Item::TypeAlias(definition) => &definition.name,
                    syntax_trees::item::Item::Machine(definition) => &definition.name,
                    syntax_trees::item::Item::Conformance(definition) => {
                        let Some(alias) = &definition.alias else {
                            return false;
                        };
                        alias
                    }
                    _ => return false,
                };
                name.source_span() == *owner
            })
            .collect::<Vec<_>>();
        let [item] = matches.as_slice() else {
            return Err(vec![
                Diagnostic::error("initializer normalization lost exact declaration ownership")
                    .with_source_span(*owner),
            ]);
        };
        push_unique(declarations, *item);
    }
    Ok(())
}

fn normalization_request<'base>(
    syntax: syntax_trees::SyntaxTrees,
    sources: Option<Arc<source::SourceMap>>,
    bindings: &[symbols::SourceScopedTopLevelBinding],
    retained_base: Option<&'base symbol_resolved_trees::SymbolResolvedTrees>,
    declarations: [Option<&'base syntax_trees::SyntaxTrees>; 2],
) -> syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest<'base> {
    let has_sources = sources.is_some();
    syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest {
        syntax,
        sources,
        top_level_bindings: if has_sources {
            bindings.to_vec()
        } else {
            Vec::new()
        },
        retained_base: if has_sources { retained_base } else { None },
        declaration_syntax: declarations[0],
        retained_declaration_syntax: declarations[1],
    }
}
