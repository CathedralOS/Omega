/*
We lay out type references (`layout_type_reference_handle`), monomorphized
generic data definitions (`layout_generic_data_definition`), and named types
(`layout_named_type`) under generic bindings.

References to slices (including domain-constrained slices such as `&[u8] in Utf8`)
size as fat `{ptr, len}` descriptors by unwrapping `Constrained` wrappers before
inspecting the referee, while references to all other carriers and `dyn Trait`
selectors size as single pointer words. Monomorphized generic data definitions
are recorded under their definition symbol on first instantiation; if a second,
distinct instantiation collides on that symbol, we un-record the symbol entry
and poison the signature so each use still sizes accurately while downstream
field-offset resolution cleanly rejects ambiguous generic field access.
*/

use crate::data_layout_queries::TypeLayout;
use crate::data_layout_queries::builder::generic_bindings::{
    GenericLayoutBinding, binding_for_type, fixed_array_length,
};
use crate::data_layout_queries::builder::{LayoutBuilder, OpaqueRepresentationDemand};
use crate::data_layout_queries::sizing::{
    fat_descriptor_layout, pointer_layout, primitive_type_layout,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::data::DataDefinition;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

impl<'program> LayoutBuilder<'program> {
    pub(crate) fn layout_type_reference_handle(
        &mut self,
        type_reference: TypeReferenceHandle,
        bindings: &[GenericLayoutBinding<'program>],
    ) -> Result<TypeLayout, Diagnostic> {
        if self
            .program
            .layout_storage_arguments(type_reference)
            .is_some()
        {
            let plan = self
                .program
                .plan_laid_layout_for_type(type_reference)
                .ok_or_else(|| {
                    Diagnostic::error("fixed representation has no installed validated geometry")
                })?;
            let geometry = plan.invariant_geometry().ok_or_else(|| {
                Diagnostic::error(
                    "fixed representation geometry has not closed under the exact target",
                )
            })?;
            return Ok(TypeLayout {
                size: geometry.size,
                alignment: geometry.align,
            });
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Reference { referee, .. } => {
                let mut referee_node = self.program.type_reference_table.type_reference(*referee);
                while let TypeReferenceNode::Constrained { base_type, .. } = referee_node {
                    referee_node = self.program.type_reference_table.type_reference(*base_type);
                }
                match referee_node {
                    TypeReferenceNode::Slice { .. } => Ok(fat_descriptor_layout(self.target)),
                    _ => Ok(pointer_layout(self.target)),
                }
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                self.layout_type_reference_handle(*base_type, bindings)
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let element_layout = self.layout_type_reference_handle(*element_type, bindings)?;
                let Some(length) =
                    fixed_array_length(self.program, type_reference, length, bindings)
                else {
                    self.unresolved_array_extent = true;
                    return Ok(TypeLayout::default());
                };

                let Some(size) = element_layout.size.checked_mul(length) else {
                    return Err(Diagnostic::error(format!(
                        "fixed array `[_; {length}]` is too large: {length} elements of {} \
                         byte(s) each overflow the addressable size",
                        element_layout.size,
                    )));
                };

                Ok(crate::data_layout_queries::type_layout(
                    size,
                    element_layout.alignment,
                ))
            }
            TypeReferenceNode::Slice { .. } => Ok(fat_descriptor_layout(self.target)),
            TypeReferenceNode::DynamicTrait { .. } => Ok(pointer_layout(self.target)),
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments,
                ..
            } => {
                if let Some(binding) = binding_for_type(*base_symbol, base_name, bindings) {
                    return self.layout_type_reference_handle(binding.argument, bindings);
                }

                if let Some(layout) = self.builtin_type_layout(*base_symbol) {
                    return Ok(layout);
                }

                if base_symbol.is_valid()
                    && let Ok(definition) = self.data_definition_by_symbol(*base_symbol)
                {
                    if !definition.type_parameters.is_empty() {
                        return self
                            .layout_generic_data_definition(definition, *arguments, bindings);
                    }

                    if !arguments.is_empty() {
                        return Err(Diagnostic::error(format!(
                            "`{base_name}` is not a generic data type"
                        )));
                    }
                    return self.layout_data_definition(*base_symbol);
                }

                Err(Diagnostic::error(format!(
                    "native layout for generic type `{base_name}` is not implemented yet"
                )))
            }
            TypeReferenceNode::Named { symbol, name } => {
                if let Some(binding) = binding_for_type(*symbol, name, bindings) {
                    return self.layout_type_reference_handle(binding.argument, bindings);
                }

                self.layout_named_type(*symbol, name)
            }
            TypeReferenceNode::ConstExpression(expression) => Err(Diagnostic::error(format!(
                "proof-static index expression `{}` reached runtime layout as a standalone type",
                self.program.expression_table.display_name(*expression)
            ))),
            TypeReferenceNode::Unit => Ok(crate::data_layout_queries::type_layout(0, 1)),
        }
    }

    fn layout_generic_data_definition(
        &mut self,
        definition: &'program DataDefinition,
        arguments: arena::HandleSpan<TypeReferenceHandle>,
        parent_bindings: &[GenericLayoutBinding<'program>],
    ) -> Result<TypeLayout, Diagnostic> {
        let parameters = self.program.data_type_parameters(definition);
        let arguments = self
            .program
            .type_reference_table
            .type_reference_handles(arguments);
        if parameters.len() != arguments.len() {
            return Err(Diagnostic::error(format!(
                "generic data `{}` expected {} type arguments but got {}",
                definition.name,
                parameters.len(),
                arguments.len()
            )));
        }

        let signature = arguments
            .iter()
            .map(|argument| {
                self.program
                    .display_type_reference_with_constraints(*argument)
            })
            .collect::<Vec<_>>()
            .join(", ");

        if self.data_visiting.contains(definition.symbol) {
            return Err(Diagnostic::error(format!(
                "recursive generic data type `{}` has no finite size: it contains itself \
                 (directly or through a cycle) as an inline field, which would nest \
                 endlessly. Break the cycle with an indirection (a reference) instead of \
                 an inline value.",
                definition.name
            )));
        }

        let mut bindings = Vec::with_capacity(parent_bindings.len() + parameters.len());
        bindings.extend_from_slice(parent_bindings);
        bindings.extend(
            parameters
                .iter()
                .zip(arguments.iter())
                .map(|(parameter, argument)| GenericLayoutBinding {
                    parameter_symbol: parameter.symbol,
                    parameter_name: parameter.name.as_str(),
                    argument: *argument,
                }),
        );

        let existing = self
            .generic_instance_signatures
            .iter()
            .position(|(symbol, _)| *symbol == definition.symbol);
        match existing {
            Some(index) if self.generic_instance_signatures[index].1 == signature => {
                if let Some((_, data_layout)) = self.data_layout_entry_by_symbol(definition.symbol)
                {
                    return Ok(data_layout.layout);
                }
            }
            Some(index) => {
                if let Some(handle) = self
                    .data_layout_entry_by_symbol(definition.symbol)
                    .map(|(handle, _)| handle)
                {
                    self.data_layouts.get_mut(handle).symbol = SymbolHandle::invalid();
                }
                self.generic_instance_signatures[index].1 = String::new();
            }
            None => {
                self.generic_instance_signatures
                    .push((definition.symbol, signature));
                self.data_visiting.push(definition.symbol);
                let data_layout = self.compute_data_layout(definition, &bindings)?;
                let layout = data_layout.layout;
                self.data_layouts.insert(data_layout);
                self.data_visiting.pop();
                return Ok(layout);
            }
        }

        self.data_visiting.push(definition.symbol);
        let data_layout = self.compute_data_layout(definition, &bindings)?;
        self.data_visiting.pop();
        Ok(data_layout.layout)
    }

    fn layout_named_type(
        &mut self,
        symbol: SymbolHandle,
        name: &str,
    ) -> Result<TypeLayout, Diagnostic> {
        let compiler_accessor = self
            .data_definition_by_symbol(symbol)
            .ok()
            .is_some_and(|definition| self.is_compiler_accessor_definition(definition));
        if compiler_accessor {
            return self.layout_data_definition(symbol);
        }

        if let Some(primitive_type) = PrimitiveType::from_name(name) {
            return Ok(primitive_type_layout(self.target, primitive_type));
        }

        if let Some(layout) = self.builtin_type_layout(symbol) {
            return Ok(layout);
        }

        if !symbol.is_valid() {
            return Err(Diagnostic::error(format!(
                "non-primitive type `{name}` is missing a resolved symbol"
            )));
        }

        if let Ok(definition) = self.data_definition_by_symbol(symbol) {
            if definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque {
                if let Some(selection) = self
                    .opaque_representation_selections
                    .iter()
                    .find(|selection| selection.opaque == symbol)
                {
                    return self.layout_type_reference_handle(selection.carrier_type, &[]);
                }
                if self.opaque_representation_demand == OpaqueRepresentationDemand::ByValue {
                    return Err(Diagnostic::error(format!(
                        "by-value layout for boundary-opaque data `{name}` requires one exact representation selected by the authoritative build"
                    )));
                }
            }
            return self.layout_data_definition(symbol);
        }

        if self.trait_definitions.iter().any(|trait_definition| {
            trait_definition.symbol == symbol && trait_definition.is_boundary
        }) {
            return Ok(crate::data_layout_queries::type_layout(0, 1));
        }

        Err(Diagnostic::error(format!(
            "unknown layout-bearing type `{name}` for symbol {}",
            symbol.arena_index()
        )))
    }
}
