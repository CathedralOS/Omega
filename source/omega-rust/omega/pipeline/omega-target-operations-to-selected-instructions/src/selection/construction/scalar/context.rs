//! Common condition-input and physical-register context reconstruction.

use crate::selection::constraints::fixed_input_constraint;
use crate::selection::shared::*;

use super::model::{ConditionInputContext, ScalarConstructionContext};

pub(super) fn reconstruct<'a>(
    function: usize,
    source: &'a SourceFunction,
    constraints: &'a SelectedSelectionConstraints,
    physical: &'a ValidatedPhysicalRegisterModel,
    catalog: &'a ValidatedRegisterConstraintCatalog,
) -> Result<ScalarConstructionContext<'a>, SelectedInstructionError> {
    let u64_type =
        ScalarType::Integer(psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"));
    let condition_inputs = match &source.condition {
        LegalizedCondition::DirectParameter {
            parameter_index,
            register,
            definition_site,
        } => vec![reconstruct_input(
            function,
            source.machine,
            source.condition_source,
            *parameter_index,
            *register,
            *definition_site,
            ScalarType::Boolean,
            constraints,
            physical,
        )?],
        LegalizedCondition::U64EqualZeroParameterV1 { parameter, .. }
        | LegalizedCondition::U64NotEqualZeroParameterV1 { parameter, .. } => {
            vec![reconstruct_input(
                function,
                source.machine,
                parameter.source_value,
                parameter.parameter_index,
                parameter.register,
                parameter.definition_site,
                u64_type,
                constraints,
                physical,
            )?]
        }
        LegalizedCondition::IntegerEqualParametersV1 { left, right, .. }
        | LegalizedCondition::IntegerLessThanParametersV1 { left, right, .. }
        | LegalizedCondition::IntegerLessOrEqualParametersV1 { left, right, .. }
        | LegalizedCondition::IntegerNotEqualParametersV1 { left, right, .. } => [left, right]
            .into_iter()
            .map(|parameter| {
                reconstruct_input(
                    function,
                    source.machine,
                    parameter.source_value,
                    parameter.parameter_index,
                    parameter.register,
                    parameter.definition_site,
                    u64_type,
                    constraints,
                    physical,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        LegalizedCondition::I64LessThanParametersV1 { left, right, .. }
        | LegalizedCondition::I64LessOrEqualParametersV1 { left, right, .. } => {
            let i64_type = ScalarType::Integer(
                psi_core::IntegerType::new(IntegerSign::Signed, 64).expect("i64"),
            );
            [left, right]
                .into_iter()
                .map(|parameter| {
                    reconstruct_input(
                        function,
                        source.machine,
                        parameter.source_value,
                        parameter.parameter_index,
                        parameter.register,
                        parameter.definition_site,
                        i64_type,
                        constraints,
                        physical,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?
        }
    };
    Ok(ScalarConstructionContext {
        function,
        source,
        constraints,
        physical,
        catalog,
        condition_inputs,
        u64_type,
    })
}

#[allow(clippy::too_many_arguments)]
fn reconstruct_input(
    function: usize,
    machine: psi_core::MachineId,
    source_value: psi_core::ValueId,
    parameter_index: usize,
    register: omega_target_operations::MachineRegister,
    definition_site: ValueDefinitionSite,
    scalar_type: ScalarType,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<ConditionInputContext, SelectedInstructionError> {
    let input = fixed_input_constraint(
        machine,
        source_value,
        parameter_index,
        register,
        &constraints.fixed_inputs,
    )
    .ok_or(SelectedInstructionError::MissingInputRegisterView { function })?;
    let class = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == input.fixed_view)
        .ok_or(SelectedInstructionError::MissingInputRegisterView { function })?
        .class;
    Ok(ConditionInputContext {
        source_value,
        parameter_index,
        definition_site,
        scalar_type,
        class,
        view: input.fixed_view,
    })
}
