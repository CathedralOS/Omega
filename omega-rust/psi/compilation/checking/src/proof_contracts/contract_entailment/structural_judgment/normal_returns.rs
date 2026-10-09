//! Selected guarantees for captured normal-return values.
//!
//! A runtime result is one occurrence, not a replayable mathematical call.
//! Capture operands before substituting the selected public signature, prove
//! its entry requirements, then import its unconditional normal guarantees.
//! This environment belongs to one straight normal-return path; it is neither
//! a loop invariant nor permission to replay mutable storage observations.

use super::super::specification_calls::{
    SelectedCallContract, owner_contracts, owner_parameters, owner_symbol, selected_call_contract,
};
use super::{StructuralJudge, StructuralJudgment, StructuralTerm};
use crate::expression_traversal::children;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::SignatureContractKind;
use typed_trees::types::TypeReferenceHandle;

impl<'program> StructuralJudge<'program> {
    /// Observe the value of one expression on the current normal-return path.
    /// Calls without a complete admitted contract remain with the existing
    /// term owner; absence of an import never supplies a guarantee.
    pub(in crate::proof_contracts::contract_entailment) fn normal_return_term(
        &mut self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: (
            &typed_trees::machine::Machine,
            &typed_trees::state::State,
            TypeReferenceHandle,
        ),
    ) -> Option<StructuralTerm> {
        // Structural child edges do not establish execution: match arms and
        // short-circuit operands belong to separately selected paths. Decline
        // that whole formation until its execution schedule is represented.
        if !unconditional_expression(self.program, expression, 0) {
            return None;
        }
        // Admission is transactional. An unsupported outer value cannot leak
        // guarantees from an inner call into a later local or return path.
        let mut candidate = self.clone();
        candidate.capture_normal_returns(expression, environment, scope, 0);
        let value = candidate.callee_term_in_scope(Some(scope), expression, environment, 0)?;
        *self = candidate;
        Some(value)
    }

    fn capture_normal_returns(
        &mut self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: (
            &typed_trees::machine::Machine,
            &typed_trees::state::State,
            TypeReferenceHandle,
        ),
        depth: usize,
    ) {
        if depth >= 64
            || !program_expression_is_live(self.program, expression)
            || self
                .normal_results
                .iter()
                .any(|(source, _)| *source == expression)
        {
            return;
        }
        let program = self.program;
        let node = program.expression_table.expression(expression);
        let mut operands = Vec::new();
        children(program, node, |child| operands.push(child));
        for operand in operands {
            self.capture_normal_returns(operand, environment, scope, depth + 1);
        }
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
        // All actual values belong to the caller environment. Installing a
        // formal map before this capture would reinterpret another actual in
        // the callee's telescope.
        let Some(actuals) = arguments
            .iter()
            .map(|argument| self.callee_term_in_scope(Some(scope), *argument, environment, 0))
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };
        let bindings = parameters
            .iter()
            .zip(actuals)
            .map(|(parameter, value)| (parameter.symbol, value))
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
        let captured = StructuralTerm::CapturedCallResult {
            expression,
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
        if equations.is_empty() {
            return;
        }
        self.normal_results.push((expression, captured));
        for (left, right) in equations {
            self.normal_guarantees.push((left.clone(), right.clone()));
            self.intake_equation(left, right, 0);
        }
    }
}

/// Only total eager value edges are a schedule in this structural owner.
/// Selective or unsupported evaluation stays with the ordinary flow owner;
/// walking all structural descendants would invent normal-return authority.
fn unconditional_expression(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= 64 || !program_expression_is_live(program, expression) {
        return false;
    }
    let node = program.expression_table.expression(expression);
    if matches!(node, ExpressionNode::Match(_) | ExpressionNode::Atomic(_))
        || matches!(node, ExpressionNode::Binary(binary)
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or))
    {
        return false;
    }
    let mut admitted = true;
    children(program, node, |child| {
        // An absent receiver is a structural edge, not an executed operand.
        if child.is_valid() {
            admitted &= unconditional_expression(program, child, depth + 1);
        }
    });
    admitted
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
            selected_call_contract(program, call)?;
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
                target: super::super::structural_terms::structural_call_target(program, call),
                selections: call.machine_arguments.to_vec(),
                machine: super::super::structural_terms::structural_call_machine_name(
                    call.target.as_str(),
                    &call.machine_arguments,
                    &[],
                ),
                arguments,
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
