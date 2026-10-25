//! Physical backing of a completed descriptor, independent of scalar identity.
//! Both compatibility and write invalidation consume this same association.
//! Enclosures are conservative, every alternative must resolve, and a missing
//! lifetime/address bridge stays unavailable. The caller may resolve an
//! unchanged loan at its capture's own completion cut; explicit unavailable
//! storage never invokes that resolver or recovers an old initializer.
use arena::Handle;
use checked_trees::{CapturedPlace, FlowControlFacts, FlowOperandFact};
use facts::PlaceSegment;

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
    }
    .places(operand, path)
}

struct Resolver<'a, 'resolver> {
    program: &'a dyn crate::CheckingInput,
    control: &'a FlowControlFacts,
    unchanged_loan: &'resolver mut UnchangedLoan<'a>,
    active: Vec<Query>,
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
        let capture = self.control.operands.get(operand);
        if path.is_empty()
            && capture.alternatives.is_empty()
            && crate::reference_shell(self.program, capture.type_reference).is_some()
            && let Some(profile) = self.profile(capture)
            && profile.enclosing_storage.is_valid()
        {
            return self.enclosing(profile.enclosing_storage);
        }
        if path.is_empty()
            && capture.alternatives.is_empty()
            && crate::reference_shell(self.program, capture.type_reference).is_some()
            && let Some(profile) = self.profile(capture)
            && !profile.enclosing_regions.is_empty()
        {
            return self.regions(profile.enclosing_regions);
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
                let descriptor = self.control.operands.get(selected.capture);
                let profile = self.profile(descriptor);
                if profile.is_some_and(|profile| profile.immutable_image)
                    && self
                        .control
                        .retained_roots(descriptor.retention)
                        .is_some_and(|roots| roots.is_empty())
                {
                    continue;
                }
                if let Some(profile) = profile
                    && profile.enclosing_storage.is_valid()
                {
                    self.enclosing(profile.enclosing_storage)?
                } else if let Some(profile) = profile
                    && !profile.enclosing_regions.is_empty()
                {
                    self.regions(profile.enclosing_regions)?
                } else if !descriptor.referents.is_empty() {
                    self.referents(descriptor)?
                } else {
                    let Some(suppliers) = super::descriptor_suppliers(
                        self.control,
                        descriptor.retention,
                        &mut Vec::new(),
                    ) else {
                        needs_unchanged_loan = true;
                        continue;
                    };
                    if suppliers.is_empty() {
                        return None;
                    }
                    let mut places = Vec::new();
                    for supplier in suppliers {
                        let projection = self
                            .control
                            .operand_retained_paths
                            .span(supplier.projection)?;
                        places.extend(self.places(supplier.operand, projection)?);
                    }
                    places
                }
            };
            for place in places {
                if !output.contains(&place) {
                    output.push(place);
                }
            }
        }
        if needs_unchanged_loan {
            // Resolve the original observation at its completion, not a
            // producer before its loan existed or a later replaced home.
            for place in (self.unchanged_loan)(operand, path)? {
                if !output.contains(&place) {
                    output.push(place);
                }
            }
        }
        Some(output)
    }

    fn profile(&self, capture: &FlowOperandFact) -> Option<checked_trees::FlowOperandRetention> {
        self.control
            .operand_retentions
            .is_valid(capture.retention)
            .then(|| *self.control.operand_retentions.get(capture.retention))
    }

    /// Every candidate contributes to the conservative footprint. One missing
    /// candidate withholds coverage; no region is an exact pointer or permission.
    fn regions(
        &mut self,
        regions: arena::HandleSpan<checked_trees::FlowOperandRetainedRegion>,
    ) -> Option<Vec<CapturedPlace>> {
        let mut output = Vec::new();
        for region in self
            .control
            .operand_retained_regions
            .span(regions)?
            .to_vec()
        {
            let projection = self
                .control
                .operand_retained_paths
                .span(region.projection)?;
            for place in self.places(region.operand, projection)? {
                if !output.contains(&place) {
                    output.push(place);
                }
            }
        }
        Some(output)
    }

    fn enclosing(&mut self, enclosing: Handle<FlowOperandFact>) -> Option<Vec<CapturedPlace>> {
        if !self.control.operands.is_valid(enclosing) {
            return None;
        }
        let storage = self.control.operands.get(enclosing);
        if crate::reference_shell(self.program, storage.type_reference).is_some()
            || crate::slice_view_shell(self.program, storage.type_reference, &[]).is_some()
        {
            return self.places(enclosing, &[]);
        }
        // Owned bytes lent at formation are an address, not a descriptor
        // stored in those bytes. Their contents need not contain a pointer.
        self.referents(storage)
    }

    fn referents(&self, capture: &FlowOperandFact) -> Option<Vec<CapturedPlace>> {
        if capture.referents.is_empty() {
            return None;
        }
        self.control
            .operand_referents
            .span(capture.referents)?
            .iter()
            .map(|referent| {
                Some(CapturedPlace {
                    incoming_reference: Handle::invalid(),
                    root: referent.root,
                    segments: self
                        .control
                        .operand_referent_segments
                        .span(referent.segments)?
                        .to_vec(),
                })
            })
            .collect()
    }
}
