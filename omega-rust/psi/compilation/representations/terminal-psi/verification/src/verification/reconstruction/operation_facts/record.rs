//! Declaration-owned payload validity precedes atomic record establishment.
use super::{
    ModuleError, Obligation, ObligationClass, Operation, OperationKind, Proposition,
    ReconstructedOperationObligation, ReconstructedTerminalObligationOwner, ScalarType,
    TerminalMachine, TerminalModule,
};
use semantic_vocabulary::{CanonicalStructuralPathSegment, ScalarTerm};

pub(super) fn append(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    axioms: &mut Vec<Proposition>,
    obligations: &mut Vec<ReconstructedOperationObligation>,
) -> Result<(), ModuleError> {
    let declarations = crate::validation::record::fields(module, machine, operation)?;
    let OperationKind::EstablishRecord { fields } = &operation.kind else {
        return Err(ModuleError::RecordResultMismatch(operation.id));
    };
    let result = operation
        .result
        .structural()
        .ok_or(ModuleError::RecordResultMismatch(operation.id))?;
    // Reexecution creates fresh storage. No observation of the previous
    // referent may justify the new initializer or survive its establishment.
    axioms.retain(|proposition| {
        !crate::validation::proposition_observes_places(proposition, &[result.place])
    });
    for (declaration, binding) in declarations.clone().zip(fields) {
        if let terminal_psi::RecordFieldValue::Scalar {
            value,
            range_obligation,
        } = binding.value
            && let Some(proposition) =
                terminal_semantics::scalar_field_requirement(&declaration.field_type, value)
        {
            let id = range_obligation.ok_or(ModuleError::RecordResultMismatch(operation.id))?;
            obligations.push(ReconstructedOperationObligation {
                owner: ReconstructedTerminalObligationOwner::Operation {
                    machine: machine.id,
                    operation: operation.id,
                },
                obligation: Obligation {
                    id,
                    proposition,
                    class: ObligationClass::Derivable,
                },
                semantic_axioms: axioms.clone(),
                canonical_certificate: true,
            });
        }
    }
    for (declaration, binding) in declarations.clone().zip(fields) {
        let terminal_psi::RecordFieldValue::Scalar { value, .. } = binding.value else {
            let terminal_psi::RecordFieldValue::Structural(argument) = &binding.value else {
                unreachable!()
            };
            if let Some(source_path) = crate::validation::canonical_field_path(
                module,
                machine,
                argument.place,
                &argument.path,
            ) {
                super::owned_contents::append(
                    module,
                    &declaration.field_type,
                    result.place,
                    vec![CanonicalStructuralPathSegment::Field(binding.field)],
                    argument.place,
                    source_path,
                    axioms,
                );
            }
            continue;
        };
        let path = vec![CanonicalStructuralPathSegment::Field(binding.field)];
        let scalar_type = declaration
            .field_type
            .scalar_type()
            .ok_or(ModuleError::RecordResultMismatch(operation.id))?;
        let field = match scalar_type {
            ScalarType::Integer(integer_type) => {
                ScalarTerm::integer_field_path(result.place, path, integer_type)
            }
            ScalarType::Boolean => ScalarTerm::boolean_field_path(result.place, path),
            ScalarType::IeeeFloat(format) => {
                ScalarTerm::ieee_float_field_path(result.place, path, format)
            }
        };
        axioms.push(Proposition::Equal(
            field,
            ScalarTerm::value(value, scalar_type),
        ));
    }
    Ok(())
}

// Whole child transfer preserves observations through exact declaration paths.
// The prior child equations remain usable until the atomic operation completes.
pub(super) fn append_array(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    axioms: &mut Vec<Proposition>,
) -> Result<(), ModuleError> {
    crate::validation::structural_array::operands(module, machine, operation)?;
    let result = operation
        .result
        .structural()
        .ok_or(ModuleError::RecordResultMismatch(operation.id))?;
    let OperationKind::EstablishStructuralArray { elements } = &operation.kind else {
        return Err(ModuleError::RecordResultMismatch(operation.id));
    };
    axioms.retain(|proposition| {
        !crate::validation::proposition_observes_places(proposition, &[result.place])
    });
    for (index, argument) in elements.iter().enumerate() {
        let element = module
            .structural_types
            .iter()
            .find_map(|row| match row.shape {
                terminal_psi::StructuralTypeShape::FixedArray { element, .. }
                    if row.id == result.structural_type =>
                {
                    Some(element)
                }
                _ => None,
            })
            .ok_or(ModuleError::RecordResultMismatch(operation.id))?;
        let path = crate::validation::canonical_field_path(
            module,
            machine,
            argument.place,
            &argument.path,
        )
        .ok_or(ModuleError::RecordResultMismatch(operation.id))?;
        super::owned_contents::append(
            module,
            &terminal_psi::StructuralFieldType::Structural(element),
            result.place,
            vec![CanonicalStructuralPathSegment::FixedIndex(index as u64)],
            argument.place,
            path,
            axioms,
        );
    }
    Ok(())
}
