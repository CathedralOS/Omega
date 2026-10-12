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
use super::selected_contracts::{
    capture_equations, independent_application, selected_requirement_judgment,
};
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

pub(in crate::proof_contracts::contract_entailment) struct NormalCallEntry<'program> {
    pub arguments: Vec<StructuralTerm>,
    pub judge: StructuralJudge<'program>,
    pub environment: Vec<(String, StructuralTerm)>,
}

struct PreparedNormalCall<'program> {
    owner: SelectedCallContract<'program>,
    parameters: std::borrow::Cow<'program, [typed_trees::signature::StateParameter]>,
    bindings: Vec<(SymbolHandle, StructuralTerm)>,
    callable: SymbolHandle,
    result_type: TypeReferenceHandle,
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
            StructuralTerm::ObservedValue {
                observed_type,
                value,
            } => StructuralTerm::ObservedValue {
                observed_type,
                value: Box::new(self.completed_value_identity_at(*value, depth + 1)),
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
            StructuralTerm::CapturedValue { .. } | StructuralTerm::Projection { .. }
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
        self.normal_return_paths_since(expression, environment, scope, self.effect_epoch())
    }

    /// The environment was observed at this exact effect cursor. Child calls
    /// advance the path; they cannot make its old storage bindings fresh again.
    pub(in crate::proof_contracts::contract_entailment) fn normal_return_paths_since(
        &self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        environment_epoch: usize,
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        self.clone().evaluate_normal_paths(
            expression,
            environment,
            scope,
            0,
            ExpressionHandle::invalid(),
            &mut Vec::new(),
            environment_epoch,
        )
    }

    /// Observe one exact call entry after its enclosing statement's preceding
    /// operands, including separate roots such as named-transition arguments.
    /// The snapshot precedes the selected call's guarantee intake.
    pub(in crate::proof_contracts::contract_entailment) fn normal_call_entries(
        &self,
        statement_index: usize,
        call: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
    ) -> Option<Vec<NormalCallEntry<'program>>> {
        let operands = self.statement_observation_operands(statement_index, call, scope)?;
        let mut entries = Vec::new();
        self.clone().evaluate_normal_operands(
            &operands,
            environment,
            scope,
            0,
            call,
            &mut entries,
            self.effect_epoch(),
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
        let operands = self.statement_observation_operands(statement_index, expression, scope)?;
        let contexts = self.clone().evaluate_normal_operands(
            &operands,
            environment,
            scope,
            0,
            ExpressionHandle::invalid(),
            &mut Vec::new(),
            self.effect_epoch(),
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

    fn statement_observation_operands(
        &self,
        statement_index: usize,
        expression: ExpressionHandle,
        scope: ValueScope<'_>,
    ) -> Option<Vec<(ExpressionHandle, TypeReferenceHandle)>> {
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
                    .unwrap_or_default();
                    (root, reference)
                })
                .collect(),
        )
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
    ) -> Option<Vec<NormalCallEntry<'program>>> {
        if arguments.len() != parameters.len() {
            return None;
        }
        let operands = arguments
            .iter()
            .zip(parameters)
            .map(|(expression, parameter)| (*expression, parameter.type_reference))
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
                    self.effect_epoch(),
                )?
                .into_iter()
                .map(|(judge, values)| {
                    let environment = environment
                        .iter()
                        .filter(|(name, value)| {
                            judge.binding_survives_effects(name, value, self.effect_epoch())
                        })
                        .cloned()
                        .collect();
                    NormalCallEntry {
                        arguments: values.into_iter().map(|(_, value)| value).collect(),
                        judge,
                        environment,
                    }
                })
                .collect(),
        )
    }

    fn evaluate_normal_operands(
        self,
        operands: &[(ExpressionHandle, TypeReferenceHandle)],
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        depth: usize,
        requested_call: ExpressionHandle,
        entries: &mut Vec<NormalCallEntry<'program>>,
        environment_epoch: usize,
    ) -> Option<Vec<(Self, Vec<(ExpressionHandle, StructuralTerm)>)>> {
        let mut contexts = vec![(self, Vec::new())];
        for &(child, reference) in operands {
            let mut next = Vec::new();
            for (context, inputs) in contexts {
                for path in context.evaluate_normal_paths(
                    child,
                    environment,
                    (scope.0, scope.1, reference),
                    depth + 1,
                    requested_call,
                    entries,
                    environment_epoch,
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
        requested_call: ExpressionHandle,
        entries: &mut Vec<NormalCallEntry<'program>>,
        environment_epoch: usize,
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        if depth >= 64 || !program_expression_is_live(self.program, expression) {
            return None;
        }
        let program = self.program;
        let environment = environment
            .iter()
            .filter(|(name, value)| self.binding_survives_effects(name, value, environment_epoch))
            .cloned()
            .collect::<Vec<_>>();
        let environment = environment.as_slice();
        let node = program.expression_table.expression(expression);
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
                requested_call,
                entries,
                environment_epoch,
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
                            requested_call,
                            entries,
                            environment_epoch,
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
                requested_call,
                entries,
                environment_epoch,
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
                            requested_call,
                            entries,
                            environment_epoch,
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
                                requested_call,
                                entries,
                                environment_epoch,
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
                            requested_call,
                            entries,
                            environment_epoch,
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
        // Retire a prior evaluation of this static node. Repeated edges are
        // distinct invocations; saved earlier values already retain their own
        // capture sequence and equations in the caller environment.
        std::sync::Arc::make_mut(&mut self.normal_results)
            .retain(|(source, _)| *source != expression);
        let mut children_to_evaluate = Vec::new();
        let namespace_receiver = program
            .expression_table
            .source_call_node(expression)
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
                (child, child_reference)
            })
            .collect::<Vec<_>>();
        let contexts = self.evaluate_normal_operands(
            &operands,
            environment,
            scope,
            depth,
            requested_call,
            entries,
            environment_epoch,
        )?;
        contexts
            .into_iter()
            .map(|(mut judge, inputs)| {
                let call_inputs =
                    if let Some(call) = program.expression_table.source_call_node(expression) {
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
                let mut live_environment = environment
                    .iter()
                    .filter(|(name, value)| {
                        judge.binding_survives_effects(name, value, environment_epoch)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if requested_call == expression {
                    entries.push(NormalCallEntry {
                        arguments: call_inputs.as_ref()?.clone(),
                        judge: judge.clone(),
                        environment: live_environment.clone(),
                    });
                }
                let frame = match node {
                    ExpressionNode::Call(call)
                        if !call_has_mathematical_denotation(program, call) =>
                    {
                        Some(
                            judge
                                .call_frames()
                                .map_or_else(facts::NormalizedWriteFrame::opaque, |frames| {
                                    frames.value_call_write_frame(scope.0, expression)
                                }),
                        )
                    }
                    _ => None,
                };
                // Mathematical substitution has no runtime storage epochs.
                // Ordered operand traversal already owns nested call effects;
                // this frame describes only the selected invocation itself.
                if !judge.runtime_body_values
                    && !judge.case_facts_only
                    && frame.as_ref().is_some_and(|frame| {
                        frame.complete_paths().is_none_or(|paths| !paths.is_empty())
                    })
                {
                    return None;
                }
                let entry = frame.as_ref().and_then(|frame| {
                    judge.prepare_normal_call(
                        expression,
                        call_inputs.as_deref(),
                        &live_environment,
                        scope,
                        frame,
                    )
                });
                if let Some(frame) = frame {
                    judge.retire_call_effects(scope.1, frame);
                    live_environment.retain(|(name, value)| {
                        judge.binding_survives_effects(name, value, environment_epoch)
                    });
                }
                if let Some(entry) = entry {
                    judge.capture_normal_call(expression, entry);
                }
                let mut value = if matches!(node, ExpressionNode::Atomic(_)) {
                    // The atomic owner distinguishes an observed load from
                    // update carriers. Its value is a fresh event, not the
                    // place's prior initializer or the modeled new resident.
                    crate::value_custody::atomic_operations::atomic_load_carrier(
                        program, expression,
                    )?;
                    judge.capture_sequence += 1;
                    StructuralTerm::CapturedValue {
                        expression,
                        callable: SymbolHandle::invalid(),
                        occurrence: judge.capture_sequence,
                        result_type: crate::expression_result_type_reference(
                            program, scope.0, scope.1, expression,
                        )?,
                    }
                } else if let Some(value) =
                    judge.callee_term_in_scope(Some(scope), expression, &live_environment, 0)
                {
                    value
                } else if judge.runtime_body_values
                    && (matches!(node, ExpressionNode::Call(_))
                        || judge.storage_observation_invalidated(expression))
                {
                    // A completed call without an imported contract, or a
                    // read after a write retired the storage's old facts, is
                    // still an observation: a fresh identity, no old facts.
                    judge.capture_sequence += 1;
                    StructuralTerm::CapturedValue {
                        expression,
                        callable: SymbolHandle::invalid(),
                        occurrence: judge.capture_sequence,
                        result_type: crate::expression_result_type_reference(
                            program, scope.0, scope.1, expression,
                        )?,
                    }
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
                    value = super::super::structural_terms::indexed_term_in_scope(
                        program,
                        Some(scope),
                        None,
                        expression,
                        collection.clone(),
                        index.clone(),
                    )?;
                }
                if let Some(reference) =
                    crate::expression_result_type_reference(program, scope.0, scope.1, expression)
                {
                    value = judge.capture_scalar_observation(expression, reference, value);
                }
                Some(NormalReturnPath { value, judge }.at_expression(expression))
            })
            .collect()
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
                value: Box::new(value),
            })
        } else {
            Some(value)
        }
    }

    fn prepare_normal_call(
        &self,
        expression: ExpressionHandle,
        actuals: Option<&[StructuralTerm]>,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        writes: &facts::NormalizedWriteFrame,
    ) -> Option<PreparedNormalCall<'program>> {
        let program = self.program;
        let node = program.expression_table.expression(expression);
        let ExpressionNode::Call(call) = node else {
            return None;
        };
        let call = program.expression_table.source_call(call)?;
        let Some(owner) = selected_call_contract(program, call) else {
            return None;
        };
        // Computed proof machines are erased mathematical applications. Their
        // definitions and induction remain with the existing term owner;
        // replacing them with opaque runtime occurrences loses that denotation.
        if call_has_mathematical_denotation(program, call) {
            return None;
        }
        let public_application = program
            .selected_public_requirement_application(
                call.authored_machine_application,
                call.static_requirement_dispatch
                    .as_ref()
                    .map_or(call.target_symbol, |dispatch| dispatch.requirement),
            )
            .is_some();
        if (!call.machine_arguments.is_empty() && !public_application)
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
        let mut selections = call.machine_arguments.to_vec();
        if public_application {
            // The complete application can infer a selection without spelling
            // it at the call. Its callback dependencies still owe the same
            // source/proof independence as explicitly selected operands.
            for binding in program
                .expression_table
                .static_machine_application_bindings(call.authored_machine_application)
            {
                match &binding.value {
                    typed_trees::static_applications::StaticMachineBindingValue::Machine(
                        argument,
                    )
                    | typed_trees::static_applications::StaticMachineBindingValue::Conformance(
                        argument,
                    ) => {
                        selections.push(argument.clone());
                    }
                    _ => {}
                }
            }
        }
        if !independent_application(
            program,
            self.machine_symbol,
            owner_symbol(&owner),
            &selections,
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
        let required_parameters = contract_parameter_dependencies(
            program,
            &owner,
            SignatureContractKind::Requires,
            parameters,
        )?;
        let machine = program.machine(scope.0.symbol)?;
        let frames = self.call_frames()?;
        let subjects = frames.expression_call_reference_subjects_for_parameters(
            machine,
            expression,
            parameters,
            &required_parameters,
            writes,
        )?;
        let bindings = parameters
            .iter()
            .zip(actuals)
            .filter(|(parameter, _)| required_parameters.contains(&parameter.symbol))
            .map(|(parameter, value)| {
                let value = if crate::reference_shell(program, parameter.type_reference).is_some() {
                    let subject = subjects
                        .iter()
                        .find(|subject| subject.parameter == parameter.symbol)?;
                    self.reference_subject_term(subject, environment, scope)?
                } else {
                    value.clone()
                };
                Some((parameter.symbol, value))
            })
            .collect::<Option<Vec<_>>>()?;
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
        for contract in contracts
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let ProofFact::Expression(required) = fact else {
                    return None;
                };
                if super::super::struct_trace() {
                    eprintln!(
                        "STRUCT normal-capture requires expression={expression:?} required={}",
                        program.expression_table.display_name(*required)
                    );
                }
                let judgment = selected_requirement_judgment(
                    self, *required, callable, parameters, &bindings, 0,
                );
                if super::super::struct_trace() {
                    let verdict = match &judgment {
                        StructuralJudgment::Proven => "proven",
                        StructuralJudgment::Refuted => "refuted",
                        StructuralJudgment::Unknown => "unknown",
                    };
                    eprintln!(
                        "STRUCT normal-capture entry-verdict expression={expression:?} judgment={verdict}"
                    );
                }
                if !matches!(judgment, StructuralJudgment::Proven) {
                    return None;
                }
            }
        }
        let guaranteed_parameters = contract_parameter_dependencies(
            program,
            &owner,
            SignatureContractKind::Ensures,
            parameters,
        )?;
        let reference_parameters = parameters
            .iter()
            .filter(|parameter| {
                guaranteed_parameters.contains(&parameter.symbol)
                    && crate::reference_shell(program, parameter.type_reference).is_some()
            })
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        let post_subjects = frames.expression_call_reference_subjects_for_parameters(
            machine,
            expression,
            parameters,
            &reference_parameters,
            writes,
        );
        let mut post_bindings = Vec::new();
        for (parameter, value) in parameters.iter().zip(actuals) {
            if parameter.is_mutable || !guaranteed_parameters.contains(&parameter.symbol) {
                continue;
            }
            if crate::reference_shell(program, parameter.type_reference).is_some() {
                if let Some(subjects) = post_subjects.as_ref()
                    && let Some(subject) = subjects
                        .iter()
                        .find(|subject| subject.parameter == parameter.symbol)
                    && subject.preserved_on_return
                    && let Some(value) = self.reference_subject_term(subject, environment, scope)
                {
                    post_bindings.push((parameter.symbol, value));
                }
            } else if frames.proof_value_is_caller_isolated(parameter.type_reference) {
                post_bindings.push((parameter.symbol, self.resolve(value.clone())));
            }
        }
        Some(PreparedNormalCall {
            owner,
            parameters: parameter_scope,
            bindings: post_bindings,
            callable,
            result_type,
        })
    }

    fn capture_normal_call(
        &mut self,
        expression: ExpressionHandle,
        entry: PreparedNormalCall<'program>,
    ) {
        let program = self.program;
        let PreparedNormalCall {
            owner,
            parameters: parameter_scope,
            bindings,
            callable,
            result_type,
        } = entry;
        let parameters = parameter_scope.as_ref();
        let contracts = owner_contracts(program, &owner).collect::<Vec<_>>();
        self.capture_sequence += 1;
        let captured = StructuralTerm::CapturedValue {
            expression,
            occurrence: self.capture_sequence,
            callable,
            result_type,
        };
        let mut equations = Vec::new();
        // A saved local now resolves to this occurrence, so the callee's case
        // tags must classify it here; the citation route's saved-binding tags
        // never meet that environment term. Tags never become value equations.
        let tag_callee = match owner {
            SelectedCallContract::Concrete(machine, _) => Some(machine),
            _ => None,
        };
        let mut tag_map = parameters
            .iter()
            .filter_map(|parameter| {
                bindings
                    .iter()
                    .find(|(symbol, _)| *symbol == parameter.symbol)
                    .map(|(_, value)| (parameter.name.as_str().to_owned(), value.clone()))
            })
            .collect::<Vec<_>>();
        tag_map.push((
            crate::proof_contracts::contract_entailment::RESULT_BINDER.to_owned(),
            captured.clone(),
        ));
        let mut case_guarantees = Vec::new();
        for contract in contracts
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let ProofFact::Expression(guarantee) = fact else {
                    continue;
                };
                capture_equations(
                    program,
                    *guarantee,
                    callable,
                    parameters,
                    callable,
                    &bindings,
                    &captured,
                    &mut equations,
                    0,
                );
                if let Some(callee) = tag_callee
                    && parameter_dependencies(program, [*guarantee], parameters).is_some_and(
                        |required| {
                            required.iter().all(|required| {
                                bindings.iter().any(|(symbol, _)| symbol == required)
                            })
                        },
                    )
                {
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
fn call_has_mathematical_denotation(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    matches!(selected_call_contract(program, call), Some(SelectedCallContract::Concrete(machine, _))
        if crate::proof_only_classification(program).is_proof_machine(program, machine))
}

fn program_expression_is_live(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    expression.is_valid() && program.expression_table.expression_is_valid(expression)
}

/// Runtime formal dependencies of the exact selected contract graph. Static
/// binders and the reserved result remain with their existing source owners.
fn contract_parameter_dependencies(
    program: &dyn crate::CheckingInput,
    owner: &SelectedCallContract<'_>,
    kind: SignatureContractKind,
    parameters: &[typed_trees::signature::StateParameter],
) -> Option<Vec<SymbolHandle>> {
    let expressions = owner_contracts(program, owner)
        .filter(|contract| contract.kind == kind)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .map(|fact| match fact {
            ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    parameter_dependencies(program, expressions, parameters)
}

pub(in crate::proof_contracts::contract_entailment) fn parameter_dependencies(
    program: &dyn crate::CheckingInput,
    expressions: impl IntoIterator<Item = ExpressionHandle>,
    parameters: &[typed_trees::signature::StateParameter],
) -> Option<Vec<SymbolHandle>> {
    let mut pending = expressions.into_iter().collect::<Vec<_>>();
    let mut visited = Vec::new();
    let mut dependencies = Vec::new();
    while let Some(expression) = pending.pop() {
        if !program_expression_is_live(program, expression) {
            return None;
        }
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        if let ExpressionNode::Name(path) = node {
            // A projected name keeps its supplying formal in head_symbol and
            // its terminal member declaration in symbol. Both coordinates
            // belong to the retained graph; neither spelling is a subject key.
            for symbol in [path.head_symbol, path.symbol] {
                if parameters
                    .iter()
                    .any(|parameter| parameter.symbol == symbol)
                {
                    if !dependencies.contains(&symbol) {
                        dependencies.push(symbol);
                    }
                } else if program.symbols.get(symbol).kind == symbols::SymbolKind::Parameter {
                    return None;
                }
            }
        }
        children(program, node, |child| {
            if child.is_valid() {
                pending.push(child);
            }
        });
    }
    Some(dependencies)
}
