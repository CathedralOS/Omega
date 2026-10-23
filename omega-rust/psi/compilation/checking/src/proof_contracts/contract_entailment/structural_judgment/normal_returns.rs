//! Selected guarantees for captured normal-return values.
//!
//! A runtime result is one occurrence, not a replayable mathematical call.
//! Capture operands before substituting the selected public signature, prove
//! its entry requirements, then import its unconditional normal guarantees.
//! This environment belongs to one executed normal-return path; it is neither
//! a loop invariant nor permission to replay mutable storage observations.

mod operand_types;

use super::super::specification_calls::{
    SelectedCallContract, owner_contracts, owner_leading_operands, owner_parameter_scope,
    owner_return_type, owner_symbol, selected_call_contract,
};
use super::selected_contracts::bound_term;
use super::value_predicates::boolean_term;
use super::{MembershipGuarantee, StructuralJudge, StructuralJudgment, StructuralTerm};
use crate::expression_traversal::children;
use crate::machine_calls::contract_dependencies::independent_application;
use facts::NormalizedWriteFrame;
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

/// An observation stops its execution path at the requested semantic cut.
/// Entry captures operands before the operation; completion captures both the
/// result and its proof context before any later sibling can contribute facts.
enum NormalObservation<'program> {
    None,
    Entry {
        expression: ExpressionHandle,
        paths: Vec<NormalOperationEntry<'program>>,
    },
    Completion {
        expression: ExpressionHandle,
        paths: Vec<NormalReturnPath<'program>>,
    },
}

impl NormalObservation<'_> {
    fn requests(&self, requested: ExpressionHandle) -> bool {
        match self {
            Self::None => false,
            Self::Entry { expression, .. } | Self::Completion { expression, .. } => {
                *expression == requested
            }
        }
    }
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
            &mut NormalObservation::None,
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
        // Earlier siblings still execute in order, but their unused aggregate
        // values need not be retained to observe this operation's operands.
        // The requested operation and every effectful child keep value demand.
        let observations = [call];
        let operands = self.statement_observation_operands(
            statement_index,
            call,
            scope,
            NormalValueDemand::Observe(&observations),
        )?;
        let mut observation = NormalObservation::Entry {
            expression: call,
            paths: Vec::new(),
        };
        self.clone().evaluate_normal_operands(
            &operands,
            environment,
            scope,
            0,
            &mut observation,
        )?;
        let NormalObservation::Entry { paths, .. } = observation else {
            return None;
        };
        (!paths.is_empty()).then_some(paths)
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
        let requested = [expression];
        let operands = self.statement_observation_operands(
            statement_index,
            expression,
            scope,
            NormalValueDemand::Observe(&requested),
        )?;
        let mut observation = NormalObservation::Completion {
            expression,
            paths: Vec::new(),
        };
        self.clone().evaluate_normal_operands(
            &operands,
            environment,
            scope,
            0,
            &mut observation,
        )?;
        let NormalObservation::Completion { paths, .. } = observation else {
            return None;
        };
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
                    &mut NormalObservation::None,
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
            &mut NormalObservation::None,
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
                &mut NormalObservation::None,
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
                    &mut NormalObservation::None,
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
        observation: &mut NormalObservation<'program>,
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
                    observation,
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
        self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        depth: usize,
        observation: &mut NormalObservation<'program>,
        demand: NormalValueDemand<'_>,
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        let paths = self.evaluate_normal_value_paths(
            expression,
            environment,
            scope,
            depth,
            observation,
            demand,
        )?;
        if let NormalObservation::Completion {
            expression: requested,
            paths: completed,
        } = observation
            && *requested == expression
        {
            completed.extend(paths);
            // The completion is the end of this observation, not merely a
            // saved value inside a later sibling's proof environment.
            return Some(Vec::new());
        }
        Some(paths)
    }

    fn evaluate_normal_value_paths(
        mut self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        depth: usize,
        observation: &mut NormalObservation<'program>,
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
        // An implementation selector constructs code identity, not a value
        // obtained by evaluating the implementation name. The declaration
        // owner separately validates its exact conformance/application. Keep
        // only a fresh opaque value here: no implementation-body facts escape.
        if self.runtime_body_values
            && let ExpressionNode::Cast(cast) = node
            && !cast.form.is_recast()
            && cast.domain == numerics::arithmetic::ArithmeticDomain::Exact
            && cast.semantic_domain.is_empty()
            && cast.semantic_domain_arguments.is_empty()
            && matches!(
                program
                    .type_reference_table
                    .type_reference(cast.target_type),
                typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
            )
            && let ExpressionNode::Name(name) = program.expression_table.expression(cast.value)
            && name.symbol.is_valid()
            && program.symbols.get(name.symbol).kind == symbols::SymbolKind::Conformance
        {
            let value = self.capture_unknown_value(expression, scope)?;
            return Some(vec![
                NormalReturnPath { judge: self, value }.at_expression(expression),
            ]);
        }
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
                observation,
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
                            observation,
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
                observation,
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
                            observation,
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
                                observation,
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
                            observation,
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
        let has_own_effect = match node {
            ExpressionNode::Call(call) => !call_has_mathematical_denotation(program, call),
            ExpressionNode::Atomic(_) => {
                crate::value_custody::atomic_operations::atomic_once_carrier(program, expression)
                    .is_some()
            }
            _ => false,
        };
        let writes = if has_own_effect {
            let machine = program.machine(scope.0.symbol)?;
            let writes = self
                .frame_resolver()?
                .operation_write_frame(machine, expression);
            if super::super::struct_trace() {
                eprintln!(
                    "STRUCT normal-write-frame caller={} expression={expression:?} writes={writes:?}",
                    scope.0.name
                );
            }
            if !writes.is_complete() && !self.requires_value_preservation() {
                return None;
            }
            // Mathematical body substitution has no runtime storage epochs.
            // It cannot unfold effectful calls as repeatable applications.
            // Case-only paths separately establish tag preservation.
            if !writes.paths().is_empty() && !self.runtime_body_values && !self.case_facts_only {
                return None;
            }
            writes
        } else {
            NormalizedWriteFrame::complete(Vec::new())
        };
        // Retire a prior evaluation of this static node. Repeated edges are
        // distinct invocations; saved earlier values already retain their own
        // capture sequence and equations in the caller environment.
        std::sync::Arc::make_mut(&mut self.normal_results)
            .retain(|(source, _)| *source != expression);
        let scalar_field_read = self.requires_value_preservation()
            && super::super::observed_projections::declared_scalar_field_read(
                program, scope.0, scope.1, expression,
            )
            .is_some();
        let mut children_to_evaluate = Vec::new();
        let child_references =
            operand_types::receiving_types(program, scope.0, expression, node, scope.2);
        let namespace_receiver = program
            .expression_table
            .call_node(expression)
            .filter(|call| super::super::structural_terms::static_call_receiver(program, call))
            .map(|call| call.receiver);
        if let ExpressionNode::Indexed(indexed) = node
            && let Some(range) = program.expression_table.range_node(indexed.index)
        {
            // A window operation consumes its collection and each authored
            // bound, not a first-class Range value. Keep the same operand
            // roster as runtime lowering and capture each bound in order.
            // Completion needs this roster too: a standalone selector has no
            // value carrier and would discard the later state's entire prefix.
            children_to_evaluate.extend(
                [indexed.collection, range.start, range.end]
                    .into_iter()
                    .filter(|child| child.is_valid())
                    .map(|child| (child, scope.2)),
            );
        } else {
            let mut position = 0;
            children(program, node, |child| {
                // Keep the shared roster's positions, including an absent
                // receiver and repeated edges, before pruning non-values.
                let reference = child_references
                    .as_ref()
                    .and_then(|references| references.get(position))
                    .copied()
                    .filter(|reference| reference.is_valid())
                    .unwrap_or(scope.2);
                position += 1;
                if child.is_valid() && Some(child) != namespace_receiver {
                    children_to_evaluate.push((child, reference));
                }
            });
        }
        let operands = children_to_evaluate
            .into_iter()
            .map(|(child, child_reference)| {
                let child_demand = match node {
                    ExpressionNode::Member(member)
                        if (scalar_field_read || matches!(demand, NormalValueDemand::Location))
                            && child == member.receiver =>
                    {
                        NormalValueDemand::Location
                    }
                    ExpressionNode::Indexed(indexed)
                        if matches!(demand, NormalValueDemand::Location)
                            || program.expression_table.range_node(indexed.index).is_some() =>
                    {
                        // A view's pointer/count descriptor is evaluated before
                        // its selector. Fixed-array geometry needs only a place,
                        // not a read of possibly uninitialized element storage.
                        if child == indexed.collection
                            && crate::expression_result_type_reference(
                                program, scope.0, scope.1, child,
                            )
                            .and_then(|reference| crate::slice_view_shell(program, reference, &[]))
                            .is_none()
                        {
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
        let contexts =
            self.evaluate_normal_operands(&operands, environment, scope, depth, observation)?;
        contexts
            .into_iter()
            .map(|(mut judge, inputs)| {
                if !observation.requests(expression)
                    && matches!(
                        demand,
                        NormalValueDemand::Observe(_) | NormalValueDemand::Location
                    )
                {
                    // Only the completed descendants are requested. No value
                    // identity or equation is published for this unused result.
                    // A requested location still publishes its incoming operands
                    // below, without reading or performing the destination store.
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
                if let NormalObservation::Entry { expression: requested, paths } = observation
                    && *requested == expression
                {
                    paths.push(NormalOperationEntry {
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
                    scalar_field_read && judge.storage_write_sequence != receiver_write_sequence;
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
                // Metadata observes the descriptor child that just completed,
                // not a replay of its source place. A backing write may retire
                // content observations without replacing this descriptor.
                let completed_measure = if let ExpressionNode::Member(member) = node {
                    crate::expression_result_type_reference(
                        program,
                        scope.0,
                        scope.1,
                        member.receiver,
                    )
                    .and_then(|reference| {
                        super::super::structural_terms::collection_measure_term(
                            program,
                            member,
                            reference,
                            || {
                                inputs
                                    .iter()
                                    .find(|(source, _)| *source == member.receiver)
                                    .map(|(_, value)| value.clone())
                            },
                        )
                    })
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
                } else if let Some(value) = completed_measure {
                    value
                } else if matches!(node, ExpressionNode::Atomic(_)) {
                    // The atomic owner distinguishes an observed load from
                    // update carriers. Its value is a fresh event, not the
                    // place's prior initializer or the modeled new resident.
                    if crate::value_custody::atomic_operations::atomic_load_carrier(program, expression).is_none()
                        && crate::value_custody::atomic_operations::atomic_once_carrier(program, expression).is_none()
                    { return None; }
                    judge.capture_unknown_value(expression, scope)?
                } else if let Some(value) =
                    judge.callee_term_in_scope(Some(scope), expression, environment, 0)
                {
                    value
                } else if judge.runtime_body_values
                    && (matches!(node, ExpressionNode::Call(_))
                        || matches!(node, ExpressionNode::Indexed(indexed)
                            if program.expression_table.range_node(indexed.index).is_some()
                                && crate::value_custody::places::place_has_builtin_coordinates(program, scope.0, Some(scope.1), expression))
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
                    let constructed = super::super::structural_terms::constructor_literal_term(
                        program,
                        literal,
                        |declared, supplied| {
                            if let Some(field) = supplied {
                                let position = fields
                                    .iter()
                                    .position(|source| source.field_symbol == field.field_symbol)?;
                                return inputs.get(position).map(|(_, value)| value.clone());
                            }
                            if !judge.runtime_body_values
                                || !crate::has_contents(
                                    program,
                                    declared.type_reference,
                                    &[],
                                    crate::value_custody::storage_contents::ContentsRequirement::BorrowFree,
                                )
                            {
                                return None;
                            }
                            // Default-domain admission is not a loan origin.
                            // Only finite borrow-free runtime zeros receive an
                            // opaque identity; erased terms and reference-bearing
                            // defaults still need their independent producers.
                            judge.capture_sequence += 1;
                            Some(StructuralTerm::CapturedValue {
                                expression,
                                callable: SymbolHandle::invalid(),
                                occurrence: judge.capture_sequence,
                                result_type: declared.type_reference,
                            })
                        },
                    );
                    value = match constructed {
                        Some(constructed) => constructed,
                        None if judge.runtime_body_values => {
                            // A completed construction can contain an omitted
                            // established value with no mathematical denotation.
                            // Preserve its executed effects and a fresh result;
                            // an unrelated later predicate still sees its prefix.
                            judge.capture_unknown_value(expression, scope)?
                        }
                        None => return None,
                    };
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
                if let ExpressionNode::Indexed(indexed) = node
                    && program.expression_table.range_node(indexed.index).is_none()
                {
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
                    if let Some(reference) = crate::expression_result_type_reference(
                        program,
                        scope.0,
                        scope.1,
                        expression,
                    ) {
                        judge.intake_admitted_value_predicates(reference, value.clone());
                    }
                }
                if let ExpressionNode::Indexed(indexed) = node
                    && let Some(range) = program.expression_table.range_node(indexed.index)
                {
                    // A completed runtime view keeps its captured descriptor;
                    // there is no element observation to form. Its extent is
                    // exactly the authored window: the formation's own bounds
                    // check already admitted start <= end <= len(collection).
                    let input_value = |source: ExpressionHandle| {
                        inputs
                            .iter()
                            .find(|(input, _)| *input == source)
                            .map(|(_, value)| value.clone())
                    };
                    let Some(collection) = input_value(indexed.collection) else {
                        return None;
                    };
                    let start = range
                        .start
                        .is_valid()
                        .then(|| input_value(range.start))
                        .flatten();
                    let end = range
                        .end
                        .is_valid()
                        .then(|| input_value(range.end))
                        .flatten();
                    let collection_measure =
                        |collection: &StructuralTerm, collection_source| {
                            let receiver = crate::expression_result_type_reference(
                                program,
                                scope.0,
                                scope.1,
                                collection_source,
                            )?;
                            super::super::structural_terms::observed_collection_measure(
                                program,
                                receiver,
                                language_semantics::declaration_selection::CollectionMeasure::Length,
                                || Some(collection.clone()),
                            )
                        };
                    let difference = |left: StructuralTerm, right: StructuralTerm| {
                        StructuralTerm::MathematicalBinary {
                            operator: BinaryOperator::Subtract,
                            left: Box::new(left),
                            right: Box::new(right),
                        }
                    };
                    let extent = match (start, end) {
                        (Some(start), Some(end)) => Some(match start {
                            StructuralTerm::Integer(ref literal) if literal.is_zero() => end,
                            _ => difference(end, start),
                        }),
                        (None, Some(end)) => Some(end),
                        (start, None) => collection_measure(&collection, indexed.collection).map(
                            |full| match start {
                                Some(start) => difference(full, start),
                                None => full,
                            },
                        ),
                    };
                    if let Some(extent) = extent
                        && let Some(view) = judge.normal_value_type_reference(expression, scope)
                        && let Some(measure) =
                            super::super::structural_terms::observed_collection_measure(
                                program,
                                view,
                                language_semantics::declaration_selection::CollectionMeasure::Length,
                                || Some(value.clone()),
                            )
                    {
                        judge.assume_normal_equation(measure, extent);
                    }
                }
                if let ExpressionNode::Borrow(borrow) = node {
                    // A checked borrow selects its referent; it does not create
                    // a new case-bearing value. The referent's completed term
                    // keeps the same descriptor identity the mathematical
                    // `callee_term` walk would have produced, so later `len`
                    // and field observations name this exact view.
                    if let Some((_, referent)) =
                        inputs.iter().find(|(input, _)| *input == borrow.target)
                    {
                        value = referent.clone();
                    }
                }
                if scalar_field_read {
                    // Completing a typed read observes its actual value even
                    // when no mathematical entry value is known. Integers,
                    // addresses, and IEEE floats share that coordinate, so an
                    // exclusive loan retires the read only when it overlaps.
                    // Source text is never its identity.
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
                // A whole owned-value read captures its revision before a
                // named state can inspect its fields. A reference descriptor
                // does not capture the contents of its referent.
                if matches!(node, ExpressionNode::Name(_) | ExpressionNode::Member(_))
                    && let Some(reference) = crate::expression_result_type_reference(program, scope.0, scope.1, expression)
                    && program.primitive_type_reference(reference).is_none()
                    && crate::reference_shell(program, reference).is_none()
                {
                    judge.observe_completed_storage_read(scope.0, scope.1, expression, value.clone());
                }
                Some(Some(
                    NormalReturnPath { value, judge }.at_expression(expression),
                ))
            })
            .collect::<Option<Vec<_>>>()
            .map(|paths| paths.into_iter().flatten().collect())
    }

    fn normal_value_type_reference(
        &self,
        expression: ExpressionHandle,
        scope: ValueScope<'_>,
    ) -> Option<TypeReferenceHandle> {
        let program = self.program;
        crate::expression_result_type_reference(program, scope.0, scope.1, expression).or_else(
            || {
                let indexed = program.expression_table.indexed_node(expression)?;
                program.expression_table.range_node(indexed.index)?;
                crate::slice_view_shell(program, scope.2, &[])?;
                if !crate::value_custody::places::place_has_builtin_coordinates(
                    program,
                    scope.0,
                    Some(scope.1),
                    expression,
                ) {
                    return None;
                }
                let source = crate::expression_result_type_reference(
                    program,
                    scope.0,
                    scope.1,
                    indexed.collection,
                )?;
                let source_element =
                    crate::value_custody::places::collection_element_type_reference(
                        program, source,
                    )?;
                let receiving_element =
                    crate::value_custody::places::collection_element_type_reference(
                        program, scope.2,
                    )?;
                (program.normalized_type_identity(source_element)
                    == program.normalized_type_identity(receiving_element))
                .then_some(scope.2)
            },
        )
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
            result_type: self
                .normal_value_type_reference(expression, scope)
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
        if let StructuralTerm::CapturedValue { result_type, .. } = &value {
            self.intake_admitted_scalar_bounds(*result_type, &value);
        }
        if let ExpressionNode::String(bytes) = self.program.expression_table.expression(expression)
            && let StructuralTerm::CapturedValue { result_type, .. } = &value
        {
            // The completed quoted-byte view has exact descriptor geometry.
            // Its contents remain opaque: a byte count supplies neither an
            // encoding predicate nor authority to observe another referent.
            let extent = StructuralTerm::Integer(numerics::bignum::BigInt::from_u64(
                u64::try_from(bytes.len()).ok()?,
            ));
            for measure in [
                language_semantics::declaration_selection::CollectionMeasure::Length,
                language_semantics::declaration_selection::CollectionMeasure::Capacity,
            ] {
                if let Some(observation) =
                    super::super::structural_terms::observed_collection_measure(
                        self.program,
                        *result_type,
                        measure,
                        || Some(value.clone()),
                    )
                {
                    self.assume_normal_equation(observation, extent.clone());
                }
            }
        }
        Some(value)
    }

    fn assume_normal_equation(&mut self, left: StructuralTerm, right: StructuralTerm) {
        std::sync::Arc::make_mut(&mut self.normal_guarantees).push((left.clone(), right.clone()));
        self.intake_equation(left, right, 0);
    }

    pub(in crate::proof_contracts::contract_entailment) fn reference_subject_term(
        &self,
        subject: &crate::machine_calls::calls::InvocationReferenceSubject,
        actual: &StructuralTerm,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
    ) -> Option<StructuralTerm> {
        let program = self.program;
        let [source] = subject.sources.as_slice() else {
            // Origin authority quantifies over the complete join. Its actual
            // value was selected by this path's ordered operand evaluation.
            // Later sibling writes retire that observation; a produced
            // descriptor never supplies referent contents by type alone.
            if subject.sources.is_empty()
                || self.storage_observation_invalidated(subject.argument)
                || subject
                    .sources
                    .iter()
                    .any(|source| self.storage_binding_invalidated(source.root))
            {
                return None;
            }
            // Resolution may replace the outer carrier. Check authority on
            // both sides so an equation cannot launder a raw descriptor.
            if !super::super::structural_terms::scalar_operand_is_observed(
                program,
                subject.parameter_type,
                actual,
            ) {
                return None;
            }
            let value = self.resolve(actual.clone());
            if !super::super::structural_terms::scalar_operand_is_observed(
                program,
                subject.parameter_type,
                &value,
            ) {
                return None;
            }
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
        };
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
                .operation_write_frame(scope.0, expression);
            if !writes.is_complete() && !judge.requires_value_preservation() {
                return None;
            }
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
        writes: &NormalizedWriteFrame,
    ) -> Option<()> {
        let post_storage =
            self.capture_normal_call(expression, actuals, environment, scope, writes);
        self.observe_storage_effects(scope.1, writes.complete_paths())?;
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
        writes: &NormalizedWriteFrame,
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
        if !call.evidence_arguments.is_empty() || call.quotient_operation.is_some() {
            return None;
        }
        let static_bindings = super::super::specification_calls::call_scalar_substitution(
            program,
            &owner,
            &call.machine_arguments,
        )?;
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
        // An opaque frame establishes no preservation or post-call reference
        // correspondence. Isolated operand snapshots and independent result
        // guarantees remain usable, but no live referent is reintroduced.
        let subjects = frames.filter(|_| writes.is_complete()).and_then(|frames| {
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
        let mut entry_bindings = parameters
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
                                self.reference_subject_term(subject, value, environment, scope)
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
        entry_bindings.extend(
            static_bindings
                .iter()
                .map(|(parameter, value)| (parameter.symbol, value.clone())),
        );
        let mut bindings = entry_bindings
            .iter()
            .filter(|(symbol, _)| {
                let Some(parameter) = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == *symbol)
                else {
                    return static_bindings
                        .iter()
                        .any(|(parameter, _)| parameter.symbol == *symbol);
                };
                if crate::reference_shell(program, parameter.type_reference).is_none() {
                    !parameter.is_mutable
                } else {
                    subjects.as_ref().is_some_and(|subjects| {
                        subjects.iter().any(|subject| {
                            subject.parameter == *symbol && subject.preserved_on_return
                        })
                    }) || (!parameter.is_mutable
                        && crate::slice_view_shell(program, parameter.type_reference, &[])
                            .is_some())
                    // A view whose referent origin cannot be authenticated
                    // still supplies its passed descriptor: `window.len` in a
                    // guarantee names this exact extent. Referent contents
                    // stay unobserved; mutable/exclusive formals could denote
                    // a reseated post-call descriptor and keep the drop.
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
        let Some(result_type) =
            crate::expression_result_type_reference(program, scope.0, scope.1, expression)
        else {
            return None;
        };
        // A selected policy adapter may change the returned denotation. A raw
        // declaration's equation cannot describe that adapted result.
        if program.arithmetic_domain_for_type_reference(result_type)
            != program.arithmetic_domain_for_type_reference(owner_return_type(&owner))
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
                if let ProofFact::Membership(guarantee) = fact {
                    // A result qualification stated in `ensures` carries the
                    // same admission as one stated on the result type. Retain
                    // its exact detached occurrence, never a live caller name
                    // or the result's borrowed referent. Later writes change
                    // the observed subject and cannot reuse this snapshot.
                    if let Some(subject) = bound_term(
                        program,
                        guarantee.value,
                        callable,
                        parameters,
                        callable,
                        &bindings,
                        Some(&captured),
                        0,
                    ) {
                        let mut root = &subject;
                        while let StructuralTerm::Projection { subject, .. } = root {
                            root = subject;
                        }
                        if root == &captured && guarantee.semantic_domain.is_valid() {
                            std::sync::Arc::make_mut(&mut self.normal_memberships).push(
                                MembershipGuarantee {
                                    subject,
                                    domain: guarantee.domain_symbol,
                                    semantic_domain: guarantee.semantic_domain,
                                },
                            );
                        }
                    }
                }
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
        self.intake_admitted_value_predicates(result_type, captured.clone());
        let recorded = if let SelectedCallContract::Concrete(callee, _) = &owner
            && let Some(constructor) = returned_record_term(program, callee, parameters, actuals)
        {
            constructor
        } else {
            captured
        };
        std::sync::Arc::make_mut(&mut self.normal_results).push((expression, recorded));
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
/// atomics, executable conversions and non-builtin operators keep their complete
/// owner. Representation recasts, construction and projection have only their
/// ordered children's effects; the recast checker owns compatibility and loans.
fn discarded_value_has_no_own_effects(
    program: &dyn crate::CheckingInput,
    scope: ValueScope<'_>,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(_) | ExpressionNode::Atomic(_) | ExpressionNode::Unary(_) => false,
        // A recast borrows existing backing; it does not execute a conversion.
        // Its completed reference may remain opaque without discarding the
        // prefix's independent premises or claiming new referent contents.
        ExpressionNode::Cast(cast) => cast.form.is_recast(),
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

/// A call whose body is one record literal records that constructor. Field
/// projections of the result use the substituted fields. A body that calls
/// again stays a captured occurrence.
fn returned_record_term(
    program: &dyn crate::CheckingInput,
    callee: &typed_trees::machine::Machine,
    parameters: &[typed_trees::signature::StateParameter],
    actuals: &[StructuralTerm],
) -> Option<StructuralTerm> {
    if parameters.len() != actuals.len() {
        return None;
    }
    let state = program.machine_states(callee).first()?;
    let statements: Vec<_> = program
        .statement_table
        .indexed_statements(state.statement_nodes)
        .collect();
    let [(_, statement)] = statements.as_slice() else {
        return None;
    };
    let typed_trees::statement::StatementNode::Expression(expression) = statement else {
        return None;
    };
    let mut handles = Vec::new();
    crate::expression_traversal::collect_expression_tree(program, *expression, &mut handles);
    if handles.into_iter().any(|handle| {
        matches!(
            program.expression_table.expression(handle),
            ExpressionNode::Call(_)
        )
    }) {
        return None;
    }
    let environment = parameters
        .iter()
        .zip(actuals)
        .map(|(parameter, term)| (parameter.name.as_str().to_owned(), term.clone()))
        .collect::<Vec<_>>();
    let judge = StructuralJudge::from_runtime_requires(program, callee, &[]);
    let paths = judge.normal_return_paths(
        *expression,
        &environment,
        (callee, state, state.return_type),
    )?;
    let [path] = paths.as_slice() else {
        return None;
    };
    matches!(path.value, StructuralTerm::Constructor { .. }).then(|| path.value.clone())
}
