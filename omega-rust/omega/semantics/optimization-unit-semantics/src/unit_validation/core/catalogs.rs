//! Active/pruned machine rosters and structural/service catalog validation.

use super::*;

pub(super) struct UnitIndexes<'a> {
    pub(super) machines: BTreeMap<MachineId, &'a PsiOptimizationFunction>,
    pub(super) boundary_machines:
        BTreeMap<BoundaryMachineId, &'a terminal_psi::BoundaryMachineDeclaration>,
    pub(super) services: BTreeMap<ServiceId, &'a terminal_psi::ServiceDeclaration>,
    pub(super) pruned: BTreeSet<MachineId>,
}

pub(super) fn index_and_validate_unit_catalogs<'unit>(
    unit: &'unit PsiOptimizationUnit,
    cycle_policy: &function_structure::ControlCyclePolicy,
) -> Result<UnitIndexes<'unit>, OptimizationUnitValidationError> {
    let mut machines = BTreeMap::new();
    for function in &unit.functions {
        if machines.insert(function.machine, function).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateMachine(
                function.machine,
            ));
        }
    }
    if unit
        .pruned_machines
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(OptimizationUnitValidationError::NonCanonicalPrunedMachineRoster);
    }
    let pruned = unit
        .pruned_machines
        .iter()
        .map(|custody| custody.machine)
        .collect::<BTreeSet<_>>();
    if pruned.len() != unit.pruned_machines.len() {
        return Err(OptimizationUnitValidationError::NonCanonicalPrunedMachineRoster);
    }
    if let Some(machine) = machines
        .keys()
        .find(|machine| pruned.contains(machine))
        .copied()
    {
        return Err(OptimizationUnitValidationError::ActivePrunedMachineOverlap(
            machine,
        ));
    }
    if pruned.contains(&unit.entry) {
        return Err(OptimizationUnitValidationError::PrunedEntryMachine(
            unit.entry,
        ));
    }
    if let Some(machine) = unit
        .provider_candidates
        .iter()
        .map(|candidate| candidate.candidate)
        .find(|machine| pruned.contains(machine))
    {
        return Err(OptimizationUnitValidationError::PrunedProviderMachine(
            machine,
        ));
    }
    if unit
        .accepted_obligation_facts
        .iter()
        .any(|fact| !machines.contains_key(&fact.machine) && !pruned.contains(&fact.machine))
    {
        return Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch);
    }
    if unit.proof_questions.iter().any(|question| {
        let machine = question.owner.machine();
        !machines.contains_key(&machine) && !pruned.contains(&machine)
    }) {
        return Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch);
    }
    let mut boundary_machines = BTreeMap::new();
    for boundary in &unit.boundary_machines {
        if boundary_machines.insert(boundary.id, boundary).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateBoundaryMachine(
                boundary.id,
            ));
        }
    }
    let services = index_service_catalog(unit)?;
    let (structural_types, structural_domains) = index_structural_catalogs(unit)?;
    for boundary in &unit.boundary_machines {
        if !valid_service_ceiling(&boundary.published_service_ceiling, &services) {
            return Err(
                OptimizationUnitValidationError::InvalidBoundaryServiceCeiling(boundary.id),
            );
        }
        if !boundary_structural_signature_matches(boundary, &structural_types, &structural_domains)
        {
            return Err(OptimizationUnitValidationError::StructuralCatalogMismatch {
                machine: None,
            });
        }
    }
    validate_provider_service_refinements(unit, &machines, &boundary_machines)?;
    for function in &unit.functions {
        validate_function(
            function,
            unit.entry,
            &machines,
            &boundary_machines,
            &services,
            &structural_types,
            &structural_domains,
            cycle_policy,
        )?;
        validate_scalar_case_range_authority(unit, function, &structural_types)?;
    }
    Ok(UnitIndexes {
        machines,
        boundary_machines,
        services,
        pruned,
    })
}

fn validate_scalar_case_range_authority(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    use semantic_vocabulary::{Proposition, ScalarTerm};
    for node in function.blocks.iter().flat_map(|block| &block.nodes) {
        let abstract_operations::AbstractOperation::EstablishScalarCase { psi_operation, result, result_case, fields } = &node.operation else { continue; };
        let Some(terminal_psi::StructuralTypeDeclaration { shape: terminal_psi::StructuralTypeShape::Sum { cases }, .. }) = types.get(&result.structural_type).copied() else { continue; };
        let Some(case) = cases.iter().find(|case| case.id == *result_case) else { continue; };
        for (declaration, binding) in case.fields.iter().zip(fields) {
            let terminal_psi::StructuralFieldType::BoundedInteger(bounds) = declaration.field_type else { continue; };
            let integer_type = bounds.integer_type();
            let value = ScalarTerm::value(binding.value, ScalarType::Integer(integer_type));
            let endpoint = |value| ScalarTerm::Integer { scalar_type: integer_type, value };
            let mut clauses = vec![Proposition::LessOrEqual(endpoint(bounds.minimum()), value.clone()), Proposition::LessOrEqual(value, endpoint(bounds.maximum()))];
            clauses.sort();
            let proposition = terminal_codec::canonical_proposition_order_key(&Proposition::Conjunction(clauses))
                .map_err(|_| OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)?;
            if unit.accepted_obligation_facts.iter().filter(|fact| {
                fact.machine == function.machine && fact.operation == *psi_operation
                    && Some(fact.obligation) == binding.range_obligation
                    && fact.proposition == proposition && fact.psi == unit.psi && fact.has_canonical_identity()
            }).count() != 1 {
                return Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch);
            }
        }
    }
    Ok(())
}

pub(super) fn validate_final_authorities(
    unit: &PsiOptimizationUnit,
    indexes: &UnitIndexes<'_>,
) -> Result<(), OptimizationUnitValidationError> {
    let UnitIndexes {
        machines,
        boundary_machines,
        services,
        pruned,
    } = indexes;
    for fact in &unit.ownership_frontier_facts {
        if unit
            .functions
            .iter()
            .find(|function| function.machine == fact.machine)
            .is_none()
            && !pruned.contains(&fact.machine)
        {
            return Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch);
        }
    }
    if !machines.contains_key(&unit.entry) {
        return Err(OptimizationUnitValidationError::MissingEntryMachine(
            unit.entry,
        ));
    }
    validate_root_service_reach(unit, machines, boundary_machines, services)?;
    Ok(())
}
