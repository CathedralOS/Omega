//! Exact guarantees keep their declaration roles before algebraic normalization.
//!
//! Applied requirement clauses and provider clauses share the canonical contract
//! identity owner. Positional symbols bind the complete invocation telescope;
//! selected operator meaning is checked in each declaration's own parameter scope.
//! This establishes no new equation and imports no private strengthening. Original
//! clause handles remain the receipt authority after applied payload substitution.

use crate::proof_contracts::contract_structure::{
    callable_binders, correspondence_fact, generic_binders,
};
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContractKind, StateParameter, StateSignature};
use typed_trees::state::State;
use typed_trees::trait_definition::AppliedTraitScope;

pub(super) struct DeclarationCorrespondence<'program> {
    requirement: &'program StateSignature,
    requirement_binders: Vec<(SymbolHandle, String)>,
    provider: SymbolHandle,
    provider_parameters: &'program [StateParameter],
    provider_binders: Vec<(SymbolHandle, String)>,
    applied_clauses: Vec<(ExpressionHandle, ExpressionHandle)>,
}

impl<'program> DeclarationCorrespondence<'program> {
    pub(super) fn new(
        program: &'program dyn crate::CheckingInput,
        machine: &'program Machine,
        entry: &'program State,
        application: &AppliedTraitScope<'_>,
        declared: &'program StateSignature,
    ) -> Option<Self> {
        let requirement = match program.applied_callable_signature(application, declared.symbol) {
            Some(applied) => applied,
            None if declared.type_parameters.is_empty()
                && application.substitutions.is_empty()
                && application.declaration.lifetime_parameters.is_empty() =>
            {
                declared
            }
            None => return None,
        };
        let requirement_parameters = program.state_signature_parameters(requirement);
        let provider_parameters = program.state_parameters(entry);
        if requirement_parameters.len() != provider_parameters.len() {
            return None;
        }
        let invocation_parameters = crate::machine_calls::static_arguments::invocation_parameters(
            program,
            machine,
            machine,
            program.machine_type_parameters(machine),
            application.declaration.symbol,
            declared.symbol,
        );
        let mut applied_clauses = Vec::new();
        for contract in program
            .state_signature_contracts(declared)
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for handle in contract.facts.handles() {
                let Some(original) = program.proof_facts.get(handle).as_expression() else {
                    continue;
                };
                let applied = program
                    .applied_signature_contract_fact(declared, requirement, handle)
                    .and_then(ProofFact::as_expression)?;
                pair_conjuncts(program, *original, *applied, &mut applied_clauses)?;
            }
        }
        Some(Self {
            requirement,
            requirement_binders: callable_binders(
                &generic_binders(program.state_signature_type_parameters(requirement)),
                requirement_parameters,
            ),
            provider: machine.symbol,
            provider_parameters,
            provider_binders: callable_binders(
                &generic_binders(&invocation_parameters),
                provider_parameters,
            ),
            applied_clauses,
        })
    }

    pub(super) fn matching_guarantee(
        &self,
        program: &dyn crate::CheckingInput,
        original: ExpressionHandle,
        proven: &[(ExpressionHandle, ExpressionHandle)],
    ) -> Option<ExpressionHandle> {
        let (_, applied) = self
            .applied_clauses
            .iter()
            .find(|(clause, _)| *clause == original)?;
        if !crate::has_builtin_parameter_bound_expression_meaning(
            program,
            self.requirement.symbol,
            program.state_signature_parameters(self.requirement),
            *applied,
        ) {
            return None;
        }
        let required = correspondence_fact(
            program,
            &ProofFact::Expression(*applied),
            &self.requirement_binders,
        )?;
        proven.iter().find_map(|(clause, source)| {
            if !crate::has_builtin_parameter_bound_expression_meaning(
                program,
                self.provider,
                self.provider_parameters,
                *clause,
            ) {
                return None;
            }
            (correspondence_fact(
                program,
                &ProofFact::Expression(*clause),
                &self.provider_binders,
            )
            .is_some_and(|provided| provided == required))
            .then_some(*source)
        })
    }
}

/// Callable application preserves the conjunction tree. Rejoin its payload by
/// checked positions, never by expression display text or a coincidental count.
fn pair_conjuncts(
    program: &dyn crate::CheckingInput,
    original: ExpressionHandle,
    applied: ExpressionHandle,
    clauses: &mut Vec<(ExpressionHandle, ExpressionHandle)>,
) -> Option<()> {
    match (
        program.expression_table.binary_node(original),
        program.expression_table.binary_node(applied),
    ) {
        (Some(source), Some(selected)) if source.operator == selected.operator => {
            match source.operator {
                BinaryOperator::And => {
                    pair_conjuncts(program, source.left, selected.left, clauses)?;
                    pair_conjuncts(program, source.right, selected.right, clauses)?;
                }
                BinaryOperator::Equal => clauses.push((original, applied)),
                _ => {}
            }
            Some(())
        }
        (None, None) => Some(()),
        _ => None,
    }
}
