//! Computing the layouts of data definitions, case-bearing shapes and
//! machines.

use crate::builder::LayoutBuilder;
use crate::builder::generic_bindings::GenericLayoutBinding;
use crate::builder::private_callback_closure::close_private_callback_demands;
use crate::builder::semantic_ranges::canonical_plan_laid_data_identity;
use crate::packing::{
    PlannedField, align_to, pack_fields, pack_fields_at, place_fields_by_plan, placement_overflow,
};
use crate::{
    BitFieldFragment, BitFieldLayout, DataLayout, DataShape, MachineLayout, RepeatedFieldLayout,
    StoredIntegerLayout, TargetClosedPlanLaidDataLayoutIdentity, TypeLayout, VariantLayout,
};
use calling_conventions::{callback_layout_plan_id, callback_plan_laid_layout_id};
use checked_trees::data::{DataDefinition, DataMember, DataShapeKind};
use checked_trees::machine::Machine;
use diagnostics::Diagnostic;

impl<'program> LayoutBuilder<'program> {
    pub(crate) fn compute_data_layout(
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
            if definition.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque {
                return Err(Diagnostic::error(format!(
                    "compiler-derived placed accessor `{}` lost its opaque supply mode",
                    definition.name
                )));
            }
            return Ok(DataLayout {
                symbol: definition.symbol,
                name: definition.name.clone(),
                shape: DataShape::Record {
                    fields: arena::HandleSpan::empty(),
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
            )?;
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
            let native_layout_report_fingerprint =
                layout_plans::normalized_native_layout_plan_report_fingerprint(
                    &layout_plans::NativeLayoutPlanReport {
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
                native_layout_report_fingerprint,
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
                    canonical_layout_subject,
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
                        native_layout_report_fingerprint,
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

        let (fields, layout) = pack_fields(&mut self.fields, fields)?;

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
            pack_fields_at(&mut self.fields, planned_common, TAG_LAYOUT.size)?;
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
        let payload_base = align_to(common_end, payload_alignment).ok_or_else(|| {
            placement_overflow(format!(
                "aligning the payload base of `{}` at offset {common_end} to {payload_alignment} byte(s)",
                definition.name
            ))
        })?;

        let mut end_offset = common_end;
        let mut variant_layouts = Vec::with_capacity(planned_variants.len());
        for (symbol, name, planned) in planned_variants {
            let (fields, payload_layout) = pack_fields_at(&mut self.fields, planned, payload_base)?;
            end_offset = end_offset.max(payload_layout.size);
            variant_layouts.push(VariantLayout {
                symbol,
                name,
                fields,
            });
        }
        let variants = self.variants.insert_many(variant_layouts);
        let size = align_to(end_offset, alignment).ok_or_else(|| {
            placement_overflow(format!(
                "aligning the {end_offset}-byte extent of `{}` to {alignment} byte(s)",
                definition.name
            ))
        })?;

        Ok(DataLayout {
            symbol: definition.symbol,
            name: definition.name.clone(),
            shape: DataShape::Enum {
                common_fields,
                variants,
            },
            layout: TypeLayout { size, alignment },
        })
    }

    pub(crate) fn compute_machine_layout(
        &mut self,
        machine: &Machine,
    ) -> Result<MachineLayout, Diagnostic> {
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
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "machine `{}` field inventory overflows the compiler host",
                    machine.name
                ))
            })?;
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

        let (fields, layout) = pack_fields(&mut self.fields, fields)?;

        Ok(MachineLayout {
            symbol: machine.symbol,
            name: machine.name.clone(),
            attached_data: machine.attached_data.clone(),
            fields,
            layout,
        })
    }
}
