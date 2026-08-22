use crate::packing::{PlannedField, pack_fields, pack_fields_at, place_fields_by_plan};
use crate::sizing::{
    dynamic_trait_descriptor_layout, fat_descriptor_layout, primitive_type_layout,
};
use crate::{
    BitFieldFragment, BitFieldLayout, DataLayout, DataShape, FieldLayout, LayoutPlan,
    MachineLayout, RepeatedFieldLayout, StoredIntegerLayout, TypeLayout, TypeLayoutDescriptor,
    VariantLayout,
};
use omega_target::NativeTarget;
use psi_arena::Arena;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::data::{DataDefinition, DataMember, DataShapeKind};
use psi_checked_trees::machine::Machine;
use psi_checked_trees::trait_definition::TraitDefinition;
use psi_checked_trees::types::{
    FixedArrayLength, PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use psi_diagnostics::Diagnostic;
use psi_symbols::{BuiltinType, SymbolHandle};

pub fn build_layout_plan(
    program: &CheckedTrees,
    target: NativeTarget,
) -> Result<LayoutPlan, Diagnostic> {
    let mut builder = LayoutBuilder::new(program, target);

    // Math roster N1: proof-only data (recursive, or holding proof-only
    // inline) HAS no layout, by definition -- skip it the way generic
    // templates are skipped. Validation has already fenced every runtime
    // consumption; anything that still demands a layout for one of these
    // downstream is a pipeline bug, caught by the visit-stack backstop.
    let proof_only = psi_checked_trees::proof_only::classify(&program.typed);

    for data_definition in program.data_definitions() {
        if !data_definition.type_parameters.is_empty() {
            continue;
        }
        if data_definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque {
            continue;
        }
        if proof_only.is_proof_only(data_definition.symbol) {
            continue;
        }
        builder.layout_data_definition(data_definition.symbol)?;
    }

    for machine in program.machines() {
        // A generic TEMPLATE machine (unresolved type parameters -- e.g. the
        // `Box::stored<T>` a container instance was cloned FROM) has no
        // layout; its concrete clones lay out instead. Stage-1
        // monomorphization clears the parameter span when it substitutes
        // in place, so anything still carrying parameters here is
        // template-only (its value calls stay behind the validation fence).
        // The same holds for a machine ATTACHED to generic template data
        // (`Cell::touch_count(&self)` -- no own params, but `self` is the
        // template `Cell<T>`): its clones attach to the concrete instances.
        if !program.machine_type_parameters(machine).is_empty() {
            continue;
        }
        if machine.attached_data.as_ref().is_some_and(|attached| {
            program.data_definitions().iter().any(|definition| {
                definition.name.as_str() == attached.as_str()
                    && !definition.type_parameters.is_empty()
            })
        }) {
            continue;
        }
        builder.layout_machine(machine.symbol)?;
    }

    Ok(builder.finish())
}

/// Compute the selected target's concrete size/alignment for one checked type
/// reference. Task activation elaboration uses this for argument, outcome, and
/// continuation-live values without duplicating layout rules.
pub fn layout_type_reference(
    program: &CheckedTrees,
    target: NativeTarget,
    type_reference: TypeReferenceHandle,
) -> Result<TypeLayout, Diagnostic> {
    LayoutBuilder::new(program, target).layout_type_reference_handle(type_reference)
}

struct LayoutBuilder<'program> {
    data_definitions: &'program [DataDefinition],
    data_layouts: Arena<DataLayout>,
    data_visiting: LayoutVisitStack,
    fields: Arena<FieldLayout>,
    bit_fields: Vec<BitFieldLayout>,
    stored_integers: Vec<StoredIntegerLayout>,
    repeated_fields: Vec<RepeatedFieldLayout>,
    /// One recorded MONOMORPHIZED instance per generic data definition: the
    /// definition symbol paired with the canonical display of its type
    /// arguments. The instance's `DataLayout` is keyed by the DEFINITION symbol
    /// (that is what downstream field-offset resolution looks up through the
    /// type descriptor), so a program may instantiate each generic data with
    /// ONE argument list; a second, DIFFERENT instantiation is a clean error
    /// until per-instance identity is threaded through descriptors.
    generic_instance_signatures: Vec<(SymbolHandle, String)>,
    machine_definitions: &'program [Machine],
    machine_layouts: Arena<MachineLayout>,
    machine_visiting: LayoutVisitStack,
    trait_definitions: &'program [TraitDefinition],
    program: &'program CheckedTrees,
    target: NativeTarget,
    variants: Arena<VariantLayout>,
}

const INLINE_LAYOUT_VISIT_COUNT: usize = 16;

struct LayoutVisitStack {
    inline: [Option<SymbolHandle>; INLINE_LAYOUT_VISIT_COUNT],
    len: usize,
    overflow: Vec<SymbolHandle>,
}

#[derive(Debug, Clone, Copy)]
struct GenericLayoutBinding<'program> {
    parameter_symbol: SymbolHandle,
    parameter_name: &'program str,
    argument: TypeReferenceHandle,
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
    fn new(program: &'program CheckedTrees, target: NativeTarget) -> Self {
        let data_definitions = program.data_definitions();
        let machine_definitions = program.machines();
        let field_capacity = data_definitions
            .iter()
            .map(|definition| {
                program
                    .data_members(definition)
                    .iter()
                    .filter(|member| matches!(member, DataMember::Field(_)))
                    .count()
            })
            .sum::<usize>()
            .checked_add(
                machine_definitions
                    .iter()
                    .map(|machine| program.machine_owned_data(machine).len())
                    .sum::<usize>(),
            )
            .expect("layout field capacity overflow");
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
            generic_instance_signatures: Vec::new(),
            machine_definitions,
            machine_layouts: Arena::with_capacity(machine_definitions.len()),
            machine_visiting: LayoutVisitStack::with_capacity(machine_definitions.len()),
            trait_definitions: program.traits(),
            program,
            target,
            variants: Arena::with_capacity(variant_capacity),
        }
    }

    fn finish(self) -> LayoutPlan {
        LayoutPlan {
            data_layouts: self.data_layouts,
            fields: self.fields,
            bit_fields: self.bit_fields,
            stored_integers: self.stored_integers,
            repeated_fields: self.repeated_fields,
            machine_layouts: self.machine_layouts,
            variants: self.variants,
        }
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

    fn layout_machine(&mut self, symbol: SymbolHandle) -> Result<TypeLayout, Diagnostic> {
        if let Some(machine_layout) = self
            .machine_layouts
            .iter()
            .find(|(_, machine_layout)| machine_layout.symbol == symbol)
            .map(|(_, machine_layout)| machine_layout)
        {
            return Ok(machine_layout.layout);
        }

        if self.machine_visiting.contains(symbol) {
            return Err(Diagnostic::error(format!(
                "recursive machine layout is not supported yet for symbol {}",
                symbol.arena_index()
            )));
        }

        self.machine_visiting.push(symbol);

        let machine = self.machine_definition_by_symbol(symbol)?;
        let machine_layout = self.compute_machine_layout(machine)?;
        let layout = machine_layout.layout;

        self.machine_layouts.insert(machine_layout);
        self.machine_visiting.pop();

        Ok(layout)
    }

    fn compute_data_layout(
        &mut self,
        definition: &DataDefinition,
        bindings: &[GenericLayoutBinding<'program>],
    ) -> Result<DataLayout, Diagnostic> {
        // A compiler-derived placed accessor is opaque to source construction,
        // but it is not zero-sized at runtime: helpers may receive one without
        // the enclosing Placed view, so the value must retain its exact field
        // address. The typed placed-plan table is the authority for this
        // classification; generated-name patterns are not.
        if self.is_compiler_placed_accessor_definition(definition) {
            if definition.supply_mode != psi_language_semantics::DataSupplyMode::BoundaryOpaque {
                return Err(Diagnostic::error(format!(
                    "compiler-derived placed accessor `{}` lost its opaque supply mode",
                    definition.name
                )));
            }
            return Ok(DataLayout {
                symbol: definition.symbol,
                name: definition.name.clone(),
                shape: DataShape::Record {
                    fields: psi_arena::HandleSpan::empty(),
                },
                layout: TypeLayout {
                    size: self.target.pointer_size,
                    alignment: self.target.pointer_alignment,
                },
            });
        }

        let members = self.program.data_members(definition);
        if matches!(
            DataDefinition::shape_kind_from_members(members),
            DataShapeKind::Enum | DataShapeKind::Mixed
        ) {
            return self.compute_case_bearing_layout(definition, members, bindings);
        }

        let fields = members
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
                DataMember::Field(_) => None,
                DataMember::Variant(_) => None,
            })
            .map(|field| {
                let layout = self
                    .layout_type_reference_handle_with_bindings(field.type_reference, bindings)?;
                Ok(PlannedField {
                    symbol: field.symbol,
                    name: field.name.clone(),
                    type_symbol: self.program.type_reference_symbol(field.type_reference),
                    type_name: self
                        .program
                        .display_type_reference_with_constraints(field.type_reference)
                        .into(),
                    type_descriptor: self
                        .type_descriptor_with_bindings(field.type_reference, bindings),
                    layout,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;

        // PLAN-LAID VALUE TYPES (layouts L4): a synthesized `Policy<Schema>`
        // instance is placed at its validated plan's offsets instead of the
        // native packing; downstream field-offset resolution reads the baked
        // `FieldLayout.offset` exactly as for packed records.
        if let Some(plan) = self
            .program
            .plan_laid_layouts
            .iter()
            .find(|plan| definition.symbol == plan.data_symbol)
        {
            if plan.offsets.len() != fields.len()
                || plan
                    .field_symbols
                    .iter()
                    .copied()
                    .ne(fields.iter().map(|field| field.symbol))
            {
                return Err(Diagnostic::error(format!(
                    "plan-laid data `{}` changed its exact field identity inventory ({} fields, {} planned offsets)",
                    definition.name,
                    fields.len(),
                    plan.offsets.len()
                )));
            }
            let (fields, layout) = place_fields_by_plan(
                &mut self.fields,
                fields,
                &plan.offsets,
                TypeLayout {
                    size: plan.size,
                    alignment: plan.align,
                },
            );
            for bit_field in &plan.bit_fields {
                let Some(field) = self
                    .fields
                    .span(fields)
                    .and_then(|fields| fields.get(bit_field.field_index))
                else {
                    return Err(Diagnostic::error(format!(
                        "plan-laid data `{}` has no field at bit-placement index {}",
                        definition.name, bit_field.field_index
                    )));
                };
                self.bit_fields.push(BitFieldLayout {
                    field: field.symbol,
                    fragments: bit_field
                        .fragments
                        .iter()
                        .map(|fragment| BitFieldFragment {
                            container_byte_offset: fragment.container_byte_offset,
                            container_width_bits: fragment.container_width_bits,
                            destination_lsb: fragment.destination_lsb,
                            source_lsb: fragment.source_lsb,
                            width: fragment.width,
                        })
                        .collect(),
                });
            }
            for integer_field in &plan.integer_fields {
                let Some(field) = self
                    .fields
                    .span(fields)
                    .and_then(|fields| fields.get(integer_field.field_index))
                else {
                    return Err(Diagnostic::error(format!(
                        "plan-laid data `{}` has no field at stored-integer index {}",
                        definition.name, integer_field.field_index
                    )));
                };
                self.stored_integers.push(StoredIntegerLayout {
                    field: field.symbol,
                    stored_width_bits: integer_field.stored_width_bits,
                    interpretation: integer_field.interpretation,
                    write_is_total: integer_field.write_is_total,
                });
            }
            for repeated_field in &plan.repeated_fields {
                let Some(field) = self
                    .fields
                    .span(fields)
                    .and_then(|fields| fields.get(repeated_field.field_index))
                else {
                    return Err(Diagnostic::error(format!(
                        "plan-laid data `{}` has no field at repeated-placement index {}",
                        definition.name, repeated_field.field_index
                    )));
                };
                self.repeated_fields.push(RepeatedFieldLayout {
                    field: field.symbol,
                    element_stride: repeated_field.element_stride,
                });
            }

            return Ok(DataLayout {
                symbol: definition.symbol,
                name: definition.name.clone(),
                shape: DataShape::Record { fields },
                layout,
            });
        }

        let (fields, layout) = pack_fields(&mut self.fields, fields);

        Ok(DataLayout {
            symbol: definition.symbol,
            name: definition.name.clone(),
            shape: DataShape::Record { fields },
            layout,
        })
    }

    /// Lay out a case-bearing shape (sum OR mixed) as a TAG-PREFIXED OVERLAY:
    /// the i32 tag sits at offset 0, the COMMON fields (mixed shapes only) pack
    /// immediately after the tag, and every case's payload fields pack from a
    /// SHARED base offset after the common fields (aligned up to the strictest
    /// payload field alignment), overlaying each other. The value's size covers
    /// the LARGEST case payload; payload-less pure sums keep the historical
    /// 4-byte tag-only layout. Tag-first is deliberate (see `DataShape::Enum`):
    /// tag-only compares address the first `ENUM_TAG_BYTES` of the value with
    /// no layout context, so the tag offset must stay the constant 0; common-
    /// field offsets remain case-independent constants either way. No niche
    /// packing ever: the zero bit pattern is the zero case (with zeroed common
    /// fields and payload) and stays valid.
    fn compute_case_bearing_layout(
        &mut self,
        definition: &DataDefinition,
        members: &[DataMember],
        bindings: &[GenericLayoutBinding<'program>],
    ) -> Result<DataLayout, Diagnostic> {
        const TAG_LAYOUT: TypeLayout = TypeLayout {
            size: crate::ENUM_TAG_BYTES,
            alignment: crate::ENUM_TAG_BYTES,
        };

        // Common fields (mixed shapes) pack right after the tag; their end
        // offset is the floor under every case's payload overlay.
        let planned_common = members
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
                DataMember::Field(_) => None,
                DataMember::Variant(_) => None,
            })
            .map(|field| {
                let layout = self
                    .layout_type_reference_handle_with_bindings(field.type_reference, bindings)?;
                Ok(PlannedField {
                    symbol: field.symbol,
                    name: field.name.clone(),
                    type_symbol: self.program.type_reference_symbol(field.type_reference),
                    type_name: self
                        .program
                        .display_type_reference_with_constraints(field.type_reference)
                        .into(),
                    type_descriptor: self
                        .type_descriptor_with_bindings(field.type_reference, bindings),
                    layout,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let (common_fields, common_layout) =
            pack_fields_at(&mut self.fields, planned_common, TAG_LAYOUT.size);
        let common_end = common_layout.size.max(TAG_LAYOUT.size);

        // Plan every case's payload fields next: the shared payload base offset
        // depends on the strictest alignment across ALL cases.
        let mut planned_variants = Vec::new();
        let mut payload_alignment = 1usize;
        for member in members {
            let DataMember::Variant(variant) = member else {
                continue;
            };
            let planned = self
                .program
                .data_payload_fields(variant)
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| {
                    let layout = self.layout_type_reference_handle_with_bindings(
                        field.type_reference,
                        bindings,
                    )?;
                    Ok(PlannedField {
                        symbol: field.symbol,
                        name: field.name.clone(),
                        type_symbol: self.program.type_reference_symbol(field.type_reference),
                        type_name: self
                            .program
                            .display_type_reference_with_constraints(field.type_reference)
                            .into(),
                        type_descriptor: self
                            .type_descriptor_with_bindings(field.type_reference, bindings),
                        layout,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            for field in &planned {
                payload_alignment = payload_alignment.max(field.layout.alignment);
            }
            planned_variants.push((variant.symbol, variant.name.clone(), planned));
        }

        let alignment = TAG_LAYOUT
            .alignment
            .max(common_layout.alignment)
            .max(payload_alignment);
        let payload_base = common_end.div_ceil(payload_alignment) * payload_alignment;

        let mut end_offset = common_end;
        let variant_layouts = planned_variants
            .into_iter()
            .map(|(symbol, name, planned)| {
                let (fields, payload_layout) =
                    pack_fields_at(&mut self.fields, planned, payload_base);
                end_offset = end_offset.max(payload_layout.size);
                VariantLayout {
                    symbol,
                    name,
                    fields,
                }
            })
            .collect::<Vec<_>>();
        let variants = self.variants.insert_many(variant_layouts);

        Ok(DataLayout {
            symbol: definition.symbol,
            name: definition.name.clone(),
            shape: DataShape::Enum {
                common_fields,
                variants,
            },
            layout: TypeLayout {
                size: end_offset.div_ceil(alignment) * alignment,
                alignment,
            },
        })
    }

    fn compute_machine_layout(&mut self, machine: &Machine) -> Result<MachineLayout, Diagnostic> {
        let data_field_capacity = self
            .data_definitions
            .iter()
            .find(|definition| Some(&definition.name) == machine.attached_data.as_ref())
            .map(|definition| {
                self.program
                    .data_members(definition)
                    .iter()
                    .filter(|member| {
                        matches!(member, DataMember::Field(field) if !field.relevance.is_erased())
                    })
                    .count()
            })
            .unwrap_or(0);
        let field_capacity = data_field_capacity
            .checked_add(self.program.machine_owned_data(machine).len())
            .expect("machine layout field capacity overflow");
        let mut fields = Vec::with_capacity(field_capacity);

        if let Some(data_definition) = self
            .data_definitions
            .iter()
            .find(|definition| Some(&definition.name) == machine.attached_data.as_ref())
        {
            for member in self.program.data_members(data_definition) {
                let DataMember::Field(field) = member else {
                    continue;
                };
                if field.relevance.is_erased() {
                    continue;
                }

                fields.push(PlannedField {
                    symbol: field.symbol,
                    name: field.name.clone(),
                    type_symbol: self.program.type_reference_symbol(field.type_reference),
                    type_name: self
                        .program
                        .display_type_reference_with_constraints(field.type_reference)
                        .into(),
                    type_descriptor: self.type_descriptor(field.type_reference),
                    layout: self.layout_type_reference_handle(field.type_reference)?,
                });
            }
        }

        for owned_data in self.program.machine_owned_data(machine) {
            fields.push(PlannedField {
                symbol: owned_data.symbol,
                name: owned_data.name.clone(),
                type_symbol: self
                    .program
                    .type_reference_symbol(owned_data.type_reference),
                type_name: self
                    .program
                    .display_type_reference_with_constraints(owned_data.type_reference)
                    .into(),
                type_descriptor: self.type_descriptor(owned_data.type_reference),
                layout: self.layout_type_reference_handle(owned_data.type_reference)?,
            });
        }

        let (fields, layout) = pack_fields(&mut self.fields, fields);

        Ok(MachineLayout {
            symbol: machine.symbol,
            name: machine.name.clone(),
            attached_data: machine.attached_data.clone(),
            fields,
            layout,
        })
    }

    fn slice_layout(&self) -> TypeLayout {
        fat_descriptor_layout(self.target)
    }

    fn layout_type_reference_handle(
        &mut self,
        type_reference: TypeReferenceHandle,
    ) -> Result<TypeLayout, Diagnostic> {
        self.layout_type_reference_handle_with_bindings(type_reference, &[])
    }

    fn layout_type_reference_handle_with_bindings(
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
                // A reference to an UNSIZED referee -- a slice `&[T]` or the
                // `string` text view -- is a FAT `{ptr, len}` pointer: a thin
                // pointer cannot carry the element/byte count. (`fat_descriptor_
                // layout` is documented "for slices and text windows".) Every
                // other reference (`&SizedType`) stays a thin pointer.
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
                let referee_unsized = matches!(referee_node, TypeReferenceNode::Slice { .. })
                    || matches!(
                        referee_node,
                        TypeReferenceNode::Named { name, .. } if name.as_str() == "string"
                    );
                if referee_unsized {
                    Ok(fat_descriptor_layout(self.target))
                } else {
                    Ok(TypeLayout {
                        size: self.target.pointer_size,
                        alignment: self.target.pointer_alignment,
                    })
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
                            !psi_checked_trees::wire::is_layout_domain_constraint(name)
                                && psi_language_semantics::CarryPermission::from_name(name.as_str())
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
                    return Ok(TypeLayout {
                        size: self.target.pointer_size.saturating_add(base_layout.size),
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
        arguments: psi_arena::HandleSpan<TypeReferenceHandle>,
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

        if self
            .data_definitions
            .iter()
            .any(|definition| definition.symbol == symbol)
        {
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

    fn machine_definition_by_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> Result<&'program Machine, Diagnostic> {
        self.machine_definitions
            .iter()
            .find(|machine| machine.symbol == symbol)
            .ok_or_else(|| {
                Diagnostic::error(format!("unknown machine symbol {}", symbol.arena_index()))
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

    fn type_descriptor(&self, type_reference: TypeReferenceHandle) -> TypeLayoutDescriptor {
        self.type_descriptor_with_bindings(type_reference, &[])
    }

    /// Like `type_descriptor`, but a reference to a bound GENERIC TYPE PARAMETER
    /// resolves to the descriptor of its ARGUMENT, so a monomorphized instance's
    /// `val: T` field carries the substituted descriptor (arithmetic domain,
    /// storage symbol) instead of an opaque parameter symbol.
    fn type_descriptor_with_bindings(
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
                referee,
                is_mutable,
                ..
            } => TypeLayoutDescriptor::Reference {
                referee: Box::new(self.type_descriptor_with_bindings(*referee, bindings)),
                is_mutable: *is_mutable,
            },
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let base = self.type_descriptor_with_bindings(*base_type, bindings);
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
                        !psi_checked_trees::wire::is_layout_domain_constraint(name)
                            && psi_language_semantics::CarryPermission::from_name(name.as_str())
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
                            .unwrap_or(psi_numerics::arithmetic::ArithmeticDomain::Exact),
                    },
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => match fixed_array_length_with_bindings(self.program, length, bindings) {
                Some(length) => TypeLayoutDescriptor::FixedArray {
                    element_type: Box::new(
                        self.type_descriptor_with_bindings(*element_type, bindings),
                    ),
                    length,
                },
                // ConstCall lengths are substituted to literals by the
                // orchestration const-eval pass before layout; an unresolved
                // one degrades to Unit exactly like an unresolved const
                // parameter (layout cannot size it).
                None => TypeLayoutDescriptor::Unit,
            },
            TypeReferenceNode::Slice { element_type } => TypeLayoutDescriptor::Slice {
                element_type: Box::new(self.type_descriptor_with_bindings(*element_type, bindings)),
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
                    return self.type_descriptor_with_bindings(binding.argument, bindings);
                }
                TypeLayoutDescriptor::Named {
                    symbol: *base_symbol,
                    name: base_name.clone(),
                }
            }
            TypeReferenceNode::Named { symbol, name } => {
                if let Some(binding) = binding_for_type(*symbol, name, bindings) {
                    return self.type_descriptor_with_bindings(binding.argument, bindings);
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

fn binding_for_type<'program>(
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

fn fixed_array_length_with_bindings(
    program: &CheckedTrees,
    length: &FixedArrayLength,
    bindings: &[GenericLayoutBinding<'_>],
) -> Option<usize> {
    match length {
        FixedArrayLength::Literal(length) => Some(*length),
        FixedArrayLength::ConstParameter { symbol, name } => {
            let binding = binding_for_type(*symbol, name, bindings)?;
            const_argument_value(program, binding.argument, bindings, 0)
        }
        FixedArrayLength::ConstCall { .. } => None,
    }
}

fn const_argument_value(
    program: &CheckedTrees,
    argument: TypeReferenceHandle,
    bindings: &[GenericLayoutBinding<'_>],
    depth: usize,
) -> Option<usize> {
    if depth >= 16 {
        return None;
    }
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(argument)
    else {
        return None;
    };
    if !symbol.is_valid() {
        if let Ok(value) = name.as_str().parse::<usize>() {
            return Some(value);
        }
    }
    let binding = binding_for_type(*symbol, name, bindings)?;
    const_argument_value(program, binding.argument, bindings, depth + 1)
}

#[cfg(test)]
mod tests {
    use super::build_layout_plan;
    use crate::DataShape;
    use omega_target::NativeTarget;
    use psi_checked_trees::{CheckFacts, CheckedTrees};
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    fn checked(source: &str) -> CheckedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        CheckedTrees::with_roots(typed, CheckFacts::default())
    }

    #[test]
    fn transparent_record_layout_excludes_erased_fields() {
        let source = r#"
            data Packed {
                head: u8;
                proof [erased]: u64;
                tail: u8;
            }
        "#;
        let checked = checked(source);

        let plan = build_layout_plan(&checked, NativeTarget::host()).expect("layout");
        let packed = plan
            .data_layouts
            .iter()
            .map(|(_, layout)| layout)
            .find(|layout| layout.name.as_str() == "Packed")
            .expect("Packed layout");
        assert_eq!(packed.layout.size, 2);
        assert_eq!(packed.layout.alignment, 1);
        let DataShape::Record { fields } = packed.shape else {
            panic!("Packed should have record layout");
        };
        let fields = plan.fields.span_or_empty(fields);
        assert_eq!(
            fields
                .iter()
                .map(|field| (field.name.as_str(), field.offset))
                .collect::<Vec<_>>(),
            [("head", 0), ("tail", 1)]
        );
    }

    #[test]
    fn attached_machine_layout_excludes_erased_record_fields() {
        let checked = checked(
            r#"
            data Packed {
                head: u8;
                proof [erased]: u64;
                tail: u16;
            }
            machine Packed::read(&self) -> u8 { self.head }
            "#,
        );

        let plan = build_layout_plan(&checked, NativeTarget::host()).expect("layout");
        let data = plan
            .data_layouts
            .iter()
            .map(|(_, layout)| layout)
            .find(|layout| layout.name.as_str() == "Packed")
            .expect("Packed data layout");
        let machine = plan
            .machine_layouts
            .iter()
            .map(|(_, layout)| layout)
            .find(|layout| layout.name.as_str() == "Packed::read")
            .expect("Packed::read machine layout");

        assert_eq!(machine.layout, data.layout);
        assert_eq!(
            plan.fields
                .span_or_empty(machine.fields)
                .iter()
                .map(|field| (field.name.as_str(), field.offset))
                .collect::<Vec<_>>(),
            [("head", 0), ("tail", 2)]
        );
    }

    #[test]
    fn pure_sum_layout_excludes_erased_payloads_without_changing_variants() {
        let checked = checked(
            r#"
            data Message {
                case Empty;
                case Data(value: u8, proof [erased]: u64);
                case ProofOnly(proof [erased]: u64);
            }
            "#,
        );
        let plan = build_layout_plan(&checked, NativeTarget::host()).expect("layout");
        let message = plan
            .data_layouts
            .iter()
            .map(|(_, layout)| layout)
            .find(|layout| layout.name.as_str() == "Message")
            .expect("Message layout");
        assert_eq!(message.layout.size, 8);
        assert_eq!(message.layout.alignment, 4);
        let DataShape::Enum {
            common_fields,
            variants,
        } = message.shape
        else {
            panic!("Message should have case layout");
        };
        assert!(plan.fields.span_or_empty(common_fields).is_empty());
        let variants = plan.variants.span_or_empty(variants);
        assert_eq!(
            variants
                .iter()
                .map(|variant| variant.name.as_str())
                .collect::<Vec<_>>(),
            ["Empty", "Data", "ProofOnly"]
        );
        assert!(plan.fields.span_or_empty(variants[0].fields).is_empty());
        assert_eq!(
            plan.fields
                .span_or_empty(variants[1].fields)
                .iter()
                .map(|field| (field.name.as_str(), field.offset))
                .collect::<Vec<_>>(),
            [("value", 4)]
        );
        assert!(plan.fields.span_or_empty(variants[2].fields).is_empty());
    }

    #[test]
    fn mixed_case_layout_excludes_erased_common_and_payload_fields() {
        let checked = checked(
            r#"
            data Event {
                sequence: u8;
                common_proof [erased]: u64;
                case Ready(value: u16, payload_proof [erased]: u64);
                case Waiting(code: u8);
            }
            "#,
        );
        let plan = build_layout_plan(&checked, NativeTarget::host()).expect("layout");
        let event = plan
            .data_layouts
            .iter()
            .map(|(_, layout)| layout)
            .find(|layout| layout.name.as_str() == "Event")
            .expect("Event layout");
        assert_eq!(event.layout.size, 8);
        assert_eq!(event.layout.alignment, 4);
        let DataShape::Enum {
            common_fields,
            variants,
        } = event.shape
        else {
            panic!("Event should have mixed case layout");
        };
        assert_eq!(
            plan.fields
                .span_or_empty(common_fields)
                .iter()
                .map(|field| (field.name.as_str(), field.offset))
                .collect::<Vec<_>>(),
            [("sequence", 4)]
        );
        let variants = plan.variants.span_or_empty(variants);
        assert_eq!(
            variants
                .iter()
                .map(|variant| variant.name.as_str())
                .collect::<Vec<_>>(),
            ["Ready", "Waiting"]
        );
        assert_eq!(
            plan.fields
                .span_or_empty(variants[0].fields)
                .iter()
                .map(|field| (field.name.as_str(), field.offset))
                .collect::<Vec<_>>(),
            [("value", 6)]
        );
        assert_eq!(
            plan.fields
                .span_or_empty(variants[1].fields)
                .iter()
                .map(|field| (field.name.as_str(), field.offset))
                .collect::<Vec<_>>(),
            [("code", 6)]
        );
    }
}
