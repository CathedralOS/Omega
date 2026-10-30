//! Scalar application identity and signature checks at external ingress.
//! These establish a well-formed computation, not its value or admission.

use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::{IntegerCarrier, IntegerSign, ScalarType};
use terminal_psi::{
    LayoutMeasurement, ScalarApplication, ScalarConditionSubject, ScalarRangeCondition,
    ScalarRangeObservation, TerminalCallable, TerminalModule,
};

pub(super) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
    super::proof_scope::validate(&module.scalar_applications.proof_scope, &module.identities)?;
    let scope = &module.scalar_applications.proof_scope;
    for (_, row) in scope.callables.iter() {
        let terminal_psi::ProofCallableAssociation::Callable {
            declaration,
            callable,
        } = row
        else {
            return malformed("proof scope callable association is absent");
        };
        let emitted = match callable {
            TerminalCallable::Machine(body) => module
                .machine(*body)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "proof scope names an absent emitted body",
                ))?
                .declaration
                .as_ref(),
            TerminalCallable::Boundary(boundary) => {
                let boundary = module.boundary_machine(*boundary).ok_or(
                    CodecError::MalformedStructuralFoundation(
                        "proof scope names an absent emitted boundary",
                    ),
                )?;
                if boundary.declaration.is_none()
                    && boundary.kind == terminal_psi::BoundaryRequirementKind::Operator
                {
                    validate_operator_association(
                        module,
                        boundary,
                        scope.declaration(*declaration),
                    )?;
                }
                boundary.declaration.as_ref().map(|row| &row.source)
            }
        };
        if let Some(emitted) = emitted
            && !matches!(scope.declaration(*declaration), Some(terminal_psi::ProofScopeDeclaration::Callable(retained)) if retained == emitted)
        {
            return malformed("proof scope callable differs from its emitted declaration");
        }
    }
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
                    TerminalCallable::Machine(machine) => {
                        module.machine(machine).is_some_and(|machine| {
                            machine.parameters.is_empty()
                                && machine.structural_parameters.is_empty()
                                && machine
                                    .result
                                    .scalar_ref()
                                    .is_some_and(|result| result.scalar_type == scalar_type)
                        })
                    }
                    TerminalCallable::Boundary(boundary) => {
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
    for condition in conditions.completions() {
        let terminal_psi::ScalarCompletionCondition::Complete {
            subject,
            application,
        } = condition
        else {
            return malformed("scalar completion is absent");
        };
        validate_subject(module, subject)?;
        if !matches!(
            module.scalar_applications.declaration(*application),
            Some(ScalarApplication::Evaluate { .. } | ScalarApplication::LayoutObservation { .. })
        ) {
            return malformed("scalar completion has no defined application");
        }
    }
    for condition in conditions.ranges() {
        let ScalarRangeCondition::Range { subject, .. } = condition else {
            return malformed("scalar condition is absent");
        };
        validate_subject(module, subject)?;
        conditions
            .steps(condition)
            .map_err(CodecError::MalformedStructuralFoundation)?;
    }
    for condition in conditions.predicates() {
        validate_subject(module, &condition.subject)?;
    }
    for observation in conditions.observations() {
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
                    entry: TerminalCallable::Machine(entry),
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
    Ok(())
}

/// Operator signatures retain a requirement rather than an ordinary machine
/// declaration. Compare that exact coordinate, never treat `None` as a match.
fn validate_operator_association(
    module: &TerminalModule,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    declaration: Option<&terminal_psi::ProofScopeDeclaration>,
) -> Result<(), CodecError> {
    let Some(terminal_psi::ProofScopeDeclaration::Operator(requirement)) = declaration else {
        return malformed("proof scope operator has no exact requirement declaration");
    };
    if requirement.declaration() != &boundary.owner
        || module.identities.text(boundary.identity) != Some(requirement.overload())
    {
        return malformed("proof scope operator differs from its emitted requirement");
    }
    for operation in module.operations() {
        if !matches!(operation.kind, terminal_psi::OperationKind::BoundaryCall { boundary: called, .. } if called == boundary.id)
        {
            continue;
        }
        if let Some(demand) = module
            .boundary_application_demands
            .iter()
            .find(|demand| demand.terminal_operation() == operation.id)
            && demand.requirement() != requirement
        {
            return malformed("proof scope operator differs from its application demand");
        }
    }
    Ok(())
}

fn validate_subject(
    module: &TerminalModule,
    subject: &ScalarConditionSubject,
) -> Result<(), CodecError> {
    let has_subject = match subject {
        ScalarConditionSubject::Module => true,
        ScalarConditionSubject::Declaration(declaration) => module
            .scalar_applications
            .proof_scope
            .declaration(*declaration)
            .is_some(),
        ScalarConditionSubject::Application(application) => module
            .scalar_applications
            .declaration(*application)
            .is_some(),
    };
    if !has_subject {
        return malformed("scalar condition lost its exact subject");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sections::semantic_module::declaration_catalog_tests::{
        identity_table, unit_machine,
    };
    use semantic_vocabulary::MachineId;
    use terminal_psi::{
        ProofCallableAssociation, ProofDeclarationId, ProofScopeDeclaration, TerminalIdentityIndex,
    };

    #[test]
    fn proof_scope_refuses_swapped_same_nominal_callable_descriptions() {
        let first = MachineId::new(1).unwrap();
        let second = MachineId::new(2).unwrap();
        let mut module = TerminalModule::for_entry(first);
        module.identities = identity_table(10);
        for (position, machine) in [(1, first), (2, second)] {
            let mut body = unit_machine(machine);
            let declaration = body.declaration.as_mut().unwrap();
            declaration.semantic_declaration = TerminalIdentityIndex(3);
            declaration.callable = TerminalIdentityIndex(4);
            declaration.result_type = TerminalIdentityIndex(5);
            declaration.specialization = Some(terminal_psi::CheckedMachineSpecialization {
                template: declaration.machine.clone(),
                template_callable: declaration.callable,
                template_semantic_declaration: declaration.semantic_declaration,
                commitment: [position as u8; 32],
                capture: None,
            });
            module
                .scalar_applications
                .proof_scope
                .declarations
                .insert(ProofScopeDeclaration::Callable(declaration.clone()));
            module.scalar_applications.proof_scope.callables.insert(
                ProofCallableAssociation::Callable {
                    declaration: ProofDeclarationId(position),
                    callable: TerminalCallable::Machine(machine),
                },
            );
            module.machines.push(body);
        }
        validate(&module).unwrap();
        module
            .scalar_applications
            .proof_scope
            .callables
            .for_each_mut(|_, row| {
                if let ProofCallableAssociation::Callable { declaration, .. } = row {
                    declaration.0 = 3 - declaration.0;
                }
            });
        assert!(matches!(
            validate(&module),
            Err(CodecError::MalformedStructuralFoundation(
                "proof scope callable differs from its emitted declaration"
            ))
        ));
    }

    #[test]
    fn proof_scope_refuses_swapped_operator_associations_without_machine_descriptions() {
        let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let overload = identities.intern("operator::choose(u64,u64)->u64");
        let owner =
            terminal_psi::NominalDeclarationIdentity::new(identities.intern("package::choose"));
        let other_owner =
            terminal_psi::NominalDeclarationIdentity::new(identities.intern("other::choose"));
        module.identities = identities.build();
        let boundary = terminal_psi::BoundaryMachineDeclaration {
            id: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
            declaration: None,
            numeric_intrinsic: None,
            role: terminal_psi::BoundaryDeclarationRole::Callable,
            kind: terminal_psi::BoundaryRequirementKind::Operator,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            unconditional_termination: false,
            identity: overload,
            owner: owner.clone(),
            attachment: None,
            parameter_order: Vec::new(),
            scalar_parameters: Vec::new(),
            crash_routes: Vec::new(),
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            normal_qualifications: Vec::new(),
            scalar_domain_requirements: Vec::new(),
            scalar_requires: Vec::new(),
            scalar_ensures: Vec::new(),
            reference_outputs: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            fixed_service_reach: Vec::new(),
            owning_service: None,
            published_service_ceiling: Vec::new(),
        };
        module.boundary_machines.push(boundary);
        let description = |owner, overload: &str| {
            ProofScopeDeclaration::Operator(
                terminal_psi::BoundaryOperatorRequirement::new(owner, overload.into(), None)
                    .unwrap(),
            )
        };
        let scope = &mut module.scalar_applications.proof_scope;
        for row in [
            description(owner.clone(), "operator::choose(u64,u64)->u64"),
            description(other_owner, "operator::choose(u64,u64)->u64"),
            description(owner, "operator::choose(u32,u32)->u32"),
            ProofScopeDeclaration::Nominal(overload),
        ] {
            scope.declarations.insert(row);
        }
        let association = scope.callables.insert(ProofCallableAssociation::Callable {
            declaration: ProofDeclarationId(1),
            callable: TerminalCallable::Boundary(module.boundary_machines[0].id),
        });
        validate(&module).unwrap();
        for owner in 2..=4 {
            let ProofCallableAssociation::Callable { declaration, .. } = module
                .scalar_applications
                .proof_scope
                .callables
                .get_mut(association)
            else {
                unreachable!()
            };
            *declaration = ProofDeclarationId(owner);
            assert!(
                validate(&module).is_err(),
                "wrong owner/overload or nominal fallback {owner}"
            );
        }
    }
}
