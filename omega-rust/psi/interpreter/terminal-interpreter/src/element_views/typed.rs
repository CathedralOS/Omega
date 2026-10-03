//! Typed representation views map observations back to the original live cells.
//!
//! The shared semantic relation pairs normalized leaves, including declaration
//! bounds and padding. Execution never materializes a target record or changes
//! the stored source carrier. Descriptor copies retain the original allocation
//! and activation home; only the selected scalar crosses the bit mapping.

use super::{ElementView, ElementViewBacking, ElementViewHome};
use crate::custody::{project_structural_value, resolve_structural_arguments};
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::values::{
    StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue,
    TerminalStructuralValue,
};
use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralPathSegment, StructuralTypeShape,
};
use terminal_semantics::{
    ElementRepresentationLeaf, ElementRepresentationRelation, ScalarViewValue,
};

pub(super) struct TypedElementView {
    collection: Option<TerminalStructuralValue>,
    home: ElementViewHome,
    pub(super) relation: ElementRepresentationRelation,
}

fn invalid() -> TerminalInterpretError {
    TerminalInterpretError::VerifiedOperationMalformed
}

impl TypedElementView {
    fn selected<'a>(
        &'a self,
        view: &ElementView,
        index: usize,
        path: &[StructuralPathSegment],
        scalar: ScalarType,
    ) -> Result<
        (
            usize,
            &'a ElementRepresentationLeaf,
            &'a ElementRepresentationLeaf,
        ),
        TerminalInterpretError,
    > {
        if index >= view.len() {
            return Err(invalid());
        }
        let position = view.window.start.checked_add(index).ok_or_else(invalid)?;
        let mut matching = self
            .relation
            .source_layout
            .leaves
            .iter()
            .zip(&self.relation.target_layout.leaves)
            .filter(|(_, target)| target.path == path && target.scalar_type == scalar);
        let (Some((source, target)), None) = (matching.next(), matching.next()) else {
            return Err(invalid());
        };
        Ok((position, source, target))
    }
}

fn checked_value(value: TerminalScalarValue, leaf: &ElementRepresentationLeaf) -> bool {
    value.scalar_type() == leaf.scalar_type && crate::scalar_operations::terminal_scalar_belongs_to_type(value)
        && leaf.bounds.is_none_or(|bounds| matches!(value, TerminalScalarValue::Integer { value, .. } if bounds.contains(value)))
}

fn reinterpret(
    value: TerminalScalarValue,
    target: ScalarType,
) -> Result<TerminalScalarValue, TerminalInterpretError> {
    let input = match value {
        TerminalScalarValue::Boolean(value) => ScalarViewValue::Boolean(value),
        TerminalScalarValue::Integer { value, .. } => ScalarViewValue::Integer(value),
        TerminalScalarValue::IeeeFloat(value) => ScalarViewValue::IeeeFloat(value),
    };
    let output =
        terminal_semantics::scalar_reference_view_value(value.scalar_type(), target, input)
            .or_else(|| {
                terminal_semantics::ScalarRepresentationViewPair::classify(
                    value.scalar_type(),
                    target,
                )?
                .view(input)
            })
            .ok_or_else(invalid)?;
    match (target, output) {
        (ScalarType::Boolean, ScalarViewValue::Boolean(value)) => {
            Ok(TerminalScalarValue::Boolean(value))
        }
        (ScalarType::Integer(scalar_type), ScalarViewValue::Integer(value)) => {
            Ok(TerminalScalarValue::Integer { scalar_type, value })
        }
        (ScalarType::IeeeFloat(_), ScalarViewValue::IeeeFloat(value)) => {
            Ok(TerminalScalarValue::IeeeFloat(value))
        }
        _ => Err(invalid()),
    }
}

impl TerminalExecution {
    pub(super) fn typed_element_view(
        &self,
        source: &StructuralArgument,
        source_element: StructuralTypeId,
        target_element: StructuralTypeId,
    ) -> Result<TypedElementView, TerminalInterpretError> {
        let relation = terminal_semantics::element_representation_relation(
            self.program.structural_types.values(),
            source_element,
            target_element,
            source.access,
        )
        .ok_or_else(invalid)?;
        let collection = if self.scalar_array_values.contains_key(&source.place) {
            // Inline scalar-array payloads use the pinned frame home. They have
            // no opaque structural identity and cannot contain record fields.
            None
        } else {
            Some(
                resolve_structural_arguments(
                    &self.program.structural_types,
                    &self.structural_values,
                    std::slice::from_ref(source),
                )?
                .pop()
                .ok_or_else(invalid)?,
            )
        };
        Ok(TypedElementView {
            collection,
            home: ElementViewHome {
                storage: self.capture_primitive_home(source.place)?,
                path: source.path.clone(),
            },
            relation,
        })
    }

    /// Ordinary field operations and typed views address the same record cell.
    /// Resolve the original containing declaration before selecting its field ID;
    /// target names and target IDs never enter the backing storage key.
    fn typed_element_field(
        &self,
        view: &TypedElementView,
        index: usize,
        leaf: &ElementRepresentationLeaf,
    ) -> Result<Option<StructuralScalarRuntimeField>, TerminalInterpretError> {
        let Some((StructuralPathSegment::Field(identity), prefix)) = leaf.path.split_last() else {
            return Ok(None);
        };
        let mut path = vec![StructuralPathSegment::FixedIndex(
            u64::try_from(index).map_err(|_| invalid())?,
        )];
        path.extend_from_slice(prefix);
        let parent = project_structural_value(
            &self.program.structural_types,
            view.collection.as_ref().ok_or_else(invalid)?,
            &path,
        )?;
        let StructuralTypeShape::Record { fields } = &self
            .program
            .structural_types
            .get(&parent.structural_type)
            .ok_or_else(invalid)?
            .shape
        else {
            return Err(invalid());
        };
        let mut matching = fields
            .iter()
            .filter(|field| field.identity == *identity && !field.relevance.is_erased());
        let (Some(field), None) = (matching.next(), matching.next()) else {
            return Err(invalid());
        };
        if field.field_type.scalar_type() != Some(leaf.scalar_type) {
            return Err(invalid());
        }
        Ok(Some(StructuralScalarRuntimeField::member(
            StructuralRuntimePlace::from(&parent),
            field.id,
        )))
    }

    pub(super) fn read_typed_element(
        &self,
        view: &ElementView,
        index: usize,
        path: &[StructuralPathSegment],
        expected: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        let ElementViewBacking::Typed(typed) = &view.backing else {
            return Err(invalid());
        };
        let (position, source, target) = typed.selected(view, index, path, expected)?;
        let value = if let Some(field) = self.typed_element_field(typed, position, source)? {
            self.structural_scalar_fields
                .get(&field)
                .copied()
                .ok_or_else(invalid)?
        } else {
            let mut source_path = typed.home.path.clone();
            source_path.push(StructuralPathSegment::FixedIndex(
                u64::try_from(position).map_err(|_| invalid())?,
            ));
            source_path.extend_from_slice(&source.path);
            self.read_primitive_home(typed.home.storage, &source_path, source.scalar_type)?
        };
        if !checked_value(value, source) {
            return Err(invalid());
        }
        let result = reinterpret(value, target.scalar_type)?;
        if !checked_value(result, target) {
            return Err(invalid());
        }
        Ok(result)
    }

    pub(super) fn write_typed_element(
        &mut self,
        view: &ElementView,
        index: usize,
        path: &[StructuralPathSegment],
        value: TerminalScalarValue,
    ) -> Result<(), TerminalInterpretError> {
        if view.access != StructuralAccess::MutableBorrow {
            return Err(invalid());
        }
        let ElementViewBacking::Typed(typed) = &view.backing else {
            return Err(invalid());
        };
        let (position, source, target) = typed.selected(view, index, path, value.scalar_type())?;
        if !checked_value(value, target) {
            return Err(invalid());
        }
        let value = reinterpret(value, source.scalar_type)?;
        if !checked_value(value, source) {
            return Err(invalid());
        }
        if let Some(field) = self.typed_element_field(typed, position, source)? {
            let stored = self
                .structural_scalar_fields
                .get_mut(&field)
                .ok_or_else(invalid)?;
            if !checked_value(*stored, source) {
                return Err(invalid());
            }
            *stored = value;
            Ok(())
        } else {
            let mut source_path = typed.home.path.clone();
            source_path.push(StructuralPathSegment::FixedIndex(
                u64::try_from(position).map_err(|_| invalid())?,
            ));
            source_path.extend_from_slice(&source.path);
            self.store_primitive_home(typed.home.storage, &source_path, value)
        }
    }
}
