//! Predicate consequences of independently established incoming values.
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
    /// A successful nominal guard establishes only the selected case's facts.
    /// Its fresh common/payload terms identify this observed value, so later
    /// state substitution transports the predicate without rereading storage.
    pub(in crate::proof_contracts::contract_entailment) fn intake_selected_case_predicates(
        &mut self,
        definition: &typed_trees::data::DataDefinition,
        variant: &typed_trees::data::DataVariant,
        value: &StructuralTerm,
    ) {
        let StructuralTerm::Constructor { fields, .. } = value else {
            return;
        };
        let program = self.program;
        let declared = program
            .data_members(definition)
            .iter()
            .filter_map(|member| member.as_field())
            .chain(program.data_payload_fields(variant));
        let mut parameters = Vec::new();
        let mut bindings = Vec::new();
        for field in declared {
            let mut values = fields
                .iter()
                .filter(|(name, _)| name == field.name.as_str());
            let Some((_, value)) = values.next() else {
                return;
            };
            if values.next().is_some() {
                return;
            }
            parameters.push(StateParameter {
                symbol: field.symbol,
                name: field.name.clone(),
                type_reference: field.type_reference,
                ..Default::default()
            });
            bindings.push((field.name.to_string(), value.clone()));
        }
        for fact in program.proof_facts.span_or_empty(variant.where_facts) {
            if let ProofFact::Expression(expression) = fact {
                self.intake_instantiated_predicate(
                    definition.symbol,
                    &parameters,
                    *expression,
                    &bindings,
                );
            }
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn intake_entry_domain_predicates(
        &mut self,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) {
        let program = self.program;
        for parameter in program.state_parameters(state) {
            self.intake_admitted_value_predicates(
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

    pub(super) fn intake_admitted_value_predicates(
        &mut self,
        reference: TypeReferenceHandle,
        value: StructuralTerm,
    ) {
        self.intake_admitted_scalar_bounds(reference, &value);
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
        self.intake_admitted_data_predicates(reference, value, &mut Vec::new());
    }

    /// The carrier belongs to this completed value, even after conversion or
    /// a later write to its former source. A reference descriptor alone does
    /// not establish a scalar observation. These intrinsic integer facts say
    /// nothing about user domains, a call's private body or a pending result.
    pub(super) fn intake_admitted_scalar_bounds(
        &mut self,
        reference: TypeReferenceHandle,
        value: &StructuralTerm,
    ) {
        let program = self.program;
        let Some(meaning) =
            crate::value_custody::expression_types::scalar_observation_meaning(program, reference)
        else {
            return;
        };
        // Address geometry remains symbolic and target-independent; the
        // hosted implementation's Addr integer_bounds is not that contract.
        if !meaning.0.is_fixed_integer() {
            return;
        }
        let Some((minimum, maximum)) = meaning.0.integer_bounds() else {
            return;
        };
        if !super::super::structural_terms::scalar_operand_is_observed(program, reference, value) {
            return;
        }
        for (operator, bound) in [
            (BinaryOperator::GreaterOrEqual, minimum),
            (BinaryOperator::LessOrEqual, maximum),
        ] {
            let predicate = StructuralTerm::ScalarBinary {
                references: [reference; 2],
                operator,
                meaning: [meaning; 2],
                left: Box::new(value.clone()),
                right: Box::new(StructuralTerm::Integer(
                    numerics::bignum::BigInt::from_i128(bound),
                )),
            };
            self.intake_equation(predicate, super::value_predicates::boolean_term(true), 0);
        }
    }

    /// Only admitted inputs and completed call results enter here. A pending
    /// constructor's receiving annotation is not evidence of its own `where`.
    /// Readable input references expose their entry value; produced reference
    /// descriptors still cannot manufacture observations of their referents.
    fn intake_admitted_data_predicates(
        &mut self,
        reference: TypeReferenceHandle,
        value: StructuralTerm,
        active: &mut Vec<SymbolHandle>,
    ) {
        let program = self.program;
        let Some(definition) =
            crate::field_domains::readable_nominal_definition(program, reference)
        else {
            return;
        };
        if active.contains(&definition.symbol) {
            return;
        }
        active.push(definition.symbol);
        let mut parameters = Vec::new();
        let mut bindings = Vec::new();
        // Unknown case identity admits only common fields. Payload predicates
        // enter through the separately authenticated selected-case arm; reading
        // every variant here would project inactive same-named payloads onto
        // one subject and lend their unrelated default-domain facts.
        for field in program
            .data_members(definition)
            .iter()
            .filter_map(|member| member.as_field())
        {
            let path = [facts::PlaceSegment::Field {
                symbol: field.symbol,
            }];
            let Some(projected) = super::super::observed_projections::project_observed_value(
                program,
                reference,
                value.clone(),
                &path,
            ) else {
                continue;
            };
            let Some(projected_type) =
                crate::project_type_reference_from_segments(program, reference, &path)
            else {
                continue;
            };
            parameters.push(StateParameter {
                symbol: field.symbol,
                name: field.name.clone(),
                type_reference: projected_type,
                ..Default::default()
            });
            bindings.push((field.name.to_string(), projected.clone()));
            self.intake_admitted_scalar_bounds(projected_type, &projected);
            self.intake_admitted_data_predicates(projected_type, projected, active);
        }
        for fact in program.proof_facts.span_or_empty(definition.where_facts) {
            if let ProofFact::Expression(expression) = fact {
                self.intake_instantiated_predicate(
                    definition.symbol,
                    &parameters,
                    *expression,
                    &bindings,
                );
            }
        }
        active.pop();
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
