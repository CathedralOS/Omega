//! Laying out type references, generic data definitions and named types
//! under generic bindings.

use crate::TypeLayout;
use crate::builder::generic_bindings::{
    GenericLayoutBinding, binding_for_type, fixed_array_length_with_bindings,
};
use crate::builder::{LayoutBuilder, OpaqueRepresentationDemand};
use crate::packing::placement_overflow;
use crate::sizing::{
    dynamic_trait_descriptor_layout, fat_descriptor_layout, primitive_type_layout,
};
use checked_trees::data::DataDefinition;
use checked_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use diagnostics::Diagnostic;
use representation_selections::selection_for_opaque;
use symbols::SymbolHandle;

impl<'program> LayoutBuilder<'program> {
    pub(crate) fn layout_type_reference_handle(
        &mut self,
        type_reference: TypeReferenceHandle,
    ) -> Result<TypeLayout, Diagnostic> {
        self.layout_type_reference_handle_with_bindings(type_reference, &[])
    }

    pub(crate) fn layout_type_reference_handle_with_bindings(
        &mut self,
        type_reference: TypeReferenceHandle,
        bindings: &[GenericLayoutBinding<'program>],
    ) -> Result<TypeLayout, Diagnostic> {
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Reference { referee, .. } => {
                // Borrowed views retain their descriptor: `{ptr, len}` for
                // slices/text and `{instance, table}` for dynamic traits.
                // Keeping only the instance pointer loses dispatch identity
                // and disagrees with calling-policy parameter placement.
                // References to sized carriers (including records containing
                // a descriptor) remain thin pointers.
                //
                // A domain-constrained slice view (`&[u8] in Utf8`) is
                // `Reference { Constrained { Slice } }`: the constraint is a
                // compile-time fact that does not change the storage shape, so
                // the referee must be unwrapped past any `Constrained` wrappers
                // before deciding sized-ness -- otherwise a `&[u8] in Utf8`
                // field would be sized as a thin 8-byte pointer while the
                // call-result/return path (which unwraps Constrained) sizes it
                // as the fat 16-byte descriptor, and the byte-count mismatch
                // silently drops the descriptor copy.
                let mut referee_node = self.program.type_reference_table.type_reference(*referee);
                while let TypeReferenceNode::Constrained { base_type, .. } = referee_node {
                    referee_node = self.program.type_reference_table.type_reference(*base_type);
                }
                match referee_node {
                    TypeReferenceNode::DynamicTrait { .. } => {
                        Ok(dynamic_trait_descriptor_layout(self.target))
                    }
                    TypeReferenceNode::Slice { .. } => Ok(fat_descriptor_layout(self.target)),
                    TypeReferenceNode::Named { name, .. } if name.as_str() == "string" => {
                        Ok(fat_descriptor_layout(self.target))
                    }
                    _ => Ok(TypeLayout {
                        size: self.target.pointer_size,
                        alignment: self.target.pointer_alignment,
                    }),
                }
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let base_layout =
                    self.layout_type_reference_handle_with_bindings(*base_type, bindings)?;
                // The owned bounded byte carrier `[u8; N] in <named-domain>` lays
                // out as `{ len, bytes }`: a pointer-sized length word followed by
                // the N inline bytes. Must agree with the BoundedByteBuffer arm of
                // instruction-selection's `descriptor_layout`. The OmegaLayout
                // FAMILY is excluded: `[u8; N] in OmegaLayout<Save>` records what
                // the bytes hold, never changes what they are -- the wire codec
                // addresses the plain array directly.
                let has_named_domain = self
                    .program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| match constraint {
                        TypeConstraintNode::Domain(name) => {
                            !checked_trees::wire::is_layout_domain_constraint(name)
                                && language_semantics::CarryPermission::from_name(name.as_str())
                                    .is_none()
                        }
                        _ => false,
                    });
                if has_named_domain
                    && matches!(
                        self.program.type_reference_table.type_reference(*base_type),
                        TypeReferenceNode::FixedArray { .. }
                    )
                {
                    // The length word plus the inline bytes must still fit the
                    // addressable size; a saturated extent would silently lay
                    // out a buffer smaller than its declared capacity.
                    let Some(size) = self.target.pointer_size.checked_add(base_layout.size) else {
                        return Err(placement_overflow(format!(
                            "bounded byte buffer of {} byte(s) behind a {}-byte length word",
                            base_layout.size, self.target.pointer_size
                        )));
                    };
                    return Ok(TypeLayout {
                        size,
                        alignment: self.target.pointer_alignment,
                    });
                }
                Ok(base_layout)
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let element_layout =
                    self.layout_type_reference_handle_with_bindings(*element_type, bindings)?;
                let Some(length) = fixed_array_length_with_bindings(self.program, length, bindings)
                else {
                    return Ok(TypeLayout::default());
                };

                // `count * element_size` can exceed the addressable size for an absurd
                // length (`[i32; 5e18]`); use checked arithmetic so a too-large array is a
                // clean diagnostic, never a compiler panic on `attempt to multiply with
                // overflow`.
                let Some(size) = element_layout.size.checked_mul(length) else {
                    return Err(Diagnostic::error(format!(
                        "fixed array `[_; {length}]` is too large: {length} elements of {} \
                         byte(s) each overflow the addressable size",
                        element_layout.size,
                    )));
                };

                Ok(TypeLayout {
                    size,
                    alignment: element_layout.alignment,
                })
            }
            TypeReferenceNode::Slice { .. } => Ok(self.slice_layout()),
            TypeReferenceNode::DynamicTrait { .. } => {
                Ok(dynamic_trait_descriptor_layout(self.target))
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments,
                ..
            } => {
                if let Some(binding) = binding_for_type(*base_symbol, base_name, bindings) {
                    return self
                        .layout_type_reference_handle_with_bindings(binding.argument, bindings);
                }

                if let Some(layout) = self.builtin_type_layout(*base_symbol) {
                    return Ok(layout);
                }

                if base_symbol.is_valid()
                    && let Ok(definition) = self.data_definition_by_symbol(*base_symbol)
                {
                    // A GENERIC definition (record OR enum -- `Option<T>` included)
                    // lays out as a recorded monomorphized instance; the shared
                    // compute path dispatches on the shape internally. Only a
                    // non-generic definition goes through the plain symbol route.
                    if !definition.type_parameters.is_empty() {
                        return self
                            .layout_generic_data_definition(definition, *arguments, bindings);
                    }

                    return self.layout_data_definition(*base_symbol);
                }

                Err(Diagnostic::error(format!(
                    "native layout for generic type `{base_name}` is not implemented yet"
                )))
            }
            TypeReferenceNode::Named { symbol, name } => {
                if let Some(binding) = binding_for_type(*symbol, name, bindings) {
                    return self
                        .layout_type_reference_handle_with_bindings(binding.argument, bindings);
                }

                self.layout_named_type(*symbol, name)
            }
            TypeReferenceNode::ConstExpression(expression) => Err(Diagnostic::error(format!(
                "proof-static index expression `{}` reached runtime layout as a standalone type",
                self.program.expression_table.display_name(*expression)
            ))),
            TypeReferenceNode::Unit => Ok(TypeLayout {
                size: 0,
                alignment: 1,
            }),
        }
    }

    /// Lays out a MONOMORPHIZED instance of a generic data definition and RECORDS
    /// it in the plan, so downstream field-offset resolution (which looks up
    /// `data_layouts` by the definition symbol through the type descriptor) works
    /// on generic instances exactly as on concrete data. Delegates the actual
    /// field/variant packing to the shared `compute_data_layout` with the
    /// parameter->argument bindings, so records AND case-bearing shapes
    /// (`Option<T>`-style enums) are covered by one computation.
    ///
    /// STAGE-1 BOUNDARY: the instance is keyed by the DEFINITION symbol, so each
    /// generic data may be instantiated with ONE argument list per program. A
    /// second, different instantiation is a clean error (per-instance identity
    /// through descriptors is the later stage).
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

        // Canonical signature of this instantiation (argument display list).
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
                // Memo hit: the recorded instance is this one.
                if let Some(data_layout) = self
                    .data_layouts
                    .iter()
                    .find(|(_, data_layout)| data_layout.symbol == definition.symbol)
                    .map(|(_, data_layout)| data_layout)
                {
                    return Ok(data_layout.layout);
                }
                // Same signature but nothing recorded: the entry was POISONED by
                // an earlier collision. Compute sizes fresh; record nothing.
            }
            Some(index) => {
                // COLLISION: a second, different instantiation. Field offsets
                // recorded under the definition symbol are now ambiguous, so
                // UN-RECORD the first instance (downstream field access then
                // falls back to the pre-existing clean "needs runtime storage
                // lowering" rejection instead of silently using the wrong
                // instance's offsets). Sizes stay correct for BOTH: each use
                // computes its own layout with its own bindings below.
                if let Some(handle) = self
                    .data_layouts
                    .iter()
                    .find(|(_, data_layout)| data_layout.symbol == definition.symbol)
                    .map(|(handle, _)| handle)
                {
                    self.data_layouts.get_mut(handle).symbol = SymbolHandle::invalid();
                }
                self.generic_instance_signatures[index].1 = String::new(); // poisoned
            }
            None => {
                // First instantiation: compute AND record it, keyed by the
                // definition symbol, so field-offset resolution works natively.
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

        // Poisoned (or collision just detected): size this use privately.
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
        // Atomic placed accessors retain their underlying primitive class for
        // operation typing, so their generated name parses as primitive here.
        // The exact typed placed-plan row wins for runtime representation:
        // this is an address carrier, not an inline atomic resident.
        let compiler_placed_accessor = self
            .data_definitions
            .iter()
            .find(|definition| definition.symbol == symbol)
            .is_some_and(|definition| self.is_compiler_placed_accessor_definition(definition));
        if compiler_placed_accessor {
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

        if let Some(definition) = self
            .data_definitions
            .iter()
            .find(|definition| definition.symbol == symbol)
        {
            if definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque {
                if let Some(selection) =
                    selection_for_opaque(self.opaque_representation_selections, symbol)
                {
                    return self.layout_data_definition(selection.carrier());
                }
                if self.opaque_representation_demand == OpaqueRepresentationDemand::ByValue {
                    return Err(Diagnostic::error(format!(
                        "by-value layout for boundary-opaque data `{name}` requires one exact representation selected by the authoritative build"
                    )));
                }
            }
            return self.layout_data_definition(symbol);
        }

        if self
            .machine_definitions
            .iter()
            .any(|machine| machine.symbol == symbol)
        {
            return self.layout_machine(symbol);
        }

        if self.trait_definitions.iter().any(|trait_definition| {
            trait_definition.symbol == symbol && trait_definition.is_boundary
        }) {
            return Ok(TypeLayout {
                size: 0,
                alignment: 1,
            });
        }

        Err(Diagnostic::error(format!(
            "unknown layout-bearing type `{name}` for symbol {}",
            symbol.arena_index()
        )))
    }
}
