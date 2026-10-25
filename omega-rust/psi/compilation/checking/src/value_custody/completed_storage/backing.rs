//! Physical backing of a completed descriptor, independent of scalar identity.
//! Both compatibility and write invalidation consume this same association.
//! Every selected/profile alternative must resolve. Frozen descriptor geometry
//! stays separate from deferred inventory: only the caller can authenticate a
//! loan at its original completion cut. No lifetime root supplies an address.
use arena::{Handle, HandleSpan};
use checked_trees::{CapturedPlace, FlowControlFacts, FlowOperandFact};
use facts::PlaceSegment;

use super::{FrozenReferenceSelection, frozen_reference_leaf};

type Query = (Handle<FlowOperandFact>, Vec<PlaceSegment>);
type UnchangedLoan<'a> =
    dyn FnMut(Handle<FlowOperandFact>, &[PlaceSegment]) -> Option<Vec<CapturedPlace>> + 'a;

pub fn reference_backing_places(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    operand: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    unchanged_loan: &mut UnchangedLoan<'_>,
) -> Option<Vec<CapturedPlace>> {
    Resolver {
        program,
        control,
        unchanged_loan,
        active: Vec::new(),
        active_profiles: Vec::new(),
    }
    .places(operand, path)
}

struct Resolver<'a, 'resolver> {
    program: &'a dyn crate::CheckingInput,
    control: &'a FlowControlFacts,
    unchanged_loan: &'resolver mut UnchangedLoan<'a>,
    active: Vec<Query>,
    active_profiles: Vec<Handle<checked_trees::FlowOperandRetention>>,
}

/// Scratch result of a complete retention profile. Unchanged is distinct from
/// a proved empty immutable image; mixing it with resolved alternatives cannot
/// silently drop the alternative whose pointer was unavailable.
enum ProfileBacking {
    Known(Vec<CapturedPlace>),
    Unchanged,
}

impl Resolver<'_, '_> {
    fn places(
        &mut self,
        operand: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
    ) -> Option<Vec<CapturedPlace>> {
        let query = (operand, path.to_vec());
        if self.active.contains(&query) || !self.control.operands.is_valid(operand) {
            return None;
        }
        self.active.push(query);
        let result = self.resolve(operand, path);
        self.active.pop();
        result
    }

    fn resolve(
        &mut self,
        operand: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
    ) -> Option<Vec<CapturedPlace>> {
        let capture = *self.control.operands.get(operand);
        if let checked_trees::FlowOperandCompletion::Selected(alternatives) = capture.completion {
            let rows = self.control.operand_alternatives.span(alternatives)?;
            if rows.is_empty() || rows.iter().any(|row| !row.source_arm.is_valid()) {
                return None;
            }
            let values = rows.iter().map(|row| row.value).collect::<Vec<_>>();
            let mut output = Vec::new();
            for value in values {
                Self::append(&mut output, self.places(value, path)?);
            }
            return Some(output);
        }
        match frozen_reference_leaf(self.program, self.control, operand, path) {
            FrozenReferenceSelection::Unavailable => return None,
            FrozenReferenceSelection::Endpoint { leaf, .. } => {
                if leaf.referents.is_empty() {
                    let profiles = super::captured_retention_profiles_at_path(
                        self.program,
                        self.control,
                        &capture,
                        path,
                    )?;
                    return (!profiles.is_empty()
                        && profiles.iter().all(|profile| {
                            self.control.operand_retentions.is_valid(*profile)
                                && self
                                    .control
                                    .operand_retentions
                                    .get(*profile)
                                    .immutable_image
                                && self
                                    .control
                                    .retained_roots(*profile)
                                    .is_some_and(|roots| roots.is_empty())
                        }))
                    .then_some(Vec::new());
                }
                return self.referents_or_loan(operand, path, leaf.referents);
            }
            FrozenReferenceSelection::NotFrozen | FrozenReferenceSelection::ThroughReference => {}
        }
        if path.is_empty()
            && crate::reference_shell(self.program, capture.type_reference).is_some()
            && let Some(profile) = self.profile(&capture)
            && profile.alternatives.is_empty()
            && profile.enclosing_storage.is_valid()
        {
            return self.enclosing(profile.enclosing_storage);
        }
        let Some(selected) = super::reference_captures_at_path(
            self.program,
            self.control,
            operand,
            path,
            capture.storage_snapshot,
        ) else {
            return (self.unchanged_loan)(operand, path);
        };
        let selected = selected.captures()?;
        if selected.is_empty() {
            return None;
        }
        let mut output = Vec::new();
        let mut needs_unchanged_loan = false;
        for selected in selected {
            let places = if !selected.projection.is_empty() {
                self.places(selected.capture, &selected.projection)?
            } else {
                if !self.control.operands.is_valid(selected.capture) {
                    return None;
                }
                let descriptor = *self.control.operands.get(selected.capture);
                let profile = self.profile(&descriptor);
                if profile.is_some_and(|profile| !profile.alternatives.is_empty()) {
                    match self.profile_backing(descriptor.retention)? {
                        ProfileBacking::Known(places) => places,
                        ProfileBacking::Unchanged => {
                            needs_unchanged_loan = true;
                            continue;
                        }
                    }
                } else if profile.is_some_and(|profile| profile.immutable_image)
                    && self
                        .control
                        .retained_roots(descriptor.retention)
                        .is_some_and(|roots| roots.is_empty())
                {
                    continue;
                } else if let Some(profile) = profile
                    && profile.enclosing_storage.is_valid()
                {
                    self.enclosing(profile.enclosing_storage)?
                } else if !descriptor.referents.is_empty() {
                    self.referents_or_loan(selected.capture, &[], descriptor.referents)?
                } else {
                    match self.profile_backing(descriptor.retention)? {
                        ProfileBacking::Known(places) => places,
                        ProfileBacking::Unchanged => {
                            needs_unchanged_loan = true;
                            continue;
                        }
                    }
                }
            };
            Self::append(&mut output, places);
        }
        if needs_unchanged_loan {
            // The original query identifies its completion cut, not an older
            // producer before loan formation or a newer same-slot occupant.
            Self::append(&mut output, (self.unchanged_loan)(operand, path)?);
        }
        Some(output)
    }

    fn profile(&self, capture: &FlowOperandFact) -> Option<checked_trees::FlowOperandRetention> {
        self.control
            .operand_retentions
            .is_valid(capture.retention)
            .then(|| *self.control.operand_retentions.get(capture.retention))
    }

    fn profile_backing(
        &mut self,
        profile: Handle<checked_trees::FlowOperandRetention>,
    ) -> Option<ProfileBacking> {
        if !profile.is_valid() {
            return Some(ProfileBacking::Unchanged);
        }
        if !self.control.operand_retentions.is_valid(profile) {
            return None;
        }
        if self.active_profiles.contains(&profile) {
            return None;
        }
        self.active_profiles.push(profile);
        let result = self.resolve_profile(profile);
        self.active_profiles.pop();
        result
    }

    fn resolve_profile(
        &mut self,
        profile: Handle<checked_trees::FlowOperandRetention>,
    ) -> Option<ProfileBacking> {
        let row = *self.control.operand_retentions.get(profile);
        if !row.alternatives.is_empty() {
            // Alternatives are profiles, not fake evaluated child operands.
            self.control.operand_retentions.span(row.alternatives)?;
            let handles = row.alternatives.handles().collect::<Vec<_>>();
            let mut output = Vec::new();
            let mut known = false;
            let mut unchanged = false;
            for profile in handles {
                match self.profile_backing(profile)? {
                    ProfileBacking::Known(places) => {
                        known = true;
                        Self::append(&mut output, places);
                    }
                    ProfileBacking::Unchanged => unchanged = true,
                }
            }
            return if known && unchanged {
                None
            } else if unchanged {
                Some(ProfileBacking::Unchanged)
            } else {
                Some(ProfileBacking::Known(output))
            };
        }
        if row.immutable_image
            && self
                .control
                .retained_roots(profile)
                .is_some_and(|roots| roots.is_empty())
        {
            return Some(ProfileBacking::Known(Vec::new()));
        }
        if row.enclosing_storage.is_valid() {
            return Some(ProfileBacking::Known(
                self.enclosing(row.enclosing_storage)?,
            ));
        }
        if row.descriptor.is_valid() {
            return Some(ProfileBacking::Known(self.places(row.descriptor, &[])?));
        }
        if row.suppliers.is_empty() {
            return Some(ProfileBacking::Unchanged);
        }
        let suppliers = self
            .control
            .operand_retained_suppliers
            .span(row.suppliers)?
            .to_vec();
        if suppliers.is_empty() {
            return None;
        }
        let mut output = Vec::new();
        for supplier in suppliers {
            let path = self
                .control
                .operand_retained_paths
                .span(supplier.projection)?
                .to_vec();
            Self::append(&mut output, self.places(supplier.operand, &path)?);
        }
        Some(ProfileBacking::Known(output))
    }

    fn enclosing(&mut self, enclosing: Handle<FlowOperandFact>) -> Option<Vec<CapturedPlace>> {
        if !self.control.operands.is_valid(enclosing) {
            return None;
        }
        let storage = *self.control.operands.get(enclosing);
        if crate::reference_shell(self.program, storage.type_reference).is_some()
            || crate::slice_view_shell(self.program, storage.type_reference, &[]).is_some()
        {
            return self.places(enclosing, &[]);
        }
        // Owned bytes supply original address geometry, not a descriptor in
        // their contents. Deferred inventory still requires its checked loan.
        self.referents_or_loan(enclosing, &[], storage.referents)
    }

    fn referents_or_loan(
        &mut self,
        operand: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
        span: HandleSpan<checked_trees::FlowOperandReferent>,
    ) -> Option<Vec<CapturedPlace>> {
        let referents = self.control.operand_referents.span(span)?;
        if referents.is_empty() {
            return None;
        }
        for referent in referents {
            if referent.formation.is_valid() {
                if !self
                    .control
                    .referent_formations
                    .is_valid(referent.formation)
                {
                    return None;
                }
                let formation = self.control.referent_formations.get(referent.formation);
                if formation.owner.is_valid() && !self.control.operands.is_valid(formation.owner) {
                    return None;
                }
            }
        }
        let deferred = |referent: &checked_trees::FlowOperandReferent| {
            self.control
                .referent_formations
                .is_valid(referent.formation)
                && self
                    .control
                    .referent_formations
                    .get(referent.formation)
                    .owner
                    .is_valid()
        };
        if referents.iter().any(deferred) {
            if !referents.iter().all(deferred) {
                return None;
            }
            return (self.unchanged_loan)(operand, path);
        }
        direct_referent_places(self.control, span)
    }

    fn append(output: &mut Vec<CapturedPlace>, places: Vec<CapturedPlace>) {
        for place in places {
            if !output.contains(&place) {
                output.push(place);
            }
        }
    }
}

/// Geometry explicitly captured by a producer, without deferred inventory.
/// An invalid nonzero formation or any owned-inventory namespace withholds
/// these coordinates. Zero formation is the producer's direct geometry lane;
/// this query supplies no case, permission, membership or lifetime evidence.
pub fn direct_referent_places(
    control: &FlowControlFacts,
    span: HandleSpan<checked_trees::FlowOperandReferent>,
) -> Option<Vec<CapturedPlace>> {
    let referents = control.operand_referents.span(span)?;
    if referents.is_empty() {
        return None;
    }
    let mut output = Vec::new();
    for referent in referents {
        if !referent.root.is_valid() {
            return None;
        }
        if referent.formation.is_valid() {
            if !control.referent_formations.is_valid(referent.formation)
                || control
                    .referent_formations
                    .get(referent.formation)
                    .owner
                    .is_valid()
            {
                return None;
            }
        }
        let place = CapturedPlace {
            incoming_reference: Handle::invalid(),
            root: referent.root,
            segments: control
                .operand_referent_segments
                .span(referent.segments)?
                .to_vec(),
        };
        if !output.contains(&place) {
            output.push(place);
        }
    }
    Some(output)
}
