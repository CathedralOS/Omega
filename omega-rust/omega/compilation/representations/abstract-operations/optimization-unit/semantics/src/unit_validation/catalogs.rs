//! Active/pruned machine rosters and structural/service catalog validation.
use crate::OptimizationUnitValidationError;
use crate::unit_validation::function_structure;
use crate::unit_validation::function_structure::validate_function;
use crate::unit_validation::services::{
    boundary_structural_signature_matches, index_service_catalog, valid_service_ceiling,
    validate_provider_service_refinements, validate_root_service_reach,
};
use crate::unit_validation::structural_catalog::index_structural_catalogs;
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, OperationId, ScalarType, ServiceId, StructuralTypeId, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::find_structural_case;

pub(super) struct UnitIndexes<'a> {
    pub(super) machines: BTreeMap<MachineId, &'a PsiOptimizationFunction>,
    pub(super) boundary_machines:
        BTreeMap<BoundaryMachineId, &'a terminal_psi::BoundaryMachineDeclaration>,
    pub(super) services: BTreeMap<ServiceId, &'a terminal_psi::ServiceDeclaration>,
    pub(super) pruned: BTreeSet<MachineId>,
    pub(super) ownership_frontiers: Vec<optimization_unit::OwnershipFrontierFact>,
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
    validate_dynamic_catalog(unit, &machines, &structural_types)?;
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
    let call_accesses = crate::current_ownership::CallAccesses::new(
        &machines,
        boundary_machines.values().copied(),
        &structural_types,
    );
    let mut descriptor_requirements = BTreeMap::new();
    let mut ownership_frontiers = Vec::new();
    for function in &unit.functions {
        validate_function(
            unit,
            function,
            &machines,
            &boundary_machines,
            &services,
            &structural_types,
            &structural_domains,
            cycle_policy,
            &call_accesses,
            descriptor_requirements.entry(function.machine).or_default(),
            &mut ownership_frontiers,
        )?;
        validate_structural_field_range_authority(unit, function, &structural_types)?;
    }
    crate::current_ownership::validate_descriptor_compatibility(unit, &descriptor_requirements)?;
    Ok(UnitIndexes {
        machines,
        boundary_machines,
        services,
        pruned,
        ownership_frontiers,
    })
}

fn validate_dynamic_catalog(
    unit: &PsiOptimizationUnit,
    machines: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let invalid = || OptimizationUnitValidationError::DynamicDispatchCatalogMismatch;
    for (ordinal, interface) in unit.dynamic_dispatch.interfaces.iter().enumerate() {
        if usize::try_from(interface.id).ok() != Some(ordinal)
            || interface.identity == terminal_psi::TerminalIdentityIndex::ABSENT
        {
            return Err(invalid());
        }
        for (ordinal, slot) in interface.slots.iter().enumerate() {
            terminal_semantics::validate_dynamic_reference_regions(types.values().copied(), slot)
                .map_err(
                |reason| OptimizationUnitValidationError::DynamicReferenceContract {
                    interface: interface.id,
                    slot: slot.slot,
                    reason,
                },
            )?;
            if usize::try_from(slot.slot).ok() != Some(ordinal)
                || slot.declaring_trait_identity == terminal_psi::TerminalIdentityIndex::ABSENT
                || slot.requirement_identity == terminal_psi::TerminalIdentityIndex::ABSENT
                || slot
                    .structural_parameters
                    .iter()
                    .any(|formal| !types.contains_key(&formal.structural_type))
            {
                return Err(invalid());
            }
            if slot
                .result_claim_paths
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            {
                return Err(invalid());
            }
            match &slot.result {
                terminal_psi::BoundaryMachineResult::Unit
                | terminal_psi::BoundaryMachineResult::Scalar(_) => {
                    if !slot.result_claim_paths.is_empty() {
                        return Err(invalid());
                    }
                }
                terminal_psi::BoundaryMachineResult::Structural(result) => {
                    if !types.contains_key(&result.structural_type)
                        || match result.multiplicity {
                            terminal_psi::StructuralMultiplicity::Unrestricted => {
                                !slot.result_claim_paths.is_empty()
                            }
                            terminal_psi::StructuralMultiplicity::Linear => {
                                !matches!(slot.result_claim_paths.as_slice(), [path] if path.is_empty())
                            }
                            terminal_psi::StructuralMultiplicity::Affine => {
                                slot.result_claim_paths.iter().any(Vec::is_empty)
                            }
                        }
                    {
                        return Err(invalid());
                    }
                    for (position, path) in slot.result_claim_paths.iter().enumerate() {
                        if path.iter().any(|segment| {
                            !matches!(
                                segment,
                                terminal_psi::StructuralPathSegment::Field(_)
                                    | terminal_psi::StructuralPathSegment::Case(_)
                                    | terminal_psi::StructuralPathSegment::FixedIndex(_)
                            )
                        })
                            || crate::unit_validation::structural_catalog::resolve_structural_path(
                                types,
                                result.structural_type,
                                path,
                            )
                            .is_none()
                            || slot.result_claim_paths[..position].iter().any(|previous| {
                                path.starts_with(previous) || previous.starts_with(path)
                            })
                        {
                            return Err(invalid());
                        }
                    }
                }
            }
            if slot
                .entry_claim_paths
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            {
                return Err(invalid());
            }
            for (claim_position, claim) in slot.entry_claim_paths.iter().enumerate() {
                let Some(formal) = slot
                    .structural_parameters
                    .get(claim.argument_index as usize)
                else {
                    return Err(invalid());
                };
                if formal.access != terminal_psi::StructuralAccess::Owned
                    || formal.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    || (claim.path.is_empty()
                        && formal.multiplicity != terminal_psi::StructuralMultiplicity::Linear)
                    || claim.path.iter().any(|segment| {
                        !matches!(
                            segment,
                            terminal_psi::StructuralPathSegment::Field(_)
                                | terminal_psi::StructuralPathSegment::Case(_)
                                | terminal_psi::StructuralPathSegment::FixedIndex(_)
                        )
                    })
                    || crate::unit_validation::structural_catalog::resolve_structural_path(
                        types,
                        formal.structural_type,
                        &claim.path,
                    )
                    .is_none()
                    || slot.entry_claim_paths[..claim_position]
                        .iter()
                        .any(|previous| {
                            previous.argument_index == claim.argument_index
                                && (previous.path.starts_with(&claim.path)
                                    || claim.path.starts_with(&previous.path))
                        })
                {
                    return Err(invalid());
                }
            }
            // Even a slot without any executable rows must describe complete
            // disposal custody; table absence cannot authorize a malformed shape.
            if slot
                .structural_parameters
                .iter()
                .enumerate()
                .any(|(argument_index, formal)| {
                    formal.access == terminal_psi::StructuralAccess::Owned
                        && formal.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                        && !slot
                            .entry_claim_paths
                            .iter()
                            .any(|claim| claim.argument_index as usize == argument_index)
                })
            {
                return Err(invalid());
            }
        }
    }
    for (ordinal, table) in unit.dynamic_dispatch.tables.iter().enumerate() {
        let interface = unit
            .dynamic_dispatch
            .interface(table.interface)
            .ok_or_else(invalid)?;
        if usize::try_from(table.id).ok() != Some(ordinal)
            || table.implementation_identity == terminal_psi::TerminalIdentityIndex::ABSENT
            || table.rows.len() != interface.slots.len()
        {
            return Err(invalid());
        }
        for (row, slot) in table.rows.iter().zip(&interface.slots) {
            let Some(machine) = row else {
                continue;
            };
            let function = machines.get(machine).ok_or_else(invalid)?;
            let result_matches = match &function.result {
                abstract_operations::AbstractFunctionResult::Unit => slot.result.is_unit(),
                abstract_operations::AbstractFunctionResult::Scalar(result) => {
                    slot.result.scalar() == Some(result.scalar_type)
                }
                abstract_operations::AbstractFunctionResult::Structural(result) => {
                    terminal_semantics::reference_result_refines_regions(
                        types.values().copied(),
                        &function.structural_parameters,
                        result,
                        &slot.result_reference_regions,
                    ) && slot.result.matches_machine_result(
                        &terminal_psi::TerminalMachineResult::Structural(result.clone()),
                    )
                }
            } && terminal_semantics::result_claim_paths_match_returns(
                &slot.result_claim_paths,
                function
                    .block_nodes()
                    .filter_map(|node| match &node.operation {
                        abstract_operations::AbstractOperation::ReturnStructural {
                            returned_claims,
                            ..
                        } => Some(returned_claims.as_slice()),
                        _ => None,
                    }),
            );
            if !result_matches
                || crate::unit_validation::operation_contracts::function_entry_claim_paths(function)
                    .as_ref()
                    != Some(&slot.entry_claim_paths)
                || function.parameters.len() != slot.scalar_parameters.len()
                || !function
                    .parameters
                    .iter()
                    .zip(&slot.scalar_parameters)
                    .all(|(actual, formal)| actual.scalar_type == *formal)
                || function.structural_parameters.len() != slot.structural_parameters.len()
                || !function
                    .structural_parameters
                    .iter()
                    .zip(&slot.structural_parameters)
                    .all(|(actual, formal)| {
                        // Compare the same place-free signature projection
                        // used at the call site. The present requirement
                        // schema exposes empty qualification rosters; reject
                        // stronger private preconditions rather than infer
                        // caller authority from one executable table row.
                        let actual = abstract_operations::StructuralCallFormal::from(actual);
                        let formal = abstract_operations::StructuralCallFormal::from(formal);
                        actual.position == formal.position
                            && actual.structural_type == formal.structural_type
                            && actual.multiplicity == formal.multiplicity
                            && actual.access == formal.access
                            && actual.qualifications == formal.qualifications
                            && actual.projected_qualifications == formal.projected_qualifications
                    })
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

fn validate_structural_field_range_authority(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    use abstract_operations::AbstractOperation as O;
    use terminal_psi::{RecordFieldValue, StructuralTypeShape};
    for node in function.block_nodes() {
        // Edge-owned receivers use the same immutable source-cut catalog as
        // instruction paths. Equal numeric edge/operation IDs are distinct
        // owners, and a bound from another edge cannot authorize this one.
        let paths = node
            .successors
            .iter()
            .flat_map(|edge| {
                edge.structural_bindings
                    .iter()
                    .map(move |binding| (edge.psi_edge, binding.argument.path.as_slice()))
            })
            .chain(
                node.operation
                    .cleanup_rosters()
                    .into_iter()
                    .flat_map(|(edge, actions)| {
                        actions.iter().map(move |action| (edge, action.path()))
                    }),
            );
        for (edge, path) in paths {
            for (_, obligation) in path
                .iter()
                .filter_map(terminal_psi::StructuralPathSegment::runtime_index)
            {
                if !unit.accepted_obligation_facts.iter().any(|fact| {
                    fact.machine == function.machine
                        && fact.owner == optimization_unit::PsiProvenance::Edge(edge)
                        && fact.obligation == obligation
                }) {
                    return Err(
                        OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch,
                    );
                }
            }
        }
        match &node.operation {
            O::EstablishElementView {
                psi_operation,
                source,
                element,
                region:
                    terminal_psi::ElementViewRegion::RepresentationBytes {
                        start,
                        end,
                        obligation,
                    },
                ..
            } => {
                let invalid = OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch;
                let root =
                    super::operation_contracts::structural_source_contract(function, source.place)
                        .ok_or_else(|| invalid.clone())?;
                let collection = terminal_semantics::runtime_structural_path_tip(
                    types.values().copied(),
                    root.structural_type,
                    &source.path,
                )
                .ok_or_else(|| invalid.clone())?;
                let extent = terminal_semantics::representation_byte_extent(
                    types.values().copied(),
                    collection.id,
                )
                .ok_or_else(|| invalid.clone())?;
                let layout = terminal_semantics::representation_element_layout(
                    types.values().copied(),
                    *element,
                )
                .ok_or_else(|| invalid.clone())?;
                let goal = terminal_semantics::representation_region_goal(
                    *start,
                    *end,
                    extent,
                    layout.size,
                )
                .ok_or_else(|| invalid.clone())?;
                terminal_codec::canonical_proposition_order_key(&goal)
                    .map_err(|_| invalid.clone())?;
                // Rewrites retain the accepted bounds and tiling fact only
                // when it still proves this operation's exact current region.
                if unit
                    .accepted_obligation_facts
                    .iter()
                    .filter(|fact| {
                        fact.machine == function.machine
                            && fact.owner
                                == optimization_unit::PsiProvenance::Operation(*psi_operation)
                            && fact.obligation == *obligation
                            && fact.psi == unit.psi
                            && fact.has_canonical_identity()
                    })
                    .count()
                    != 1
                {
                    return Err(invalid);
                }
            }
            O::ElementViewWrite {
                psi_operation,
                destination,
                path,
                index,
                value,
                length,
                obligation,
            } => {
                let invalid = OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch;
                let root = super::operation_contracts::structural_source_contract(
                    function,
                    destination.place,
                )
                .ok_or_else(|| invalid.clone())?;
                let selected = terminal_semantics::runtime_structural_path_tip(
                    types.values().copied(),
                    root.structural_type,
                    &destination.path,
                )
                .ok_or_else(|| invalid.clone())?;
                let Some(StructuralTypeShape::ElementView { element, .. }) = Some(&selected.shape)
                else {
                    return Err(invalid);
                };
                if destination.access != terminal_psi::StructuralAccess::MutableBorrow
                    || !terminal_semantics::descriptor_source_projection_accessible(
                        types.values().copied(),
                        root.structural_type,
                        root.multiplicity,
                        root.backing_access(types.values().copied()),
                        &destination.path,
                        terminal_psi::StructuralAccess::MutableBorrow,
                    )
                {
                    return Err(invalid);
                }
                let mut written = vec![terminal_psi::StructuralPathSegment::RuntimeIndex {
                    index: *index,
                    obligation: *obligation,
                }];
                written.extend(path.iter().cloned());
                if !root.memberships_disjoint_from_write(&written) {
                    return Err(invalid);
                }
                let goal = terminal_semantics::element_view_write_goal(
                    &unit.structural_types,
                    *element,
                    path,
                    *index,
                    *length,
                    *value,
                )
                .ok_or_else(|| invalid.clone())?;
                terminal_codec::canonical_proposition_order_key(&goal)
                    .map_err(|_| invalid.clone())?;
                if unit
                    .accepted_obligation_facts
                    .iter()
                    .filter(|fact| {
                        fact.machine == function.machine
                            && fact.owner
                                == optimization_unit::PsiProvenance::Operation(*psi_operation)
                            && fact.obligation == *obligation
                            && fact.psi == unit.psi
                            && fact.has_canonical_identity()
                    })
                    .count()
                    != 1
                {
                    return Err(invalid);
                }
            }
            abstract_operations::byte_sequence_write_pair_pattern!(
                psi_operation,
                index,
                length,
                obligation,
                ..
            )
            | O::StructuralByteSequenceFieldRead {
                psi_operation,
                index,
                length,
                obligation,
                ..
            } => {
                use semantic_vocabulary::{IntegerSign, IntegerType, Proposition, ScalarTerm};
                let invalid = OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch;
                let integer =
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid.clone())?;
                let mut goal = Proposition::LessThan(
                    ScalarTerm::value(*index, ScalarType::Integer(integer)),
                    ScalarTerm::value(*length, ScalarType::Integer(integer)),
                );
                // A store into a predicate-bearing field was admitted only with
                // the stored byte's class legs proved beside its bound. The
                // accepted fact must be that exact strengthened goal, rebuilt
                // from the current destination declaration.
                if let O::StructuralByteSequenceFieldByteStore {
                    destination,
                    path,
                    field,
                    value,
                    ..
                } = &node.operation
                {
                    let predicates = function_structure::byte_field_predicates(
                        unit,
                        function,
                        *destination,
                        path,
                        *field,
                    )
                    .ok_or(invalid.clone())?;
                    goal = terminal_semantics::indexed_byte_store_goal(goal, *value, predicates)
                        .map_err(|_| invalid.clone())?;
                }
                if let O::ByteSequenceWrite {
                    destination, value, ..
                } = &node.operation
                {
                    let predicates =
                        function_structure::byte_write_predicates(unit, function, *destination)
                            .ok_or(invalid.clone())?;
                    goal = terminal_semantics::indexed_byte_store_goal(goal, *value, predicates)
                        .map_err(|_| invalid.clone())?;
                }
                terminal_codec::canonical_proposition_order_key(&goal)
                    .map_err(|_| invalid.clone())?;
                if unit
                    .accepted_obligation_facts
                    .iter()
                    .filter(|fact| {
                        fact.machine == function.machine
                            && fact.owner
                                == optimization_unit::PsiProvenance::Operation(*psi_operation)
                            && fact.obligation == *obligation
                            && fact.psi == unit.psi
                            && fact.has_canonical_identity()
                    })
                    .count()
                    != 1
                {
                    return Err(invalid);
                }
            }
            O::EstablishByteSequenceConcatenation {
                psi_operation,
                result,
                left,
                right,
                capacity_obligation,
            } => {
                use semantic_vocabulary::{
                    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition,
                };
                let invalid = OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch;
                if !function_structure::concatenation_preserves_predicates(
                    unit,
                    function,
                    &node.operation,
                ) {
                    return Err(invalid);
                }
                let Some(terminal_psi::StructuralTypeDeclaration {
                    shape:
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                        ),
                    ..
                }) = types.get(&result.structural_type).copied()
                else {
                    return Err(invalid);
                };
                let source_type =
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid.clone())?;
                terminal_codec::canonical_proposition_order_key(
                    &Proposition::IntegerMathLessOrEqual(
                        IntegerMathTerm::Add(
                            Box::new(IntegerMathTerm::MathValue {
                                source_type,
                                value: left.length(),
                            }),
                            Box::new(IntegerMathTerm::MathValue {
                                source_type,
                                value: right.length(),
                            }),
                        ),
                        IntegerMathTerm::literal(IntegerValue::Unsigned(u128::from(*capacity))),
                    ),
                )
                .map_err(|_| invalid.clone())?;
                if unit
                    .accepted_obligation_facts
                    .iter()
                    .filter(|fact| {
                        fact.machine == function.machine
                            && fact.owner
                                == optimization_unit::PsiProvenance::Operation(*psi_operation)
                            && fact.obligation == *capacity_obligation
                            && fact.psi == unit.psi
                            && fact.has_canonical_identity()
                    })
                    .count()
                    != 1
                {
                    return Err(invalid);
                }
            }
            abstract_operations::byte_sequence_store_pair_pattern!(
                psi_operation,
                length,
                obligation,
                ..
            ) => {
                use semantic_vocabulary::{
                    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm,
                };
                let invalid = OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch;
                let capacity =
                    function_structure::byte_field_store_capacity(function, &node.operation, types)
                        .ok_or(invalid.clone())?;
                if !function_structure::replacement_preserves_predicates(
                    unit,
                    function,
                    &node.operation,
                ) {
                    return Err(invalid);
                }
                let integer =
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid.clone())?;
                terminal_codec::canonical_proposition_order_key(&Proposition::LessOrEqual(
                    ScalarTerm::value(*length, ScalarType::Integer(integer)),
                    ScalarTerm::Integer {
                        scalar_type: integer,
                        value: IntegerValue::Unsigned(u128::from(capacity)),
                    },
                ))
                .map_err(|_| invalid.clone())?;
                if unit
                    .accepted_obligation_facts
                    .iter()
                    .filter(|fact| {
                        fact.machine == function.machine
                            && fact.owner
                                == optimization_unit::PsiProvenance::Operation(*psi_operation)
                            && fact.obligation == *obligation
                            && fact.psi == unit.psi
                            && fact.has_canonical_identity()
                    })
                    .count()
                    != 1
                {
                    return Err(invalid);
                }
            }
            O::StructuralByteViewFieldStore {
                psi_operation,
                destination,
                path,
                field,
                source,
                predicate_obligation,
            } => {
                // A retained certificate proves this exact source at this
                // operation. Reconstruct after rewriting: neither a sibling
                // field's qualification nor an old source's proof transfers.
                let invalid = OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch;
                let required = match field {
                    Some(field) => function_structure::byte_field_predicates(
                        unit,
                        function,
                        *destination,
                        path,
                        *field,
                    ),
                    // A declared home declares its qualifications' predicates.
                    None => function_structure::declared_descriptor_home(function, *destination)
                        .and_then(|home| {
                            terminal_semantics::byte_qualification_predicates_in(
                                unit.structural_domains.iter(),
                                home.structural_type,
                                &home.qualifications,
                            )
                        }),
                }
                .ok_or_else(|| invalid.clone())?;
                let established = function_structure::whole_source_predicates(
                    unit.structural_types.iter(),
                    unit.structural_domains.iter(),
                    function,
                    *source,
                )
                .ok_or_else(|| invalid.clone())?;
                if let Some(obligation) = predicate_obligation {
                    if required.is_empty() {
                        return Err(invalid);
                    }
                    let goal = terminal_semantics::byte_predicate_requirement(
                        required,
                        established,
                        *source,
                        &[],
                    );
                    terminal_codec::canonical_proposition_order_key(&goal)
                        .map_err(|_| invalid.clone())?;
                    if unit
                        .accepted_obligation_facts
                        .iter()
                        .filter(|fact| {
                            fact.machine == function.machine
                                && fact.owner
                                    == optimization_unit::PsiProvenance::Operation(*psi_operation)
                                && fact.obligation == *obligation
                                && fact.psi == unit.psi
                                && fact.has_canonical_identity()
                        })
                        .count()
                        != 1
                    {
                        return Err(invalid);
                    }
                } else if !established.implies(required) {
                    return Err(invalid);
                }
            }
            O::EstablishPrimitiveLocal {
                psi_operation,
                result,
                value,
                range_obligation,
            } => {
                let declaration = types
                    .get(&result.structural_type)
                    .ok_or(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)?;
                let restriction = declaration.scalar_restriction.as_ref();
                if restriction.is_some() != range_obligation.is_some() {
                    return Err(
                        OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch,
                    );
                }
                if let Some(restriction) = restriction {
                    let receiving = optimization_unit::SourceCheckedScalarReceiving {
                        value: value.value,
                        restriction: restriction.clone(),
                    };
                    if unit
                        .accepted_obligation_facts
                        .iter()
                        .filter(|fact| {
                            fact.machine == function.machine
                                && fact.owner
                                    == optimization_unit::PsiProvenance::Operation(*psi_operation)
                                && Some(fact.obligation) == *range_obligation
                                && fact.psi == unit.psi
                                && fact.has_canonical_identity()
                                && fact.scalar_receiving.as_ref() == Some(&receiving)
                        })
                        .count()
                        != 1
                    {
                        return Err(
                            OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch,
                        );
                    }
                    validate_scalar_range_authority(
                        unit,
                        function.machine,
                        *psi_operation,
                        restriction,
                        value.value,
                        *range_obligation,
                    )?;
                }
            }
            O::WriteOnlyPrimitiveStore {
                psi_operation,
                destination,
                path,
                value,
                range_obligation,
            } => {
                let (whole, projected) = match destination {
                    abstract_operations::StructuralScalarStoreDestination::Parameter(parameter)
                    | abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter { parameter, .. } => (&parameter.qualifications, &parameter.projected_qualifications),
                    abstract_operations::StructuralScalarStoreDestination::OwnedResult { result, .. } => (&result.qualifications, &result.projected_qualifications),
                };
                let restriction = terminal_semantics::primitive_store_restriction(
                    types.values().copied(),
                    unit.structural_domains.iter(),
                    destination.structural_type(),
                    whole,
                    projected,
                    path,
                )
                .ok_or(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)?;
                if restriction.is_some() != range_obligation.is_some() {
                    return Err(
                        OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch,
                    );
                }
                if let Some(restriction) = restriction {
                    let receiving = optimization_unit::SourceCheckedScalarReceiving {
                        value: value.value,
                        restriction: restriction.clone(),
                    };
                    if unit
                        .accepted_obligation_facts
                        .iter()
                        .filter(|fact| {
                            fact.machine == function.machine
                                && fact.owner
                                    == optimization_unit::PsiProvenance::Operation(*psi_operation)
                                && Some(fact.obligation) == *range_obligation
                                && fact.psi == unit.psi
                                && fact.has_canonical_identity()
                                && fact.scalar_receiving.as_ref() == Some(&receiving)
                        })
                        .count()
                        != 1
                    {
                        return Err(
                            OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch,
                        );
                    }
                    validate_scalar_range_authority(
                        unit,
                        function.machine,
                        *psi_operation,
                        &restriction,
                        value.value,
                        *range_obligation,
                    )?;
                }
            }
            O::StructuralScalarFieldStore {
                psi_operation,
                destination,
                path,
                field,
                value,
                range_obligation,
            } => {
                // Check the current store, not merely the original Terminal
                // write: a rewrite can change its RHS or selected declaration.
                let (storage_type, storage_path) = if path
                    .contains(&terminal_psi::StructuralPathSegment::Referent)
                {
                    super::references::record_field_destination(
                        function,
                        types,
                        destination.place(),
                        path,
                        terminal_psi::StructuralAccess::WriteOnlyBorrow,
                    )
                    .ok_or(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)?
                } else {
                    (destination.structural_type(), path.as_slice())
                };
                let declaration = terminal_semantics::projected_field_declaration(
                    types.values().copied(),
                    storage_type,
                    storage_path,
                    *field,
                )
                .ok_or(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)?;
                if terminal_semantics::scalar_field_requires_evidence(&declaration.field_type)
                    != range_obligation.is_some()
                {
                    return Err(
                        OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch,
                    );
                }
                validate_field_range_authority(
                    unit,
                    function.machine,
                    *psi_operation,
                    declaration,
                    value.value,
                    *range_obligation,
                )?;
            }
            O::EstablishCase {
                psi_operation,
                result,
                result_case,
                fields,
            } => {
                let Some(terminal_psi::StructuralTypeDeclaration {
                    shape: StructuralTypeShape::Sum { cases },
                    ..
                }) = types.get(&result.structural_type).copied()
                else {
                    continue;
                };
                let Some(case) = find_structural_case(cases, *result_case) else {
                    continue;
                };
                for (declaration, binding) in case.fields.iter().zip(fields) {
                    if let RecordFieldValue::Scalar {
                        value,
                        range_obligation,
                    } = &binding.value
                    {
                        validate_field_range_authority(
                            unit,
                            function.machine,
                            *psi_operation,
                            declaration,
                            *value,
                            *range_obligation,
                        )?;
                    }
                }
            }
            O::EstablishRecord {
                psi_operation,
                result,
                fields,
            } => {
                let Some(terminal_psi::StructuralTypeDeclaration {
                    shape:
                        StructuralTypeShape::Record {
                            fields: declarations,
                            ..
                        },
                    ..
                }) = types.get(&result.structural_type).copied()
                else {
                    continue;
                };
                for (declaration, initializer) in declarations.iter().zip(fields) {
                    if let RecordFieldValue::Scalar {
                        value,
                        range_obligation,
                    } = &initializer.value
                    {
                        validate_field_range_authority(
                            unit,
                            function.machine,
                            *psi_operation,
                            declaration,
                            *value,
                            *range_obligation,
                        )?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_field_range_authority(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    operation: OperationId,
    declaration: &terminal_psi::StructuralFieldDeclaration,
    value: ValueId,
    obligation: Option<semantic_vocabulary::ObligationId>,
) -> Result<(), OptimizationUnitValidationError> {
    validate_scalar_range_authority(
        unit,
        machine,
        operation,
        &declaration.field_type,
        value,
        obligation,
    )
}

fn validate_scalar_range_authority(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    operation: OperationId,
    restriction: &terminal_psi::StructuralFieldType,
    value: ValueId,
    obligation: Option<semantic_vocabulary::ObligationId>,
) -> Result<(), OptimizationUnitValidationError> {
    let Some(requirement) = terminal_semantics::scalar_field_requirement(restriction, value) else {
        return Ok(());
    };
    terminal_codec::canonical_proposition_order_key(&requirement)
        .map_err(|_| OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)?;
    if unit
        .accepted_obligation_facts
        .iter()
        .filter(|fact| {
            fact.machine == machine
                && fact.owner == optimization_unit::PsiProvenance::Operation(operation)
                && Some(fact.obligation) == obligation
                && fact.psi == unit.psi
                && fact.has_canonical_identity()
        })
        .count()
        != 1
    {
        return Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch);
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
        ..
    } = indexes;
    for fact in &unit.ownership_frontier_facts {
        if unit.function(fact.machine).is_none() && !pruned.contains(&fact.machine) {
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
