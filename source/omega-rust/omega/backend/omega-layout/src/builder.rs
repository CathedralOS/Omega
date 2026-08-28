use crate::packing::{PlannedField, pack_fields, pack_fields_at, place_fields_by_plan};
use crate::sizing::{
    dynamic_trait_descriptor_layout, fat_descriptor_layout, primitive_type_layout,
};
use crate::{
    BitFieldFragment, BitFieldLayout, DataLayout, DataShape, FieldLayout, LayoutPlan,
    MachineLayout, RepeatedFieldLayout, StoredIntegerLayout,
    TargetClosedPlanLaidDataLayoutIdentity, TargetClosedPrivateCallbackDemand,
    TargetClosedTwoHopPrivateCallbackPath, TypeLayout, TypeLayoutDescriptor, VariantLayout,
};
use omega_calling_conventions::{
    callback_layout_field_slot_id, callback_layout_plan_id, callback_layout_slot_id,
    callback_plan_laid_layout_id, callback_requirement_id,
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

    builder.finish()
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
    private_callback_demands: Vec<TargetClosedPrivateCallbackDemand>,
    plan_laid_layout_identities: Vec<TargetClosedPlanLaidDataLayoutIdentity>,
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
            private_callback_demands: Vec::new(),
            plan_laid_layout_identities: Vec::new(),
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
            machine_layouts: self.machine_layouts,
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
            let canonical_layout_subject = self
                .program
                .normalized_hermetic_symbol_identity(plan.policy_symbol)
                .ok()
                .or_else(|| {
                    plan.private_callback_demands
                        .first()
                        .map(|demand| demand.layout_subject_identity.clone())
                })
                .or_else(|| {
                    let mut retained = self
                        .program
                        .typed
                        .plan_laid_layouts
                        .iter()
                        .filter(|candidate| candidate.policy_symbol == plan.policy_symbol)
                        .flat_map(|candidate| candidate.private_callback_demands.iter())
                        .map(|demand| demand.layout_subject_identity.as_str())
                        .collect::<Vec<_>>();
                    retained.sort_unstable();
                    retained.dedup();
                    let [identity] = retained.as_slice() else {
                        return None;
                    };
                    Some((*identity).to_owned())
                });
            let native_layout_fingerprint =
                psi_layout_plans::normalized_native_layout_plan_fingerprint(
                    &psi_layout_plans::NativeLayoutPlanReport {
                        layout: plan.validated_layout.clone(),
                        private_callback_demands: plan.private_callback_demands.clone(),
                    },
                );
            let canonical_data_identity =
                canonical_layout_subject
                    .as_deref()
                    .and_then(|canonical_layout_subject| {
                        canonical_plan_laid_data_identity(
                            self.program,
                            plan,
                            definition,
                            canonical_layout_subject,
                        )
                    });
            let terminal_layout_identity = callback_layout_plan_id(
                native_layout_fingerprint,
                self.target.pointer_size,
                self.target.pointer_alignment,
            );
            let closed_demands = if plan.private_callback_demands.is_empty() {
                Vec::new()
            } else {
                let canonical_layout_subject =
                    canonical_layout_subject.as_deref().ok_or_else(|| {
                        Diagnostic::error(format!(
                            "plan-laid data `{}` lost its retained private-callback layout subject",
                            plan.data_name
                        ))
                    })?;
                close_private_callback_demands(
                    plan,
                    self.fields
                        .span(fields)
                        .expect("newly inserted plan-laid fields retain their span"),
                    self.target,
                    &canonical_layout_subject,
                    terminal_layout_identity,
                )?
            };
            self.private_callback_demands.extend(closed_demands);
            if let (Some(canonical_data_identity), Some(canonical_layout_subject)) =
                (canonical_data_identity, canonical_layout_subject)
            {
                let identity = TargetClosedPlanLaidDataLayoutIdentity {
                    data_symbol: definition.symbol,
                    layout: callback_plan_laid_layout_id(
                        native_layout_fingerprint,
                        &canonical_data_identity,
                        &canonical_layout_subject,
                        self.target.pointer_size,
                        self.target.pointer_alignment,
                    ),
                    data_identity: canonical_data_identity.into(),
                    layout_subject_identity: canonical_layout_subject.into(),
                    physical: layout,
                };
                if self.plan_laid_layout_identities.iter().any(|prior| {
                    prior.data_symbol == identity.data_symbol || prior.layout == identity.layout
                }) {
                    return Err(Diagnostic::error(format!(
                        "plan-laid data `{}` repeats or collides on its target-closed layout identity",
                        definition.name
                    )));
                }
                self.plan_laid_layout_identities.push(identity);
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
                referee, access, ..
            } => TypeLayoutDescriptor::Reference {
                referee: Box::new(self.type_descriptor_with_bindings(*referee, bindings)),
                is_mutable: access.is_exclusive(),
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

fn canonical_plan_laid_data_identity(
    program: &CheckedTrees,
    plan: &psi_typed_trees::typed_trees::PlanLaidLayout,
    definition: &DataDefinition,
    canonical_layout_subject: &str,
) -> Option<String> {
    program
        .normalized_hermetic_symbol_identity(definition.symbol)
        .ok()
        .or_else(|| {
            definition.generic_instance.map(|origin| {
                format!(
                    "closed-data-instance::{}",
                    program.package_qualified_type_identity(origin)
                )
            })
        })
        .or_else(|| {
            program
                .normalized_hermetic_symbol_identity(plan.schema_symbol)
                .ok()
                .map(|schema_identity| {
                    format!("closed-plan-laid-data::{canonical_layout_subject}::{schema_identity}")
                })
        })
}

fn close_two_hop_private_callback_paths(
    program: &CheckedTrees,
    data_layouts: &Arena<DataLayout>,
    fields: &Arena<FieldLayout>,
    layout_identities: &[TargetClosedPlanLaidDataLayoutIdentity],
    private_demands: &[TargetClosedPrivateCallbackDemand],
) -> Result<Vec<TargetClosedTwoHopPrivateCallbackPath>, Diagnostic> {
    let mut paths = Vec::new();
    for (root_layout_index, root_identity) in layout_identities.iter().enumerate() {
        let matching_roots = data_layouts
            .iter()
            .filter(|(_, layout)| layout.symbol == root_identity.data_symbol)
            .collect::<Vec<_>>();
        let [(_, root)] = matching_roots.as_slice() else {
            return Err(Diagnostic::error(format!(
                "target-closed plan-laid root resolves to {} exact data layouts",
                matching_roots.len()
            )));
        };
        if root.layout != root_identity.physical {
            return Err(Diagnostic::error(
                "target-closed plan-laid root changed its physical layout",
            ));
        }
        let DataShape::Record {
            fields: root_fields,
        } = root.shape
        else {
            continue;
        };
        let root_field_layouts = fields.span(root_fields).ok_or_else(|| {
            Diagnostic::error("target-closed plan-laid root retained an invalid field span")
        })?;
        for (field_ordinal, field_layout) in root_field_layouts.iter().enumerate() {
            let TypeLayoutDescriptor::Named {
                symbol: child_symbol,
                ..
            } = field_layout.type_descriptor
            else {
                continue;
            };
            if root_field_layouts
                .iter()
                .filter(|candidate| candidate.symbol == field_layout.symbol)
                .count()
                != 1
            {
                return Err(Diagnostic::error(
                    "target-closed callback path field symbol is not unique in its root record",
                ));
            }
            if field_layout.type_symbol.is_valid() && field_layout.type_symbol != child_symbol {
                return Err(Diagnostic::error(format!(
                    "target-closed callback path field changed its exact named child symbol from {:?} to {:?}",
                    field_layout.type_symbol, child_symbol
                )));
            }
            let matching_children = layout_identities
                .iter()
                .enumerate()
                .filter(|(_, identity)| identity.data_symbol == child_symbol)
                .collect::<Vec<_>>();
            let [(child_layout_index, child_identity)] = matching_children.as_slice() else {
                continue;
            };
            if root_identity.data_symbol == child_identity.data_symbol {
                return Err(Diagnostic::error(
                    "target-closed callback path cannot recursively contain its root layout",
                ));
            }
            let matching_child_data = data_layouts
                .iter()
                .filter(|(_, layout)| layout.symbol == child_symbol)
                .collect::<Vec<_>>();
            let [(_, child_data)] = matching_child_data.as_slice() else {
                return Err(Diagnostic::error(format!(
                    "target-closed callback path child resolves to {} exact data layouts",
                    matching_child_data.len()
                )));
            };
            if !matches!(child_data.shape, DataShape::Record { .. })
                || child_data.layout != child_identity.physical
                || field_layout.layout != child_identity.physical
                || field_layout.layout.alignment == 0
                || !field_layout
                    .offset
                    .is_multiple_of(field_layout.layout.alignment)
                || field_layout
                    .offset
                    .checked_add(field_layout.layout.size)
                    .is_none_or(|end| end > root_identity.physical.size)
            {
                return Err(Diagnostic::error(
                    "target-closed callback path changed its exact inline child geometry",
                ));
            }
            let field_identity = program
                .normalized_hermetic_symbol_identity(field_layout.symbol)
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "target-closed callback path cannot rederive its exact field identity: {reason}"
                    ))
                })?;
            let field_slot = callback_layout_field_slot_id(root_identity.layout, &field_identity);
            let field_index = root_fields
                .start()
                .arena_index()
                .checked_add(u32::try_from(field_ordinal).map_err(|_| {
                    Diagnostic::error("target-closed callback path field ordinal overflowed")
                })?)
                .ok_or_else(|| {
                    Diagnostic::error("target-closed callback path field handle overflowed")
                })?;
            let field =
                psi_arena::Handle::from_parts(field_index, root_fields.start().generation());
            for (terminal_demand_index, terminal_demand) in private_demands
                .iter()
                .enumerate()
                .filter(|(_, demand)| demand.data_symbol == child_symbol)
            {
                if terminal_demand.alignment == 0
                    || !terminal_demand
                        .offset
                        .is_multiple_of(terminal_demand.alignment)
                    || terminal_demand
                        .offset
                        .checked_add(terminal_demand.byte_size)
                        .is_none_or(|end| end > child_identity.physical.size)
                {
                    return Err(Diagnostic::error(
                        "target-closed callback path changed its terminal child geometry",
                    ));
                }
                let composed_offset = field_layout
                    .offset
                    .checked_add(terminal_demand.offset)
                    .ok_or_else(|| {
                        Diagnostic::error("target-closed callback path composed offset overflowed")
                    })?;
                if !composed_offset.is_multiple_of(terminal_demand.alignment)
                    || composed_offset
                        .checked_add(terminal_demand.byte_size)
                        .is_none_or(|end| end > root_identity.physical.size)
                {
                    return Err(Diagnostic::error(
                        "target-closed callback path composed range is outside or misaligned for its root layout",
                    ));
                }
                paths.push(TargetClosedTwoHopPrivateCallbackPath {
                    root_layout_index,
                    root_layout: root_identity.clone(),
                    field_symbol: field_layout.symbol,
                    field,
                    field_layout: field_layout.clone(),
                    field_identity: field_identity.clone().into(),
                    field_slot,
                    field_relative_offset: field_layout.offset,
                    field_extent: field_layout.layout.size,
                    field_alignment: field_layout.layout.alignment,
                    child_layout_index: *child_layout_index,
                    child_layout: (*child_identity).clone(),
                    terminal_demand_index,
                    terminal_demand: terminal_demand.clone(),
                    composed_offset,
                });
            }
        }
    }
    paths.sort_unstable_by_key(|path| {
        (
            path.root_layout.layout,
            path.field_slot,
            path.terminal_demand.slot,
        )
    });
    for (index, path) in paths.iter().enumerate() {
        if paths[..index].iter().any(|prior| {
            prior.root_layout.layout == path.root_layout.layout
                && prior.field_slot == path.field_slot
                && (prior.field != path.field
                    || prior.field_symbol != path.field_symbol
                    || prior.field_layout != path.field_layout
                    || prior.child_layout != path.child_layout)
        }) {
            return Err(Diagnostic::error(
                "target-closed callback path field slot collides across distinct root-to-child edges",
            ));
        }
    }
    if paths.windows(2).any(|pair| {
        pair[0].root_layout.layout == pair[1].root_layout.layout
            && pair[0].field_slot == pair[1].field_slot
            && pair[0].terminal_demand.slot == pair[1].terminal_demand.slot
    }) {
        return Err(Diagnostic::error(
            "target-closed callback path catalog repeats one exact two-hop path",
        ));
    }
    Ok(paths)
}

fn close_private_callback_demands(
    plan: &psi_typed_trees::typed_trees::PlanLaidLayout,
    fields: &[FieldLayout],
    target: NativeTarget,
    canonical_layout_subject: &str,
    layout: omega_calling_conventions::LayoutPlanId,
) -> Result<Vec<TargetClosedPrivateCallbackDemand>, Diagnostic> {
    if plan.private_callback_demands.is_empty() {
        return Ok(Vec::new());
    }
    if plan.validated_layout.size != u64::try_from(plan.size).ok()
        || plan.validated_layout.align != plan.align as u64
    {
        return Err(Diagnostic::error(format!(
            "plan-laid data `{}` changed its validated size/alignment before private callback closure",
            plan.data_name
        )));
    }
    if target.pointer_size == 0
        || target.pointer_alignment == 0
        || !target.pointer_alignment.is_power_of_two()
    {
        return Err(Diagnostic::error(format!(
            "plan-laid data `{}` cannot close private callback slots for invalid target pointer geometry {}/{}",
            plan.data_name, target.pointer_size, target.pointer_alignment
        )));
    }
    if plan.align < target.pointer_alignment || !plan.align.is_multiple_of(target.pointer_alignment)
    {
        return Err(Diagnostic::error(format!(
            "plan-laid data `{}` has alignment {}, which cannot align a {}-byte private callback slot",
            plan.data_name, plan.align, target.pointer_alignment
        )));
    }

    let semantic_ranges = plan_laid_semantic_ranges(plan, fields)?;
    let mut occupied_private = Vec::<(usize, usize, &str)>::new();
    let mut closed = Vec::with_capacity(plan.private_callback_demands.len());
    for demand in &plan.private_callback_demands {
        if demand.layout_subject_identity != canonical_layout_subject {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` private callback slot `{}` changed layout subject from `{canonical_layout_subject}` to `{}`",
                plan.data_name, demand.slot_identity, demand.layout_subject_identity
            )));
        }
        if closed
            .iter()
            .any(|prior: &TargetClosedPrivateCallbackDemand| {
                prior.slot_identity.as_ref() == demand.slot_identity
            })
        {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` repeats private callback slot `{}` during target closure",
                plan.data_name, demand.slot_identity
            )));
        }
        let offset = usize::try_from(demand.offset).map_err(|_| {
            Diagnostic::error(format!(
                "plan-laid data `{}` private callback slot `{}` offset {} cannot be represented for the selected target",
                plan.data_name, demand.slot_identity, demand.offset
            ))
        })?;
        if !offset.is_multiple_of(target.pointer_alignment) {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` private callback slot `{}` offset {} is not aligned to the selected target's {}-byte function-pointer alignment",
                plan.data_name, demand.slot_identity, offset, target.pointer_alignment
            )));
        }
        let end = offset.checked_add(target.pointer_size).ok_or_else(|| {
            Diagnostic::error(format!(
                "plan-laid data `{}` private callback slot `{}` extent overflows",
                plan.data_name, demand.slot_identity
            ))
        })?;
        if end > plan.size {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` private callback slot `{}` range {}..{} lies outside its {}-byte layout",
                plan.data_name, demand.slot_identity, offset, end, plan.size
            )));
        }
        if semantic_ranges
            .iter()
            .any(|&(start, semantic_end)| ranges_overlap(offset, end, start, semantic_end))
        {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` private callback slot `{}` range {}..{} overlaps semantic field storage",
                plan.data_name, demand.slot_identity, offset, end
            )));
        }
        if let Some((_, _, prior)) = occupied_private
            .iter()
            .find(|&&(start, prior_end, _)| ranges_overlap(offset, end, start, prior_end))
        {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` private callback slots `{prior}` and `{}` overlap",
                plan.data_name, demand.slot_identity
            )));
        }

        let slot = callback_layout_slot_id(layout, &demand.slot_identity);
        let requirement = callback_requirement_id(&demand.callback_requirement_identity);
        if let Some(prior) = closed
            .iter()
            .find(|prior: &&TargetClosedPrivateCallbackDemand| {
                prior.slot == slot && prior.slot_identity.as_ref() != demand.slot_identity
            })
        {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` private callback slots `{}` and `{}` collide on one nominal slot identity",
                plan.data_name, prior.slot_identity, demand.slot_identity
            )));
        }
        if let Some(prior) = closed
            .iter()
            .find(|prior: &&TargetClosedPrivateCallbackDemand| {
                prior.requirement == requirement
                    && prior.callback_requirement_identity.as_ref()
                        != demand.callback_requirement_identity
            })
        {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` callback requirements `{}` and `{}` collide on one nominal requirement identity",
                plan.data_name,
                prior.callback_requirement_identity,
                demand.callback_requirement_identity
            )));
        }

        occupied_private.push((offset, end, demand.slot_identity.as_str()));
        closed.push(TargetClosedPrivateCallbackDemand {
            data_symbol: plan.data_symbol,
            slot_identity: demand.slot_identity.clone().into(),
            layout_subject_identity: demand.layout_subject_identity.clone().into(),
            callback_requirement_identity: demand.callback_requirement_identity.clone().into(),
            layout,
            slot,
            requirement,
            offset,
            byte_size: target.pointer_size,
            alignment: target.pointer_alignment,
        });
    }
    closed.sort_unstable_by(|left, right| left.slot_identity.cmp(&right.slot_identity));
    Ok(closed)
}

fn plan_laid_semantic_ranges(
    plan: &psi_typed_trees::typed_trees::PlanLaidLayout,
    fields: &[FieldLayout],
) -> Result<Vec<(usize, usize)>, Diagnostic> {
    let mut ranges = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        if let Some(bit_field) = plan
            .bit_fields
            .iter()
            .find(|candidate| candidate.field_index == index)
        {
            for fragment in &bit_field.fragments {
                let byte_size = usize::from(fragment.container_width_bits).div_ceil(8);
                push_semantic_range(
                    plan,
                    field.name.as_str(),
                    fragment.container_byte_offset,
                    byte_size,
                    &mut ranges,
                )?;
            }
            continue;
        }
        if let Some(integer_field) = plan
            .integer_fields
            .iter()
            .find(|candidate| candidate.field_index == index)
        {
            push_semantic_range(
                plan,
                field.name.as_str(),
                field.offset,
                usize::from(integer_field.stored_width_bits).div_ceil(8),
                &mut ranges,
            )?;
            continue;
        }
        if let Some(repeated_field) = plan
            .repeated_fields
            .iter()
            .find(|candidate| candidate.field_index == index)
        {
            let Some((_, length)) = field.type_descriptor.fixed_array() else {
                return Err(Diagnostic::error(format!(
                    "plan-laid data `{}` repeated field `{}` lost its fixed-array shape during private callback closure",
                    plan.data_name, field.name
                )));
            };
            if length == 0 || !field.layout.size.is_multiple_of(length) {
                return Err(Diagnostic::error(format!(
                    "plan-laid data `{}` repeated field `{}` has invalid target element geometry",
                    plan.data_name, field.name
                )));
            }
            let element_size = field.layout.size / length;
            // Repeated placement owns each compiler-derived element extent,
            // not the whole stride. The already-validated inter-element
            // padding is intentionally available to a private demand; an
            // ordinary one-entry aggregate `At` row instead reaches the
            // whole-field arm below and occupies its complete layout size.
            for element in 0..length {
                let start = field
                    .offset
                    .checked_add(
                        element
                            .checked_mul(repeated_field.element_stride)
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "repeated field callback-closure offset overflows",
                                )
                            })?,
                    )
                    .ok_or_else(|| {
                        Diagnostic::error("repeated field callback-closure offset overflows")
                    })?;
                push_semantic_range(plan, field.name.as_str(), start, element_size, &mut ranges)?;
            }
            continue;
        }
        push_semantic_range(
            plan,
            field.name.as_str(),
            field.offset,
            field.layout.size,
            &mut ranges,
        )?;
    }
    Ok(ranges)
}

fn push_semantic_range(
    plan: &psi_typed_trees::typed_trees::PlanLaidLayout,
    field: &str,
    start: usize,
    byte_size: usize,
    ranges: &mut Vec<(usize, usize)>,
) -> Result<(), Diagnostic> {
    if byte_size == 0 {
        return Ok(());
    }
    let end = start.checked_add(byte_size).ok_or_else(|| {
        Diagnostic::error(format!(
            "plan-laid data `{}` semantic field `{field}` extent overflows during private callback closure",
            plan.data_name
        ))
    })?;
    if end > plan.size {
        return Err(Diagnostic::error(format!(
            "plan-laid data `{}` semantic field `{field}` range {start}..{end} lies outside its {}-byte layout during private callback closure",
            plan.data_name, plan.size
        )));
    }
    ranges.push((start, end));
    Ok(())
}

fn ranges_overlap(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    left_start < right_end && right_start < left_end
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
    use crate::{DataShape, FieldLayout, TypeLayout};
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

    fn private_callback_layout(
        offset: u64,
        size: usize,
    ) -> psi_typed_trees::typed_trees::PlanLaidLayout {
        psi_typed_trees::typed_trees::PlanLaidLayout {
            data_name: "Spread<ForeignRecord>".to_owned(),
            data_symbol: psi_symbols::SymbolHandle::from_arena_index(1),
            field_symbols: vec![psi_symbols::SymbolHandle::from_arena_index(2)],
            schema_symbol: psi_symbols::SymbolHandle::from_arena_index(3),
            schema_field_symbols: vec![psi_symbols::SymbolHandle::from_arena_index(4)],
            policy_symbol: psi_symbols::SymbolHandle::from_arena_index(5),
            policy_plan_machine_symbol: psi_symbols::SymbolHandle::from_arena_index(6),
            validated_layout: psi_layout_plans::LayoutPlanReport {
                schema_identity: 0x51,
                entries: vec![psi_layout_plans::LayoutFieldEntryReport {
                    field: "payload".to_owned(),
                    member_identity: None,
                    placement: psi_layout_plans::LayoutPlacementReport::At { offset: 0 },
                }],
                offsets: Some(vec![0]),
                size: Some(size as u64),
                align: 8,
            },
            private_callback_demands: vec![psi_layout_plans::PrivateCallbackLayoutDemandReport {
                slot_identity: "package::WndClassWindowProcedureSlot#exact".to_owned(),
                layout_subject_identity: "package::Spread".to_owned(),
                callback_requirement_identity: "package::WindowProcedure::call#exact".to_owned(),
                offset,
            }],
            offsets: vec![0],
            bit_fields: Vec::new(),
            integer_fields: Vec::new(),
            repeated_fields: Vec::new(),
            size,
            align: 8,
        }
    }

    fn private_callback_fields() -> [FieldLayout; 1] {
        [FieldLayout {
            symbol: psi_symbols::SymbolHandle::from_arena_index(2),
            name: psi_checked_trees::name::Identifier::from("payload"),
            offset: 0,
            layout: TypeLayout {
                size: 8,
                alignment: 8,
            },
            ..FieldLayout::default()
        }]
    }

    fn close_private_callback_demands(
        plan: &psi_typed_trees::typed_trees::PlanLaidLayout,
        fields: &[FieldLayout],
        target: NativeTarget,
        canonical_layout_subject: &str,
    ) -> Result<Vec<crate::TargetClosedPrivateCallbackDemand>, psi_diagnostics::Diagnostic> {
        let fingerprint = psi_layout_plans::normalized_native_layout_plan_fingerprint(
            &psi_layout_plans::NativeLayoutPlanReport {
                layout: plan.validated_layout.clone(),
                private_callback_demands: plan.private_callback_demands.clone(),
            },
        );
        let layout = omega_calling_conventions::callback_layout_plan_id(
            fingerprint,
            target.pointer_size,
            target.pointer_alignment,
        );
        super::close_private_callback_demands(
            plan,
            fields,
            target,
            canonical_layout_subject,
            layout,
        )
    }

    #[test]
    fn target_closes_private_callback_geometry_and_rejects_exact_mutations() {
        let target = NativeTarget::windows_x64();
        let fields = private_callback_fields();
        let valid = private_callback_layout(8, 16);
        let [closed] = close_private_callback_demands(&valid, &fields, target, "package::Spread")
            .expect("aligned private callback padding should close")
            .try_into()
            .expect("one private callback demand");
        assert_eq!(
            (closed.offset, closed.byte_size, closed.alignment),
            (8, 8, 8)
        );
        assert_eq!(
            closed.requirement,
            omega_calling_conventions::callback_requirement_id(
                "package::WindowProcedure::call#exact"
            )
        );

        let moved = close_private_callback_demands(
            &private_callback_layout(16, 24),
            &fields,
            target,
            "package::Spread",
        )
        .expect("another valid offset should close");
        assert_ne!(closed.layout, moved[0].layout);
        assert_ne!(closed.slot, moved[0].slot);

        let unaligned = close_private_callback_demands(
            &private_callback_layout(9, 24),
            &fields,
            target,
            "package::Spread",
        )
        .expect_err("unaligned callback slot must reject");
        assert!(unaligned.message.contains("is not aligned"));

        let outside = close_private_callback_demands(
            &private_callback_layout(16, 16),
            &fields,
            target,
            "package::Spread",
        )
        .expect_err("out-of-bounds callback slot must reject");
        assert!(outside.message.contains("lies outside its 16-byte layout"));

        let semantic_overlap = close_private_callback_demands(
            &private_callback_layout(0, 16),
            &fields,
            target,
            "package::Spread",
        )
        .expect_err("semantic/private overlap must reject");
        assert!(
            semantic_overlap
                .message
                .contains("overlaps semantic field storage")
        );

        let mut private_overlap = valid;
        let mut second = private_overlap.private_callback_demands[0].clone();
        second.slot_identity.push_str("::Second");
        private_overlap.private_callback_demands.push(second);
        let private_overlap =
            close_private_callback_demands(&private_overlap, &fields, target, "package::Spread")
                .expect_err("private/private overlap must reject");
        assert!(
            private_overlap.message.contains("private callback slots")
                && private_overlap.message.contains("overlap")
        );

        let subject_tamper = close_private_callback_demands(
            &private_callback_layout(8, 16),
            &fields,
            target,
            "package::OtherLayout",
        )
        .expect_err("layout-subject substitution must reject");
        assert!(subject_tamper.message.contains("changed layout subject"));

        let mut duplicate_slot = private_callback_layout(8, 24);
        let mut conflicting = duplicate_slot.private_callback_demands[0].clone();
        conflicting
            .callback_requirement_identity
            .push_str("::Other");
        conflicting.offset = 16;
        duplicate_slot.private_callback_demands.push(conflicting);
        let duplicate_slot =
            close_private_callback_demands(&duplicate_slot, &fields, target, "package::Spread")
                .expect_err("one canonical slot cannot close under conflicting requirements");
        assert!(
            duplicate_slot
                .message
                .contains("repeats private callback slot")
        );
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
