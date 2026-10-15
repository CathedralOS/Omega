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
        self.clone().evaluate_normal_paths(
            expression,
            environment,
            scope,
            0,
            ExpressionHandle::invalid(),
            &mut Vec::new(),
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
                    .unwrap_or(scope.2);
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
                )?
                .into_iter()
                .map(|(judge, values)| NormalCallEntry {
                    arguments: values.into_iter().map(|(_, value)| value).collect(),
                    judge,
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
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        if depth >= 64 || !program_expression_is_live(self.program, expression) {
            return None;
        }
        let program = self.program;
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
        )?;
        contexts
            .into_iter()
            .map(|(mut judge, inputs)| {
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
                if requested_call == expression {
                    entries.push(NormalCallEntry {
                        arguments: call_inputs.as_ref()?.clone(),
                        judge: judge.clone(),
                    });
                }
                judge.capture_normal_call(expression, call_inputs.as_deref(), environment, scope);
                // Arguments have completed, so their copied values keep their
                // identities. Only later storage observations see these writes.
                // Selected contracts separately require preserved live subjects;
                // mutation cannot turn pre-state arguments into post-state facts.
                judge.observe_storage_writes(scope.1, &writes)?;
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
                    judge.callee_term_in_scope(Some(scope), expression, environment, 0)
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

    fn capture_normal_call(
        &mut self,
        expression: ExpressionHandle,
        actuals: Option<&[StructuralTerm]>,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
    ) {
        let program = self.program;
        let node = program.expression_table.expression(expression);
        let ExpressionNode::Call(call) = node else {
            return;
        };
        let Some(owner) = selected_call_contract(program, call) else {
            return;
        };
        // Computed proof machines are erased mathematical applications. Their
        // definitions and induction remain with the existing term owner;
        // replacing them with opaque runtime occurrences loses that denotation.
        if call_has_mathematical_denotation(program, call) {
            return;
        }
        if !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || call.quotient_operation.is_some()
        {
            return;
        }
        let Some(parameter_scope) = owner_parameter_scope(program, &owner) else {
            return;
        };
        let parameters = parameter_scope.as_ref();
        if super::super::struct_trace() {
            eprintln!(
                "STRUCT normal-capture entry expression={expression:?} owner={:?} arguments={actuals:?}",
                owner_symbol(&owner)
            );
        }
        if parameters.iter().any(|parameter| parameter.is_mutable) {
            return;
        }
        // References are live subjects. Only the frame owner can publish their
        // exact prefix origins and preservation through this normal return.
        // A written subject or replaced formal binding needs post-state facts,
        // not the detached child term captured during operand evaluation.
        let Some(machine) = program.machine(scope.0.symbol) else {
            return;
        };
        let Some(subjects) = self.frame_resolver().and_then(|frames| {
            frames.expression_call_reference_subjects(machine, expression, parameters)
        }) else {
            return;
        };
        if !independent_application(
            program,
            self.machine_symbol,
            owner_symbol(&owner),
            &call.machine_arguments,
        ) {
            return;
        }
        let leading_operands = owner_leading_operands(&owner);
        let Some(arguments) = program
            .expression_table
            .expression_handles(call.arguments)
            .get(leading_operands..)
        else {
            return;
        };
        if arguments.len() != parameters.len() {
            return;
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
            return;
        };
        let Some(bindings) = parameters
            .iter()
            .zip(actuals)
            .map(|(parameter, value)| {
                let value = if crate::reference_shell(program, parameter.type_reference).is_some() {
                    let subject = subjects
                        .iter()
                        .find(|subject| subject.parameter == parameter.symbol)?;
                    if !subject.preserved_on_return {
                        return None;
                    }
                    self.reference_subject_term(subject, environment, scope)?
                } else {
                    value.clone()
                };
                Some((parameter.symbol, value))
            })
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };
        let callable = match owner {
            SelectedCallContract::Concrete(machine, _) => machine.symbol,
            _ => owner_symbol(&owner),
        };
        let result_type = owner_return_type(&owner);
        let Some(observed_type) =
            crate::expression_result_type_reference(program, scope.0, scope.1, expression)
        else {
            return;
        };
        // A selected policy adapter may change the returned denotation. A raw
        // declaration's equation cannot describe that adapted result.
        if !crate::declared_carriers_match(program, observed_type, result_type)
            || program.arithmetic_domain_for_type_reference(observed_type)
                != program.arithmetic_domain_for_type_reference(result_type)
        {
            return;
        }
        let contracts = owner_contracts(program, &owner).collect::<Vec<_>>();
        for contract in contracts
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let ProofFact::Expression(required) = fact else {
                    return;
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
                    return;
                }
            }
        }
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
        let tag_map = parameters
            .iter()
            .zip(&bindings)
            .map(|(parameter, (_, value))| (parameter.name.as_str().to_owned(), value.clone()))
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
