use super::*;

pub(in crate::legalization) fn read(
    function: &PsiOptimizationFunction,
    operation: &AbstractOperation,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<(
    semantic_vocabulary::OperationId,
    abstract_operations::AbstractResult,
    terminal_psi::StructuralArgument,
    semantic_vocabulary::StructuralFieldId,
)> {
    let (operation, result, place, field) = match operation {
        AbstractOperation::IntegerStructuralField {
            psi_operation,
            result,
            source,
            field,
        } => {
            if !function.structural_parameters.contains(source) {
                return None;
            }
            (*psi_operation, *result, source.place, *field)
        }
        AbstractOperation::BooleanStructuralField {
            psi_operation,
            result,
            source,
            field,
        } => (
            *psi_operation,
            abstract_operations::AbstractResult {
                value: *result,
                scalar_type: ScalarType::Boolean,
            },
            *source,
            *field,
        ),
        _ => return None,
    };
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)?;
    let source = terminal_psi::StructuralArgument {
        place,
        access: parameter.access,
        path: Vec::new(),
    };
    crate::structural_reference_input::field_read(
        parameter,
        &source,
        field,
        result.scalar_type,
        types,
    )?;
    Some((operation, result, source, field))
}
