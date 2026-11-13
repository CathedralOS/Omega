/*
We compute physical layouts for record and case-bearing (`Enum` and `Mixed`)
data declarations (`compute_data_layout`).

Compiler-derived accessors retain their exact field address as a pointer-sized
record rather than collapsing to zero-sized types, and quotient data types
retain their representative carrier's runtime layout retagged to the quotient's
symbol. Plan-laid records transcribe validated field offsets, bit-field
fragments, stored-integer widths, repeated-field strides, and target-closed
private callback demands from the selected target's `PlanLaidGeometry`.

Case-bearing shapes (pure sums and mixed common-field + case shapes) lay out as
a tag-prefixed overlay without niche packing: the `i32` tag sits at constant
offset 0, common fields pack immediately after the tag, and every case's
payload fields pack from a shared base offset aligned to the strictest payload
alignment across all cases.
*/

use crate::data_layout_queries::builder::LayoutBuilder;
use crate::data_layout_queries::builder::generic_bindings::GenericLayoutBinding;
use crate::data_layout_queries::builder::private_callback_closure::close_private_callback_demands;
use crate::data_layout_queries::packing::{
    PlannedField, align_to, pack_fields, pack_fields_at, place_fields_by_plan, placement_overflow,
};
use crate::data_layout_queries::sizing::pointer_layout;
use crate::data_layout_queries::{
    BitFieldFragment, BitFieldLayout, DataLayout, DataShape, RepeatedFieldLayout,
    StoredIntegerLayout, TargetClosedDataLayoutIdentity, TypeLayout, VariantLayout,
};
use calling_conventions::{callback_layout_plan_id, callback_plan_laid_layout_id};
use diagnostics::Diagnostic;
use typed_trees::data::{DataDefinition, DataField, DataMember, DataShapeKind};

impl<'program> LayoutBuilder<'program> {
    pub(crate) fn compute_data_layout(
        &mut self,
        definition: &DataDefinition,
        bindings: &[GenericLayoutBinding<'program>],
    ) -> Result<DataLayout, Diagnostic> {
        if self.is_compiler_accessor_definition(definition) {
            if definition.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque {
                return Err(Diagnostic::error(format!(
                    "compiler-derived accessor `{}` lost its opaque supply mode",
                    definition.name
                )));
            }
            return Ok(DataLayout {
                symbol: definition.symbol,
                name: definition.name.clone(),
                shape: DataShape::Record {
                    fields: arena::HandleSpan::empty(),
                },
                layout: pointer_layout(self.target),
            });
        }

        if let Some(quotient) = definition.quotient.as_ref() {
            let layout = self.layout_type_reference_handle(quotient.carrier, bindings)?;
            let carrier_symbol = self.program.type_reference_symbol(quotient.carrier);
            let shape = self
                .data_layouts
                .iter()
                .find(|(_, candidate)| {
                    carrier_symbol.is_valid() && candidate.symbol == carrier_symbol
                })
                .map(|(_, candidate)| candidate.shape.clone())
                .unwrap_or_default();
            return Ok(DataLayout {
                symbol: definition.symbol,
                name: definition.name.clone(),
                shape,
                layout,
            });
        }

        let members = self.program.data_members(definition);
        if matches!(
            DataDefinition::shape_kind_from_members(members),
            DataShapeKind::Enum | DataShapeKind::Mixed
        ) {
            return self.compute_case_bearing_layout(definition, members, bindings);
        }

        let fields = self
            .program
            .live_data_fields(members)
            .map(|field| self.plan_field(field, bindings))
            .collect::<Result<Vec<_>, Diagnostic>>()?;

        if let Some(plan) = self.program.plan_laid_layout_for_data(definition) {
            let geometry = plan
                .geometry_for_target(
                    self.program,
                    self.layout_profile.map(target::TargetProfile::target_name),
                )
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "fixed representation data `{}` has no admitted layout for typed layout target `{}`",
                        plan.data_name,
                        self.layout_profile
                            .map(target::TargetProfile::target_name)
                            .unwrap_or("<unselected>")
                    ))
                })?;
            if geometry.offsets.len() != fields.len()
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
                    geometry.offsets.len()
                )));
            }
            let (fields, layout) = place_fields_by_plan(
                &mut self.fields,
                fields,
                &geometry.offsets,
                crate::data_layout_queries::type_layout(geometry.size, geometry.align),
            )?;
            let placed = self
                .fields
                .span(fields)
                .expect("newly inserted plan-laid fields retain their span");
            let field_at = |index: usize, placement: &str| {
                placed.get(index).map(|field| field.symbol).ok_or_else(|| {
                    Diagnostic::error(format!(
                        "plan-laid data `{}` has no field at {placement} index {index}",
                        definition.name
                    ))
                })
            };
            for bit_field in &geometry.bit_fields {
                let field = field_at(bit_field.field_index, "bit-placement")?;
                self.bit_fields.push(BitFieldLayout {
                    field,
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
            for integer_field in &geometry.integer_fields {
                let field = field_at(integer_field.field_index, "stored-integer")?;
                self.stored_integers.push(StoredIntegerLayout {
                    field,
                    stored_width_bits: integer_field.stored_width_bits,
                    interpretation: integer_field.interpretation,
                    write_is_total: integer_field.write_is_total,
                });
            }
            for repeated_field in &geometry.repeated_fields {
                let field = field_at(repeated_field.field_index, "repeated-placement")?;
                self.repeated_fields.push(RepeatedFieldLayout {
                    field,
                    element_stride: repeated_field.element_stride,
                });
            }
            let canonical_layout_subject = plan.native_policy_identity(self.program).ok();
            let native_layout_report_fingerprint =
                layout_plans::normalized_native_layout_plan_report_fingerprint(
                    &geometry.native_layout_report(),
                );
            let canonical_data_identity =
                canonical_layout_subject
                    .as_deref()
                    .and_then(|canonical_layout_subject| {
                        plan.native_data_identity(
                            self.program,
                            definition,
                            canonical_layout_subject,
                        )
                        .ok()
                    });
            let terminal_layout_identity = callback_layout_plan_id(
                native_layout_report_fingerprint,
                self.target.pointer_size,
                self.target.pointer_alignment,
            );
            let closed_demands = if geometry.private_callback_demands.is_empty() {
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
                    geometry,
                    placed,
                    self.target,
                    canonical_layout_subject,
                    terminal_layout_identity,
                )?
            };
            self.private_callback_demands.extend(closed_demands);
            if let (Some(canonical_data_identity), Some(canonical_layout_subject)) =
                (canonical_data_identity, canonical_layout_subject)
            {
                let identity = TargetClosedDataLayoutIdentity {
                    data_symbol: definition.symbol,
                    layout: callback_plan_laid_layout_id(
                        native_layout_report_fingerprint,
                        &canonical_data_identity,
                        &canonical_layout_subject,
                        self.target.pointer_size,
                        self.target.pointer_alignment,
                    ),
                    data_identity: canonical_data_identity.into(),
                    layout_subject_identity: Some(canonical_layout_subject.into()),
                    physical: layout,
                };
                if self.data_layout_identities.iter().any(|prior| {
                    prior.data_symbol == identity.data_symbol || prior.layout == identity.layout
                }) {
                    return Err(Diagnostic::error(format!(
                        "plan-laid data `{}` repeats or collides on its target-closed layout identity",
                        definition.name
                    )));
                }
                self.data_layout_identities.push(identity);
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

    fn plan_field(
        &mut self,
        field: &DataField,
        bindings: &[GenericLayoutBinding<'program>],
    ) -> Result<PlannedField, Diagnostic> {
        let layout = self.layout_type_reference_handle(field.type_reference, bindings)?;
        Ok(PlannedField {
            symbol: field.symbol,
            name: field.name.clone(),
            type_symbol: self.program.type_reference_symbol(field.type_reference),
            type_name: self
                .program
                .display_type_reference_with_constraints(field.type_reference)
                .into(),
            type_descriptor: self.type_descriptor(field.type_reference, bindings),
            layout,
        })
    }

    fn compute_case_bearing_layout(
        &mut self,
        definition: &DataDefinition,
        members: &[DataMember],
        bindings: &[GenericLayoutBinding<'program>],
    ) -> Result<DataLayout, Diagnostic> {
        const TAG_LAYOUT: TypeLayout = crate::data_layout_queries::type_layout(
            crate::data_layout_queries::ENUM_TAG_BYTES,
            crate::data_layout_queries::ENUM_TAG_BYTES,
        );

        let planned_common = self
            .program
            .live_data_fields(members)
            .map(|field| self.plan_field(field, bindings))
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let (common_fields, common_layout) =
            pack_fields_at(&mut self.fields, planned_common, TAG_LAYOUT.size)?;
        let common_end = common_layout.size.max(TAG_LAYOUT.size);

        let mut planned_variants = Vec::new();
        let mut payload_alignment = 1usize;
        for member in members {
            let Some(variant) = member.as_variant() else {
                continue;
            };
            let planned = self
                .program
                .data_payload_fields(variant)
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| self.plan_field(field, bindings))
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
}
