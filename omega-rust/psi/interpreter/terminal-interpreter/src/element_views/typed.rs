//! Typed representation views map observations back to the original live cells.
//!
//! The shared semantic relation pairs normalized leaves, including declaration
//! bounds and padding. Execution never materializes a target record or changes
//! the stored source carrier. Descriptor copies retain the original allocation
//! and activation home; only the selected scalar crosses the bit mapping.

use super::{ElementView, ElementViewBacking, ElementViewHome};
use crate::custody::resolve_structural_arguments;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::{TerminalScalarValue, TerminalStructuralValue};
use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralArgument, StructuralPathSegment};
use terminal_semantics::{
    ElementRepresentationLeaf, ElementRepresentationRelation, ScalarViewValue,
};

pub(super) struct TypedElementView {
    collection: Option<TerminalStructuralValue>,
    home: ElementViewHome,
    pub(super) relation: ElementRepresentationRelation,
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
            return Err(malformed());
        }
        let position = view.window.start.checked_add(index).ok_or_else(malformed)?;
        let mut matching = self
            .relation
            .source_layout
            .leaves
            .iter()
            .zip(&self.relation.target_layout.leaves)
            .filter(|(_, target)| target.path == path && target.scalar_type == scalar);
        let (Some((source, target)), None) = (matching.next(), matching.next()) else {
            return Err(malformed());
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
            .ok_or_else(malformed)?;
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
        _ => Err(malformed()),
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
        .ok_or_else(malformed)?;
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
                .ok_or_else(malformed)?,
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

    pub(super) fn read_typed_element(
        &self,
        view: &ElementView,
        index: usize,
        path: &[StructuralPathSegment],
        expected: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        let ElementViewBacking::Typed(typed) = &view.backing else {
            return Err(malformed());
        };
        let (position, source, target) = typed.selected(view, index, path, expected)?;
        let value = if let Some(field) = self.element_scalar_field(
            typed.collection.as_ref(),
            position,
            &source.path,
            source.scalar_type,
        )? {
            self.structural_scalar_fields
                .get(&field)
                .copied()
                .ok_or_else(malformed)?
        } else {
            let source_path = typed.home.selected_path(position, &source.path)?;
            self.read_primitive_home(typed.home.storage, &source_path, source.scalar_type)?
        };
        if !checked_value(value, source) {
            return Err(malformed());
        }
        let result = reinterpret(value, target.scalar_type)?;
        if !checked_value(result, target) {
            return Err(malformed());
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
            return Err(malformed());
        }
        let ElementViewBacking::Typed(typed) = &view.backing else {
            return Err(malformed());
        };
        let (position, source, target) = typed.selected(view, index, path, value.scalar_type())?;
        if !checked_value(value, target) {
            return Err(malformed());
        }
        let value = reinterpret(value, source.scalar_type)?;
        if !checked_value(value, source) {
            return Err(malformed());
        }
        if let Some(field) = self.element_scalar_field(
            typed.collection.as_ref(),
            position,
            &source.path,
            source.scalar_type,
        )? {
            let stored = self
                .structural_scalar_fields
                .get_mut(&field)
                .ok_or_else(malformed)?;
            if !checked_value(*stored, source) {
                return Err(malformed());
            }
            *stored = value;
            Ok(())
        } else {
            let source_path = typed.home.selected_path(position, &source.path)?;
            self.store_primitive_home(typed.home.storage, &source_path, value)
        }
    }
}
