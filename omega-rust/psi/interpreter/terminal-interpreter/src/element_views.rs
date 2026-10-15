//! Frame-local views over contiguous structural collections.
//!
//! Ordinary views retain their original storage identity, activation home and
//! collection path, regardless of the element type. A scalar snapshot cannot
//! supply an element loan's backing and goes stale when a mutable descendant
//! returns; reads and writes therefore share the same live collection owner.
//! Byte representation views decode their original allocation;
//! typed representation views map target leaves to original typed cells using the
//! independently reconstructed validity relation and padded layout. A window
//! selects the same backing across local bindings,
//! calls and state transfer, without relying on a frame-local source place ID.
//! The verified loan controls interference; execution preserves its access and
//! exact bounds when resolving a field below the descriptor.
//! Direct and recast record views resolve scalar cells through the same original
//! declaration. A record backing retains its activation home as well as its
//! structural identity, so nested primitive leaves and record fields both write
//! through after calls without manufacturing a second element snapshot.

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{OperationFlow, TerminalExecution};
use crate::scalar_operations::terminal_scalar_belongs_to_type;
use crate::values::StructuralScalarRuntimeField;
use crate::values::TerminalScalarValue;
use crate::values::TerminalStructuralValue;
use semantic_vocabulary::IntegerSign;
use semantic_vocabulary::IntegerType;
use semantic_vocabulary::IntegerValue;
use semantic_vocabulary::ScalarType;
use semantic_vocabulary::StructuralPlaceKind;
use semantic_vocabulary::ValueId;
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;
use terminal_psi::ElementViewRegion;
use terminal_psi::Operation;
use terminal_psi::OperationKind;
use terminal_psi::OperationResult;
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::StructuralPathSegment;
use terminal_psi::StructuralTypeShape;

/// Original scalar storage retained by a view. Access belongs to the descriptor;
/// a shared descendant can keep this home for current reads without write authority.
#[derive(Clone)]
pub(crate) struct ElementViewHome {
    pub(crate) storage: crate::primitive_storage::PrimitiveStorageHome,
    pub(crate) path: Vec<StructuralPathSegment>,
}

impl ElementViewHome {
    fn selected_path(
        &self,
        position: usize,
        suffix: &[StructuralPathSegment],
    ) -> Result<Vec<StructuralPathSegment>, TerminalInterpretError> {
        let mut path = Vec::with_capacity(self.path.len() + 1 + suffix.len());
        path.extend_from_slice(&self.path);
        path.push(StructuralPathSegment::FixedIndex(
            u64::try_from(position).map_err(|_| malformed())?,
        ));
        path.extend_from_slice(suffix);
        Ok(path)
    }
}

#[derive(Clone)]
pub(crate) struct ElementView {
    backing: ElementViewBacking,
    window: Range<usize>,
    home: Option<Arc<ElementViewHome>>,
    descriptor_home: Option<TerminalStructuralValue>,
    access: StructuralAccess,
}

#[derive(Clone)]
enum ElementViewBacking {
    Structural { collection: TerminalStructuralValue },
    Typed(Arc<typed::TypedElementView>),
    Representation(Arc<representation::RepresentationView>),
}

impl ElementView {
    /// Freeze a projection in the supplying home, rather than retaining a
    /// path through a descriptor that can subsequently be replaced.
    pub(crate) fn captured_storage_home(
        &self,
        types: &crate::program::ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
        path: &[StructuralPathSegment],
    ) -> Result<ElementViewHome, TerminalInterpretError> {
        let home = self.home.as_ref().ok_or_else(malformed)?;
        if path.is_empty() {
            // Only a complete fixed-array companion can become a whole-array
            // home. A partial/nonzero window must retain its selected offset.
            // This validates placement and existing access, not the payload.
            self.structural_projection(types, path, self.access != StructuralAccess::SharedBorrow)?;
            return Ok((**home).clone());
        }
        let Some((StructuralPathSegment::FixedIndex(element_index), suffix)) = path.split_first()
        else {
            return Err(malformed());
        };
        let element_index = usize::try_from(*element_index).map_err(|_| malformed())?;
        if element_index >= self.len() {
            return Err(malformed());
        }
        Ok(ElementViewHome {
            storage: home.storage,
            path: home.selected_path(
                self.window
                    .start
                    .checked_add(element_index)
                    .ok_or_else(malformed)?,
                suffix,
            )?,
        })
    }

    pub(crate) fn structural(
        collection: TerminalStructuralValue,
        length: usize,
        access: StructuralAccess,
        home: ElementViewHome,
    ) -> Self {
        Self {
            backing: ElementViewBacking::Structural { collection },
            window: 0..length,
            home: Some(Arc::new(home)),
            descriptor_home: None,
            access,
        }
    }

    /// Resolve a selected element into its original collection. Selectors have
    /// already been evaluated by the operation-wide runtime-index owner.
    pub(crate) fn structural_projection(
        &self,
        types: &crate::program::ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
        path: &[StructuralPathSegment],
        writing: bool,
    ) -> Result<(TerminalStructuralValue, Vec<StructuralPathSegment>), TerminalInterpretError> {
        let ElementViewBacking::Structural { collection } = &self.backing else {
            return Err(malformed());
        };
        let requested = if writing {
            StructuralAccess::WriteOnlyBorrow
        } else {
            StructuralAccess::SharedBorrow
        };
        if !requested.admits_referent(self.access) {
            return Err(malformed());
        }
        if path.is_empty() {
            // A fixed-array formal uses this descriptor as a storage companion,
            // not as an unsized view. Only its exact complete collection can be
            // replaced whole; a slice descriptor never owns its backing array.
            let home = self.descriptor_home.as_ref().ok_or_else(malformed)?;
            let StructuralTypeShape::FixedArray { length, .. } = types
                .get(&collection.structural_type)
                .ok_or_else(malformed)?
                .shape
            else {
                return Err(malformed());
            };
            if home.structural_type != collection.structural_type
                || self.window.start != 0
                || u64::try_from(self.window.end).ok() != Some(length)
                || self.home.is_none()
            {
                return Err(malformed());
            }
            return Ok((collection.clone(), Vec::new()));
        }
        let Some((StructuralPathSegment::FixedIndex(index), suffix)) = path.split_first() else {
            return Err(malformed());
        };
        let index = usize::try_from(*index).map_err(|_| malformed())?;
        if index >= self.len() {
            return Err(malformed());
        }
        let selected = self.window.start.checked_add(index).ok_or_else(malformed)?;
        let mut path = Vec::with_capacity(path.len());
        path.push(StructuralPathSegment::FixedIndex(
            u64::try_from(selected).map_err(|_| malformed())?,
        ));
        path.extend_from_slice(suffix);
        Ok((collection.clone(), path))
    }

    pub(crate) fn with_descriptor_home(mut self, home: TerminalStructuralValue) -> Self {
        self.descriptor_home = Some(home);
        self
    }

    pub(crate) fn detached_descriptor(&self) -> Self {
        let mut descriptor = self.clone();
        descriptor.descriptor_home = None;
        descriptor
    }

    pub(crate) fn matches_descriptor_home(&self, value: &TerminalStructuralValue) -> bool {
        self.descriptor_home
            .as_ref()
            .map_or_else(|| value.path.is_empty(), |home| home == value)
    }

    /// A descriptor without an explicit home inherits only its keyed frame
    /// binding. An empty selected path cannot identify some other whole view.
    fn matches_bound_descriptor_home(
        &self,
        binding: &TerminalStructuralValue,
        selected: &TerminalStructuralValue,
    ) -> bool {
        let same_home = |home: &TerminalStructuralValue| {
            home.opaque_identity == selected.opaque_identity
                && home.path == selected.path
                && home.structural_type == selected.structural_type
        };
        same_home(binding) && self.descriptor_home.as_ref().is_none_or(same_home)
    }

    pub(crate) fn len(&self) -> usize {
        self.window.end - self.window.start
    }

    pub(crate) fn access(&self) -> StructuralAccess {
        self.access
    }

    /// Custody of the descriptor is distinct from access to its backing.
    /// Shared descriptors may be copied; exclusive descriptors must move.
    pub(crate) fn owned_multiplicity(&self) -> Option<StructuralMultiplicity> {
        match self.access {
            StructuralAccess::SharedBorrow => Some(StructuralMultiplicity::Unrestricted),
            StructuralAccess::MutableBorrow => Some(StructuralMultiplicity::Affine),
            _ => None,
        }
    }

    /// Authenticate a captured element against this live view's original
    /// backing and window. The descriptor's opaque identity is not the backing
    /// identity, including when a view arrives from a caller's frame.
    pub(crate) fn lends_structural_storage(&self, selected: &TerminalStructuralValue) -> bool {
        let ElementViewBacking::Structural { collection } = &self.backing else {
            return false;
        };
        let Some(path) = selected.path.strip_prefix(collection.path.as_slice()) else {
            return false;
        };
        let Some(StructuralPathSegment::FixedIndex(element_index)) = path.first() else {
            return false;
        };
        selected.opaque_identity == collection.opaque_identity
            && self.access == StructuralAccess::MutableBorrow
            && usize::try_from(*element_index)
                .is_ok_and(|element_index| self.window.contains(&element_index))
    }

    /// Invert only a captured structural coordinate inside this exact window.
    /// The returned position is a candidate for typed projection, not authority;
    /// the caller must resolve it and compare the complete returned backing.
    pub(crate) fn captured_structural_index(
        &self,
        selected: &TerminalStructuralValue,
    ) -> Option<u64> {
        let ElementViewBacking::Structural { collection } = &self.backing else {
            return None;
        };
        if selected.opaque_identity != collection.opaque_identity {
            return None;
        }
        let suffix = selected.path.strip_prefix(collection.path.as_slice())?;
        let StructuralPathSegment::FixedIndex(index) = suffix.first()? else {
            return None;
        };
        let index = usize::try_from(*index).ok()?;
        self.window
            .contains(&index)
            .then(|| u64::try_from(index - self.window.start).ok())
            .flatten()
    }

    /// Select original scalar storage below a collection loan. No snapshot or
    /// byte geometry participates; the owner's accessor checks the frame home.
    pub(crate) fn scalar_storage(
        &self,
        execution: &TerminalExecution,
        path: &[StructuralPathSegment],
        writing: bool,
    ) -> Result<
        (crate::primitive_storage::PrimitiveStorage, ScalarType, bool),
        TerminalInterpretError,
    > {
        let requested = if writing {
            StructuralAccess::WriteOnlyBorrow
        } else {
            StructuralAccess::SharedBorrow
        };
        if !requested.admits_referent(self.access) {
            return Err(malformed());
        }
        let Some((StructuralPathSegment::FixedIndex(element_index), suffix)) = path.split_first()
        else {
            return Err(malformed());
        };
        let element_index = usize::try_from(*element_index).map_err(|_| malformed())?;
        if element_index >= self.len() {
            return Err(malformed());
        }
        let home = self.home.as_ref().ok_or_else(malformed)?;
        let path = home.selected_path(
            self.window
                .start
                .checked_add(element_index)
                .ok_or_else(malformed)?,
            suffix,
        )?;
        execution.primitive_home_access(home.storage, writing, &path)
    }

    /// Preserve the original backing while narrowing its window. Attenuation
    /// changes descriptor authority, never the backing identity or parent access.
    pub(crate) fn subslice(
        &self,
        start: usize,
        end: usize,
        access: StructuralAccess,
    ) -> Option<Self> {
        if start > end || end > self.len() || !access.admits_referent(self.access) {
            return None;
        }
        Some(Self {
            backing: self.backing.clone(),
            window: self.window.start.checked_add(start)?..self.window.start.checked_add(end)?,
            home: self.home.clone(),
            descriptor_home: None,
            access,
        })
    }
}

fn count(
    operand: &ValueId,
    values: &BTreeMap<ValueId, TerminalScalarValue>,
) -> Result<usize, TerminalInterpretError> {
    let count_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
    let TerminalScalarValue::Integer {
        scalar_type,
        value: IntegerValue::Unsigned(value),
    } = values
        .get(operand)
        .ok_or(TerminalInterpretError::VerifiedValueMissing(*operand))?
    else {
        return Err(malformed());
    };
    if *scalar_type != count_type {
        return Err(malformed());
    }
    let value = u64::try_from(*value).map_err(|_| malformed())?;
    usize::try_from(value).map_err(|_| malformed())
}

impl TerminalExecution<'_> {
    /// Resolve the descriptor itself at its current structural home. Projecting
    /// a stored view never reconstructs its window from the owner's backing.
    pub(crate) fn element_view_argument(
        &self,
        argument: &terminal_psi::StructuralArgument,
        resolved: &TerminalStructuralValue,
    ) -> Result<&ElementView, TerminalInterpretError> {
        if argument.path.is_empty()
            && let Some(view) = self.element_view_values.get(&argument.place)
        {
            return if self
                .structural_values
                .get(&argument.place)
                .is_some_and(|home| view.matches_bound_descriptor_home(home, resolved))
            {
                Ok(view)
            } else {
                Err(malformed())
            };
        }
        self.element_view_at_resolved_home(resolved)
    }

    /// Keyed runtime homes, including suspended activations, identify the
    /// descriptor. Its backing allocation alone cannot identify its window.
    pub(crate) fn element_view_at_resolved_home(
        &self,
        resolved: &TerminalStructuralValue,
    ) -> Result<&ElementView, TerminalInterpretError> {
        self.structural_element_view_fields
            .get(&crate::values::StructuralRuntimePlace::from(resolved))
            .or_else(|| {
                self.element_view_values.iter().find_map(|(place, view)| {
                    self.structural_values
                        .get(place)
                        .filter(|home| view.matches_bound_descriptor_home(home, resolved))
                        .map(|_| view)
                })
            })
            .or_else(|| {
                self.call_stack.iter().find_map(|frame| {
                    frame.element_view_values.iter().find_map(|(place, view)| {
                        frame
                            .structural_values
                            .get(place)
                            .filter(|home| view.matches_bound_descriptor_home(home, resolved))
                            .map(|_| view)
                    })
                })
            })
            .ok_or_else(malformed)
    }

    /// Descriptor lifetime ends with its frame. Only debt-free views retire
    /// implicitly; an affine descriptor or live claim still needs the exact
    /// cleanup action checked by the return, regardless of its carrier.
    pub(crate) fn retire_unrestricted_views(&mut self) {
        let retired = self
            .byte_sequence_values
            .keys()
            .chain(self.element_view_values.keys())
            .copied()
            .filter(|place| {
                !self.live_affine_discard_at(*place).is_some()
                    && !self
                        .live_claims
                        .values()
                        .any(|claim| claim.place == Some(*place))
            })
            .collect::<Vec<_>>();
        for place in retired {
            // These are frame bindings, including aliases of stored fields.
            // Ending a callee's borrow cannot dispose the owner's descriptor.
            self.structural_values.remove(&place);
            self.byte_sequence_values.remove(&place);
            self.element_view_values.remove(&place);
        }
    }

    /// Both view operations borrow the same collection storage. Only the
    /// authority and result carrier differ; both retain the original home.
    pub(crate) fn execute_establish_element_view(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let (destination, source, element, region, access, multiplicity) = match &operation.kind {
            OperationKind::EstablishElementView {
                destination,
                source,
                element,
                region,
            } => (
                *destination,
                source,
                *element,
                *region,
                StructuralAccess::SharedBorrow,
                StructuralMultiplicity::Unrestricted,
            ),
            OperationKind::EstablishMutableElementView {
                destination,
                source,
                element,
                region,
            } => (
                *destination,
                source,
                *element,
                *region,
                StructuralAccess::MutableBorrow,
                StructuralMultiplicity::Affine,
            ),
            _ => return Err(malformed()),
        };
        let Some(result) = operation.result.as_structural() else {
            return Err(malformed());
        };
        if destination != result.place
            || source.access != access
            || result.multiplicity != multiplicity
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return Err(malformed());
        }
        let machine = self.program.verified_machine(self.current_machine)?;
        if !machine.structural_places.iter().any(|place| {
            place.id == result.place
                && place.kind.as_operation_result() == Some((operation.id, result.structural_type))
        }) {
            return Err(malformed());
        }
        let element_scalar = match &self
            .program
            .structural_types()
            .get(&element)
            .ok_or_else(malformed)?
            .shape
        {
            StructuralTypeShape::PrimitiveScalar(scalar) => Some(*scalar),
            _ => None,
        };
        let descriptor = &self
            .program
            .structural_types()
            .get(&result.structural_type)
            .ok_or_else(malformed)?
            .shape;
        match descriptor {
            StructuralTypeShape::ElementView {
                element: view_element,
                ..
            } if *view_element == element => {}
            StructuralTypeShape::ByteSequence(_)
                if descriptor.is_borrowed_byte_carrier()
                    && element_scalar
                        == Some(ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?,
                        )) => {}
            _ => return Err(malformed()),
        }
        if self.structural_values.contains_key(&result.place)
            || self.scalar_array_values.contains_key(&result.place)
            || self.byte_sequence_values.contains_key(&result.place)
            || self.element_view_values.contains_key(&result.place)
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(result.place))
            || self.live_affine_discard_at(result.place).is_some()
        {
            return Err(malformed());
        }
        // A fixed byte range selects a descriptor window over its collection;
        // it is not a child type or storage-home path. Resolve the backing first
        // and apply the selected extent through the same narrowing operation
        // used by a subsequent subslice.
        let formation_path = &source.path;
        let (backing_path, selected_window) = match source.path.split_last() {
            Some((StructuralPathSegment::FixedByteRange { start, end }, backing)) => {
                if region != ElementViewRegion::WholeCollection || start > end {
                    return Err(malformed());
                }
                (
                    backing,
                    Some(
                        usize::try_from(*start).map_err(|_| malformed())?
                            ..usize::try_from(*end).map_err(|_| malformed())?,
                    ),
                )
            }
            _ => (source.path.as_slice(), None),
        };
        let backing_source = terminal_psi::StructuralArgument {
            place: source.place,
            path: backing_path.to_vec(),
            access: source.access,
        };
        let source = &backing_source;
        let root_type = self.live_structural_type(source.place)?;
        // Execution has replaced runtime selectors with captured fixed indices.
        // Those coordinates select live storage, but a dynamic view's type
        // cannot prove a fixed bound. Retain the declared path for type geometry
        // and use the selected path only when resolving the actual descriptor.
        let declared_source = match &machine.operation(operation.id).ok_or_else(malformed)?.kind {
            OperationKind::EstablishElementView { source, .. }
            | OperationKind::EstablishMutableElementView { source, .. } => source,
            _ => return Err(malformed()),
        };
        if declared_source.place != source.place {
            return Err(malformed());
        }
        let declared_backing_path = match declared_source.path.split_last() {
            Some((StructuralPathSegment::FixedByteRange { .. }, prefix)) => prefix,
            _ => declared_source.path.as_slice(),
        };
        let collection_shape = crate::custody::resolve_structural_path_shape(
            &self.program.structural_types(),
            root_type,
            declared_backing_path,
        )?;
        let collection = terminal_semantics::element_view_shape_region_geometry(
            self.program.structural_types().values(),
            &collection_shape,
            element,
            ElementViewRegion::WholeCollection,
            source.access,
        )
        .and_then(|geometry| match geometry {
            terminal_semantics::ElementViewRegionGeometry::WholeCollection(backing) => {
                Some(backing)
            }
            terminal_semantics::ElementViewRegionGeometry::RepresentationBytes { .. } => None,
        });
        let extent = collection.map(|backing| backing.extent);
        terminal_semantics::collection_view_descriptor(
            descriptor,
            extent,
            region,
            formation_path,
            self.program.structural_types().get(&element),
        )
        .ok_or_else(malformed)?;
        if let Some(window) = &selected_window {
            let StructuralTypeShape::FixedArray {
                element: source_element,
                length,
            } = *collection_shape
            else {
                return Err(malformed());
            };
            if source_element != element
                || element_scalar
                    != Some(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?,
                    ))
                || u64::try_from(window.end).map_err(|_| malformed())? > length
            {
                return Err(malformed());
            }
        }
        if let ElementViewRegion::RepresentationBytes { start, end, .. } = region {
            let start = count(&start, &self.values)?;
            let end = count(&end, &self.values)?;
            let mut descriptor_value = TerminalStructuralValue {
                opaque_identity: self.local_structural_identities.allocate()?,
                structural_type: result.structural_type,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                path: Vec::new(),
            };
            if descriptor.is_borrowed_byte_view() {
                // This remains an exact byte collection, not an arbitrary
                // record whose layout happens to occupy the same byte count.
                // The result-family check above independently requires u8.
                let collection = collection.ok_or_else(malformed)?;
                let bound = match collection.extent {
                    terminal_semantics::CollectionExtent::Fixed(length) => length,
                    // An owned carrier's live count is bounded by its
                    // capacity: a byte window may not reach past the
                    // declared storage even before the live-length read.
                    terminal_semantics::CollectionExtent::LiveLength { capacity } => capacity,
                    // Dynamic descriptor backing is not fixed representation
                    // bytes and cannot supply this region's static bound.
                    terminal_semantics::CollectionExtent::StoredLength => return Err(malformed()),
                };
                if start > end || u64::try_from(end).map_err(|_| malformed())? > bound {
                    return Err(malformed());
                }
                let mut cursor = self.local_structural_identities.cursor();
                let (source_value, binding) = self.establish_byte_array_binding(
                    source,
                    result.structural_type,
                    &collection,
                    &mut cursor,
                )?;
                let (value, binding) = self.narrow_byte_sequence_binding(
                    &binding,
                    &source_value,
                    start..end,
                    access,
                    descriptor_value,
                )?;
                descriptor_value = value;
                self.local_structural_identities.commit_cursor(cursor);
                self.byte_sequence_values.insert(result.place, binding);
            } else {
                if !matches!(descriptor, StructuralTypeShape::ElementView { .. }) {
                    return Err(malformed());
                }
                let collection_type = crate::custody::resolve_structural_path_type(
                    &self.program.structural_types(),
                    root_type,
                    &source.path,
                )?;
                let view =
                    self.representation_view(source, collection_type, element, start, end)?;
                let extent = view.length;
                self.element_view_values.insert(
                    result.place,
                    ElementView {
                        backing: ElementViewBacking::Representation(Arc::new(view)),
                        window: 0..extent,
                        home: None,
                        descriptor_home: None,
                        access,
                    },
                );
            }
            self.structural_values
                .insert(result.place, descriptor_value);
            if multiplicity == StructuralMultiplicity::Affine {
                self.live_affine_frontier
                    .insert(terminal_psi::StructuralAffineDiscard {
                        place: result.place,
                        path: Vec::new(),
                        structural_type: result.structural_type,
                    });
            }
            return Ok(OperationFlow::Advance);
        }
        if let StructuralTypeShape::FixedArray {
            element: source_element,
            length,
        } = *collection_shape
            && source_element != element
        {
            if !matches!(descriptor, StructuralTypeShape::ElementView { .. }) {
                return Err(malformed());
            }
            let typed = self.typed_element_view(source, source_element, element)?;
            let extent = usize::try_from(length).map_err(|_| malformed())?;
            self.element_view_values.insert(
                result.place,
                ElementView {
                    backing: ElementViewBacking::Typed(Arc::new(typed)),
                    window: 0..extent,
                    home: None,
                    descriptor_home: None,
                    access,
                },
            );
            self.structural_values.insert(
                result.place,
                TerminalStructuralValue {
                    opaque_identity: self.local_structural_identities.allocate()?,
                    structural_type: result.structural_type,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    path: Vec::new(),
                },
            );
            if multiplicity == StructuralMultiplicity::Affine {
                self.live_affine_frontier
                    .insert(terminal_psi::StructuralAffineDiscard {
                        place: result.place,
                        path: Vec::new(),
                        structural_type: result.structural_type,
                    });
            }
            return Ok(OperationFlow::Advance);
        }
        let collection = collection.ok_or_else(malformed)?;
        if collection.storage == terminal_semantics::CollectionStorage::Borrowed
            && !descriptor.is_borrowed_byte_carrier()
            && collection.extent != terminal_semantics::CollectionExtent::StoredLength
        {
            return Err(malformed());
        }
        // A whole-view reborrow captures the existing descriptor, including a
        // stored field's window. Its carrier home is not the collection backing.
        let borrowed_view =
            if collection.extent == terminal_semantics::CollectionExtent::StoredLength {
                let resolved =
                    self.resolve_live_projection(source.place, &source.path, source.access)?;
                let view = self.element_view_argument(source, &resolved)?;
                Some(view.subslice(0, view.len(), access).ok_or_else(malformed)?)
            } else {
                None
            };
        let collection_length = match collection.extent {
            terminal_semantics::CollectionExtent::Fixed(length) => length,
            terminal_semantics::CollectionExtent::StoredLength => {
                u64::try_from(borrowed_view.as_ref().ok_or_else(malformed)?.len())
                    .map_err(|_| malformed())?
            }
            // An owned byte carrier's live count is dynamic; its byte-view
            // establishment enters the byte-array channel, which reads that
            // store's own live extent. Element snapshots of one cannot read
            // scalar cells the carrier does not have.
            terminal_semantics::CollectionExtent::LiveLength { capacity }
                if matches!(descriptor, StructuralTypeShape::ByteSequence(_)) =>
            {
                capacity
            }
            _ => return Err(malformed()),
        };
        let extent = usize::try_from(collection_length).map_err(|_| malformed())?;
        let mut descriptor_value = TerminalStructuralValue {
            // Static result places recur across ordinary-call activations.
            // The descriptor, unlike its declaration, owns a fresh runtime
            // home through the execution-wide storage identity allocator.
            opaque_identity: self.local_structural_identities.allocate()?,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            path: Vec::new(),
        };
        match descriptor {
            StructuralTypeShape::ByteSequence(_) => {
                let mut cursor = self.local_structural_identities.cursor();
                let (value, binding) = self.establish_byte_array_binding(
                    source,
                    result.structural_type,
                    &collection,
                    &mut cursor,
                )?;
                let (value, binding) = if let Some(window) = &selected_window {
                    self.narrow_byte_sequence_binding(
                        &binding,
                        &value,
                        window.clone(),
                        access,
                        descriptor_value.clone(),
                    )?
                } else {
                    (value, binding)
                };
                let binding = if access == StructuralAccess::MutableBorrow {
                    let mut binding = binding;
                    if matches!(
                        descriptor,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                        )
                    ) {
                        // The owned pointer carrier and borrowed array have
                        // distinct identities, as an ordinary reference does.
                        // Its backing/window stays in the exact array binding.
                        let crate::byte_sequences::binding::ByteSequenceBinding::MutableArray {
                            descriptor_home,
                            ..
                        } = &mut binding
                        else {
                            return Err(malformed());
                        };
                        *descriptor_home = Some(descriptor_value.clone());
                    } else {
                        descriptor_value = value;
                    }
                    binding
                } else {
                    crate::byte_sequences::binding::ByteSequenceBinding::Immutable(
                        binding
                            .immutable()?
                            .clone()
                            .with_descriptor_home(descriptor_value.clone()),
                    )
                };
                self.local_structural_identities.commit_cursor(cursor);
                self.byte_sequence_values.insert(result.place, binding);
            }
            _ => {
                // A borrowed fixed array already carries its original activation
                // home. Forming a descriptor from that binding must retain the
                // loan, rather than pinning the callee's temporary formal.
                let mut cursor = self.local_structural_identities.cursor();
                let view = if let Some(view) = borrowed_view {
                    view.with_descriptor_home(descriptor_value.clone())
                } else if source.path.is_empty()
                    && let Some(loan) = self.element_view_values.get(&source.place)
                {
                    let referent = self
                        .structural_values
                        .get(&source.place)
                        .ok_or_else(malformed)?;
                    if referent.structural_type != root_type
                        || !loan.matches_descriptor_home(referent)
                        || loan.len() != extent
                    {
                        return Err(malformed());
                    }
                    loan.subslice(0, extent, access).ok_or_else(malformed)?
                } else {
                    // Inline scalar arrays have frame-owned payload, not a
                    // structural scalar row. Their presentation root is only
                    // an identity; the retained home selects the actual cells.
                    let collection = if self.scalar_array_values.contains_key(&source.place) {
                        let (root, _) = self.array_storage_root_staged(
                            source.place,
                            &mut cursor,
                            &mut BTreeMap::new(),
                        )?;
                        crate::custody::project_structural_value(
                            &self.program.structural_types(),
                            &root,
                            &source.path,
                        )?
                    } else {
                        self.resolve_live_projection(source.place, &source.path, source.access)?
                    };
                    ElementView::structural(
                        collection,
                        extent,
                        access,
                        ElementViewHome {
                            storage: self.capture_primitive_home(source.place)?,
                            path: source.path.clone(),
                        },
                    )
                };
                let view = if let Some(window) = &selected_window {
                    view.subslice(window.start, window.end, access)
                        .ok_or_else(malformed)?
                } else {
                    view
                };
                self.local_structural_identities.commit_cursor(cursor);
                self.element_view_values.insert(result.place, view);
            }
        }
        self.structural_values
            .insert(result.place, descriptor_value);
        if multiplicity == StructuralMultiplicity::Affine {
            self.live_affine_frontier
                .insert(terminal_psi::StructuralAffineDiscard {
                    place: result.place,
                    path: Vec::new(),
                    structural_type: result.structural_type,
                });
        }
        Ok(OperationFlow::Advance)
    }

    /// Resolve a record cell in the original collection, for both direct and
    /// recast views. Structural primitive children keep using their frame home;
    /// only a declared scalar field has a record-cell key.
    fn element_scalar_field(
        &self,
        collection: Option<&TerminalStructuralValue>,
        position: usize,
        path: &[StructuralPathSegment],
        scalar_type: ScalarType,
        access: StructuralAccess,
    ) -> Result<Option<(StructuralScalarRuntimeField, ScalarType)>, TerminalInterpretError> {
        let Some((prefix, field_path)) = crate::primitive_storage::scalar_field_projection(path)
        else {
            return Ok(None);
        };
        let mut selected_path = Vec::with_capacity(1 + prefix.len());
        selected_path.push(StructuralPathSegment::FixedIndex(
            u64::try_from(position).map_err(|_| malformed())?,
        ));
        selected_path.extend_from_slice(prefix);
        // The element may itself contain windows or references. Its record
        // cell belongs to the final captured backing, not to an inline walk
        // through the outer collection's descriptor storage. Typed element
        // views supply their source-layout path and scalar carrier here.
        let (parent, view) = self
            .resolve_reference_view_path(collection.ok_or_else(malformed)?, &selected_path, access)?
            .ok_or_else(malformed)?;
        let Some(observed) = self.scalar_field_at_carrier(parent, view, field_path, scalar_type)?
        else {
            return Ok(None);
        };
        Ok(Some((observed.cell, observed.stored_type)))
    }

    fn read_structural_element(
        &self,
        view: &ElementView,
        index: usize,
        path: &[StructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        let ElementViewBacking::Structural { collection } = &view.backing else {
            return Err(malformed());
        };
        let position = view.window.start.checked_add(index).ok_or_else(malformed)?;
        if let Some((field, stored_type)) = self.element_scalar_field(
            Some(collection),
            position,
            path,
            scalar_type,
            StructuralAccess::SharedBorrow,
        )? {
            let value = self
                .structural_scalar_fields
                .get(&field)
                .copied()
                .ok_or_else(malformed)?;
            if value.scalar_type() != stored_type {
                return Err(malformed());
            }
            if stored_type == scalar_type {
                Ok(value)
            } else {
                crate::primitive_storage::scalar_reference_observation(value, scalar_type)
            }
        } else {
            let home = view.home.as_ref().ok_or_else(malformed)?;
            self.read_primitive_home(
                home.storage,
                &home.selected_path(position, path)?,
                scalar_type,
            )
        }
    }

    fn write_structural_element(
        &mut self,
        view: &ElementView,
        index: usize,
        path: &[StructuralPathSegment],
        scalar: TerminalScalarValue,
    ) -> Result<(), TerminalInterpretError> {
        let ElementViewBacking::Structural { collection } = &view.backing else {
            return Err(malformed());
        };
        let position = view.window.start.checked_add(index).ok_or_else(malformed)?;
        if let Some((field, stored_type)) = self.element_scalar_field(
            Some(collection),
            position,
            path,
            scalar.scalar_type(),
            StructuralAccess::MutableBorrow,
        )? {
            if stored_type != scalar.scalar_type() {
                return Err(malformed());
            }
            let stored = self
                .structural_scalar_fields
                .get_mut(&field)
                .ok_or_else(malformed)?;
            if stored.scalar_type() != scalar.scalar_type() {
                return Err(malformed());
            }
            *stored = scalar;
            Ok(())
        } else {
            let home = view.home.as_ref().ok_or_else(malformed)?;
            self.store_primitive_home(home.storage, &home.selected_path(position, path)?, scalar)
        }
    }

    /// Write one scalar element through the view's exclusive loan. The
    /// dominating length operand must equal the view's live window exactly
    /// and the index must be in bounds. Only original backing storage changes.
    pub(crate) fn execute_element_view_write(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::ElementViewWrite {
            destination,
            index,
            value,
            length,
            path,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        if operation.result != OperationResult::Unit {
            return Err(malformed());
        }
        let index = count(index, &self.values)?;
        let length = count(length, &self.values)?;
        let element = self.scalar_value(*value)?;
        if !terminal_scalar_belongs_to_type(element) {
            return Err(malformed());
        }
        self.validate_descriptor_borrow(destination, StructuralAccess::MutableBorrow)?;
        let selected = self.resolve_live_projection(
            destination.place,
            &destination.path,
            StructuralAccess::MutableBorrow,
        )?;
        let view = self.element_view_argument(destination, &selected)?.clone();
        if view.access != StructuralAccess::MutableBorrow || view.len() != length || index >= length
        {
            return Err(malformed());
        }
        match &view.backing {
            ElementViewBacking::Typed(_) => {
                self.write_typed_element(&view, index, path, element)?
            }
            ElementViewBacking::Structural { .. } => {
                self.write_structural_element(&view, index, path, element)?
            }
            ElementViewBacking::Representation(_) => {
                self.write_representation_element(&view, index, path, element)?
            }
        }
        Ok(OperationFlow::Advance)
    }

    /// One u64 count of the view's live window.
    pub(crate) fn execute_element_view_length(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::ElementViewLength { source } = &operation.kind else {
            return Err(malformed());
        };
        let result = operation.result.scalar().ok_or_else(malformed)?;
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
        if result.scalar_type != ScalarType::Integer(integer_type) {
            return Err(malformed());
        }
        if source.access == StructuralAccess::WriteOnlyBorrow {
            return Err(malformed());
        }
        self.validate_descriptor_borrow(source, StructuralAccess::SharedBorrow)?;
        // Read the selected descriptor through the shared live-path owner.
        // Reference crossings retain their captured backing; this observation
        // attenuates access without copying an exclusive descriptor.
        let selected = self.resolve_live_projection(
            source.place,
            &source.path,
            StructuralAccess::SharedBorrow,
        )?;
        if !self
            .program
            .structural_types()
            .get(&selected.structural_type)
            .is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
            })
        {
            return Err(malformed());
        }
        let length = self.element_view_argument(source, &selected)?.len();
        self.values.insert(
            result.id,
            TerminalScalarValue::Integer {
                scalar_type: integer_type,
                value: IntegerValue::Unsigned(u128::try_from(length).map_err(|_| malformed())?),
            },
        );
        Ok(OperationFlow::Advance)
    }

    /// Every scalar observation reads the original backing. A descendant may
    /// have updated it while this descriptor's authority was suspended.
    pub(crate) fn read_element_view_scalar(
        &self,
        view: &ElementView,
        index: usize,
        path: &[StructuralPathSegment],
        expected: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        if !crate::reference::access_covers(view.access(), StructuralAccess::SharedBorrow)
            || index >= view.len()
        {
            return Err(malformed());
        }
        let element = match &view.backing {
            ElementViewBacking::Typed(_) => self.read_typed_element(view, index, path, expected)?,
            ElementViewBacking::Representation(_) => {
                self.read_representation_element(view, index, path, expected)?
            }
            ElementViewBacking::Structural { .. } => {
                self.read_structural_element(view, index, path, expected)?
            }
        };
        if element.scalar_type() != expected || !terminal_scalar_belongs_to_type(element) {
            return Err(malformed());
        }
        Ok(element)
    }

    /// Copy one scalar element; the dominating length operand must equal the
    /// view's live window exactly and the index must be in bounds.
    pub(crate) fn execute_element_view_read(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::ElementViewRead {
            source,
            index,
            length,
            path,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        // Scalar element reads use this operation; structural element fields
        // resolve through the common structural projection owner.
        let result = operation.result.scalar().ok_or_else(malformed)?;
        let index = count(index, &self.values)?;
        let length = count(length, &self.values)?;
        let view = crate::custody::verified_structural_place(&self.element_view_values, *source)?;
        if view.len() != length || index >= length {
            return Err(malformed());
        }
        let element = self.read_element_view_scalar(view, index, path, result.scalar_type)?;
        self.values.insert(result.id, element);
        Ok(OperationFlow::Advance)
    }

    /// Derive a windowed view sharing the source's backing, mirroring the
    /// byte-sequence subslice posture: validate everything before any
    /// descriptor is rebound.
    pub(crate) fn execute_element_view_subslice(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let OperationKind::ElementViewSubslice {
            source,
            start,
            end,
            length,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        let result = operation.result.structural().ok_or_else(malformed)?;
        let start = count(start, &self.values)?;
        let end = count(end, &self.values)?;
        let length = count(length, &self.values)?;
        let access = match result.multiplicity {
            StructuralMultiplicity::Unrestricted => StructuralAccess::SharedBorrow,
            StructuralMultiplicity::Affine => StructuralAccess::MutableBorrow,
            StructuralMultiplicity::Linear => return Err(malformed()),
        };
        self.validate_descriptor_borrow(source, access)?;
        let source_value = self.resolve_live_projection(source.place, &source.path, access)?;
        let view = self.element_view_argument(source, &source_value)?.clone();
        let machine = self.program.verified_machine(self.current_machine)?;
        let types = self.program.structural_types();
        let same_element = types
            .get(&source_value.structural_type)
            .zip(types.get(&result.structural_type))
            .is_some_and(|(source, result)| {
                matches!((&source.shape, &result.shape),
                (StructuralTypeShape::ElementView { element: source, .. },
                 StructuralTypeShape::ElementView { element: result, access: ceiling })
                    if source == result && ceiling.is_none_or(|ceiling| ceiling == access))
            });
        if result.place == source.place
            || !machine.structural_places.iter().any(|place| {
                place.id == result.place
                    && place.kind
                        == StructuralPlaceKind::OperationResult {
                            producer: operation.id,
                            structural_type: result.structural_type,
                        }
            })
            || view.len() != length
            || !same_element
            || !crate::reference::access_covers(view.access, access)
            || !source_value.qualifications.is_empty()
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
            || !self
                .program
                .structural_types()
                .get(&result.structural_type)
                .is_some_and(|row| matches!(row.shape, StructuralTypeShape::ElementView { .. }))
            || !result.claims.is_empty()
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(source.place) || claim.place == Some(result.place))
            || self.live_affine_discard_at(result.place).is_some()
        {
            return Err(malformed());
        }
        match (
            self.structural_values.get(&result.place),
            self.element_view_values.get(&result.place),
        ) {
            (None, None) => {}
            (Some(previous), Some(previous_view))
                if *previous
                    == TerminalStructuralValue::minted(
                        previous.opaque_identity,
                        result.structural_type,
                        Vec::new(),
                    )
                    && previous_view.matches_bound_descriptor_home(previous, previous) => {}
            _ => return Err(malformed()),
        }
        let mut identity_cursor = self.local_structural_identities.cursor();
        let destination = TerminalStructuralValue::minted(
            self.local_structural_identities
                .allocate_staged(&mut identity_cursor)?,
            result.structural_type,
            Vec::new(),
        );
        let window = view
            .subslice(start, end, access)
            .ok_or_else(malformed)?
            .with_descriptor_home(destination.clone());
        self.local_structural_identities
            .commit_cursor(identity_cursor);
        self.element_view_values.insert(result.place, window);
        self.structural_values.insert(result.place, destination);
        if result.multiplicity == StructuralMultiplicity::Affine {
            self.live_affine_frontier
                .insert(terminal_psi::StructuralAffineDiscard {
                    place: result.place,
                    path: Vec::new(),
                    structural_type: result.structural_type,
                });
        }
        Ok(())
    }
}

mod representation;
mod typed;
