use std::collections::BTreeMap;

use arena::{IdTable, IdTableBuildError};
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

pub struct TerminalScalarCaptures {
    pub writes: BTreeMap<
        DescriptorPosition,
        Result<Vec<(DescriptorLocation, ScalarTerm)>, &'static str>,
    >,
    pub arrivals: BTreeMap<DescriptorPosition, Vec<Vec<(ValueId, CapturedScalarOperand)>>>,
}

impl TerminalScalarCaptures {
    pub fn new(
        module: &TerminalModule,
        machine: &TerminalMachine,
    ) -> Result<Self, &'static str> {
        let place_types = place_type_index(machine)?;
        let mut type_rows = Vec::new();
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
            type_rows.push((value.id, Some(value.scalar_type)));
        }
        type_rows.sort_by_key(|(id, _)| *id);
        let types = IdTable::from_rows_with_gaps(type_rows, 65_536)
            .map_err(capture_inventory_failure)?;
        let mut block_rows = machine
            .blocks
            .iter()
            .map(|block| (block.id, Some(block)))
            .collect::<Vec<_>>();
        block_rows.sort_by_key(|(id, _)| *id);
        let blocks = IdTable::from_rows_with_gaps(block_rows, 65_536)
            .map_err(capture_inventory_failure)?;
        let mut result = Self {
            writes: BTreeMap::new(),
            arrivals: BTreeMap::new(),
        };
        for block in &machine.blocks {
            for (ordinal, operation) in block.operations.iter().enumerate() {
                if let Some(writes) = scalar_storage_writes(
                    module,
                    |root| place_types.get(root).copied().flatten(),
                    operation,
                ) {
                    let position = (
                        block.id,
                        u32::try_from(ordinal).map_err(
                            |_| "scalar write ordinal exceeds its canonical position",
                        )?,
                    );
                    let writes = writes.and_then(|rows| {
                        rows.into_iter()
                            .map(|(location, actual)| {
                                let scalar_type =
                                    types.get(actual).copied().flatten().ok_or(
                                        "scalar writer has no evaluated operand binder",
                                    )?;
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
            for (ordinal, binding) in
                crate::control_graph::forwarded_bindings(block).enumerate()
            {
                let target = blocks
                    .get(binding.target)
                    .copied()
                    .flatten()
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
                            let scalar_type = types
                                .get(*actual)
                                .copied()
                                .flatten()
                                .ok_or("scalar arrival has no actual operand binder")?;
                            if scalar_type != formal.scalar_type {
                                return Err(
                                    "scalar arrival changes its actual operand type",
                                );
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
                    let root = place_types
                        .get(*source)
                        .copied()
                        .flatten()
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
                u32::try_from(ordinal).map_err(
                    |_| "resident capture ordinal exceeds its canonical position",
                )?,
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

fn place_type_index(
    machine: &TerminalMachine,
) -> Result<IdTable<PlaceId, Option<StructuralTypeId>>, &'static str> {
    let mut type_rows = Vec::new();
    let mut parameter_rows = Vec::new();
    for (ordinal, parameter) in machine.structural_parameters.iter().enumerate() {
        if usize::try_from(parameter.position).ok() != Some(ordinal) {
            return Err("scalar capture signature parameter has no dense coordinate");
        }
        type_rows.push((parameter.place, Some(parameter.structural_type)));
        parameter_rows.push((
            parameter.place,
            Some(StructuralPlaceKind::Parameter {
                position: parameter.position,
            }),
        ));
    }
    for block in &machine.blocks {
        for (ordinal, parameter) in block.structural_parameters.iter().enumerate() {
            if usize::try_from(parameter.position).ok() != Some(ordinal) {
                return Err("scalar capture block parameter has no dense coordinate");
            }
            type_rows.push((parameter.place, Some(parameter.structural_type)));
            parameter_rows.push((
                parameter.place,
                Some(StructuralPlaceKind::BlockParameter {
                    block: block.id,
                    position: parameter.position,
                }),
            ));
        }
    }
    parameter_rows.sort_by_key(|(id, _)| *id);
    let parameter_kinds = IdTable::from_rows_with_gaps(parameter_rows, 65_536)
        .map_err(capture_inventory_failure)?;
    let mut declared = machine
        .structural_places
        .iter()
        .map(|place| place.id)
        .collect::<Vec<_>>();
    declared.sort_unstable();
    if declared.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("scalar capture roster repeats a structural place");
    }
    for place in &machine.structural_places {
        if parameter_kinds
            .get(place.id)
            .and_then(Option::as_ref)
            .is_some_and(|kind| *kind != place.kind)
        {
            return Err("scalar capture parameter changes its declared storage kind");
        }
        match place.kind {
            kind @ (StructuralPlaceKind::Parameter { .. }
            | StructuralPlaceKind::BlockParameter { .. }) => {
                if parameter_kinds.get(place.id).and_then(Option::as_ref) != Some(&kind) {
                    return Err("scalar capture place changes its parameter coordinate");
                }
            }
            StructuralPlaceKind::Result => {
                let result = machine
                    .result
                    .structural()
                    .filter(|result| result.place == place.id)
                    .ok_or(
                        "scalar capture result place has no matching result signature",
                    )?;
                type_rows.push((place.id, Some(result.structural_type)));
            }
            StructuralPlaceKind::ProviderAttachment { .. } => {}
            kind => {
                let carrier = kind
                    .produced_structural_type()
                    .ok_or("scalar capture producer place has no carrier")?;
                type_rows.push((place.id, Some(carrier)));
            }
        }
    }
    if parameter_kinds
        .iter()
        .any(|(place, kind)| kind.is_some() && declared.binary_search(&place).is_err())
    {
        return Err("scalar capture parameter has no declared storage place");
    }
    type_rows.sort_by_key(|(id, _)| *id);
    IdTable::from_rows_with_gaps(type_rows, 65_536).map_err(capture_inventory_failure)
}

fn capture_inventory_failure(reason: IdTableBuildError) -> &'static str {
    match reason {
        IdTableBuildError::RepeatedOrUnordered { .. } => {
            "scalar capture inventory has repeated or unordered identities"
        }
        IdTableBuildError::SpanOverflow => {
            "scalar capture inventory identity span overflows"
        }
        IdTableBuildError::SpanLimit { .. } => {
            "scalar capture inventory identity span exceeds its allocation bound"
        }
        IdTableBuildError::AllocationFailed => "scalar capture inventory allocation failed",
    }
}

pub(crate) fn scalar_storage_writes(
    module: &TerminalModule,
    place_type: impl Fn(PlaceId) -> Option<StructuralTypeId>,
    operation: &Operation,
) -> Option<Result<Vec<(DescriptorLocation, ValueId)>, &'static str>> {
    let mut writes = Vec::new();
    let mut write = |root, path: &[terminal_psi::StructuralPathSegment], field, value| {
        let root_type =
            place_type(root).ok_or("scalar writer has no declared destination carrier")?;
        let location =
            descriptor_location(&module.structural_types, root, root_type, path, field)
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
                        .ok_or(
                            "case initializer changes its declared scalar field roster",
                        )?;
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
mod tests;
