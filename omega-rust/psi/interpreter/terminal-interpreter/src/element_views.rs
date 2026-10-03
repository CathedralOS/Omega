//! Frame-local views over contiguous structural collections.
//!
//! Scalar views retain their element snapshot; structural views retain the original
//! storage identity and collection path. Neither descriptor copies an owned
//! structural element. Byte representation views decode their original allocation;
//! typed representation views map target leaves to original typed cells using the
//! independently reconstructed validity relation and padded layout. A window
//! selects the same backing across local bindings,
//! calls and state transfer, without relying on a frame-local source place ID.
//! The verified loan controls interference; execution preserves its access and
//! exact bounds when resolving a field below the descriptor.

use crate::errors::TerminalInterpretError;
use crate::execution::{OperationFlow, TerminalExecution};
use crate::scalar_operations::terminal_scalar_belongs_to_type;
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
pub(crate) struct ElementViewHome {
    storage: crate::primitive_storage::PrimitiveStorageHome,
    path: Vec<StructuralPathSegment>,
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
    Scalars(Arc<[TerminalScalarValue]>),
    Structural { collection: TerminalStructuralValue },
    Typed(Arc<typed::TypedElementView>),
    Representation(Arc<representation::RepresentationView>),
}

impl ElementView {
    pub(crate) fn new(elements: Vec<TerminalScalarValue>) -> Self {
        let length = elements.len();
        Self {
            backing: ElementViewBacking::Scalars(elements.into()),
            window: 0..length,
            home: None,
            descriptor_home: None,
            access: StructuralAccess::SharedBorrow,
        }
    }

    /// One mutable view: `backing` is still the read snapshot the window
    /// selects, and `home` is the storage a write-through updates beside it.
    pub(crate) fn new_mutable(elements: Vec<TerminalScalarValue>, home: ElementViewHome) -> Self {
        let length = elements.len();
        Self {
            backing: ElementViewBacking::Scalars(elements.into()),
            window: 0..length,
            home: Some(Arc::new(home)),
            descriptor_home: None,
            access: StructuralAccess::MutableBorrow,
        }
    }

    fn structural(
        collection: TerminalStructuralValue,
        length: usize,
        access: StructuralAccess,
    ) -> Self {
        Self {
            backing: ElementViewBacking::Structural { collection },
            window: 0..length,
            home: None,
            descriptor_home: None,
            access,
        }
    }

    /// Resolve a selected element into its original collection. Selectors have
    /// already been evaluated by the operation-wide runtime-index owner.
    pub(crate) fn structural_projection(
        &self,
        path: &[StructuralPathSegment],
        writing: bool,
    ) -> Result<(TerminalStructuralValue, Vec<StructuralPathSegment>), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let ElementViewBacking::Structural { collection } = &self.backing else {
            return Err(invalid());
        };
        if writing && self.access != StructuralAccess::MutableBorrow {
            return Err(invalid());
        }
        let Some((StructuralPathSegment::FixedIndex(index), suffix)) = path.split_first() else {
            return Err(invalid());
        };
        let index = usize::try_from(*index).map_err(|_| invalid())?;
        if index >= self.len() {
            return Err(invalid());
        }
        let selected = self.window.start.checked_add(index).ok_or_else(invalid)?;
        let mut path = Vec::with_capacity(path.len());
        path.push(StructuralPathSegment::FixedIndex(
            u64::try_from(selected).map_err(|_| invalid())?,
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

    pub(crate) fn len(&self) -> usize {
        self.window.end - self.window.start
    }

    pub(crate) fn get(&self, index: usize) -> Option<&TerminalScalarValue> {
        let ElementViewBacking::Scalars(elements) = &self.backing else {
            return None;
        };
        elements.get(self.window.clone())?.get(index)
    }

    /// Preserve the original backing while narrowing its window. Attenuation
    /// changes descriptor authority, never the backing identity or parent access.
    pub(crate) fn subslice(
        &self,
        start: usize,
        end: usize,
        access: StructuralAccess,
    ) -> Option<Self> {
        if start > end
            || end > self.len()
            || !matches!(
                access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            )
            || !matches!(
                self.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            )
            || (access == StructuralAccess::MutableBorrow && self.access != access)
        {
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
    let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
    let count_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?;
    let TerminalScalarValue::Integer {
        scalar_type,
        value: IntegerValue::Unsigned(value),
    } = values
        .get(operand)
        .ok_or(TerminalInterpretError::VerifiedValueMissing(*operand))?
    else {
        return Err(invalid());
    };
    if *scalar_type != count_type {
        return Err(invalid());
    }
    let value = u64::try_from(*value).map_err(|_| invalid())?;
    usize::try_from(value).map_err(|_| invalid())
}

impl TerminalExecution {
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
            return if view.matches_descriptor_home(resolved) {
                Ok(view)
            } else {
                Err(TerminalInterpretError::VerifiedOperationMalformed)
            };
        }
        self.structural_element_view_fields
            .get(&crate::values::StructuralRuntimePlace::from(resolved))
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)
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
                !self
                    .live_affine_frontier
                    .iter()
                    .any(|entry| entry.place == *place)
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
    /// authority and result carrier differ: mutable descriptors retain their
    /// original storage home, while shared scalar descriptors retain snapshots.
    pub(crate) fn execute_establish_element_view(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
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
            _ => return Err(invalid()),
        };
        let OperationResult::Structural(result) = &operation.result else {
            return Err(invalid());
        };
        if destination != result.place
            || source.access != access
            || result.multiplicity != multiplicity
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return Err(invalid());
        }
        let machine = self.program.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        if !machine.structural_places.iter().any(|place| {
            place.id == result.place
                && place.kind
                    == StructuralPlaceKind::OperationResult {
                        producer: operation.id,
                        structural_type: result.structural_type,
                    }
        }) {
            return Err(invalid());
        }
        let element_scalar = match &self
            .program
            .structural_types
            .get(&element)
            .ok_or_else(invalid)?
            .shape
        {
            StructuralTypeShape::PrimitiveScalar(scalar) => Some(*scalar),
            _ => None,
        };
        let descriptor = &self
            .program
            .structural_types
            .get(&result.structural_type)
            .ok_or_else(invalid)?
            .shape;
        match descriptor {
            StructuralTypeShape::ElementView {
                element: view_element,
            } if *view_element == element => {}
            StructuralTypeShape::ByteSequence(_)
                if descriptor.is_borrowed_byte_view()
                    && element_scalar
                        == Some(ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| invalid())?,
                        )) => {}
            _ => return Err(invalid()),
        }
        if self.structural_values.contains_key(&result.place)
            || self.scalar_case_values.contains_key(&result.place)
            || self.scalar_array_values.contains_key(&result.place)
            || self.byte_sequence_values.contains_key(&result.place)
            || self.element_view_values.contains_key(&result.place)
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(result.place))
            || self
                .live_affine_frontier
                .iter()
                .any(|entry| entry.place == result.place)
            || (source.path.is_empty()
                && self
                    .structural_values
                    .get(&source.place)
                    .is_some_and(|value| !value.qualifications.is_empty()))
        {
            return Err(invalid());
        }
        // A fixed byte range selects a descriptor window over its collection;
        // it is not a child type or storage-home path. Resolve the backing first
        // and apply the selected extent through the same narrowing operation
        // used by a subsequent subslice.
        let (backing_path, selected_window) = match source.path.split_last() {
            Some((StructuralPathSegment::FixedByteRange { start, end }, backing)) => {
                if region != ElementViewRegion::WholeCollection || start > end {
                    return Err(invalid());
                }
                (
                    backing,
                    Some(
                        usize::try_from(*start).map_err(|_| invalid())?
                            ..usize::try_from(*end).map_err(|_| invalid())?,
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
        let collection_type = crate::custody::resolve_structural_path_type(
            &self.program.structural_types,
            root_type,
            &source.path,
        )?;
        if let Some(window) = &selected_window {
            let StructuralTypeShape::FixedArray {
                element: source_element,
                length,
            } = self.program.structural_types[&collection_type].shape
            else {
                return Err(invalid());
            };
            if source_element != element
                || element_scalar
                    != Some(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| invalid())?,
                    ))
                || u64::try_from(window.end).map_err(|_| invalid())? > length
            {
                return Err(invalid());
            }
        }
        if let ElementViewRegion::RepresentationBytes { start, end, .. } = region {
            let start = count(&start, &self.values)?;
            let end = count(&end, &self.values)?;
            let mut descriptor_value = TerminalStructuralValue {
                opaque_identity: result.place.get(),
                structural_type: result.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            };
            if descriptor.is_borrowed_byte_view() {
                // This remains an exact byte collection, not an arbitrary
                // record whose layout happens to occupy the same byte count.
                // The result-family check above independently requires u8.
                let collection = terminal_semantics::collection_backing(
                    self.program.structural_types.values(),
                    collection_type,
                    element,
                )
                .ok_or_else(invalid)?;
                let bound = match collection.extent {
                    terminal_semantics::CollectionExtent::Fixed(length) => length,
                    // An owned carrier's live count is bounded by its
                    // capacity: a byte window may not reach past the
                    // declared storage even before the live-length read.
                    terminal_semantics::CollectionExtent::LiveLength { capacity } => capacity,
                };
                if start > end || u64::try_from(end).map_err(|_| invalid())? > bound {
                    return Err(invalid());
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
                    return Err(invalid());
                }
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
        } = self.program.structural_types[&collection_type].shape
            && source_element != element
        {
            if !matches!(descriptor, StructuralTypeShape::ElementView { .. }) {
                return Err(invalid());
            }
            let typed = self.typed_element_view(source, source_element, element)?;
            let extent = usize::try_from(length).map_err(|_| invalid())?;
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
                    opaque_identity: result.place.get(),
                    structural_type: result.structural_type,
                    qualifications: Vec::new(),
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
        let collection = terminal_semantics::collection_backing(
            self.program.structural_types.values(),
            collection_type,
            element,
        )
        .ok_or_else(invalid)?;
        if collection.storage == terminal_semantics::CollectionStorage::Borrowed
            && !descriptor.is_borrowed_byte_view()
        {
            return Err(invalid());
        }
        let collection_length = match collection.extent {
            terminal_semantics::CollectionExtent::Fixed(length) => length,
            // An owned byte carrier's live count is dynamic; its byte-view
            // establishment enters the byte-array channel, which reads that
            // store's own live extent. Element snapshots of one cannot read
            // scalar cells the carrier does not have.
            terminal_semantics::CollectionExtent::LiveLength { capacity }
                if matches!(descriptor, StructuralTypeShape::ByteSequence(_)) =>
            {
                capacity
            }
            _ => return Err(invalid()),
        };
        let extent = usize::try_from(collection_length).map_err(|_| invalid())?;
        // Byte loans resolve their live ArrayStorage directly. Only consumers
        // retaining scalar snapshots need to walk and allocate element values.
        let snapshot_scalar = element_scalar
            .filter(|_| matches!(descriptor, StructuralTypeShape::ElementView { .. }));
        let mut elements = Vec::with_capacity(if snapshot_scalar.is_some() { extent } else { 0 });
        let mut path = source.path.clone();
        if let Some(element_scalar) = snapshot_scalar {
            for position in 0..collection_length {
                path.push(StructuralPathSegment::FixedIndex(position));
                elements.push(self.read_primitive_place(source.place, &path, element_scalar)?);
                path.pop();
            }
        }
        let mut descriptor_value = TerminalStructuralValue {
            opaque_identity: result.place.get(),
            structural_type: result.structural_type,
            qualifications: Vec::new(),
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
                    descriptor_value = value;
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
                let view = if element_scalar.is_none() {
                    let collection = crate::custody::resolve_structural_arguments(
                        &self.program.structural_types,
                        &self.structural_values,
                        std::slice::from_ref(source),
                    )?
                    .pop()
                    .ok_or_else(invalid)?;
                    ElementView::structural(collection, extent, access)
                } else if access == StructuralAccess::MutableBorrow {
                    ElementView::new_mutable(
                        elements,
                        ElementViewHome {
                            storage: self.capture_primitive_home(source.place)?,
                            path: source.path.clone(),
                        },
                    )
                } else {
                    ElementView::new(elements)
                };
                let view = if let Some(window) = &selected_window {
                    view.subslice(window.start, window.end, access)
                        .ok_or_else(invalid)?
                } else {
                    view
                };
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

    /// Write one scalar element through the view's exclusive loan. The
    /// dominating length operand must equal the view's live window exactly
    /// and the index must be in bounds; the store reseats both the view's
    /// snapshot and the home storage the loan covers.
    pub(crate) fn execute_element_view_write(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::ElementViewWrite {
            destination,
            index,
            value,
            length,
            path,
            ..
        } = &operation.kind
        else {
            return Err(invalid());
        };
        if operation.result != OperationResult::Unit {
            return Err(invalid());
        }
        let index = count(index, &self.values)?;
        let length = count(length, &self.values)?;
        let element = self
            .values
            .get(value)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(*value))?;
        if !terminal_scalar_belongs_to_type(element) {
            return Err(invalid());
        }
        if let Some(view) = self.element_view_values.get(destination)
            && matches!(
                view.backing,
                ElementViewBacking::Representation(_) | ElementViewBacking::Typed(_)
            )
        {
            let view = view.clone();
            if view.access != StructuralAccess::MutableBorrow
                || view.len() != length
                || index >= length
            {
                return Err(invalid());
            }
            match &view.backing {
                ElementViewBacking::Typed(_) => {
                    self.write_typed_element(&view, index, path, element)?
                }
                _ => self.write_representation_element(&view, index, path, element)?,
            }
            return Ok(OperationFlow::Advance);
        }
        if !path.is_empty() {
            return Err(invalid());
        }
        let Some(view) = self.element_view_values.get_mut(destination) else {
            return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                *destination,
            ));
        };
        let Some(home) = view.home.clone() else {
            return Err(invalid());
        };
        if view.access != StructuralAccess::MutableBorrow || view.len() != length || index >= length
        {
            return Err(invalid());
        }
        let position = view.window.start.checked_add(index).ok_or_else(invalid)?;
        let Some(existing) = view.get(index) else {
            return Err(invalid());
        };
        if existing.scalar_type() != element.scalar_type() {
            return Err(invalid());
        }
        let mut path = home.path.clone();
        path.push(StructuralPathSegment::FixedIndex(
            u64::try_from(position).map_err(|_| invalid())?,
        ));
        self.store_primitive_home(home.storage, &path, element)?;
        let Some(view) = self.element_view_values.get_mut(destination) else {
            return Err(invalid());
        };
        let ElementViewBacking::Scalars(elements) = &mut view.backing else {
            return Err(invalid());
        };
        Arc::make_mut(elements)[position] = element;
        Ok(OperationFlow::Advance)
    }

    /// One u64 count of the view's live window.
    pub(crate) fn execute_element_view_length(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::ElementViewLength { source } = &operation.kind else {
            return Err(invalid());
        };
        let result = operation.result.scalar().ok_or_else(invalid)?;
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?;
        if result.scalar_type != ScalarType::Integer(integer_type) {
            return Err(invalid());
        }
        let view = self.element_view_values.get(source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(*source),
        )?;
        self.values.insert(
            result.id,
            TerminalScalarValue::Integer {
                scalar_type: integer_type,
                value: IntegerValue::Unsigned(u128::try_from(view.len()).map_err(|_| invalid())?),
            },
        );
        Ok(OperationFlow::Advance)
    }

    /// Copy one scalar element; the dominating length operand must equal the
    /// view's live window exactly and the index must be in bounds.
    pub(crate) fn execute_element_view_read(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::ElementViewRead {
            source,
            index,
            length,
            path,
            ..
        } = &operation.kind
        else {
            return Err(invalid());
        };
        // Scalar element reads use this operation; structural element fields
        // resolve through the common structural projection owner.
        let result = operation.result.scalar().ok_or_else(invalid)?;
        let index = count(index, &self.values)?;
        let length = count(length, &self.values)?;
        let view = self.element_view_values.get(source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(*source),
        )?;
        if view.len() != length || index >= length {
            return Err(invalid());
        }
        // A mutable descendant may have updated the original storage while
        // this descriptor was suspended. Snapshots cannot authorize stale reads
        // after restoration; shared descendants retain read-only access to home.
        let element = if matches!(view.backing, ElementViewBacking::Typed(_)) {
            self.read_typed_element(view, index, path, result.scalar_type)?
        } else if matches!(view.backing, ElementViewBacking::Representation(_)) {
            self.read_representation_element(view, index, path, result.scalar_type)?
        } else if !path.is_empty() {
            return Err(invalid());
        } else if let Some(home) = &view.home {
            let mut path = home.path.clone();
            path.push(StructuralPathSegment::FixedIndex(
                u64::try_from(view.window.start.checked_add(index).ok_or_else(invalid)?)
                    .map_err(|_| invalid())?,
            ));
            self.read_primitive_home(home.storage, &path, result.scalar_type)?
        } else {
            view.get(index).copied().ok_or_else(invalid)?
        };
        if element.scalar_type() != result.scalar_type || !terminal_scalar_belongs_to_type(element)
        {
            return Err(invalid());
        }
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
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::ElementViewSubslice {
            source,
            start,
            end,
            length,
            ..
        } = &operation.kind
        else {
            return Err(invalid());
        };
        let result = operation.result.structural().ok_or_else(invalid)?;
        let start = count(start, &self.values)?;
        let end = count(end, &self.values)?;
        let length = count(length, &self.values)?;
        let source_value = self.structural_values.get(source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(*source),
        )?;
        let view = self.element_view_values.get(source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(*source),
        )?;
        let machine = self.program.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        let access = match result.multiplicity {
            StructuralMultiplicity::Unrestricted => StructuralAccess::SharedBorrow,
            StructuralMultiplicity::Affine => StructuralAccess::MutableBorrow,
            StructuralMultiplicity::Linear => return Err(invalid()),
        };
        // A call or edge may attenuate a descriptor while retaining the same
        // physical backing. The receiving formal remains authoritative.
        let mut parameters = machine
            .structural_parameters
            .iter()
            .chain(
                machine
                    .blocks
                    .values()
                    .flat_map(|block| &block.structural_parameters),
            )
            .filter(|parameter| parameter.place == *source);
        if let Some(parameter) = parameters.next() {
            if parameters.next().is_some()
                || parameter.structural_type != source_value.structural_type
                || !matches!(
                    parameter.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                )
                || (access == StructuralAccess::MutableBorrow && parameter.access != access)
            {
                return Err(invalid());
            }
        } else if view.access == StructuralAccess::MutableBorrow
            && !self
                .live_affine_frontier
                .contains(&terminal_psi::StructuralAffineDiscard {
                    place: *source,
                    path: Vec::new(),
                    structural_type: source_value.structural_type,
                })
        {
            return Err(invalid());
        }
        if result.place == *source
            || !machine.structural_places.iter().any(|place| {
                place.id == result.place
                    && place.kind
                        == StructuralPlaceKind::OperationResult {
                            producer: operation.id,
                            structural_type: result.structural_type,
                        }
            })
            || view.len() != length
            || source_value.structural_type != result.structural_type
            || !view.matches_descriptor_home(source_value)
            || !source_value.qualifications.is_empty()
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
            || !self
                .program
                .structural_types
                .get(&result.structural_type)
                .is_some_and(|row| matches!(row.shape, StructuralTypeShape::ElementView { .. }))
            || !result.claims.is_empty()
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(*source) || claim.place == Some(result.place))
            || self
                .live_affine_frontier
                .iter()
                .any(|entry| entry.place == result.place)
            || self.scalar_case_values.contains_key(source)
            || self.scalar_case_values.contains_key(&result.place)
        {
            return Err(invalid());
        }
        let destination = TerminalStructuralValue {
            opaque_identity: result.place.get(),
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        match (
            self.structural_values.get(&result.place),
            self.element_view_values.get(&result.place),
        ) {
            (None, None) => {}
            (Some(previous), Some(_)) if *previous == destination => {}
            _ => return Err(invalid()),
        }
        let window = view
            .subslice(start, end, access)
            .ok_or_else(invalid)?
            .with_descriptor_home(destination.clone());
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

#[cfg(test)]
mod tests;
