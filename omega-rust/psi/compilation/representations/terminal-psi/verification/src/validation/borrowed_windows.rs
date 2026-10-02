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
//! Debt stays attached to the exact storage carrier that opened the window,
//! including borrowed block parameters. A repair must name that same carrier:
//! two block parameters with identical possible origins need not select the
//! same storage at runtime. Possible aliases therefore establish interference,
//! never permission to repair. Interference resolves both the observed region
//! and the debt through `window_aliases` before comparing canonical prefixes.
//! Block parameters inherit window authority only from eligible storage roots.
//! Byte-sequence views are deliberately outside this scope: they are
//! whole-place loans on a disjoint place namespace, and no view is ever a
//! restoration target.
//!
//! A root window (`field: None`, empty path) vacates the whole referent of an
//! exclusive `MutableBorrow` parameter: whole-value assignment through
//! `&mut T`. Its hole is the empty canonical path, so every observation of
//! the root overlaps it until the repair. The moved-out value is an ordinary
//! owned result under its declared multiplicity, exactly as a field window's:
//! displaced affine contents stay an obligation the frontier requires
//! disposed, plain contents need nothing. A borrowed parameter's multiplicity
//! describes its loan carrier, not the referent, so it is no evidence for
//! either value's. Owned homes never open a root window; their whole value
//! moves by ordinary transfer.

use super::structural::operations::{
    canonical_field_path, canonical_mutable_record_referent_path, structural_field_store_write_path,
};
use super::structural::result_contracts::source_signature;
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
pub(crate) type WindowAliases =
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
    let region = canonical_mutable_record_referent_path(module, machine, argument.place, &path)
        .or_else(|| canonical_field_path(module, machine, argument.place, &path))
        .unwrap_or_default();
    (argument.place, region)
}

/// Compute the block-parameter alias map for one machine: every borrowed
/// block parameter names the (root place, canonical prefix) set its incoming
/// edge arguments may carry, transitively closed through earlier borrowed
/// parameters. The closure is monotone and bounded by the finite edge
/// spellings, so the fixpoint always terminates.
pub(crate) fn window_aliases(module: &TerminalModule, machine: &TerminalMachine) -> WindowAliases {
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
    // Seed borrowed parameters as unresolved, not as roots of their own.
    // Otherwise a loop backedge permanently contributes a phantom origin.
    let mut aliases: WindowAliases = borrowed_parameters
        .iter()
        .map(|place| (*place, BTreeSet::new()))
        .collect();
    loop {
        let mut expanded = aliases.clone();
        for block in &machine.blocks {
            for (position, parameter) in block.structural_parameters.iter().enumerate() {
                if !borrowed_parameters.contains(&parameter.place) {
                    continue;
                }
                let mut sources = bound_sources(&expanded, parameter.place);
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
    let canonical_region = |place: PlaceId, path: &[StructuralPathSegment]| {
        canonical_mutable_record_referent_path(module, machine, place, path)
            .or_else(|| canonical_field_path(module, machine, place, path))
    };
    let field_region = |place: &PlaceId, path: &[StructuralPathSegment], field| {
        let region = canonical_region(*place, path).map_or_else(Vec::new, |mut region| {
            region.push(CanonicalStructuralPathSegment::Field(field));
            region
        });
        (*place, region)
    };
    // A primitive leaf's region is its canonical path; a runtime-selected
    // element touches the whole array it selects in.
    let primitive_region = |place: &PlaceId, path: &[StructuralPathSegment]| {
        let static_prefix = path
            .iter()
            .position(|segment| segment.runtime_index().is_some())
            .unwrap_or(path.len());
        (
            *place,
            canonical_region(*place, &path[..static_prefix]).unwrap_or_default(),
        )
    };
    match &operation.kind {
        OperationKind::EstablishByteSequenceConcatenation { left, right, .. } => [left, right]
            .into_iter()
            .map(|operand| match operand {
                terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => {
                    (*source, Vec::new())
                }
                terminal_psi::control_flow::ByteSequenceOperand::Field {
                    source, field, ..
                } => field_region(&source.place, &source.path, *field),
            })
            .collect(),
        OperationKind::EstablishReference { source } => {
            vec![argument_region(module, machine, source)]
        }
        OperationKind::ReleaseReference { source } => vec![(*source, Vec::new())],
        OperationKind::PrimitiveScalarRead { source, path } => {
            vec![primitive_region(source, path)]
        }
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
        } => vec![field_region(destination, path, *field)],
        // A runtime element in the carrier touches every element of the
        // array it selects in, as a primitive leaf's does.
        OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            ..
        } => vec![if terminal_psi::is_static_structural_path(path) {
            field_region(destination, path, *field)
        } else {
            primitive_region(destination, path)
        }],
        OperationKind::ByteSequenceStore {
            destination,
            source,
            ..
        } => vec![(*destination, Vec::new()), (*source, Vec::new())],
        OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            ..
        }
        | OperationKind::StructuralByteViewFieldStore {
            destination,
            path,
            field,
            source,
        } => vec![
            field_region(destination, path, *field),
            (*source, Vec::new()),
        ],
        OperationKind::WriteOnlyPrimitiveStore {
            destination, path, ..
        } => vec![primitive_region(destination, path)],
        OperationKind::AtomicAccess {
            place, path, field, ..
        } => vec![field_region(place, path, *field)],
        OperationKind::StructuralCaseMembership { source, path, .. } => {
            vec![(*source, canonical_region(*source, path).unwrap_or_default())]
        }
        // The carrier path is already canonical and excludes the read field,
        // so the observed region is that one scalar leaf: a counter beside an
        // open element window (`self.items[self.r] = ..` re-reads `self.r`
        // for the repair store's index) touches no absent storage, while a
        // read beneath the hole still overlaps it.
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
            vec![(*source, region)]
        }
        OperationKind::ByteSequenceSubslice { source, .. }
        | OperationKind::ByteSequenceLength { source }
        | OperationKind::ByteSequenceRead { source, .. } => vec![(*source, Vec::new())],
        OperationKind::ByteSequenceWrite { destination, .. }
        | OperationKind::ElementViewWrite { destination, .. } => {
            vec![(*destination, Vec::new())]
        }
        OperationKind::EstablishRecord { fields }
        | OperationKind::EstablishStructuralCase { fields, .. } => fields
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

/// Possible origins are sufficient to reject interfering access, but not to
/// establish that a replacement restores the particular carrier we vacated.
fn regions_may_alias(
    aliases: &WindowAliases,
    left: PlaceId,
    left_region: &[CanonicalStructuralPathSegment],
    right: PlaceId,
    right_region: &[CanonicalStructuralPathSegment],
) -> bool {
    let left_sources = expand_region(aliases, left, left_region.to_vec());
    let right_sources = expand_region(aliases, right, right_region.to_vec());
    left_sources.iter().any(|(left_root, left_path)| {
        right_sources.iter().any(|(right_root, right_path)| {
            left_root == right_root && regions_overlap(left_path, right_path)
        })
    })
}

fn overlapping_debt(
    aliases: &WindowAliases,
    frontier: &super::frontier::StructuralOwnershipFrontier,
    place: PlaceId,
    region: &[CanonicalStructuralPathSegment],
) -> Option<PlaceId> {
    frontier.restoration_debt.iter().find_map(|(root, holes)| {
        holes
            .keys()
            .any(|hole| regions_may_alias(aliases, place, region, *root, hole))
            .then_some(*root)
    })
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
    for (place, region) in touched_regions(walk.module, walk.machine, operation) {
        if let Some(root) = overlapping_debt(walk.window_aliases, frontier, place, &region) {
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
        OperationKind::MoveStructuralField { source, path, .. } => {
            let (root, hole) =
                structural_field_store_write_path(walk.module, walk.machine, operation).ok_or(
                    ModuleError::InvalidBorrowedStorageWindow {
                        operation: operation.id,
                        place: *source,
                    },
                )?;
            let reference_root = super::references::live_mutable_record_field_destination(
                walk.module,
                walk.machine,
                &frontier.references,
                *source,
                path,
            )?
            .is_some();
            // A second extraction overlapping an open hole — the same field
            // again or a nested subtree beneath it — reads storage that is
            // already absent.
            if overlapping_debt(walk.window_aliases, frontier, root, &hole).is_some() {
                return Err(ModuleError::BorrowedStorageFieldAbsent {
                    operation: operation.id,
                    place: root,
                });
            }
            // A live reference carrier or claim inside the moving subtree
            // would leave the loan pointing at relocated storage.
            let observed = frontier.references.iter().any(|reference| {
                // This carrier is the authority for the move, not a child
                // observer into the subtree. The exact parent and absence of
                // live descendants were reconstructed immediately above.
                if reference_root && reference.carrier == root && reference.carrier_path.is_empty()
                {
                    return false;
                }
                let slot = canonical_field_path(
                    walk.module,
                    walk.machine,
                    reference.carrier,
                    &reference.carrier_path,
                )
                .unwrap_or_default();
                regions_may_alias(walk.window_aliases, root, &hole, reference.carrier, &slot)
            }) || frontier.claims.values().any(|claim| {
                claim.input.is_some_and(|input| {
                    let region =
                        canonical_field_path(walk.module, walk.machine, input, &claim.path)
                            .unwrap_or_default();
                    regions_may_alias(walk.window_aliases, root, &hole, input, &region)
                })
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
            destination,
            path,
            value,
            ..
        } => {
            let (root, hole) =
                structural_field_store_write_path(walk.module, walk.machine, operation).ok_or(
                    ModuleError::BorrowedStorageRepairMismatch {
                        operation: operation.id,
                        place: *destination,
                    },
                )?;
            super::references::live_mutable_record_field_destination(
                walk.module,
                walk.machine,
                &frontier.references,
                *destination,
                path,
            )?;
            // The repair value cannot carry live claim authority, and a
            // windowed place cannot serve as the replacement subtree.
            if frontier
                .claims
                .values()
                .any(|claim| claim.input == Some(value.place))
                || overlapping_debt(walk.window_aliases, frontier, value.place, &[]).is_some()
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
/// resolved beneath its root through the spelled path. A root window names
/// the whole referent, spelled with no path. `None` means the destination
/// does not host a structural field there.
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
    let (root_type, path) = super::references::mutable_record_field_destination(
        module, machine, root, path,
    )
    .unwrap_or((
        source_signature(machine, root)?.structural_type,
        path.as_slice(),
    ));
    let Some(field) = field else {
        return path.is_empty().then_some(root_type);
    };
    if matches!(
        path.last(),
        Some(StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. })
    ) {
        // An element leaf: `field` names the array's own field declaration —
        // the last field segment in its owner record — and the hole's type is
        // the element the spelled path selects.
        let field_position = path
            .iter()
            .rposition(|segment| matches!(segment, StructuralPathSegment::Field(_)))?;
        let owner = super::foundation::resolve_runtime_projection(
            module,
            root_type,
            &path[..field_position],
        )?;
        let declaration = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == owner)?;
        let fields = match &declaration.shape {
            StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. } => {
                fields
            }
            _ => return None,
        };
        fields.iter().find(|candidate| {
            candidate.id == field
                && !candidate.relevance.is_erased()
                && matches!(candidate.field_type, StructuralFieldType::Structural(_))
        })?;
        return super::foundation::resolve_runtime_projection(module, root_type, path);
    }
    let parent_type = super::foundation::resolve_runtime_projection(module, root_type, path)?;
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

/// The root storage a window operation may name: either a mutable-borrowed
/// machine parameter (borrow authority, a transferable multiplicity, no
/// qualifications or entry claims) or an owned home place this machine
/// completed itself — an established record or structural leaf whose
/// restoration debt is the same contract on owned storage.
fn direct_window_root(
    module: &TerminalModule,
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<()> {
    if let Some(parameter) = machine.structural_parameters.iter().find(|parameter| {
        parameter.place == place && parameter.access == StructuralAccess::MutableBorrow
    }) && matches!(
        parameter.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && !machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == place)
        && !machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == place)
    {
        return Some(());
    }
    super::record::completed_source(module, machine, place).map(|_| ())
}

fn borrowed_window_root(
    module: &TerminalModule,
    machine: &TerminalMachine,
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<()> {
    if super::references::mutable_record_field_destination(module, machine, place, path).is_some() {
        return Some(());
    }
    if direct_window_root(module, machine, place).is_some() {
        return Some(());
    }
    let parameter = super::block_views::parameter(machine, place)?;
    if parameter.access != StructuralAccess::MutableBorrow
        || !matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
    {
        return None;
    }
    let aliases = window_aliases(module, machine);
    let sources = aliases.get(&place)?;
    (!sources.is_empty()
        && sources
            .iter()
            .all(|(root, _)| direct_window_root(module, machine, *root).is_some()))
    .then_some(())
}

/// Whether `place` is an exclusive loan whose whole referent a root window
/// may replace: an eligible mutable-borrowed machine or block parameter. An
/// owned home is no loan, so it never opens a root window.
fn is_root_window_loan(module: &TerminalModule, machine: &TerminalMachine, place: PlaceId) -> bool {
    let parameter = machine
        .structural_parameters
        .iter()
        .any(|parameter| parameter.place == place)
        || super::block_views::parameter(machine, place).is_some();
    parameter && borrowed_window_root(module, machine, place, &[]).is_some()
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
    let OperationKind::MoveStructuralField { source, path, .. } = &operation.kind else {
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
    if borrowed_window_root(module, machine, *source, path).is_none()
        || declared_window_field_type(module, machine, operation) != Some(result.structural_type)
    {
        return Err(invalid());
    }
    // A root window displaces the whole referent of an exclusive loan; an
    // owned home is no loan.
    if let OperationKind::MoveStructuralField { field: None, .. } = &operation.kind
        && !is_root_window_loan(module, machine, *source)
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
        destination,
        path,
        value,
        ..
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
    if borrowed_window_root(module, machine, *destination, path).is_none()
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
    if let OperationKind::StoreStructuralField { field: None, .. } = &operation.kind
        && !is_root_window_loan(module, machine, *destination)
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
        if overlapping_debt(aliases, frontier, place, &region).is_some() {
            return Err(ModuleError::InvalidStructuralSuccessorArgument {
                edge,
                place: argument.place,
            });
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
    if overlapping_debt(aliases, frontier, source, &[]).is_some() {
        return Err(ModuleError::BorrowedStorageRestorationPending {
            machine: machine.id,
            block,
            place: source,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        BTreeSet, CanonicalStructuralPathSegment, PlaceId, WindowAliases, regions_may_alias,
    };
    use semantic_vocabulary::StructuralFieldId;

    #[test]
    fn interference_resolves_both_carriers_and_preserves_projection_prefixes() {
        let storage = PlaceId::new(1).unwrap();
        let block_carrier = PlaceId::new(2).unwrap();
        let other_carrier = PlaceId::new(3).unwrap();
        let left = CanonicalStructuralPathSegment::Field(StructuralFieldId::new(1).unwrap());
        let right = CanonicalStructuralPathSegment::Field(StructuralFieldId::new(2).unwrap());
        let aliases = WindowAliases::from([
            (
                block_carrier,
                BTreeSet::from([(storage, vec![left.clone()])]),
            ),
            (other_carrier, BTreeSet::from([(storage, Vec::new())])),
        ]);
        for observer in [storage, other_carrier] {
            assert!(regions_may_alias(
                &aliases,
                observer,
                std::slice::from_ref(&left),
                block_carrier,
                &[]
            ));
            assert!(regions_may_alias(
                &aliases,
                block_carrier,
                &[],
                observer,
                std::slice::from_ref(&left)
            ));
            assert!(!regions_may_alias(
                &aliases,
                observer,
                std::slice::from_ref(&right),
                block_carrier,
                &[]
            ));
        }
    }
}
