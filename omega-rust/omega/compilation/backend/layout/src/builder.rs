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
use crate::builder::private_callback_closure::close_two_hop_private_callback_paths;
use crate::sizing::fat_descriptor_layout;
use crate::{
    BitFieldLayout, DataLayout, FieldLayout, LayoutPlan, RepeatedFieldLayout, StoredIntegerLayout,
    TargetClosedPlanLaidDataLayoutIdentity, TargetClosedPrivateCallbackDemand, TypeLayout,
    TypeLayoutDescriptor, VariantLayout,
};
use arena::Arena;
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
    plan_laid_layout_identities: Vec<TargetClosedPlanLaidDataLayoutIdentity>,
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
            plan_laid_layout_identities: Vec::new(),
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
        let two_hop_private_callback_paths = close_two_hop_private_callback_paths(
            self.program,
            &self.data_layouts,
            &self.fields,
            &self.plan_laid_layout_identities,
            &self.private_callback_demands,
        )?;
        Ok(LayoutPlan {
            data_layouts: self.data_layouts,
            fields: self.fields,
            bit_fields: self.bit_fields,
            stored_integers: self.stored_integers,
            repeated_fields: self.repeated_fields,
            variants: self.variants,
            private_callback_demands: self.private_callback_demands,
            plan_laid_layout_identities: self.plan_laid_layout_identities,
            two_hop_private_callback_paths,
        })
    }

    fn layout_data_definition(&mut self, symbol: SymbolHandle) -> Result<TypeLayout, Diagnostic> {
        if let Some(data_layout) = self
            .data_layouts
            .iter()
            .find(|(_, data_layout)| data_layout.symbol == symbol)
            .map(|(_, data_layout)| data_layout)
        {
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
                // An owned `[u8; N] in <named-domain>` field is the variable-fill
                // bounded byte carrier (#66): a NAMED (text) domain over a fixed
                // array. It becomes its own `BoundedByteBuffer` descriptor --
                // `{len, bytes}` is a distinct layout from the always-full
                // `FixedArray`, and the named domain does not otherwise survive to
                // the backend (only arithmetic domains do), so the carrier needs
                // its own variant to be recognizable downstream. The OmegaLayout
                // family stays a PLAIN array (see the TypeLayout arm above).
                let has_named_domain = constraint_list.iter().any(|constraint| match constraint {
                    TypeConstraintNode::Domain(name) => {
                        !typed_trees::wire::is_layout_domain_constraint(name)
                            && language_semantics::CarryPermission::from_name(name.as_str())
                                .is_none()
                    }
                    _ => false,
                });
                match base {
                    TypeLayoutDescriptor::FixedArray {
                        element_type,
                        length,
                    } if has_named_domain => TypeLayoutDescriptor::BoundedByteBuffer {
                        element_type,
                        capacity: length,
                    },
                    base => TypeLayoutDescriptor::Constrained {
                        base_type: Box::new(base),
                        domain: constraint_list
                            .iter()
                            .find_map(|constraint| match constraint {
                                TypeConstraintNode::ArithmeticDomain(domain) => Some(*domain),
                                _ => None,
                            })
                            .unwrap_or(numerics::arithmetic::ArithmeticDomain::Exact),
                    },
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => match fixed_array_length(self.program, length, bindings) {
                Some(length) => TypeLayoutDescriptor::FixedArray {
                    element_type: Box::new(self.type_descriptor(*element_type, bindings)),
                    length,
                },
                // ConstCall lengths are substituted to literals by the
                // orchestration const-eval pass before layout; an unresolved
                // one degrades to Unit exactly like an unresolved const
                // parameter (layout cannot size it).
                None => TypeLayoutDescriptor::Unit,
            },
            TypeReferenceNode::Slice { element_type } => TypeLayoutDescriptor::Slice {
                element_type: Box::new(self.type_descriptor(*element_type, bindings)),
            },
            TypeReferenceNode::DynamicTrait {
                symbol,
                name,
                conformance,
                conformance_carrier,
                conformance_name,
            } => TypeLayoutDescriptor::DynamicTrait {
                symbol: *symbol,
                name: name.clone(),
                conformance: *conformance,
                conformance_carrier: conformance_carrier.clone(),
                conformance_name: conformance_name.clone(),
            },
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
