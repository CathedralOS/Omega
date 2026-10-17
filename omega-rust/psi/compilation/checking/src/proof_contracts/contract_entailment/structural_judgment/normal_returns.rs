//! Selected guarantees for captured normal-return values.
//!
//! A runtime result is one occurrence, not a replayable mathematical call.
//! Capture operands before substituting the selected public signature, prove
//! its entry requirements, then import its unconditional normal guarantees.
//! This environment belongs to one executed normal-return path; it is neither
//! a loop invariant nor permission to replay mutable storage observations.

use super::super::specification_calls::{
    SelectedCallContract, owner_contracts, owner_leading_operands, owner_parameter_scope,
    owner_return_type, owner_symbol, selected_call_contract,
};
use super::selected_contracts::independent_application;
use super::value_predicates::boolean_term;
use super::{StructuralJudge, StructuralJudgment, StructuralTerm};
use crate::expression_traversal::children;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern};
use typed_trees::signature::SignatureContractKind;
use typed_trees::types::TypeReferenceHandle;

pub(in crate::proof_contracts::contract_entailment) struct NormalReturnPath<'program> {
    pub value: StructuralTerm,
    pub judge: StructuralJudge<'program>,
}

pub(in crate::proof_contracts::contract_entailment) struct NormalOperationEntry<'program> {
    pub arguments: Vec<StructuralTerm>,
    pub judge: StructuralJudge<'program>,
}

/// A consumer may demand completed fields without using its enclosing value.
/// Required values still use the same strict normal-return operation.
#[derive(Clone, Copy)]
enum NormalValueDemand<'query> {
    Value,
    // Evaluate a checked storage coordinate without observing aggregate contents.
    Location,
    Observe(&'query [ExpressionHandle]),
}

#[derive(Clone, Copy)]
struct NormalOperand<'query> {
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    demand: NormalValueDemand<'query>,
}

type ValueScope<'scope> = (
    &'scope typed_trees::machine::Machine,
    &'scope typed_trees::state::State,
    TypeReferenceHandle,
);

impl<'program> StructuralJudge<'program> {
    pub(in crate::proof_contracts::contract_entailment) fn completed_expression_values(
        &self,
    ) -> &[(ExpressionHandle, StructuralTerm)] {
        self.normal_results.as_slice()
    }

    /// Projection demands carried by this exact completed result's public
    /// equations. The consumer authenticates each field against its declared
    /// type; unrelated captures, hypotheses and private bodies contribute none.
    pub(in crate::proof_contracts::contract_entailment) fn completed_value_projection_paths(
        &self,
        value: &StructuralTerm,
    ) -> Vec<String> {
        let (root, prefix) = match value.semantic_value() {
            StructuralTerm::Projection { subject, path } => (subject.as_ref(), Some(path.as_str())),
            _ => (value, None),
        };
        let mut paths = Vec::new();
        for (left, right) in self.captured_result_equations.iter() {
            for term in [left, right] {
                let StructuralTerm::Projection { subject, path } = term else {
                    continue;
                };
                if subject.as_ref() != root {
                    continue;
                }
                let path = if let Some(prefix) = prefix {
                    let Some(suffix) = path
                        .strip_prefix(prefix)
                        .and_then(|suffix| suffix.strip_prefix('.'))
                    else {
                        continue;
                    };
                    suffix
                } else {
                    path.as_str()
                };
                if !paths.iter().any(|previous| previous == path) {
                    paths.push(path.to_owned());
                }
            }
        }
        paths
    }

    /// Resolve captured value identities without folding arithmetic or using
    /// caller hypotheses. Discovery needs the producing operand structure;
    /// ring equality cannot replace its intermediate formation obligations.
    pub(in crate::proof_contracts::contract_entailment) fn completed_value_identity(
        &self,
        value: StructuralTerm,
    ) -> StructuralTerm {
        self.completed_value_identity_at(value, 0)
    }

    fn completed_value_identity_at(&self, value: StructuralTerm, depth: usize) -> StructuralTerm {
        if depth >= 64 {
            return value;
        }
        let value = match value {
            StructuralTerm::IntegerEmbedding { reference, value } => {
                StructuralTerm::IntegerEmbedding {
                    reference,
                    value: Box::new(self.completed_value_identity_at(*value, depth + 1)),
                }
            }
            StructuralTerm::MathematicalBinary {
                operator,
                left,
                right,
            } => StructuralTerm::MathematicalBinary {
                operator,
                left: Box::new(self.completed_value_identity_at(*left, depth + 1)),
                right: Box::new(self.completed_value_identity_at(*right, depth + 1)),
            },
            StructuralTerm::ObservedValue {
                observed_type,
                value,
                descriptor,
            } => StructuralTerm::ObservedValue {
                observed_type,
                value: Box::new(self.completed_value_identity_at(*value, depth + 1)),
                descriptor: descriptor.map(|descriptor| {
                    Box::new(self.completed_value_identity_at(*descriptor, depth + 1))
                }),
            },
            StructuralTerm::Constructor { data, case, fields } => StructuralTerm::Constructor {
                data,
                case,
                fields: fields
                    .into_iter()
                    .map(|(name, value)| (name, self.completed_value_identity_at(value, depth + 1)))
                    .collect(),
            },
            StructuralTerm::ScalarBinary {
                references,
                operator,
                meaning,
                left,
                right,
            } => StructuralTerm::ScalarBinary {
                references,
                operator,
                meaning,
                left: Box::new(self.completed_value_identity_at(*left, depth + 1)),
                right: Box::new(self.completed_value_identity_at(*right, depth + 1)),
            },
            StructuralTerm::CollectionMeasure {
                measure,
                collection,
            } => StructuralTerm::CollectionMeasure {
                measure,
                collection: Box::new(
                    self.completed_value_identity_at(*collection, depth + 1)
                        .into_collection_descriptor(),
                ),
            },
            StructuralTerm::Projection { subject, path } => {
                let subject = self.completed_value_identity_at(*subject, depth + 1);
                path.split('.')
                    .try_fold(
                        subject.clone(),
                        super::super::observed_projections::project_field,
                    )
                    .unwrap_or(StructuralTerm::Projection {
                        subject: Box::new(subject),
                        path,
                    })
            }
            value => value,
        };
        if !matches!(
            value.semantic_value(),
            StructuralTerm::CapturedValue { .. }
                | StructuralTerm::Projection { .. }
                | StructuralTerm::CollectionMeasure { .. }
        ) {
            return value;
        }
        for (left, right) in self.captured_result_equations.iter() {
            for (pattern, replacement) in [(left, right), (right, left)] {
                if pattern == &value
                    && replacement != &value
                    && !super::super::structural_terms::term_contains(replacement, &value)
                {
                    return self.completed_value_identity_at(replacement.clone(), depth + 1);
                }
            }
        }
        value
    }

    /// Project normal outcomes under their own invocation facts. Every eager
    /// operand executes in order; selective operands fork the same path owner.
    /// No branch's hypotheses are donated to a different normal outcome.
    pub(in crate::proof_contracts::contract_entailment) fn normal_return_paths(
        &self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        self.clone().evaluate_normal_paths(
            expression,
            environment,
            scope,
            0,
            ExpressionHandle::invalid(),
            &mut Vec::new(),
            NormalValueDemand::Value,
        )
    }

    /// Observe an operation after its operands complete, including preceding
    /// statement roots such as named-transition arguments. The snapshot is
    /// before the operation itself: a call cannot donate its own guarantee,
    /// and a cast cannot supply its own destination range.
    pub(in crate::proof_contracts::contract_entailment) fn normal_operation_entries(
        &self,
        statement_index: usize,
        call: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
    ) -> Option<Vec<NormalOperationEntry<'program>>> {
        let operands = self.statement_observation_operands(
            statement_index,
            call,
            scope,
            NormalValueDemand::Value,
        )?;
        let mut entries = Vec::new();
        self.clone().evaluate_normal_operands(
            &operands,
            environment,
            scope,
            0,
            call,
            &mut entries,
        )?;
        (!entries.is_empty()).then_some(entries)
    }

    /// Completed value observers use the same ordered statement operands as
    /// call-premise observers. Select the recorded occurrence after evaluation;
    /// no standalone replay can omit an earlier sibling's effects.
    pub(in crate::proof_contracts::contract_entailment) fn completed_statement_paths(
        &self,
        statement_index: usize,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        let operands = self.statement_observation_operands(
            statement_index,
            expression,
            scope,
            NormalValueDemand::Value,
        )?;
        let contexts = self.clone().evaluate_normal_operands(
            &operands,
            environment,
            scope,
            0,
            ExpressionHandle::invalid(),
            &mut Vec::new(),
        )?;
        let paths = contexts
            .into_iter()
            .filter_map(|(judge, _)| {
                let value = judge
                    .normal_results
                    .iter()
                    .find(|(source, _)| *source == expression)?
                    .1
                    .clone();
                Some(NormalReturnPath { value, judge })
            })
            .collect::<Vec<_>>();
        (!paths.is_empty()).then_some(paths)
    }

    /// Observe requested field operands through normal evaluation without
    /// requiring an unrelated enclosing result to have a mathematical term.
    pub(in crate::proof_contracts::contract_entailment) fn completed_observation_paths(
        &self,
        statement_index: usize,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        observations: &[ExpressionHandle],
    ) -> Option<Vec<Self>> {
        let operands = self.statement_observation_operands(
            statement_index,
            expression,
            scope,
            NormalValueDemand::Observe(observations),
        )?;
        Some(
            self.clone()
                .evaluate_normal_operands(
                    &operands,
                    environment,
                    scope,
                    0,
                    ExpressionHandle::invalid(),
                    &mut Vec::new(),
                )?
                .into_iter()
                .map(|(judge, _)| judge)
                .collect(),
        )
    }

    fn statement_observation_operands<'query>(
        &self,
        statement_index: usize,
        expression: ExpressionHandle,
        scope: ValueScope<'_>,
        demand: NormalValueDemand<'query>,
    ) -> Option<Vec<NormalOperand<'query>>> {
        let roots = super::super::structural_prefix::expression_prefix_roots(
            self.program,
            scope.1,
            statement_index,
            expression,
        )?;
        Some(
            roots
                .into_iter()
                .map(|root| {
                    let reference = crate::expression_result_type_reference(
                        self.program,
                        scope.0,
                        scope.1,
                        root,
                    )
                    .unwrap_or(scope.2);
                    NormalOperand {
                        expression: root,
                        reference,
                        demand: if self
                            .program
                            .statement_table
                            .statements(scope.1.statement_nodes)
                            .get(statement_index)
                            .and_then(|statement| statement.as_assignment())
                            .is_some_and(|assignment| assignment.target == root)
                        {
                            NormalValueDemand::Location
                        } else {
                            demand
                        },
                    }
                })
                .collect(),
        )
    }

    /// A store captures its destination before evaluating the replacement.
    /// The location demand completes selectors without reading or moving the
    /// old destination. Its captured backing and selector values then survive
    /// RHS effects; the final write retires current contents before installation.
    pub(in crate::proof_contracts::contract_entailment) fn normal_assignment_paths(
        &self,
        statement: &typed_trees::statement::StatementNode,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        let assignment = statement.as_assignment()?;
        let reference =
            crate::assignment_stored_value_type(self.program, scope.0, scope.1, assignment)?;
        let scope = (scope.0, scope.1, reference);
        let destinations = self.clone().evaluate_normal_paths(
            assignment.target,
            environment,
            scope,
            0,
            ExpressionHandle::invalid(),
            &mut Vec::new(),
            NormalValueDemand::Location,
        )?;
        let mut paths = Vec::new();
        for destination in destinations {
            let captured = destination
                .judge
                .capture_store_destination(scope.0, scope.1, statement)?;
            for mut result in destination.judge.evaluate_normal_paths(
                assignment.value,
                environment,
                scope,
                0,
                ExpressionHandle::invalid(),
                &mut Vec::new(),
                NormalValueDemand::Value,
            )? {
                result.judge.observe_captured_store(
                    scope.1,
                    captured.clone(),
                    result.value.clone(),
                )?;
                paths.push(result);
            }
        }
        Some(paths)
    }

    /// Complete a declaration's actuals in source order before checking its
    /// receiving contract. Named transitions and expression calls share the
    /// same operand scheduler; the destination's ensures are never available.
    pub(in crate::proof_contracts::contract_entailment) fn normal_argument_entries(
        &self,
        arguments: &[ExpressionHandle],
        parameters: &[typed_trees::signature::StateParameter],
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
    ) -> Option<Vec<NormalOperationEntry<'program>>> {
        if arguments.len() != parameters.len() {
            return None;
        }
        let operands = arguments
            .iter()
            .zip(parameters)
            .map(|(expression, parameter)| NormalOperand {
                expression: *expression,
                reference: parameter.type_reference,
                demand: NormalValueDemand::Value,
            })
            .collect::<Vec<_>>();
        Some(
            self.clone()
                .evaluate_normal_operands(
                    &operands,
                    environment,
                    scope,
                    0,
                    ExpressionHandle::invalid(),
                    &mut Vec::new(),
                )?
                .into_iter()
                .map(|(judge, values)| NormalOperationEntry {
                    arguments: values.into_iter().map(|(_, value)| value).collect(),
                    judge,
                })
                .collect(),
        )
    }

    fn evaluate_normal_operands(
        self,
        operands: &[NormalOperand<'_>],
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        depth: usize,
        requested_operation: ExpressionHandle,
        entries: &mut Vec<NormalOperationEntry<'program>>,
    ) -> Option<Vec<(Self, Vec<(ExpressionHandle, StructuralTerm)>)>> {
        let mut contexts = vec![(self, Vec::new())];
        for &NormalOperand {
            expression: child,
            reference,
            demand,
        } in operands
        {
            let mut next = Vec::new();
            for (context, inputs) in contexts {
                for path in context.evaluate_normal_paths(
                    child,
                    environment,
                    (scope.0, scope.1, reference),
                    depth + 1,
                    requested_operation,
                    entries,
                    demand,
                )? {
                    let mut inputs = inputs.clone();
                    inputs.push((child, path.value));
                    next.push((path.judge, inputs));
                }
            }
            contexts = next;
        }
        Some(contexts)
    }

    fn evaluate_normal_paths(
        mut self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        depth: usize,
        requested_operation: ExpressionHandle,
        entries: &mut Vec<NormalOperationEntry<'program>>,
        demand: NormalValueDemand<'_>,
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        if depth >= 64 || !program_expression_is_live(self.program, expression) {
            return None;
        }
        let program = self.program;
        let node = program.expression_table.expression(expression);
        let demand = match demand {
            NormalValueDemand::Location => {
                let is_location = match node {
                    ExpressionNode::Name(_) => true,
                    ExpressionNode::Member(_) => {
                        discarded_value_has_no_own_effects(program, scope, expression)
                    }
                    ExpressionNode::Indexed(_) => {
                        crate::value_custody::places::place_has_builtin_coordinates(
                            program,
                            scope.0,
                            Some(scope.1),
                            expression,
                        )
                    }
                    _ => false,
                };
                if is_location {
                    NormalValueDemand::Location
                } else {
                    NormalValueDemand::Value
                }
            }
            NormalValueDemand::Observe(observations)
                if observations.contains(&expression)
                    || !discarded_value_has_no_own_effects(program, scope, expression) =>
            {
                NormalValueDemand::Value
            }
            demand => demand,
        };
        if let ExpressionNode::Binary(binary) = node
            && matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
            && crate::has_builtin_binary_expression_meaning(
                program,
                scope.0,
                Some(scope.1),
                expression,
            )
        {
            let evaluate_right = binary.operator == BinaryOperator::And;
            let mut outcomes = Vec::new();
            for left in self.evaluate_normal_paths(
                binary.left,
                environment,
                scope,
                depth + 1,
                requested_operation,
                entries,
                NormalValueDemand::Value,
            )? {
                for truth in [false, true] {
                    let mut selected = left.judge.clone();
                    let guard_value = boolean_term(truth);
                    if matches!(
                        selected.judge_equation(
                            selected.resolve(left.value.clone()),
                            guard_value.clone(),
                            0
                        ),
                        StructuralJudgment::Refuted
                    ) {
                        continue;
                    }
                    selected.assume_normal_equation(left.value.clone(), guard_value);
                    if truth == evaluate_right {
                        for right in selected.evaluate_normal_paths(
                            binary.right,
                            environment,
                            scope,
                            depth + 1,
                            requested_operation,
                            entries,
                            demand,
                        )? {
                            outcomes.push(right.at_expression(expression));
                        }
                    } else {
                        outcomes.push(
                            NormalReturnPath {
                                value: boolean_term(truth),
                                judge: selected,
                            }
                            .at_expression(expression),
                        );
                    }
                }
            }
            return Some(outcomes);
        }
        if let ExpressionNode::Match(dispatch) = node {
            let mut outcomes = Vec::new();
            let mut remaining = self.evaluate_normal_paths(
                dispatch.subject,
                environment,
                scope,
                depth + 1,
                requested_operation,
                entries,
                NormalValueDemand::Value,
            )?;
            for arm in program.expression_table.match_arms(dispatch.arms) {
                let mut next_remaining = Vec::new();
                for subject in remaining {
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        // Patterns execute only after every preceding pattern
                        // failed. Keep their guarantees in that remaining path.
                        for tested in subject.judge.evaluate_normal_paths(
                            pattern,
                            environment,
                            scope,
                            depth + 1,
                            requested_operation,
                            entries,
                            NormalValueDemand::Value,
                        )? {
                            let primitive = crate::match_subject_primitive_type(program, dispatch);
                            let equation_meaning = primitive.is_some_and(|primitive| {
                                primitive == typed_trees::types::PrimitiveType::Bool
                                    || primitive.is_integer()
                            });
                            let comparison = if equation_meaning {
                                tested.judge.judge_equation(
                                    tested.judge.resolve(subject.value.clone()),
                                    tested.judge.resolve(tested.value.clone()),
                                    0,
                                )
                            } else {
                                StructuralJudgment::Unknown
                            };
                            if !matches!(comparison, StructuralJudgment::Proven) {
                                let mut skipped = tested.judge.clone();
                                if primitive == Some(typed_trees::types::PrimitiveType::Bool) {
                                    let pattern = skipped.resolve(tested.value.clone());
                                    for truth in [false, true] {
                                        if matches!(
                                            skipped.judge_equation(
                                                pattern.clone(),
                                                boolean_term(truth),
                                                0
                                            ),
                                            StructuralJudgment::Proven
                                        ) {
                                            skipped.assume_normal_equation(
                                                subject.value.clone(),
                                                boolean_term(!truth),
                                            );
                                            break;
                                        }
                                    }
                                }
                                next_remaining.push(NormalReturnPath {
                                    value: subject.value.clone(),
                                    judge: skipped,
                                });
                            }
                            if matches!(comparison, StructuralJudgment::Refuted) {
                                continue;
                            }
                            let mut selected = tested.judge;
                            if equation_meaning {
                                selected
                                    .assume_normal_equation(subject.value.clone(), tested.value);
                            }
                            for value in selected.evaluate_normal_paths(
                                arm.value,
                                environment,
                                scope,
                                depth + 1,
                                requested_operation,
                                entries,
                                demand,
                            )? {
                                outcomes.push(value.at_expression(expression));
                            }
                        }
                    } else {
                        for value in subject.judge.evaluate_normal_paths(
                            arm.value,
                            environment,
                            scope,
                            depth + 1,
                            requested_operation,
                            entries,
                            demand,
                        )? {
                            outcomes.push(value.at_expression(expression));
                        }
                    }
                }
                remaining = next_remaining;
                if matches!(arm.pattern, MatchPattern::Wildcard) {
                    break;
                }
            }
            return Some(outcomes);
        }
        let writes = if let ExpressionNode::Call(call) = node
            && !call_has_mathematical_denotation(program, call)
        {
            let machine = program.machine(scope.0.symbol)?;
            let writes = self
                .frame_resolver()?
                .expression_may_write_paths(machine, expression);
            if super::super::struct_trace() {
                eprintln!(
                    "STRUCT normal-write-frame caller={} expression={expression:?} writes={writes:?}",
                    scope.0.name
                );
            }
            let writes = writes?;
            // Mathematical body substitution has no runtime storage epochs.
            // It cannot unfold effectful calls as repeatable applications.
            // Case-only paths separately establish tag preservation.
            if !writes.is_empty() && !self.runtime_body_values && !self.case_facts_only {
                return None;
            }
            writes
        } else {
            Vec::new()
        };
        // Retire a prior evaluation of this static node. Repeated edges are
        // distinct invocations; saved earlier values already retain their own
        // capture sequence and equations in the caller environment.
        std::sync::Arc::make_mut(&mut self.normal_results)
            .retain(|(source, _)| *source != expression);
        let integer_field_read = self.requires_value_preservation()
            && super::super::observed_projections::declared_integer_field_read(
                program, scope.0, scope.1, expression,
            )
            .is_some();
        let mut children_to_evaluate = Vec::new();
        let namespace_receiver = program
            .expression_table
            .call_node(expression)
            .filter(|call| super::super::structural_terms::static_call_receiver(program, call))
            .map(|call| call.receiver);
        children(program, node, |child| {
            if child.is_valid() && Some(child) != namespace_receiver {
                children_to_evaluate.push(child);
            }
        });
        let operands = children_to_evaluate
            .into_iter()
            .map(|child| {
                // Constructor operands land in their selected field declaration,
                // rather than inheriting the aggregate's carrier. This is the
                // same qualification retained by the construction owner and lets
                // anonymous arithmetic form under its actual receiving field.
                let child_reference = if let ExpressionNode::StructLiteral(literal) = node {
                    program
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .find(|field| field.value == child)
                        .and_then(|field| {
                            crate::value_custody::struct_literals::selected_construction_field_type(
                                program,
                                literal,
                                field.field_symbol,
                            )
                        })
                        .unwrap_or(scope.2)
                } else {
                    scope.2
                };
                let child_demand = match node {
                    ExpressionNode::Member(member)
                        if (integer_field_read
                            || matches!(demand, NormalValueDemand::Location))
                            && child == member.receiver =>
                    {
                        NormalValueDemand::Location
                    }
                    ExpressionNode::Indexed(indexed)
                        if matches!(demand, NormalValueDemand::Location) =>
                    {
                        if child == indexed.collection {
                            NormalValueDemand::Location
                        } else {
                            NormalValueDemand::Value
                        }
                    }
                    _ => demand,
                };
                NormalOperand {
                    expression: child,
                    reference: child_reference,
                    demand: child_demand,
                }
            })
            .collect::<Vec<_>>();
        let receiver_write_sequence = self.storage_write_sequence;
        let contexts = self.evaluate_normal_operands(
            &operands,
            environment,
            scope,
            depth,
            requested_operation,
            entries,
        )?;
        contexts
            .into_iter()
            .map(|(mut judge, inputs)| {
                if matches!(
                    demand,
                    NormalValueDemand::Observe(_) | NormalValueDemand::Location
                ) {
                    // Only the completed descendants are requested. No value
                    // identity or equation is published for this unused result.
                    return Some(Some(NormalReturnPath {
                        value: StructuralTerm::Opaque("unrequested result".to_owned()),
                        judge,
                    }));
                }
                let call_inputs = if let Some(call) = program.expression_table.call_node(expression)
                {
                    let mut cursor = 0;
                    Some(
                        program
                            .expression_table
                            .expression_handles(call.arguments)
                            .iter()
                            .map(|source| {
                                let offset = inputs[cursor..]
                                    .iter()
                                    .position(|(input, _)| input == source)?;
                                cursor += offset + 1;
                                Some(inputs[cursor - 1].1.clone())
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                } else {
                    None
                };
                if requested_operation == expression {
                    entries.push(NormalOperationEntry {
                        arguments: call_inputs.clone().unwrap_or_else(|| {
                            inputs.iter().map(|(_, value)| value.clone()).collect()
                        }),
                        judge: judge.clone(),
                    });
                    // Nothing after this operation is part of its incoming
                    // proof context. A stopped path is not an executed result.
                    return Some(None);
                }
                judge.complete_normal_call(
                    expression,
                    call_inputs.as_deref(),
                    environment,
                    scope,
                    &writes,
                )?;
                let receiver_changed =
                    integer_field_read && judge.storage_write_sequence != receiver_write_sequence;
                let captured_field = if receiver_changed {
                    let ExpressionNode::Member(member) = node else {
                        return None;
                    };
                    let receiver_type = crate::expression_result_type_reference(
                        program,
                        scope.0,
                        scope.1,
                        member.receiver,
                    )?;
                    // An actual owned operand can expose its completed field.
                    // A reference descriptor cannot stand in for its referent.
                    if crate::reference_shell(program, receiver_type).is_none() {
                        inputs
                            .iter()
                            .find(|(source, _)| *source == member.receiver)
                            .and_then(|(_, value)| {
                                judge.observed_field_term(value.clone(), member.member.as_str(), 0)
                            })
                            .filter(|value| value.observation_identity_key().is_some())
                    } else {
                        None
                    }
                } else {
                    None
                };
                let mut value = if receiver_changed {
                    // The receiver selected its base before a later operand
                    // wrote storage. Reconstructing it from the live place now
                    // could silently select a replacement descriptor.
                    match captured_field {
                        Some(value) => value,
                        None => judge.capture_unknown_value(expression, scope)?,
                    }
                } else if matches!(node, ExpressionNode::Atomic(_)) {
                    // The atomic owner distinguishes an observed load from
                    // update carriers. Its value is a fresh event, not the
                    // place's prior initializer or the modeled new resident.
                    crate::value_custody::atomic_operations::atomic_load_carrier(
                        program, expression,
                    )?;
                    judge.capture_unknown_value(expression, scope)?
                } else if let Some(value) =
                    judge.callee_term_in_scope(Some(scope), expression, environment, 0)
                {
                    value
                } else if judge.runtime_body_values
                    && (matches!(node, ExpressionNode::Call(_))
                        || judge.storage_observation_invalidated(expression)
                        || discarded_value_has_no_own_effects(program, scope, expression)
                        || matches!(node, ExpressionNode::Cast(cast)
                            if !cast.form.is_recast()
                                && program.primitive_type_reference(cast.target_type).is_some()))
                {
                    // Completion does not require a mathematical denotation.
                    // Every child has already run, and the own-effect owner
                    // above handled calls/atomics. Pure construction and
                    // primitive conversion may therefore retain a fresh value
                    // with no equations. In particular an earlier address cast
                    // must not erase unrelated premises for a later cast.
                    judge.capture_unknown_value(expression, scope)?
                } else {
                    return None;
                };
                if let (StructuralTerm::Application { arguments, .. }, Some(inputs)) =
                    (&mut value, call_inputs)
                {
                    *arguments = inputs;
                }
                if let ExpressionNode::StructLiteral(literal) = node {
                    let fields = program.expression_table.struct_fields(literal.fields);
                    value = super::super::structural_terms::constructor_literal_term(
                        program,
                        literal,
                        |field| {
                            let position = fields
                                .iter()
                                .position(|source| source.field_symbol == field.field_symbol)?;
                            inputs.get(position).map(|(_, value)| value.clone())
                        },
                    )?;
                }
                if let ExpressionNode::Binary(_) = node {
                    let [(_, left), (_, right)] = inputs.as_slice() else {
                        return None;
                    };
                    value = super::super::structural_terms::binary_term_in_scope(
                        program,
                        Some(scope),
                        None,
                        expression,
                        left.clone(),
                        right.clone(),
                    )?;
                }
                if let ExpressionNode::Indexed(_) = node {
                    let [(_, collection), (_, index)] = inputs.as_slice() else {
                        return None;
                    };
                    if let Some(observed) = super::super::structural_terms::indexed_term_in_scope(
                        program,
                        Some(scope),
                        None,
                        expression,
                        collection.clone(),
                        index.clone(),
                    ) {
                        value = observed;
                    } else if !judge.runtime_body_values {
                        return None;
                    }
                    // A completed runtime read still has its captured scalar
                    // value when its collection's contents have no structural
                    // denotation. A stable descriptor alone cannot reconstruct
                    // an element identity from an earlier storage epoch.
                }
                if integer_field_read {
                    // Completing a typed read observes its actual value even
                    // when no mathematical entry value is known. Retain that
                    // snapshot at its storage coordinate until the write owner
                    // retires it; source text is never its identity.
                    if value.observation_identity_key().is_none() {
                        value = judge.capture_unknown_value(expression, scope)?;
                    }
                    if !receiver_changed {
                        judge.observe_completed_storage_read(
                            scope.0,
                            scope.1,
                            expression,
                            value.clone(),
                        );
                    }
                }
                Some(Some(
                    NormalReturnPath { value, judge }.at_expression(expression),
                ))
            })
            .collect::<Option<Vec<_>>>()
            .map(|paths| paths.into_iter().flatten().collect())
    }

    fn capture_unknown_value(
        &mut self,
        expression: ExpressionHandle,
        scope: ValueScope<'_>,
    ) -> Option<StructuralTerm> {
        self.capture_sequence += 1;
        let value = StructuralTerm::CapturedValue {
            expression,
            callable: SymbolHandle::invalid(),
            occurrence: self.capture_sequence,
            result_type: crate::expression_result_type_reference(
                self.program,
                scope.0,
                scope.1,
                expression,
            )
            .or_else(|| {
                // Collection literals have contextual storage types, not one
                // context-free result reference. Constructor operands already
                // carry the selected field type; retain that receiving carrier
                // without claiming anything about its contents.
                matches!(
                    self.program.expression_table.expression(expression),
                    ExpressionNode::String(_) | ExpressionNode::ArrayLiteral(_)
                )
                .then(|| {
                    crate::value_custody::places::collection_element_type_reference(
                        self.program,
                        scope.2,
                    )
                })
                .flatten()
                .filter(|element| {
                    matches!(
                        self.program.expression_table.expression(expression),
                        ExpressionNode::ArrayLiteral(_)
                    ) || self.program.primitive_type_reference(*element)
                        == Some(typed_trees::types::PrimitiveType::U8)
                })
                .map(|_| scope.2)
            })?,
        };
        self.intake_current_storage_fields(scope.0, scope.1, expression, &value);
        Some(value)
    }

    fn assume_normal_equation(&mut self, left: StructuralTerm, right: StructuralTerm) {
        std::sync::Arc::make_mut(&mut self.normal_guarantees).push((left.clone(), right.clone()));
        self.intake_equation(left, right, 0);
    }

    pub(in crate::proof_contracts::contract_entailment) fn reference_subject_term(
        &self,
        subject: &crate::machine_calls::calls::InvocationReferenceSubject,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
    ) -> Option<StructuralTerm> {
        let program = self.program;
        let source = &subject.source;
        if let Some(value) = self
            .captured_reference_subject(subject)
            .and_then(|coordinate| self.current_storage_value(&coordinate))
        {
            return Some(
                if program
                    .primitive_type_reference(subject.type_reference)
                    .is_some()
                {
                    StructuralTerm::ObservedValue {
                        observed_type: subject.type_reference,
                        descriptor: None,
                        value: Box::new(value),
                    }
                } else {
                    value
                },
            );
        }
        if self.storage_binding_invalidated(source.root) {
            return None;
        }
        // The frame publishes a checked exact declaration path. Scope lookup
        // only locates that root's current observation; no initializer replay
        // or similarly spelled declaration may stand in for it.
        let is_parameter = program
            .state_parameter_by_symbol(scope.1, source.root)
            .is_some();
        let is_local = program
            .state_local_declaration(scope.1, source.root)
            .is_some();
        if !(is_parameter || is_local) || !subject.type_reference.is_valid() {
            return None;
        }
        let name = program.symbols.name(source.root);
        let mut roots = environment.iter().filter(|(binding, _)| binding == name);
        let mut value = roots.next()?.1.clone();
        if roots.next().is_some() {
            return None;
        }
        for segment in &source.segments {
            match segment {
                facts::PlaceSegment::Field { symbol } => {
                    value = self.observed_field_term(value, program.symbols.name(*symbol), 0)?;
                }
                // The current structural term owner has no indexed or case
                // projection observation. Exact origin/type alone cannot
                // manufacture its value or erase an active-case obligation.
                _ => return None,
            }
        }
        if program
            .primitive_type_reference(subject.type_reference)
            .is_some()
        {
            Some(StructuralTerm::ObservedValue {
                observed_type: subject.type_reference,
                descriptor: None,
                value: Box::new(value),
            })
        } else {
            Some(value)
        }
    }

    /// Observe this operation's own normal result immediately after its
    /// captured operands. The entry traversal stops before later siblings;
    /// completing the call here does not evaluate its operands a second time.
    pub(in crate::proof_contracts::contract_entailment) fn normal_operation_results(
        &self,
        statement_index: usize,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        let entries =
            self.normal_operation_entries(statement_index, expression, environment, scope)?;
        let mut results = Vec::with_capacity(entries.len());
        for entry in entries {
            let mut judge = entry.judge;
            let writes = judge
                .frame_resolver()?
                .expression_may_write_paths(scope.0, expression)?;
            judge.complete_normal_call(
                expression,
                Some(&entry.arguments),
                environment,
                scope,
                &writes,
            )?;
            let value = judge
                .normal_results
                .iter()
                .find(|(source, _)| *source == expression)?
                .1
                .clone();
            results.push(NormalReturnPath { value, judge });
        }
        (!results.is_empty()).then_some(results)
    }

    fn complete_normal_call(
        &mut self,
        expression: ExpressionHandle,
        actuals: Option<&[StructuralTerm]>,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        writes: &[String],
    ) -> Option<()> {
        let post_storage = self.capture_normal_call(expression, actuals, environment, scope);
        self.observe_storage_writes(scope.1, writes)?;
        for (coordinate, value) in post_storage.into_iter().flatten() {
            self.retain_storage_value(coordinate, value, false);
        }
        Some(())
    }

    fn capture_normal_call(
        &mut self,
        expression: ExpressionHandle,
        actuals: Option<&[StructuralTerm]>,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
    ) -> Option<
        Vec<(
            super::storage_effects::CapturedStorageCoordinate,
            StructuralTerm,
        )>,
    > {
        let program = self.program;
        let node = program.expression_table.expression(expression);
        let ExpressionNode::Call(call) = node else {
            return None;
        };
        let Some(owner) = selected_call_contract(program, call) else {
            return None;
        };
        // Computed proof machines are erased mathematical applications. Their
        // definitions and induction remain with the existing term owner;
        // replacing them with opaque runtime occurrences loses that denotation.
        if call_has_mathematical_denotation(program, call) {
            return None;
        }
        if !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || call.quotient_operation.is_some()
        {
            return None;
        }
        let Some(parameter_scope) = owner_parameter_scope(program, &owner) else {
            return None;
        };
        let parameters = parameter_scope.as_ref();
        if super::super::struct_trace() {
            eprintln!(
                "STRUCT normal-capture entry expression={expression:?} owner={:?} arguments={actuals:?}",
                owner_symbol(&owner)
            );
        }
        // References are live subjects. Only the frame owner can publish their
        // exact prefix origins and preservation through this normal return.
        // A written subject or replaced formal binding needs post-state facts,
        // not the detached child term captured during operand evaluation.
        let Some(machine) = program.machine(scope.0.symbol) else {
            return None;
        };
        let frames = self.frame_resolver();
        let subjects = frames.and_then(|frames| {
            frames.expression_call_reference_subjects(machine, expression, parameters)
        });
        if !independent_application(
            program,
            self.machine_symbol,
            owner_symbol(&owner),
            &call.machine_arguments,
        ) {
            return None;
        }
        let leading_operands = owner_leading_operands(&owner);
        let Some(arguments) = program
            .expression_table
            .expression_handles(call.arguments)
            .get(leading_operands..)
        else {
            return None;
        };
        if arguments.len() != parameters.len() {
            return None;
        }
        // The checked local-name gate rejects parameter/local collisions and
        // duplicate locals in each state; state forwarding creates a fresh
        // environment. Names here locate values only in that validated scope;
        // exact symbols still own the public formal/result substitution.
        // All actual values belong to the caller environment. Installing a
        // formal map before this capture would reinterpret another actual in
        // the callee's telescope.
        let Some(actuals) = actuals
            .and_then(|actuals| actuals.get(leading_operands..))
            .filter(|actuals| actuals.len() == arguments.len())
        else {
            return None;
        };
        // Entry requirements observe the arguments before this call's writes.
        // Guarantees may use only formals whose observations survive return.
        // Keep both substitutions: dropping a mutable input's post-state
        // observation must not erase a proven entry requirement or an
        // independent guarantee about the captured result.
        let entry_bindings = parameters
            .iter()
            .zip(actuals)
            .filter_map(|(parameter, value)| {
                let value = if crate::reference_shell(program, parameter.type_reference).is_some() {
                    super::super::structural_terms::reference_argument_observation(
                        program,
                        parameter.type_reference,
                        value,
                        subjects
                            .as_ref()
                            .and_then(|subjects| {
                                subjects
                                    .iter()
                                    .find(|subject| subject.parameter == parameter.symbol)
                            })
                            .and_then(|subject| {
                                self.reference_subject_term(subject, environment, scope)
                            }),
                    )?
                } else {
                    if !frames?.proof_value_is_caller_isolated(parameter.type_reference) {
                        return None;
                    }
                    value.clone()
                };
                Some((parameter.symbol, value))
            })
            .collect::<Vec<_>>();
        let mut bindings = entry_bindings
            .iter()
            .filter(|(symbol, _)| {
                let Some(parameter) = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == *symbol)
                else {
                    return false;
                };
                if crate::reference_shell(program, parameter.type_reference).is_none() {
                    !parameter.is_mutable
                } else {
                    subjects.as_ref().is_some_and(|subjects| {
                        subjects.iter().any(|subject| {
                            subject.parameter == *symbol && subject.preserved_on_return
                        })
                    })
                }
            })
            .cloned()
            .collect::<Vec<_>>();
        // A stable reference binding can designate changed contents. Capture
        // those contents as a fresh post-call value, never as the entry value.
        // The frame owns the exact referent coordinate; selected public
        // guarantees own its equations. Install it only after retiring writes.
        let mut post_storage = Vec::new();
        for subject in subjects.as_ref().into_iter().flatten() {
            if subject.preserved_on_return || !subject.stable_on_return {
                continue;
            }
            let Some(coordinate) = self.captured_reference_subject(subject) else {
                continue;
            };
            self.capture_sequence += 1;
            let value = StructuralTerm::CapturedValue {
                expression,
                callable: owner_symbol(&owner),
                occurrence: self.capture_sequence,
                result_type: subject.type_reference,
            };
            let observed = if program
                .primitive_type_reference(subject.type_reference)
                .is_some()
            {
                StructuralTerm::ObservedValue {
                    observed_type: subject.type_reference,
                    descriptor: None,
                    value: Box::new(value.clone()),
                }
            } else {
                value.clone()
            };
            bindings.push((subject.parameter, observed));
            post_storage.push((coordinate, value));
        }
        let callable = match owner {
            SelectedCallContract::Concrete(machine, _) => machine.symbol,
            _ => owner_symbol(&owner),
        };
        let result_type = owner_return_type(&owner);
        let Some(observed_type) =
            crate::expression_result_type_reference(program, scope.0, scope.1, expression)
        else {
            return None;
        };
        // A selected policy adapter may change the returned denotation. A raw
        // declaration's equation cannot describe that adapted result.
        if !crate::declared_carriers_match(program, observed_type, result_type)
            || program.arithmetic_domain_for_type_reference(observed_type)
                != program.arithmetic_domain_for_type_reference(result_type)
        {
            return None;
        }
        let contracts = owner_contracts(program, &owner).collect::<Vec<_>>();
        self.capture_sequence += 1;
        let captured = StructuralTerm::CapturedValue {
            expression,
            occurrence: self.capture_sequence,
            callable,
            result_type,
        };
        // An overwritten referent supplies no stable input observation, but
        // does not erase independent guarantees about this returned value.
        // Equations requiring an omitted formal fail exact substitution.
        let Some(equations) =
            self.selected_guarantee_equations(&owner, &entry_bindings, &bindings, &captured, 0)
        else {
            return None;
        };
        // A saved local now resolves to this occurrence, so the callee's case
        // tags must classify it here; the citation route's saved-binding tags
        // never meet that environment term. Tags never become value equations.
        let tag_callee = match owner {
            SelectedCallContract::Concrete(machine, _) if bindings.len() == parameters.len() => {
                Some(machine)
            }
            _ => None,
        };
        let tag_map = parameters
            .iter()
            .filter_map(|parameter| {
                bindings
                    .iter()
                    .find(|(symbol, _)| *symbol == parameter.symbol)
                    .map(|(_, value)| (parameter.name.as_str().to_owned(), value.clone()))
            })
            .chain(std::iter::once((
                crate::proof_contracts::contract_entailment::RESULT_BINDER.to_owned(),
                captured.clone(),
            )))
            .collect::<Vec<_>>();
        let mut case_guarantees = Vec::new();
        for contract in contracts
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let ProofFact::Expression(guarantee) = fact else {
                    continue;
                };
                if let Some(callee) = tag_callee {
                    self.instantiated_case_guarantees(
                        callee,
                        *guarantee,
                        &tag_map,
                        &mut case_guarantees,
                    );
                }
            }
        }
        if super::super::struct_trace() {
            eprintln!(
                "STRUCT normal-call expression={expression:?} public_owner={:?} result_type={result_type:?} equations={equations:?}",
                owner_symbol(&owner),
            );
        }
        self.intake_declared_value_domains(result_type, captured.clone());
        std::sync::Arc::make_mut(&mut self.normal_results).push((expression, captured));
        for (left, right) in equations {
            std::sync::Arc::make_mut(&mut self.captured_result_equations)
                .push((left.clone(), right.clone()));
            std::sync::Arc::make_mut(&mut self.normal_guarantees)
                .push((left.clone(), right.clone()));
            self.intake_equation(left, right, 0);
        }
        for guarantee in &case_guarantees {
            self.intake_case_guarantee(guarantee);
        }
        Some(post_storage)
    }
}

/// Discarding a value never discards an operation's own effects. Calls,
/// atomics, casts and non-builtin operators keep their complete owner; ordinary
/// construction and projection have only the effects of their ordered children.
fn discarded_value_has_no_own_effects(
    program: &dyn crate::CheckingInput,
    scope: ValueScope<'_>,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Unary(_) => false,
        ExpressionNode::Binary(_) => crate::has_builtin_binary_expression_meaning(
            program,
            scope.0,
            Some(scope.1),
            expression,
        ),
        ExpressionNode::Indexed(indexed) => {
            crate::has_builtin_subslice_meaning(program, scope.0, Some(scope.1), expression)
                || crate::expression_result_type_reference(
                    program,
                    scope.0,
                    scope.1,
                    indexed.collection,
                )
                .is_some_and(|collection| {
                    crate::value_custody::places::value_element_has_builtin_coordinate(
                        program,
                        scope.0,
                        Some(scope.1),
                        expression,
                        collection,
                    )
                })
        }
        ExpressionNode::Member(member) => {
            let Some(reference) =
                crate::expression_result_type_reference(program, scope.0, scope.1, member.receiver)
            else {
                return false;
            };
            if crate::value_custody::places::collection_operations::collection_measure_member(
                program,
                member,
                crate::value_custody::places::collection_operations::MeasureReceiver::Declared(
                    reference,
                ),
            )
            .is_some()
            {
                return true;
            }
            crate::value_custody::places::data_definition_for_type(program, reference)
                .and_then(|data| {
                    crate::value_custody::places::exact_data_member_field(
                        program,
                        data,
                        member.member_symbol,
                        member.member.as_str(),
                        member.case_variant.as_ref().map(|case| case.as_str()),
                    )
                })
                .is_some()
        }
        ExpressionNode::Match(_)
        | ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::String(_) => true,
        ExpressionNode::UndeterminedNaN(_) | ExpressionNode::ZeroValue(_) => false,
    }
}

impl NormalReturnPath<'_> {
    fn at_expression(mut self, expression: ExpressionHandle) -> Self {
        std::sync::Arc::make_mut(&mut self.judge.normal_results)
            .retain(|(source, _)| *source != expression);
        std::sync::Arc::make_mut(&mut self.judge.normal_results)
            .push((expression, self.value.clone()));
        self
    }
}

/// The shared proof-only classifier, rather than a carrier spelling or
/// source position, establishes which calls have no runtime observation or
/// caller-storage event. Runtime calls still require complete write frames.
pub(super) fn call_has_mathematical_denotation(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    if let Some(signature) = crate::machine_calls::denotational_calls::abstract_value_call_signature(
        program,
        call.target_symbol,
    ) {
        return crate::proof_only_classification(program)
            .proof_only_mention(program, signature.return_type)
            .is_some();
    }
    match selected_call_contract(program, call) {
        Some(SelectedCallContract::Concrete(machine, _)) => {
            crate::proof_only_classification(program).is_proof_machine(program, machine)
        }
        Some(SelectedCallContract::Operator(operator)) => {
            // A sealed total proof-value operator has the same erased
            // denotation as a computed proof machine. Retain its selected
            // identity; an unresolved or ordinary runtime operator must still
            // establish its write frame and capture its own result occurrence.
            crate::proof_only_classification(program)
                .proof_only_mention(program, operator.return_type)
                .is_some()
                && crate::machine_calls::denotational_calls::selected_operator_has_closed_value_denotation(program, operator)
        }
        _ => false,
    }
}

fn program_expression_is_live(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    expression.is_valid() && program.expression_table.expression_is_valid(expression)
}
