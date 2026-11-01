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
    observe_runtime_state_prefix_batched,
};
use symbols::SymbolKind;
use typed_trees::ProvenTypeContext;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Static worlds, including transition targets, consume conditional equations
/// derived from the same admitted-value judgment as executed expression cuts.
pub(crate) fn predicate_case_type_contexts(
    program: &dyn crate::CheckingInput,
) -> Vec<typed_trees::CaseMembershipTypeFacts> {
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
    let formal_references = TypeParameterReferences::new(program, |_| true);
    let mut contexts = Vec::new();
    for machine in program.machines() {
        let formals = formal_references.of(program.machine_type_parameters(machine));
        if formals.is_empty() {
            continue;
        }
        for state in program.machine_states(machine) {
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
                    let Some((definition, _)) = super::structural_terms::case_guard_classifier(
                        program,
                        machine,
                        Some(state),
                        expression,
                    ) else {
                        continue;
                    };
                    let Some(binary) = program.expression_table.binary_node(expression) else {
                        continue;
                    };
                    let relevant = !definition.where_facts.is_empty()
                        || crate::expression_result_type_reference(
                            program,
                            machine,
                            state,
                            binary.left,
                        )
                        .is_some_and(|reference| {
                            type_carries_predicate_context(program, reference, &mut Vec::new())
                        });
                    if !relevant {
                        continue;
                    }
                    let mut visitor = ConditionalCaseEquations {
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
                        candidates: formals
                            .iter()
                            .flat_map(|formal| {
                                candidates
                                    .iter()
                                    .map(move |candidate| (*formal, *candidate))
                            })
                            .collect(),
                        equations: None,
                        possible: false,
                        observed: false,
                        complete: true,
                    };
                    let complete =
                        observe_runtime_state_prefix(program, machine, state, &mut visitor)
                            .is_some()
                            && visitor.observed
                            && visitor.complete;
                    if super::struct_trace() {
                        eprintln!(
                            "STRUCT predicate-case machine={} state={} expression={:?} complete={} possible={} equations={:?}",
                            machine.name,
                            state.name,
                            expression,
                            complete,
                            visitor.possible,
                            visitor.equations
                        );
                    }
                    contexts.push(typed_trees::CaseMembershipTypeFacts {
                        membership: expression,
                        impossible: complete && !visitor.possible,
                        equations: if complete {
                            visitor.equations.unwrap_or_default()
                        } else {
                            Vec::new()
                        },
                    });
                }
            }
        }
    }
    // A reused node must satisfy every occurrence, including refused queries.
    contexts.sort_unstable_by_key(|context| {
        (
            context.membership.arena_index(),
            context.membership.generation(),
        )
    });
    let mut joined: Vec<typed_trees::CaseMembershipTypeFacts> = Vec::new();
    for context in contexts {
        if let Some(previous) = joined
            .last_mut()
            .filter(|previous| previous.membership == context.membership)
        {
            previous.impossible &= context.impossible;
            previous
                .equations
                .retain(|equation| context.equations.contains(equation));
        } else {
            joined.push(context);
        }
    }
    joined
}

/// The reference rows that name each machine type binder, found in one pass
/// over the table. Scanning every row for every machine made these queries
/// quadratic in program size.
struct TypeParameterReferences {
    by_binder: std::collections::HashMap<
        symbols::SymbolHandle,
        Vec<TypeReferenceHandle>,
        symbols::BuildSymbolHasher,
    >,
}

impl TypeParameterReferences {
    /// Only binders that `accepts_binder` admits can be named by a formal.
    fn new(
        program: &dyn crate::CheckingInput,
        accepts_binder: impl Fn(symbols::SymbolHandle) -> bool,
    ) -> Self {
        let mut by_binder = std::collections::HashMap::default();
        for machine in program.machines() {
            for parameter in program.machine_type_parameters(machine) {
                if matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                    && accepts_binder(parameter.symbol)
                {
                    by_binder.entry(parameter.symbol).or_insert_with(Vec::new);
                }
            }
        }
        for (reference, node) in program.type_reference_table.type_reference_rows() {
            if let TypeReferenceNode::Named { symbol, .. } = node
                && let Some(references) = by_binder.get_mut(symbol)
            {
                references.push(reference);
            }
        }
        Self { by_binder }
    }

    /// References naming a type binder of `parameters`, in table order.
    fn of(&self, parameters: &[typed_trees::data::TypeParameter]) -> Vec<TypeReferenceHandle> {
        let mut formals: Vec<TypeReferenceHandle> = parameters
            .iter()
            .filter(|parameter| {
                matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
            })
            .filter_map(|parameter| self.by_binder.get(&parameter.symbol))
            .flatten()
            .copied()
            .collect();
        formals.sort_unstable_by_key(|reference| (reference.arena_index(), reference.generation()));
        formals.dedup();
        formals
    }
}

struct ConditionalCaseEquations {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    candidates: Vec<(TypeReferenceHandle, TypeReferenceHandle)>,
    equations: Option<Vec<(TypeReferenceHandle, TypeReferenceHandle)>>,
    possible: bool,
    observed: bool,
    complete: bool,
}

impl StructuralPrefixVisitor for ConditionalCaseEquations {
    fn site(&self) -> StructuralPrefixSite {
        self.site
    }
    fn unresolved(&mut self, _: &typed_trees::machine::Machine) {
        self.complete = false;
    }
    fn observe<'program>(
        &mut self,
        (program, machine, state, judge, environment): PrefixObservation<'_, 'program>,
    ) {
        let Some((definition, variant)) = super::structural_terms::case_guard_classifier(
            program,
            machine,
            Some(state),
            self.expression,
        ) else {
            self.complete = false;
            return;
        };
        let Some(binary) = program.expression_table.binary_node(self.expression) else {
            self.complete = false;
            return;
        };
        let Some(reference) =
            crate::expression_result_type_reference(program, machine, state, binary.left)
        else {
            self.complete = false;
            return;
        };
        let Some(entries) = judge.normal_value_entries(
            self.site.statement_index,
            binary.left,
            environment,
            (machine, state, state.return_type),
        ) else {
            self.complete = false;
            return;
        };
        self.observed = true;
        for entry in entries {
            let [Some(value)] = entry.arguments.as_slice() else {
                self.complete = false;
                return;
            };
            let Some((impossible, equations)) = entry.judge.conditional_case_type_equations(
                binary.left,
                value,
                reference,
                definition.symbol,
                variant.symbol,
                &self.candidates,
            ) else {
                self.complete = false;
                return;
            };
            if impossible {
                continue;
            }
            self.possible = true;
            if let Some(previous) = &mut self.equations {
                previous.retain(|equation| equations.contains(equation));
            } else {
                self.equations = Some(equations);
            }
        }
    }
}

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
    let formal_references = TypeParameterReferences::new(program, |symbol| {
        symbol.is_valid() && program.symbols.get(symbol).kind == SymbolKind::TypeParameter
    });
    let mut equations = Vec::new();
    for machine in program.machines() {
        let formals = formal_references.of(program.machine_type_parameters(machine));
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
                // An expression's prefix depends on its statement and whether it
                // sits in the transition target, not on the expression itself.
                // Walk each such site once for all of its expressions: the walk
                // explores every arm and admits each arm's data predicates, so
                // one walk per expression multiplied that work by the
                // statement's size.
                let mut visitors = Vec::new();
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
                        visitors.push((None, expression));
                        continue;
                    }
                    let visitor = TypeEquations {
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
                    visitors.push((Some((visitor, false)), expression));
                }
                for in_transition_target in [false, true] {
                    let mut group = visitors
                        .iter_mut()
                        .filter_map(|(visitor, _)| visitor.as_mut())
                        .filter(|(visitor, _)| {
                            visitor.site.in_transition_target == in_transition_target
                        })
                        .collect::<Vec<_>>();
                    if group.is_empty() {
                        continue;
                    }
                    let walked = {
                        let mut members = group
                            .iter_mut()
                            .map(|(visitor, _)| visitor as &mut dyn StructuralPrefixVisitor)
                            .collect::<Vec<_>>();
                        phase_profile::scope("05.overloads.4a_prefix_observation", || {
                            observe_runtime_state_prefix_batched(
                                program,
                                machine,
                                state,
                                &mut members,
                            )
                            .is_some()
                        })
                    };
                    for (_, walked_here) in group {
                        *walked_here = walked;
                    }
                }
                for (visitor, expression) in visitors {
                    match visitor {
                        Some((visitor, true)) if visitor.observed && visitor.complete => {
                            equations.push(ProvenTypeContext {
                                expression,
                                equations: visitor.equations,
                                reachable: visitor.reachable,
                                case_value: visitor.case_value,
                                admitted_cases: visitor.admitted_cases,
                            });
                        }
                        // A reused expression must also account for occurrences
                        // whose prefix cannot be observed. Refusal proves nothing.
                        // Metadata and predicate-free states are always reached.
                        _ => equations.push(ProvenTypeContext {
                            expression,
                            equations: Vec::new(),
                            reachable: true,
                            case_value: None,
                            admitted_cases: None,
                        }),
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

#[cfg(test)]
mod tests {
    use super::TypeParameterReferences;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::data::{TypeParameter, TypeParameterKind};
    use typed_trees::machine::Machine;
    use typed_trees::name::Identifier;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    fn named(program: &mut TypedTrees, symbol: SymbolHandle) -> TypeReferenceHandle {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: Identifier::from("T"),
            })
    }

    #[test]
    fn formals_are_the_rows_naming_a_machine_type_binder_in_table_order() {
        let first = SymbolHandle::from_parts(1, 1);
        let second = SymbolHandle::from_parts(2, 1);
        let value = SymbolHandle::from_parts(3, 1);
        let other = SymbolHandle::from_parts(4, 1);
        let mut program = TypedTrees::default();
        let parameters = program.data_type_parameters.insert_many([
            TypeParameter {
                symbol: first,
                ..TypeParameter::default()
            },
            TypeParameter {
                symbol: second,
                ..TypeParameter::default()
            },
            TypeParameter {
                symbol: value,
                kind: TypeParameterKind::Const {
                    type_reference: TypeReferenceHandle::invalid(),
                },
                ..TypeParameter::default()
            },
        ]);
        program.push_machine(Machine {
            type_parameters: parameters,
            ..Machine::default()
        });
        let on_second = named(&mut program, second);
        named(&mut program, other);
        let on_first = named(&mut program, first);
        named(&mut program, value);
        let on_second_again = named(&mut program, second);
        let machine = program.machines()[0].clone();

        let references = TypeParameterReferences::new(&program, |_| true);
        assert_eq!(
            references.of(program.machine_type_parameters(&machine)),
            [on_second, on_first, on_second_again]
        );
        // A binder the pass rejects names no formal, and no binders name none.
        let references = TypeParameterReferences::new(&program, |symbol| symbol != second);
        assert_eq!(
            references.of(program.machine_type_parameters(&machine)),
            [on_first]
        );
        assert!(references.of(&[]).is_empty());
    }
}
