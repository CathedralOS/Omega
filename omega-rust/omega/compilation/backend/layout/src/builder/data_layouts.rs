//! Computing the layouts of records and case-bearing data.

use crate::builder::LayoutBuilder;
use crate::builder::generic_bindings::GenericLayoutBinding;
use crate::builder::private_callback_closure::close_private_callback_demands;
use crate::builder::semantic_ranges::canonical_plan_laid_data_identity;
use crate::packing::{
    PlannedField, align_to, pack_fields, pack_fields_at, place_fields_by_plan, placement_overflow,
};
use crate::{
    BitFieldFragment, BitFieldLayout, DataLayout, DataShape, RepeatedFieldLayout,
    StoredIntegerLayout, TargetClosedPlanLaidDataLayoutIdentity, TypeLayout, VariantLayout,
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

        // A quotient retains the chosen representative without normalization:
        // the runtime layout IS the carrier's, retagged to the quotient's
        // symbol so downstream lookups keep the authored identity. Carriers
        // that are themselves proof-only never reach this arm -- the
        // classification keeps such a quotient proof-only and the plan loop
        // skips it.
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
            let geometry = plan.invariant_geometry().ok_or_else(|| {
                Diagnostic::error(format!(
                    "plan-laid data `{}` retains an unresolved target-family layout; the typed layout backend requires an invariant layout or a target-closed Terminal view",
                    plan.data_name
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
                TypeLayout {
                    size: geometry.size,
                    alignment: geometry.align,
                },
            )?;
            for bit_field in &geometry.bit_fields {
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
            for integer_field in &geometry.integer_fields {
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
            for repeated_field in &geometry.repeated_fields {
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
                    geometry
                        .private_callback_demands
                        .first()
                        .map(|demand| demand.layout_subject_identity.clone())
                })
                .or_else(|| {
                    let mut retained = self
                        .program
                        .plan_laid_layouts
                        .iter()
                        .filter(|candidate| candidate.policy_symbol == plan.policy_symbol)
                        .flat_map(|candidate| candidate.geometries(self.program))
                        .flat_map(|geometry| geometry.private_callback_demands.iter())
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
                        layout: geometry.validated_layout.clone(),
                        private_callback_demands: geometry.private_callback_demands.clone(),
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

    /// Resolve recursive geometry before deriving this field's metadata.
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
        let planned_common = self
            .program
            .live_data_fields(members)
            .map(|field| self.plan_field(field, bindings))
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let (common_fields, common_layout) =
            pack_fields_at(&mut self.fields, planned_common, TAG_LAYOUT.size)?;
        let common_end = common_layout.size.max(TAG_LAYOUT.size);

        // Plan every case's payload fields next: the shared payload base offset
        // depends on the strictest alignment across ALL cases.
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
