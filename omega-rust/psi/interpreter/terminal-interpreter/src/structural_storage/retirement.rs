//! Reclaim a retired home's descriptor cells through actual retained storage.
//! Home retirement does not dispose a referent or restore a loan window. A
//! captured outgoing value can still name this storage after its binding ends.
//! Resident frames and prepared outputs are roots; descriptor cells are edges,
//! so unrelated dead descriptors cannot keep one another alive.

use crate::execution::{CleanupContinuation, SuspendedCallResult, TerminalExecution};
use crate::results::TerminalExecutionResult;
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};
use std::ops::Range;
use terminal_psi::{ReferenceOriginProjection, StructuralPathSegment};

#[derive(Clone, PartialEq, Eq)]
enum Region {
    Subtree(StructuralRuntimePlace),
    Projection {
        root: StructuralRuntimePlace,
        path: Vec<ReferenceOriginProjection>,
    },
    Elements {
        collection: StructuralRuntimePlace,
        window: Range<usize>,
        path: Vec<ReferenceOriginProjection>,
    },
}

impl Region {
    fn root(&self) -> &StructuralRuntimePlace {
        match self {
            Self::Subtree(root) | Self::Projection { root, .. } => root,
            Self::Elements { collection, .. } => collection,
        }
    }

    /// Byte-field cells retain canonical declaration IDs, not path identities.
    /// Without an authenticated field projection, retain at the reached physical
    /// parent. This may keep sibling byte bindings at that parent alive; it is
    /// a reclamation bound, never field identity, access or disjointness proof.
    fn may_retain_field_parent(&self, parent: &StructuralRuntimePlace) -> bool {
        let root = self.root();
        root.opaque_identity == parent.opaque_identity
            && (root.path.starts_with(&parent.path) || self.selected_suffix(parent).is_some())
    }

    /// The remaining logical selection after reaching this physical carrier.
    /// Case tags do not occupy storage-path segments; declaration-keyed payload
    /// fields keep their own identities. A view edge may still owe an element
    /// selection, while a reference edge is followed only at the endpoint.
    fn selected_suffix<'region>(
        &'region self,
        place: &StructuralRuntimePlace,
    ) -> Option<&'region [ReferenceOriginProjection]> {
        let root = self.root();
        if place.opaque_identity != root.opaque_identity {
            return None;
        }
        let mut storage = place.path.strip_prefix(root.path.as_slice())?;
        let path = match self {
            Self::Subtree(_) => return Some(&[]),
            Self::Projection { path, .. } => path,
            Self::Elements { window, path, .. } => {
                let (StructuralPathSegment::FixedIndex(index), suffix) = storage.split_first()?
                else {
                    return None;
                };
                if !usize::try_from(*index).is_ok_and(|index| window.contains(&index)) {
                    return None;
                }
                storage = suffix;
                path
            }
        };
        let mut projection = path.as_slice();
        loop {
            while matches!(
                projection.first(),
                Some(ReferenceOriginProjection::Exact(
                    StructuralPathSegment::Case(_)
                ))
            ) {
                projection = &projection[1..];
            }
            let Some((selected, remaining)) = projection.split_first() else {
                return Some(&[]);
            };
            let Some((actual, suffix)) = storage.split_first() else {
                return Some(projection);
            };
            let matches = match selected {
                ReferenceOriginProjection::Exact(selected) => selected == actual,
                ReferenceOriginProjection::Element => {
                    matches!(actual, StructuralPathSegment::FixedIndex(_))
                }
            };
            if !matches {
                return None;
            }
            projection = remaining;
            storage = suffix;
        }
    }
}

/// Construction scratch shared by resident frames, outgoing transactions and
/// descriptor payload owners. A region describes storage, never authority.
#[derive(Default)]
pub(crate) struct RetainedStorage {
    regions: Vec<Region>,
    frame_arrays: Vec<(usize, semantic_vocabulary::PlaceId)>,
}

impl RetainedStorage {
    pub(crate) fn frame_array(&mut self, depth: usize, place: semantic_vocabulary::PlaceId) {
        if !self.frame_arrays.contains(&(depth, place)) {
            self.frame_arrays.push((depth, place));
        }
    }

    pub(crate) fn primitive_home(&mut self, home: crate::primitive_storage::PrimitiveStorageHome) {
        let (depth, place, _) = home.scalar_array_frame();
        self.frame_array(depth, place);
    }

    pub(crate) fn array_storage(
        &mut self,
        array: &crate::structural_inputs::byte_arrays::storage::ArrayStorage,
    ) {
        if let crate::structural_inputs::byte_arrays::storage::ArrayStorageHome::Frame {
            depth,
            place,
            ..
        } = array.home
        {
            self.frame_array(depth, place);
        }
    }

    pub(crate) fn primitive_endpoint(
        &mut self,
        endpoint: &crate::primitive_storage::PrimitiveStorageEndpoint,
    ) {
        use crate::primitive_storage::PrimitiveStorageEndpoint;
        match endpoint {
            PrimitiveStorageEndpoint::OwnedArrayElement { home, .. } => self.primitive_home(*home),
            PrimitiveStorageEndpoint::ArrayElement { array, .. } => self.array_storage(array),
            _ => {}
        }
    }

    pub(crate) fn byte_binding(
        &mut self,
        binding: &crate::byte_sequences::binding::ByteSequenceBinding,
    ) {
        if let crate::byte_sequences::binding::ByteSequenceBinding::MutableArray { array, .. } =
            binding
        {
            self.array_storage(array);
        }
    }

    fn insert(&mut self, region: Region) {
        if !self.regions.contains(&region) {
            self.regions.push(region);
        }
    }

    pub(crate) fn value(&mut self, value: &TerminalStructuralValue) {
        self.place(StructuralRuntimePlace::from(value));
    }

    fn place(&mut self, place: StructuralRuntimePlace) {
        self.insert(Region::Subtree(place));
    }

    pub(crate) fn projection(
        &mut self,
        value: &TerminalStructuralValue,
        path: &[ReferenceOriginProjection],
    ) {
        if path.is_empty() {
            self.value(value);
        } else {
            self.insert(Region::Projection {
                root: StructuralRuntimePlace::from(value),
                path: path.to_vec(),
            });
        }
    }

    pub(crate) fn elements(
        &mut self,
        collection: &TerminalStructuralValue,
        window: Range<usize>,
        path: &[ReferenceOriginProjection],
    ) {
        self.insert(Region::Elements {
            collection: StructuralRuntimePlace::from(collection),
            window,
            path: path.to_vec(),
        });
    }

    pub(crate) fn aggregate_backing(&mut self, backing: &crate::reference::AggregateBacking) {
        self.value(&backing.value);
        if let Some(region) = &backing.byte_region {
            self.primitive_endpoint(&region.endpoint);
        }
    }

    pub(crate) fn reference(&mut self, reference: &crate::reference::ReferenceReferent) {
        self.value(&reference.view);
        if let Some(loan) = &reference.primitive_loan {
            self.primitive_endpoint(&loan.storage.endpoint);
        }
        if let Some(region) = &reference.byte_region {
            self.primitive_endpoint(&region.endpoint);
        }
        if let Some(binding) = &reference.byte_owner {
            self.byte_binding(binding);
        }
        if let Some(view) = &reference.collection_loan {
            view.retain_storage(self);
        }
    }

    fn continuation(&mut self, continuation: &CleanupContinuation) {
        match continuation {
            CleanupContinuation::Return(returned) => {
                if let TerminalExecutionResult::Structural(value) = &returned.value {
                    self.value(&value.value);
                }
                if let Some(binding) = &returned.byte_sequence {
                    self.byte_binding(binding);
                }
                if let Some(view) = &returned.element_view {
                    view.retain_storage(self);
                }
            }
            CleanupContinuation::Successor { bindings, .. } => bindings.retain_storage(self),
        }
    }

    fn reference_is_retained(&self, place: &StructuralRuntimePlace) -> bool {
        self.regions
            .iter()
            .any(|region| region.selected_suffix(place).is_some_and(<[_]>::is_empty))
    }

    fn view_is_retained(&self, place: &StructuralRuntimePlace) -> bool {
        self.regions.iter().any(|region| {
            region.selected_suffix(place).is_some_and(|suffix| {
                suffix.is_empty()
                    || matches!(
                        suffix.first(),
                        Some(
                            ReferenceOriginProjection::Element
                                | ReferenceOriginProjection::Exact(
                                    StructuralPathSegment::FixedIndex(_)
                                )
                        )
                    )
            })
        })
    }
}

impl TerminalExecution<'_> {
    fn descriptor_cells_exist(&self, root: &StructuralRuntimePlace) -> bool {
        super::cells(&self.reference_referents, root)
            .next()
            .is_some()
            || super::cells(&self.structural_element_view_fields, root)
                .next()
                .is_some()
    }

    /// Queue only an actual completed binding retirement. The candidate itself
    /// never retains storage. A surviving borrower can defer reclamation until
    /// its descriptor ends, even when no second RetireHome names this home.
    pub(crate) fn queue_retired_descriptor_home(&mut self, root: StructuralRuntimePlace) {
        if self.descriptor_cells_exist(&root) && !self.retired_descriptor_homes.contains(&root) {
            self.retired_descriptor_homes.push(root);
        }
    }

    /// Only stable operation boundaries and prepared cleanup transactions call
    /// this collector. Captured outgoing values join all resident/saved roots;
    /// no half-built constructor or uninstalled call input is observable here.
    pub(crate) fn collect_retired_descriptor_storage(
        &mut self,
        continuation: Option<&CleanupContinuation>,
    ) {
        self.collect_retired_storage(continuation, None);
    }

    pub(crate) fn collect_retired_successor_storage(
        &mut self,
        bindings: &crate::terminators::block_bindings::BlockBindings,
    ) {
        self.collect_retired_storage(None, Some(bindings));
    }

    fn collect_retired_storage(
        &mut self,
        continuation: Option<&CleanupContinuation>,
        bindings: Option<&crate::terminators::block_bindings::BlockBindings>,
    ) {
        let has_retired_arrays = self.scalar_array_values.has_retired()
            || self
                .call_stack
                .iter()
                .any(|frame| frame.scalar_array_values.has_retired());
        if self.retired_descriptor_homes.is_empty() && !has_retired_arrays {
            return;
        }
        self.retired_descriptor_homes = std::mem::take(&mut self.retired_descriptor_homes)
            .into_iter()
            .filter(|root| self.descriptor_cells_exist(root))
            .collect();
        if self.retired_descriptor_homes.is_empty() && !has_retired_arrays {
            return;
        }
        let mut retained = RetainedStorage::default();
        for value in self.structural_values.values() {
            retained.value(value);
        }
        for (place, backing) in &self.aggregate_argument_backings {
            if self.structural_values.contains_key(place) {
                retained.aggregate_backing(backing);
            }
        }
        for occurrence in self.placed_view_occurrences.values() {
            retained.place(occurrence.referent.clone());
        }
        for (place, view) in &self.element_view_values {
            if self.structural_values.contains_key(place) {
                view.retain_storage(&mut retained);
            }
        }
        for (place, loan) in &self.primitive_loan_values {
            if self.structural_values.contains_key(place) {
                retained.primitive_endpoint(&loan.storage.endpoint);
            }
        }
        for (place, binding) in &self.byte_sequence_values {
            if self.structural_values.contains_key(place) {
                retained.byte_binding(binding);
            }
        }
        for original in &self.private_entry_ram {
            retained.value(&original.allocation);
        }
        for frame in &self.call_stack {
            for value in frame.structural_values.values() {
                retained.value(value);
            }
            for (place, backing) in &frame.aggregate_argument_backings {
                if frame.structural_values.contains_key(place) {
                    retained.aggregate_backing(backing);
                }
            }
            for occurrence in frame.placed_view_occurrences.values() {
                retained.place(occurrence.referent.clone());
            }
            for (place, view) in &frame.element_view_values {
                if frame.structural_values.contains_key(place) {
                    view.retain_storage(&mut retained);
                }
            }
            for (place, loan) in &frame.primitive_loan_values {
                if frame.structural_values.contains_key(place) {
                    retained.primitive_endpoint(&loan.storage.endpoint);
                }
            }
            for (place, binding) in &frame.byte_sequence_values {
                if frame.structural_values.contains_key(place) {
                    retained.byte_binding(binding);
                }
            }
            match &frame.result {
                SuspendedCallResult::Structural {
                    expected_reference_backings,
                    ..
                } => {
                    for backing in expected_reference_backings.iter().flatten() {
                        backing.retain_storage(&mut retained);
                    }
                }
                SuspendedCallResult::NominalCleanups {
                    completed,
                    continuation,
                    ..
                } => {
                    retained.value(&completed.1);
                    retained.continuation(continuation);
                }
                _ => {}
            }
        }
        if let Some(continuation) = continuation {
            retained.continuation(continuation);
        }
        if let Some(bindings) = bindings {
            bindings.retain_storage(&mut retained);
        }
        if let Some(TerminalExecutionResult::Structural(value)) = &self.result {
            retained.value(&value.value);
        }

        // Each region is processed once. Indexed subtree lookup avoids a scan
        // of historical unrelated storage for each retiring descriptor.
        let mut cursor = 0;
        while let Some(region) = retained.regions.get(cursor).cloned() {
            cursor += 1;
            for (carrier, reference) in super::cells(&self.reference_referents, region.root()) {
                if region.selected_suffix(carrier).is_some_and(<[_]>::is_empty) {
                    retained.reference(reference);
                }
            }
            // Index the same existing opaque identity, including byte cells
            // whose key names an ancestor parent of the selected descriptor.
            // This lower-bound key is not a newly established runtime home.
            let field_namespace = <StructuralRuntimePlace as super::StorageKey>::identity_start(
                region.root().opaque_identity,
            );
            for (field, binding) in
                super::cells(&self.structural_byte_sequence_fields, &field_namespace)
            {
                if region.may_retain_field_parent(&field.parent) {
                    retained.byte_binding(binding);
                }
            }
            for (carrier, binding) in super::cells(&self.structural_byte_descriptors, region.root())
            {
                if region.selected_suffix(carrier).is_some_and(<[_]>::is_empty) {
                    retained.byte_binding(binding);
                }
            }
            for (carrier, view) in super::cells(&self.structural_element_view_fields, region.root())
            {
                if let Some(suffix) = region.selected_suffix(carrier) {
                    view.retain_projection_storage(&mut retained, suffix);
                }
            }
        }
        let current_depth = self.call_stack.len();
        self.scalar_array_values
            .collect_retired(|place| retained.frame_arrays.contains(&(current_depth, place)));
        for (depth, frame) in self.call_stack.iter_mut().enumerate() {
            frame
                .scalar_array_values
                .collect_retired(|place| retained.frame_arrays.contains(&(depth, place)));
        }
        let candidates = std::mem::take(&mut self.retired_descriptor_homes);
        for root in candidates {
            let references: Vec<_> = super::cells(&self.reference_referents, &root)
                .filter(|(carrier, _)| !retained.reference_is_retained(carrier))
                .map(|(carrier, _)| carrier.clone())
                .collect();
            let views: Vec<_> = super::cells(&self.structural_element_view_fields, &root)
                .filter(|(carrier, _)| !retained.view_is_retained(carrier))
                .map(|(carrier, _)| carrier.clone())
                .collect();
            for carrier in references {
                self.reference_referents.remove(&carrier);
            }
            for carrier in views {
                self.structural_element_view_fields.remove(&carrier);
            }
            if self.descriptor_cells_exist(&root) {
                self.retired_descriptor_homes.push(root);
            }
        }
    }
}
