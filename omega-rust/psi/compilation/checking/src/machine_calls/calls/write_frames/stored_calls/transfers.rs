//! Publish and install completed slot transfers without re-evaluating their sources.
//! Completed operands retain their exact source cuts and carrier/backing association.

use super::super as frames;
use super::*;

pub(in crate::machine_calls::calls::write_frames) fn public_effects(
    effects: Vec<SlotEffect>,
) -> Option<Vec<StoredReferenceEffect>> {
    effects
        .into_iter()
        .map(|effect| {
            let stored_origins::StoredOriginKind::Reference(access) = effect.kind else {
                return None;
            };
            let sources = effect.sources?;
            let has_static_values = sources
                .iter()
                .any(|source| source.origin.source.is_static() && source.lifetime_access.is_none());
            let incoming_identity = effect.incoming && sources.is_empty();
            Some(StoredReferenceEffect {
                suppliers_complete: sources
                    .iter()
                    .all(|source| source.lifetime_access.is_none()),
                has_static_values,
                destination: LocalWriteOrigin {
                    local_symbol: effect.root,
                    local_segments: effect.segments,
                    source: None,
                },
                sources: sources
                    .into_iter()
                    .filter(|source| !source.origin.source.is_static())
                    .map(|source| StoredReferenceSource {
                        lifetime_access: source.lifetime_access,
                        carrier_symbol: source.carrier.root,
                        carrier_segments: source.carrier.segments,
                        carrier_backing: source.carrier_backing.map(LocalWriteSource::from),
                        backing: LocalWriteSource::from(source.origin),
                    })
                    .collect(),
                retains_incoming: effect.incoming,
                incoming_identity,
                may_select: effect.may_select,
                access,
            })
        })
        .collect()
}

pub(in crate::machine_calls::calls::write_frames) fn apply_transfer(
    program: &dyn crate::CheckingInput,
    stored: &mut Vec<StoredLocalOrigins>,
    transfer: &SlotRelationTransfer,
) {
    // Retire prior case/move epochs before publishing current pointer slots.
    // Only a complete normal route can justify a closed case-frontier update.
    if transfer.is_complete() {
        for effect in &transfer.case_effects {
            effect.install(stored);
        }
    }
    apply(program, stored, &transfer.effects);
}

pub(in crate::machine_calls::calls::write_frames) fn apply(
    program: &dyn crate::CheckingInput,
    stored: &mut Vec<StoredLocalOrigins>,
    effects: &[SlotEffect],
) {
    // Every source above was captured from the same pre-call inventory.
    for effect in effects {
        let index = stored
            .iter()
            .position(|record| record.local_symbol == effect.root)
            .unwrap_or_else(|| {
                stored.push(StoredLocalOrigins {
                    local_symbol: effect.root,
                    references: Vec::new(),
                    cases: Vec::new(),
                    moves: Vec::new(),
                    symbolic: false,
                });
                stored.len() - 1
            });
        let record = &mut stored[index];
        let invalidates_descendants = effect.may_select
            && effect
                .sources
                .as_ref()
                .is_none_or(|sources| !sources.is_empty());
        if effect.sources.is_none() || invalidates_descendants {
            // Exact overlays cannot hide an unknown covering region, including
            // an equal-depth wildcard. Keep each known debt, beside a separate
            // incomplete row at every already-materialized covered coordinate.
            let mut unresolved = Vec::new();
            for leaf in &record.references {
                if stored_origins::path_covers(&effect.segments, &leaf.local_segments)
                    && (effect.sources.is_none()
                        || leaf.local_segments.len() > effect.segments.len())
                {
                    let mut unknown = leaf.clone();
                    unknown.origin = None;
                    unknown.lifetime_dependencies.clear();
                    if !record
                        .references
                        .iter()
                        .chain(unresolved.iter())
                        .any(|prior| {
                            prior.local_segments == unknown.local_segments
                                && prior.kind == unknown.kind
                                && prior.may_select == unknown.may_select
                                && prior.origin.is_none()
                                && prior.lifetime_dependencies.is_empty()
                        })
                    {
                        unresolved.push(unknown);
                    }
                }
            }
            record.references.extend(unresolved);
        }
        // Materialize the old concrete slot before installing an overlay on
        // a conditional write. Keeping only its wildcard default would let
        // the new exact row shadow the no-write branch's surviving origin.
        let incoming = if effect.incoming && !effect.may_select {
            stored_origins::selected_slot_sources(record, &effect.segments, Some(false))
        } else {
            Some(Vec::new())
        };
        let incoming_unknown = incoming.is_none() || effect.sources.is_none();
        // Installing a pointer changes the storage reached beneath it. Old
        // pointee-slot snapshots cannot outlive that replacement, even when
        // another normal route preserves the pointer's incoming contents.
        record.references.retain(|leaf| {
            // Unknown contents authorize poisoning, not replacing a typed row.
            // A later complete definite overwrite may retire these snapshots.
            effect.sources.is_none()
                || effect.may_select
                || (!((!effect.incoming
                    || effect
                        .sources
                        .as_ref()
                        .is_none_or(|sources| !sources.is_empty()))
                    && leaf.local_segments.len() > effect.segments.len()
                    && leaf.local_segments.starts_with(&effect.segments))
                    && (effect.incoming || leaf.local_segments != effect.segments))
        });
        let sources: Vec<_> = incoming
            .into_iter()
            .flatten()
            .chain(effect.sources.iter().flatten().cloned())
            .collect();
        let lifetime_dependencies: Vec<_> = sources
            .iter()
            .filter(|source| source.lifetime_access.is_some())
            .cloned()
            .collect();
        let origins = sources
            .iter()
            .filter(|source| !source.lifetime_access.is_some())
            .map(|source| (Some(source.origin.clone()), lifetime_dependencies.clone()))
            .chain(
                (!lifetime_dependencies.is_empty())
                    .then_some((None, lifetime_dependencies.clone())),
            )
            // A partial debt roster is not a complete opaque dependency answer.
            .chain(incoming_unknown.then_some((None, Vec::new())));
        for (origin, lifetime_dependencies) in origins {
            if record.references.iter().any(|leaf| {
                leaf.local_segments == effect.segments
                    && leaf.kind == effect.kind
                    && leaf.origin == origin
                    && leaf.may_select == effect.may_select
                    && leaf.lifetime_dependencies == lifetime_dependencies
            }) {
                continue;
            }
            record.references.push(StoredWriteOrigin {
                lifetime_dependencies: lifetime_dependencies.clone(),
                kind: effect.kind,
                local_symbol: effect.root,
                local_path: path(
                    program,
                    &FrameSourcePlace {
                        root: effect.root,
                        segments: effect.segments.clone(),
                        builtin_coordinates: true,
                        ..Default::default()
                    },
                ),
                local_segments: effect.segments.clone(),
                may_select: effect.may_select,
                origin,
            });
        }
        if effect.kind == stored_origins::StoredOriginKind::OpaqueRegion {
            record
                .cases
                .retain(|path| !stored_origins::path_covers(&effect.segments, path));
            record.moves.retain(|moved| {
                !stored_origins::path_covers(&effect.segments, &moved.local_segments)
            });
        }
        record.symbolic = false;
    }
}
