//! Declaration-owned payload validity precedes atomic record establishment.

use super::*;
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
    for (declaration, binding) in declarations.iter().zip(fields) {
        if let terminal_psi::StructuralFieldType::BoundedInteger(bounds) = declaration.field_type {
            let terminal_psi::RecordFieldValue::Scalar {
                value,
                range_obligation,
            } = binding.value
            else {
                return Err(ModuleError::RecordResultMismatch(operation.id));
            };
            let id = range_obligation.ok_or(ModuleError::RecordResultMismatch(operation.id))?;
            let integer_type = bounds.integer_type();
            let value = ScalarTerm::value(value, ScalarType::Integer(integer_type));
            let endpoint = |value| ScalarTerm::Integer {
                scalar_type: integer_type,
                value,
            };
            let mut bounds = vec![
                Proposition::LessOrEqual(endpoint(bounds.minimum()), value.clone()),
                Proposition::LessOrEqual(value, endpoint(bounds.maximum())),
            ];
            bounds.sort();
            obligations.push(ReconstructedOperationObligation {
                owner: ReconstructedTerminalObligationOwner::Operation {
                    machine: machine.id,
                    operation: operation.id,
                },
                obligation: Obligation {
                    id,
                    proposition: Proposition::Conjunction(bounds),
                    class: ObligationClass::Derivable,
                },
                semantic_axioms: axioms.clone(),
                canonical_certificate: true,
            });
        }
    }
    for (declaration, binding) in declarations.iter().zip(fields) {
        let terminal_psi::RecordFieldValue::Scalar { value, .. } = binding.value else {
            let terminal_psi::RecordFieldValue::Structural(argument) = &binding.value else {
                unreachable!()
            };
            let terminal_psi::StructuralFieldType::Structural(child) = declaration.field_type
            else {
                return Err(ModuleError::RecordResultMismatch(operation.id));
            };
            append_child_fields(
                module,
                child,
                result.place,
                vec![CanonicalStructuralPathSegment::Field(binding.field)],
                argument.place,
                Vec::new(),
                axioms,
            );
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
            ScalarType::IeeeFloat(_) => continue,
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
fn append_child_fields(
    module: &TerminalModule,
    structural_type: semantic_vocabulary::StructuralTypeId,
    destination: semantic_vocabulary::PlaceId,
    destination_path: Vec<CanonicalStructuralPathSegment>,
    source: semantic_vocabulary::PlaceId,
    source_path: Vec<CanonicalStructuralPathSegment>,
    axioms: &mut Vec<Proposition>,
) {
    let Some(declaration) = module
        .structural_types
        .iter()
        .find(|item| item.id == structural_type)
    else {
        return;
    };
    let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
        return;
    };
    for field in fields {
        let mut target = destination_path.clone();
        target.push(CanonicalStructuralPathSegment::Field(field.id));
        let mut origin = source_path.clone();
        origin.push(CanonicalStructuralPathSegment::Field(field.id));
        if let terminal_psi::StructuralFieldType::Structural(child) = field.field_type {
            append_child_fields(module, child, destination, target, source, origin, axioms);
        } else {
            let equation = match field.field_type.scalar_type() {
                Some(ScalarType::Integer(integer)) => Some((
                    ScalarTerm::integer_field_path(destination, target, integer),
                    ScalarTerm::integer_field_path(source, origin, integer),
                )),
                Some(ScalarType::Boolean) => Some((
                    ScalarTerm::boolean_field_path(destination, target),
                    ScalarTerm::boolean_field_path(source, origin),
                )),
                _ => None,
            };
            if let Some((left, right)) = equation {
                axioms.push(Proposition::Equal(left, right));
            }
        }
    }
}
