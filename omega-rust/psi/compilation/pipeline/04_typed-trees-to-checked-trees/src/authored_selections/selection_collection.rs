//! Collecting statement, transition-target and proof membership
//! selections.

use crate::authored_selections::call_targets::{
    checked_statement_call_target, implementation_call_selection,
};
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelectionTarget,
};
use symbols::{SymbolHandle, SymbolKind};
use validation::compiler_operations::classify_statement_call_intrinsic;
use validation::selection_resolution::{
    AuthoredSelectionResolution, AuthoredSelectionResolutionTarget,
};
use validation::selection_resolution::{
    collect_checked_transition_target_selections, declaration_target, push_consistent_resolution,
};

pub(crate) fn checked_struct_literal_type_symbol(
    program: &dyn ::validation::CheckingInput,
    literal: &typed_trees::expression::TableStructLiteral,
    source_span: source::SourceSpan,
) -> SymbolHandle {
    if literal.type_symbol.is_valid() {
        return literal.type_symbol;
    }

    // This selection was deliberately retained as late-bound. At the checked
    // boundary the complete symbol table can resolve the authored head in its
    // exact source/package visibility context. The ledger stores the resulting
    // symbol; downstream consumers never repeat this lookup.
    program
        .symbols
        .find_top_level_by_name_and_kinds_from_source(
            literal.type_name.as_str(),
            &[SymbolKind::Data],
            source_span,
        )
        .unwrap_or_else(SymbolHandle::invalid)
}

pub(crate) fn collect_checked_proof_membership_selections(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    resolutions: &mut Vec<AuthoredSelectionResolution>,
) -> Result<(), Diagnostic> {
    for (fact_handle, fact) in typed_trees::references::program_proof_facts(program) {
        let Some(membership) = fact.as_membership() else {
            continue;
        };
        let Some(occurrence) = membership.authored_domain_selection else {
            continue;
        };
        let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
            return Err(Diagnostic::error(format!(
                "proof membership retains unknown authored selection occurrence {}",
                occurrence.ordinal(),
            )));
        };
        if selection.kind() != AuthoredDeclarationSelectionKind::DomainMembership {
            return Err(Diagnostic::error(
                "proof membership retains mismatched authored domain-selection evidence",
            )
            .with_source_span(selection.source_span()));
        }
        if selection.target()
            != AuthoredDeclarationSelectionTarget::LateBound(
                AuthoredDeclarationSelectionLateBinding::CheckedDomainMembership,
            )
        {
            continue;
        }

        let target = declaration_target(membership.domain_symbol).or_else(|| {
            let mut permission = None;
            for (_, checked_fact) in facts.semantic.facts.iter() {
                let facts::FactPayload::ContractCarryPermission {
                    fact,
                    permission: candidate,
                    ..
                } = checked_fact.payload
                else {
                    continue;
                };
                if fact != fact_handle {
                    continue;
                }
                if permission.is_some_and(|retained| retained != candidate) {
                    return None;
                }
                permission = Some(candidate);
            }
            permission.map(|permission| {
                AuthoredSelectionResolutionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::CarryPermission(permission),
                )
            })
        });
        if let Some(target) = target {
            push_consistent_resolution(
                resolutions,
                AuthoredSelectionResolution {
                    occurrence,
                    binding: AuthoredDeclarationSelectionLateBinding::CheckedDomainMembership,
                    target,
                },
            )?;
        }
    }
    Ok(())
}

/// Contract-position calls whose spelled target selects no declaration are
/// admitted proof views: `Seq`/`Bag`/`Range`-style schematic atoms mint no
/// machine realization, numeric interpretation, or equality semantics
/// (validation/src/proof_contracts/contract_entailment/proof_view_tests.rs
/// pins this admission). Their checked call occurrences still need an
/// explicit ledger target, so they finalize as the compiler-owned ProofView
/// intrinsic rather than staying unresolved after checking. Executable
/// positions never reach this collector: an undeclared call there fails
/// resolution before checking admits the program.
///
/// The resolver deliberately mints no Call occurrence for a contract callee
/// that names no visible declaration (the `unbound_contract_call` gate in
/// syntax-trees-to-symbol-resolved-trees). Such spellings cannot be bound
/// here because there is nothing to bind; instead they are reported through
/// `unoccurred_view_calls` so finalization can mint a finalized ProofView row
/// directly and retain its call-expression association in checked facts.
pub(crate) fn collect_checked_proof_view_call_selections(
    program: &dyn ::validation::CheckingInput,
    resolutions: &mut Vec<AuthoredSelectionResolution>,
    unoccurred_view_calls: &mut Vec<(
        source::SourceSpan,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
        typed_trees::expression::ExpressionHandle,
    )>,
) -> Result<(), Diagnostic> {
    let mut fact_roots = Vec::new();
    for (_, fact) in typed_trees::references::program_proof_facts(program) {
        match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                fact_roots.push(*expression);
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                fact_roots.push(membership.value);
            }
            typed_trees::domain::ProofFact::Proposition(application) => {
                for argument in program
                    .expression_table
                    .expression_handles(application.arguments)
                {
                    fact_roots.push(*argument);
                }
            }
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
            {
                let Some(fact) = statement.as_assembly_fact() else {
                    continue;
                };
                fact_roots.extend(program.tables.proof_fact_expression_roots(fact.fact));
            }
        }
    }
    for root in fact_roots {
        let mut subtree = Vec::new();
        validation::expression_traversal::collect_expression_tree(program, root, &mut subtree);
        // The clause's authored exposure survives on any sibling occurrence in
        // the same fact subtree; a subtree authored without occurrences (a
        // bare uninterpreted atom alone) falls back to private custody.
        let subtree_exposure = subtree
            .iter()
            .flat_map(|expression| {
                program.expression_table.authored_selection_occurrences(*expression)
            })
            .find_map(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .map(|selection| selection.exposure())
            })
            .unwrap_or(
                language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            );
        for expression in subtree {
            let typed_trees::expression::ExpressionNode::Call(call) =
                program.expression_table.expression(expression)
            else {
                continue;
            };
            // A spelled view atom is a bare unqualified name: `Bag(items)`, not
            // `Packet::is_ready(self)` or `self.measure()`. Any authored receiver
            // means the call names a declared or context-owned selection and must
            // keep its ordinary custody resolution (and rejection) behavior.
            if call.target_symbol.is_valid() || call.receiver.is_valid() {
                continue;
            }
            let occurrences = program
                .expression_table
                .authored_selection_occurrences(expression)
                .collect::<Vec<_>>();
            for occurrence in occurrences.iter().copied() {
                // Earlier collectors (statement-position intrinsic selection,
                // membership finalization) can already bind the same occurrence
                // through a shared statement/proof-fact lowering; a pending
                // resolution wins over admission.
                if resolutions
                    .iter()
                    .any(|resolution| resolution.occurrence == occurrence)
                {
                    continue;
                }
                let Some(selection) = program.authored_declaration_selections().get(occurrence)
                else {
                    return Err(Diagnostic::error(format!(
                        "proof view call retains unknown authored selection occurrence {}",
                        occurrence.ordinal(),
                    )));
                };
                if selection.kind() != AuthoredDeclarationSelectionKind::Call
                    || selection.target()
                        != AuthoredDeclarationSelectionTarget::LateBound(
                            AuthoredDeclarationSelectionLateBinding::CheckedCall,
                        )
                {
                    continue;
                }
                push_consistent_resolution(
                    resolutions,
                    AuthoredSelectionResolution {
                        occurrence,
                        binding: AuthoredDeclarationSelectionLateBinding::CheckedCall,
                        target: AuthoredSelectionResolutionTarget::Intrinsic(
                            AuthoredDeclarationSelectionIntrinsic::ProofView,
                        ),
                    },
                )?;
            }
            let has_call_occurrence = occurrences.iter().copied().any(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .is_some_and(|selection| {
                        selection.kind() == AuthoredDeclarationSelectionKind::Call
                    })
            });
            if !has_call_occurrence {
                unoccurred_view_calls.push((
                    program.expression_table.source_span(expression),
                    subtree_exposure,
                    expression,
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn collect_checked_statement_selections(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    resolutions: &mut Vec<AuthoredSelectionResolution>,
) -> Result<(), Diagnostic> {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                let Some(call) = statement.as_call() else {
                    continue;
                };
                if call.operational_acknowledgement.origin
                    != language_semantics::CallOperationalAcknowledgementOrigin::Source
                    || call.source_span.span.start >= call.source_span.span.end
                {
                    continue;
                }
                let target = implementation_call_selection(program, call.receiver_symbol)
                    .unwrap_or_else(|| {
                        checked_statement_call_target(
                            program,
                            facts,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            call.target_symbol,
                        )
                    });
                let Some(occurrence) = call.authored_call_selection else {
                    return Err(Diagnostic::error(
                        "source-authored checked statement call has no attached call selection",
                    )
                    .with_source_span(call.source_span));
                };
                let Some(selection) = program.authored_declaration_selections().get(occurrence)
                else {
                    return Err(Diagnostic::error(format!(
                        "statement call retains unknown authored selection occurrence {}",
                        occurrence.ordinal(),
                    ))
                    .with_source_span(call.source_span));
                };
                if selection.kind() != AuthoredDeclarationSelectionKind::Call
                    || selection.exposure()
                        != language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
                    || selection.source_span() != call.source_span
                {
                    return Err(Diagnostic::error(
                        "statement call retains mismatched authored call-selection evidence",
                    )
                    .with_source_span(call.source_span));
                }
                let resolution_target = classify_statement_call_intrinsic(program, call)
                    .map(AuthoredSelectionResolutionTarget::Intrinsic)
                    .or_else(|| {
                        crate::flow::resolved_operator_statement_symbol(program, call)
                            .and_then(declaration_target)
                    })
                    .or_else(|| declaration_target(target));
                if selection.target()
                    == AuthoredDeclarationSelectionTarget::LateBound(
                        AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    )
                    && let Some(target) = resolution_target
                {
                    push_consistent_resolution(
                        resolutions,
                        AuthoredSelectionResolution {
                            occurrence,
                            binding: AuthoredDeclarationSelectionLateBinding::CheckedCall,
                            target,
                        },
                    )?;
                }
            }
        }
    }
    collect_checked_transition_target_selections(program, resolutions)?;
    Ok(())
}
