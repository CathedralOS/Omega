//! Predicate consequences of independently established incoming membership.
//!
//! State parameters and Requires describe this invocation's admitted inputs;
//! a completed call's declared result describes that exact returned value.
//! A local annotation or destination type is never an incoming premise. Bind
//! each domain subject to its captured value before retaining its predicates,
//! so later storage writes cannot reinterpret them over replacement contents.

use super::{StructuralJudge, StructuralTerm};
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::signature::{SignatureContractKind, StateParameter};
use typed_trees::types::TypeReferenceHandle;

impl StructuralJudge<'_> {
    pub(super) fn intake_entry_domain_predicates(
        &mut self,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) {
        let program = self.program;
        for parameter in program.state_parameters(state) {
            self.intake_declared_value_domains(
                parameter.type_reference,
                StructuralTerm::Variable(parameter.name.as_str().to_owned()),
            );
        }
        let entry = program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry.symbol == state.symbol);
        for contract in program
            .state_contracts(state)
            .iter()
            .chain(program.machine_contracts(machine).iter().filter(|_| entry))
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let ProofFact::Membership(membership) = fact else {
                    continue;
                };
                if !membership.domain_arguments.is_empty() {
                    continue;
                }
                let Some(reference) = crate::expression_result_type_reference(
                    program,
                    machine,
                    state,
                    membership.value,
                ) else {
                    continue;
                };
                let Some(value) = super::super::structural_terms::structural_term(
                    program,
                    Some((machine, state, reference)),
                    None,
                    membership.value,
                    None,
                ) else {
                    continue;
                };
                self.intake_domain_predicates(
                    membership.domain_symbol,
                    reference,
                    value,
                    &mut Vec::new(),
                );
            }
        }
    }

    pub(super) fn intake_declared_value_domains(
        &mut self,
        reference: TypeReferenceHandle,
        value: StructuralTerm,
    ) {
        let program = self.program;
        for row in crate::field_domains::declared_type_domain_rows(program, reference) {
            let Some(projected) = super::super::observed_projections::project_observed_value(
                program,
                reference,
                value.clone(),
                &row.path,
            ) else {
                continue;
            };
            let Some(projected_type) =
                crate::project_type_reference_from_segments(program, reference, &row.path)
            else {
                continue;
            };
            self.intake_domain_predicates(
                row.domain_symbol,
                projected_type,
                projected,
                &mut Vec::new(),
            );
        }
    }

    fn intake_domain_predicates(
        &mut self,
        symbol: SymbolHandle,
        reference: TypeReferenceHandle,
        value: StructuralTerm,
        active: &mut Vec<SymbolHandle>,
    ) {
        let program = self.program;
        if active.contains(&symbol) || !typed_trees::domain::has_symbol_only_proof(program, symbol)
        {
            return;
        }
        let Some(domain) = program
            .domain_definition(symbol)
            .filter(|domain| domain.alias.is_none())
        else {
            return;
        };
        active.push(symbol);
        let parameters = [StateParameter {
            symbol: domain.subject_symbol,
            name: domain.subject_name.clone(),
            type_reference: reference,
            ..Default::default()
        }];
        let bindings = [(domain.subject_name.as_str().to_owned(), value)];
        for fact in program.proof_facts(domain) {
            match fact {
                ProofFact::Expression(expression) => {
                    self.intake_instantiated_predicate(symbol, &parameters, *expression, &bindings)
                }
                ProofFact::Membership(membership) if membership.domain_arguments.is_empty() => {
                    let Some(reference) = crate::parameter_expression_result_type_reference(
                        program,
                        symbol,
                        &parameters,
                        membership.value,
                    ) else {
                        continue;
                    };
                    let Some(term) = super::super::structural_terms::structural_term(
                        program,
                        None,
                        Some((symbol, &parameters)),
                        membership.value,
                        None,
                    ) else {
                        continue;
                    };
                    let value = Self::substitute_call_term(&term, &bindings, &[]);
                    self.intake_domain_predicates(
                        membership.domain_symbol,
                        reference,
                        value,
                        active,
                    );
                }
                _ => {}
            }
        }
        active.pop();
    }

    fn intake_instantiated_predicate(
        &mut self,
        owner: SymbolHandle,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
        bindings: &[(String, StructuralTerm)],
    ) {
        let program = self.program;
        if let Some(binary) = program.expression_table.binary_node(expression) {
            if binary.operator == BinaryOperator::And {
                self.intake_instantiated_predicate(owner, parameters, binary.left, bindings);
                self.intake_instantiated_predicate(owner, parameters, binary.right, bindings);
                return;
            }
            if super::super::structural_terms::has_identity_equality_meaning(
                program, owner, parameters, expression,
            ) && let Some((left, right)) =
                super::super::structural_terms::structural_term_pair_for_parameters(
                    program, owner, parameters, binary,
                )
            {
                let left = Self::substitute_call_term(&left, bindings, &[]);
                let right = Self::substitute_call_term(&right, bindings, &[]);
                if binary.operator == BinaryOperator::Equal {
                    self.intake_equation(left, right, 0);
                } else if binary.operator == BinaryOperator::NotEqual {
                    std::sync::Arc::make_mut(&mut self.disequalities).push((left, right));
                }
            }
        }
        if let Some(term) = super::super::structural_terms::structural_term(
            program,
            None,
            Some((owner, parameters)),
            expression,
            None,
        ) {
            let term = Self::substitute_call_term(&term, bindings, &[]);
            self.intake_equation(term, super::value_predicates::boolean_term(true), 0);
        }
    }
}
