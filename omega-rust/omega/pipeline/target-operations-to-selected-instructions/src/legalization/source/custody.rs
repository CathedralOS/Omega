use super::shared::*;

pub(super) fn validate_source_custody(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    if optimization_validation::validate_psi_optimization_unit(unit).is_err()
        || target.psi != abstract_plan.psi
        || target.psi != unit.psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
        || optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
    {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(())
}

pub(super) fn validate_source_register_architecture(
    functions: &[SourceFunction],
    architecture: target::Architecture,
) -> Result<(), LegalizationError> {
    if functions.iter().any(|function| {
        let condition_register_mismatch = match &function.condition {
            LegalizedCondition::DirectParameter { register, .. } => {
                register.architecture() != architecture
            }
            LegalizedCondition::IntegerEqualParametersV1 { left, right, .. }
            | LegalizedCondition::IntegerLessThanParametersV1 { left, right, .. }
            | LegalizedCondition::IntegerLessOrEqualParametersV1 { left, right, .. }
            | LegalizedCondition::I64LessThanParametersV1 { left, right, .. }
            | LegalizedCondition::I64LessOrEqualParametersV1 { left, right, .. } => {
                left.register.architecture() != architecture
                    || right.register.architecture() != architecture
            }
            LegalizedCondition::IntegerNotEqualParametersV1 { left, right, .. } => {
                left.register.architecture() != architecture
                    || right.register.architecture() != architecture
            }
            LegalizedCondition::U64EqualZeroParameterV1 { parameter, .. }
            | LegalizedCondition::U64NotEqualZeroParameterV1 { parameter, .. } => {
                parameter.register.architecture() != architecture
            }
        };
        condition_register_mismatch
            || match (&function.when_true.value, &function.when_false.value) {
                (
                    SourceLeafValue::EntryParameter { register: left, .. },
                    SourceLeafValue::EntryParameter {
                        register: right, ..
                    },
                ) => left.architecture() != architecture || right.architecture() != architecture,
                _ => false,
            }
    }) {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(())
}
