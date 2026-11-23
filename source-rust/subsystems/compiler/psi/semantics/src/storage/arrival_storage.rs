use crate::exactly_one::exactly_one;
use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::StructuralTypeShape;
use terminal_psi::{Block, StructuralParameterDeclaration, TerminalMachine};

pub fn view_descriptor_projected_qualifications<'types>(
    declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration>,
    root: semantic_vocabulary::StructuralTypeId,
    projected: &[terminal_psi::StructuralPathQualification],
) -> bool {
    if projected.is_empty() {
        return true;
    }
    let mut matching = declarations.filter(|row| row.id == root);
    let Some(declaration) = matching.next() else {
        return false;
    };
    matching.next().is_none()
        && matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
        && projected.iter().all(|row| row.is_element_membership())
}

pub fn view_descriptor_parameter<'types>(
    parameter: &StructuralParameterDeclaration,
    declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    if !matches!(
        parameter.access,
        terminal_psi::reading_borrow_pair_pattern!()
    ) || !matches!(
        parameter.multiplicity,
        terminal_psi::StructuralMultiplicity::Unrestricted
            | terminal_psi::StructuralMultiplicity::Affine
    ) {
        return false;
    }
    let mut matching =
        declarations.filter(|declaration| declaration.id == parameter.structural_type);
    let Some(declaration) = matching.next() else {
        return false;
    };
    matching.next().is_none()
        && (parameter.projected_qualifications.is_empty()
            || matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
                && parameter
                    .projected_qualifications
                    .iter()
                    .all(|row| row.is_element_membership()))
        && (declaration.shape.is_canonical_view_descriptor()
            || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                && crate::descriptors::descriptor_results::descriptor_value_access(
                    std::iter::once(declaration),
                    parameter.structural_type,
                    parameter.multiplicity,
                )
                .is_some_and(|ceiling| parameter.access.admits_referent(ceiling)))
}

pub fn borrowed_storage_parameter<'types>(
    parameter: &StructuralParameterDeclaration,
    declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    if parameter.access == terminal_psi::StructuralAccess::Owned
        || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
    {
        return false;
    }
    let mut matching = declarations.filter(|row| row.id == parameter.structural_type);
    let Some(declaration) = matching.next() else {
        return false;
    };
    matching.next().is_none() && !declaration.shape.is_canonical_view_descriptor()
}

/// A loan of a writable descriptor slot keeps the slot's parent
/// occurrence. A direct view lends the original payload instead.
pub fn borrowed_view_payload_parameter<'types>(
    parameter: &StructuralParameterDeclaration,
    declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    let Some(declaration) =
        exactly_one(declarations, |row| row.id == parameter.structural_type)
    else {
        return false;
    };
    declaration.shape.is_view_descriptor()
        && (crate::stored_reference_access(&declaration.shape)
            != Some(terminal_psi::StructuralAccess::MutableBorrow)
            || !borrowed_storage_parameter(parameter, std::iter::once(declaration)))
}

pub fn borrowed_storage_reference_ceiling<'types>(
    parameter: &StructuralParameterDeclaration,
    mut declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
) -> Option<terminal_psi::StructuralAccess> {
    use terminal_psi::StructuralAccess;
    if !borrowed_storage_parameter(parameter, declarations.clone()) {
        return None;
    }
    let declaration = declarations.find(|row| row.id == parameter.structural_type)?;
    let inner = crate::stored_reference_access(&declaration.shape)?;
    match parameter.access {
        StructuralAccess::SharedBorrow if inner != StructuralAccess::WriteOnlyBorrow => {
            Some(StructuralAccess::SharedBorrow)
        }
        StructuralAccess::MutableBorrow => Some(inner),
        _ => None,
    }
}

pub fn descriptor_parameter_access<'types>(
    parameter: &StructuralParameterDeclaration,
    declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
) -> Option<terminal_psi::StructuralAccess> {
    if parameter.access == terminal_psi::StructuralAccess::Owned {
        if !view_descriptor_projected_qualifications(
            declarations.clone(),
            parameter.structural_type,
            &parameter.projected_qualifications,
        ) {
            return None;
        }
        crate::descriptors::descriptor_results::descriptor_value_access(
            declarations,
            parameter.structural_type,
            parameter.multiplicity,
        )
    } else if view_descriptor_parameter(parameter, declarations.clone()) {
        Some(parameter.access)
    } else if declarations.clone().any(|row| {
        row.id == parameter.structural_type
            && crate::references::reference_views::descriptor_declared_access(&row.shape)
                .is_some()
    }) {
        borrowed_storage_reference_ceiling(parameter, declarations)
    } else {
        None
    }
}

pub fn arrival_parameter(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<&StructuralParameterDeclaration> {
    let place_declaration = exactly_one(machine.structural_places.iter(), |declaration| {
        declaration.id == place
    })?;

    match place_declaration.kind {
        StructuralPlaceKind::Parameter { position } => {
            let parameter = machine.structural_parameters.get(position as usize)?;
            (parameter.place == place
                && parameter.position == position
                && machine
                    .structural_parameters
                    .iter()
                    .filter(|candidate| candidate.place == place)
                    .count()
                    == 1
                && machine.blocks.iter().all(|block| {
                    block
                        .structural_parameters
                        .iter()
                        .all(|candidate| candidate.place != place)
                }))
            .then_some(parameter)
        }
        StructuralPlaceKind::BlockParameter { block, position } => {
            if block == machine.entry
                || machine
                    .structural_parameters
                    .iter()
                    .any(|candidate| candidate.place == place)
            {
                return None;
            }
            let owner = exactly_one(machine.blocks.iter(), |candidate| candidate.id == block)?;
            let parameter = owner.structural_parameters.get(position as usize)?;
            (parameter.place == place
                && parameter.position == position
                && machine
                    .blocks
                    .iter()
                    .flat_map(|candidate| &candidate.structural_parameters)
                    .filter(|candidate| candidate.place == place)
                    .count()
                    == 1)
                .then_some(parameter)
        }
        _ => None,
    }
}

/// Read the existing root's type, access and qualifications for argument
/// presentation. This grants no loan: resource claims and result
/// qualification establishments still require the consumer's own checks.
/// A non-parameter source has no parameter position, so its caller supplies
/// that label. Literal roots own an unrestricted descriptor value, never the
/// immutable image backing it denotes. Availability and live custody remain
/// separate obligations for every consumer of this metadata.
pub fn structural_argument_source(
    machine: &TerminalMachine,
    place: PlaceId,
    result_position: u32,
) -> Option<std::borrow::Cow<'_, StructuralParameterDeclaration>> {
    if let Some(parameter) = arrival_parameter(machine, place) {
        return Some(std::borrow::Cow::Borrowed(parameter));
    }
    let root = exactly_one(machine.structural_places.iter(), |root| root.id == place)?;
    let (structural_type, multiplicity, qualifications, projected, additional) = match root.kind
    {
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            let operation =
                exactly_one(machine.operations(), |operation| operation.id == producer)?;
            let result = operation.result.as_structural().filter(|result| {
                result.place == place && result.structural_type == structural_type
            })?;
            (
                structural_type,
                result.multiplicity,
                result.qualifications.clone(),
                result.projected_qualifications.clone(),
                result.additional_input_qualifications.clone(),
            )
        }
        StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        } => {
            let operation = exactly_one(
                machine.operations(),
                |operation| matches!(operation.kind, terminal_psi::OperationKind::EstablishByteSequenceLiteral { destination, .. } if destination == place),
            )?;
            let terminal_psi::OperationKind::EstablishByteSequenceLiteral {
                qualifications,
                ..
            } = &operation.kind
            else {
                return None;
            };
            if operation.result != terminal_psi::OperationResult::Unit {
                return None;
            }
            (
                structural_type,
                terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications.clone(),
                Vec::new(),
                Vec::new(),
            )
        }
        StructuralPlaceKind::TrivialAffineLocal {
            structural_type, ..
        } => {
            // Unlike a literal's single producer, a local may be
            // established again after disposal. Its live-state judgment
            // decides whether the current contents can be transferred.
            let mut established = false;
            for operation in machine.operations().filter(|operation| {
                matches!(operation.kind, terminal_psi::OperationKind::EstablishTrivialAffineLocal { destination } if destination == place)
            }) {
                if operation.result != terminal_psi::OperationResult::Unit {
                    return None;
                }
                established = true;
            }
            if !established {
                return None;
            }
            (
                structural_type,
                terminal_psi::StructuralMultiplicity::Affine,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        }
        _ => return None,
    };
    Some(std::borrow::Cow::Owned(StructuralParameterDeclaration {
        place,
        position: result_position,
        structural_type,
        multiplicity,
        access: terminal_psi::StructuralAccess::Owned,
        qualifications,
        projected_qualifications: projected,
        additional_input_qualifications: additional,
    }))
}

/// The permission whose storage a projection actually reaches. Crossing
/// a reference uses that reference's permission, not ownership of its home.
pub fn structural_projection_authority(
    machine: &TerminalMachine,
    types: &[terminal_psi::StructuralTypeDeclaration],
    argument: &terminal_psi::StructuralArgument,
) -> Option<terminal_psi::StructuralAccess> {
    let source = structural_argument_source(machine, argument.place, 0)?;
    let projected_access = |path: &[terminal_psi::StructuralPathSegment], mut access| {
        let selected =
            crate::runtime_structural_path_tip(types.iter(), source.structural_type, path);
        let Some(selected) = selected else {
            if matches!(
                crate::runtime_read_projection_tip(types.iter(), source.structural_type, path,),
                Some(crate::Position::ByteElement)
            ) {
                return Some(access);
            }
            let carrier =
                crate::runtime_structural_field(types.iter(), source.structural_type, path)?
                    .field_type
                    .byte_sequence_carrier()?;
            return match carrier {
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(retained),
                }
                | terminal_psi::ByteSequenceCarrier::BoundedView {
                    access: Some(retained),
                    ..
                } => Some(retained),
                terminal_psi::ByteSequenceCarrier::BorrowedView { access: None }
                | terminal_psi::ByteSequenceCarrier::BoundedView { access: None, .. } => None,
                _ => Some(access),
            };
        };
        // A canonical direct view leaves permission on its declared formal.
        // Its carrier alone has no access to use in place of that loan.
        if selected.shape.is_canonical_view_descriptor()
            && path.is_empty()
            && let Some(parameter) = arrival_parameter(machine, argument.place)
            && parameter.access != terminal_psi::StructuralAccess::Owned
        {
            return descriptor_parameter_access(parameter, types.iter());
        }
        match &selected.shape {
            terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(retained),
                }
                | terminal_psi::ByteSequenceCarrier::BoundedView {
                    access: Some(retained),
                    ..
                },
            )
            | terminal_psi::StructuralTypeShape::ElementView {
                access: Some(retained),
                ..
            } => access = *retained,
            _ if crate::descriptor_results::is_descriptor(types.iter(), selected.id) => {
                access = crate::descriptor_results::descriptor_value_access(
                    types.iter(),
                    selected.id,
                    source.multiplicity,
                )?;
            }
            _ => {}
        }
        Some(access)
    };
    let mut access = source.access;
    for (position, step) in argument.path.iter().enumerate() {
        let prefix = &argument.path[..position];
        if *step == terminal_psi::StructuralPathSegment::Referent {
            let terminal_psi::StructuralTypeShape::Reference { access: parent, .. } =
                &crate::runtime_structural_path_tip(
                    types.iter(),
                    source.structural_type,
                    prefix,
                )?
                .shape
            else {
                return None;
            };
            access = *parent;
        } else if matches!(
            step,
            terminal_psi::StructuralPathSegment::FixedIndex(_)
                | terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
        ) {
            // Indexing reaches the descriptor's payload. Keep its real
            // access instead of the access to the descriptor's home.
            access = projected_access(prefix, access)?;
        }
    }
    projected_access(&argument.path, access)
}

pub fn root_before_edge(
    machine: &TerminalMachine,
    block: BlockId,
    predecessor: &Block,
    place: PlaceId,
) -> Option<PlaceId> {
    let projection = projection_before_edge(machine, block, predecessor, place)?;
    projection.path.is_empty().then_some(projection.root)
}

pub fn projection_before_edge<'edge>(
    machine: &TerminalMachine,
    block: BlockId,
    predecessor: &'edge Block,
    place: PlaceId,
) -> Option<terminal_psi::OperationProjection<'edge>> {
    let Some(position) = machine
        .structural_places
        .iter()
        .find_map(|row| match row.kind {
            StructuralPlaceKind::BlockParameter {
                block: owner,
                position,
            } if row.id == place && owner == block => Some(position),
            _ => None,
        })
    else {
        return Some(terminal_psi::OperationProjection {
            field: None,
            root: place,
            path: &[],
        });
    };
    let mut forwarded = None;
    for binding in crate::control_graph::forwarded_bindings(predecessor) {
        if binding.target != block {
            continue;
        }
        let argument = binding.structural_arguments.get(position as usize)?;
        let projection = terminal_psi::OperationProjection {
            field: None,
            root: argument.place,
            path: &argument.path,
        };
        if forwarded.is_some_and(|earlier| earlier != projection) {
            return None;
        }
        forwarded = Some(projection);
    }
    forwarded
}

pub struct StorageNames {
    behind: BTreeMap<BlockId, BTreeSet<PlaceId>>,
}

impl StorageNames {
    pub fn in_block(&self, block: BlockId) -> impl Iterator<Item = PlaceId> + '_ {
        self.behind.get(&block).into_iter().flatten().copied()
    }

    pub fn captured_roots(&self) -> impl Iterator<Item = PlaceId> + '_ {
        self.behind.values().flat_map(|roots| roots.iter().copied())
    }
}

/// The incoming blocks that can forward a storage name to each destination.
pub(crate) struct StoragePredecessors<'a> {
    machine: &'a TerminalMachine,
    incoming: Vec<(BlockId, usize)>,
}

impl<'a> StoragePredecessors<'a> {
    pub(crate) fn new(machine: &'a TerminalMachine) -> Self {
        let mut incoming: Vec<_> = machine
            .blocks
            .iter()
            .enumerate()
            .flat_map(|(source, block)| {
                crate::control_graph::forwarded_bindings(block)
                    .map(move |binding| (binding.target, source))
            })
            .collect();
        incoming.sort_unstable();
        // Several arms of one branch may reach the same block.
        incoming.dedup();
        Self { machine, incoming }
    }

    pub(crate) fn storage_names(&self, block: BlockId, root: PlaceId) -> StorageNames {
        let mut behind: BTreeMap<BlockId, BTreeSet<PlaceId>> = BTreeMap::new();
        let mut pending = vec![(block, root)];
        while let Some((block, root)) = pending.pop() {
            if !behind.entry(block).or_default().insert(root) {
                continue;
            }
            let first = self.incoming.partition_point(|&(target, _)| target < block);
            for &(_, source) in self.incoming[first..]
                .iter()
                .take_while(|&&(target, _)| target == block)
            {
                let predecessor = &self.machine.blocks[source];
                if let Some(before) = root_before_edge(self.machine, block, predecessor, root) {
                    pending.push((predecessor.id, before));
                }
            }
        }
        StorageNames { behind }
    }
}

pub fn storage_names(machine: &TerminalMachine, block: BlockId, root: PlaceId) -> StorageNames {
    StoragePredecessors::new(machine).storage_names(block, root)
}
