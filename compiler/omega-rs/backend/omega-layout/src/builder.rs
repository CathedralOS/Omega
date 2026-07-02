use crate::packing::{PlannedField, pack_fields, pack_fields_at};
use crate::sizing::{fat_descriptor_layout, primitive_type_layout};
use crate::{
    DataLayout, DataShape, FieldLayout, LayoutPlan, MachineLayout, TypeLayout,
    TypeLayoutDescriptor, VariantLayout,
};
use omega_checked_trees::CheckedTrees;
use omega_checked_trees::data::{DataDefinition, DataMember, DataShapeKind};
use omega_checked_trees::machine::Machine;
use omega_checked_trees::platform::Platform;
use omega_checked_trees::trait_definition::TraitDefinition;
use omega_checked_trees::types::{
    FixedArrayLength, PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::{BuiltinType, SymbolHandle};
use omega_target::NativeTarget;

pub fn build_layout_plan(
    program: &CheckedTrees,
    target: NativeTarget,
) -> Result<LayoutPlan, Diagnostic> {
    let mut builder = LayoutBuilder::new(program, target);

    for data_definition in program.data_definitions() {
        if !data_definition.type_parameters.is_empty() {
            continue;
        }
        builder.layout_data_definition(data_definition.symbol)?;
    }

    for machine in program.machines() {
        builder.layout_machine(machine.symbol)?;
    }

    Ok(builder.finish())
}

struct LayoutBuilder<'program> {
    data_definitions: &'program [DataDefinition],
    data_layouts: Arena<DataLayout>,
    data_visiting: LayoutVisitStack,
    fields: Arena<FieldLayout>,
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
    platform_definitions: &'program [Platform],
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
                    .map(|machine| {
                        program.machine_owned_data(machine).len()
                            + program.machine_contained_objects(machine).len()
                    })
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
            generic_instance_signatures: Vec::new(),
            machine_definitions,
            machine_layouts: Arena::with_capacity(machine_definitions.len()),
            machine_visiting: LayoutVisitStack::with_capacity(machine_definitions.len()),
            platform_definitions: program.platforms(),
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
            return Err(Diagnostic::error(format!(
                "recursive data layout is not supported yet for symbol {}",
                symbol.arena_index()
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
                DataMember::Field(field) => Some(field),
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
                DataMember::Field(field) => Some(field),
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
                .map(|field| {
                    let layout = self.layout_type_reference_handle(field.type_reference)?;
                    Ok(PlannedField {
                        symbol: field.symbol,
                        name: field.name.clone(),
                        type_symbol: self.program.type_reference_symbol(field.type_reference),
                        type_name: self
                            .program
                            .display_type_reference_with_constraints(field.type_reference)
                            .into(),
                        type_descriptor: self.type_descriptor(field.type_reference),
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
                    .filter(|member| matches!(member, DataMember::Field(_)))
                    .count()
            })
            .unwrap_or(0);
        let field_capacity = data_field_capacity
            .checked_add(self.program.machine_owned_data(machine).len())
            .and_then(|count| {
                count.checked_add(self.program.machine_contained_objects(machine).len())
            })
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

        for contained_object in self.program.machine_contained_objects(machine) {
            if self
                .machine_definition_by_symbol(contained_object.type_symbol)
                .is_ok()
            {
                fields.push(PlannedField {
                    symbol: contained_object.symbol,
                    name: contained_object.name.clone(),
                    type_symbol: contained_object.type_symbol,
                    type_name: contained_object.type_name.as_str().into(),
                    type_descriptor: TypeLayoutDescriptor::Named {
                        symbol: contained_object.type_symbol,
                        name: contained_object.type_name.clone(),
                    },
                    layout: self.layout_machine(contained_object.type_symbol)?,
                });
            }
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
                // instruction-selection's `descriptor_layout`.
                let has_named_domain = self
                    .program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(_)));
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
                let FixedArrayLength::Literal(length) = length else {
                    return Ok(TypeLayout::default());
                };

                Ok(TypeLayout {
                    size: element_layout.size * length,
                    alignment: element_layout.alignment,
                })
            }
            TypeReferenceNode::Slice { .. } => Ok(self.slice_layout()),
            TypeReferenceNode::DynamicTrait { .. } => Ok(fat_descriptor_layout(self.target)),
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments,
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
        arguments: omega_core::arena::HandleSpan<TypeReferenceHandle>,
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
                "recursive data layout is not supported yet for generic `{}`",
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

        if self
            .platform_definitions
            .iter()
            .any(|platform| platform.symbol == symbol)
        {
            return Ok(TypeLayout {
                size: self.target.pointer_size,
                alignment: self.target.pointer_alignment,
            });
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

        if Some(symbol) == self.program.symbols.builtin_type_symbol(BuiltinType::Real) {
            return Some(TypeLayout {
                size: 8,
                alignment: 8,
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
                let constraint_list =
                    self.program.type_reference_table.constraints(*constraints);
                // An owned `[u8; N] in <named-domain>` field is the variable-fill
                // bounded byte carrier (#66): a NAMED (text) domain over a fixed
                // array. It becomes its own `BoundedByteBuffer` descriptor --
                // `{len, bytes}` is a distinct layout from the always-full
                // `FixedArray`, and the named domain does not otherwise survive to
                // the backend (only arithmetic domains do), so the carrier needs
                // its own variant to be recognizable downstream.
                let has_named_domain = constraint_list
                    .iter()
                    .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(_)));
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
                            .unwrap_or(omega_core::arithmetic::ArithmeticDomain::Exact),
                    },
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => match length {
                FixedArrayLength::Literal(length) => TypeLayoutDescriptor::FixedArray {
                    element_type: Box::new(
                        self.type_descriptor_with_bindings(*element_type, bindings),
                    ),
                    length: *length,
                },
                // ConstCall lengths are substituted to literals by the
                // orchestration const-eval pass before layout; an unresolved
                // one degrades to Unit exactly like an unresolved const
                // parameter (layout cannot size it).
                FixedArrayLength::ConstParameter { .. } | FixedArrayLength::ConstCall { .. } => {
                    TypeLayoutDescriptor::Unit
                }
            },
            TypeReferenceNode::Slice { element_type } => TypeLayoutDescriptor::Slice {
                element_type: Box::new(
                    self.type_descriptor_with_bindings(*element_type, bindings),
                ),
            },
            TypeReferenceNode::DynamicTrait { symbol, name } => {
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
            (symbol.is_valid() && binding.parameter_symbol == symbol)
                || binding.parameter_name == name
        })
        .copied()
}
