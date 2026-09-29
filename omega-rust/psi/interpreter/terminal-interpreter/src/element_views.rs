//! Frame-local immutable element views over contiguous structural
//! collections.
//!
//! An element view is a shared descriptor, not a copy: `backing` holds one
//! snapshot of the source collection's scalar elements and `window` selects
//! the visible element range. Subslices share the backing with a narrowed
//! window, matching `ByteSequenceView`; elements are language scalars, so no
//! target layout claim enters the interpreter. The verifier guarantees the
//! source collection is not written while a shared view of it is live, so a
//! captured snapshot cannot diverge from the borrower's contents.
//!
//! Element contents come from the storage the interpreter already owns:
//! whole-place owned scalar arrays and primitive leaf storage under
//! `FixedIndex` projection. A source without contents in either map (for
//! example a record field whose element leaves were never supplied at entry)
//! fails closed as malformed rather than fabricating elements.

use crate::errors::TerminalInterpretError;
use crate::execution::{OperationFlow, TerminalExecution};
use crate::scalar_operations::terminal_scalar_belongs_to_type;
use crate::values::StructuralRuntimePlace;
use crate::values::TerminalScalarValue;
use crate::values::TerminalStructuralValue;
use semantic_vocabulary::IntegerSign;
use semantic_vocabulary::IntegerType;
use semantic_vocabulary::IntegerValue;
use semantic_vocabulary::PlaceId;
use semantic_vocabulary::ScalarType;
use semantic_vocabulary::StructuralPlaceKind;
use semantic_vocabulary::ValueId;
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;
use terminal_psi::Operation;
use terminal_psi::OperationKind;
use terminal_psi::OperationResult;
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::StructuralPathSegment;
use terminal_psi::StructuralTypeShape;

/// The storage a mutable element view writes through to. A shared view has
/// no home: its captured snapshot is the only truth a read can observe.
pub(crate) enum ElementViewHome {
    /// Whole-place owned scalar array; elements live in `scalar_array_values`.
    ScalarArray(PlaceId),
    /// One primitive leaf per `FixedIndex` under a projected collection place,
    /// in `structural_primitive_storage`.
    PrimitiveStorage(StructuralRuntimePlace),
}

#[derive(Clone)]
pub(crate) struct ElementView {
    backing: Arc<[TerminalScalarValue]>,
    window: Range<usize>,
    home: Option<Arc<ElementViewHome>>,
}

impl ElementView {
    pub(crate) fn new(elements: Vec<TerminalScalarValue>) -> Self {
        let length = elements.len();
        Self {
            backing: elements.into(),
            window: 0..length,
            home: None,
        }
    }

    /// One mutable view: `backing` is still the read snapshot the window
    /// selects, and `home` is the storage a write-through updates beside it.
    pub(crate) fn new_mutable(elements: Vec<TerminalScalarValue>, home: ElementViewHome) -> Self {
        let length = elements.len();
        Self {
            backing: elements.into(),
            window: 0..length,
            home: Some(Arc::new(home)),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.window.end - self.window.start
    }

    pub(crate) fn get(&self, index: usize) -> Option<&TerminalScalarValue> {
        self.backing[self.window.clone()].get(index)
    }

    /// Share the backing under a narrowed window, mirroring
    /// `ByteSequenceView::subslice`. A derived window is immutable: the home
    /// does not transfer, since a narrowed write target was never verified.
    pub(crate) fn subslice(&self, start: usize, end: usize) -> Option<Self> {
        self.backing[self.window.clone()].get(start..end)?;
        Some(Self {
            backing: Arc::clone(&self.backing),
            window: self.window.start.checked_add(start)?..self.window.start.checked_add(end)?,
            home: None,
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
    /// Establish one shared element view over the source collection's stored
    /// elements. The result place binds the view payload; the source keeps
    /// its own storage and custody untouched.
    pub(crate) fn execute_establish_element_view(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::EstablishElementView {
            destination,
            source,
            element,
        } = &operation.kind
        else {
            return Err(invalid());
        };
        let OperationResult::Structural(result) = &operation.result else {
            return Err(invalid());
        };
        if *destination != result.place {
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
        let Some(&StructuralTypeShape::ElementView {
            element: view_element,
        }) = self
            .program
            .structural_types
            .get(&result.structural_type)
            .map(|declaration| &declaration.shape)
        else {
            return Err(invalid());
        };
        if view_element != *element
            || result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return Err(invalid());
        }
        let Some(&StructuralTypeShape::PrimitiveScalar(element_scalar)) = self
            .program
            .structural_types
            .get(element)
            .map(|declaration| &declaration.shape)
        else {
            return Err(invalid());
        };
        let resolved = crate::custody::resolve_structural_arguments(
            &self.program.structural_types,
            &self.structural_values,
            std::slice::from_ref(source),
        )?
        .pop()
        .ok_or_else(invalid)?;
        if source.access != StructuralAccess::SharedBorrow
            || !resolved.qualifications.is_empty()
            || self.structural_values.contains_key(&result.place)
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
        {
            return Err(invalid());
        }
        let StructuralTypeShape::FixedArray {
            element: collection_element,
            length: collection_length,
        } = self
            .program
            .structural_types
            .get(&resolved.structural_type)
            .ok_or_else(invalid)?
            .shape
        else {
            return Err(invalid());
        };
        if collection_element != *element {
            return Err(invalid());
        }
        // Whole-place owned arrays carry their elements directly; projected
        // fixed arrays gather one primitive leaf per declared index.
        let elements = if resolved.path.is_empty() {
            let array = self
                .scalar_array_values
                .get(&source.place)
                .ok_or_else(invalid)?;
            if array.structural_type != resolved.structural_type
                || usize::try_from(collection_length).ok() != Some(array.elements.len())
            {
                return Err(invalid());
            }
            if array
                .elements
                .iter()
                .any(|value| value.scalar_type() != element_scalar)
            {
                return Err(invalid());
            }
            array.elements.clone()
        } else {
            let extent = usize::try_from(collection_length).map_err(|_| invalid())?;
            let mut elements = Vec::with_capacity(extent);
            for index in 0..extent {
                let mut place = StructuralRuntimePlace::from(&resolved);
                place
                    .path
                    .push(StructuralPathSegment::FixedIndex(index as u64));
                let leaf = self
                    .structural_primitive_storage
                    .get(&place)
                    .copied()
                    .ok_or_else(invalid)?;
                if leaf.scalar_type() != element_scalar {
                    return Err(invalid());
                }
                elements.push(leaf);
            }
            elements
        };
        self.element_view_values
            .insert(result.place, ElementView::new(elements));
        self.structural_values.insert(
            result.place,
            TerminalStructuralValue {
                opaque_identity: result.place.get(),
                structural_type: result.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            },
        );
        Ok(OperationFlow::Advance)
    }

    /// Establish one mutable element view over the source collection's stored
    /// elements. The binding keeps the snapshot shape of its shared sibling
    /// for reads, and records the home a mutable loan's writes reseat into —
    /// the exclusive loan means the source cannot be observed through any
    /// other channel while the view is live.
    pub(crate) fn execute_establish_mutable_element_view(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::EstablishMutableElementView {
            destination,
            source,
            element,
        } = &operation.kind
        else {
            return Err(invalid());
        };
        let OperationResult::Structural(result) = &operation.result else {
            return Err(invalid());
        };
        if *destination != result.place {
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
        let Some(&StructuralTypeShape::ElementView {
            element: view_element,
        }) = self
            .program
            .structural_types
            .get(&result.structural_type)
            .map(|declaration| &declaration.shape)
        else {
            return Err(invalid());
        };
        if view_element != *element
            || result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return Err(invalid());
        }
        let Some(&StructuralTypeShape::PrimitiveScalar(element_scalar)) = self
            .program
            .structural_types
            .get(element)
            .map(|declaration| &declaration.shape)
        else {
            return Err(invalid());
        };
        let resolved = crate::custody::resolve_structural_arguments(
            &self.program.structural_types,
            &self.structural_values,
            std::slice::from_ref(source),
        )?
        .pop()
        .ok_or_else(invalid)?;
        if source.access != StructuralAccess::MutableBorrow
            || !resolved.qualifications.is_empty()
            || self.structural_values.contains_key(&result.place)
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
        {
            return Err(invalid());
        }
        let StructuralTypeShape::FixedArray {
            element: collection_element,
            length: collection_length,
        } = self
            .program
            .structural_types
            .get(&resolved.structural_type)
            .ok_or_else(invalid)?
            .shape
        else {
            return Err(invalid());
        };
        if collection_element != *element {
            return Err(invalid());
        }
        let (elements, home) = if resolved.path.is_empty() {
            let array = self
                .scalar_array_values
                .get(&source.place)
                .ok_or_else(invalid)?;
            if array.structural_type != resolved.structural_type
                || usize::try_from(collection_length).ok() != Some(array.elements.len())
            {
                return Err(invalid());
            }
            if array
                .elements
                .iter()
                .any(|value| value.scalar_type() != element_scalar)
            {
                return Err(invalid());
            }
            (
                array.elements.clone(),
                ElementViewHome::ScalarArray(source.place),
            )
        } else {
            let extent = usize::try_from(collection_length).map_err(|_| invalid())?;
            let mut elements = Vec::with_capacity(extent);
            for index in 0..extent {
                let mut place = StructuralRuntimePlace::from(&resolved);
                place
                    .path
                    .push(StructuralPathSegment::FixedIndex(index as u64));
                let leaf = self
                    .structural_primitive_storage
                    .get(&place)
                    .copied()
                    .ok_or_else(invalid)?;
                if leaf.scalar_type() != element_scalar {
                    return Err(invalid());
                }
                elements.push(leaf);
            }
            (
                elements,
                ElementViewHome::PrimitiveStorage(StructuralRuntimePlace::from(&resolved)),
            )
        };
        self.element_view_values
            .insert(result.place, ElementView::new_mutable(elements, home));
        self.structural_values.insert(
            result.place,
            TerminalStructuralValue {
                opaque_identity: result.place.get(),
                structural_type: result.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            },
        );
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
        let Some(view) = self.element_view_values.get_mut(destination) else {
            return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                *destination,
            ));
        };
        let Some(home) = view.home.clone() else {
            return Err(invalid());
        };
        if view.len() != length || index >= length {
            return Err(invalid());
        }
        let position = view.window.start.checked_add(index).ok_or_else(invalid)?;
        let Some(existing) = view.backing.get(position) else {
            return Err(invalid());
        };
        if existing.scalar_type() != element.scalar_type() {
            return Err(invalid());
        }
        match &*home {
            ElementViewHome::ScalarArray(place) => {
                let Some(array) = self.scalar_array_values.get_mut(place) else {
                    return Err(invalid());
                };
                let Some(slot) = array.elements.get_mut(position) else {
                    return Err(invalid());
                };
                *slot = element;
            }
            ElementViewHome::PrimitiveStorage(root) => {
                let mut place = root.clone();
                place
                    .path
                    .push(StructuralPathSegment::FixedIndex(position as u64));
                if self
                    .structural_primitive_storage
                    .get(&place)
                    .map(|leaf| leaf.scalar_type())
                    != Some(element.scalar_type())
                {
                    return Err(invalid());
                }
                self.structural_primitive_storage.insert(place, element);
            }
        }
        let Some(view) = self.element_view_values.get_mut(destination) else {
            return Err(invalid());
        };
        Arc::make_mut(&mut view.backing)[position] = element;
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
        // Backings hold scalar elements only: a record-element view is never
        // established here, so an element path has no element to project.
        if !path.is_empty() {
            return Err(invalid());
        }
        let result = operation.result.scalar().ok_or_else(invalid)?;
        let index = count(index, &self.values)?;
        let length = count(length, &self.values)?;
        let view = self.element_view_values.get(source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(*source),
        )?;
        if view.len() != length || index >= length {
            return Err(invalid());
        }
        let element = view.get(index).copied().ok_or_else(invalid)?;
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
        if !machine.structural_places.iter().any(|place| {
            place.id == result.place
                && place.kind
                    == StructuralPlaceKind::OperationResult {
                        producer: operation.id,
                        structural_type: result.structural_type,
                    }
        }) || view.len() != length
            || source_value.structural_type != result.structural_type
            || !source_value.path.is_empty()
            || !source_value.qualifications.is_empty()
            || result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(*source) || claim.place == Some(result.place))
            || self
                .live_affine_frontier
                .iter()
                .any(|entry| entry.place == *source || entry.place == result.place)
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
        let window = view.subslice(start, end).ok_or_else(invalid)?;
        self.element_view_values.insert(result.place, window);
        self.structural_values.insert(result.place, destination);
        Ok(())
    }
}
