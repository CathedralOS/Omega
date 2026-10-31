//! Type equations proved from the same executed predicates as value bounds.
//!
//! The ordinary prefix owner authenticates subjects, selected cases, and write
//! effects. This consumer exports equations true on every arrival, pure tests
//! of completed subjects, and the cases still possible after subject completion.
//! Destination types and the operation's own guarantees are absent. No result
//! unreachable marker can erase an effectful pattern: only its independently
//! false tag prefix permits dispatch planning to omit that alternative.

use super::structural_judgment::StructuralJudgment;
use super::structural_prefix::{
    PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor, observe_runtime_state_prefix,
};
use symbols::SymbolKind;
use typed_trees::ProvenTypeContext;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn predicate_type_contexts(
    program: &dyn crate::CheckingInput,
) -> Vec<ProvenTypeContext> {
    let mut candidates = Vec::new();
    for (expression, _) in program.expression_table.expression_entries() {
        if let Some((left, right, _)) = program.static_type_predicate_operands(expression) {
            for reference in [left, right] {
                if typed_trees::domain::index_argument_is_closed(program, reference)
                    && !candidates.iter().any(|previous| {
                        program.normalized_type_identity(*previous)
                            == program.normalized_type_identity(reference)
                    })
                {
                    candidates.push(reference);
                }
            }
        }
    }
    let mut equations = Vec::new();
    for machine in program.machines() {
        let parameters = program.machine_type_parameters(machine);
        let formals = program
            .type_reference_table
            .type_reference_rows()
            .filter_map(|(reference, node)| {
                let TypeReferenceNode::Named { symbol, .. } = node else {
                    return None;
                };
                (symbol.is_valid()
                    && program.symbols.get(*symbol).kind == SymbolKind::TypeParameter
                    && parameters.iter().any(|parameter| {
                        parameter.symbol == *symbol
                            && matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                    }))
                .then_some(reference)
            })
            .collect::<Vec<_>>();
        for state in program.machine_states(machine) {
            let has_predicates = program.state_parameters(state).iter().any(|parameter| {
                type_carries_predicate_context(program, parameter.type_reference, &mut Vec::new())
            }) || type_carries_predicate_context(
                program,
                state.return_type,
                &mut Vec::new(),
            );
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let mut expressions = Vec::new();
                crate::collect_statement_expression_trees(program, statement, &mut expressions);
                expressions.sort_unstable_by_key(|expression| {
                    (expression.arena_index(), expression.generation())
                });
                expressions.dedup();
                for expression in expressions {
                    // Classifiers and type operands are semantic metadata. They
                    // are not runtime observations, so absence from the evaluator
                    // is not evidence that their enclosing arm is impossible.
                    let metadata = matches!(
                        program.expression_table.expression(expression),
                        typed_trees::expression::ExpressionNode::TypeExpression(_)
                    ) || matches!(program.expression_table.expression(expression),
                            typed_trees::expression::ExpressionNode::Name(path)
                            if path.symbol.is_valid() && program.symbols.get(path.symbol).kind == SymbolKind::Variant);
                    if !has_predicates || metadata {
                        equations.push(ProvenTypeContext {
                            expression,
                            equations: Vec::new(),
                            reachable: true,
                            case_value: None,
                            admitted_cases: None,
                        });
                        continue;
                    }
                    let mut visitor = TypeEquations {
                        site: StructuralPrefixSite {
                            state: state.symbol,
                            statement_index,
                            in_transition_target:
                                super::structural_prefix::expression_is_transition_target(
                                    program,
                                    state,
                                    statement_index,
                                    expression,
                                ),
                        },
                        expression,
                        equations: formals
                            .iter()
                            .flat_map(|formal| {
                                candidates
                                    .iter()
                                    .map(move |candidate| (*formal, *candidate))
                            })
                            .collect(),
                        case_value: None,
                        admitted_cases: None,
                        cases_observed: false,
                        observed: false,
                        reachable: false,
                        complete: true,
                    };
                    let complete =
                        observe_runtime_state_prefix(program, machine, state, &mut visitor)
                            .is_some()
                            && visitor.observed
                            && visitor.complete;
                    if complete {
                        equations.push(ProvenTypeContext {
                            expression,
                            equations: visitor.equations,
                            reachable: visitor.reachable,
                            case_value: visitor.case_value,
                            admitted_cases: visitor.admitted_cases,
                        });
                    } else {
                        // A reused expression must also account for occurrences
                        // whose prefix cannot be observed. Refusal proves nothing.
                        equations.push(ProvenTypeContext {
                            expression,
                            equations: Vec::new(),
                            reachable: true,
                            case_value: None,
                            admitted_cases: None,
                        });
                    }
                }
            }
        }
    }
    equations
}

struct TypeEquations {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    equations: Vec<(TypeReferenceHandle, TypeReferenceHandle)>,
    case_value: Option<bool>,
    admitted_cases: Option<Vec<symbols::SymbolHandle>>,
    cases_observed: bool,
    observed: bool,
    reachable: bool,
    complete: bool,
}

impl StructuralPrefixVisitor for TypeEquations {
    fn site(&self) -> StructuralPrefixSite {
        self.site
    }
    fn unresolved(&mut self, _: &typed_trees::machine::Machine) {
        self.complete = false;
    }
    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let (program, machine, state, judge, environment) = observation;
        // Observe the subject from this statement's original prefix, separately
        // from before-expression entries. Replaying the statement from an
        // already advanced entry would repeat earlier operand effects.
        if let typed_trees::expression::ExpressionNode::Match(dispatch) =
            program.expression_table.expression(self.expression)
        {
            let admitted = (|| {
                let reference = crate::expression_result_type_reference(
                    program,
                    machine,
                    state,
                    dispatch.subject,
                )?;
                let definition =
                    crate::field_domains::readable_nominal_definition(program, reference)?;
                let entries = judge.normal_value_entries(
                    self.site.statement_index,
                    dispatch.subject,
                    environment,
                    (machine, state, state.return_type),
                )?;
                let mut cases = Vec::new();
                for entry in entries {
                    if entry.judge.hypotheses_contradictory {
                        continue;
                    }
                    let [Some(value)] = entry.arguments.as_slice() else {
                        return None;
                    };
                    for variant in program
                        .data_members(definition)
                        .iter()
                        .filter_map(typed_trees::data::DataMember::as_variant)
                    {
                        if !matches!(
                            entry.judge.admitted_case_judgment(
                                dispatch.subject,
                                value,
                                reference,
                                definition.symbol,
                                variant.symbol
                            ),
                            StructuralJudgment::Refuted
                        ) && !cases.contains(&variant.symbol)
                        {
                            cases.push(variant.symbol);
                        }
                    }
                }
                Some(cases)
            })();
            if !self.cases_observed {
                self.admitted_cases = admitted;
                self.cases_observed = true;
            } else {
                match (&mut self.admitted_cases, admitted) {
                    (Some(previous), Some(cases)) => {
                        for case in cases {
                            if !previous.contains(&case) {
                                previous.push(case);
                            }
                        }
                    }
                    _ => self.admitted_cases = None,
                }
            }
        }
        let Some(entries) = judge.normal_expression_entries(
            self.site.statement_index,
            self.expression,
            environment,
            (machine, state, state.return_type),
        ) else {
            self.complete = false;
            return;
        };
        self.observed = true;
        for entry in entries {
            if entry.judge.hypotheses_contradictory {
                continue;
            }
            let case_value = (|| {
                let binary = program.expression_table.binary_node(self.expression)?;
                let typed_trees::expression::ExpressionNode::SavedSubject(saved) =
                    program.expression_table.expression(binary.left)
                else {
                    return None;
                };
                let (definition, variant) = super::structural_terms::case_guard_classifier(
                    program,
                    machine,
                    Some(state),
                    self.expression,
                )?;
                let reference =
                    crate::expression_result_type_reference(program, machine, state, binary.left)?;
                let selected = program.data_constructor_definition(reference, definition.symbol)?;
                let case =
                    program.data_constructor_case(reference, definition.symbol, variant.symbol)?;
                let subject = entry.judge.completed_match_subject(saved)?;
                match entry.judge.admitted_case_judgment(
                    binary.left,
                    &subject,
                    reference,
                    selected.symbol,
                    case.symbol,
                ) {
                    StructuralJudgment::Proven => Some(true),
                    StructuralJudgment::Refuted => Some(false),
                    StructuralJudgment::Unknown => None,
                }
            })();
            if !self.reachable {
                self.case_value = case_value;
            } else if self.case_value != case_value {
                self.case_value = None;
            }
            self.reachable = true;
            self.equations.retain(|(parameter, definition)| {
                super::structural_terms::static_type_equation_term(
                    program,
                    *parameter,
                    *definition,
                    false,
                    &[],
                )
                .is_some_and(|predicate| {
                    matches!(
                        entry.judge.judge_observed_predicate(predicate, 0),
                        StructuralJudgment::Proven
                    )
                })
            });
        }
    }
}

fn type_carries_predicate_context(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    active: &mut Vec<TypeReferenceHandle>,
) -> bool {
    if !reference.is_valid() || active.contains(&reference) {
        return false;
    }
    active.push(reference);
    let has_predicate = |facts| {
        program.proof_facts.span_or_empty(facts).iter().any(|fact| {
            let Some(expression) = fact.as_expression() else {
                return false;
            };
            let mut expressions = Vec::new();
            crate::expression_traversal::collect_expression_tree(
                program,
                *expression,
                &mut expressions,
            );
            expressions
                .into_iter()
                .any(|expression| program.static_type_predicate_operands(expression).is_some())
        })
    };
    // Numeric predicates on a sum can decide its cases even when no type
    // equation occurs anywhere in the program. Plain scalar bounds retain
    // their existing value-judgment owner and need no dispatch context.
    let sum = crate::field_domains::readable_nominal_definition(program, reference).is_some_and(
        |definition| {
            program
                .data_members(definition)
                .iter()
                .any(|member| member.as_variant().is_some())
        },
    );
    let relevant = |facts| {
        has_predicate(facts) || (sum && !program.proof_facts.span_or_empty(facts).is_empty())
    };
    let result = crate::field_domains::declared_domain_constraint_records(program, reference)
        .iter()
        .any(|constraint| {
            program
                .domain_definition(constraint.symbol)
                .is_some_and(|domain| relevant(domain.facts))
        })
        || crate::field_domains::readable_nominal_definition(program, reference).is_some_and(
            |definition| {
                relevant(definition.where_facts)
                    || definition
                        .generic_instance
                        .and_then(|application| {
                            let TypeReferenceNode::Generic { base_symbol, .. } =
                                program.type_reference_table.type_reference(application)
                            else {
                                return None;
                            };
                            program.unique_data_definition_by_symbol(*base_symbol)
                        })
                        .is_some_and(|template| relevant(template.where_facts))
                    || program
                        .data_members(definition)
                        .iter()
                        .any(|member| match member {
                            typed_trees::data::DataMember::Field(field) => {
                                type_carries_predicate_context(
                                    program,
                                    field.type_reference,
                                    active,
                                )
                            }
                            typed_trees::data::DataMember::Variant(variant) => {
                                !variant.where_facts.is_empty()
                                    || program.data_payload_fields(variant).iter().any(|field| {
                                        type_carries_predicate_context(
                                            program,
                                            field.type_reference,
                                            active,
                                        )
                                    })
                            }
                        })
            },
        );
    active.pop();
    result
}
