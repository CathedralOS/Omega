use crate::runtime_structural_path_tip;
use crate::substitution;
use crate::substitution::substitute_proposition_values;
use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};
use std::collections::BTreeMap;

pub(crate) fn scalar_conjuncts(propositions: &[Proposition]) -> Vec<Proposition> {
    let mut pending = propositions.iter().rev().collect::<Vec<_>>();
    let mut result = Vec::new();
    while let Some(proposition) = pending.pop() {
        if let Proposition::Conjunction(children) = proposition {
            pending.extend(children.iter().rev());
        } else if proposition.visit_value_ids(|_| {}) {
            result.push(proposition.clone());
        }
    }
    result
}

pub fn instantiate_payload_scalar_guarantees(
    contract: &terminal_psi::MachineContract,
    bindings: &[(ValueId, ScalarTerm)],
) -> Vec<Proposition> {
    let substitutions = bindings.iter().cloned().collect::<BTreeMap<_, _>>();
    if substitutions.len() != bindings.len() {
        return Vec::new();
    }
    let guarantees = contract
        .ensures
        .iter()
        .map(|clause| clause.proposition.clone())
        .collect::<Vec<_>>();
    scalar_conjuncts(&guarantees)
        .into_iter()
        .filter_map(|proposition| {
            let mut bound = true;
            proposition
                .visit_value_ids(|value| bound &= substitutions.contains_key(&value));
            bound.then(|| substitute_proposition_values(&proposition, &substitutions))
        })
        .collect()
}

pub(crate) struct TerminalPostconditions {
    pub scalar: Vec<Proposition>,
    pub stored: Vec<(super::DescriptorLocation, ScalarTerm)>,
}

pub(crate) fn terminal_postconditions(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    operation: &terminal_psi::Operation,
    scalar_expressions: &super::payload_predicates::TerminalScalarExpressions<'_>,
    nominal_calls: &[terminal_psi::generic_calls::NominalCallArgument],
) -> Option<TerminalPostconditions> {
    use terminal_psi::{
        OperationKind, OperationResult, StructuralAccess, TerminalMachineResult,
    };
    let (callee, arguments, structural_arguments): (_, &[_], &[_]) = match &operation.kind {
        OperationKind::Call {
            callee, arguments, ..
        } => (*callee, arguments, &[]),
        OperationKind::CallUnit {
            callee,
            arguments,
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            callee,
            arguments,
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            callee,
            arguments,
            structural_arguments,
            ..
        } => (*callee, arguments, structural_arguments),
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            ..
        } => (*callee, &[], structural_arguments),
        _ => return None,
    };
    let mut matching = module
        .machines
        .iter()
        .filter(|machine| machine.id == callee);
    let callee = matching.next()?;
    if matching.next().is_some()
        || arguments.len() != callee.parameters.len()
        || structural_arguments.len() != callee.structural_parameters.len()
        || !callee.contract.erased_scalar_formals.is_empty()
        || !callee.contract.erased_proof_formals.is_empty()
    {
        return None;
    }
    let types = machine
        .parameters
        .iter()
        .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
        .chain(
            machine
                .operations()
                .filter_map(|operation| operation.result.scalar_ref()),
        )
        .collect::<Vec<_>>();
    let mut bindings = Vec::new();
    for (formal, actual) in callee.parameters.iter().zip(arguments) {
        if !types
            .iter()
            .any(|value| value.id == *actual && value.scalar_type == formal.scalar_type)
            || types
                .iter()
                .any(|value| value.id == *actual && value.scalar_type != formal.scalar_type)
        {
            return None;
        }
        bindings.push((formal.id, scalar_expressions.expression(*actual)?));
    }
    let mut storage = Vec::new();
    for (ordinal, (formal, actual)) in callee
        .structural_parameters
        .iter()
        .zip(structural_arguments)
        .enumerate()
    {
        let root_type = machine.structural_place_type(actual.place)?;
        let selected = runtime_structural_path_tip(
            module.structural_types.iter(),
            root_type,
            &actual.path,
        );
        if actual.access != formal.access
            || (!selected.is_some_and(|selected| {
                selected.id == formal.structural_type
                    || nominal_calls
                        .binary_search_by_key(
                            &(machine.id, operation.id, ordinal as u32),
                            |row| (row.caller, row.operation, row.argument),
                        )
                        .ok()
                        .is_some_and(|position| {
                            let row = &nominal_calls[position];
                            row.actual == selected.id
                                && row.expected == formal.structural_type
                                && crate::calls::nominal_argument_matches(
                                    module, machine, callee, arguments, row,
                                )
                        })
                    || crate::calls::const_array_argument_matches(
                        module,
                        machine,
                        callee,
                        arguments,
                        selected.id,
                        formal.structural_type,
                    )
            }) && crate::borrowed_primitive_projection_declaration(
                module.structural_types.iter(),
                root_type,
                &actual.path,
                Some(formal.structural_type),
            ) != Some(formal.structural_type))
        {
            return None;
        }
        if actual.access == StructuralAccess::Owned {
            continue;
        }
        if let Some(location) =
            crate::descriptors::descriptor_lifetimes::descriptor_location(
                &module.structural_types,
                actual.place,
                root_type,
                &actual.path,
                None,
            )
        {
            storage.push((formal.place, formal.structural_type, location));
        }
    }
    match (&callee.result, &operation.result) {
        (TerminalMachineResult::Unit, OperationResult::Unit) => {}
        (TerminalMachineResult::Scalar(signature), OperationResult::Scalar(result))
            if signature.scalar_type == result.scalar_type
                && !arguments.contains(&result.id) =>
        {
            bindings.push((
                signature.id,
                ScalarTerm::value(result.id, result.scalar_type),
            ));
        }
        (
            TerminalMachineResult::Structural(signature),
            OperationResult::Structural(result),
        ) if signature.structural_type == result.structural_type
            && !structural_arguments
                .iter()
                .any(|actual| actual.place == result.place) =>
        {
            storage.push((
                signature.place,
                signature.structural_type,
                super::DescriptorLocation::whole(result.place),
            ));
        }
        _ => return None,
    }
    let substitutions = bindings.iter().cloned().collect::<BTreeMap<_, _>>();
    if substitutions.len() != bindings.len() {
        return None;
    }
    let mut postconditions = TerminalPostconditions {
        scalar: instantiate_payload_scalar_guarantees(&callee.contract, &bindings),
        stored: Vec::new(),
    };
    let mut pending = callee
        .contract
        .ensures
        .iter()
        .map(|clause| &clause.proposition)
        .collect::<Vec<_>>();
    while let Some(proposition) = pending.pop() {
        if let Proposition::Conjunction(children) = proposition {
            pending.extend(children);
            continue;
        }
        let finite = proof_admission::lower_integer_math_relation(proposition);
        let Proposition::Equal(left, right) = finite.as_ref().unwrap_or(proposition) else {
            continue;
        };
        for (subject, value) in [(left, right), (right, left)] {
            let (root, path) = match subject {
                ScalarTerm::IntegerField { root, path, .. }
                | ScalarTerm::BooleanField {
                    version: semantic_vocabulary::ContentPlaceVersion::Current,
                    root,
                    path,
                } => (*root, path),
                _ => continue,
            };
            if subject.scalar_type() != value.scalar_type() {
                continue;
            }
            let mut bound = true;
            let immutable = Proposition::Equal(value.clone(), value.clone())
                .visit_value_ids(|value| bound &= substitutions.contains_key(&value));
            if !immutable || !bound {
                continue;
            }
            let Some((_, carrier, destination)) =
                storage.iter().find(|(formal, _, _)| *formal == root)
            else {
                continue;
            };
            let Some(selected) =
                crate::descriptors::descriptor_lifetimes::canonical_descriptor_location(
                    &module.structural_types,
                    root,
                    *carrier,
                    path,
                )
            else {
                continue;
            };
            if super::payload_predicates::observation_type(
                &module.structural_types,
                *carrier,
                &selected.path,
            ) != Some(value.scalar_type())
            {
                continue;
            }
            let mut destination = destination.clone();
            destination.path.extend(selected.path);
            postconditions.stored.push((
                destination,
                substitution::substitute_scalar_term_values(value, &substitutions),
            ));
        }
    }
    Some(postconditions)
}

pub fn case_payload_scalar_types(
    types: &[terminal_psi::StructuralTypeDeclaration],
    root: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    case: semantic_vocabulary::StructuralCaseId,
    fields: &[semantic_vocabulary::StructuralFieldId],
) -> Option<Vec<semantic_vocabulary::ScalarType>> {
    let carrier = runtime_structural_path_tip(types.iter(), root, path)?;
    terminal_psi::find_structural_case(carrier.shape.sum_cases()?, case)?;
    let mut selected = path.to_vec();
    selected.push(terminal_psi::StructuralPathSegment::Case(case));
    fields
        .iter()
        .map(|field| {
            let declaration =
                crate::projected_field_declaration(types.iter(), root, &selected, *field)?;
            (!declaration.relevance.is_erased()).then_some(())?;
            declaration.field_type.scalar_type()
        })
        .collect()
}
