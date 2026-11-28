/*
    Operand-owner tables and cohort-scoped guards for Stage 03 (`close_typed_cohort`).

    A table answers exactly only for the program it was built from. The owner
    walk reads nearly every typed-trees field, every preparation pass writes
    some of them, and typed trees keep no revision, so `close_typed_cohort`
    builds one table for each pass that reads owners rather than sharing one.
    A fresh cohort uses the parallel `collect_program_operand_owners` walk.
    Incremental cohorts (`lower_seeded_extension` and `lower_body_completion`)
    walk only the declarations the cohort added or completed, so
    already-closed declarations are not re-traversed.
*/

use crate::lowerer::TypingCohort;
use crate::source_state::TypedTrees;
use crate::source_state::expression::{ExpressionHandle, ExpressionNode};
use crate::source_state::references::{
    DeclarationSelection, ProgramOperandOwners, ScopedOccurrences, TypedReferences,
    collect_program_operand_owners, collect_selected_program_operand_owners, visit_program,
};
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::{SymbolHandle, SymbolKind};

/// The owners visible to a single pass, from the exact snapshot it reads.
pub(crate) struct CohortOperandOwners(ProgramOperandOwners);

impl CohortOperandOwners {
    pub(crate) fn full(program: &TypedTrees) -> Self {
        Self(collect_program_operand_owners(program))
    }

    pub(crate) fn collect(program: &TypedTrees, cohort: &TypingCohort<'_>) -> Self {
        if cohort.is_fresh() {
            return Self::full(program);
        }
        Self(collect_selected_program_operand_owners(
            program,
            cohort.declarations(program),
        ))
    }

    pub(crate) fn expression_owners(
        &self,
        expression: ExpressionHandle,
    ) -> impl Iterator<Item = SymbolHandle> + '_ {
        self.0.expression_owners(expression)
    }

    pub(crate) fn type_owners(
        &self,
        reference: TypeReferenceHandle,
    ) -> impl Iterator<Item = SymbolHandle> + '_ {
        self.0.type_owners(reference)
    }
}

/// Retain static argument contexts for the calls `cohort` added.
///
/// A continuing cohort gives each new expression call its context through
/// `retain_call_static_argument_context`, walking only the cohort's
/// declarations. Statement calls have no such per-call operation in typed
/// trees, so a cohort machine with a statement call that needs a context
/// (`cohort_has_static_statement_calls`) still runs the program-wide pass, as
/// does a fresh cohort. Returns whether the program-wide pass ran, since it
/// may rewrite retained declarations.
pub(crate) fn retain_cohort_static_argument_contexts(
    program: &mut TypedTrees,
    cohort: &TypingCohort<'_>,
) -> Result<bool, &'static str> {
    if cohort.is_fresh() || cohort_has_static_statement_calls(program, cohort) {
        return program.retain_static_argument_contexts().map(|()| true);
    }
    if program.expression_table.expression_count() <= cohort.typed_expressions {
        return Ok(false);
    }
    let new_calls = program
        .expression_table
        .iter_expressions_from(cohort.typed_expressions + 1)
        .filter(|(_, node)| matches!(node, ExpressionNode::Call(_)))
        .count();
    if new_calls == 0 {
        return Ok(false);
    }

    struct CohortCalls {
        selected: DeclarationSelection,
        expression_frontier: usize,
        owner: SymbolHandle,
        outer: Vec<SymbolHandle>,
        seen: ScopedOccurrences<ExpressionNode>,
        calls: Vec<(ExpressionHandle, SymbolHandle)>,
    }
    impl Extend<SymbolHandle> for CohortCalls {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
    }
    impl TypedReferences for CohortCalls {
        fn derived_declaration_references(&self) -> bool {
            false
        }
        fn collects_symbols(&self) -> bool {
            false
        }
        fn declaration(&mut self, owner: SymbolHandle) -> bool {
            if !self.selected.contains(owner) {
                return false;
            }
            self.owner = owner;
            self.outer.clear();
            true
        }
        fn enter_lexical_scope(&mut self, owner: SymbolHandle) {
            self.outer.push(self.owner);
            if owner.is_valid() {
                self.owner = owner;
            }
        }
        fn leave_lexical_scope(&mut self) {
            if let Some(owner) = self.outer.pop() {
                self.owner = owner;
            }
        }
        fn expression(&mut self, expression: ExpressionHandle) -> bool {
            if !self.seen.insert(self.owner, expression) {
                return false;
            }
            if expression.arena_index() as usize > self.expression_frontier {
                self.calls.push((expression, self.owner));
            }
            true
        }
    }

    let mut collector = CohortCalls {
        selected: cohort.declarations(program),
        expression_frontier: cohort.typed_expressions,
        owner: SymbolHandle::invalid(),
        outer: Vec::new(),
        seen: ScopedOccurrences::default(),
        calls: Vec::new(),
    };
    visit_program(program, &mut collector);
    // A new call that the cohort's declarations do not reach belongs to a
    // retained declaration; only the program-wide pass gives it its context.
    let mut reached_calls = collector
        .calls
        .iter()
        .filter(|(expression, _)| program.expression_table.call_node(*expression).is_some())
        .map(|(expression, _)| expression.arena_index())
        .collect::<Vec<_>>();
    reached_calls.sort_unstable();
    reached_calls.dedup();
    if reached_calls.len() != new_calls {
        return program.retain_static_argument_contexts().map(|()| true);
    }
    for (expression, owner) in collector.calls {
        if program.expression_table.call_node(expression).is_some() {
            program.retain_call_static_argument_context(expression, owner)?;
        }
    }
    Ok(false)
}

/// Whether a machine in `cohort` has a statement call that
/// `TypedTrees::retain_static_argument_contexts` would give a source
/// application. The predicate is that pass's own statement filter. It includes
/// ordinary direct calls to a machine or state, because their result evidence
/// needs the exact declaration and lexical owner too.
fn cohort_has_static_statement_calls(program: &TypedTrees, cohort: &TypingCohort<'_>) -> bool {
    let direct_declaration = |target: SymbolHandle| {
        program
            .machine_by_symbol(target)
            .or_else(|| program.machine_holding_state(target))
            .is_some()
    };
    let has_region_telescope = |target: SymbolHandle| {
        program
            .machine_by_symbol(target)
            .or_else(|| program.machine_holding_state(target))
            .is_some_and(|machine| !machine.lifetime_parameters.is_empty())
    };
    let implementation_namespace = |symbol: SymbolHandle| {
        symbol.is_valid()
            && (program.conformance_by_symbol(symbol).is_some()
                || program.symbols.get(symbol).kind == SymbolKind::ConformanceParameter)
    };
    program
        .machines()
        .iter()
        .enumerate()
        .filter(|(ordinal, machine)| cohort.owns_machine(*ordinal, machine.symbol))
        .any(|(_, machine)| {
            program.machine_states(machine).iter().any(|state| {
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .any(|statement| {
                        let Some(call) = statement.as_call() else {
                            return false;
                        };
                        call.authored_machine_application.is_valid()
                            || !call.machine_arguments.is_empty()
                            || has_region_telescope(call.target_symbol)
                            || implementation_namespace(call.receiver_symbol)
                            || (call.runtime_call.is_none()
                                && call.installed_interrupt_operation.is_none()
                                && call.static_requirement_dispatch.is_none()
                                && program
                                    .trait_machine_signature(call.target_symbol)
                                    .is_some())
                            || (!call.static_machine_parameter.is_valid()
                                && call.static_requirement_dispatch.is_none()
                                && direct_declaration(call.target_symbol))
                    })
            })
        })
}

/// Returns `true` when an incremental cohort introduced declarations, trait scopes, or invocation
/// sites that could require `prepare_applied_callable_signatures`.
///
/// That operation always walks the whole program: a new conformance or trait
/// can give an already-typed application new requirement signatures, and typed
/// trees offers a narrower form only for one known application. This guard is
/// therefore the only cohort scoping the pass gets.
pub(crate) fn cohort_may_need_applied_callable_signatures(
    program: &TypedTrees,
    cohort: &TypingCohort<'_>,
) -> bool {
    if cohort.is_fresh() {
        return true;
    }
    if program.conformances().len() > cohort.conformances
        || program.traits().len() > cohort.traits
        || program.service_reaches.all_definitions().count() > cohort.typed_service_reaches
        || program.product_operand_applications.len() > cohort.typed_product_operand_applications
    {
        return true;
    }
    if program
        .machines()
        .iter()
        .skip(cohort.typed_machines)
        .any(|machine| !machine.satisfies.is_empty() || !machine.conformance_bounds.is_empty())
    {
        return true;
    }
    if program
        .expression_table
        .static_machine_applications()
        .skip(cohort.typed_static_machine_applications)
        .any(|(_, source)| {
            source.conformance_selection.is_some()
                || program
                    .machine_by_symbol(source.binding_declaration)
                    .is_some_and(|machine| {
                        machine.conformance_bounds.iter().any(|bound| {
                            program
                                .trait_definition_by_symbol(bound.trait_symbol)
                                .is_some_and(|trait_def| !trait_def.refinement_clauses.is_empty())
                        })
                    })
        })
    {
        return true;
    }
    program
        .type_reference_table
        .type_reference_rows_from(cohort.typed_type_references)
        .any(|(_, node)| match node {
            TypeReferenceNode::Named { symbol, .. }
            | TypeReferenceNode::Generic {
                base_symbol: symbol,
                ..
            } => {
                symbol.is_valid()
                    && program.symbols.symbols().nodes().is_valid(*symbol)
                    && matches!(
                        program.symbols.get(*symbol).kind,
                        SymbolKind::Trait | SymbolKind::Conformance
                    )
            }
            _ => false,
        })
}
