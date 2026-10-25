//! Normal-return contracts bind to completed scalars and current payload storage.
//! Storage equalities become typed observations, never timeless scalar premises:
//! the payload owner resolves backing and invalidates them after mutation.
//! Call adapters supply exact typed formal/result bindings from the current
//! checked signature, never implementation-private facts or receiving domains.
use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};
use std::borrow::Borrow;
use std::collections::BTreeMap;

pub(super) fn scalar_conjuncts(propositions: &[Proposition]) -> Vec<Proposition> {
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

/// Rebind facts simultaneously. A retired incoming value may only survive as
/// the new formal receiving it; numeric SSA reuse never equates two iterations.
pub(super) fn arrival_requirements<Requirement: Borrow<Proposition>>(
    requirements: &[Requirement],
    bindings: &[(ValueId, ScalarTerm)],
    parameters: &[ValueId],
) -> Vec<Proposition> {
    let mut substitutions = BTreeMap::new();
    for (parameter, actual) in bindings {
        if let ScalarTerm::Value { id, scalar_type } = actual {
            substitutions
                .entry(*id)
                .or_insert_with(|| ScalarTerm::value(*parameter, *scalar_type));
        }
    }
    let surviving = |value| !parameters.contains(&value) || substitutions.contains_key(&value);
    let mut result = requirements
        .iter()
        .map(Borrow::<Proposition>::borrow)
        .filter(|premise| !premise.any_value_id(|value| !surviving(value)))
        .map(|premise| crate::substitution::substitute_proposition_values(premise, &substitutions))
        .collect::<Vec<_>>();
    for (parameter, actual) in bindings {
        if actual.any_value_id(|value| !surviving(value)) {
            continue;
        }
        let value = crate::substitution::substitute_scalar_term_values(actual, &substitutions);
        result.push(Proposition::Equal(
            ScalarTerm::value(*parameter, value.scalar_type()),
            value,
        ));
        // Non-rebound names still denote the same captured incoming value.
        if !actual.any_value_id(|value| parameters.contains(&value)) {
            result.push(Proposition::Equal(
                ScalarTerm::value(*parameter, actual.scalar_type()),
                actual.clone(),
            ));
        }
    }
    result
}

/// Project immutable, fully bound scalar guarantees into the caller namespace.
/// Unsupported or unbound conjuncts supply no evidence; a matching numeric
/// ValueId in the caller is never a substitute for an actual/formal binding.
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
            proposition.visit_value_ids(|value| bound &= substitutions.contains_key(&value));
            bound.then(|| {
                crate::substitution::substitute_proposition_values(&proposition, &substitutions)
            })
        })
        .collect()
}

pub(super) struct TerminalPostconditions {
    pub scalar: Vec<Proposition>,
    pub stored: Vec<(super::DescriptorLocation, ScalarTerm)>,
}

pub(super) fn terminal_postconditions(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    operation: &terminal_psi::Operation,
    scalar_expressions: &super::payload_predicates::TerminalScalarExpressions<'_>,
) -> Option<TerminalPostconditions> {
    use terminal_psi::{OperationKind, OperationResult, StructuralAccess, TerminalMachineResult};
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
    for (formal, actual) in callee
        .structural_parameters
        .iter()
        .zip(structural_arguments)
    {
        let root_type = super::place_type(machine, actual.place)?;
        let selected = crate::runtime_structural_path_tip(
            module.structural_types.iter(),
            root_type,
            &actual.path,
        );
        if actual.access != formal.access
            || (!selected.is_some_and(|selected| selected.id == formal.structural_type)
                && crate::borrowed_primitive_projection_declaration(
                    module.structural_types.iter(),
                    root_type,
                    &actual.path,
                    Some(formal.structural_type),
                ) != Some(formal.structural_type))
        {
            return None;
        }
        // Owned inputs may be copied into the callee. Their post-state cannot
        // describe the caller's original storage, even for equal carriers.
        if actual.access == StructuralAccess::Owned {
            continue;
        }
        if let Some(location) = crate::descriptor_lifetimes::descriptor_location(
            &module.structural_types,
            actual.place,
            root_type,
            &actual.path,
            None,
        ) {
            storage.push((formal.place, formal.structural_type, location));
        }
    }
    match (&callee.result, &operation.result) {
        (TerminalMachineResult::Unit, OperationResult::Unit) => {}
        (TerminalMachineResult::Scalar(signature), OperationResult::Scalar(result))
            if signature.scalar_type == result.scalar_type && !arguments.contains(&result.id) =>
        {
            bindings.push((
                signature.id,
                ScalarTerm::value(result.id, result.scalar_type),
            ));
        }
        (TerminalMachineResult::Structural(signature), OperationResult::Structural(result))
            if signature.structural_type == result.structural_type
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
                | ScalarTerm::BooleanField { root, path } => (*root, path),
                _ => continue,
            };
            if subject.scalar_type() != value.scalar_type() {
                continue;
            }
            // Reuse the scalar proposition owner's storage-free classification
            // and authenticate every RHS value before positional substitution.
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
            let Some(selected) = crate::descriptor_lifetimes::canonical_descriptor_location(
                &module.structural_types,
                root,
                *carrier,
                path,
            ) else {
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
                crate::substitution::substitute_scalar_term_values(value, &substitutions),
            ));
        }
    }
    Some(postconditions)
}

/// A case edge captures these scalar fields directly; it does not transport
/// predecessor SSA arguments. Read the exact selected declaration roster without
/// adding receiving qualifications, scalar equalities or availability evidence.
pub fn case_payload_scalar_types(
    types: &[terminal_psi::StructuralTypeDeclaration],
    root: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    case: semantic_vocabulary::StructuralCaseId,
    fields: &[semantic_vocabulary::StructuralFieldId],
) -> Option<Vec<semantic_vocabulary::ScalarType>> {
    let carrier = crate::runtime_structural_path_tip(types.iter(), root, path)?;
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
