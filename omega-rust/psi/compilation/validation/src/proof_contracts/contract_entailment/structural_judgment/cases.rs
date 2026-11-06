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
use std::collections::HashMap;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::data::{DataDefinition, DataMember, DataVariant};
use typed_trees::expression::StaticMachineArgument;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::types::TypeReferenceHandle;

// Match the extractor's connective traversal. Shared expression children
// retain one metadata result per complete handle within this query.
fn prepare_case_observation_expressions(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    expressions: &mut HashMap<ExpressionHandle, bool>,
) -> bool {
    if let Some(contains_case) = expressions.get(&expression) {
        return *contains_case;
    }
    let contains_case = program
        .expression_table
        .binary_node(expression)
        .is_some_and(|binary| match binary.operator {
            BinaryOperator::And | BinaryOperator::Or => {
                let left = prepare_case_observation_expressions(program, binary.left, expressions);
                let right =
                    prepare_case_observation_expressions(program, binary.right, expressions);
                left || right
            }
            _ => structural_terms::is_case_observation(program, expression),
        });
    expressions.insert(expression, contains_case);
    contains_case
}

#[derive(Clone)]
pub(super) struct CaseFact {
    subject: StructuralTerm,
    data: SymbolHandle,
    case: SymbolHandle,
    holds: bool,
}

/// Translate an authenticated classifier into its subject's exact application.
/// The premise supplies tag truth; carrier correspondence supplies only identity.
pub(super) fn applied_case_classifier(
    program: &dyn crate::CheckingInput,
    classifier: ExpressionHandle,
    subject_type: Option<TypeReferenceHandle>,
) -> Option<(&DataDefinition, &DataVariant)> {
    let (definition, variant) = structural_terms::case_classifier(program, classifier)?;
    let Some(reference) = subject_type else {
        return Some((definition, variant));
    };
    let reference = program
        .type_reference_table
        .peeled_type_reference(reference);
    Some((
        program.data_constructor_definition(reference, definition.symbol)?,
        program.data_constructor_case(reference, definition.symbol, variant.symbol)?,
    ))
}

impl StructuralJudge<'_> {
    /// A failed case test excludes that case on this subject. Keep its exact
    /// declaration and application just as for a successful test; neither
    /// observation supplies unobserved payload fields.
    pub(super) fn intake_case_condition(&mut self, fact: ExpressionHandle, holds: bool) {
        let program = self.program;
        let Some(binary) = program.expression_table.binary_node(fact) else {
            return;
        };
        if !structural_terms::is_case_observation(program, fact)
            || !self.is_case_membership(fact, binary)
        {
            return;
        }
        let reference = if self.runtime_body_values {
            self.runtime_declaration_scope()
                .and_then(|(machine, state)| {
                    crate::proof_contracts::bound_expression_meaning::operand_type(
                        program,
                        machine,
                        Some(state),
                        binary.left,
                    )
                })
        } else {
            self.declaration_expression_type(binary.left)
        };
        let Some((definition, variant)) = applied_case_classifier(program, binary.right, reference)
        else {
            return;
        };
        let Some(subject) = self.declaration_term(binary.left) else {
            return;
        };
        self.intake_observed_case(subject.clone(), definition.symbol, variant.symbol, holds);
        // Only a successful tag test on a carrier without common or payload
        // fields describes a whole value. Exclusion never constructs a value.
        if !holds
            || program
                .data_members(definition)
                .iter()
                .any(|member| matches!(member, DataMember::Field(_)))
            || !program.data_payload_fields(variant).is_empty()
        {
            return;
        }
        let value = StructuralTerm::Constructor {
            data: definition.name.as_str().to_owned(),
            case: variant.name.as_str().to_owned(),
            fields: Vec::new(),
        };
        self.intake_case_equation(subject.clone(), value.clone(), 0);
        self.intake_equation(subject, value, 0);
    }

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
        machine_map: &[(SymbolHandle, TypeReferenceHandle, StaticMachineArgument)],
    ) -> StructuralJudgment {
        self.instantiated_case_guarantee(scope, parameters, fact, map, machine_map)
            .map_or(StructuralJudgment::Unknown, |guarantee| {
                self.judge_case(guarantee.subject, guarantee.data, guarantee.case)
            })
    }

    pub(in crate::proof_contracts::contract_entailment) fn instantiated_case_guarantee(
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
    ) -> Option<CaseGuarantee> {
        let binary = self.program.expression_table.binary_node(fact)?;
        if !crate::proof_contracts::bound_expression_meaning::has_exact_declaration_case_membership_meaning(
            self.program, scope, parameters, fact, binary,
        ) {
            return None;
        }
        let reference = crate::parameter_expression_result_type_reference(
            self.program,
            scope,
            parameters,
            binary.left,
        );
        let (definition, variant) = applied_case_classifier(self.program, binary.right, reference)?;
        let subject = structural_terms::structural_term(
            self.program,
            None,
            Some((scope, parameters)),
            binary.left,
            None,
        )?;
        Some(CaseGuarantee {
            subject: Self::substitute_call_term(&subject, map, machine_map),
            data: definition.symbol,
            case: variant.symbol,
        })
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
        if !self.has_exact_case_subjects() {
            return;
        }
        let mut expressions = HashMap::new();
        if !prepare_case_observation_expressions(self.program, fact, &mut expressions) {
            return;
        }
        self.extend_contract_case_guarantees(owner, fact, map, guarantees, &expressions);
    }

    fn extend_contract_case_guarantees(
        &self,
        owner: &specification_calls::SelectedCallContract<'_>,
        fact: ExpressionHandle,
        map: &[(String, StructuralTerm)],
        guarantees: &mut Vec<CaseGuarantee>,
        case_expressions: &HashMap<ExpressionHandle, bool>,
    ) {
        use specification_calls::{SelectedCallContract, owner_parameters};
        let Some(binary) = self.program.expression_table.binary_node(fact) else {
            return;
        };
        // Logical conjunction has fixed compiler meaning. Its children retain
        // their own selected meanings: a case classifier is not scalar equality.
        if binary.operator == BinaryOperator::And {
            self.extend_contract_case_guarantees(
                owner,
                binary.left,
                map,
                guarantees,
                case_expressions,
            );
            self.extend_contract_case_guarantees(
                owner,
                binary.right,
                map,
                guarantees,
                case_expressions,
            );
            return;
        }
        if binary.operator == BinaryOperator::Or {
            // A checked normal return establishes the complete disjunction,
            // not either alternative alone. Eliminate only a sibling that the
            // existing selected-declaration judgment independently refutes
            // under these preserved, completed argument observations.
            let scope = match owner {
                SelectedCallContract::Concrete(machine, _) => machine.symbol,
                _ => specification_calls::owner_symbol(owner),
            };
            let parameters = owner_parameters(self.program, owner);
            for (alternative, sibling) in [(binary.left, binary.right), (binary.right, binary.left)]
            {
                if case_expressions.get(&alternative) != Some(&true) {
                    continue;
                }
                if matches!(
                    self.captured_operand_predicate_judgment(scope, parameters, sibling, map),
                    StructuralJudgment::Refuted
                ) {
                    self.extend_contract_case_guarantees(
                        owner,
                        alternative,
                        map,
                        guarantees,
                        case_expressions,
                    );
                }
            }
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
        let reference = crate::parameter_expression_result_type_reference(
            self.program,
            scope,
            parameters,
            binary.left,
        );
        let Some((definition, variant)) =
            applied_case_classifier(self.program, binary.right, reference)
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
        self.intake_observed_case(
            guarantee.subject.clone(),
            guarantee.data,
            guarantee.case,
            true,
        );
    }

    pub(super) fn intake_observed_case(
        &mut self,
        subject: StructuralTerm,
        data: SymbolHandle,
        case: SymbolHandle,
        holds: bool,
    ) {
        Arc::make_mut(&mut self.case_facts).push(CaseFact {
            subject,
            data,
            case,
            holds,
        });
        self.activate_conditional_guarantees();
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
                    DataMember::Variant(variant) if variant.name.as_str() == case => {
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
        let mut excluded = Vec::new();
        for fact in self.case_facts.iter() {
            if fact.data == data_symbol
                && self.resolve_case_subject(fact.subject.clone(), 0).as_ref() == Some(&subject)
            {
                if fact.holds {
                    return if fact.case == case_symbol {
                        StructuralJudgment::Proven
                    } else {
                        StructuralJudgment::Refuted
                    };
                }
                if !excluded.contains(&fact.case) {
                    excluded.push(fact.case);
                }
            }
        }
        if excluded.contains(&case_symbol) {
            return StructuralJudgment::Refuted;
        }
        // After excluding cases, payload access is valid only if exactly one
        // declared case remains for this same value and type application.
        if !excluded.is_empty()
            && let Some(definition) = self.program.data_definition_by_symbol(data_symbol)
        {
            let mut remaining = self
                .program
                .data_members(definition)
                .iter()
                .filter_map(|member| member.as_variant())
                .filter(|variant| !excluded.contains(&variant.symbol));
            if let Some(only) = remaining.next()
                && remaining.next().is_none()
                && only.symbol == case_symbol
            {
                return StructuralJudgment::Proven;
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
            // The tag is already produced, regardless of whether a payload
            // has a tag denotation of its own. Resolving every field here
            // discarded valid constructors containing scalar computations.
            // Payload projection and equality own their selected reads.
            StructuralTerm::Constructor { .. } => Some(term),
            StructuralTerm::RepeatedArray { length, element } => {
                Some(StructuralTerm::RepeatedArray {
                    length,
                    element: match element {
                        Some(element) => {
                            Some(Box::new(self.resolve_case_subject(*element, depth + 1)?))
                        }
                        None => None,
                    },
                })
            }
            StructuralTerm::Integer(_)
            | StructuralTerm::Float(_)
            | StructuralTerm::ScalarRepresentation { .. }
            | StructuralTerm::BoundValue(_) => Some(term),
            StructuralTerm::CapturedValue { .. } => {
                let observed = self.completed_nominal_observation(&term);
                if observed != term {
                    self.resolve_case_subject(observed, depth + 1)
                } else {
                    Some(term)
                }
            }
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
            } => {
                let collection = self.resolve_case_subject(*collection, depth + 1)?;
                let index = self.resolve_case_subject(*index, depth + 1)?;
                Some(
                    observed_projections::uniform_element(&collection, &index).unwrap_or_else(
                        || StructuralTerm::IndexedValue {
                            element_type,
                            element_identity,
                            collection: Box::new(collection),
                            index: Box::new(index),
                        },
                    ),
                )
            }
            StructuralTerm::CallProjection { .. }
            | StructuralTerm::Opaque(_)
            | StructuralTerm::ScalarBinary { .. }
            | StructuralTerm::MathematicalBinary { .. }
            | StructuralTerm::IntegerEmbedding { .. }
            | StructuralTerm::CollectionMeasure { .. } => None,
        }
    }
}
