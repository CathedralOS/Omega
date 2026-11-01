//! Substitute a formal storage route through one completed actual. Returning a
//! reference and installing a reference slot use the same typed crossings. The
//! destination mode stops at the final descriptor's home; value mode loads it.
use super::Execution;
use arena::Handle;
use checked_trees::{FlowOperandFact, FlowOperandRetention};
use facts::PlaceSegment;

#[derive(Clone, Copy)]
pub(super) enum ReferenceProjection {
    Value,
    Destination,
}

#[derive(Debug, Clone, Copy)]
enum ReferenceProjectionRefusal {
    UnsupportedCoordinate,
    NoCapturedReferenceCrossing,
    OwnedDescriptorDestination,
    MissingCapturedLeaf,
    EmptyActualAddress,
    UnknownProjectedType { position: usize },
    UnavailableProjection { position: usize },
    MissingStoredDescriptor { position: usize },
    EmptyStoredDescriptor { position: usize },
}

impl ReferenceProjectionRefusal {
    fn position(self) -> Option<usize> {
        match self {
            Self::UnknownProjectedType { position }
            | Self::UnavailableProjection { position }
            | Self::MissingStoredDescriptor { position }
            | Self::EmptyStoredDescriptor { position } => Some(position),
            _ => None,
        }
    }
}

pub(super) struct CapturedReferenceProjection {
    pub(super) places: Vec<super::super::CapturedReferencePlace>,
    pub(super) retention: Handle<FlowOperandRetention>,
}

impl Execution<'_, '_, '_> {
    pub(super) fn capture_reference_projection(
        &mut self,
        actual: &FlowOperandFact,
        segments: &[PlaceSegment],
        projection: ReferenceProjection,
    ) -> Option<CapturedReferenceProjection> {
        let result = self.try_capture_reference_projection(actual, segments, projection);
        if let Err(reason) = &result {
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE reference-projection-refusal machine={:?} state={:?} statement={} expression={:?} type={:?} destination={} path_count={} path_prefix={:?} position={:?} reason={:?}",
                    self.machine.symbol,
                    self.state.symbol,
                    self.statement_index,
                    actual.expression,
                    actual.type_reference,
                    matches!(projection, ReferenceProjection::Destination),
                    segments.len(),
                    &segments[..segments.len().min(32)],
                    reason.position(),
                    reason,
                );
            }
        }
        result.ok()
    }

    fn try_capture_reference_projection(
        &mut self,
        actual: &FlowOperandFact,
        segments: &[PlaceSegment],
        projection: ReferenceProjection,
    ) -> Result<CapturedReferenceProjection, ReferenceProjectionRefusal> {
        let read_final = matches!(projection, ReferenceProjection::Value);
        // These are caller-executable typed coordinates. Callee-local dynamic
        // selectors, recast views, and coarse footprints require their own
        // complete relation; none can be treated as an exact field route.
        if !segments.iter().all(|segment| {
            matches!(
                segment,
                PlaceSegment::Field { .. }
                    | PlaceSegment::Case { .. }
                    | PlaceSegment::FixedIndex { .. }
            )
        }) {
            return Err(ReferenceProjectionRefusal::UnsupportedCoordinate);
        }
        let mut start = 0;
        let mut retention;
        let mut places = if self.is_view_type(actual.type_reference) {
            retention = self.captured_retention_at_path(*(actual), &[]);
            self.captured_reference_places(actual.referents)
        } else {
            let length = (1..=segments.len())
                .find(|length| {
                    !matches!(segments[length - 1], PlaceSegment::Case { .. })
                        && validation::project_type_reference_from_segments(
                            self.program,
                            actual.type_reference,
                            &segments[..*length],
                        )
                        .is_some_and(|reference| self.is_view_type(reference))
                })
                .ok_or(ReferenceProjectionRefusal::NoCapturedReferenceCrossing)?;
            // An owned actual supplies the value of a captured descriptor leaf,
            // not a writable address for that leaf in the callee's local copy.
            if !read_final && length == segments.len() {
                return Err(ReferenceProjectionRefusal::OwnedDescriptorDestination);
            }
            start = length;
            retention = self.captured_retention_at_path(*(actual), &segments[..length]);
            // The completed aggregate's descriptor children own its addresses.
            // Resolve their retained epochs before crossing the reference shell;
            // an outer capture need not repeat its children's referents.
            let capture = self.context.control.operands.insert(*actual);
            let selected = validation::completed_storage::reference_captures_at_path(
                self.program,
                &self.context.control,
                capture,
                &segments[..length],
                self.storage_snapshot,
            )
            .and_then(|stored| stored.captures())
            .ok_or(ReferenceProjectionRefusal::MissingCapturedLeaf)?;
            let mut places = Vec::new();
            for descriptor in selected {
                // A completed inline copy retains the parent occurrence and
                // typed residual path. Read that exact captured leaf, including
                // its original formation, rather than requiring a new child.
                let addresses =
                    super::super::operand_values::captured_reference_places_with_formation(
                        self.program,
                        &self.context.control,
                        descriptor.capture,
                        descriptor.reference,
                        &descriptor.projection,
                    )
                    .ok_or(ReferenceProjectionRefusal::MissingCapturedLeaf)?;
                if addresses.is_empty() {
                    return Err(ReferenceProjectionRefusal::EmptyActualAddress);
                }
                for address in addresses {
                    crate::collections::push_unique(&mut places, address);
                }
            }
            places
        };
        if places.is_empty() {
            return Err(ReferenceProjectionRefusal::EmptyActualAddress);
        }
        for (position, segment) in segments.iter().enumerate().skip(start) {
            let projected = validation::project_type_reference_from_segments(
                self.program,
                actual.type_reference,
                &segments[..=position],
            )
            .ok_or(ReferenceProjectionRefusal::UnknownProjectedType { position })?;
            let reads_descriptor = !matches!(segment, PlaceSegment::Case { .. })
                && self.is_view_type(projected)
                && (read_final || position + 1 != segments.len());
            let mut projected_places = Vec::new();
            let mut roots = Vec::new();
            let mut lifetime_known = true;
            for place in places {
                let place = place
                    .projected(std::slice::from_ref(segment))
                    .ok_or(ReferenceProjectionRefusal::UnavailableProjection { position })?;
                if reads_descriptor {
                    let stored = self
                        .stored_view_referents_at_place(&place.place)
                        .ok_or(ReferenceProjectionRefusal::MissingStoredDescriptor { position })?;
                    let stored = self.captured_reference_places(stored);
                    if stored.is_empty() {
                        return Err(ReferenceProjectionRefusal::EmptyStoredDescriptor { position });
                    }
                    let stored_retention = self
                        .stored_retention_at_place(&place.place)
                        .unwrap_or_default();
                    if let Some(retained) = self.context.control.retained_roots(stored_retention) {
                        for root in retained {
                            if !roots.contains(root) {
                                roots.push(*root);
                            }
                        }
                    } else {
                        lifetime_known = false;
                    }
                    for origin in stored {
                        crate::collections::push_unique(&mut projected_places, origin);
                    }
                } else if !projected_places.contains(&place) {
                    projected_places.push(place);
                }
            }
            if reads_descriptor {
                retention = if lifetime_known {
                    self.append_retention(&roots)
                } else {
                    Handle::invalid()
                };
            }
            places = projected_places;
        }
        Ok(CapturedReferenceProjection { places, retention })
    }
}
