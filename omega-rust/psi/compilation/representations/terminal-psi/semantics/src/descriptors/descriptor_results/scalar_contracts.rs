//! Normal-return contracts bind to completed scalars and current payload storage.
//! Storage equalities become typed observations, never timeless scalar premises:
//! the payload owner resolves backing and invalidates them after mutation.
//! Call adapters supply exact typed formal/result bindings from the current
//! checked signature, never implementation-private facts or receiving domains.
use crate::runtime_structural_path_tip;
use crate::substitution;
use crate::substitution::substitute_proposition_values;
use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};
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
            bound.then(|| substitute_proposition_values(&proposition, &substitutions))
        })
        .collect()
}

pub(super) struct TerminalPostconditions {
    pub scalar: Vec<Proposition>,
    pub stored: Vec<(super::DescriptorLocation, ScalarTerm)>,
}

/// An actual/formal binding is a coordinate, not a postcondition. Formal
/// qualifications remain entry preconditions. Keep owned and unresolved actuals
/// explicit so missing coordinates cannot be mistaken for absent duties.
pub(super) struct TerminalCallParameterBinding {
    pub position: usize,
    pub formal: semantic_vocabulary::PlaceId,
    pub carrier: semantic_vocabulary::StructuralTypeId,
    pub access: terminal_psi::StructuralAccess,
    pub actual: terminal_psi::StructuralArgument,
    pub root_type: semantic_vocabulary::StructuralTypeId,
    pub location: Option<super::DescriptorLocation>,
    pub contents: Option<super::DescriptorContentsQuery>,
}

/// Keep the actual carrier, access and current coordinate join common to
/// direct-body summaries and public normal contracts. This binds no fact.
pub(super) fn terminal_structural_argument_binding(
    types: &[terminal_psi::StructuralTypeDeclaration],
    machine: &terminal_psi::TerminalMachine,
    formal: &terminal_psi::StructuralParameterDeclaration,
    actual: &terminal_psi::StructuralArgument,
    position: usize,
) -> Result<TerminalCallParameterBinding, &'static str> {
    let root_type = machine
        .structural_place_type(actual.place)
        .ok_or("a structural actual has no unique current root carrier")?;
    let selected = runtime_structural_path_tip(types.iter(), root_type, &actual.path);
    if actual.access != formal.access
        || (!selected.is_some_and(|selected| selected.id == formal.structural_type)
            && crate::borrowed_primitive_projection_declaration(
                types.iter(),
                root_type,
                &actual.path,
                Some(formal.structural_type),
            ) != Some(formal.structural_type))
    {
        return Err("a structural actual disagrees with its formal access or carrier");
    }
    Ok(TerminalCallParameterBinding {
        position,
        formal: formal.place,
        carrier: formal.structural_type,
        access: formal.access,
        actual: actual.clone(),
        root_type,
        location: crate::descriptors::descriptor_lifetimes::descriptor_location(
            types,
            actual.place,
            root_type,
            &actual.path,
            None,
        ),
        contents: super::contents::descriptor_contents_query(
            types,
            actual.place,
            root_type,
            &actual.path,
            None,
        ),
    })
}

pub(super) struct TerminalCallBindings<'module> {
    pub callee: &'module terminal_psi::TerminalMachine,
    pub parameters: Vec<TerminalCallParameterBinding>,
    scalar_bindings: Vec<(ValueId, ScalarTerm)>,
    substitutions: BTreeMap<ValueId, ScalarTerm>,
    storage: Vec<(
        semantic_vocabulary::PlaceId,
        semantic_vocabulary::StructuralTypeId,
        super::DescriptorLocation,
    )>,
}

impl TerminalCallBindings<'_> {
    pub(super) fn scalar_substitutions(&self) -> &BTreeMap<ValueId, ScalarTerm> {
        &self.substitutions
    }
}

/// Bind a performed direct call once. Scalar ensures and normal-exit payload
/// summaries share these type/access/tuple/result checks. Selecting a public
/// body for effects is not permission to export its private postconditions.
pub(super) fn terminal_call_bindings<'module>(
    module: &'module terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    operation: &terminal_psi::Operation,
    scalar_expressions: &super::payload_predicates::TerminalScalarExpressions<'_>,
) -> Result<TerminalCallBindings<'module>, &'static str> {
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
        _ => return Err("the operation is not an admitted direct call kind"),
    };
    let mut matching = module
        .machines
        .iter()
        .filter(|machine| machine.id == callee);
    let callee = matching.next().ok_or("the called machine is absent")?;
    if matching.next().is_some()
        || arguments.len() != callee.parameters.len()
        || structural_arguments.len() != callee.structural_parameters.len()
        || !callee.contract.erased_scalar_formals.is_empty()
        || !callee.contract.erased_proof_formals.is_empty()
    {
        return Err("the call signature, actual rosters or erased formals disagree");
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
            return Err("a scalar actual disagrees with its formal type");
        }
        bindings.push((
            formal.id,
            scalar_expressions
                .expression(*actual)
                .ok_or("a scalar actual has no current expression binding")?,
        ));
    }
    let mut storage = Vec::new();
    let mut parameters = Vec::new();
    for (position, (formal, actual)) in callee
        .structural_parameters
        .iter()
        .zip(structural_arguments)
        .enumerate()
    {
        let binding = terminal_structural_argument_binding(
            &module.structural_types,
            machine,
            formal,
            actual,
            position,
        )?;
        let location = binding.location.clone();
        parameters.push(binding);
        // An owned input can be copied. Keep its position above, but never
        // map its callee post-state onto the caller's original storage.
        if actual.access == StructuralAccess::Owned {
            continue;
        }
        if let Some(location) = location {
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
        _ => return Err("the performed call result disagrees with its signature"),
    }
    let substitutions = bindings.iter().cloned().collect::<BTreeMap<_, _>>();
    if substitutions.len() != bindings.len() {
        return Err("the call has ambiguous scalar substitutions");
    }
    Ok(TerminalCallBindings {
        callee,
        parameters,
        scalar_bindings: bindings,
        substitutions,
        storage,
    })
}

pub(super) fn terminal_postconditions(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    operation: &terminal_psi::Operation,
    scalar_expressions: &super::payload_predicates::TerminalScalarExpressions<'_>,
) -> Option<TerminalPostconditions> {
    let TerminalCallBindings {
        callee,
        scalar_bindings: bindings,
        substitutions,
        storage,
        ..
    } = terminal_call_bindings(module, machine, operation, scalar_expressions).ok()?;
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
