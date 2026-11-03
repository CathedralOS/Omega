//! Reference values follow a source leaf, not a may-write overlap closure.
//! An owned field beside a reference cannot acquire that reference's origin
//! merely because indexing made their write footprints identical.

use super::super::place_paths::{FramePathPrecision, FrameSourcePlace};
use super::super::stored_calls::SlotSource;
use super::{FramePlaceOrigin, StoredLocalOrigins, compose_origin, place_suffix};
use facts::PlaceSegment;
use typed_trees::types::TypeReferenceHandle;

pub(in crate::machine_calls::calls::write_frames) fn canonical_reference_origins(
    program: &dyn crate::CheckingInput,
    origin: &Option<FramePlaceOrigin>,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Vec<Option<FramePlaceOrigin>> {
    spatial_origins(resolve_sources(program, origin, aliases, stored, None))
}

/// Capture a descriptor cell's home. Ancestor descriptors still load their
/// complete captured backing; the final cell's old contents do not select the
/// address of its own replacement. Unknown ancestors remain unknown.
pub(in crate::machine_calls::calls::write_frames) fn canonical_storage_origins(
    program: &dyn crate::CheckingInput,
    origin: &FramePlaceOrigin,
    aliases: &[(String, FramePlaceOrigin)],
    divergent: &[(String, Vec<FramePlaceOrigin>)],
    stored: &[StoredLocalOrigins],
) -> Vec<Option<FramePlaceOrigin>> {
    // A bare pointer's final cell is its own descriptor home. Its alias names
    // the old pointee and must not move a write to that different storage.
    // Nonempty projections still cross their containing reference first.
    let aliases = if origin.source.segments.is_empty() {
        &[][..]
    } else {
        aliases
    };
    if !origin.source.segments.is_empty()
        && origin.source.root.is_valid()
        && let Some((name, alternatives)) = divergent
            .iter()
            .find(|(name, _)| name == program.symbols.name(origin.source.root))
    {
        if alternatives.is_empty() {
            return vec![None];
        }
        let mut sources = Vec::new();
        for alternative in alternatives {
            let mut branch = aliases.to_vec();
            branch.retain(|(prior, _)| prior != name);
            branch.push((name.clone(), alternative.clone()));
            sources.extend(spatial_origins(
                resolve_steps(program, &Some(origin.clone()), &branch, stored, None, false)
                    .into_iter()
                    .map(|step| step.map(|step| step.source))
                    .collect(),
            ));
        }
        return sources;
    }
    spatial_origins(
        resolve_steps(program, &Some(origin.clone()), aliases, stored, None, false)
            .into_iter()
            .map(|step| step.map(|step| step.source))
            .collect(),
    )
}

pub(in crate::machine_calls::calls::write_frames) fn canonical_reference_sources(
    program: &dyn crate::CheckingInput,
    origin: &Option<FramePlaceOrigin>,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Vec<Option<SlotSource>> {
    resolve_sources(program, origin, aliases, stored, None)
}

pub(in crate::machine_calls::calls::write_frames) fn captured_reference_projection_origins(
    program: &dyn crate::CheckingInput,
    captured: &FramePlaceOrigin,
    projected: &FramePlaceOrigin,
    stored: &[StoredLocalOrigins],
) -> Vec<Option<FramePlaceOrigin>> {
    spatial_origins(captured_reference_projection_sources(
        program, captured, projected, stored,
    ))
}

pub(in crate::machine_calls::calls::write_frames) fn captured_reference_projection_sources(
    program: &dyn crate::CheckingInput,
    captured: &FramePlaceOrigin,
    projected: &FramePlaceOrigin,
    stored: &[StoredLocalOrigins],
) -> Vec<Option<SlotSource>> {
    if !captured.source.same_namespace(&projected.source)
        || !projected
            .source
            .segments
            .starts_with(&captured.source.segments)
    {
        return vec![None];
    }
    resolve_sources(
        program,
        &Some(projected.clone()),
        &[],
        stored,
        Some(captured.source.segments.len()),
    )
}

fn spatial_origins(sources: Vec<Option<SlotSource>>) -> Vec<Option<FramePlaceOrigin>> {
    sources
        .into_iter()
        .map(|source| {
            source.and_then(|source| {
                (source.lifetime_access.is_none()
                    && source.origin.source.output_identity().is_none())
                .then_some(source.origin)
            })
        })
        .collect()
}

/// A loaded slot records the captured backing-prefix depth independently of
/// the final projection. Following another pointer must skip that already
/// observed descriptor and retain the actual slot which supplied the new one.
struct ReferenceStep {
    source: SlotSource,
    backing_depth: Option<usize>,
    loaded_slot: Option<CapturedReferenceLoad>,
}

/// One resolution retains the slot which supplied its pointer beside the
/// resulting backing. Slot coordinates are invalidation evidence only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::machine_calls::calls::write_frames) struct CapturedReferenceLoad {
    pub place: FramePlaceOrigin,
    pub access: language_semantics::ReferenceAccess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::machine_calls::calls::write_frames) struct CapturedReferenceRead {
    pub source: SlotSource,
    pub slots: Vec<CapturedReferenceLoad>,
}

pub(in crate::machine_calls::calls::write_frames) fn captured_reference_reads(
    program: &dyn crate::CheckingInput,
    origin: &Option<FramePlaceOrigin>,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Vec<Option<CapturedReferenceRead>> {
    resolve_steps(program, origin, aliases, stored, None, true)
        .into_iter()
        .map(|step| {
            let mut step = step?;
            let slots = step.loaded_slot.into_iter().collect();
            if step.backing_depth.is_some() && step.source.lifetime_access.is_none() {
                step.source.carrier = step.source.origin.source.clone();
                step.source.carrier_backing = None;
            }
            Some(CapturedReferenceRead {
                source: step.source,
                slots,
            })
        })
        .collect()
}

fn spatial_source(origin: FramePlaceOrigin) -> Option<ReferenceStep> {
    Some(ReferenceStep {
        source: SlotSource {
            carrier_backing: None,
            carrier: origin.source.clone(),
            origin,
            lifetime_access: None,
        },
        backing_depth: None,
        loaded_slot: None,
    })
}

fn resolve_sources(
    program: &dyn crate::CheckingInput,
    origin: &Option<FramePlaceOrigin>,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
    crossed: Option<usize>,
) -> Vec<Option<SlotSource>> {
    resolve_steps(program, origin, aliases, stored, crossed, true)
        .into_iter()
        .map(|step| {
            step.map(|mut step| {
                // Spatial callers historically retain only the resulting backing.
                // The completed-value query below consumes the full step instead,
                // including the descriptor row which authenticated a pointer load.
                if step.backing_depth.is_some() && step.source.lifetime_access.is_none() {
                    step.source.carrier = step.source.origin.source.clone();
                    step.source.carrier_backing = None;
                }
                step.source
            })
        })
        .collect()
}

/// Complete a callee-relative projection of an already observed descriptor.
/// The descriptor value is fixed; each further slot load uses this invocation's
/// entry inventory. Returned carrier coordinates come from the selected stored
/// row, never by appending a suffix to an earlier descriptor's carrier.
pub(in crate::machine_calls::calls::write_frames) fn completed_reference_projection_sources(
    program: &dyn crate::CheckingInput,
    captured: &SlotSource,
    projected: &FramePlaceOrigin,
    stored: &[StoredLocalOrigins],
) -> Option<Vec<SlotSource>> {
    let captured = CapturedReferenceRead {
        source: captured.clone(),
        slots: Vec::new(),
    };
    completed_reference_projection_reads(program, &captured, projected, stored)
        .map(|reads| reads.into_iter().map(|read| read.source).collect())
}

/// Continue the same completed descriptor projection while retaining every
/// matched load. Spatial consumers deliberately discard this roster; a value
/// observation keeps it so any replaced carrier can retire its correspondence.
pub(in crate::machine_calls::calls::write_frames) fn completed_reference_projection_reads(
    program: &dyn crate::CheckingInput,
    captured: &CapturedReferenceRead,
    projected: &FramePlaceOrigin,
    stored: &[StoredLocalOrigins],
) -> Option<Vec<CapturedReferenceRead>> {
    let source = &captured.source;
    if !source.origin.source.same_namespace(&projected.source)
        || !projected
            .source
            .segments
            .starts_with(&source.origin.source.segments)
    {
        return None;
    }
    let mut pending = vec![(
        CapturedReferenceRead {
            source: SlotSource {
                origin: projected.clone(),
                carrier_backing: source.carrier_backing.clone().or_else(|| {
                    source
                        .lifetime_access
                        .is_none()
                        .then(|| source.origin.clone())
                }),
                ..source.clone()
            },
            slots: captured.slots.clone(),
        },
        source.origin.source.segments.len(),
        Vec::new(),
    )];
    let mut completed = Vec::new();
    while let Some((read, crossed, mut ancestors)) = pending.pop() {
        let source = read.source;
        if source.origin.source.segments.len() == crossed || source.lifetime_access.is_some() {
            let read = CapturedReferenceRead {
                source,
                slots: read.slots,
            };
            if !completed.contains(&read) {
                completed.push(read);
            }
            continue;
        }
        let identity = (source.origin.clone(), crossed);
        if ancestors.contains(&identity) {
            return None;
        }
        ancestors.push(identity);
        for step in resolve_steps(
            program,
            &Some(source.origin.clone()),
            &[],
            stored,
            Some(crossed),
            true,
        ) {
            let mut step = step?;
            let mut slots = read.slots.clone();
            if let Some(slot) = step.loaded_slot
                && !slots.contains(&slot)
            {
                slots.push(slot);
            }
            if let Some(depth) = step.backing_depth {
                step.source.lifetime_access =
                    step.source.lifetime_access.or(source.lifetime_access);
                pending.push((
                    CapturedReferenceRead {
                        source: step.source,
                        slots,
                    },
                    depth,
                    ancestors.clone(),
                ));
            } else {
                if step.source.lifetime_access.is_none() {
                    step.source.carrier = source.carrier.clone();
                    step.source.carrier_backing = source.carrier_backing.clone();
                }
                let read = CapturedReferenceRead {
                    source: step.source,
                    slots,
                };
                if !completed.contains(&read) {
                    completed.push(read);
                }
            }
        }
    }
    (!completed.is_empty()).then_some(completed)
}

/// The supplied leaf fixes one opaque pointer. Owned projections beneath
/// that pointer keep the same namespace; another descriptor load needs its
/// independently captured contents and cannot be inferred from a suffix.
fn opaque_output_projection_known(
    program: &dyn crate::CheckingInput,
    source: &super::super::place_paths::FrameSourcePlace,
) -> bool {
    let Some(identity) = source.output_identity() else {
        return false;
    };
    if !source.recasts.is_empty() || source.representation.is_some() || !source.builtin_coordinates
    {
        return false;
    }
    let Some(parameter) = crate::invocation_output_parameter(program, identity) else {
        return false;
    };
    let Some(outer) = crate::reference_shell(program, parameter.type_reference) else {
        return false;
    };
    for depth in 0..=source.segments.len() {
        let Some(reference) = crate::owned_projection_type_reference(
            program,
            outer.referee,
            &source.segments[..depth],
        ) else {
            return false;
        };
        if let Some(shell) = crate::reference_shell(program, reference) {
            if depth == source.segments.len() {
                return true;
            }
            return crate::owned_projection_type_reference(
                program,
                shell.referee,
                &source.segments[depth..],
            )
            .is_some_and(|reference| crate::reference_shell(program, reference).is_none());
        }
    }
    false
}

fn resolve_steps(
    program: &dyn crate::CheckingInput,
    origin: &Option<FramePlaceOrigin>,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
    mut crossed: Option<usize>,
    dereference_endpoint: bool,
) -> Vec<Option<ReferenceStep>> {
    let Some(origin) = origin else {
        return vec![None];
    };
    // An opaque result referent is already terminal provenance: its segments
    // are selectors beneath the returned leaf, not slots this frame reloads.
    if origin.source.is_static() || origin.source.result_home().is_some() {
        return vec![spatial_source(origin.clone())];
    }
    if origin.source.output_identity().is_some() {
        return vec![
            opaque_output_projection_known(program, &origin.source)
                .then(|| spatial_source(origin.clone()))
                .flatten(),
        ];
    }
    if !origin.source.root.is_valid() {
        // Existing path-only evidence can still conservatively invalidate
        // storage. It cannot later export an owned-formal reference boundary.
        return super::canonical_origins(program, origin, aliases, stored)
            .map(|origins| origins.into_iter().map(spatial_source).collect())
            .unwrap_or_else(|| vec![None]);
    }
    let mut origin = origin.clone();
    if let Some((_, prior)) = aliases
        .iter()
        .find(|(name, _)| program.symbols.name(origin.source.root) == name)
    {
        crossed = Some(prior.source.segments.len());
        let (_, suffix) = super::split_place_root(&origin.path);
        let source = prior.source.append_source(&origin.source);
        origin = compose_origin(program, prior, suffix, origin.precision);
        origin.source = source;
    }
    let Some(local) = stored
        .iter()
        .find(|local| local.local_symbol == origin.source.root)
    else {
        return vec![spatial_source(origin)];
    };
    // A retained nested slot records its current contents. Its incoming
    // ancestor descriptor only identifies storage and must not reintroduce
    // the pre-store value beside that current slot (including unknown rows).
    let selected_depth = local
        .references
        .iter()
        .filter(|leaf| {
            matches!(leaf.kind, super::StoredOriginKind::Reference(_))
                && !crossed.is_some_and(|depth| leaf.local_segments.len() <= depth)
                && (dereference_endpoint
                    || leaf.local_segments.len() < origin.source.segments.len())
                && source_reaches_leaf(&origin.source.segments, &leaf.local_segments)
                && !super::projections::reference_is_shadowed(local, leaf, &origin.source.segments)
        })
        .map(|leaf| leaf.local_segments.len())
        .max();
    let mut sources = Vec::new();
    for leaf in &local.references {
        if !matches!(leaf.kind, super::StoredOriginKind::Reference(_))
            || Some(leaf.local_segments.len()) != selected_depth
            || !source_reaches_leaf(&origin.source.segments, &leaf.local_segments)
            || super::projections::reference_is_shadowed(local, leaf, &origin.source.segments)
        {
            continue;
        }
        let suffix = place_suffix(&leaf.local_path, &origin.path).unwrap_or("");
        // Captured lifetime debts survive projection as a whole. Appending
        // output selectors would invent geometry inside an unrelated input.
        let Some(dependencies) = leaf.lifetime_dependencies.as_ref() else {
            sources.push(None);
            continue;
        };
        for dependency in dependencies {
            if !sources.iter().any(|step: &Option<ReferenceStep>| {
                step.as_ref().is_some_and(|step| step.source == *dependency)
            }) {
                sources.push(Some(ReferenceStep {
                    source: dependency.clone(),
                    backing_depth: None,
                    loaded_slot: None,
                }));
            }
        }
        let Some(mut captured) = leaf.origin.clone() else {
            if dependencies.is_empty() && !sources.iter().any(Option::is_none) {
                sources.push(None);
            }
            continue;
        };
        if captured.source.root == local.local_symbol
            && captured.source.segments == leaf.local_segments
        {
            for (segment, selected) in captured
                .source
                .segments
                .iter_mut()
                .zip(&origin.source.segments)
            {
                if matches!(segment, PlaceSegment::Index { expression } if !expression.is_valid()) {
                    *segment = *selected;
                }
            }
        }
        // Loading a stored reference substitutes its captured referent before
        // any subsequent view. Trim storage-slot coordinates, never the logical
        // selectors inside a presentation, and retain every formation event.
        let mut relative = origin.source.clone();
        relative.segments.drain(..leaf.local_segments.len());
        for recast in &mut relative.recasts {
            if recast.root != origin.source.root
                || !source_reaches_leaf(&recast.segments, &leaf.local_segments)
            {
                return vec![None];
            }
            recast.segments.drain(..leaf.local_segments.len());
        }
        // The slot's containing collection may have a coarse write cover.
        // Loading its complete descriptor yields the captured pointer instead:
        // only selectors still beneath that pointer affect its precision.
        // The loaded-slot record below retains the original cover, so replacing
        // the descriptor can still retire its correspondence.
        let mut source = compose_origin(
            program,
            &captured,
            suffix,
            remaining_projection_precision(&relative, origin.precision),
        );
        if origin.precision == FramePathPrecision::CollectionCoarse
            && std::env::var_os("OMEGA_STRUCT_TRACE").is_some()
        {
            eprintln!(
                "FRAME reference-load slot={:?} captured={:?} remaining={:?} builtin={} result={:?}",
                leaf.local_segments,
                captured.precision,
                relative.segments,
                relative.builtin_coordinates,
                source.precision,
            );
        }
        source.source = captured.source.append_source(&relative);
        let removed = origin.source.segments.len() - leaf.local_segments.len();
        let mut carrier = origin.source.clone();
        carrier.segments.truncate(leaf.local_segments.len());
        if let Some((_, selectors)) = &mut carrier.representation {
            let Some(length) = selectors.len().checked_sub(removed) else {
                return vec![None];
            };
            selectors.truncate(length);
        }
        carrier.recasts.retain(|recast| {
            recast.root != carrier.root || recast.segments.len() <= carrier.segments.len()
        });
        let backing_depth = captured.source.segments.len();
        let super::StoredOriginKind::Reference(access) = leaf.kind else {
            return vec![None];
        };
        let loaded_slot = CapturedReferenceLoad {
            place: FramePlaceOrigin {
                path: leaf.local_path.clone(),
                precision: origin.precision,
                source: carrier.clone(),
            },
            access,
        };
        let step = ReferenceStep {
            source: SlotSource {
                carrier_backing: Some(captured),
                origin: source,
                carrier,
                lifetime_access: None,
            },
            backing_depth: Some(backing_depth),
            loaded_slot: Some(loaded_slot),
        };
        if !sources.iter().any(|prior| {
            prior.as_ref().is_some_and(|prior| {
                prior.source == step.source && prior.backing_depth == step.backing_depth
            })
        }) {
            sources.push(Some(step));
        }
    }
    // No reference boundary was crossed. Preserve the private source so
    // exported result analysis rejects it; do not substitute a sibling leaf.
    if sources.is_empty() {
        vec![spatial_source(origin)]
    } else {
        sources
    }
}

fn remaining_projection_precision(
    source: &FrameSourcePlace,
    known: FramePathPrecision,
) -> FramePathPrecision {
    // An exact query already owns its selector evidence. Rebuild only the
    // coarse cover whose consumed slot selectors no longer name the pointee.
    if known == FramePathPrecision::Exact {
        return known;
    }
    let fixed = |segments: &[PlaceSegment]| {
        segments.iter().all(|segment| match segment {
            PlaceSegment::Field { symbol } => symbol.is_valid(),
            PlaceSegment::Case { variant } => variant.is_valid(),
            PlaceSegment::FixedIndex { .. } => true,
            PlaceSegment::Index { .. }
            | PlaceSegment::FixedRange { .. }
            | PlaceSegment::OpaqueRange => false,
        })
    };
    let fixed_presentation = |presentation: &Option<(TypeReferenceHandle, Vec<PlaceSegment>)>| {
        presentation.as_ref().is_none_or(|(_, path)| fixed(path))
    };
    if source.builtin_coordinates
        && fixed(&source.segments)
        && fixed_presentation(&source.representation)
        && source
            .recasts
            .iter()
            .all(|recast| fixed(&recast.segments) && fixed_presentation(&recast.representation))
    {
        FramePathPrecision::Exact
    } else {
        FramePathPrecision::CollectionCoarse
    }
}

/// The source must select the leaf or storage beneath it. Ancestor overlap
/// is useful for invalidating facts, never evidence of a reference value.
pub(in crate::machine_calls::calls::write_frames) fn source_reaches_leaf(
    source: &[PlaceSegment],
    leaf: &[PlaceSegment],
) -> bool {
    source.len() >= leaf.len()
        && source.iter().zip(leaf).all(|(source, leaf)| {
            source == leaf
                || matches!(
                    (source, leaf),
                    (
                        PlaceSegment::Index { .. },
                        PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. }
                    ) | (PlaceSegment::FixedIndex { .. }, PlaceSegment::Index { .. })
                )
        })
}
