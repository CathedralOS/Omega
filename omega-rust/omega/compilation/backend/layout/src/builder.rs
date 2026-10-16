//! The layout plan builder.
//!
//! This file owns the plan entry points, the builder and its visit stack.
//! `data_layouts.rs` computes record and sum layouts,
//! `type_reference_layouts.rs` lays out type references under generic
//! bindings, `generic_bindings.rs` resolves those bindings,
//! `private_callback_closure.rs` closes private callback demands and
//! `semantic_ranges.rs` derives plan-laid semantic ranges.

mod data_layouts;
mod generic_bindings;
mod private_callback_closure;
mod semantic_ranges;
#[cfg(test)]
mod tests;
mod type_reference_layouts;

use crate::builder::generic_bindings::GenericLayoutBinding;
use crate::builder::generic_bindings::binding_for_type;
use crate::builder::generic_bindings::fixed_array_length;
use crate::builder::private_callback_closure::close_private_callback_layouts;
use crate::sizing::fat_descriptor_layout;
use crate::{
    BitFieldLayout, DataLayout, FieldLayout, LayoutPlan, RepeatedFieldLayout, StoredIntegerLayout,
    TargetClosedDataLayoutIdentity, TargetClosedPrivateCallbackDemand, TypeLayout,
    TypeLayoutDescriptor, VariantLayout,
};
use arena::{Arena, Handle};
use diagnostics::Diagnostic;
use representation_selections::OpaqueRepresentationSelection;
use symbols::{BuiltinType, SymbolHandle};
use target::NativeTarget;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// Build the selected target's general physical layout plan.
///
/// Opaque declarations remain absent from the eager top-level catalog. When a
/// by-value layout reaches one through another concrete shape, the exact
/// compiler-validated selection redirects physical derivation to its carrier.
/// Merely supplying a selection does not create a package demand or expose the
/// carrier as the semantic source type.
pub fn build_layout_plan(
    program: &TypedTrees,
    target: NativeTarget,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
) -> Result<LayoutPlan, Diagnostic> {
    build_layout_plan_for_roots(
        program,
        target,
        opaque_representation_selections,
        program.data_definitions().iter().map(|data| data.symbol),
    )
}

/// Build the physical catalog reachable from the requested declaration roots.
///
/// Callback closure requests only its native parameter roots. Scalar symbols
/// have no data declaration, and generic, opaque, and proof-only declarations
/// remain outside the top-level catalog just as in the complete catalog.
/// Inline field dependencies are laid out recursively. An unresolved family
/// reached through a requested root still requires a target-closed Terminal
/// view; unrelated declarations never acquire a physical layout demand.
pub fn build_layout_plan_for_roots(
    program: &TypedTrees,
    target: NativeTarget,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
    roots: impl IntoIterator<Item = SymbolHandle>,
) -> Result<LayoutPlan, Diagnostic> {
    let roots = roots.into_iter().collect::<Vec<_>>();
    let mut builder = LayoutBuilder::new(
        program,
        target,
        opaque_representation_selections,
        OpaqueRepresentationDemand::CatalogConstruction,
    );

    // Math roster N1: proof-only data (recursive, or holding proof-only
    // inline) HAS no layout, by definition -- skip it the way generic
    // templates are skipped. Runtime admissibility belongs to checking; this
    // catalog only omits carriers without geometry. An explicit recursive
    // by-value demand is rejected by the visit-stack backstop.
    let proof_only = typed_trees::proof_only::classify(program);

    for data_definition in program.data_definitions() {
        if !roots.contains(&data_definition.symbol) {
            continue;
        }
        if !data_definition.type_parameters.is_empty() {
            continue;
        }
        if data_definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque {
            continue;
        }
        if proof_only.is_proof_only(data_definition.symbol) {
            continue;
        }
        builder.layout_data_definition(data_definition.symbol)?;
    }

    builder.finish()
}

/// Compute the selected target's concrete size/alignment for one typed reference.
/// This consumes type structure, not check facts, and does not authorize execution.
/// Task activation elaboration uses this for argument, outcome, and
/// continuation-live values without duplicating layout rules. References do
/// not descend into their referee and therefore never demand or consume an
/// opaque representation selection. A direct by-value opaque request is an
/// actual layout demand and rejects when the authoritative build selected no
/// representation.
pub fn layout_type_reference(
    program: &TypedTrees,
    target: NativeTarget,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
    type_reference: TypeReferenceHandle,
) -> Result<TypeLayout, Diagnostic> {
    LayoutBuilder::new(
        program,
        target,
        opaque_representation_selections,
        OpaqueRepresentationDemand::ByValue,
    )
    .layout_type_reference_handle(type_reference, &[])
}

/// The complete layout catalog eagerly visits declarations that may never be
/// used. Only an explicit type-layout request is proof that an unselected
/// opaque is demanded by value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpaqueRepresentationDemand {
    CatalogConstruction,
    ByValue,
}

struct LayoutBuilder<'program> {
    data_definitions: &'program [DataDefinition],
    data_layouts: Arena<DataLayout>,
    data_visiting: LayoutVisitStack,
    fields: Arena<FieldLayout>,
    bit_fields: Vec<BitFieldLayout>,
    stored_integers: Vec<StoredIntegerLayout>,
    repeated_fields: Vec<RepeatedFieldLayout>,
    private_callback_demands: Vec<TargetClosedPrivateCallbackDemand>,
    data_layout_identities: Vec<TargetClosedDataLayoutIdentity>,
    opaque_representation_selections: &'program [OpaqueRepresentationSelection],
    opaque_representation_demand: OpaqueRepresentationDemand,
    /// One recorded MONOMORPHIZED instance per generic data definition: the
    /// definition symbol paired with the canonical display of its type
    /// arguments. The instance's `DataLayout` is keyed by the DEFINITION symbol
    /// (that is what downstream field-offset resolution looks up through the
    /// type descriptor), so a program may instantiate each generic data with
    /// ONE argument list; a second, DIFFERENT instantiation is a clean error
    /// until per-instance identity is threaded through descriptors.
    generic_instance_signatures: Vec<(SymbolHandle, String)>,
    trait_definitions: &'program [TraitDefinition],
    program: &'program TypedTrees,
    target: NativeTarget,
    variants: Arena<VariantLayout>,
}

const INLINE_LAYOUT_VISIT_COUNT: usize = 16;

struct LayoutVisitStack {
    inline: [Option<SymbolHandle>; INLINE_LAYOUT_VISIT_COUNT],
    len: usize,
    overflow: Vec<SymbolHandle>,
}

impl LayoutVisitStack {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            inline: [None; INLINE_LAYOUT_VISIT_COUNT],
            len: 0,
            overflow: Vec::with_capacity(capacity.saturating_sub(INLINE_LAYOUT_VISIT_COUNT)),
        }
    }

    fn contains(&self, symbol: SymbolHandle) -> bool {
        self.inline
            .iter()
            .take(self.len.min(INLINE_LAYOUT_VISIT_COUNT))
            .flatten()
            .any(|candidate| *candidate == symbol)
            || self.overflow.contains(&symbol)
    }

    fn push(&mut self, symbol: SymbolHandle) {
        if self.len < INLINE_LAYOUT_VISIT_COUNT {
            self.inline[self.len] = Some(symbol);
        } else {
            self.overflow.push(symbol);
        }

        self.len += 1;
    }

    fn pop(&mut self) {
        if self.len == 0 {
            return;
        }

        self.len -= 1;
        if self.len < INLINE_LAYOUT_VISIT_COUNT {
            self.inline[self.len] = None;
        } else {
            self.overflow.pop();
        }
    }
}

impl<'program> LayoutBuilder<'program> {
    fn new(
        program: &'program TypedTrees,
        target: NativeTarget,
        opaque_representation_selections: &'program [OpaqueRepresentationSelection],
        opaque_representation_demand: OpaqueRepresentationDemand,
    ) -> Self {
        let data_definitions = program.data_definitions();
        let field_capacity = data_definitions
            .iter()
            .map(|definition| {
                program
                    .data_members(definition)
                    .iter()
                    .filter(|member| matches!(member, DataMember::Field(_)))
                    .count()
            })
            .sum::<usize>();
        let variant_capacity = data_definitions
            .iter()
            .map(|definition| {
                program
                    .data_members(definition)
                    .iter()
                    .filter(|member| matches!(member, DataMember::Variant(_)))
                    .count()
            })
            .sum();

        Self {
            data_definitions,
            data_layouts: Arena::with_capacity(data_definitions.len()),
            data_visiting: LayoutVisitStack::with_capacity(data_definitions.len()),
            fields: Arena::with_capacity(field_capacity),
            bit_fields: Vec::new(),
            stored_integers: Vec::new(),
            repeated_fields: Vec::new(),
            private_callback_demands: Vec::new(),
            data_layout_identities: Vec::new(),
            opaque_representation_selections,
            opaque_representation_demand,
            generic_instance_signatures: Vec::new(),
            trait_definitions: program.traits(),
            program,
            target,
            variants: Arena::with_capacity(variant_capacity),
        }
    }

    fn finish(self) -> Result<LayoutPlan, Diagnostic> {
        let (data_layout_identities, private_callback_layouts) = close_private_callback_layouts(
            self.program,
            &self.data_layouts,
            &self.fields,
            &self.data_layout_identities,
            &self.private_callback_demands,
            self.target,
        )?;
        Ok(LayoutPlan {
            data_layouts: self.data_layouts,
            fields: self.fields,
            bit_fields: self.bit_fields,
            stored_integers: self.stored_integers,
            repeated_fields: self.repeated_fields,
            variants: self.variants,
            private_callback_demands: self.private_callback_demands,
            data_layout_identities: data_layout_identities,
            private_callback_layouts,
        })
    }

    fn layout_data_definition(&mut self, symbol: SymbolHandle) -> Result<TypeLayout, Diagnostic> {
        if let Some((_, data_layout)) = self.data_layout_entry_by_symbol(symbol) {
            return Ok(data_layout.layout);
        }

        if self.data_visiting.contains(symbol) {
            // A data type reached while it is still being laid out contains itself as an
            // INLINE value (directly, or through a cycle of inline fields), so it has no
            // finite size -- a `Node { next: Node }` value would nest endlessly. This is
            // inherently impossible, not a missing feature; name the type and point at the
            // fix (indirection) rather than exposing the internal symbol index.
            let name = self
                .data_definition_by_symbol(symbol)
                .ok()
                .map(|definition| definition.name.as_str().to_owned());
            let target = name
                .as_deref()
                .map(|name| format!("`{name}`"))
                .unwrap_or_else(|| format!("data symbol {}", symbol.arena_index()));
            return Err(Diagnostic::error(format!(
                "recursive data type {target} has no finite size: it contains itself \
                 (directly or through a cycle) as an inline field, which would nest \
                 endlessly. Break the cycle by making the recursive field an indirection \
                 (a reference) instead of an inline value."
            )));
        }

        self.data_visiting.push(symbol);

        let definition = self.data_definition_by_symbol(symbol)?;
        let data_layout = self.compute_data_layout(definition, &[])?;
        let layout = data_layout.layout;

        self.data_layouts.insert(data_layout);
        self.data_visiting.pop();

        Ok(layout)
    }

    fn slice_layout(&self) -> TypeLayout {
        fat_descriptor_layout(self.target)
    }

    fn is_compiler_placed_accessor_definition(&self, definition: &DataDefinition) -> bool {
        self.program
            .placed_view_plans
            .iter()
            .flat_map(|view| &view.fields)
            .any(|field| field.accessor_data_symbol == definition.symbol)
    }

    fn data_definition_by_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> Result<&'program DataDefinition, Diagnostic> {
        self.data_definitions
            .iter()
            .find(|definition| definition.symbol == symbol)
            .ok_or_else(|| {
                Diagnostic::error(format!("unknown data type symbol {}", symbol.arena_index()))
            })
    }

    /// The planned layout entry keyed `symbol`, when that data type already has one.
    fn data_layout_entry_by_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> Option<(Handle<DataLayout>, &DataLayout)> {
        self.data_layouts
            .iter()
            .find(|(_, data_layout)| data_layout.symbol == symbol)
    }

    fn builtin_type_layout(&self, symbol: SymbolHandle) -> Option<TypeLayout> {
        if Some(symbol) == self.program.symbols.builtin_type_symbol(BuiltinType::UInt) {
            return Some(TypeLayout {
                size: self.target.pointer_size,
                alignment: self.target.pointer_alignment,
            });
        }

        if Some(symbol) == self.program.symbols.builtin_type_symbol(BuiltinType::Int) {
            return Some(TypeLayout {
                size: self.target.pointer_size,
                alignment: self.target.pointer_alignment,
            });
        }

        None
    }

    /// The descriptor of a type reference under `bindings`: a reference to a
    /// bound GENERIC TYPE PARAMETER resolves to the descriptor of its ARGUMENT,
    /// so a monomorphized instance's `val: T` field carries the substituted
    /// descriptor (arithmetic domain, storage symbol) instead of an opaque
    /// parameter symbol. Callers outside any instance pass no bindings.
    fn type_descriptor(
        &self,
        type_reference: TypeReferenceHandle,
        bindings: &[GenericLayoutBinding<'program>],
    ) -> TypeLayoutDescriptor {
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Reference {
                referee, access, ..
            } => TypeLayoutDescriptor::Reference {
                referee: Box::new(self.type_descriptor(*referee, bindings)),
                is_mutable: access.is_exclusive(),
            },
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let base = self.type_descriptor(*base_type, bindings);
                let constraint_list = self.program.type_reference_table.constraints(*constraints);
                // Qualification preserves the base descriptor, including an
                // exact fixed array. Arithmetic policy remains separate metadata.
                TypeLayoutDescriptor::Constrained {
                    base_type: Box::new(base),
                    domain: constraint_list
                        .iter()
                        .find_map(|constraint| match constraint {
                            TypeConstraintNode::ArithmeticDomain(domain) => Some(*domain),
                            _ => None,
                        })
                        .unwrap_or(numerics::arithmetic::ArithmeticDomain::Exact),
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => match fixed_array_length(self.program, type_reference, length, bindings) {
                Some(length) => TypeLayoutDescriptor::FixedArray {
                    element_type: Box::new(self.type_descriptor(*element_type, bindings)),
                    length,
                },
                // A recipe length closes only in a closed program copy; an
                // unclosed one degrades to Unit exactly like an unresolved
                // const parameter (layout cannot size it).
                None => TypeLayoutDescriptor::Unit,
            },
            TypeReferenceNode::Slice { element_type } => TypeLayoutDescriptor::Slice {
                element_type: Box::new(self.type_descriptor(*element_type, bindings)),
            },
            TypeReferenceNode::DynamicTrait { symbol, name, .. } => {
                TypeLayoutDescriptor::DynamicTrait {
                    symbol: *symbol,
                    name: name.clone(),
                }
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                ..
            } => {
                if let Some(binding) = binding_for_type(*base_symbol, base_name, bindings) {
                    return self.type_descriptor(binding.argument, bindings);
                }
                TypeLayoutDescriptor::Named {
                    symbol: *base_symbol,
                    name: base_name.clone(),
                }
            }
            TypeReferenceNode::Named { symbol, name } => {
                if let Some(binding) = binding_for_type(*symbol, name, bindings) {
                    return self.type_descriptor(binding.argument, bindings);
                }
                TypeLayoutDescriptor::Named {
                    symbol: *symbol,
                    name: name.clone(),
                }
            }
            TypeReferenceNode::ConstExpression(_) => TypeLayoutDescriptor::Unit,
            TypeReferenceNode::Unit => TypeLayoutDescriptor::Unit,
        }
    }
}
