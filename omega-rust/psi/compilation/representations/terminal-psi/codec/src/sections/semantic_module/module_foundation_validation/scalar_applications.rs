//! Scalar application identity and signature checks at external ingress.
//! These establish a well-formed computation, not its value or admission.

use crate::codec_error::{CodecError, malformed};
use numerics::integer_bounds::IntegerBoundStep;
use semantic_vocabulary::{IntegerCarrier, IntegerSign, ScalarType};
use terminal_psi::{
    LayoutMeasurement, ScalarApplication, ScalarConditionSubject, ScalarRangeCondition,
    ScalarRangeObservation, TargetMachineCallable, TerminalModule,
};

pub(super) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
    super::proof_scope::validate(&module.scalar_applications.proof_scope, &module.identities)?;
    for declaration in &module.structural_types {
        if let terminal_psi::StructuralTypeShape::FixedArray {
            length: terminal_psi::ArrayExtent::Application(application),
            ..
        } = declaration.shape
        {
            let scalar_type = module
                .scalar_applications
                .declaration(application)
                .and_then(|declaration| declaration.scalar_type());
            // An extent helper retains its authored integer carrier. Target
            // admission separately proves the value nonnegative and fitting
            // the element-count representation; ingress grants neither fact.
            if !matches!(scalar_type, Some(ScalarType::Integer(_))) {
                return malformed("array extent application has no integer result carrier");
            }
        }
    }
    for application in module.scalar_applications.declarations() {
        match *application {
            ScalarApplication::Missing => {
                return malformed("scalar application catalog contains the absent sentinel");
            }
            ScalarApplication::LayoutObservation {
                subject,
                semantics_version,
                scalar_type,
                ..
            } => {
                if module.structural_type(subject).is_none() {
                    return malformed("layout observation has no structural subject declaration");
                }
                if semantics_version != LayoutMeasurement::SEMANTICS_VERSION {
                    return malformed("layout observation semantics version is unsupported");
                }
                if !matches!(scalar_type, ScalarType::Integer(integer)
                    if integer.carrier() == IntegerCarrier::Fixed
                        && integer.sign() == IntegerSign::Unsigned
                        && integer.bits() == 64)
                {
                    return malformed("layout observation result is not the byte-count carrier");
                }
            }
            ScalarApplication::Evaluate { entry, scalar_type } => {
                let signature_matches = match entry {
                    TargetMachineCallable::Machine(machine) => {
                        module.machine(machine).is_some_and(|machine| {
                            machine.parameters.is_empty()
                                && machine.structural_parameters.is_empty()
                                && machine
                                    .result
                                    .scalar_ref()
                                    .is_some_and(|result| result.scalar_type == scalar_type)
                        })
                    }
                    TargetMachineCallable::Boundary(boundary) => {
                        module.boundary_machine(boundary).is_some_and(|boundary| {
                            boundary.scalar_parameters.is_empty()
                                && boundary.structural_parameters.is_empty()
                                && boundary.result.scalar() == Some(scalar_type)
                        })
                    }
                };
                if !signature_matches {
                    return malformed("scalar application entry has a different closed signature");
                }
            }
        }
    }
    let conditions = &module.scalar_applications.conditions;
    for condition in conditions.ranges() {
        let ScalarRangeCondition::Range { subject, .. } = condition else {
            return malformed("scalar condition is absent");
        };
        let has_subject = match subject {
            ScalarConditionSubject::Module => true,
            ScalarConditionSubject::Declaration(declaration) => module
                .scalar_applications
                .proof_scope
                .declaration(*declaration)
                .is_some(),
            ScalarConditionSubject::StructuralType(subject) => {
                module.structural_type(*subject).is_some()
            }
            ScalarConditionSubject::Application(application) => module
                .scalar_applications
                .declaration(*application)
                .is_some(),
        };
        if !has_subject {
            return malformed("scalar condition lost its exact subject");
        }
        for step in conditions
            .steps(condition)
            .map_err(CodecError::MalformedStructuralFoundation)?
        {
            let IntegerBoundStep::Observation(observation) = step else {
                continue;
            };
            match observation {
                ScalarRangeObservation::Missing => {
                    return malformed("scalar condition contains an absent observation");
                }
                ScalarRangeObservation::Literal(_) => {}
                ScalarRangeObservation::Application(application) => {
                    if !module
                        .scalar_applications
                        .declaration(*application)
                        .and_then(|application| application.scalar_type())
                        .is_some_and(|scalar_type| scalar_type.as_integer().is_some())
                    {
                        return malformed("scalar bound observation has no integer application");
                    }
                }
                ScalarRangeObservation::EntryResult {
                    application,
                    machine,
                    result,
                    scalar_type,
                } => {
                    let Some(ScalarApplication::Evaluate {
                        entry: TargetMachineCallable::Machine(entry),
                        ..
                    }) = module.scalar_applications.declaration(*application)
                    else {
                        return malformed("scalar bound result has no ordinary evaluation entry");
                    };
                    if entry != machine || scalar_type.as_integer().is_none() {
                        return malformed(
                            "scalar bound result differs from its exact entry or carrier",
                        );
                    }
                    let found = module.machine(*machine).and_then(|machine| {
                        machine
                            .operations()
                            .filter_map(|operation| operation.result.as_scalar())
                            .find(|value| value.id == *result)
                    });
                    if !found.is_some_and(|value| value.scalar_type == *scalar_type) {
                        return malformed(
                            "scalar bound result has no exact integer-producing occurrence",
                        );
                    }
                }
            }
        }
    }
    Ok(())
}
