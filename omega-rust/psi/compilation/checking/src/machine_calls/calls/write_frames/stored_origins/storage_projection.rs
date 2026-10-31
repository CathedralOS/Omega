//! Exact owned geometry with its declaration application kept across loads.
//!
//! An owned projection never crosses a reference. A read planner may stop at
//! that boundary, authenticate the current descriptor elsewhere, then continue
//! through its referee without losing generic substitutions or a case selector.

use super::super::type_instantiation::DeclarationScopes;
use crate::frozen_program::expression_table;
use facts::PlaceSegment;
use symbols::SymbolKind;
use typed_trees::data::{DataMember, DataVariant};
use typed_trees::expression::ExpressionNode;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(in crate::machine_calls::calls::write_frames) struct StorageProjectionCursor<'program> {
    program: &'program dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    scopes: DeclarationScopes,
    scope: usize,
    selected_case: Option<&'program DataVariant>,
}

impl<'program> StorageProjectionCursor<'program> {
    pub(in crate::machine_calls::calls::write_frames) fn new(
        program: &'program dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Self {
        Self {
            program,
            reference,
            scopes: DeclarationScopes::default(),
            scope: 0,
            selected_case: None,
        }
    }

    fn normalize(&mut self) -> Option<TypeReferenceHandle> {
        (self.reference, self.scope) = self.scopes.head(self.program, self.reference, self.scope);
        self.reference = self
            .program
            .type_reference_table
            .unconstrained_type_reference(self.reference);
        self.reference.is_valid().then_some(self.reference)
    }

    pub(in crate::machine_calls::calls::write_frames) fn reference_access(
        &mut self,
    ) -> Option<language_semantics::ReferenceAccess> {
        let reference = self.normalize()?;
        match self.program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { access, .. } => Some(*access),
            _ => None,
        }
    }

    /// This advances geometry only; it supplies no load, lifetime or authority.
    pub(in crate::machine_calls::calls::write_frames) fn enter_referee(&mut self) -> Option<()> {
        if self.selected_case.is_some() {
            return None;
        }
        let reference = self.normalize()?;
        let TypeReferenceNode::Reference {
            referee, access, ..
        } = self.program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        self.reference = *referee;
        Some(())
    }

    pub(in crate::machine_calls::calls::write_frames) fn advance(
        &mut self,
        segment: &PlaceSegment,
    ) -> Option<()> {
        self.normalize()?;
        match (
            self.program
                .type_reference_table
                .type_reference(self.reference),
            segment,
        ) {
            (
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. },
                PlaceSegment::Case { variant },
            ) => {
                if self.selected_case.is_some()
                    || self.program.symbols.get(*variant).kind != SymbolKind::Variant
                {
                    return None;
                }
                let (definition, nested_scope) =
                    self.scopes
                        .definition(self.program, self.reference, self.scope)?;
                self.scope = nested_scope;
                self.selected_case =
                    self.program
                        .data_members(definition)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(candidate) if candidate.symbol == *variant => {
                                Some(candidate)
                            }
                            _ => None,
                        });
                self.selected_case?;
            }
            (
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. },
                PlaceSegment::Field { symbol: field },
            ) => {
                if self.program.symbols.get(*field).kind != SymbolKind::Field {
                    return None;
                }
                self.reference = if let Some(variant) = self.selected_case.take() {
                    self.program
                        .data_payload_field_by_symbol(variant, *field)?
                        .type_reference
                } else {
                    let (definition, nested_scope) =
                        self.scopes
                            .definition(self.program, self.reference, self.scope)?;
                    self.scope = nested_scope;
                    self.program.data_members(definition).iter().find_map(
                        |member| match member {
                            DataMember::Field(candidate) if candidate.symbol == *field => {
                                Some(candidate.type_reference)
                            }
                            _ => None,
                        },
                    )?
                };
            }
            (
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                },
                index,
            ) => {
                if self.selected_case.is_some() {
                    return None;
                }
                match index {
                    PlaceSegment::FixedIndex { index } if index < length => {}
                    PlaceSegment::Index { expression }
                        if *length > 0
                            && expression_table(self.program)
                                .constant_integer_value(*expression)
                                .is_none()
                            && !matches!(
                                expression_table(self.program).expression(*expression),
                                ExpressionNode::Range(_)
                            ) => {}
                    _ => return None,
                }
                self.reference = *element_type;
            }
            (
                TypeReferenceNode::Slice { element_type },
                PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. },
            ) => {
                if self.selected_case.is_some() {
                    return None;
                }
                // Storage shape supplies the element type, never evidence
                // that an element exists. Executable access still needs the
                // selected descriptor's checked bound and loan authority.
                self.reference = *element_type;
            }
            // Never peel a reference to turn a loaded carrier into an owned
            // value move. Reference result relations have their own evidence.
            _ => return None,
        }
        Some(())
    }

    pub(in crate::machine_calls::calls::write_frames) fn finish(
        mut self,
    ) -> Option<TypeReferenceHandle> {
        if self.selected_case.is_some() {
            return None;
        }
        self.normalize()
    }
}

pub(in crate::machine_calls::calls::write_frames) fn projected_storage_type(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    segments: &[PlaceSegment],
) -> Option<TypeReferenceHandle> {
    let mut cursor = StorageProjectionCursor::new(program, reference);
    for segment in segments {
        cursor.advance(segment)?;
    }
    cursor.finish()
}

pub(in crate::machine_calls::calls::write_frames) fn projected_type(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    segments: &[PlaceSegment],
) -> Option<TypeReferenceHandle> {
    let reference = projected_storage_type(program, reference, segments)?;
    (!super::super::type_reference_is_reference(program, reference)).then_some(reference)
}
