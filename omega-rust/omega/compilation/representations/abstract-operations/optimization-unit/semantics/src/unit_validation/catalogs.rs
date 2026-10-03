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
    let call_accesses = crate::current_ownership::CallAccesses::new(&machines, &structural_types);
    let mut descriptor_requirements = BTreeMap::new();
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
        )?;
        validate_structural_field_range_authority(unit, function, &structural_types)?;
    }
    crate::current_ownership::validate_descriptor_compatibility(unit, &descriptor_requirements)?;
    Ok(UnitIndexes {
        machines,
        boundary_machines,
        services,
        pruned,
    })
}

fn validate_structural_field_range_authority(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    use abstract_operations::AbstractOperation as O;
    use terminal_psi::{RecordFieldValue, StructuralTypeShape};
    for node in function.blocks.iter().flat_map(|block| &block.nodes) {
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
                let proposition = terminal_codec::canonical_proposition_order_key(&goal)
                    .map_err(|_| invalid.clone())?;
                // Rewrites retain the accepted bounds and tiling fact only
                // when it still proves this operation's exact current region.
                if unit
                    .accepted_obligation_facts
                    .iter()
                    .filter(|fact| {
                        fact.machine == function.machine
                            && fact.operation == *psi_operation
                            && fact.obligation == *obligation
                            && fact.proposition == proposition
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
                let root =
                    super::operation_contracts::structural_source_contract(function, *destination)
                        .ok_or_else(|| invalid.clone())?;
                let Some(StructuralTypeShape::ElementView { element }) =
                    types.get(&root.structural_type).map(|row| &row.shape)
                else {
                    return Err(invalid);
                };
                if root.access != terminal_psi::StructuralAccess::MutableBorrow {
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
                let proposition = terminal_codec::canonical_proposition_order_key(&goal)
                    .map_err(|_| invalid.clone())?;
                if unit
                    .accepted_obligation_facts
                    .iter()
                    .filter(|fact| {
                        fact.machine == function.machine
                            && fact.operation == *psi_operation
                            && fact.obligation == *obligation
                            && fact.proposition == proposition
                            && fact.psi == unit.psi
                            && fact.has_canonical_identity()
                    })
                    .count()
                    != 1
                {
                    return Err(invalid);
                }
            }
            O::ByteSequenceWrite {
                psi_operation,
                index,
                length,
                obligation,
                ..
            }
            | O::StructuralByteSequenceFieldByteStore {
                psi_operation,
                index,
                length,
                obligation,
                ..
            }
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
                    let predicates = if function.structural_places.iter().any(|place| {
                        place.id == *destination
                            && matches!(
                                place.kind,
                                semantic_vocabulary::StructuralPlaceKind::OperationResult { .. }
                            )
                    }) {
                        function_structure::owned_byte_predicates(unit, function, *destination)
                            .ok_or(invalid.clone())?
                    } else {
                        terminal_psi::ByteSequencePredicateSet::EMPTY
                    };
                    goal = terminal_semantics::indexed_byte_store_goal(goal, *value, predicates)
                        .map_err(|_| invalid.clone())?;
                }
                let proposition = terminal_codec::canonical_proposition_order_key(&goal)
                    .map_err(|_| invalid.clone())?;
                if unit
                    .accepted_obligation_facts
                    .iter()
                    .filter(|fact| {
                        fact.machine == function.machine
                            && fact.operation == *psi_operation
                            && fact.obligation == *obligation
                            && fact.proposition == proposition
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
                let proposition = terminal_codec::canonical_proposition_order_key(
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
                            && fact.operation == *psi_operation
                            && fact.obligation == *capacity_obligation
                            && fact.proposition == proposition
                            && fact.psi == unit.psi
                            && fact.has_canonical_identity()
                    })
                    .count()
                    != 1
                {
                    return Err(invalid);
                }
            }
            O::ByteSequenceStore {
                psi_operation,
                length,
                obligation,
                ..
            }
            | O::StructuralByteSequenceFieldStore {
                psi_operation,
                length,
                obligation,
                ..
            } => {
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
                let proposition =
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
                            && fact.operation == *psi_operation
                            && fact.obligation == *obligation
                            && fact.proposition == proposition
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
                let required = function_structure::byte_field_predicates(
                    unit,
                    function,
                    *destination,
                    path,
                    *field,
                )
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
                    let proposition = terminal_codec::canonical_proposition_order_key(&goal)
                        .map_err(|_| invalid.clone())?;
                    if unit
                        .accepted_obligation_facts
                        .iter()
                        .filter(|fact| {
                            fact.machine == function.machine
                                && fact.operation == *psi_operation
                                && fact.obligation == *obligation
                                && fact.proposition == proposition
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
                let (storage_type, storage_path) = if matches!(
                    path.first(),
                    Some(terminal_psi::StructuralPathSegment::Referent)
                ) {
                    super::references::mutable_record_field_destination(
                        function,
                        types,
                        destination.place(),
                        path,
                    )
                    .ok_or(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)?
                } else {
                    (destination.structural_type(), path.as_slice())
                };
                let parent = super::structural_catalog::resolve_structural_path(
                    types,
                    storage_type,
                    storage_path,
                )
                .ok_or(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)?;
                // A mixed shape's common fields are stored directly, like
                // record fields.
                let Some(terminal_psi::StructuralTypeDeclaration {
                    shape:
                        StructuralTypeShape::Record { fields }
                        | StructuralTypeShape::Mixed { fields, .. },
                    ..
                }) = types.get(&parent).copied()
                else {
                    return Err(
                        OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch,
                    );
                };
                let declaration = fields
                    .iter()
                    .find(|candidate| candidate.id == *field)
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
                let Some(case) = cases.iter().find(|case| case.id == *result_case) else {
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
    let Some(requirement) =
        terminal_semantics::scalar_field_requirement(&declaration.field_type, value)
    else {
        return Ok(());
    };
    let proposition = terminal_codec::canonical_proposition_order_key(&requirement)
        .map_err(|_| OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)?;
    if unit
        .accepted_obligation_facts
        .iter()
        .filter(|fact| {
            fact.machine == machine
                && fact.operation == operation
                && Some(fact.obligation) == obligation
                && fact.proposition == proposition
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::fixtures::structural_catalog::structural_scalar_field_store_unit;
    use crate::tests::support::id;
    use terminal_psi::{
        StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape,
    };

    #[test]
    fn reference_scalar_store_replays_referent_range_authority() {
        // Exercise this receiving pass directly: whole-unit ownership and
        // producer replay are separate gates covered by the authored fixture.
        let mut unit = structural_scalar_field_store_unit();
        let carrier = id(9_201, StructuralTypeId::new);
        let parameter = &mut unit.functions[0].structural_parameters[0];
        unit.structural_types
            .make_mut()
            .push(terminal_psi::StructuralTypeDeclaration {
                id: carrier,
                identity: "validation::mutable-store-reference".into(),
                shape: StructuralTypeShape::Reference {
                    referent: parameter.structural_type,
                    access: StructuralAccess::MutableBorrow,
                },
            });
        parameter.structural_type = carrier;
        parameter.access = StructuralAccess::Owned;
        parameter.multiplicity = StructuralMultiplicity::Affine;
        let retained = parameter.clone();
        let abstract_operations::AbstractOperation::StructuralScalarFieldStore {
            destination,
            path,
            ..
        } = &mut unit.functions[0].blocks[0].nodes[1].operation
        else {
            unreachable!()
        };
        *destination = abstract_operations::StructuralScalarStoreDestination::Parameter(retained);
        path.insert(0, StructuralPathSegment::Referent);
        let validate = |unit: &PsiOptimizationUnit| {
            let types = unit
                .structural_types
                .iter()
                .map(|declaration| (declaration.id, declaration))
                .collect();
            validate_structural_field_range_authority(unit, &unit.functions[0], &types)
        };
        assert!(
            validate(&unit).is_ok(),
            "unbounded field still owes exact referent geometry"
        );
        let mut malformed = unit.clone();
        let abstract_operations::AbstractOperation::StructuralScalarFieldStore { path, .. } =
            &mut malformed.functions[0].blocks[0].nodes[1].operation
        else {
            unreachable!()
        };
        path.remove(0);
        assert!(
            validate(&malformed).is_err(),
            "carrier is not record backing"
        );
        let mut shared = unit.clone();
        let StructuralTypeShape::Reference { access, .. } =
            &mut shared.structural_types.make_mut().last_mut().unwrap().shape
        else {
            unreachable!()
        };
        *access = StructuralAccess::SharedBorrow;
        assert!(validate(&shared).is_err());

        let abstract_operations::AbstractOperation::StructuralScalarFieldStore {
            field, value, ..
        } = unit.functions[0].blocks[0].nodes[1].operation
        else {
            unreachable!()
        };
        let ScalarType::Integer(integer) = value.scalar_type else {
            unreachable!()
        };
        let selected = unit
            .structural_types
            .make_mut()
            .iter_mut()
            .find_map(|declaration| {
                let StructuralTypeShape::Record { fields } = &mut declaration.shape else {
                    return None;
                };
                fields.iter_mut().find(|candidate| candidate.id == field)
            })
            .unwrap();
        selected.field_type = terminal_psi::StructuralFieldType::BoundedInteger(
            semantic_vocabulary::BoundedIntegerType::new(
                integer,
                semantic_vocabulary::IntegerValue::Signed(-7),
                semantic_vocabulary::IntegerValue::Signed(17),
            )
            .unwrap(),
        );
        assert!(
            validate(&unit).is_err(),
            "reference geometry cannot waive a bounded field obligation"
        );
    }
}
