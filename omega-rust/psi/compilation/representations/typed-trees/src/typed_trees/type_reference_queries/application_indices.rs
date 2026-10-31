//! Discovery of value arguments in a retained declaration application.

use crate::data::TypeParameterKind;
use crate::typed_trees::TypedTrees;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};

/// Value-argument obligations present in a retained type application.
#[derive(Clone, Copy, Debug, Default)]
pub struct ApplicationValueIndices {
    const_values: bool,
    runtime_values: bool,
}

impl ApplicationValueIndices {
    pub fn is_empty(self) -> bool {
        !self.const_values && !self.runtime_values
    }

    pub fn has_runtime_values(self) -> bool {
        self.runtime_values
    }
}

impl TypedTrees {
    /// Whether compatibility of this application includes captured value
    /// operands. Data and selected code use the same declaration telescope;
    /// callers must not discard a code argument's own const/value bindings.
    pub fn type_application_value_indices(
        &self,
        reference: TypeReferenceHandle,
    ) -> ApplicationValueIndices {
        let mut indices = ApplicationValueIndices::default();
        let mut pending = vec![reference];
        let mut visited = Vec::new();
        while let Some(reference) = pending.pop() {
            if !self.type_reference_table.contains_type_reference(reference)
                || visited.contains(&reference)
            {
                continue;
            }
            visited.push(reference);
            match self.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => pending.push(*base_type),
                TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } => {
                    if base_symbol.is_valid() {
                        for parameter in self.declaration_type_parameters(*base_symbol) {
                            match parameter.kind {
                                TypeParameterKind::Const { .. } => indices.const_values = true,
                                TypeParameterKind::Value { .. } => indices.runtime_values = true,
                                _ => {}
                            }
                        }
                        if arguments.count() == 0
                            && let Some(origin) = self
                                .data_definition_by_symbol(*base_symbol)
                                .and_then(|declaration| declaration.generic_instance)
                        {
                            pending.push(origin);
                        }
                    }
                    pending.extend_from_slice(
                        self.type_reference_table.type_reference_handles(*arguments),
                    );
                }
                TypeReferenceNode::Named { symbol, .. } => {
                    if let Some(origin) = self
                        .data_definition_by_symbol(*symbol)
                        .and_then(|declaration| declaration.generic_instance)
                    {
                        pending.push(origin);
                    }
                }
                _ => {}
            }
        }
        indices
    }
}
