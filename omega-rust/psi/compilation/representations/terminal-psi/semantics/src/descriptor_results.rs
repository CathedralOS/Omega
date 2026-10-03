//! Returned descriptors borrow their original backing, never their carrier home.
//!
//! Reconstruct origins from the current bodies, including ordinary callees and
//! simultaneous block bindings. These are analysis results, not producer claims.
//! Operation verification still checks access, initialization, bounds and order.
//! An unresolved or recursive origin supplies no permission to escape storage.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{MachineId, PlaceId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, Operation, OperationKind, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalMachine, TerminalModule, Terminator,
};

use crate::borrowed_storage::Projection;

#[derive(Clone)]
pub enum DescriptorResultOrigin {
    Parameter {
        position: usize,
        projection: Projection,
    },
    Literal {
        machine: MachineId,
        place: PlaceId,
    },
}

#[derive(Clone)]
pub enum DescriptorCallOrigin {
    Argument {
        source: PlaceId,
        projection: Projection,
    },
    Literal {
        machine: MachineId,
        place: PlaceId,
    },
}

/// Shape only: access, multiplicity and origin remain separate judgments.
pub fn is_shared_descriptor<'a>(
    types: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
) -> bool {
    let mut declarations = types.into_iter().filter(|item| item.id == structural_type);
    let Some(declaration) = declarations.next() else {
        return false;
    };
    declarations.next().is_none()
        && matches!(
            declaration.shape,
            StructuralTypeShape::ElementView { .. }
                | StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: None | Some(StructuralAccess::SharedBorrow),
                })
        )
}

/// Every possible backing of a normal shared descriptor result. A literal has
/// module residence; a parameter position denotes the backing supplied there.
pub fn result_origins(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Option<Vec<DescriptorResultOrigin>> {
    let result = machine.result.structural()?;
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !is_shared_descriptor(module.structural_types.iter(), result.structural_type)
    {
        return None;
    }
    reconstruct_result_origins(
        &module.structural_types,
        &terminal_graph(module),
        machine.id,
    )
}

/// The exact returned occurrence must itself resolve to nonlocal storage.
pub fn return_origins(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
) -> Option<Vec<DescriptorResultOrigin>> {
    let graph = terminal_graph(module);
    let returns = graph
        .iter()
        .find(|row| row.id == machine.id)?
        .returns
        .as_ref()?;
    if !returns.contains(&source) {
        return None;
    }
    Reconstruction::new(&module.structural_types, &graph).returned(machine.id, source, 0)
}

/// Substitute the callee's formal backing with this call's actual projections.
/// Literal origins retain their callee namespace and never become caller places.
pub fn call_origins(
    module: &TerminalModule,
    caller: &TerminalMachine,
    operation: &Operation,
) -> Option<Vec<DescriptorCallOrigin>> {
    if !matches!(
        operation.kind,
        OperationKind::CallStructural { .. }
            | OperationKind::CallStructuralWithScalarArguments { .. }
    ) {
        return None;
    }
    let result = operation.result.structural()?;
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !is_shared_descriptor(module.structural_types.iter(), result.structural_type)
    {
        return None;
    }

    reconstruct_call_origins(
        &module.structural_types,
        &terminal_graph(module),
        caller.id,
        result.place,
    )
}

/// Current-program adapter records. These are disposable analysis inputs, not
/// retained origin authority. Adapters authenticate signatures and producers.
pub struct DescriptorMachine {
    pub id: MachineId,
    pub returns: Option<Vec<PlaceId>>,
    pub places: Vec<DescriptorPlace>,
}

pub struct DescriptorPlace {
    pub place: PlaceId,
    pub definition: DescriptorDefinition,
}

pub enum DescriptorDefinition {
    Parameter {
        position: usize,
        access: StructuralAccess,
        structural_type: StructuralTypeId,
    },
    Local,
    Literal,
    Alias(Vec<DescriptorSource>),
    Call {
        callee: MachineId,
        arguments: Vec<StructuralArgument>,
    },
}

pub struct DescriptorSource {
    pub source: PlaceId,
    pub projection: Projection,
}

pub fn reconstruct_result_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    machine: MachineId,
) -> Option<Vec<DescriptorResultOrigin>> {
    Reconstruction::new(types, machines).result(machine, 0)
}

pub fn reconstruct_call_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    caller: MachineId,
    result: PlaceId,
) -> Option<Vec<DescriptorCallOrigin>> {
    Reconstruction::new(types, machines).call(caller, result, 0)
}

struct Reconstruction<'a> {
    types: &'a [StructuralTypeDeclaration],
    machines: &'a [DescriptorMachine],
    summaries: BTreeMap<MachineId, Option<Vec<DescriptorResultOrigin>>>,
}

impl<'a> Reconstruction<'a> {
    fn new(types: &'a [StructuralTypeDeclaration], machines: &'a [DescriptorMachine]) -> Self {
        Self {
            types,
            machines,
            summaries: BTreeMap::new(),
        }
    }

    fn machine(&self, identity: MachineId) -> Option<&'a DescriptorMachine> {
        let mut matching = self
            .machines
            .iter()
            .filter(|machine| machine.id == identity);
        let machine = matching.next()?;
        matching.next().is_none().then_some(machine)
    }

    fn definition(&self, machine: MachineId, place: PlaceId) -> Option<&'a DescriptorDefinition> {
        let mut matching = self
            .machine(machine)?
            .places
            .iter()
            .filter(|row| row.place == place);
        let row = matching.next()?;
        matching.next().is_none().then_some(&row.definition)
    }

    fn result(&mut self, identity: MachineId, depth: usize) -> Option<Vec<DescriptorResultOrigin>> {
        if let Some(summary) = self.summaries.get(&identity) {
            return summary.clone();
        }
        self.summaries.insert(identity, None);
        if depth >= 128 {
            return None;
        }
        let returns = self.machine(identity)?.returns.as_ref()?;
        if returns.is_empty() {
            return None;
        }
        let mut result = Vec::new();
        for source in returns {
            result.extend(self.returned(identity, *source, depth + 1)?);
        }
        self.summaries.insert(identity, Some(result.clone()));
        Some(result)
    }

    fn returned(
        &mut self,
        machine: MachineId,
        source: PlaceId,
        depth: usize,
    ) -> Option<Vec<DescriptorResultOrigin>> {
        let origins = self.source(
            machine,
            source,
            Projection::new(&[]),
            &mut BTreeSet::new(),
            depth,
        )?;
        origins
            .into_iter()
            .map(|origin| match origin {
                DescriptorCallOrigin::Literal { machine, place } => {
                    Some(DescriptorResultOrigin::Literal { machine, place })
                }
                DescriptorCallOrigin::Argument {
                    source,
                    mut projection,
                } => {
                    let DescriptorDefinition::Parameter {
                        position,
                        access,
                        structural_type,
                    } = self.definition(machine, source)?
                    else {
                        return None;
                    };
                    if !matches!(
                        access,
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    ) && !(*access == StructuralAccess::Owned
                        && external_backing(self.types, *structural_type, &projection.path))
                    {
                        return None;
                    }
                    // A stored descriptor's field names its carrier, not its
                    // external referent. Without an exact backing join it may
                    // alias other incoming storage.
                    if !projection.path.is_empty()
                        && external_backing(self.types, *structural_type, &projection.path)
                    {
                        projection.indirect = true;
                    }
                    Some(DescriptorResultOrigin::Parameter {
                        position: *position,
                        projection: projection.without_runtime_coordinates(),
                    })
                }
            })
            .collect()
    }

    fn call(
        &mut self,
        caller: MachineId,
        result: PlaceId,
        depth: usize,
    ) -> Option<Vec<DescriptorCallOrigin>> {
        let DescriptorDefinition::Call { callee, arguments } = self.definition(caller, result)?
        else {
            return None;
        };
        let callee = *callee;
        self.result(callee, depth + 1)?
            .into_iter()
            .map(|origin| match origin {
                DescriptorResultOrigin::Literal { machine, place } => {
                    Some(DescriptorCallOrigin::Literal { machine, place })
                }
                DescriptorResultOrigin::Parameter {
                    position,
                    projection,
                } => {
                    let actual = arguments.get(position)?;
                    if actual.access == StructuralAccess::WriteOnlyBorrow {
                        return None;
                    }
                    Some(DescriptorCallOrigin::Argument {
                        source: actual.place,
                        projection: Projection::new(&actual.path).then(&projection),
                    })
                }
            })
            .collect()
    }

    fn source(
        &mut self,
        machine: MachineId,
        place: PlaceId,
        projection: Projection,
        active: &mut BTreeSet<PlaceId>,
        depth: usize,
    ) -> Option<Vec<DescriptorCallOrigin>> {
        if depth >= 128 || !active.insert(place) {
            return None;
        }
        let result = self.source_inner(machine, place, projection, active, depth);
        active.remove(&place);
        result
    }

    fn source_inner(
        &mut self,
        machine: MachineId,
        place: PlaceId,
        projection: Projection,
        active: &mut BTreeSet<PlaceId>,
        depth: usize,
    ) -> Option<Vec<DescriptorCallOrigin>> {
        match self.definition(machine, place)? {
            DescriptorDefinition::Literal => {
                Some(vec![DescriptorCallOrigin::Literal { machine, place }])
            }
            DescriptorDefinition::Parameter { .. } | DescriptorDefinition::Local => {
                Some(vec![DescriptorCallOrigin::Argument {
                    source: place,
                    projection,
                }])
            }
            DescriptorDefinition::Alias(sources) => {
                if sources.is_empty() {
                    return None;
                }
                let mut result = Vec::new();
                for source in sources {
                    result.extend(self.source(
                        machine,
                        source.source,
                        source.projection.then(&projection),
                        active,
                        depth + 1,
                    )?);
                }
                Some(result)
            }
            DescriptorDefinition::Call { .. } => {
                let mut result = Vec::new();
                for origin in self.call(machine, place, depth + 1)? {
                    match origin {
                        DescriptorCallOrigin::Argument {
                            source,
                            projection: prefix,
                        } => result.extend(self.source(
                            machine,
                            source,
                            prefix.then(&projection),
                            active,
                            depth + 1,
                        )?),
                        literal @ DescriptorCallOrigin::Literal { .. } => result.push(literal),
                    }
                }
                Some(result)
            }
        }
    }
}

fn external_backing(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> bool {
    (0..=path.len()).any(|length| {
        let Some(declaration) =
            crate::runtime_structural_path_tip(types.iter(), root, &path[..length])
        else {
            return false;
        };
        if matches!(
            declaration.shape,
            StructuralTypeShape::Reference {
                access: StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow,
                ..
            } | StructuralTypeShape::ElementView { .. }
                | StructuralTypeShape::ByteSequence(
                    ByteSequenceCarrier::BorrowedView {
                        access: None
                            | Some(
                                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                            )
                    } | ByteSequenceCarrier::BoundedView {
                        access: None
                            | Some(
                                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                            ),
                        ..
                    }
                )
        ) {
            return true;
        }
        let Some(StructuralPathSegment::Field(identity)) = path.get(length) else {
            return false;
        };
        let (StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. }) =
            &declaration.shape
        else {
            return false;
        };
        fields
            .iter()
            .find(|field| field.identity == *identity && !field.relevance.is_erased())
            .and_then(|field| field.field_type.byte_sequence_carrier())
            .is_some_and(|carrier| {
                matches!(
                    carrier,
                    ByteSequenceCarrier::BorrowedView {
                        access: None
                            | Some(
                                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                            )
                    } | ByteSequenceCarrier::BoundedView {
                        access: None
                            | Some(
                                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                            ),
                        ..
                    }
                )
            })
    })
}

fn place_type(machine: &TerminalMachine, place: PlaceId) -> Option<StructuralTypeId> {
    machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters)
                .find(|parameter| parameter.place == place)
                .map(|parameter| parameter.structural_type)
        })
        .or_else(|| {
            machine
                .structural_places
                .iter()
                .find_map(|row| match row.kind {
                    StructuralPlaceKind::OperationResult {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    } if row.id == place => Some(structural_type),
                    _ => None,
                })
        })
}

fn terminal_graph(module: &TerminalModule) -> Vec<DescriptorMachine> {
    module
        .machines
        .iter()
        .map(|machine| DescriptorMachine {
            id: machine.id,
            returns: terminal_returns(module, machine),
            places: machine
                .structural_places
                .iter()
                .filter_map(|place| {
                    Some(DescriptorPlace {
                        place: place.id,
                        definition: terminal_definition(module, machine, place.id, place.kind)?,
                    })
                })
                .collect(),
        })
        .collect()
}

fn terminal_returns(module: &TerminalModule, machine: &TerminalMachine) -> Option<Vec<PlaceId>> {
    let result = machine.result.structural()?;
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !is_shared_descriptor(module.structural_types.iter(), result.structural_type)
    {
        return None;
    }
    let mut pending = vec![machine.entry];
    let mut visited = BTreeSet::new();
    let mut returns = Vec::new();
    while let Some(identity) = pending.pop() {
        if !visited.insert(identity) {
            continue;
        }
        let block = machine.blocks.iter().find(|block| block.id == identity)?;
        match &block.terminator {
            Terminator::ReturnStructural { source, .. } => {
                if place_type(machine, *source)? != result.structural_type {
                    return None;
                }
                returns.push(*source);
            }
            Terminator::Jump { target, .. } => pending.push(*target),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => pending.extend([when_true.target, when_false.target]),
            Terminator::StructuralCase { cases, .. } => {
                pending.extend(cases.iter().map(|case| case.target))
            }
            Terminator::Crash { .. } => {}
            _ => return None,
        }
    }
    Some(returns)
}

fn terminal_definition(
    module: &TerminalModule,
    machine: &TerminalMachine,
    place: PlaceId,
    kind: StructuralPlaceKind,
) -> Option<DescriptorDefinition> {
    match kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            let (index, parameter) =
                machine
                    .structural_parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| {
                        parameter.place == place
                            && parameter.position == position
                            && parameter.is_self == is_self
                    })?;
            Some(DescriptorDefinition::Parameter {
                position: index,
                access: parameter.access,
                structural_type: parameter.structural_type,
            })
        }
        StructuralPlaceKind::ByteSequenceLiteral { .. } => {
            let mut establishments = machine.blocks.iter().flat_map(|block| &block.operations).filter(|operation|
                matches!(operation.kind, OperationKind::EstablishByteSequenceLiteral { destination, .. } if destination == place));
            establishments.next()?;
            establishments
                .next()
                .is_none()
                .then_some(DescriptorDefinition::Literal)
        }
        StructuralPlaceKind::BlockParameter { block, position } => {
            let destination = machine
                .blocks
                .iter()
                .find(|candidate| candidate.id == block)?;
            if destination
                .structural_parameters
                .get(position as usize)?
                .place
                != place
            {
                return None;
            }
            let mut incoming = Vec::new();
            for predecessor in &machine.blocks {
                let mut bind = |target, arguments: &[StructuralArgument]| -> Option<()> {
                    if target == block {
                        if arguments.len() != destination.structural_parameters.len() {
                            return None;
                        }
                        incoming.push(argument_source(&arguments[position as usize])?);
                    }
                    Some(())
                };
                match &predecessor.terminator {
                    Terminator::Jump {
                        target,
                        structural_arguments,
                        ..
                    } => bind(*target, structural_arguments)?,
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        bind(when_true.target, &when_true.structural_arguments)?;
                        bind(when_false.target, &when_false.structural_arguments)?;
                    }
                    Terminator::StructuralCase { cases, .. }
                        if cases.iter().any(|case| case.target == block) =>
                    {
                        return None;
                    }
                    _ => {}
                }
            }
            Some(DescriptorDefinition::Alias(incoming))
        }
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            let mut matching = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| operation.id == producer);
            let operation = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            let result = operation.result.structural()?;
            if result.place != place || result.structural_type != structural_type {
                return None;
            }
            match &operation.kind {
                OperationKind::EstablishElementView { source, .. }
                | OperationKind::EstablishMutableElementView { source, .. }
                | OperationKind::EstablishReference { source } => {
                    Some(DescriptorDefinition::Alias(vec![argument_source(source)?]))
                }
                OperationKind::StructuralLeafCopy { source, path }
                    if result.multiplicity == StructuralMultiplicity::Unrestricted
                        && is_shared_descriptor(
                            module.structural_types.iter(),
                            result.structural_type,
                        ) =>
                {
                    let mut projection = Projection::new(path);
                    projection.indirect = !path.is_empty();
                    Some(DescriptorDefinition::Alias(vec![DescriptorSource {
                        source: *source,
                        projection,
                    }]))
                }
                OperationKind::ByteSequenceSubslice { source, .. }
                | OperationKind::ElementViewSubslice { source, .. } => {
                    // Relative callee endpoints cannot cross scalar namespaces.
                    let mut projection = Projection::new(&[]);
                    projection.exact = false;
                    Some(DescriptorDefinition::Alias(vec![DescriptorSource {
                        source: *source,
                        projection,
                    }]))
                }
                OperationKind::CallStructural {
                    callee,
                    structural_arguments,
                    ..
                }
                | OperationKind::CallStructuralWithScalarArguments {
                    callee,
                    structural_arguments,
                    ..
                } if is_shared_descriptor(
                    module.structural_types.iter(),
                    result.structural_type,
                ) =>
                {
                    let mut matching = module
                        .machines
                        .iter()
                        .filter(|machine| machine.id == *callee);
                    let callee = matching.next()?;
                    if matching.next().is_some() {
                        return None;
                    }
                    let signature = callee.result.structural()?;
                    if structural_arguments.len() != callee.structural_parameters.len()
                        || result.structural_type != signature.structural_type
                        || result.multiplicity != signature.multiplicity
                        || result.qualifications != signature.qualifications
                        || result.projected_qualifications != signature.projected_qualifications
                        || !result.claims.is_empty()
                    {
                        return None;
                    }
                    Some(DescriptorDefinition::Call {
                        callee: callee.id,
                        arguments: structural_arguments.clone(),
                    })
                }
                _ => Some(DescriptorDefinition::Local),
            }
        }
        _ => Some(DescriptorDefinition::Local),
    }
}

fn argument_source(argument: &StructuralArgument) -> Option<DescriptorSource> {
    (argument.access != StructuralAccess::WriteOnlyBorrow).then(|| DescriptorSource {
        source: argument.place,
        projection: Projection::new(&argument.path),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine(value: u64) -> MachineId {
        MachineId::new(value).unwrap()
    }
    fn place(value: u64) -> PlaceId {
        PlaceId::new(value).unwrap()
    }
    fn view_type() -> StructuralTypeId {
        StructuralTypeId::new(1).unwrap()
    }
    fn types() -> Vec<StructuralTypeDeclaration> {
        vec![StructuralTypeDeclaration {
            id: view_type(),
            identity: "bytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: None,
            }),
        }]
    }
    fn parameter(position: usize, access: StructuralAccess) -> DescriptorPlace {
        DescriptorPlace {
            place: place(position as u64 + 1),
            definition: DescriptorDefinition::Parameter {
                position,
                access,
                structural_type: view_type(),
            },
        }
    }
    fn forwarder(identity: u64) -> DescriptorMachine {
        DescriptorMachine {
            id: machine(identity),
            returns: Some(vec![place(1)]),
            places: vec![parameter(0, StructuralAccess::SharedBorrow)],
        }
    }
    fn call(result: u64, target: u64, actual: u64) -> DescriptorPlace {
        DescriptorPlace {
            place: place(result),
            definition: DescriptorDefinition::Call {
                callee: machine(target),
                arguments: vec![StructuralArgument {
                    place: place(actual),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                }],
            },
        }
    }

    #[test]
    fn nested_descriptor_returns_substitute_original_actual_backing() {
        let mut middle = forwarder(2);
        middle.returns = Some(vec![place(2)]);
        middle.places.push(call(2, 1, 1));
        let caller = DescriptorMachine {
            id: machine(3),
            returns: None,
            places: vec![
                DescriptorPlace {
                    place: place(9),
                    definition: DescriptorDefinition::Local,
                },
                call(10, 2, 9),
            ],
        };
        let graph = [forwarder(1), middle, caller];
        let origins = reconstruct_call_origins(&types(), &graph, machine(3), place(10)).unwrap();
        assert!(
            matches!(origins.as_slice(), [DescriptorCallOrigin::Argument { source, projection }]
            if *source == place(9) && projection.path.is_empty())
        );
        let origins = reconstruct_result_origins(&types(), &graph, machine(2)).unwrap();
        assert!(matches!(
            origins.as_slice(),
            [DescriptorResultOrigin::Parameter { position: 0, .. }]
        ));
    }

    #[test]
    fn returning_local_backing_rejects_even_through_a_valid_forwarder() {
        let caller = DescriptorMachine {
            id: machine(2),
            returns: Some(vec![place(10)]),
            places: vec![
                DescriptorPlace {
                    place: place(9),
                    definition: DescriptorDefinition::Local,
                },
                call(10, 1, 9),
            ],
        };
        assert!(
            reconstruct_result_origins(&types(), &[forwarder(1), caller], machine(2)).is_none()
        );
        let owned = DescriptorMachine {
            id: machine(3),
            returns: Some(vec![place(1)]),
            places: vec![DescriptorPlace {
                place: place(1),
                definition: DescriptorDefinition::Parameter {
                    position: 0,
                    access: StructuralAccess::Owned,
                    structural_type: StructuralTypeId::new(2).unwrap(),
                },
            }],
        };
        assert!(reconstruct_result_origins(&types(), &[owned], machine(3)).is_none());
    }

    #[test]
    fn descriptor_custody_and_static_backing_can_outlive_their_carrier() {
        let owned_descriptor = DescriptorMachine {
            id: machine(1),
            returns: Some(vec![place(1)]),
            places: vec![parameter(0, StructuralAccess::Owned)],
        };
        assert!(reconstruct_result_origins(&types(), &[owned_descriptor], machine(1)).is_some());
        let literal = DescriptorMachine {
            id: machine(2),
            returns: Some(vec![place(1)]),
            places: vec![DescriptorPlace {
                place: place(1),
                definition: DescriptorDefinition::Literal,
            }],
        };
        let caller = DescriptorMachine {
            id: machine(3),
            returns: Some(vec![place(1)]),
            places: vec![DescriptorPlace {
                place: place(1),
                definition: DescriptorDefinition::Call {
                    callee: machine(2),
                    arguments: Vec::new(),
                },
            }],
        };
        let origins = reconstruct_result_origins(&types(), &[literal, caller], machine(3)).unwrap();
        assert!(
            matches!(origins.as_slice(), [DescriptorResultOrigin::Literal { machine: identity, place: source }]
            if *identity == machine(2) && *source == place(1))
        );
    }

    #[test]
    fn substituted_sources_and_recursive_claims_cannot_invent_provenance() {
        let mut callee = forwarder(1);
        callee
            .places
            .push(parameter(1, StructuralAccess::SharedBorrow));
        callee.returns = Some(vec![place(2)]);
        let caller = DescriptorMachine {
            id: machine(2),
            returns: None,
            places: vec![DescriptorPlace {
                place: place(5),
                definition: DescriptorDefinition::Call {
                    callee: machine(1),
                    arguments: vec![
                        StructuralArgument {
                            place: place(7),
                            path: Vec::new(),
                            access: StructuralAccess::SharedBorrow,
                        },
                        StructuralArgument {
                            place: place(8),
                            path: Vec::new(),
                            access: StructuralAccess::SharedBorrow,
                        },
                    ],
                },
            }],
        };
        let origins =
            reconstruct_call_origins(&types(), &[callee, caller], machine(2), place(5)).unwrap();
        assert!(
            matches!(origins.as_slice(), [DescriptorCallOrigin::Argument { source, .. }] if *source == place(8))
        );
        let mut recursive = forwarder(3);
        recursive.returns = Some(vec![place(2)]);
        recursive.places.push(call(2, 3, 1));
        assert!(reconstruct_result_origins(&types(), &[recursive], machine(3)).is_none());
    }
}
