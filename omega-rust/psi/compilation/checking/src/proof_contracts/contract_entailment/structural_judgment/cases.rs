//! Nominal case judgments over exact observed subjects.
//!
//! Scope authenticates the subject/carrier/selected case before intake. A tag
//! remains a classifier, never a constructor payload or a whole-value equation.
//! Normal completions may publish the selected public case only after their
//! entry obligations and effects have been settled. Case substitution uses this
//! restricted ledger rather than arbitrary value rewrites or body unfolding.

use super::{CaseGuarantee, StructuralJudge, StructuralJudgment, StructuralTerm, term_contains};
use crate::proof_contracts::contract_entailment::{
    observed_projections, specification_calls, structural_terms,
};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::types::TypeReferenceHandle;

impl StructuralJudge<'_> {
    pub(super) fn is_case_membership(
        &self,
        expression: ExpressionHandle,
        comparison: &typed_trees::expression::TableBinaryExpression,
    ) -> bool {
        if !self.runtime_body_values {
            return crate::proof_contracts::bound_expression_meaning::has_exact_declaration_case_membership_meaning(
                self.program, self.machine_symbol, &self.declaration_parameters, expression, comparison,
            );
        }
        self.program
            .machine(self.machine_symbol)
            .is_some_and(|machine| {
                crate::proof_contracts::bound_expression_meaning::has_exact_case_membership_meaning(
                    self.program,
                    machine,
                    self.declaration_state
                        .is_valid()
                        .then(|| {
                            self.program
                                .state_in_machine(machine, self.declaration_state)
                        })
                        .flatten(),
                    expression,
                    comparison,
                )
            })
    }

    /// A callee's classifier is checked in its own parameter scope before its
    /// subject is substituted into the caller. Checking the uninstantiated
    /// expression against the caller would confuse identically named binders.
    pub(in crate::proof_contracts::contract_entailment) fn instantiated_case_judgment(
        &self,
        scope: SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        fact: ExpressionHandle,
        map: &[(String, StructuralTerm)],
    ) -> StructuralJudgment {
        self.instantiated_selected_case_judgment(scope, parameters, fact, map, &[])
    }

    pub(in crate::proof_contracts::contract_entailment) fn instantiated_selected_case_judgment(
        &self,
        scope: SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        fact: ExpressionHandle,
        map: &[(String, StructuralTerm)],
        machine_map: &[(
            SymbolHandle,
            TypeReferenceHandle,
            typed_trees::expression::StaticMachineArgument,
        )],
    ) -> StructuralJudgment {
        let Some(binary) = self.program.expression_table.binary_node(fact) else {
            return StructuralJudgment::Unknown;
        };
        if !crate::proof_contracts::bound_expression_meaning::has_exact_declaration_case_membership_meaning(
            self.program, scope, parameters, fact, binary,
        ) {
            return StructuralJudgment::Unknown;
        }
        let Some((definition, variant)) =
            structural_terms::case_classifier(self.program, binary.right)
        else {
            return StructuralJudgment::Unknown;
        };
        let Some(subject) = structural_terms::structural_term(
            self.program,
            None,
            Some((scope, parameters)),
            binary.left,
            None,
        ) else {
            return StructuralJudgment::Unknown;
        };
        self.judge_case(
            Self::substitute_call_term(&subject, map, machine_map),
            definition.symbol,
            variant.symbol,
        )
    }

    /// The selected public telescope owns tag formation too. A retained
    /// requirement can classify its explicit arguments or exact result without
    /// reopening a private realization's stronger contract or storage names.
    pub(in crate::proof_contracts::contract_entailment) fn instantiated_contract_case_guarantees(
        &self,
        owner: &specification_calls::SelectedCallContract<'_>,
        fact: ExpressionHandle,
        map: &[(String, StructuralTerm)],
        guarantees: &mut Vec<CaseGuarantee>,
    ) {
        use specification_calls::{SelectedCallContract, owner_parameters, owner_symbol};
        if !self.has_exact_case_subjects() {
            return;
        }
        let Some(binary) = self.program.expression_table.binary_node(fact) else {
            return;
        };
        let scope = match owner {
            SelectedCallContract::Concrete(machine, _) => machine.symbol,
            _ => owner_symbol(owner),
        };
        let parameters = owner_parameters(self.program, owner);
        // Logical conjunction has fixed compiler meaning. Its children retain
        // their own selected meanings: a case classifier is not scalar equality.
        if binary.operator == BinaryOperator::And {
            self.instantiated_contract_case_guarantees(owner, binary.left, map, guarantees);
            self.instantiated_contract_case_guarantees(owner, binary.right, map, guarantees);
            return;
        }
        if !structural_terms::is_case_observation(self.program, fact) {
            return;
        }
        let parameters = owner_parameters(self.program, owner);
        let (scope, exact) = match owner {
            SelectedCallContract::Concrete(callee, _) => (
                callee.symbol,
                crate::proof_contracts::bound_expression_meaning::has_exact_case_membership_meaning(
                    self.program,
                    callee,
                    None,
                    fact,
                    binary,
                ),
            ),
            SelectedCallContract::Signature(signature)
            | SelectedCallContract::DynamicSlot { signature, .. } => (
                signature.symbol,
                crate::proof_contracts::bound_expression_meaning::has_exact_declaration_case_membership_meaning(
                    self.program, signature.symbol, parameters, fact, binary,
                ),
            ),
            SelectedCallContract::Operator(operator) => (
                operator.symbol,
                crate::proof_contracts::bound_expression_meaning::has_exact_declaration_case_membership_meaning(
                    self.program, operator.symbol, parameters, fact, binary,
                ),
            ),
        };
        if !exact {
            return;
        }
        let Some((definition, variant)) =
            structural_terms::case_classifier(self.program, binary.right)
        else {
            return;
        };
        let Some(subject) = structural_terms::structural_term(
            self.program,
            None,
            Some((scope, parameters)),
            binary.left,
            None,
        ) else {
            return;
        };
        let Some(subject) = self.resolve_case_subject(Self::substitute_term(&subject, map), 0)
        else {
            return;
        };
        guarantees.push(CaseGuarantee {
            subject,
            data: definition.symbol,
            case: variant.symbol,
        });
    }

    pub(in crate::proof_contracts::contract_entailment) fn intake_case_guarantee(
        &mut self,
        guarantee: &CaseGuarantee,
    ) {
        Arc::make_mut(&mut self.case_facts).push((
            guarantee.subject.clone(),
            guarantee.data,
            guarantee.case,
        ));
    }

    pub(in crate::proof_contracts::contract_entailment) fn judge_case(
        &self,
        subject: StructuralTerm,
        data_symbol: SymbolHandle,
        case_symbol: SymbolHandle,
    ) -> StructuralJudgment {
        let Some(subject) = self.resolve_case_subject(subject, 0) else {
            return StructuralJudgment::Unknown;
        };
        // Checked inline observations retain this value's case identity.
        // A reference descriptor without an observed referent has no such tag.
        let subject = if matches!(&subject, StructuralTerm::ObservedValue { .. }) {
            if structural_terms::owned_observation_type(self.program, &subject).is_none()
                && structural_terms::shared_inline_observation_type(self.program, &subject)
                    .is_none()
            {
                return StructuralJudgment::Unknown;
            }
            subject.into_semantic_value()
        } else {
            subject
        };
        if let StructuralTerm::Constructor { data, case, .. } = &subject {
            // Legacy value terms retain nominal names. Rejoin only a unique
            // declaration; same-spelled foreign data is not this classifier.
            let Some(definition) = self.program.data_definition_by_symbol(data_symbol) else {
                return StructuralJudgment::Unknown;
            };
            if definition.name.as_str() != data
                || self
                    .program
                    .data_definitions()
                    .iter()
                    .filter(|candidate| candidate.name.as_str() == data)
                    .count()
                    != 1
            {
                return StructuralJudgment::Unknown;
            }
            let observed = self
                .program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == case =>
                    {
                        Some(variant.symbol)
                    }
                    _ => None,
                });
            return match observed {
                Some(observed) if observed == case_symbol => StructuralJudgment::Proven,
                Some(_) => StructuralJudgment::Refuted,
                None => StructuralJudgment::Unknown,
            };
        }
        // A captured value is one occurrence. Mathematical applications have
        // already rejoined their exact declaration and denotational closure in
        // resolve_case_subject; their tags never require body unfolding.
        if !matches!(
            subject,
            StructuralTerm::Variable(_)
                | StructuralTerm::Projection { .. }
                | StructuralTerm::BoundProjection { .. }
                | StructuralTerm::IndexedValue { .. }
                | StructuralTerm::BoundValue(_)
                | StructuralTerm::CapturedValue { .. }
                | StructuralTerm::Application { .. }
        ) {
            return StructuralJudgment::Unknown;
        }
        for (known_subject, known_data, known_case) in self.case_facts.iter() {
            if *known_data == data_symbol
                && self.resolve_case_subject(known_subject.clone(), 0).as_ref() == Some(&subject)
            {
                return if *known_case == case_symbol {
                    StructuralJudgment::Proven
                } else {
                    StructuralJudgment::Refuted
                };
            }
        }
        StructuralJudgment::Unknown
    }

    /// Only source premises and checked arm refinements feed this store. In
    /// particular, legacy citation results and already-unfolded value equations
    /// are not evidence for new tag judgments. No callable body is interpreted.
    pub(in crate::proof_contracts::contract_entailment) fn intake_case_equation(
        &mut self,
        left: StructuralTerm,
        right: StructuralTerm,
        depth: usize,
    ) {
        if depth >= 32 {
            return;
        }
        let (Some(left), Some(right)) = (
            self.resolve_case_subject(left, 0),
            self.resolve_case_subject(right, 0),
        ) else {
            return;
        };
        match (&left, &right) {
            (StructuralTerm::Variable(name), _)
                if left != right && !term_contains(&right, &left) =>
            {
                Arc::make_mut(&mut self.case_substitutions).push((name.clone(), right));
            }
            (_, StructuralTerm::Variable(name))
                if left != right && !term_contains(&left, &right) =>
            {
                Arc::make_mut(&mut self.case_substitutions).push((name.clone(), left));
            }
            (
                StructuralTerm::Constructor {
                    data: left_data,
                    case: left_case,
                    fields: left_fields,
                },
                StructuralTerm::Constructor {
                    data: right_data,
                    case: right_case,
                    fields: right_fields,
                },
            ) if left_data == right_data
                && left_case == right_case
                && left_fields.len() == right_fields.len() =>
            {
                for (name, left) in left_fields {
                    if let Some((_, right)) = right_fields.iter().find(|(field, _)| field == name) {
                        self.intake_case_equation(left.clone(), right.clone(), depth + 1);
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn resolve_case_subject(
        &self,
        term: StructuralTerm,
        depth: usize,
    ) -> Option<StructuralTerm> {
        if depth >= 32 {
            return None;
        }
        match term {
            StructuralTerm::ObservedValue {
                observed_type,
                value,
                descriptor,
            } => Some(StructuralTerm::ObservedValue {
                observed_type,
                descriptor,
                value: Box::new(self.resolve_case_subject(*value, depth + 1)?),
            }),
            StructuralTerm::Variable(ref name) => match self
                .case_substitutions
                .iter()
                .find(|(variable, _)| variable == name)
            {
                Some((_, replacement)) => self.resolve_case_subject(replacement.clone(), depth + 1),
                None => Some(term),
            },
            StructuralTerm::Constructor { data, case, fields } => {
                Some(StructuralTerm::Constructor {
                    data,
                    case,
                    fields: fields
                        .into_iter()
                        .map(|(name, field)| {
                            Some((name, self.resolve_case_subject(field, depth + 1)?))
                        })
                        .collect::<Option<Vec<_>>>()?,
                })
            }
            StructuralTerm::Integer(_)
            | StructuralTerm::Float(_)
            | StructuralTerm::ScalarRepresentation { .. }
            | StructuralTerm::BoundValue(_) => Some(term),
            StructuralTerm::CapturedValue { .. } => Some(term),
            application @ StructuralTerm::Application { .. } => {
                self.selected_application_case_subject(&application, depth)
            }
            StructuralTerm::Projection { subject, path } => {
                let subject = self.resolve_case_subject(*subject, depth + 1)?;
                observed_projections::project_field(subject, &path)
            }
            StructuralTerm::BoundProjection { .. } => Some(term),
            StructuralTerm::IndexedValue {
                element_type,
                element_identity,
                collection,
                index,
            } => Some(StructuralTerm::IndexedValue {
                element_type,
                element_identity,
                collection: Box::new(self.resolve_case_subject(*collection, depth + 1)?),
                index: Box::new(self.resolve_case_subject(*index, depth + 1)?),
            }),
            StructuralTerm::CallProjection { .. }
            | StructuralTerm::Opaque(_)
            | StructuralTerm::ScalarBinary { .. }
            | StructuralTerm::MathematicalBinary { .. }
            | StructuralTerm::IntegerEmbedding { .. }
            | StructuralTerm::CollectionMeasure { .. } => None,
        }
    }
}
