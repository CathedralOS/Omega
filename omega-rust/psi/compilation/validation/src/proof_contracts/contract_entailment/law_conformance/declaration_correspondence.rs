//! Exact guarantees keep their declaration roles before algebraic normalization.
//!
//! Applied requirement clauses and provider clauses share the canonical contract
//! identity owner. Positional symbols bind the complete invocation telescope;
//! selected operator meaning is checked in each declaration's own parameter scope.
//! This establishes no new equation and imports no private strengthening. Original
//! clause handles remain the receipt authority after applied payload substitution.

use crate::proof_contracts::contract_structure::{callable_binders, generic_binders};
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContractKind, StateParameter, StateSignature};
use typed_trees::state::State;
use typed_trees::trait_definition::AppliedTraitScope;
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::TypeReferenceHandle;

use super::equality_conjuncts::{EqualityConjunct, equality_conjunct, matching_equality_conjuncts};

pub(super) struct DeclarationCorrespondence<'program> {
    requirement: &'program StateSignature,
    requirement_parameters: &'program [StateParameter],
    requirement_binders: Vec<(SymbolHandle, String)>,
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
                pair_conjuncts(
                    program,
                    (
                        declared.symbol,
                        program.state_signature_parameters(declared),
                    ),
                    (requirement.symbol, requirement_parameters),
                    *original,
                    *applied,
                    &mut applied_clauses,
                )?;
            }
        }
        Some(Self {
            requirement,
            requirement_parameters,
            requirement_binders: callable_binders(
                &generic_binders(program.state_signature_type_parameters(requirement)),
                requirement_parameters,
            ),
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
        proven: &[(EqualityConjunct<'_>, ExpressionHandle)],
    ) -> Option<ExpressionHandle> {
        let required = self.required_conjunct(program, original)?;
        proven.iter().find_map(|(clause, source)| {
            self.matching_conjuncts(program, required, *clause)
                .then_some(*source)
        })
    }

    pub(super) fn required_scope(&self) -> (SymbolHandle, &'program [StateParameter]) {
        (self.requirement.symbol, self.requirement_parameters)
    }

    pub(super) fn required_conjunct(
        &self,
        program: &dyn crate::CheckingInput,
        original: ExpressionHandle,
    ) -> Option<EqualityConjunct<'program>> {
        let (_, applied) = self
            .applied_clauses
            .iter()
            .find(|(source, _)| *source == original)?;
        equality_conjunct(
            program,
            self.requirement.symbol,
            self.requirement_parameters,
            *applied,
        )
    }

    pub(super) fn identity_carriers_match(
        &self,
        program: &dyn crate::CheckingInput,
        required: TypeReferenceHandle,
        provided: TypeReferenceHandle,
    ) -> bool {
        program.type_identity(TypeIdentityRequest {
            binders: &self.requirement_binders,
            ..TypeIdentityRequest::ordinary(required)
        }) == program.type_identity(TypeIdentityRequest {
            binders: &self.provider_binders,
            ..TypeIdentityRequest::ordinary(provided)
        })
    }

    pub(super) fn matching_conjuncts(
        &self,
        program: &dyn crate::CheckingInput,
        required: EqualityConjunct<'_>,
        provided: EqualityConjunct<'_>,
    ) -> bool {
        matching_equality_conjuncts(
            program,
            required,
            &self.requirement_binders,
            provided,
            &self.provider_binders,
        )
    }
}

/// Callable application preserves the conjunction tree. Rejoin its payload by
/// checked positions, never by expression display text or a coincidental count.
fn pair_conjuncts(
    program: &dyn crate::CheckingInput,
    original_scope: (SymbolHandle, &[StateParameter]),
    applied_scope: (SymbolHandle, &[StateParameter]),
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
                    pair_conjuncts(
                        program,
                        original_scope,
                        applied_scope,
                        source.left,
                        selected.left,
                        clauses,
                    )?;
                    pair_conjuncts(
                        program,
                        original_scope,
                        applied_scope,
                        source.right,
                        selected.right,
                        clauses,
                    )?;
                }
                BinaryOperator::Equal => clauses.push((original, applied)),
                _ => {}
            }
            Some(())
        }
        (None, None) => {
            if equality_conjunct(program, original_scope.0, original_scope.1, original).is_some()
                && equality_conjunct(program, applied_scope.0, applied_scope.1, applied).is_some()
            {
                clauses.push((original, applied));
            }
            Some(())
        }
        _ => None,
    }
}
