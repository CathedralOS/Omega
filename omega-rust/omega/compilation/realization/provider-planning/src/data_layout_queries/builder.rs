/*
We own the layout-plan entry points (`build_layout_plan`,
`build_layout_plan_for_roots`, `layout_type_reference`, and
`try_layout_type_reference`), the `LayoutBuilder` state, its inline-first visit
stack (`LayoutVisitStack`), and generic type/const-argument binding resolution
(`generic_bindings`).

Opaque declarations, generic templates, and proof-only data (recursive or
holding proof-only inline) remain absent from the eager top-level catalog. When
a by-value layout reaches a boundary-opaque declaration through another concrete
shape, the exact compiler-validated selection redirects physical derivation to
its carrier. Inline recursion is caught by `LayoutVisitStack` with an actionable
indirection diagnostic, and monomorphized generic data instances are recorded
under their definition symbol (or poisoned on multi-instantiation collision so
each use sizes privately without ambiguous field offsets).
*/

mod data_layouts;
mod private_callback_closure;
mod semantic_ranges;
#[cfg(test)]
mod tests;
mod type_reference_layouts;

use crate::data_layout_queries::OpaqueCarrierSelection;
use crate::data_layout_queries::builder::generic_bindings::{
    GenericLayoutBinding, binding_for_type, fixed_array_length,
};
use crate::data_layout_queries::builder::private_callback_closure::close_private_callback_layouts;
use crate::data_layout_queries::sizing::pointer_layout;
use crate::data_layout_queries::{
    BitFieldLayout, DataLayout, FieldLayout, LayoutPlan, RepeatedFieldLayout, StoredIntegerLayout,
    TargetClosedDataLayoutIdentity, TargetClosedPrivateCallbackDemand, TypeLayout,
    TypeLayoutDescriptor, VariantLayout,
};
use arena::{Arena, Handle};
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::data::{DataDefinition, DataMember};
use checked_trees::typed_trees::trait_definition::TraitDefinition;
use checked_trees::typed_trees::types::{
    TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use diagnostics::Diagnostic;
use symbols::{BuiltinType, SymbolHandle};
use target::NativeTarget;

pub fn build_layout_plan(
    program: &TypedTrees,
    target: impl Into<crate::data_layout_queries::LayoutTarget>,
    opaque_representation_selections: &[OpaqueCarrierSelection],
) -> Result<LayoutPlan, Diagnostic> {
    build_layout_plan_for_roots(
        program,
        target,
        opaque_representation_selections,
        program.data_definitions().iter().map(|data| data.symbol),
    )
}

pub fn build_layout_plan_for_roots(
    program: &TypedTrees,
    target: impl Into<crate::data_layout_queries::LayoutTarget>,
    opaque_representation_selections: &[OpaqueCarrierSelection],
    roots: impl IntoIterator<Item = SymbolHandle>,
) -> Result<LayoutPlan, Diagnostic> {
    let roots = roots.into_iter().collect::<Vec<_>>();
    let mut builder = LayoutBuilder::new(
        program,
        target.into(),
        opaque_representation_selections,
        OpaqueRepresentationDemand::CatalogConstruction,
    );

    let proof_only = checked_trees::typed_trees::proof_only::classify(program);

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

pub fn layout_type_reference(
    program: &TypedTrees,
    target: impl Into<crate::data_layout_queries::LayoutTarget>,
    opaque_representation_selections: &[OpaqueCarrierSelection],
    type_reference: TypeReferenceHandle,
) -> Result<TypeLayout, Diagnostic> {
    LayoutBuilder::new(
        program,
        target.into(),
        opaque_representation_selections,
        OpaqueRepresentationDemand::ByValue,
    )
    .layout_type_reference_handle(type_reference, &[])
}

pub fn try_layout_type_reference(
    program: &TypedTrees,
    target: impl Into<crate::data_layout_queries::LayoutTarget>,
    opaque_representation_selections: &[OpaqueCarrierSelection],
    type_reference: TypeReferenceHandle,
) -> Result<Option<TypeLayout>, Diagnostic> {
    let mut builder = LayoutBuilder::new(
        program,
        target.into(),
        opaque_representation_selections,
        OpaqueRepresentationDemand::ByValue,
    );
    let layout = builder.layout_type_reference_handle(type_reference, &[])?;
    Ok((!builder.unresolved_array_extent).then_some(layout))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpaqueRepresentationDemand {
    CatalogConstruction,
    ByValue,
}

struct LayoutBuilder<'program> {
    data_definitions: &'program [DataDefinition],
    data_layouts: Arena<DataLayout>,
    data_visiting: LayoutVisitStack,
    unresolved_array_extent: bool, // set by by-value traversal when a fixed-array extent has not closed
    fields: Arena<FieldLayout>,
    bit_fields: Vec<BitFieldLayout>,
    stored_integers: Vec<StoredIntegerLayout>,
    repeated_fields: Vec<RepeatedFieldLayout>,
    private_callback_demands: Vec<TargetClosedPrivateCallbackDemand>,
    data_layout_identities: Vec<TargetClosedDataLayoutIdentity>,
    opaque_representation_selections: &'program [OpaqueCarrierSelection],
    opaque_representation_demand: OpaqueRepresentationDemand,
    generic_instance_signatures: Vec<(SymbolHandle, String)>, // one monomorphized instance per generic data definition symbol
    trait_definitions: &'program [TraitDefinition],
    program: &'program TypedTrees,
    target: NativeTarget,
    layout_profile: Option<target::TargetProfile>,
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
        target: crate::data_layout_queries::LayoutTarget,
        opaque_representation_selections: &'program [OpaqueCarrierSelection],
        opaque_representation_demand: OpaqueRepresentationDemand,
    ) -> Self {
        let (target, layout_profile) = target.into_parts();
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
            unresolved_array_extent: false,
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
            layout_profile,
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
            data_layout_identities,
            private_callback_layouts,
        })
    }

    fn layout_data_definition(&mut self, symbol: SymbolHandle) -> Result<TypeLayout, Diagnostic> {
        if let Some((_, data_layout)) = self.data_layout_entry_by_symbol(symbol) {
            return Ok(data_layout.layout);
        }

        if self.data_visiting.contains(symbol) {
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

    fn is_compiler_accessor_definition(&self, definition: &DataDefinition) -> bool {
        self.program
            .access_view_plans
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

    fn data_layout_entry_by_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> Option<(Handle<DataLayout>, &DataLayout)> {
        self.data_layouts
            .iter()
            .find(|(_, data_layout)| data_layout.symbol == symbol)
    }

    fn builtin_type_layout(&self, symbol: SymbolHandle) -> Option<TypeLayout> {
        if Some(symbol) == self.program.symbols.builtin_type_symbol(BuiltinType::UInt)
            || Some(symbol) == self.program.symbols.builtin_type_symbol(BuiltinType::Int)
        {
            return Some(pointer_layout(self.target));
        }

        None
    }

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
            TypeReferenceNode::Callable { contract } => TypeLayoutDescriptor::Callable {
                contract: *contract,
            },
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
            }
            | TypeReferenceNode::Named {
                symbol: base_symbol,
                name: base_name,
            } => {
                if let Some(binding) = binding_for_type(*base_symbol, base_name, bindings) {
                    return self.type_descriptor(binding.argument, bindings);
                }
                TypeLayoutDescriptor::Named {
                    symbol: *base_symbol,
                    name: base_name.clone(),
                }
            }
            TypeReferenceNode::ConstExpression(_) => TypeLayoutDescriptor::Unit,
            TypeReferenceNode::Unit => TypeLayoutDescriptor::Unit,
        }
    }
}

pub(crate) mod generic_bindings {
    use checked_trees::typed_trees::TypedTrees;
    use checked_trees::typed_trees::types::{
        FixedArrayLength, TypeReferenceHandle, TypeReferenceNode,
    };
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct GenericLayoutBinding<'program> {
        pub(crate) parameter_symbol: SymbolHandle,
        pub(crate) parameter_name: &'program str,
        pub(crate) argument: TypeReferenceHandle,
    }

    pub(crate) fn binding_for_type<'program>(
        symbol: SymbolHandle,
        name: &str,
        bindings: &[GenericLayoutBinding<'program>],
    ) -> Option<GenericLayoutBinding<'program>> {
        bindings
            .iter()
            .find(|binding| {
                if symbol.is_valid() {
                    binding.parameter_symbol == symbol
                } else {
                    binding.parameter_name == name
                }
            })
            .copied()
    }

    pub(crate) fn fixed_array_length(
        program: &TypedTrees,
        type_reference: TypeReferenceHandle,
        length: &FixedArrayLength,
        bindings: &[GenericLayoutBinding<'_>],
    ) -> Option<usize> {
        match length {
            FixedArrayLength::Literal(length) => Some(*length),
            FixedArrayLength::ConstParameter { symbol, name } => {
                let binding = binding_for_type(*symbol, name, bindings)?;
                const_argument_value(program, binding.argument, bindings, 0)
            }
            FixedArrayLength::Expression(_) => None,
            FixedArrayLength::ConstCall { .. } => program.closed_fixed_array_length(type_reference),
        }
    }

    fn const_argument_value(
        program: &TypedTrees,
        argument: TypeReferenceHandle,
        bindings: &[GenericLayoutBinding<'_>],
        depth: usize,
    ) -> Option<usize> {
        if depth >= 16 {
            return None;
        }
        if let Some(value) = program
            .type_reference_table
            .anonymous_numeric_literal_value(argument)
        {
            return Some(value);
        }
        let TypeReferenceNode::Named { symbol, name } =
            program.type_reference_table.type_reference(argument)
        else {
            return None;
        };
        let binding = binding_for_type(*symbol, name, bindings)?;
        const_argument_value(program, binding.argument, bindings, depth + 1)
    }
}
