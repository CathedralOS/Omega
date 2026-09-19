//! Borrowed-storage restoration windows at the ownership frontier.
//!
//! `MoveStructuralField` vacates one declared structural field beneath a
//! mutable-borrowed root; `StoreStructuralField` reseats exactly that vacancy
//! with an already-owned subtree of the declared type. The frontier records
//! each open window as *restoration debt* on the borrowed root, keyed by the
//! hole's canonical field path: reads, moves, loans, stores, case dispatches,
//! calls, and successor bindings that overlap the absent subtree reject until
//! the matching repair runs, and every non-crash exit must leave the map
//! empty. Debt rides the ordinary frontier snapshot, so joins require both
//! sides to agree on every open window and replay evidence needs no
//! producer-side assertion.
//!
//! Debt roots are always machine parameters: `validate_move_static` refuses
//! any other root, so a repair store can only close the window it names on
//! the same root. A block parameter can still alias a windowed root through
//! an earlier edge binding (`window_aliases`), so every comparison resolves a
//! spelled place through that map before the canonical prefix test.
//! Byte-sequence views are deliberately outside this scope: they are
//! whole-place loans on a disjoint place namespace, and no view is ever a
//! restoration target.

use super::structural_operations::{canonical_field_path, structural_field_store_write_path};
use super::structural_result_contracts::source_signature;
use super::{
    BTreeMap, BTreeSet, BlockId, CanonicalStructuralPathSegment, EdgeId, ModuleError,
    OperationKind, PlaceId, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralPathSegment, StructuralPlaceKind, StructuralTypeShape, TerminalMachine,
    TerminalModule, Terminator,
};
use terminal_psi::StructuralArgument;

/// The ultimate place each borrowed block parameter may alias, and the
/// canonical prefix beneath that place at which the alias begins. Machine
/// parameters and operation results alias only themselves, so they carry no
/// row; only `SharedBorrow`, `MutableBorrow` and `WriteOnlyBorrow` block
/// parameters can re-spell caller storage.
pub(super) type WindowAliases =
    BTreeMap<PlaceId, BTreeSet<(PlaceId, Vec<CanonicalStructuralPathSegment>)>>;

/// The transitive place/prefix set a spelled `place` resolves to. An
/// unmapped place names only itself at its own root.
fn bound_sources(
    aliases: &WindowAliases,
    place: PlaceId,
) -> BTreeSet<(PlaceId, Vec<CanonicalStructuralPathSegment>)> {
    aliases
        .get(&place)
        .cloned()
        .unwrap_or_else(|| BTreeSet::from([(place, Vec::new())]))
}

/// One argument's resolved canonical prefix, or its whole root when the
/// spelled path cannot be proven: an unresolvable region must overlap every
/// hole beneath it, never silently pass.
fn argument_region(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
) -> (PlaceId, Vec<CanonicalStructuralPathSegment>) {
    let mut path = argument.path.clone();
    // A reference argument's trailing referent marker names the loan's
    // target; the observable region is the carrier path it follows.
    if path.last() == Some(&StructuralPathSegment::Referent) {
        path.pop();
    }
    (
        argument.place,
        canonical_field_path(module, machine, argument.place, &path).unwrap_or_default(),
    )
}

/// Compute the block-parameter alias map for one machine: every borrowed
/// block parameter names the (root place, canonical prefix) set its incoming
/// edge arguments may carry, transitively closed through earlier borrowed
/// parameters. The closure is monotone and bounded by the finite edge
/// spellings, so the fixpoint always terminates.
pub(super) fn window_aliases(module: &TerminalModule, machine: &TerminalMachine) -> WindowAliases {
    let borrowed_parameters: BTreeSet<PlaceId> = machine
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .filter(|parameter| {
            matches!(
                parameter.access,
                StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            )
        })
        .map(|parameter| parameter.place)
        .collect();
    if borrowed_parameters.is_empty() {
        return WindowAliases::new();
    }
    let mut aliases = WindowAliases::new();
    loop {
        let mut expanded = aliases.clone();
        for block in &machine.blocks {
            for (position, parameter) in block.structural_parameters.iter().enumerate() {
                if !borrowed_parameters.contains(&parameter.place) {
                    continue;
                }
                let mut sources = expanded.remove(&parameter.place).unwrap_or_default();
                for predecessor in &machine.blocks {
                    match &predecessor.terminator {
                        Terminator::Jump {
                            target,
                            structural_arguments,
                            ..
                        } if *target == block.id => {
                            if let Some(argument) = structural_arguments.get(position) {
                                sources
                                    .extend(resolve_source(module, machine, &expanded, argument));
                            }
                        }
                        Terminator::Conditional {
                            when_true,
                            when_false,
                            ..
                        } => {
                            for arm in [when_true, when_false] {
                                if arm.target == block.id
                                    && let Some(argument) = arm.structural_arguments.get(position)
                                {
                                    sources.extend(resolve_source(
                                        module, machine, &expanded, argument,
                                    ));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                expanded.insert(parameter.place, sources);
            }
        }
        if expanded == aliases {
            return expanded;
        }
        aliases = expanded;
    }
}

/// One edge argument's source set under the current alias map: a borrowed
/// intermediate composes its stored prefix with the argument's own region.
fn resolve_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    aliases: &WindowAliases,
    argument: &StructuralArgument,
) -> BTreeSet<(PlaceId, Vec<CanonicalStructuralPathSegment>)> {
    let (place, region) = argument_region(module, machine, argument);
    let Some(prefixes) = aliases.get(&place) else {
        return BTreeSet::from([(place, region)]);
    };
    prefixes
        .iter()
        .map(|(root, prefix)| {
            let mut composed = prefix.clone();
            composed.extend_from_slice(&region);
            (*root, composed)
        })
        .collect()
}

/// The (spelled place, canonical region) pairs an operation reads, writes,
/// moves, or loans. The window operations' own hole regions are excluded —
/// `apply_operation` judges them against the debt map directly.
fn touched_regions(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Vec<(PlaceId, Vec<CanonicalStructuralPathSegment>)> {
    let field_region = |place: &PlaceId, path: &[StructuralPathSegment], field| {
        let mut region = canonical_field_path(module, machine, *place, path).unwrap_or_default();
        region.push(CanonicalStructuralPathSegment::Field(field));
        (*place, region)
    };
    match &operation.kind {
        OperationKind::EstablishReference { source } => {
            vec![argument_region(module, machine, source)]
        }
        OperationKind::ReleaseReference { source } => vec![(*source, Vec::new())],
        OperationKind::PrimitiveScalarRead { source, path } => vec![(*source, path.clone())],
        OperationKind::StructuralByteSequenceFieldLength {
            source,
            path,
            field,
            ..
        } => vec![field_region(source, path, *field)],
        OperationKind::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            ..
        }
        | OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            ..
        } => vec![field_region(destination, path, *field)],
        OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } => vec![
            field_region(destination, path, *field),
            (*source, Vec::new()),
        ],
        OperationKind::WriteOnlyPrimitiveStore {
            destination, path, ..
        }
        | OperationKind::WriteOnlyIndexedPrimitiveStore {
            destination, path, ..
        } => vec![(*destination, path.clone())],
        OperationKind::StructuralCaseMembership { source, path, .. } => {
            vec![(
                *source,
                canonical_field_path(module, machine, *source, path).unwrap_or_default(),
            )]
        }
        OperationKind::BooleanStructuralField { source, path, .. }
        | OperationKind::IntegerStructuralField { source, path, .. } => {
            vec![(*source, path.clone())]
        }
        OperationKind::ByteSequenceSubslice { source, .. }
        | OperationKind::ByteSequenceLength { source }
        | OperationKind::ByteSequenceRead { source, .. } => vec![(*source, Vec::new())],
        OperationKind::ByteSequenceWrite { destination, .. } => {
            vec![(*destination, Vec::new())]
        }
        OperationKind::EstablishRecord { fields } => fields
            .iter()
            .filter_map(|field| match &field.value {
                terminal_psi::RecordFieldValue::Structural(argument) => {
                    Some(argument_region(module, machine, argument))
                }
                terminal_psi::RecordFieldValue::Scalar { .. } => None,
            })
            .collect(),
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        }
        | OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments
            .iter()
            .map(|argument| argument_region(module, machine, argument))
            .collect(),
        _ => Vec::new(),
    }
}

/// Expand one spelled region through the block-parameter alias map. A place
/// bound from caller storage touches the caller root at the bound prefix.
fn expand_region(
    aliases: &WindowAliases,
    place: PlaceId,
    region: Vec<CanonicalStructuralPathSegment>,
) -> Vec<(PlaceId, Vec<CanonicalStructuralPathSegment>)> {
    bound_sources(aliases, place)
        .into_iter()
        .map(|(root, prefix)| {
            let mut expanded = prefix;
            expanded.extend_from_slice(&region);
            (root, expanded)
        })
        .collect()
}

/// Whether one region reaches into an open hole (below it) or carries the
/// vacancy (above it); equality is extraction of the absent subtree itself.
fn regions_overlap(
    region: &[CanonicalStructuralPathSegment],
    hole: &[CanonicalStructuralPathSegment],
) -> bool {
    region.starts_with(hole) || hole.starts_with(region)
}

/// The frontier check every operation passes before its other custody
/// effects: no spelled region may overlap a restoration hole open on the
/// same resolved root. A region under the hole reaches absent storage; a
/// region containing the hole would carry the vacancy out of this name.
pub(super) fn check_operation(
    walk: &super::frontier::FrontierWalk<'_>,
    operation: &terminal_psi::Operation,
    frontier: &super::frontier::StructuralOwnershipFrontier,
) -> Result<(), ModuleError> {
    if frontier.restoration_debt.is_empty() {
        return Ok(());
    }
    for (root, region) in touched_regions(walk.module, walk.machine, operation)
        .iter()
        .flat_map(|(place, region)| expand_region(walk.window_aliases, *place, region.clone()))
    {
        if let Some(holes) = frontier.restoration_debt.get(&root)
            && holes.keys().any(|hole| regions_overlap(&region, hole))
        {
            return Err(ModuleError::BorrowedStorageFieldAbsent {
                operation: operation.id,
                place: root,
            });
        }
    }
    Ok(())
}

/// The custody effect of the two window operations: `MoveStructuralField`
/// records its hole, `StoreStructuralField` closes exactly the hole it
/// reseats. Everything else leaves the debt untouched.
pub(super) fn apply_operation(
    walk: &super::frontier::FrontierWalk<'_>,
    operation: &terminal_psi::Operation,
    frontier: &mut super::frontier::StructuralOwnershipFrontier,
) -> Result<(), ModuleError> {
    match &operation.kind {
        OperationKind::MoveStructuralField { source, .. } => {
            let (root, hole) =
                structural_field_store_write_path(walk.module, walk.machine, operation).ok_or(
                    ModuleError::InvalidBorrowedStorageWindow {
                        operation: operation.id,
                        place: *source,
                    },
                )?;
            // A second extraction overlapping an open hole — the same field
            // again or a nested subtree beneath it — reads storage that is
            // already absent.
            if frontier.restoration_debt.get(&root).is_some_and(|holes| {
                holes
                    .keys()
                    .any(|existing| regions_overlap(&hole, existing))
            }) {
                return Err(ModuleError::BorrowedStorageFieldAbsent {
                    operation: operation.id,
                    place: root,
                });
            }
            // A live reference carrier or claim inside the moving subtree
            // would leave the loan pointing at relocated storage.
            let observed = frontier.references.iter().any(|reference| {
                reference.carrier == root
                    && canonical_field_path(
                        walk.module,
                        walk.machine,
                        reference.carrier,
                        &reference.carrier_path,
                    )
                    .is_some_and(|slot| regions_overlap(&hole, &slot))
            }) || frontier.claims.values().any(|claim| {
                claim.input == Some(root)
                    && canonical_field_path(walk.module, walk.machine, root, &claim.path)
                        .unwrap_or_default()
                        .starts_with(&hole)
            });
            if observed {
                return Err(ModuleError::BorrowedStorageFieldAbsent {
                    operation: operation.id,
                    place: root,
                });
            }
            let field_type = operation
                .result
                .structural()
                .map(|result| result.structural_type)
                .ok_or(ModuleError::InvalidBorrowedStorageWindow {
                    operation: operation.id,
                    place: root,
                })?;
            frontier
                .restoration_debt
                .entry(root)
                .or_default()
                .insert(hole, field_type);
            Ok(())
        }
        OperationKind::StoreStructuralField {
            destination, value, ..
        } => {
            let (root, hole) =
                structural_field_store_write_path(walk.module, walk.machine, operation).ok_or(
                    ModuleError::BorrowedStorageRepairMismatch {
                        operation: operation.id,
                        place: *destination,
                    },
                )?;
            // The repair value cannot carry live claim authority, and a
            // windowed place cannot serve as the replacement subtree.
            if frontier
                .claims
                .values()
                .any(|claim| claim.input == Some(value.place))
                || frontier.restoration_debt.contains_key(&value.place)
            {
                return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                    operation: operation.id,
                    place: value.place,
                });
            }
            let field_type = declared_window_field_type(walk.module, walk.machine, operation)
                .ok_or(ModuleError::BorrowedStorageRepairMismatch {
                    operation: operation.id,
                    place: root,
                })?;
            let Some(source) = source_signature(walk.machine, value.place) else {
                return Err(ModuleError::BorrowedStorageRepairMismatch {
                    operation: operation.id,
                    place: root,
                });
            };
            let Some(holes) = frontier.restoration_debt.get_mut(&root) else {
                return Err(ModuleError::BorrowedStorageRepairMismatch {
                    operation: operation.id,
                    place: root,
                });
            };
            if holes.get(&hole) != Some(&field_type) || source.structural_type != field_type {
                return Err(ModuleError::BorrowedStorageRepairMismatch {
                    operation: operation.id,
                    place: root,
                });
            }
            holes.remove(&hole);
            if holes.is_empty() {
                frontier.restoration_debt.remove(&root);
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The declared `Structural` type of the field the window operation names,
/// resolved beneath its root through the spelled path. `None` means the
/// destination does not host a structural field there.
fn declared_window_field_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    let (root, path, field) = match &operation.kind {
        OperationKind::MoveStructuralField {
            source,
            path,
            field,
        }
        | OperationKind::StoreStructuralField {
            destination: source,
            path,
            field,
            ..
        } => (*source, path, *field),
        _ => return None,
    };
    let root_type = source_signature(machine, root)?.structural_type;
    let parent_type = super::foundation::resolve_structural_path(module, root_type, path)?;
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parent_type)?;
    let fields = match &declaration.shape {
        StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. } => {
            fields
        }
        _ => return None,
    };
    match fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())?
        .field_type
    {
        StructuralFieldType::Structural(field_type) => Some(field_type),
        _ => None,
    }
}

/// The borrowed machine parameter a window operation may name: mutable
/// borrow authority, a transferable multiplicity, no qualifications or entry
/// claims. Block parameters are not roots — their aliased storage is reached
/// through `window_aliases` instead.
fn borrowed_window_root(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<&terminal_psi::StructuralParameterDeclaration> {
    let parameter = machine.structural_parameters.iter().find(|parameter| {
        parameter.place == place && parameter.access == StructuralAccess::MutableBorrow
    })?;
    if !matches!(
        parameter.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == place)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == place)
    {
        return None;
    }
    Some(parameter)
}

/// Static shape of a `MoveStructuralField`: a structural result on a fresh
/// operation-result place, a borrowed-window root as its source, and a
/// declared structural field beneath the spelled path carrying exactly the
/// result's declared type.
pub(super) fn validate_move_static(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let OperationKind::MoveStructuralField { source, .. } = &operation.kind else {
        return Ok(());
    };
    let invalid = || ModuleError::InvalidBorrowedStorageWindow {
        operation: operation.id,
        place: *source,
    };
    let Some(result) = operation.result.structural() else {
        return Err(ModuleError::ScalarOperationHasUnitResult(operation.id));
    };
    if !matches!(
        result.multiplicity,
        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
    ) || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return Err(invalid());
    }
    if !matches!(
        machine
            .structural_places
            .iter()
            .find(|place| place.id == result.place)
            .map(|place| place.kind),
        Some(StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        }) if producer == operation.id && structural_type == result.structural_type
    ) {
        return Err(invalid());
    }
    if borrowed_window_root(machine, *source).is_none()
        || declared_window_field_type(module, machine, operation) != Some(result.structural_type)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Static shape of a `StoreStructuralField`: the destination is a
/// borrowed-window root, the spelled path resolves to a declared structural
/// field, and `value` is an owned whole-place argument whose declared type
/// equals that field's type. Whether an open window sits exactly there is
/// the frontier's question, not this pass's.
pub(super) fn validate_store_static(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let OperationKind::StoreStructuralField {
        destination, value, ..
    } = &operation.kind
    else {
        return Ok(());
    };
    let invalid = || ModuleError::InvalidBorrowedStorageWindow {
        operation: operation.id,
        place: *destination,
    };
    let Some(field_type) = declared_window_field_type(module, machine, operation) else {
        return Err(invalid());
    };
    if borrowed_window_root(machine, *destination).is_none()
        || value.access != StructuralAccess::Owned
        || !value.path.is_empty()
    {
        return Err(invalid());
    }
    let Some(source) = source_signature(machine, value.place) else {
        return Err(invalid());
    };
    if source.structural_type != field_type
        || !source.qualifications.is_empty()
        || !source.projected_qualifications.is_empty()
    {
        return Err(invalid());
    }
    Ok(())
}

/// The successor-binding check for every structural edge argument: a window
/// cannot cross an edge under any access, because the target block cannot
/// name the hole the way this frame names it. Disjoint projections stay
/// legal — only overlap with an open hole rejects.
pub(super) fn check_edge_arguments(
    module: &TerminalModule,
    machine: &TerminalMachine,
    frontier: &super::frontier::StructuralOwnershipFrontier,
    aliases: &WindowAliases,
    edge: EdgeId,
    arguments: &[StructuralArgument],
) -> Result<(), ModuleError> {
    if frontier.restoration_debt.is_empty() {
        return Ok(());
    }
    for argument in arguments {
        let (place, region) = argument_region(module, machine, argument);
        for (root, expanded) in expand_region(aliases, place, region) {
            if frontier
                .restoration_debt
                .get(&root)
                .is_some_and(|holes| holes.keys().any(|hole| regions_overlap(&expanded, hole)))
            {
                return Err(ModuleError::InvalidStructuralSuccessorArgument {
                    edge,
                    place: argument.place,
                });
            }
        }
    }
    Ok(())
}

/// A case terminator observes the subtree at its source's resolved prefix:
/// dispatching on a region overlapping an open window would carry the
/// vacancy across a non-crash edge, so it rejects like an unfilled hole at a
/// return.
pub(super) fn check_case_source(
    machine: &TerminalMachine,
    block: BlockId,
    source: PlaceId,
    frontier: &super::frontier::StructuralOwnershipFrontier,
    aliases: &WindowAliases,
) -> Result<(), ModuleError> {
    let observed = bound_sources(aliases, source).iter().any(|(root, prefix)| {
        frontier
            .restoration_debt
            .get(root)
            .is_some_and(|holes| holes.keys().any(|hole| regions_overlap(prefix, hole)))
    });
    if observed {
        return Err(ModuleError::BorrowedStorageRestorationPending {
            machine: machine.id,
            block,
            place: source,
        });
    }
    Ok(())
}
