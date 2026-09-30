//! Exclusive whole-record state bindings keep the caller's storage identity.
//!
//! The ordinary mutable-parameter fixed point transfers the usable name on
//! each edge. This replay also follows borrowed block aliases so an earlier
//! shared name cannot observe a record after its exclusive successor took
//! custody. Calls reborrow the current name; they do not transfer it.

use super::super::borrowed_windows::window_aliases;
use super::super::{
    BTreeMap, BTreeSet, CanonicalStructuralPathSegment, ModuleError, OperationKind, PlaceId,
    StructuralAccess, StructuralParameterDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalModule, Terminator,
};

pub(crate) fn parameter_type(
    module: &TerminalModule,
    parameter: &StructuralParameterDeclaration,
) -> bool {
    parameter.access == StructuralAccess::MutableBorrow
        && module.structural_types.iter().any(|declaration| {
            declaration.id == parameter.structural_type
                && matches!(declaration.shape, StructuralTypeShape::Record { .. })
        })
        && super::super::record::plain_type(module, parameter.structural_type)
}

/// A fresh plain owned home may back a state loan without moving its bytes or
/// its affine obligation. Its producer and the frontier prove it is still live.
pub(crate) fn owned_home<'a>(
    module: &TerminalModule,
    machine: &'a TerminalMachine,
    place: PlaceId,
) -> Option<&'a terminal_psi::StructuralOperationResult> {
    let result = super::super::record::result(machine, place)?;
    (matches!(
        result.multiplicity,
        super::super::StructuralMultiplicity::Affine
            | super::super::StructuralMultiplicity::Unrestricted
    ) && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && module.structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && matches!(declaration.shape, StructuralTypeShape::Record { .. })
        })
        && super::super::record::plain_type(module, result.structural_type))
    .then_some(result)
}

/// Origin sets pair each storage root with the canonical prefix the borrowed
/// name delegates beneath it: edge arguments that forward a whole name carry
/// an empty prefix, while a projected loan names its exact subtree.
pub(crate) type BorrowedRecordOrigins =
    BTreeMap<PlaceId, BTreeSet<(PlaceId, Vec<CanonicalStructuralPathSegment>)>>;

pub(crate) fn aliases(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Option<BorrowedRecordOrigins> {
    if !machine
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .any(|parameter| parameter_type(module, parameter))
    {
        return None;
    }
    let mut origins: BorrowedRecordOrigins = window_aliases(module, machine);
    // These descriptors borrow their source storage; they are not fresh owned
    // payloads. Include their origin before closing block-parameter aliases,
    // which may already name a descriptor or one of its later subslices.
    for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
        let source = match &operation.kind {
            OperationKind::EstablishElementView { source, .. }
            | OperationKind::EstablishMutableElementView { source, .. } => source.place,
            OperationKind::ElementViewSubslice { source, .. } => *source,
            _ => continue,
        };
        if let Some(result) = operation.result.structural() {
            origins
                .entry(result.place)
                .or_default()
                .insert((source, Vec::new()));
        }
    }
    loop {
        let mut expanded = origins.clone();
        for (place, sources) in &origins {
            for (root, prefix) in sources {
                if let Some(ancestors) = origins.get(root) {
                    expanded
                        .entry(*place)
                        .or_default()
                        .extend(ancestors.iter().map(|(ancestor, ancestor_prefix)| {
                            let mut composed = ancestor_prefix.clone();
                            composed.extend_from_slice(prefix);
                            (*ancestor, composed)
                        }));
                }
            }
        }
        if expanded == origins {
            return Some(origins);
        }
        origins = expanded;
    }
}

fn same_storage(aliases: &BorrowedRecordOrigins, first: PlaceId, second: PlaceId) -> bool {
    // Distinct machine parameters retain the disjointness their caller
    // independently proves. Intermediate aliases remain in the sets so every
    // spelled descriptor and block binding shares its original whole root.
    // Exclusivity stays root-level: two names overlap whenever any delegated
    // region of one shares a storage root with the other.
    match (aliases.get(&first), aliases.get(&second)) {
        (Some(first), Some(second)) => !first
            .iter()
            .map(|(root, _)| *root)
            .collect::<BTreeSet<_>>()
            .is_disjoint(&second.iter().map(|(root, _)| *root).collect()),
        (Some(first), None) => first.iter().any(|(root, _)| *root == second),
        (None, Some(second)) => second.iter().any(|(root, _)| *root == first),
        (None, None) => first == second,
    }
}

fn usable_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    aliases: &BorrowedRecordOrigins,
    available: &BTreeSet<PlaceId>,
    source: PlaceId,
) -> bool {
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source)
        .or_else(|| super::parameter(machine, source));
    if (parameter.is_some_and(|parameter| parameter_type(module, parameter))
        || owned_home(module, machine, source).is_some())
        && !available.contains(&source)
    {
        return false;
    }
    !machine
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .any(|parameter| {
            parameter.place != source
                && parameter_type(module, parameter)
                && available.contains(&parameter.place)
                && same_storage(aliases, source, parameter.place)
        })
}

/// A read-only observation may still resolve through a suspended root name
/// when the whole region it reads lies inside the subtree one live exclusive
/// name delegates. The observation sees exactly the bytes that exclusive name
/// governs, so exclusivity is preserved: the same root still cannot move,
/// store, or loan the region until the loan dies or rejoins.
fn usable_observation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    aliases: &BorrowedRecordOrigins,
    available: &BTreeSet<PlaceId>,
    source: PlaceId,
    region: &[CanonicalStructuralPathSegment],
) -> bool {
    if usable_source(module, machine, aliases, available, source) {
        return true;
    }
    if region.is_empty() {
        return false;
    }
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .any(|parameter| {
            parameter.place != source
                && parameter_type(module, parameter)
                && available.contains(&parameter.place)
                && same_storage(aliases, source, parameter.place)
                && aliases.get(&parameter.place).is_some_and(|origins| {
                    origins.iter().any(|(root, prefix)| {
                        *root == source && !prefix.is_empty() && region.starts_with(prefix)
                    })
                })
        })
}

pub(crate) fn validate_operation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    aliases: &BorrowedRecordOrigins,
    available: &BTreeSet<PlaceId>,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let validate = |source| {
        if usable_source(module, machine, aliases, available, source) {
            Ok(())
        } else {
            Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: operation.id,
                place: source,
            })
        }
    };
    // Name-spelled field operations and call arguments share the existing
    // projection inventory. These canonical-path observations also read
    // records but deliberately do not belong to that runtime-index inventory.
    for projection in operation.kind.structural_projections() {
        validate(projection.root)?;
    }
    match &operation.kind {
        OperationKind::BooleanStructuralField {
            source,
            path,
            field,
        }
        | OperationKind::IntegerStructuralField {
            source,
            path,
            field,
        } => {
            let mut region = path.clone();
            region.push(CanonicalStructuralPathSegment::Field(*field));
            if !usable_observation(module, machine, aliases, available, *source, &region) {
                return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                    operation: operation.id,
                    place: *source,
                });
            }
        }
        OperationKind::StructuralCaseLeafCopy { source, path } => {
            if !usable_observation(module, machine, aliases, available, *source, path) {
                return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                    operation: operation.id,
                    place: *source,
                });
            }
        }
        OperationKind::ElementViewLength { source }
        | OperationKind::ElementViewRead { source, .. }
        | OperationKind::ElementViewSubslice { source, .. } => validate(*source)?,
        _ => {}
    }
    Ok(())
}

pub(crate) fn validate_terminator(
    module: &TerminalModule,
    machine: &TerminalMachine,
    aliases: &BorrowedRecordOrigins,
    available: &BTreeSet<PlaceId>,
    block: &terminal_psi::Block,
) -> Result<(), ModuleError> {
    let successor = |edge, target, arguments: &[terminal_psi::StructuralArgument]| {
        let Some(target) = machine.blocks.iter().find(|block| block.id == target) else {
            return Ok(()); // The control-flow roster reports an unknown target.
        };
        for (index, argument) in arguments.iter().enumerate() {
            let exclusive_record = target
                .structural_parameters
                .get(index)
                .is_some_and(|parameter| parameter_type(module, parameter));
            if !usable_source(module, machine, aliases, available, argument.place)
                || (exclusive_record
                    && arguments.iter().enumerate().any(|(other_index, other)| {
                        other_index != index && same_storage(aliases, argument.place, other.place)
                    }))
            {
                return Err(ModuleError::InvalidStructuralSuccessorArgument {
                    edge,
                    place: argument.place,
                });
            }
        }
        Ok(())
    };
    let backing_home = |place| {
        owned_home(module, machine, place).is_some()
            && machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters)
                .any(|parameter| {
                    parameter_type(module, parameter)
                        && same_storage(aliases, place, parameter.place)
                })
    };
    let reject_cleanup = |edge, discards: &[PlaceId]| {
        if let Some(place) = discards.iter().find(|place| backing_home(**place)) {
            Err(ModuleError::InvalidStructuralSuccessorArgument {
                edge,
                place: *place,
            })
        } else {
            Ok(())
        }
    };
    match &block.terminator {
        Terminator::Jump {
            edge,
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } => {
            reject_cleanup(*edge, trivial_affine_discards)?;
            for discard in residual_affine_discards {
                reject_cleanup(*edge, &[discard.place])?;
            }
        }
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => {
            for arm in [when_true, when_false] {
                reject_cleanup(arm.edge, &arm.trivial_affine_discards)?;
            }
        }
        Terminator::StructuralCase { cases, .. } => {
            for arm in cases {
                reject_cleanup(arm.edge, &arm.trivial_affine_discards)?;
            }
        }
        _ => {}
    }
    match &block.terminator {
        Terminator::Jump {
            edge,
            target,
            structural_arguments,
            ..
        } => {
            successor(*edge, *target, structural_arguments)?;
        }
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => {
            for arm in [when_true, when_false] {
                successor(arm.edge, arm.target, &arm.structural_arguments)?;
            }
        }
        Terminator::StructuralCase { source, .. } | Terminator::ReturnStructural { source, .. }
            if !usable_source(module, machine, aliases, available, *source) =>
        {
            return Err(ModuleError::StructuralReturnSourceNotLive {
                machine: machine.id,
                block: block.id,
                place: *source,
            });
        }
        _ => {}
    }
    Ok(())
}
