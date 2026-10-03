//! Declaration-owned payload validity precedes atomic case establishment.
use super::{
    ModuleError, Obligation, ObligationClass, Operation, OperationKind, Proposition,
    ReconstructedOperationObligation, ReconstructedTerminalObligationOwner, ScalarType,
    TerminalMachine, TerminalModule,
};
use semantic_vocabulary::{CanonicalStructuralPathSegment, ScalarTerm, StructuralCaseSubject};

pub(super) fn append(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    axioms: &mut Vec<Proposition>,
    obligations: &mut Vec<ReconstructedOperationObligation>,
) -> Result<(), ModuleError> {
    let declarations = crate::validation::structural::case::fields(module, machine, operation)?;
    let OperationKind::EstablishStructuralCase {
        result_case,
        fields,
    } = &operation.kind
    else {
        return Err(ModuleError::StructuralCaseResultMismatch(operation.id));
    };
    let result = operation
        .result
        .structural()
        .ok_or(ModuleError::StructuralCaseResultMismatch(operation.id))?;
    // Reexecution creates fresh storage. No observation of the previous
    // referent may justify the new initializer or survive its establishment.
    axioms.retain(|proposition| {
        !crate::validation::proposition_observes_places(proposition, &[result.place])
    });
    for (entry, binding) in declarations.iter().zip(fields) {
        if let terminal_psi::RecordFieldValue::Scalar {
            value,
            range_obligation,
        } = binding.value
            && let Some(proposition) =
                terminal_semantics::scalar_field_requirement(&entry.declaration.field_type, value)
        {
            let id =
                range_obligation.ok_or(ModuleError::StructuralCaseResultMismatch(operation.id))?;
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
    axioms.push(Proposition::StructuralCaseMembership {
        subject: StructuralCaseSubject::new(result.place, Vec::new()),
        case: *result_case,
    });
    for (entry, binding) in declarations.iter().zip(fields) {
        let declaration = entry.declaration;
        // A mixed shape's common field is read directly on the root; a
        // payload field only beneath its selected case.
        let path = if entry.common {
            vec![CanonicalStructuralPathSegment::Field(binding.field)]
        } else {
            vec![
                CanonicalStructuralPathSegment::Case(*result_case),
                CanonicalStructuralPathSegment::Field(binding.field),
            ]
        };
        let terminal_psi::RecordFieldValue::Scalar { value, .. } = binding.value else {
            let terminal_psi::RecordFieldValue::Structural(argument) = &binding.value else {
                unreachable!()
            };
            let terminal_psi::StructuralFieldType::Structural(child) = declaration.field_type
            else {
                return Err(ModuleError::StructuralCaseResultMismatch(operation.id));
            };
            super::record::append_child_fields(
                module,
                child,
                result.place,
                path,
                argument.place,
                Vec::new(),
                axioms,
            );
            continue;
        };
        let scalar_type = declaration
            .field_type
            .scalar_type()
            .ok_or(ModuleError::StructuralCaseResultMismatch(operation.id))?;
        let field = match scalar_type {
            ScalarType::Integer(integer_type) => {
                ScalarTerm::integer_field_path(result.place, path, integer_type)
            }
            ScalarType::Boolean => ScalarTerm::boolean_field_path(result.place, path),
            ScalarType::IeeeFloat(_) => continue,
        };
        axioms.push(Proposition::Equal(
            field,
            ScalarTerm::value(value, scalar_type),
        ));
    }
    Ok(())
}
