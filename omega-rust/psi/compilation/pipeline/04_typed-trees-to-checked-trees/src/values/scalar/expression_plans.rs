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
    retain_call_arguments, value_call_expression,
};
use crate::values::scalar::expression_facts::operator_is_builtin;
use crate::values::scalar::scalar_lowering::{lower_index_expression, lower_return_expression};
use crate::values::scalar::selected_operator_operands::lower_selected_operator_operands;
use crate::values::scalar::semantic_casts;
use checked_trees::{
    CheckedBooleanExpression, CheckedLocatedScalarExpression, CheckedOperatorFacts,
    CheckedScalarExpression, CheckedScalarExpressionBindings, CheckedScalarExpressionPlans,
    CheckedScalarExpressionRole,
};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

mod array_elements;
mod assignment;
mod call_statement;
mod expression_statement;
pub(crate) mod local_data;
mod range_operands;
mod transition;

#[derive(Debug, Clone)]
pub(crate) struct ScalarLocal {
    pub(crate) is_mutable: bool,
    pub(crate) symbol: symbols::SymbolHandle,
    pub(crate) name: String,
    pub(crate) primitive_type: PrimitiveType,
    pub(crate) arithmetic_domain: ArithmeticDomain,
    /// A reference binding is observed through its established carrier. It owns
    /// no scalar value ordinal, regardless of the expression that produced it.
    pub(crate) is_reference: bool,
}

impl ScalarLocal {
    /// The scalar reading of an ordinary `LocalData` declaration: a named,
    /// non-reference local carrying its declared primitive shape and domain.
    pub(crate) fn from_local_data(
        program: &dyn ::validation::CheckingInput,
        local: &typed_trees::statement::TableLocalData,
        primitive_type: PrimitiveType,
    ) -> Self {
        Self {
            is_mutable: local.is_mutable,
            symbol: local.symbol,
            name: local.name.as_str().to_owned(),
            primitive_type,
            arithmetic_domain: program.arithmetic_domain_for_type_reference(local.type_reference),
            is_reference: false,
        }
    }
}

impl ScalarLocal {
    /// Immutable value bindings own a `Local` position and a binding
    /// ordinal; reference carriers stay out of the
    /// value-binding namespace.
    pub(crate) fn is_value_binding(&self) -> bool {
        !self.is_mutable && !self.is_reference
    }
}

/// The scalar-parameter plus value-binding-local symbol roster every
/// located expression binding carries.
pub(crate) fn located_binding_symbols<'a>(
    scalar_parameters: &'a [typed_trees::signature::StateParameter],
    locals: &'a [ScalarLocal],
) -> impl Iterator<Item = symbols::SymbolHandle> + 'a {
    scalar_parameters
        .iter()
        .map(|parameter| parameter.symbol)
        .chain(
            locals
                .iter()
                .filter(|local| local.is_value_binding())
                .map(|local| local.symbol),
        )
}

/// One statement's place in the plan walk: the program facts every
/// statement kind reads, the state's scalar signature and the rosters each
/// statement's plans extend.
struct StatementPlanner<'p, 's> {
    program: &'p dyn ::validation::CheckingInput,
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
    /// Lower one value-call expression's scalar arguments and retain them. The
    /// call-producing statements each guard the same lowering with their own
    /// source test (`value_call_expression` for assignment and local data, the
    /// statement's own expression for an expression statement).
    pub(super) fn retain_expression_call_arguments(&mut self, expression: ExpressionHandle) {
        let Some(call) = self.program.expression_table.call_node(expression) else {
            return;
        };
        let Some(arguments) = lower_call_arguments(
            self.program,
            self.operators,
            self.state,
            self.statement_ordinal,
            0,
            &validation::CallSite::Expression { expression, call },
            self.scalar_parameters,
            self.parameters,
            self.parameter_types,
            self.locals,
            self.exact_integer_casts,
        ) else {
            return;
        };
        retain_call_arguments(
            arguments,
            self.scalar_parameters,
            self.locals,
            self.expressions,
            self.proof_terms,
            self.source_bindings,
            self.binding_symbols,
        );
    }

    /// Records the source-binding and located-expression rows one bound scalar
    /// value contributes to its plan.
    fn record_scalar_value_pair(
        &mut self,
        role: CheckedScalarExpressionRole,
        destination: symbols::SymbolHandle,
        bound_expression: ExpressionHandle,
        located_expression: CheckedScalarExpression,
    ) {
        self.source_bindings
            .append(CheckedScalarExpressionBindings {
                destination,
                state: self.state.symbol,
                statement_ordinal: self.statement_ordinal,
                role,
                expression: bound_expression,
                symbols: self
                    .binding_symbols
                    .insert_many(located_binding_symbols(self.scalar_parameters, self.locals)),
            });
        self.expressions.push(CheckedLocatedScalarExpression {
            state: self.state.symbol,
            statement_ordinal: self.statement_ordinal,
            role,
            expression: located_expression,
        });
    }
}

pub(crate) fn build_checked_scalar_expression_plans(
    program: &dyn ::validation::CheckingInput,
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
            let (scalar_parameters, parameter_types) =
                crate::values::scalar::scalar_parameter_roster(program, parameters)
                    .expect("filtered scalar parameters retain primitive carriers");
            let result_type = program.primitive_type_reference(state.return_type);
            for (statement_index, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
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
                array_elements::plan(&mut planner, statement);
                range_operands::plan(&mut planner, statement);
                match statement {
                    StatementNode::LocalData(local) => local_data::plan(&mut planner, local),
                    StatementNode::Expression(expression) => {
                        expression_statement::plan(&mut planner, *expression)
                    }
                    StatementNode::Assignment(assignment) => {
                        assignment::plan(&mut planner, assignment)
                    }
                    StatementNode::Call(call) => call_statement::plan(&mut planner, call),
                    StatementNode::Transition(transition) => {
                        transition::plan(&mut planner, transition)
                    }
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
        ..Default::default()
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
            symbols: binding_symbols
                .insert_many(located_binding_symbols(scalar_parameters, locals)),
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
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    mut expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    // Successful checking has already established literal landing and selected
    // comparison meaning. Comparing these immutable payloads needs no runtime
    // arithmetic, whether the literals are anonymous or have declared carriers.
    if let Some(binary) = program.expression_table.binary_node(expression)
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
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return None;
    };
    if !operator_is_builtin(operators, expression) {
        return None;
    }
    let left = program.expression_table.integer_node(binary.left)?;
    let right = program.expression_table.integer_node(binary.right)?;
    let left = left.value_bignum()?;
    let right = right.value_bignum()?;
    let value = binary.operator.evaluate_comparison(&left, &right)?;
    Some(CheckedBooleanExpression::Constant(value))
}

pub(crate) fn assignment_target_primitive_type(
    program: &dyn ::validation::CheckingInput,
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
