//! Evaluated scalar operands captured by storage writes and block arrivals.
//!
//! These are the original SSA operands at their pre-operation cuts, not their
//! denotations expanded through other definitions. A saved value keeps that
//! snapshot even if an operand used to compute it later renews in a cycle.

use std::collections::BTreeMap;

use semantic_vocabulary::{
    PlaceId, ScalarTerm, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Operation, OperationKind, RecordFieldValue, TerminalMachine, TerminalModule, Terminator,
};

use crate::descriptors::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPosition, descriptor_location,
};

#[derive(Clone, Debug)]
pub enum CapturedScalarOperand {
    Value {
        id: ValueId,
        scalar_type: ScalarType,
    },
    /// A case edge reads this selected payload before its ordered cleanup.
    Storage(DescriptorLocation),
}

impl CapturedScalarOperand {
    pub fn value_term(&self) -> Option<ScalarTerm> {
        match self {
            Self::Value { id, scalar_type } => Some(ScalarTerm::value(*id, *scalar_type)),
            Self::Storage(_) => None,
        }
    }
}

/// One immutable roster for a machine. Missing operations are not scalar
/// writers; an unresolved supported writer is retained as an explicit error.
/// The storage-version owner still checks all other mutations and clobbers.
pub struct TerminalScalarCaptures {
    pub writes:
        BTreeMap<DescriptorPosition, Result<Vec<(DescriptorLocation, ScalarTerm)>, &'static str>>,
    pub arrivals: BTreeMap<DescriptorPosition, Vec<Vec<(ValueId, CapturedScalarOperand)>>>,
}

impl TerminalScalarCaptures {
    pub fn new(module: &TerminalModule, machine: &TerminalMachine) -> Result<Self, &'static str> {
        let place_types = place_type_index(machine)?;
        let mut types = BTreeMap::new();
        for value in machine
            .parameters
            .iter()
            .chain(machine.block_parameters())
            .chain(
                machine
                    .operations()
                    .filter_map(|operation| operation.result.scalar_ref()),
            )
        {
            if types.insert(value.id, value.scalar_type).is_some() {
                return Err("scalar capture roster repeats a scalar binder");
            }
        }
        let blocks = machine
            .blocks
            .iter()
            .map(|block| (block.id, block))
            .collect::<BTreeMap<_, _>>();
        if blocks.len() != machine.blocks.len() {
            return Err("scalar capture roster repeats a block");
        }
        let mut result = Self {
            writes: BTreeMap::new(),
            arrivals: BTreeMap::new(),
        };
        for block in &machine.blocks {
            for (ordinal, operation) in block.operations.iter().enumerate() {
                if let Some(writes) = scalar_storage_writes(module, &place_types, operation) {
                    let position = (
                        block.id,
                        u32::try_from(ordinal)
                            .map_err(|_| "scalar write ordinal exceeds its canonical position")?,
                    );
                    let writes = writes.and_then(|rows| {
                        rows.into_iter()
                            .map(|(location, actual)| {
                                let scalar_type = *types
                                    .get(&actual)
                                    .ok_or("scalar writer has no evaluated operand binder")?;
                                Ok((location, ScalarTerm::value(actual, scalar_type)))
                            })
                            .collect()
                    });
                    result.writes.insert(position, writes);
                }
            }
            let position = (
                block.id,
                u32::try_from(block.operations.len())
                    .map_err(|_| "scalar arrival ordinal exceeds its canonical position")?,
            );
            let mut arrivals = Vec::new();
            for (ordinal, binding) in crate::control_graph::forwarded_bindings(block).enumerate() {
                let target = blocks
                    .get(&binding.target)
                    .ok_or("scalar arrival names an absent target block")?;
                let operands = if let Some(arguments) = binding.arguments {
                    if arguments.len() != target.parameters.len() {
                        return Err("scalar arrival changes its target parameter count");
                    }
                    target
                        .parameters
                        .iter()
                        .zip(arguments)
                        .map(|(formal, actual)| {
                            let scalar_type = *types
                                .get(actual)
                                .ok_or("scalar arrival has no actual operand binder")?;
                            if scalar_type != formal.scalar_type {
                                return Err("scalar arrival changes its actual operand type");
                            }
                            Ok((
                                formal.id,
                                CapturedScalarOperand::Value {
                                    id: *actual,
                                    scalar_type,
                                },
                            ))
                        })
                        .collect::<Result<Vec<_>, _>>()?
                } else {
                    let Terminator::StructuralCase {
                        source,
                        path,
                        cases,
                    } = &block.terminator
                    else {
                        return Err("scalar arrival has no original case-payload capture");
                    };
                    let edge = cases
                        .get(ordinal)
                        .ok_or("scalar arrival lost its selected case edge")?;
                    if edge.target != binding.target
                        || edge.payload_fields.len() != target.parameters.len()
                    {
                        return Err(
                            "scalar case arrival changes its original edge or parameter count",
                        );
                    }
                    let root = *place_types
                        .get(source)
                        .ok_or("scalar case source has no declared carrier")?;
                    let captured_types = super::case_payload_scalar_types(
                        &module.structural_types,
                        root,
                        path,
                        edge.case,
                        &edge.payload_fields,
                    )
                    .ok_or("scalar case arrival has no exact declared field roster")?;
                    let mut selected = path.clone();
                    selected.push(terminal_psi::StructuralPathSegment::Case(edge.case));
                    target
                        .parameters
                        .iter()
                        .zip(&edge.payload_fields)
                        .zip(captured_types)
                        .map(|((formal, field), scalar_type)| {
                            if scalar_type != formal.scalar_type {
                                return Err(
                                    "scalar case arrival changes its declared payload type",
                                );
                            }
                            let location = descriptor_location(
                                &module.structural_types,
                                *source,
                                root,
                                &selected,
                                Some(*field),
                            )
                            .ok_or("scalar case arrival has no exact payload storage coordinate")?;
                            Ok((formal.id, CapturedScalarOperand::Storage(location)))
                        })
                        .collect::<Result<Vec<_>, _>>()?
                };
                arrivals.push(operands);
            }
            if !arrivals.is_empty() {
                result.arrivals.insert(position, arrivals);
            }
        }
        Ok(result)
    }
}

#[derive(Clone, Debug)]
pub struct ResidentValueTransfer {
    pub source: DescriptorLocation,
    pub destination: DescriptorLocation,
}

pub enum ResidentValueCapture {
    Complete(Vec<ResidentValueTransfer>),
    Unresolved(&'static str),
}

pub type ResidentValueCaptures = BTreeMap<DescriptorPosition, ResidentValueCapture>;

/// Resident value captures already derived by the canonical storage adapter.
/// Address formation and view construction do not copy their backing's scalar
/// contents. Keep that distinction at the producing operation, then retain its
/// complete typed source/destination roster rather than reconstructing fields
/// at each consumer. The caller must use the descriptor graph reconstructed
/// from this same checked body and selected-call context.
pub fn terminal_resident_value_transfers(
    machine: &TerminalMachine,
    graph: &super::DescriptorMachine,
) -> Result<ResidentValueCaptures, &'static str> {
    if graph.id != machine.id || graph.entry != (machine.entry, 0) {
        return Err("resident capture graph belongs to another machine");
    }
    let mut captures = BTreeMap::new();
    for block in &machine.blocks {
        for (ordinal, operation) in block.operations.iter().enumerate() {
            let position = (
                block.id,
                u32::try_from(ordinal)
                    .map_err(|_| "resident capture ordinal exceeds its canonical position")?,
            );
            let step = graph
                .flow
                .get(&position)
                .ok_or("resident capture has no current storage step")?;
            if matches!(
                &operation.kind,
                terminal_psi::structural_leaf_copy_pair_pattern!()
                    | OperationKind::MoveStructuralValue { .. }
                    | OperationKind::TransformContent { .. }
                    | OperationKind::MoveStructuralField { .. }
                    | OperationKind::StoreStructuralField { .. }
                    | OperationKind::EstablishStructuralArray { .. }
                    | OperationKind::EstablishRecord { .. }
                    | OperationKind::EstablishStructuralCase { .. }
            ) {
                // An unresolved coordinate remains an explicit refusal. A
                // source root with no recorded transfer is never an empty
                // permission or evidence that the writer preserves contents.
                if step.may_escape {
                    captures.insert(
                        position,
                        ResidentValueCapture::Unresolved(
                            "resident value capture has an unresolved storage coordinate",
                        ),
                    );
                    continue;
                }
                if step.derivations.iter().any(|(source, destination)| {
                    graph.storage_derivations.contains(&(
                        position,
                        source.clone(),
                        destination.clone(),
                    ))
                }) {
                    captures.insert(
                        position,
                        ResidentValueCapture::Unresolved(
                            "resident value capture substitutes an address formation",
                        ),
                    );
                    continue;
                }
                captures.insert(
                    position,
                    ResidentValueCapture::Complete(
                        step.derivations
                            .iter()
                            .map(|(source, destination)| ResidentValueTransfer {
                                source: source.clone(),
                                destination: destination.clone(),
                            })
                            .collect(),
                    ),
                );
            }
        }
    }
    Ok(captures)
}

/// Build the same signature/producer carrier join used by Terminal storage,
/// once for this immutable machine. Repeated declaration or parameter keys
/// cannot select whichever carrier makes a later projection succeed.
fn place_type_index(
    machine: &TerminalMachine,
) -> Result<BTreeMap<PlaceId, StructuralTypeId>, &'static str> {
    let mut types = BTreeMap::new();
    let mut parameter_kinds = BTreeMap::new();
    for (ordinal, parameter) in machine.structural_parameters.iter().enumerate() {
        if usize::try_from(parameter.position).ok() != Some(ordinal) {
            return Err("scalar capture signature parameter has no dense coordinate");
        }
        if types
            .insert(parameter.place, parameter.structural_type)
            .is_some()
        {
            return Err("scalar capture roster repeats a structural parameter");
        }
        parameter_kinds.insert(
            parameter.place,
            StructuralPlaceKind::Parameter {
                position: parameter.position,
            },
        );
    }
    for block in &machine.blocks {
        for (ordinal, parameter) in block.structural_parameters.iter().enumerate() {
            if usize::try_from(parameter.position).ok() != Some(ordinal) {
                return Err("scalar capture block parameter has no dense coordinate");
            }
            if types
                .insert(parameter.place, parameter.structural_type)
                .is_some()
            {
                return Err("scalar capture roster repeats a structural parameter");
            }
            parameter_kinds.insert(
                parameter.place,
                StructuralPlaceKind::BlockParameter {
                    block: block.id,
                    position: parameter.position,
                },
            );
        }
    }
    let mut declared = std::collections::BTreeSet::new();
    for place in &machine.structural_places {
        if !declared.insert(place.id) {
            return Err("scalar capture roster repeats a structural place");
        }
        if parameter_kinds
            .get(&place.id)
            .is_some_and(|kind| *kind != place.kind)
        {
            return Err("scalar capture parameter changes its declared storage kind");
        }
        match place.kind {
            kind @ (StructuralPlaceKind::Parameter { .. }
            | StructuralPlaceKind::BlockParameter { .. }) => {
                if parameter_kinds.get(&place.id) != Some(&kind) {
                    return Err("scalar capture place changes its parameter coordinate");
                }
            }
            StructuralPlaceKind::Result => {
                let result = machine
                    .result
                    .structural()
                    .filter(|result| result.place == place.id)
                    .ok_or("scalar capture result place has no matching result signature")?;
                if types.insert(place.id, result.structural_type).is_some() {
                    return Err("scalar capture result repeats an existing storage namespace");
                }
            }
            StructuralPlaceKind::ProviderAttachment { .. } => {}
            kind => {
                let carrier = kind
                    .produced_structural_type()
                    .ok_or("scalar capture producer place has no carrier")?;
                if types.insert(place.id, carrier).is_some() {
                    return Err("scalar capture producer repeats an existing storage namespace");
                }
            }
        }
    }
    if types.keys().any(|place| !declared.contains(place)) {
        return Err("scalar capture parameter has no declared storage place");
    }
    Ok(types)
}

/// Enumerate only actual scalar-writing operands. Numeric postconditions and
/// equalities from completed calls are not producing storage events.
pub(super) fn scalar_storage_writes(
    module: &TerminalModule,
    place_types: &BTreeMap<PlaceId, StructuralTypeId>,
    operation: &Operation,
) -> Option<Result<Vec<(DescriptorLocation, ValueId)>, &'static str>> {
    let mut writes = Vec::new();
    let mut write = |root, path: &[terminal_psi::StructuralPathSegment], field, value| {
        let root_type = *place_types
            .get(&root)
            .ok_or("scalar writer has no declared destination carrier")?;
        let location = descriptor_location(&module.structural_types, root, root_type, path, field)
            .ok_or("scalar writer has no exact supported destination projection")?;
        writes.push((location, value));
        Ok::<_, &'static str>(())
    };
    let capture = || -> Result<_, &'static str> {
        match &operation.kind {
            OperationKind::WriteOnlyPrimitiveStore {
                destination,
                path,
                value,
                ..
            } => write(*destination, path, None, *value)?,
            OperationKind::StructuralScalarFieldStore {
                destination,
                path,
                field,
                value,
                ..
            } => write(*destination, path, Some(*field), *value)?,
            OperationKind::EstablishPrimitiveLocal { value, .. } => {
                let result = operation
                    .result
                    .structural()
                    .ok_or("primitive initializer has no structural result")?;
                write(result.place, &[], None, *value)?;
            }
            OperationKind::EstablishScalarArray { elements } => {
                let result = operation
                    .result
                    .structural()
                    .ok_or("scalar array initializer has no structural result")?;
                for (ordinal, value) in elements.iter().enumerate() {
                    let path = crate::scalar_array_leaf_path(
                        module.structural_types.iter(),
                        result.structural_type,
                        u64::try_from(ordinal)
                            .map_err(|_| "scalar array initializer ordinal is too large")?,
                    )
                    .ok_or("scalar array initializer has no declared scalar leaf")?;
                    write(result.place, &path, None, *value)?;
                }
            }
            OperationKind::EstablishScalarCase {
                result_case,
                fields,
            } => {
                let result = operation
                    .result
                    .structural()
                    .ok_or("scalar case initializer has no structural result")?;
                let path = [terminal_psi::StructuralPathSegment::Case(*result_case)];
                for field in fields {
                    write(result.place, &path, Some(field.field), field.value)?;
                }
            }
            OperationKind::EstablishRecord { fields }
            | OperationKind::EstablishStructuralCase { fields, .. } => {
                let result = operation
                    .result
                    .structural()
                    .ok_or("record initializer has no structural result")?;
                for (ordinal, field) in fields.iter().enumerate() {
                    let RecordFieldValue::Scalar { value, .. } = &field.value else {
                        continue;
                    };
                    if let OperationKind::EstablishStructuralCase { result_case, .. } =
                        &operation.kind
                    {
                        let shape = &module
                            .structural_type(result.structural_type)
                            .ok_or("case initializer has no declared carrier")?
                            .shape;
                        let path = terminal_psi::structural_case_initializer_path(
                            shape,
                            *result_case,
                            ordinal,
                            field.field,
                        )
                        .ok_or("case initializer changes its declared scalar field roster")?;
                        write(result.place, &path, None, *value)?;
                    } else {
                        write(result.place, &[], Some(field.field), *value)?;
                    }
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    };
    let mut capture = capture;
    match capture() {
        Ok(false) => None,
        Ok(true) => Some(Ok(writes)),
        Err(reason) => Some(Err(reason)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, MachineId, OperationId, PlaceId, ScalarQualificationSetId,
        StructuralTypeId,
    };
    use terminal_psi::{
        Block, OperationResult, StructuralAccess, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
        SuccessorEdge, TerminalIdentityIndex, TerminalMachineResult, ValueDeclaration,
    };
    fn machine(id: u64) -> TerminalMachine {
        let block = BlockId::new(1).unwrap();
        TerminalMachine {
            assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
            id: MachineId::new(id).unwrap(),
            declaration: None,
            attachment: None,
            nominal_cleanup_subject: None,
            parameters: Vec::new(),
            parameter_order: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            declared_service_reach: Vec::new(),
            closed_reach_application: None,
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block::plain(
                block,
                Vec::new(),
                Terminator::ReturnUnit {
                    edge: EdgeId::new(1).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            )],
            contract: terminal_psi::MachineContract::empty(ContractId::new(1).unwrap()),
        }
    }

    #[test]
    fn stored_and_forwarded_scalars_keep_evaluated_binders_and_both_edge_cuts() {
        let mut module = TerminalModule::declarations();
        let carrier = StructuralTypeId::new(1).unwrap();
        module.structural_types.push(StructuralTypeDeclaration {
            quotient_carrier: None,
            id: carrier,
            identity: TerminalIdentityIndex(1),
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            copyable: true,
            descriptor_cleanup: true,
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            scalar_restriction: None,
        });
        let scalar = |id| ValueDeclaration {
            id: ValueId::new(id).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: ScalarQualificationSetId::ZERO,
        };
        let mut machine = machine(1);
        let original = scalar(1);
        let saved = scalar(2);
        let formal = scalar(3);
        let home = PlaceId::new(1).unwrap();
        machine.parameters.push(original);
        machine.parameter_order = vec![
            terminal_psi::BoundaryParameterKind::Scalar,
            terminal_psi::BoundaryParameterKind::Structural,
        ];
        machine
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place: home,
                position: 0,
                structural_type: carrier,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::WriteOnlyBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
            });
        machine
            .structural_places
            .push(terminal_psi::StructuralPlaceDeclaration {
                id: home,
                kind: StructuralPlaceKind::Parameter { position: 0 },
            });
        machine.blocks[0].operations = vec![
            Operation::unsuspended(
                OperationId::new(1).unwrap(),
                OperationResult::Scalar(saved),
                OperationKind::BooleanNot {
                    operand: original.id,
                },
            ),
            Operation::unsuspended(
                OperationId::new(2).unwrap(),
                OperationResult::Unit,
                OperationKind::WriteOnlyPrimitiveStore {
                    destination: home,
                    path: Vec::new(),
                    value: saved.id,
                    range_obligation: None,
                },
            ),
        ];
        let target = BlockId::new(2).unwrap();
        let edge = |id, actual| SuccessorEdge {
            edge: EdgeId::new(id).unwrap(),
            target,
            arguments: vec![actual],
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            cleanup_actions: Vec::new(),
        };
        machine.blocks[0].terminator = Terminator::Conditional {
            condition: original.id,
            when_true: edge(1, saved.id),
            when_false: edge(2, original.id),
        };
        machine.blocks.push(Block::plain(
            target,
            vec![formal],
            Terminator::ReturnUnit {
                edge: EdgeId::new(3).unwrap(),
                cleanup_actions: Vec::new(),
            },
        ));
        let captures = TerminalScalarCaptures::new(&module, &machine).unwrap();
        let entry = machine.entry;
        assert_eq!(
            captures.writes[&(entry, 1)].as_ref().unwrap(),
            &vec![(
                DescriptorLocation::whole(home),
                ScalarTerm::value(saved.id, ScalarType::Boolean)
            )]
        );
        let edges = &captures.arrivals[&(entry, 2)];
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0][0].0, formal.id);
        assert_eq!(
            edges[0][0].1.value_term(),
            Some(ScalarTerm::value(saved.id, ScalarType::Boolean))
        );
        assert_eq!(
            edges[1][0].1.value_term(),
            Some(ScalarTerm::value(original.id, ScalarType::Boolean))
        );
        // The saved negation must not be rewritten as an expression mentioning
        // the potentially renewed original binder at the later observation.
        assert_ne!(
            edges[0][0].1.value_term(),
            Some(ScalarTerm::BooleanNot {
                operand: Box::new(ScalarTerm::value(original.id, ScalarType::Boolean))
            })
        );
        machine.blocks[0].operations[1].kind = OperationKind::WriteOnlyPrimitiveStore {
            destination: PlaceId::new(99).unwrap(),
            path: Vec::new(),
            value: saved.id,
            range_obligation: None,
        };
        let incomplete = TerminalScalarCaptures::new(&module, &machine).unwrap();
        assert!(incomplete.writes[&(entry, 1)].is_err());
        machine.structural_places[0].kind = StructuralPlaceKind::ProviderAttachment {
            attachment: carrier,
            field: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
            boundary: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
        };
        assert!(matches!(
            TerminalScalarCaptures::new(&module, &machine),
            Err("scalar capture parameter changes its declared storage kind")
        ));
    }
}
