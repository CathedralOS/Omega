//! Building the checked scalar expression plans of one program: the scalar
//! locals, the plan walk and the closed literal guards it lowers.
//!
//! The walk visits every statement of every state once. A
//! `StatementPlanner` carries the state's scalar signature and the rosters
//! the statement extends; `array_elements` plans a closed array literal's
//! elements first, and the statement's kind selects its own planner beside
//! this file.

use crate::values::scalar::boolean_lowering::lower_boolean_guard;
use crate::values::scalar::call_lowering::{
    LoweredCallArguments, lower_call_arguments, lower_direct_call_binding_arguments,
    retain_call_arguments, scalar_qualified_call_expression,
};
use crate::values::scalar::expression_facts::operator_is_builtin;
use crate::values::scalar::machine_parameter_booleans::lower_machine_parameter_boolean_expression;
use crate::values::scalar::scalar_lowering::{lower_index_expression, lower_return_expression};
use crate::values::scalar::selected_operator_operands::lower_selected_operator_operands;
use crate::values::scalar::semantic_casts;
use checked_trees::{
    CheckedBooleanExpression, CheckedLocatedScalarExpression, CheckedOperatorFacts,
    CheckedScalarExpression, CheckedScalarExpressionBindings, CheckedScalarExpressionPlans,
    CheckedScalarExpressionRole,
};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

mod array_elements;
mod assignment;
mod call_statement;
mod expression_statement;
mod local_data;
mod transition;

#[derive(Debug, Clone)]
pub(crate) struct ScalarLocal {
    pub(crate) is_mutable: bool,
    pub(crate) symbol: symbols::SymbolHandle,
    pub(crate) name: String,
    pub(crate) primitive_type: PrimitiveType,
    pub(crate) arithmetic_domain: ArithmeticDomain,
}

/// One statement's place in the plan walk: the program facts every
/// statement kind reads, the state's scalar signature and the rosters each
/// statement's plans extend.
struct StatementPlanner<'p, 's> {
    program: &'p TypedTrees,
    operators: &'p CheckedOperatorFacts,
    exact_integer_casts: &'p [validation::ExactIntegerCastFact],
    proof_only: &'s typed_trees::proof_only::ProofOnlyClassification,
    machine: &'p typed_trees::machine::Machine,
    state: &'p typed_trees::state::State,
    states: &'p [typed_trees::state::State],
    parameters: &'p [typed_trees::signature::StateParameter],
    scalar_parameters: &'s [typed_trees::signature::StateParameter],
    parameter_types: &'s [PrimitiveType],
    result_type: Option<PrimitiveType>,
    statement_index: usize,
    statement_ordinal: u32,
    locals: &'s mut Vec<ScalarLocal>,
    expressions: &'s mut Vec<CheckedLocatedScalarExpression>,
    proof_terms: &'s mut Vec<checked_trees::CheckedLocatedProofTerm>,
    source_bindings: &'s mut arena::Arena<CheckedScalarExpressionBindings>,
    binding_symbols: &'s mut arena::Arena<symbols::SymbolHandle>,
}

impl<'p> StatementPlanner<'p, '_> {
    /// Lend this statement's rosters to a pass that runs before the
    /// statement's own plan.
    fn reborrow(&mut self) -> StatementPlanner<'p, '_> {
        StatementPlanner {
            program: self.program,
            operators: self.operators,
            exact_integer_casts: self.exact_integer_casts,
            proof_only: self.proof_only,
            machine: self.machine,
            state: self.state,
            states: self.states,
            parameters: self.parameters,
            scalar_parameters: self.scalar_parameters,
            parameter_types: self.parameter_types,
            result_type: self.result_type,
            statement_index: self.statement_index,
            statement_ordinal: self.statement_ordinal,
            locals: self.locals,
            expressions: self.expressions,
            proof_terms: self.proof_terms,
            source_bindings: self.source_bindings,
            binding_symbols: self.binding_symbols,
        }
    }
}

pub(crate) fn build_checked_scalar_expression_plans(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    proof_terms: &mut Vec<checked_trees::CheckedLocatedProofTerm>,
) -> CheckedScalarExpressionPlans {
    let mut expressions = Vec::new();
    let mut source_bindings = arena::Arena::default();
    let mut binding_symbols = arena::Arena::default();
    let proof_only = validation::proof_only_classification(program);
    for machine in program.machines() {
        let states = program.machine_states(machine);
        for state in states {
            let mut locals = Vec::new();
            let parameters = program.state_parameters(state);
            let scalar_parameters = parameters
                .iter()
                .filter(|parameter| {
                    crate::values::scalar::occupies_scalar_position(program, parameter)
                })
                .cloned()
                .collect::<Vec<_>>();
            let parameter_types = scalar_parameters
                .iter()
                .map(|parameter| program.primitive_type_reference(parameter.type_reference))
                .collect::<Option<Vec<_>>>()
                .expect("filtered scalar parameters retain primitive carriers");
            let result_type = program.primitive_type_reference(state.return_type);
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let Ok(statement_ordinal) = u32::try_from(statement_index) else {
                    continue;
                };
                let mut planner = StatementPlanner {
                    program,
                    operators,
                    exact_integer_casts,
                    proof_only: &proof_only,
                    machine,
                    state,
                    states,
                    parameters,
                    scalar_parameters: &scalar_parameters,
                    parameter_types: &parameter_types,
                    result_type,
                    statement_index,
                    statement_ordinal,
                    locals: &mut locals,
                    expressions: &mut expressions,
                    proof_terms,
                    source_bindings: &mut source_bindings,
                    binding_symbols: &mut binding_symbols,
                };
                array_elements::plan(planner.reborrow(), statement);
                match statement {
                    StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                        local_data::plan(planner, local)
                    }
                    StatementNode::Expression(expression) => {
                        expression_statement::plan(planner, *expression)
                    }
                    StatementNode::Assignment(assignment) => assignment::plan(planner, assignment),
                    StatementNode::Call(call) => call_statement::plan(planner, call),
                    StatementNode::Transition(transition) => transition::plan(planner, transition),
                    _ => {}
                }
            }
        }
    }
    // A pure payload tree cannot carry an authored semantic qualification
    // transfer. Leave those exact source bindings to the computation graph.
    let mut retained_bindings = arena::Arena::default();
    for (_, binding) in source_bindings.iter() {
        if semantic_casts::requires_custody(program, binding.state, binding.expression) {
            expressions.retain(|expression| {
                expression.state != binding.state
                    || expression.statement_ordinal != binding.statement_ordinal
                    || expression.role != binding.role
            });
        } else {
            retained_bindings.append(binding.clone());
        }
    }
    CheckedScalarExpressionPlans {
        expressions,
        source_bindings: retained_bindings,
        binding_symbols,
    }
}

/// Retain one range's lowered endpoints as source-bound pure plans. Endpoints
/// read the site's scalar namespace but bind no destination: the range, not
/// a local or formal, consumes them.
fn retain_subslice_endpoints(
    endpoints: Vec<(
        ExpressionHandle,
        CheckedScalarExpressionRole,
        CheckedScalarExpression,
    )>,
    state: symbols::SymbolHandle,
    statement_ordinal: u32,
    scalar_parameters: &[typed_trees::signature::StateParameter],
    locals: &[ScalarLocal],
    expressions: &mut Vec<CheckedLocatedScalarExpression>,
    source_bindings: &mut arena::Arena<CheckedScalarExpressionBindings>,
    binding_symbols: &mut arena::Arena<symbols::SymbolHandle>,
) {
    for (endpoint, role, expression) in endpoints {
        source_bindings.append(CheckedScalarExpressionBindings {
            destination: symbols::SymbolHandle::invalid(),
            state,
            statement_ordinal,
            role,
            expression: endpoint,
            symbols: binding_symbols.insert_many(
                scalar_parameters
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .chain(
                        locals
                            .iter()
                            .filter(|local| !local.is_mutable)
                            .map(|local| local.symbol),
                    ),
            ),
        });
        expressions.push(CheckedLocatedScalarExpression {
            state,
            statement_ordinal,
            role,
            expression,
        });
    }
}

pub(crate) fn lower_closed_integer_literal_guard(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    mut expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    // Successful checking has already established literal landing and selected
    // comparison meaning. Comparing these immutable payloads needs no runtime
    // arithmetic, whether the literals are anonymous or have declared carriers.
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
        && binary.operator == BinaryOperator::Equal
        && operator_is_builtin(operators, expression)
    {
        match (
            program.expression_table.expression(binary.left),
            program.expression_table.expression(binary.right),
        ) {
            (ExpressionNode::Boolean(true), _) => expression = binary.right,
            (_, ExpressionNode::Boolean(true)) => expression = binary.left,
            _ => {}
        }
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if !operator_is_builtin(operators, expression) {
        return None;
    }
    let ExpressionNode::Integer(left) = program.expression_table.expression(binary.left) else {
        return None;
    };
    let ExpressionNode::Integer(right) = program.expression_table.expression(binary.right) else {
        return None;
    };
    let left = left.value_bignum()?;
    let right = right.value_bignum()?;
    let value = match binary.operator {
        BinaryOperator::Equal => left == right,
        BinaryOperator::NotEqual => left != right,
        BinaryOperator::Less => left < right,
        BinaryOperator::LessOrEqual => left <= right,
        BinaryOperator::Greater => left > right,
        BinaryOperator::GreaterOrEqual => left >= right,
        _ => return None,
    };
    Some(CheckedBooleanExpression::Constant(value))
}

pub(crate) fn assignment_target_primitive_type(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let mut crossed_reference = false;
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } if !crossed_reference => {
                crossed_reference = true;
                type_reference = *referee;
            }
            TypeReferenceNode::Reference { .. } => return None,
            _ => return program.primitive_type_reference(type_reference),
        }
    }
}
