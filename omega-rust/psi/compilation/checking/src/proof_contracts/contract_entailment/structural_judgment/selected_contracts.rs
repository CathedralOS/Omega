//! Selected contract equations under exact formal and result substitutions.
//!
//! Executed normal returns and total specification applications share this
//! binding owner. Each caller supplies its own admitted result identity and
//! establishes the selected entry premises before importing any equation.

use super::super::specification_calls::{
    SelectedCallContract, owner_contracts, owner_parameters, owner_symbol,
    selected_call_contract_for_parameters, selected_contract_by_symbol,
};
use super::{StructuralJudge, StructuralJudgment, StructuralTerm};
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::SignatureContractKind;

impl StructuralJudge<'_> {
    /// Normalize an exact selected result or a view of a nested selected
    /// result. Equations are instantiated in the callable's own telescope;
    /// this does not execute the application or create a runtime occurrence.
    pub(super) fn selected_contract_normal_form(
        &self,
        value: &StructuralTerm,
        depth: usize,
    ) -> Option<StructuralTerm> {
        if depth >= 32 {
            return None;
        }
        let mut applications = Vec::new();
        collect_applications(value, &mut applications);
        for application in applications {
            let Some(equations) = self.specification_application_equations(application, depth + 1)
            else {
                continue;
            };
            for (left, right) in equations {
                for (pattern, replacement) in [(&left, &right), (&right, &left)] {
                    if pattern == value
                        && replacement != value
                        && !super::super::structural_terms::term_contains(replacement, value)
                    {
                        return Some(replacement.clone());
                    }
                }
            }
        }
        None
    }

    fn specification_application_equations(
        &self,
        application: &StructuralTerm,
        depth: usize,
    ) -> Option<Vec<(StructuralTerm, StructuralTerm)>> {
        let StructuralTerm::Application {
            target,
            selections,
            result_type,
            arguments,
            ..
        } = application
        else {
            return None;
        };
        if depth >= 32 || !result_type.is_valid() || !selections.is_empty() {
            return None;
        }
        let program = self.program;
        let owner = selected_contract_by_symbol(program, *target)?;
        let parameters = owner_parameters(program, &owner);
        if parameters.len() != arguments.len()
            || parameters.iter().any(|parameter| {
                parameter.is_mutable
                    || crate::reference_shell(program, parameter.type_reference).is_some()
            })
        {
            return None;
        }
        let declared_result = match owner {
            SelectedCallContract::Concrete(machine, state) => {
                if !independent_callee(program, self.machine_symbol, machine.symbol) {
                    return None;
                }
                let plans = crate::operand_call_plans(program, None);
                if crate::machine_calls::denotational_calls::checked_value_call_candidate(
                    program, machine, state, &plans.0, &plans.1,
                )
                .is_err()
                {
                    return None;
                }
                state.return_type
            }
            SelectedCallContract::Signature(_) => {
                // Public interface selection does not itself establish that
                // an abstract invocation has an observation-free denotation.
                return None;
            }
            SelectedCallContract::Operator(operator) => {
                if !crate::machine_calls::denotational_calls::selected_operator_has_closed_value_denotation(
                    program, operator,
                ) {
                    return None;
                }
                operator.return_type
            }
        };
        if !crate::declared_carriers_match(program, *result_type, declared_result)
            || program.arithmetic_domain_for_type_reference(*result_type)
                != program.arithmetic_domain_for_type_reference(declared_result)
        {
            return None;
        }
        let bindings = parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, value)| (parameter.symbol, value.clone()))
            .collect::<Vec<_>>();
        let callable = match owner {
            SelectedCallContract::Concrete(machine, _) => machine.symbol,
            _ => owner_symbol(&owner),
        };
        for contract in owner_contracts(program, &owner)
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let ProofFact::Expression(required) = fact else {
                    return None;
                };
                let required = bound_term(
                    program,
                    *required,
                    callable,
                    parameters,
                    callable,
                    &bindings,
                    None,
                    depth + 1,
                )?;
                let truth = StructuralTerm::Constructor {
                    data: "bool".to_owned(),
                    case: "true".to_owned(),
                    fields: Vec::new(),
                };
                if !matches!(
                    self.judge_equation(self.resolve_at(required, depth + 1), truth, depth + 1,),
                    StructuralJudgment::Proven
                ) {
                    return None;
                }
            }
        }
        let mut equations = Vec::new();
        for contract in owner_contracts(program, &owner)
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let ProofFact::Expression(guarantee) = fact {
                    capture_equations(
                        program,
                        *guarantee,
                        callable,
                        parameters,
                        callable,
                        &bindings,
                        application,
                        &mut equations,
                        depth + 1,
                    );
                }
            }
        }
        Some(equations)
    }
}

fn collect_applications<'term>(
    term: &'term StructuralTerm,
    applications: &mut Vec<&'term StructuralTerm>,
) {
    match term {
        StructuralTerm::Application { arguments, .. } => {
            for argument in arguments {
                collect_applications(argument, applications);
            }
            applications.push(term);
        }
        StructuralTerm::Projection { subject, .. } => collect_applications(subject, applications),
        StructuralTerm::ScalarBinary { left, right, .. } => {
            collect_applications(left, applications);
            collect_applications(right, applications);
        }
        StructuralTerm::Constructor { fields, .. } => {
            for (_, field) in fields {
                collect_applications(field, applications);
            }
        }
        StructuralTerm::CallProjection { arguments, .. } => {
            for argument in arguments {
                collect_applications(argument, applications);
            }
        }
        _ => {}
    }
}

/// Contract dependency cycles need the independent induction owner. These
/// ordinary value imports cannot prove a callee by citing this caller again.
pub(super) fn independent_callee(
    program: &dyn crate::CheckingInput,
    caller: SymbolHandle,
    callee: SymbolHandle,
) -> bool {
    independent_application(program, caller, callee, &[])
}

/// Independence belongs to the selected application, including the exact
/// callback machines supplied to its generic telescope. A helper's authored
/// requirement alone does not identify the selected callback's proof sources.
pub(super) fn independent_application(
    program: &dyn crate::CheckingInput,
    caller: SymbolHandle,
    callee: SymbolHandle,
    selections: &[typed_trees::expression::StaticMachineArgument],
) -> bool {
    let mut pending = vec![callee];
    pending.extend(
        crate::machine_calls::machine_parameters::machine_selection_dependency_symbols(
            program, selections,
        ),
    );
    let mut visited = Vec::new();
    while let Some(declaration) = pending.pop() {
        let Some((owner, dependencies)) =
            crate::machine_calls::call_cycles::declaration_call_dependencies(program, declaration)
        else {
            return false;
        };
        if owner == caller {
            return false;
        }
        if visited.contains(&owner) {
            continue;
        }
        visited.push(owner);
        pending.extend(dependencies);
    }
    true
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_equations(
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
    if !super::super::structural_terms::has_identity_equality_meaning(
        program, scope, parameters, expression,
    ) {
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
pub(super) fn bound_term(
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
            let result_type = crate::parameter_expression_result_type_reference(
                program, scope, parameters, expression,
            )
            .unwrap_or_default();
            Some(StructuralTerm::Application {
                result_type,
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
