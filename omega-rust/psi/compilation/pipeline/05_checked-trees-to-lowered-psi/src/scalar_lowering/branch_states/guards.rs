//! Evaluate the selected guard once before dispatching either branch.
use super::{
    CheckedScalarExpressionRole, CheckedTrees, LoweringError, QualifiedScalarType, ScalarType,
    computations, scalar_carriers, storage, unsupported, validate_boolean_parameter_types,
};
use crate::emission::expression_validation::validate_short_circuit_expression;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::scalar_lowering::branch_states::prepared::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator,
};
use crate::scalar_lowering::contracts::LoweredProofTerm;

/// Whether the guard at `statement` replays a checked computation root (a
/// selected IEEE or boundary comparison) rather than a pure expression. A
/// computed guard has no pure guard expression to inspect; `evaluate` binds
/// its Boolean result as an appended parameter instead.
pub(crate) fn is_computed(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
) -> bool {
    checked
        .facts
        .values
        .scalar_computations
        .roots_for_statement_role(state, statement, CheckedScalarExpressionRole::Guard)
        .next()
        .is_some()
}

/// The caller has reconstructed its complete dispatch (binary or ordered).
/// This evaluator owns just one selected guard and never starts either arm.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    bindings: &storage::ScalarBindings,
    source_types: &[QualifiedScalarType],
    when_true: (
        usize,
        Vec<LoweredDirectExpression>,
        Vec<LoweredDirectExpression>,
        Vec<LoweredProofTerm>,
    ),
    when_false: (
        usize,
        Vec<LoweredDirectExpression>,
        Vec<LoweredDirectExpression>,
        Vec<LoweredProofTerm>,
    ),
    computations: &mut computations::Expansion<'_>,
) -> Result<LoweredScalarBranchTerminator, LoweringError> {
    let computed = is_computed(checked, state, statement);
    let condition = if computed {
        LoweredBooleanReturnExpression::Parameter {
            position: source_types.len(),
        }
    } else {
        let LoweredDirectExpression::Boolean { expression } = bindings.expression_at(
            identities,
            checked,
            state,
            statement,
            CheckedScalarExpressionRole::Guard,
        )?
        else {
            return unsupported("checked scalar graph guard must be Boolean");
        };
        validate_short_circuit_expression(&expression)?;
        validate_boolean_parameter_types(&expression, &scalar_carriers(source_types))?;
        *expression
    };
    let branch = LoweredScalarBranchTerminator::Conditional {
        condition,
        when_true_target: when_true.0,
        when_true_arguments: when_true.1,
        when_true_erased_arguments: when_true.2,
        when_true_erased_proof_arguments: when_true.3,
        when_false_target: when_false.0,
        when_false_arguments: when_false.1,
        when_false_erased_arguments: when_false.2,
        when_false_erased_proof_arguments: when_false.3,
    };
    if !computed {
        return Ok(branch);
    }
    // Branch arguments still refer to the unchanged source prefix. Only the
    // condition consumes the appended result; neither arm starts before it.
    let mut parameter_types = source_types.to_vec();
    parameter_types.push(ScalarType::Boolean.into());
    let target = computations.push(LoweredScalarBranchState {
        structural_parameters: Vec::new(),
        structural_effects: Vec::new(),
        parameter_types,
        erased_formal_types: Vec::new(),
        erased_proof_formals: Vec::new(),
        bindings: Vec::new(),
        terminator: branch,
    });
    let target = computations.retained_value(
        identities,
        state,
        statement,
        CheckedScalarExpressionRole::Guard,
        bindings,
        source_types,
        ScalarType::Boolean.into(),
        target,
    )?;
    Ok(LoweredScalarBranchTerminator::Jump {
        cleanup_actions: Vec::new(),
        structural_arguments: Vec::new(),
        target,
        arguments: computations::parameters(source_types),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
    })
}
