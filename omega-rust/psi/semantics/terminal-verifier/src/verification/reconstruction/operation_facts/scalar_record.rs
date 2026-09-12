//! Reconstruct initialized field equations from exact declarations and SSA inputs.

use super::*;
use semantic_vocabulary::{CanonicalStructuralPathSegment, ScalarTerm};

pub(super) fn append(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    axioms: &mut Vec<Proposition>,
) -> Result<(), ModuleError> {
    let declarations = crate::validation::scalar_record::fields(module, machine, operation)?;
    let OperationKind::EstablishScalarRecord { fields } = &operation.kind else {
        return Err(ModuleError::ScalarRecordResultMismatch(operation.id));
    };
    let result = operation
        .result
        .structural()
        .ok_or(ModuleError::ScalarRecordResultMismatch(operation.id))?;
    axioms.retain(|proposition| {
        !crate::validation::proposition_observes_places(proposition, &[result.place])
    });
    for (declaration, binding) in declarations.iter().zip(fields) {
        let scalar_type = declaration
            .field_type
            .scalar_type()
            .ok_or(ModuleError::ScalarRecordResultMismatch(operation.id))?;
        let path = vec![CanonicalStructuralPathSegment::Field(binding.field)];
        let field = match scalar_type {
            ScalarType::Integer(integer_type) => {
                ScalarTerm::integer_field_path(result.place, path, integer_type)
            }
            ScalarType::Boolean => ScalarTerm::boolean_field_path(result.place, path),
            ScalarType::IeeeFloat(_) => continue,
        };
        axioms.push(Proposition::Equal(
            field,
            ScalarTerm::value(binding.value, scalar_type),
        ));
    }
    Ok(())
}
