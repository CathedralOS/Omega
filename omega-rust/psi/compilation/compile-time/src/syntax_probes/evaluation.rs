//! Evaluation roots demand dependencies without rewriting their declarations.
//!
//! Provisional const indices describe incomplete input, not a replacement program.
//! Clearing unrelated bodies or dropping return types makes that replacement
//! observable to typing and can authorize an invented implementation. Retain the
//! resolver's real pending bodies instead. Ordinary calls demand their exact
//! owners before typing; typed operational edges add selected overload bodies.
//! The retained trees keep signature-only owners alongside an exact execution
//! selection containing only complete bodies. The caller still owes ordinary
//! checking and invocation admission on that private execution projection.

use crate::build_time_evaluation::EvaluationDomain;
use diagnostics::Diagnostic;
use source::SourceSpan;
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

/// One preparation owns both its typed selections and the resolver's input
/// dependencies. These symbol handles must never be joined to another drive.
pub(crate) struct PreparedEvaluationInputs {
    pub(crate) typed: TypedTrees,
    pub(crate) domain: EvaluationDomain,
    pub(crate) pending_type_inputs: Vec<(SymbolHandle, syntax_trees::types::TypeReferenceHandle)>,
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

    /// Typed evaluation customers share the resolver probe's exact dependency
    /// closure. Backing tables retain their handles; only declaration demand changes.
    pub(crate) fn domain_for_program(
        program: &TypedTrees,
        roots: &[SymbolHandle],
        expressions: &[SourceSpan],
    ) -> EvaluationDomain {
        let admission = crate::BuildTimeAdmissionPlan::infer(program, None);
        Self::domain_for_admission(program, &admission, roots, expressions)
    }

    /// An admission owner already inferred this immutable program's execution
    /// edges. Reuse that association when selecting its checking premises;
    /// the resulting projection still owes ordinary preparation and checking.
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

/// Compiler-created probes retain exact resolved declaration identities. Their
/// authored spans locate diagnostics and selections, never instance jobs.
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

/// Call-free scalar probes only inspect their own typed expression and constant
/// declaration table, without needing an execution or checking domain closure.
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
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_to_seeded_base(
            frontier.shared_trees(),
        )
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

    // Endpoint roots live in declared types rather than synthetic machines.
    // Every resolved copy of that retained authored occurrence contributes its
    // selected dependencies; unrelated backing expressions contribute none.
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
        // Resolve and type this cohort against the same opaque pair. Retyping
        // its predecessor would reinterpret already finalized source custody.
        let extension = frontier.into_extension()?;
        let (typing, rebased) = predecessor.append_deferred(extension)?;
        frontier = rebased;
        typing
    } else {
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_to_seeded_base(
            frontier.shared_trees(),
        )
        .map_err(|diagnostic| vec![diagnostic])?
    };
    let mut execution = execution;
    validate_declaration_roots(typing.typed(), declaration_roots)?;
    let (work, selection) = loop {
        // Typing selects operator overloads which have no ordinary call edge in
        // raw syntax. Use the existing operational graph, not token spellings or
        // a second expression walker, to demand those same selected bodies.
        let operators = validation::derive_pre_flow_operator_selections(typing.typed());
        selected_body_dependencies(typing.typed(), &operators, &mut execution);
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

/// Declaration requests keep the exact retained owner, including generated
/// schemas without an authored span. Retaining a schema supplies type work;
/// retaining a machine signature does not grant its implementation demand.
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

fn selected_body_dependencies(
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
    execution: &mut Vec<SymbolHandle>,
) {
    crate::BuildTimeAdmissionPlan::infer_execution_dependencies(program, operators, execution);
}

/// Calls are not the whole checking domain: a demanded signature, data predicate,
/// or domain fact can itself require an implementation. Close those references
/// with the representation's shared traversal before removing any root. Keeping
/// all non-machine declarations would instead check unrelated pending indices.
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
    // A signature owns work without demanding its machine implementation.
    // Actual calls from those endpoints have already entered `execution` through
    // resolved body demand. Their typed type/proof dependencies enter here, but
    // an enclosing machine does not become executable merely by owning a type.
    let endpoint_declarations = if has_endpoint_owners
        && endpoint_owners != DeclarationSelection::from_symbols([])
    {
        let endpoint_refs = collect_selected_program_references(program, &endpoint_owners);
        for mut symbol in endpoint_refs.symbols {
            while symbol.is_valid() {
                if roster.contains(symbol) && !endpoint_owners.contains(symbol) {
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
        // A family declaration is one checking unit. Retain its members and
        // their exact helper, operator, type and proof dependencies in the
        // private projection. Projection alone grants no invocation authority.
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
            // Operator meaning is selected from operand types, not necessarily a
            // resolved call target. Retain the ordinary selection's exact candidate
            // context as well as its chosen declaration; pruning must not invent a
            // builtin fallback by deleting an unresolved domain-owned candidate.
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
                    .filter(|(_, operator_use)| new_expressions.contains(&operator_use.expression))
                    .map(|(_, operator_use)| operator_use.selected_operator_symbol),
            );
        }
        for mut symbol in referenced_symbols {
            while symbol.is_valid() {
                if roster.contains(symbol) && !selection.contains(symbol) {
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
