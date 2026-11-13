//! Contract refinement is implication in one positional formal namespace.
//! The constructor and receiving verifier derive the same ordered goals.

use std::collections::BTreeMap;

use semantic_vocabulary::{
    CallableContractId, CrashCause, MachineId, PlaceId, Proposition, PropositionId, ScalarTerm,
    ValueId,
};
use terminal_psi::{
    CrashRouteGuard, EvidenceArgumentSubject, EvidenceCallableOwner, EvidenceContractLane,
    EvidenceContractLaneKind, TerminalMachineResult, TerminalModule,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CallableRefinementGoalKind {
    Input,
    NormalOutput,
    GuardedOutput {
        guard: terminal_psi::OutcomeSpecificGuard,
        position: u32,
    },
    Crash {
        cause: CrashCause,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableRefinementGoal {
    pub kind: CallableRefinementGoalKind,
    pub proposition: Proposition,
}

fn conjunction(values: impl IntoIterator<Item = Proposition>) -> Proposition {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort();
    values.dedup();
    match values.len() {
        0 => Proposition::Truth,
        1 => values.pop().expect("one conjunct"),
        _ => Proposition::Conjunction(values),
    }
}

fn disjunction(values: impl IntoIterator<Item = Proposition>) -> Proposition {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort();
    values.dedup();
    match values.len() {
        0 => Proposition::Falsehood,
        1 => values.pop().expect("one disjunct"),
        _ => Proposition::Disjunction(values),
    }
}

fn implication(premise: Proposition, conclusion: Proposition) -> Proposition {
    Proposition::Implication {
        premise: Box::new(premise),
        conclusion: Box::new(conclusion),
    }
}

fn remap_atoms(value: &Proposition, atoms: &BTreeMap<PropositionId, PropositionId>) -> Proposition {
    match value {
        Proposition::Atom(id) => Proposition::Atom(atoms.get(id).copied().unwrap_or(*id)),
        Proposition::Conjunction(values) => Proposition::Conjunction(
            values
                .iter()
                .map(|value| remap_atoms(value, atoms))
                .collect(),
        ),
        Proposition::Disjunction(values) => Proposition::Disjunction(
            values
                .iter()
                .map(|value| remap_atoms(value, atoms))
                .collect(),
        ),
        Proposition::Implication {
            premise,
            conclusion,
        } => implication(remap_atoms(premise, atoms), remap_atoms(conclusion, atoms)),
        _ => value.clone(),
    }
}

fn lane_proposition(
    module: &TerminalModule,
    lane: &EvidenceContractLane,
) -> Result<PropositionId, &'static str> {
    let mut terms = module
        .evidence_terms
        .iter()
        .filter(|term| term.id == lane.term);
    let term = terms.next().ok_or("refinement evidence lane has no term")?;
    if terms.next().is_some() {
        return Err("refinement evidence lane term is ambiguous");
    }
    Ok(term.proposition)
}

fn same_evidence_meaning(
    module: &TerminalModule,
    left: &EvidenceContractLane,
    right: &EvidenceContractLane,
) -> Result<bool, &'static str> {
    if left.argument_subjects != right.argument_subjects {
        return Ok(false);
    }
    let term = |lane: &EvidenceContractLane| {
        module
            .evidence_terms
            .iter()
            .find(|term| term.id == lane.term)
            .ok_or("refinement evidence term is absent")
    };
    let left_term = term(left)?;
    let right_term = term(right)?;
    if left_term.interface != right_term.interface {
        return Ok(false);
    }
    let application = |id| {
        module
            .proposition_applications
            .iter()
            .find(|row| row.id == id)
            .ok_or("refinement evidence application is absent")
    };
    let left_application = application(left_term.proposition)?;
    let right_application = application(right_term.proposition)?;
    if left_application.declaration != right_application.declaration
        || left_application.binder_arguments != right_application.binder_arguments
        || left_application.evidence_interface != right_application.evidence_interface
        || left_application.arguments.len() != right_application.arguments.len()
        || left_application.arguments.len() != left.argument_subjects.len()
    {
        return Ok(false);
    }
    for (position, subject) in left.argument_subjects.iter().enumerate() {
        if subject.argument_position() as usize != position {
            return Err("refinement evidence subjects are not positional");
        }
        match subject {
            EvidenceArgumentSubject::Unknown { .. } => {
                return Err("refinement evidence has an unresolved subject");
            }
            EvidenceArgumentSubject::Closed { .. }
                if left_application.arguments[position]
                    != right_application.arguments[position] =>
            {
                return Ok(false);
            }
            _ => {}
        }
    }
    Ok(true)
}

/// The catalog retains the exact required result formal; it is never inferred
/// from predicates or from the selected machine body.
/// Physical qualifications/effects use their ordinary admission owners; these
/// goals retain the complete logical input, normal, guarded and crash contract.
/// A published signature, independently of whether it has an executable body.
/// A boundary/native header carries no invented machine or evidence identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallableCrashPolicy {
    Closed,
    Open,
}

#[derive(Clone, Copy)]
pub struct CallableContractView<'a> {
    pub parameter_order: &'a [terminal_psi::BoundaryParameterKind],
    pub scalar_parameters: &'a [terminal_psi::ValueDeclaration],
    pub structural_parameters: &'a [terminal_psi::StructuralParameterDeclaration],
    pub result: &'a TerminalMachineResult,
    pub proof: &'a terminal_psi::MachineContract,
    pub evidence_owner: Option<EvidenceCallableOwner>,
    pub crash_policy: CallableCrashPolicy,
}

pub fn callable_contract_refinement_goals(
    module: &TerminalModule,
    required: CallableContractId,
    selected: MachineId,
) -> Result<Vec<CallableRefinementGoal>, &'static str> {
    let required = module
        .callables
        .contract(required)
        .ok_or("refinement has no required callable contract")?;
    let selected = module
        .machine(selected)
        .ok_or("refinement has no selected machine")?;
    if !required
        .result
        .matches_machine_result(&required.proof_result)
    {
        return Err("refinement required result differs from its retained proof formal");
    }
    contract_refinement_goals(
        module,
        CallableContractView {
            parameter_order: &required.parameter_order,
            scalar_parameters: &required.scalar_parameters,
            structural_parameters: &required.structural_parameters,
            result: &required.proof_result,
            proof: &required.proof,
            evidence_owner: Some(EvidenceCallableOwner::Runtime(required.id)),
            crash_policy: CallableCrashPolicy::Closed,
        },
        CallableContractView {
            parameter_order: &selected.parameter_order,
            scalar_parameters: &selected.parameters,
            structural_parameters: &selected.structural_parameters,
            result: &selected.result,
            proof: &selected.contract,
            evidence_owner: Some(EvidenceCallableOwner::Machine(selected.id)),
            crash_policy: CallableCrashPolicy::Closed,
        },
    )
}

/// Project the complete logical refinement into the required signature's
/// positional namespace. Both views are genuine published headers; selection
/// and imported-behavior admission remain separate operations.
pub fn contract_refinement_goals(
    module: &TerminalModule,
    required: CallableContractView<'_>,
    selected: CallableContractView<'_>,
) -> Result<Vec<CallableRefinementGoal>, &'static str> {
    if selected.crash_policy != CallableCrashPolicy::Closed {
        return Err("supplied implementation has no closed published crash contract");
    }
    let required_result = required.result;
    if required.parameter_order != selected.parameter_order
        || required.scalar_parameters.len() != selected.scalar_parameters.len()
        || required.structural_parameters.len() != selected.structural_parameters.len()
        || required.proof.erased_scalar_formals.len() != selected.proof.erased_scalar_formals.len()
        || required.proof.erased_proof_formals != selected.proof.erased_proof_formals
        || !match (required.result, selected.result) {
            (TerminalMachineResult::Unit, TerminalMachineResult::Unit) => true,
            (TerminalMachineResult::Scalar(expected), TerminalMachineResult::Scalar(actual)) => {
                expected.scalar_type == actual.scalar_type
            }
            (
                TerminalMachineResult::Structural(expected),
                TerminalMachineResult::Structural(actual),
            ) => {
                expected.structural_type == actual.structural_type
                    && expected.multiplicity == actual.multiplicity
            }
            _ => false,
        }
    {
        return Err("refinement callable formal or result shape differs");
    }
    let mut values = BTreeMap::<ValueId, ScalarTerm>::new();
    for (source, target) in selected
        .scalar_parameters
        .iter()
        .zip(required.scalar_parameters)
        .chain(
            selected
                .proof
                .erased_scalar_formals
                .iter()
                .zip(&required.proof.erased_scalar_formals),
        )
    {
        if source.scalar_type != target.scalar_type
            || values
                .insert(
                    source.id,
                    ScalarTerm::Value {
                        id: target.id,
                        scalar_type: target.scalar_type,
                    },
                )
                .is_some()
        {
            return Err("refinement scalar formal has another type or repeated identity");
        }
    }
    let mut places = BTreeMap::<
        PlaceId,
        (
            PlaceId,
            Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        ),
    >::new();
    for (source, target) in selected
        .structural_parameters
        .iter()
        .zip(required.structural_parameters)
    {
        if source.position != target.position
            || source.structural_type != target.structural_type
            || source.access != target.access
            || source.multiplicity != target.multiplicity
            || places
                .insert(source.place, (target.place, Vec::new()))
                .is_some()
        {
            return Err("refinement structural formal has another carrier, access or identity");
        }
    }
    match (selected.result, required_result) {
        (TerminalMachineResult::Unit, TerminalMachineResult::Unit) => {}
        (TerminalMachineResult::Scalar(source), TerminalMachineResult::Scalar(target)) => {
            if source.scalar_type != target.scalar_type
                || values
                    .insert(
                        source.id,
                        ScalarTerm::Value {
                            id: target.id,
                            scalar_type: target.scalar_type,
                        },
                    )
                    .is_some()
            {
                return Err("refinement scalar result aliases an input or has another type");
            }
        }
        (TerminalMachineResult::Structural(source), TerminalMachineResult::Structural(target)) => {
            if places
                .insert(source.place, (target.place, Vec::new()))
                .is_some()
            {
                return Err("refinement structural result aliases an input");
            }
        }
        _ => return Err("refinement result formal has another role"),
    }
    let required_owner = required.evidence_owner;
    let selected_owner = selected.evidence_owner;
    let required_lanes = module
        .evidence_contract_lanes
        .iter()
        .filter(|lane| Some(lane.owner) == required_owner)
        .collect::<Vec<_>>();
    let selected_lanes = module
        .evidence_contract_lanes
        .iter()
        .filter(|lane| Some(lane.owner) == selected_owner)
        .collect::<Vec<_>>();
    let mut atoms = BTreeMap::new();
    for source in &selected_lanes {
        for target in &required_lanes {
            if same_evidence_meaning(module, source, target)? {
                let source = lane_proposition(module, source)?;
                let target = lane_proposition(module, target)?;
                if atoms
                    .insert(source, target)
                    .is_some_and(|previous| previous != target)
                {
                    return Err("refinement evidence correspondence is ambiguous");
                }
            }
        }
    }
    let context = semantic_vocabulary::PropositionContext::from_value_types_and_places(
        selected
            .scalar_parameters
            .iter()
            .chain(&selected.proof.erased_scalar_formals)
            .chain(selected.result.scalar_ref())
            .map(|value| (value.id, value.scalar_type)),
        selected
            .structural_parameters
            .iter()
            .map(|parameter| {
                (
                    parameter.place,
                    semantic_vocabulary::StructuralPlaceKind::Parameter {
                        position: parameter.position,
                    },
                )
            })
            .chain(selected.result.structural().map(|result| {
                (
                    result.place,
                    semantic_vocabulary::StructuralPlaceKind::Result,
                )
            })),
    )
    .map_err(|_| "selected refinement namespace has conflicting declarations")?;
    let substitute = |proposition: &Proposition| {
        context.validate(proposition).map_err(
            |_| "selected refinement predicate escapes its exact formal/result namespace",
        )?;
        Ok(remap_atoms(
            &crate::substitution::substitute_proposition_structural_places(
                &crate::substitution::substitute_proposition_values(proposition, &values),
                &places,
            ),
            &atoms,
        ))
    };
    let evidence =
        |lanes: &[&EvidenceContractLane], kind| -> Result<Vec<Proposition>, &'static str> {
            lanes
                .iter()
                .filter(|lane| lane.kind == kind)
                .map(|lane| lane_proposition(module, lane).map(Proposition::Atom))
                .collect()
        };
    let required_inputs = required
        .proof
        .source_checked_requires
        .iter()
        .cloned()
        .chain(required.proof.requires.iter().cloned())
        .chain(evidence(
            &required_lanes,
            EvidenceContractLaneKind::Requires,
        )?)
        .collect::<Vec<_>>();
    let selected_inputs = selected
        .proof
        .source_checked_requires
        .iter()
        .chain(&selected.proof.requires)
        .map(substitute)
        .chain(
            evidence(&selected_lanes, EvidenceContractLaneKind::Requires)?
                .iter()
                .map(substitute),
        )
        .collect::<Result<Vec<_>, _>>()?;
    let selected_outputs = selected
        .proof
        .ensures
        .iter()
        .map(|clause| substitute(&clause.proposition))
        .chain(
            evidence(&selected_lanes, EvidenceContractLaneKind::Ensures)?
                .iter()
                .map(substitute),
        )
        .collect::<Result<Vec<_>, _>>()?;
    let required_outputs = required
        .proof
        .ensures
        .iter()
        .map(|clause| clause.proposition.clone())
        .chain(evidence(
            &required_lanes,
            EvidenceContractLaneKind::Ensures,
        )?);
    let normal_premise = conjunction(required_inputs.iter().cloned().chain(selected_outputs));
    let mut goals = vec![
        CallableRefinementGoal {
            kind: CallableRefinementGoalKind::Input,
            proposition: implication(
                conjunction(required_inputs.iter().cloned()),
                conjunction(selected_inputs),
            ),
        },
        CallableRefinementGoal {
            kind: CallableRefinementGoalKind::NormalOutput,
            proposition: implication(normal_premise.clone(), conjunction(required_outputs)),
        },
    ];
    for guarantee in &required.proof.outcome_specific_ensures {
        let selected_guarantees = selected
            .proof
            .outcome_specific_ensures
            .iter()
            .filter(|candidate| candidate.guard == guarantee.guard)
            .map(|candidate| substitute(&candidate.proposition))
            .collect::<Result<Vec<_>, _>>()?;
        goals.push(CallableRefinementGoal {
            kind: CallableRefinementGoalKind::GuardedOutput {
                guard: guarantee.guard,
                position: guarantee.position,
            },
            proposition: implication(
                conjunction(std::iter::once(normal_premise.clone()).chain(selected_guarantees)),
                guarantee.proposition.clone(),
            ),
        });
    }
    if required.crash_policy == CallableCrashPolicy::Closed {
        let mut causes = selected
            .proof
            .crash_routes
            .iter()
            .map(|bucket| bucket.cause)
            .collect::<Vec<_>>();
        causes.sort();
        causes.dedup();
        for cause in causes {
            let guards = |routes: &[terminal_psi::CrashRouteBucket]| {
                routes
                    .iter()
                    .filter(|bucket| bucket.cause == cause)
                    .flat_map(|bucket| bucket.alternatives.iter())
                    .map(|guard| match guard {
                        CrashRouteGuard::Truth => Proposition::Truth,
                        CrashRouteGuard::Predicate(predicate) => predicate.proposition().clone(),
                    })
                    .collect::<Vec<_>>()
            };
            let selected_guards = guards(&selected.proof.crash_routes)
                .iter()
                .map(substitute)
                .collect::<Result<Vec<_>, _>>()?;
            goals.push(CallableRefinementGoal {
                kind: CallableRefinementGoalKind::Crash { cause },
                proposition: implication(
                    conjunction(
                        required_inputs
                            .iter()
                            .cloned()
                            .chain(std::iter::once(disjunction(selected_guards))),
                    ),
                    disjunction(guards(&required.proof.crash_routes)),
                ),
            });
        }
    }
    goals.sort_by(|left, right| left.kind.cmp(&right.kind));
    if goals.windows(2).any(|pair| pair[0].kind == pair[1].kind) {
        return Err("refinement repeats a contract goal coordinate");
    }
    Ok(goals)
}

pub fn callable_refinement_context(
    module: &TerminalModule,
    contract: CallableContractId,
) -> Result<semantic_vocabulary::PropositionContext, &'static str> {
    let contract = module
        .callables
        .contract(contract)
        .ok_or("refinement context has no callable contract")?;
    contract_refinement_context(CallableContractView {
        parameter_order: &contract.parameter_order,
        scalar_parameters: &contract.scalar_parameters,
        structural_parameters: &contract.structural_parameters,
        result: &contract.proof_result,
        proof: &contract.proof,
        evidence_owner: Some(EvidenceCallableOwner::Runtime(contract.id)),
        crash_policy: CallableCrashPolicy::Closed,
    })
}

pub fn contract_refinement_context(
    contract: CallableContractView<'_>,
) -> Result<semantic_vocabulary::PropositionContext, &'static str> {
    semantic_vocabulary::PropositionContext::from_value_types_and_places(
        contract
            .scalar_parameters
            .iter()
            .chain(&contract.proof.erased_scalar_formals)
            .chain(contract.result.scalar_ref())
            .map(|value| (value.id, value.scalar_type)),
        contract
            .structural_parameters
            .iter()
            .map(|parameter| {
                (
                    parameter.place,
                    semantic_vocabulary::StructuralPlaceKind::Parameter {
                        position: parameter.position,
                    },
                )
            })
            .chain(contract.result.structural().map(|result| {
                (
                    result.place,
                    semantic_vocabulary::StructuralPlaceKind::Result,
                )
            })),
    )
    .map_err(|_| "runtime callable refinement context has conflicting declarations")
}

/// Retained IDs name these exact reconstructed questions in this order.
pub fn descriptor_refinement_goals(
    module: &TerminalModule,
    descriptor: semantic_vocabulary::MachineDescriptorId,
) -> Result<Vec<(semantic_vocabulary::ObligationId, CallableRefinementGoal)>, &'static str> {
    let descriptor = module
        .callables
        .descriptor(descriptor)
        .ok_or("refinement descriptor is absent")?;
    let goals =
        callable_contract_refinement_goals(module, descriptor.contract, descriptor.machine)?;
    if descriptor.refinement_obligations.len() != goals.len() {
        return Err("descriptor refinement IDs do not cover its complete contract");
    }
    let mut ids = descriptor.refinement_obligations.clone();
    ids.sort();
    ids.dedup();
    if ids.len() != goals.len() {
        return Err("descriptor refinement repeats an obligation identity");
    }
    Ok(descriptor
        .refinement_obligations
        .iter()
        .copied()
        .zip(goals)
        .collect())
}

/// Receiving admission reconstructs every goal from the current catalog and
/// checks its certificate in the ordinary kernel, with no ambient assumptions.
pub fn validate_descriptor_refinement_evidence(
    module: &TerminalModule,
) -> Result<Vec<proof_admission::AcceptedFact>, &'static str> {
    validate_callable_evidence_contracts_in(
        &module.callables,
        &module.proposition_declarations,
        &module.proposition_applications,
        &module.evidence_terms,
        &module.evidence_contract_lanes,
    )?;
    let mut accepted = Vec::new();
    let mut retained_ids = std::collections::BTreeSet::new();
    for descriptor in &module.callables.descriptors {
        let goals = descriptor_refinement_goals(module, descriptor.id)?;
        if goals.len() != descriptor.refinement_evidence.len() {
            return Err("descriptor refinement has missing or extra proof certificates");
        }
        let context = callable_refinement_context(module, descriptor.contract)?;
        for ((id, goal), certificate) in goals.into_iter().zip(&descriptor.refinement_evidence) {
            if !retained_ids.insert(id) {
                return Err("descriptor refinements alias an obligation identity");
            }
            accepted.push(proof_admission::verify_obligation(&context,
                &proof_admission::Obligation { id, proposition: goal.proposition, class: proof_admission::ObligationClass::Derivable },
                &[], &[], proof_admission::EvidenceRoute::CertificateDerived(certificate.clone()))
                .map_err(|_| "descriptor contract refinement certificate does not establish its reconstructed goal")?);
        }
    }
    Ok(accepted)
}

/// The supplied lane table must rejoin the complete public contract exactly.
/// All consumers, including retained native proof replay, use this operation.
pub fn validate_callable_evidence_contracts_in(
    catalog: &terminal_psi::TerminalCallableCatalog,
    declarations: &[terminal_psi::PropositionDeclaration],
    applications: &[terminal_psi::PropositionApplicationIdentity],
    terms: &[terminal_psi::EvidenceTermDeclaration],
    lanes: &[EvidenceContractLane],
) -> Result<(), &'static str> {
    for contract in &catalog.contracts {
        let mut actual = lanes
            .iter()
            .filter(|lane| lane.owner == EvidenceCallableOwner::Runtime(contract.id))
            .collect::<Vec<_>>();
        actual.sort_by_key(|lane| (lane.kind, lane.position));
        if actual.len() != contract.evidence_contract.len() {
            return Err("runtime callable evidence roster has missing or extra lanes");
        }
        let mut previous: Option<(EvidenceContractLaneKind, u32)> = None;
        for (expected, actual) in contract.evidence_contract.iter().zip(actual) {
            let key = (expected.kind, expected.position);
            let next = match previous {
                Some((kind, position)) if kind == expected.kind => position.checked_add(1),
                _ => Some(0),
            };
            if next != Some(expected.position) || previous.is_some_and(|previous| previous >= key) {
                return Err(
                    "runtime callable evidence contract lanes are repeated, unordered or incomplete",
                );
            }
            previous = Some(key);
            let expected_field = (expected.kind == EvidenceContractLaneKind::Ensures)
                .then(|| format!("${}", expected.position));
            if (actual.kind, actual.position, actual.term)
                != (expected.kind, expected.position, expected.term.id)
                || actual.output_field != expected_field
                || actual.argument_subjects != expected.argument_subjects
                || expected.term.proposition != expected.application.id
                || expected.application.declaration != expected.declaration.id
                || expected.application.evidence_interface.as_ref()
                    != Some(&expected.term.interface)
                || expected.application.arguments.len() != expected.argument_subjects.len()
                || expected
                    .argument_subjects
                    .iter()
                    .enumerate()
                    .any(|(position, subject)| {
                        subject.argument_position() as usize != position
                            || matches!(subject, EvidenceArgumentSubject::Unknown { .. })
                    })
            {
                return Err("runtime callable evidence lane changed its full public meaning");
            }
            let mut declaration = declarations
                .iter()
                .filter(|row| row.id == expected.declaration.id);
            if declaration.next() != Some(&expected.declaration) || declaration.next().is_some() {
                return Err(
                    "runtime callable evidence proposition declaration is missing, ambiguous or changed",
                );
            }
            let mut application = applications
                .iter()
                .filter(|row| row.id == expected.application.id);
            if application.next() != Some(&expected.application) || application.next().is_some() {
                return Err(
                    "runtime callable evidence application is missing, ambiguous or changed",
                );
            }
            let mut term = terms.iter().filter(|row| row.id == expected.term.id);
            if term.next() != Some(&expected.term) || term.next().is_some() {
                return Err(
                    "runtime callable evidence witness interface is missing, ambiguous or changed",
                );
            }
        }
    }
    if lanes.iter().any(|lane| matches!(lane.owner, EvidenceCallableOwner::Runtime(contract) if catalog.contract(contract).is_none())) {
        return Err("runtime evidence lane has no exact callable owner");
    }
    Ok(())
}
