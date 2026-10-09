//! Selected guarantees for captured normal-return values.
//!
//! A runtime result is one occurrence, not a replayable mathematical call.
//! Capture operands before substituting the selected public signature, prove
//! its entry requirements, then import its unconditional normal guarantees.
//! This environment belongs to one straight normal-return path; it is neither
//! a loop invariant nor permission to replay mutable storage observations.

use super::super::specification_calls::{
    SelectedCallContract, owner_contracts, owner_parameters, owner_symbol, selected_call_contract,
    selected_call_contract_for_parameters,
};
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

type ValueScope<'scope> = (
    &'scope typed_trees::machine::Machine,
    &'scope typed_trees::state::State,
    TypeReferenceHandle,
);

impl<'program> StructuralJudge<'program> {
    /// Project normal outcomes under their own invocation facts. Every eager
    /// operand executes in order; selective operands fork the same path owner.
    /// No branch's hypotheses are donated to a different normal outcome.
    pub(in crate::proof_contracts::contract_entailment) fn normal_return_paths(
        &self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        self.clone()
            .evaluate_normal_paths(expression, environment, scope, 0)
    }

    fn evaluate_normal_paths(
        mut self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        depth: usize,
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
            for left in self.evaluate_normal_paths(binary.left, environment, scope, depth + 1)? {
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
            let mut remaining =
                self.evaluate_normal_paths(dispatch.subject, environment, scope, depth + 1)?;
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
        if matches!(node, ExpressionNode::Call(_)) {
            let machine = program.machine(scope.0.symbol)?;
            let writes = crate::machine_calls::calls::CallFrameResolver::new(program)?
                .expression_may_write_paths(machine, expression)?;
            if !writes.is_empty() {
                return None;
            }
        }
        // Retire a prior evaluation of this static node. Repeated edges are
        // distinct invocations; saved earlier values already retain their own
        // capture sequence and equations in the caller environment.
        self.normal_results
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
        let mut contexts = vec![(self, Vec::new())];
        for child in children_to_evaluate {
            let mut next = Vec::new();
            for (context, inputs) in contexts {
                for path in context.evaluate_normal_paths(child, environment, scope, depth + 1)? {
                    let mut inputs = inputs.clone();
                    inputs.push((child, path.value));
                    next.push((path.judge, inputs));
                }
            }
            contexts = next;
        }
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
                judge.capture_normal_call(expression, call_inputs.as_deref(), scope);
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
                } else {
                    judge.callee_term_in_scope(Some(scope), expression, environment, 0)?
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
                        expression,
                        left.clone(),
                        right.clone(),
                    )?;
                }
                Some(NormalReturnPath { value, judge }.at_expression(expression))
            })
            .collect()
    }

    fn assume_normal_equation(&mut self, left: StructuralTerm, right: StructuralTerm) {
        self.normal_guarantees.push((left.clone(), right.clone()));
        self.intake_equation(left, right, 0);
    }

    fn capture_normal_call(
        &mut self,
        expression: ExpressionHandle,
        actuals: Option<&[StructuralTerm]>,
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
        if !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || call.quotient_operation.is_some()
        {
            return;
        }
        // A reference argument names storage, not a detached value snapshot.
        // Its mutable/post-state relation belongs to the live call-frame owner.
        let parameters = owner_parameters(program, &owner);
        if parameters.iter().any(|parameter| {
            parameter.is_mutable
                || crate::reference_shell(program, parameter.type_reference).is_some()
        }) {
            return;
        }
        if let SelectedCallContract::Concrete(callee, _) = owner
            && !independent_callee(program, self.machine_symbol, callee.symbol)
        {
            return;
        }
        let arguments = program.expression_table.expression_handles(call.arguments);
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
        let Some(actuals) = actuals.filter(|actuals| actuals.len() == arguments.len()) else {
            return;
        };
        let bindings = parameters
            .iter()
            .zip(actuals)
            .map(|(parameter, value)| (parameter.symbol, value.clone()))
            .collect::<Vec<_>>();
        let callable = match owner {
            SelectedCallContract::Concrete(machine, _) => machine.symbol,
            _ => owner_symbol(&owner),
        };
        let result_type = match owner {
            SelectedCallContract::Concrete(_, state) => state.return_type,
            SelectedCallContract::Signature(signature) => signature.return_type,
            SelectedCallContract::Operator(operator) => operator.return_type,
        };
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
                let Some(required) = bound_term(
                    program,
                    *required,
                    owner_symbol(&owner),
                    parameters,
                    callable,
                    &bindings,
                    None,
                    0,
                ) else {
                    return;
                };
                let truth = StructuralTerm::Constructor {
                    data: "bool".to_owned(),
                    case: "true".to_owned(),
                    fields: Vec::new(),
                };
                if !matches!(
                    self.judge_equation(self.resolve(required), truth, 0),
                    StructuralJudgment::Proven
                ) {
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
                    owner_symbol(&owner),
                    parameters,
                    callable,
                    &bindings,
                    &captured,
                    &mut equations,
                    0,
                );
            }
        }
        if super::super::struct_trace() {
            eprintln!(
                "STRUCT normal-call expression={expression:?} public_owner={:?} result_type={result_type:?} equations={equations:?}",
                owner_symbol(&owner),
            );
        }
        self.normal_results.push((expression, captured));
        for (left, right) in equations {
            self.normal_guarantees.push((left.clone(), right.clone()));
            self.intake_equation(left, right, 0);
        }
    }
}

impl NormalReturnPath<'_> {
    fn at_expression(mut self, expression: ExpressionHandle) -> Self {
        self.judge
            .normal_results
            .retain(|(source, _)| *source != expression);
        self.judge
            .normal_results
            .push((expression, self.value.clone()));
        self
    }
}

fn boolean_term(value: bool) -> StructuralTerm {
    StructuralTerm::Constructor {
        data: "bool".to_owned(),
        case: value.to_string(),
        fields: Vec::new(),
    }
}

fn program_expression_is_live(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    expression.is_valid() && program.expression_table.expression_is_valid(expression)
}

/// Contract dependency cycles need the independent induction owner. These
/// ordinary value imports cannot prove a callee by citing this caller again.
fn independent_callee(
    program: &dyn crate::CheckingInput,
    caller: SymbolHandle,
    callee: SymbolHandle,
) -> bool {
    crate::machine_calls::effect_inference::with_operational_plan(program, |operational| {
        let mut pending = vec![callee];
        let mut visited = Vec::new();
        while let Some(machine) = pending.pop() {
            if machine == caller {
                return false;
            }
            if visited.contains(&machine) {
                continue;
            }
            visited.push(machine);
            let Some(summary) = operational.machine_by_symbol(machine) else {
                return false;
            };
            for state in operational.states.span_or_empty(summary.states) {
                for call in operational.calls.span_or_empty(state.calls) {
                    if call.target_machine_symbol.is_valid() {
                        pending.push(call.target_machine_symbol);
                    }
                }
            }
        }
        true
    })
}

#[allow(clippy::too_many_arguments)]
fn capture_equations(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    scope: SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    callable: SymbolHandle,
    bindings: &[(SymbolHandle, StructuralTerm)],
    result: &StructuralTerm,
    equations: &mut Vec<(StructuralTerm, StructuralTerm)>,
    depth: usize,
) {
    if depth >= 64 {
        return;
    }
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return;
    };
    if binary.operator == BinaryOperator::And {
        if !crate::has_builtin_parameter_bound_expression_meaning(
            program, scope, parameters, expression,
        ) {
            return;
        }
        capture_equations(
            program,
            binary.left,
            scope,
            parameters,
            callable,
            bindings,
            result,
            equations,
            depth + 1,
        );
        capture_equations(
            program,
            binary.right,
            scope,
            parameters,
            callable,
            bindings,
            result,
            equations,
            depth + 1,
        );
        return;
    }
    if binary.operator != BinaryOperator::Equal {
        return;
    }
    let types = [binary.left, binary.right].map(|operand| {
        crate::parameter_expression_result_type_reference(program, scope, parameters, operand)
    });
    if types.iter().any(Option::is_none)
        || !typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            scope,
            expression,
            language_core::OperatorSpelling::Equal,
            &types,
        )
    {
        return;
    }
    if let (Some(left), Some(right)) = (
        bound_term(
            program,
            binary.left,
            scope,
            parameters,
            callable,
            bindings,
            Some(result),
            0,
        ),
        bound_term(
            program,
            binary.right,
            scope,
            parameters,
            callable,
            bindings,
            Some(result),
            0,
        ),
    ) {
        equations.push((left, right));
    }
}

/// Formal substitution uses selected symbols. Names are only the existing
/// structural judge's diagnostic payload; no caller spelling chooses a formal.
#[allow(clippy::too_many_arguments)]
fn bound_term(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    scope: SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    callable: SymbolHandle,
    bindings: &[(SymbolHandle, StructuralTerm)],
    result: Option<&StructuralTerm>,
    depth: usize,
) -> Option<StructuralTerm> {
    if depth >= 64 {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if let Some((_, value)) = bindings
                .iter()
                .find(|(symbol, _)| symbol.is_valid() && *symbol == path.symbol)
            {
                return Some(value.clone());
            }
            if crate::reserved_result_owner(program, expression)
                .is_some_and(|owner| owner.0 == callable)
            {
                return result.cloned();
            }
            if path.symbol.is_valid()
                && program.symbols.get(path.symbol).kind != symbols::SymbolKind::Parameter
            {
                return Some(StructuralTerm::BoundValue(path.symbol));
            }
            None
        }
        ExpressionNode::Call(call) => {
            let selected = selected_call_contract_for_parameters(program, call, scope, parameters)?;
            let arguments = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| {
                    bound_term(
                        program,
                        *argument,
                        scope,
                        parameters,
                        callable,
                        bindings,
                        result,
                        depth + 1,
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            Some(StructuralTerm::Application {
                target: owner_symbol(&selected),
                selections: call.machine_arguments.to_vec(),
                machine: super::super::structural_terms::structural_call_machine_name(
                    call.target.as_str(),
                    &call.machine_arguments,
                    &[],
                ),
                arguments,
            })
        }
        ExpressionNode::StructLiteral(literal) => {
            super::super::structural_terms::constructor_literal_term(program, literal, |field| {
                bound_term(
                    program,
                    field.value,
                    scope,
                    parameters,
                    callable,
                    bindings,
                    result,
                    depth + 1,
                )
            })
        }
        ExpressionNode::Integer(_) | ExpressionNode::Float(_) | ExpressionNode::Boolean(_) => {
            super::super::structural_terms::structural_term(program, expression)
        }
        ExpressionNode::Binary(binary) => {
            if !crate::has_builtin_parameter_bound_expression_meaning(
                program, scope, parameters, expression,
            ) {
                return None;
            }
            let meaning = [binary.left, binary.right].map(|operand| {
                let reference = crate::parameter_expression_result_type_reference(
                    program, scope, parameters, operand,
                )?;
                Some((
                    program.primitive_type_reference(reference)?,
                    program.arithmetic_domain_for_type_reference(reference),
                ))
            });
            Some(StructuralTerm::ScalarBinary {
                operator: binary.operator,
                meaning: [meaning[0]?, meaning[1]?],
                left: Box::new(bound_term(
                    program,
                    binary.left,
                    scope,
                    parameters,
                    callable,
                    bindings,
                    result,
                    depth + 1,
                )?),
                right: Box::new(bound_term(
                    program,
                    binary.right,
                    scope,
                    parameters,
                    callable,
                    bindings,
                    result,
                    depth + 1,
                )?),
            })
        }
        _ => None,
    }
}
