//! PLAN-LAID VALUE TYPES -- L4 of the LAYOUTS ladder
//! (design_briefs/programmable_layouts.md §5): a field `gdt: CLayout<Gdt>;`
//! applies a layout POLICY (ordinary data with a build-time-admissible `plan` machine)
//! to a SCHEMA (a plain record of primitives) in type position. The value
//! behaves exactly like the schema type -- same fields, ZII, projections --
//! but its native in-memory placement comes from the validated plan instead
//! of the compiler's own packing.
//!
//! Two passes, both driven from the pipeline:
//!
//! 1. `desugar_plan_laid_value_types` (PRE-RESOLUTION, on the merged syntax
//!    trees): synthesizes `data CLayout<Gdt> { <schema fields> }` and rewrites
//!    every occurrence of the generic spelling to that plain name, so fields,
//!    parameters, returns, locals, nested generic arguments, symbol resolution,
//!    typing, validation, proof, and the interpreter all see one ordinary
//!    record identity. The interpreter is name-keyed, so it needs nothing else.
//! 2. `compute_plan_laid_layouts` (POST-TYPING, after const-length
//!    substitution): evaluates the policy at build time through the existing
//!    L2/L3 pipeline (`compute_layout_plan` -- contract gate, plan validation),
//!    requires the plan be FULLY STATIC (a dynamic plan cannot be a value
//!    type: values need offsets, bytes need mints), and records the placement
//!    on `TypedTrees::plan_laid_layouts` for the native layout builder.
//!
//! v0 boundaries (documented, all clean errors): schemas are plain records of
//! primitives; construction is ZII + per-field writes (a
//! `CLayout<Gdt> { ... }` literal is not spellable).

use psi_arena::{Handle, HandleSpan};
use psi_diagnostics::Diagnostic;
use psi_layout_plans::LayoutPlacementReport;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{DataDefinition, DataMember, DataProperties, Item};
use psi_syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use psi_typed_trees::{
    PlanLaidBitField, PlanLaidBitFragment, PlanLaidIntegerField, PlanLaidLayout,
    PlanLaidRepeatedField, TypedTrees,
};
use std::collections::{HashMap, HashSet};

/// One plan-laid instantiation discovered by the desugar: the synthesized
/// data definition plus the (policy, schema) pair whose validated plan will
/// dictate its layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanLaidRecord {
    /// Name of the synthesized data definition (`CLayout<GdtEntryish>`).
    pub synthetic_name: String,
    /// Qualified policy machine (`CLayout::plan`).
    pub policy_machine: String,
    /// The schema data definition the plan places.
    pub schema_data: String,
}

struct IndexedData {
    has_type_parameters: bool,
    supply_mode: psi_language_semantics::DataSupplyMode,
    lifetime_parameters: Vec<Identifier>,
    members: HandleSpan<DataMember>,
    properties: DataProperties,
}

struct PendingRewrite {
    type_reference: TypeReferenceHandle,
    synthetic_name: String,
}

/// Find `Policy<Schema>` type applications where `Policy` is a non-generic
/// data definition with an attached `plan` machine, synthesize the instance
/// definitions, and rewrite every occurrence. Returns the records the
/// post-typing plan pass needs.
pub fn desugar_plan_laid_value_types(
    syntax: &mut SyntaxTrees,
) -> Result<Vec<PlanLaidRecord>, Vec<Diagnostic>> {
    // Index the merged program: data definitions by name, and the set of data
    // names carrying an attached `plan` machine (the structural v0 policy
    // gate; the Layout-trait signature contract tightens this later).
    let mut data_index: HashMap<String, IndexedData> = HashMap::new();
    let mut plan_policies: HashSet<String> = HashSet::new();
    for item in syntax.root_items() {
        match item {
            Item::Data(definition) => {
                data_index.insert(
                    definition.name.as_str().to_string(),
                    IndexedData {
                        has_type_parameters: !definition.type_parameters.is_empty()
                            || !definition.lifetime_parameters.is_empty(),
                        supply_mode: definition.supply_mode,
                        lifetime_parameters: definition.lifetime_parameters.clone(),
                        members: definition.members,
                        properties: definition.properties,
                    },
                );
            }
            Item::Machine(machine) => {
                if let Some(attached) = &machine.attached_data
                    && machine.name.as_str() == format!("{}::plan", attached.as_str())
                {
                    plan_policies.insert(attached.as_str().to_string());
                }
            }
            _ => {}
        }
    }

    // Scan the TYPE TABLE rather than only data fields. A plan-laid
    // application is one semantic value type wherever it occurs; relying on a
    // field and a parameter spelling to share an arena handle made that
    // identity an accidental parser-allocation property. Collection only --
    // mutation happens after the scan so the borrows stay simple.
    let mut diagnostics = Vec::new();
    let mut rewrites: Vec<PendingRewrite> = Vec::new();
    let mut records: Vec<PlanLaidRecord> = Vec::new();
    for type_reference in syntax.tables.type_references.generic_nodes() {
        let TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } = syntax.tables.type_references.type_reference(type_reference)
        else {
            unreachable!("generic_nodes returned a non-generic type reference");
        };
        if !lifetime_arguments.is_empty() {
            continue;
        }
        let base = base_name.as_str();
        let Some(base_info) = data_index.get(base) else {
            continue; // unknown base: existing generic-type paths diagnose
        };
        if base_info.has_type_parameters {
            continue; // a genuine generic data definition, not a policy
        }
        if !plan_policies.contains(base) {
            diagnostics.push(Diagnostic::error(format!(
                "data `{base}` takes no type parameters and has no attached `plan` \
                 machine, so `{base}<...>` is neither a generic instantiation nor a \
                 layout-policy application"
            )));
            continue;
        }

        let argument_handles = syntax
            .tables
            .type_references
            .type_reference_handles(*arguments);
        let schema_name = match argument_handles {
            [only] => match syntax.tables.type_references.type_reference(*only) {
                TypeReferenceNode::Named(name) => name.as_str().to_string(),
                _ => {
                    diagnostics.push(Diagnostic::error(format!(
                        "layout policy `{base}` must be applied to a plain data name \
                         (`{base}<Schema>`)"
                    )));
                    continue;
                }
            },
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "layout policy `{base}` takes exactly one schema argument"
                )));
                continue;
            }
        };
        let Some(schema_info) = data_index.get(&schema_name) else {
            diagnostics.push(Diagnostic::error(format!(
                "layout policy `{base}` is applied to `{schema_name}`, but no data \
                 definition with that name exists"
            )));
            continue;
        };
        if schema_info.has_type_parameters {
            diagnostics.push(Diagnostic::error(format!(
                "layout policy `{base}` cannot be applied to generic data `{schema_name}`"
            )));
            continue;
        }
        if schema_info.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque {
            diagnostics.push(Diagnostic::error(format!(
                "layout policy `{base}` cannot inspect opaque boundary data `{schema_name}`"
            )));
            continue;
        }
        let schema_members = syntax.tables.items.data_members(schema_info.members);
        if schema_members.is_empty()
            || !schema_members
                .iter()
                .all(|member| matches!(member, DataMember::Field(_)))
        {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid schema `{schema_name}` must be a plain record with at least \
                 one field (no cases or version blocks)"
            )));
            continue;
        }

        let synthetic_name = format!("{base}<{schema_name}>");
        rewrites.push(PendingRewrite {
            type_reference,
            synthetic_name: synthetic_name.clone(),
        });
        if !records
            .iter()
            .any(|record| record.synthetic_name == synthetic_name)
        {
            records.push(PlanLaidRecord {
                synthetic_name,
                policy_machine: format!("{base}::plan"),
                schema_data: schema_name,
            });
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    // Synthesize one instance definition per distinct application: the
    // schema's members cloned under the instance name (member records share
    // type/expression handles with the schema -- same table, same program).
    for record in &records {
        let schema_info = &data_index[&record.schema_data];
        let members: Vec<DataMember> = syntax
            .tables
            .items
            .data_members(schema_info.members)
            .to_vec();
        let mut first: Handle<DataMember> = Handle::invalid();
        let mut count = 0u32;
        for member in members {
            let handle = syntax.tables.items.append_data_member(member);
            if count == 0 {
                first = handle;
            }
            count += 1;
        }
        syntax.push_root_item(Item::Data(DataDefinition {
            name: Identifier::generated(record.synthetic_name.as_str()),
            supply_mode: psi_language_semantics::DataSupplyMode::CheckedShape,
            lifetime_parameters: schema_info.lifetime_parameters.clone(),
            type_parameters: HandleSpan::default(),
            properties: schema_info.properties,
            where_facts: psi_arena::HandleSpan::empty(),
            members: HandleSpan::from_parts(first, count),
            quotient: None,
        }));
    }

    // Rewrite the spellings to the synthesized instances' plain names.
    for rewrite in rewrites {
        syntax.tables.type_references.replace_type_reference(
            rewrite.type_reference,
            TypeReferenceNode::Named(Identifier::generated(rewrite.synthetic_name)),
        );
    }

    Ok(records)
}

/// Evaluate + validate each discovered policy application (the L2/L3
/// pipeline), require a fully static plan, and record the placements for the
/// native layout builder.
pub fn compute_plan_laid_layouts(
    typed: &mut TypedTrees,
    records: &[PlanLaidRecord],
) -> Result<(), Vec<Diagnostic>> {
    if records.is_empty() {
        return Ok(());
    }

    let mut layouts = Vec::with_capacity(records.len());
    for record in records {
        let report = crate::compute_layout_plan(typed, &record.policy_machine, &record.schema_data)
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}`: {reason}",
                    record.synthetic_name
                ))]
            })?;
        let Some(size) = report.size else {
            return Err(vec![Diagnostic::error(format!(
                "plan-laid value type `{}`: policy `{}` produced a dynamic plan; a dynamic \
                 plan cannot be a value type -- values need offsets, bytes need mints",
                record.synthetic_name, record.policy_machine
            ))]);
        };

        let policy_name = record
            .policy_machine
            .strip_suffix("::plan")
            .unwrap_or(&record.policy_machine);
        let policy_symbol = typed
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == policy_name)
            .map(|data| data.symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}` lost its exact nominal policy identity",
                    record.synthetic_name
                ))]
            })?;
        let policy_plan_machine_symbol = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == record.policy_machine)
            .map(|machine| machine.symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}` lost its exact policy plan machine",
                    record.synthetic_name
                ))]
            })?;

        let synthesized_data = typed
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == record.synthetic_name)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}` lost its exact synthesized data identity",
                    record.synthetic_name
                ))]
            })?;
        let data_symbol = synthesized_data.symbol;
        let field_symbols = typed
            .data_members(synthesized_data)
            .iter()
            .filter_map(|member| match member {
                psi_typed_trees::data::DataMember::Field(field) if !field.relevance.is_erased() => {
                    Some(field.symbol)
                }
                psi_typed_trees::data::DataMember::Field(_)
                | psi_typed_trees::data::DataMember::Variant(_) => None,
            })
            .collect::<Vec<_>>();

        let schema = typed
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == record.schema_data)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}` lost its exact source schema identity",
                    record.synthetic_name
                ))]
            })?;
        let schema_symbol = schema.symbol;
        let schema_fields = typed
            .data_members(schema)
            .iter()
            .filter_map(|member| match member {
                psi_typed_trees::data::DataMember::Field(field) => (!field.relevance.is_erased())
                    .then_some((field.name.as_str().to_owned(), field.type_reference)),
                psi_typed_trees::data::DataMember::Variant(_) => None,
            })
            .collect::<Vec<_>>();
        let schema_field_symbols = typed
            .data_members(schema)
            .iter()
            .filter_map(|member| match member {
                psi_typed_trees::data::DataMember::Field(field) if !field.relevance.is_erased() => {
                    Some(field.symbol)
                }
                psi_typed_trees::data::DataMember::Field(_)
                | psi_typed_trees::data::DataMember::Variant(_) => None,
            })
            .collect::<Vec<_>>();
        let field_count = schema_fields.len();

        let mut offsets = vec![None; field_count];
        let mut bit_fields = Vec::<PlanLaidBitField>::new();
        let mut integer_fields = Vec::<PlanLaidIntegerField>::new();
        let mut repeated_fields = Vec::<PlanLaidRepeatedField>::new();
        for (field_index, (field_name, field_type)) in schema_fields.iter().enumerate() {
            let field_entries = report
                .entries
                .iter()
                .filter(|entry| entry.field == *field_name)
                .collect::<Vec<_>>();
            match field_entries.as_slice() {
                [entry] if matches!(entry.placement, LayoutPlacementReport::At { .. }) => {
                    let LayoutPlacementReport::At { offset } = entry.placement else {
                        unreachable!()
                    };
                    offsets[field_index] = Some(usize::try_from(offset).map_err(|_| {
                        vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: byte offset {offset} cannot be represented on this compiler host",
                            record.synthetic_name
                        ))]
                    })?);
                }
                [entry] if matches!(entry.placement, LayoutPlacementReport::IntegerAt { .. }) => {
                    let LayoutPlacementReport::IntegerAt {
                        offset,
                        stored_width,
                        interpretation,
                    } = entry.placement
                    else {
                        unreachable!()
                    };
                    offsets[field_index] = Some(usize::try_from(offset).map_err(|_| {
                        vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: stored-integer byte offset {offset} cannot be represented on this compiler host",
                            record.synthetic_name
                        ))]
                    })?);
                    integer_fields.push(PlanLaidIntegerField {
                        field_index,
                        stored_width_bits: u16::try_from(stored_width).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: stored-integer width {stored_width} exceeds the backend width vocabulary",
                                record.synthetic_name
                            ))]
                        })?,
                        interpretation,
                        write_is_total: stored_integer_write_is_total(
                            typed,
                            *field_type,
                            stored_width,
                            interpretation,
                        ),
                    });
                }
                entries
                    if entries.len() > 1
                        && entries.iter().all(|entry| {
                            matches!(entry.placement, LayoutPlacementReport::At { .. })
                        }) =>
                {
                    let psi_typed_trees::types::TypeReferenceNode::FixedArray {
                        length: psi_typed_trees::types::FixedArrayLength::Literal(element_count),
                        ..
                    } = typed.type_reference_table.type_reference(*field_type)
                    else {
                        return Err(vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: field `{field_name}` has repeated byte placements but is not a literal outer fixed array",
                            record.synthetic_name
                        ))]);
                    };
                    if entries.len() != *element_count {
                        return Err(vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: field `{field_name}` has {} element placements but its outer fixed array has {element_count} elements",
                            record.synthetic_name,
                            entries.len()
                        ))]);
                    }
                    let mut element_offsets = entries
                        .iter()
                        .map(|entry| match entry.placement {
                            LayoutPlacementReport::At { offset } => offset,
                            _ => unreachable!("repeated placements were filtered to At"),
                        })
                        .collect::<Vec<_>>();
                    element_offsets.sort_unstable();
                    let stride = element_offsets[1] - element_offsets[0];
                    if stride == 0
                        || element_offsets
                            .windows(2)
                            .any(|pair| pair[1] - pair[0] != stride)
                    {
                        return Err(vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: field `{field_name}` does not retain one positive constant destination stride",
                            record.synthetic_name
                        ))]);
                    }
                    offsets[field_index] = Some(
                        usize::try_from(element_offsets[0]).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: repeated field offset {} cannot be represented on this compiler host",
                                record.synthetic_name, element_offsets[0]
                            ))]
                        })?,
                    );
                    repeated_fields.push(PlanLaidRepeatedField {
                        field_index,
                        element_stride: usize::try_from(stride).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: repeated field stride {stride} cannot be represented on this compiler host",
                                record.synthetic_name
                            ))]
                        })?,
                    });
                }
                entries
                    if entries.iter().all(|entry| {
                        matches!(entry.placement, LayoutPlacementReport::Bits { .. })
                    }) =>
                {
                    let mut fragments = Vec::with_capacity(entries.len());
                    for entry in entries {
                        let LayoutPlacementReport::Bits {
                            container,
                            container_width,
                            destination_lsb,
                            source_lsb,
                            width,
                        } = entry.placement
                        else {
                            unreachable!()
                        };
                        let fragment = PlanLaidBitFragment {
                            container_byte_offset: usize::try_from(container).map_err(|_| {
                                vec![Diagnostic::error(format!(
                                    "plan-laid value type `{}`: bit-container offset {container} cannot be represented on this compiler host",
                                    record.synthetic_name
                                ))]
                            })?,
                            container_width_bits: u16::try_from(container_width).map_err(|_| {
                                vec![Diagnostic::error(format!(
                                    "plan-laid value type `{}`: bit-container width {container_width} exceeds the backend width vocabulary",
                                    record.synthetic_name
                                ))]
                            })?,
                            destination_lsb: u16::try_from(destination_lsb).map_err(|_| {
                                vec![Diagnostic::error(format!(
                                    "plan-laid value type `{}`: destination bit {destination_lsb} exceeds the backend width vocabulary",
                                    record.synthetic_name
                                ))]
                            })?,
                            source_lsb: u16::try_from(source_lsb).map_err(|_| {
                                vec![Diagnostic::error(format!(
                                    "plan-laid value type `{}`: source bit {source_lsb} exceeds the backend width vocabulary",
                                    record.synthetic_name
                                ))]
                            })?,
                            width: u16::try_from(width).map_err(|_| {
                                vec![Diagnostic::error(format!(
                                    "plan-laid value type `{}`: fragment width {width} exceeds the backend width vocabulary",
                                    record.synthetic_name
                                ))]
                            })?,
                        };
                        offsets[field_index].get_or_insert(fragment.container_byte_offset);
                        fragments.push(fragment);
                    }
                    bit_fields.push(PlanLaidBitField {
                        field_index,
                        fragments,
                    });
                }
                _ => {
                    return Err(vec![Diagnostic::error(format!(
                        "plan-laid value type `{}`: field `{field_name}` does not have one \
                         byte placement or a completely tiled bit-fragment placement",
                        record.synthetic_name
                    ))]);
                }
            }
        }

        // Normalized plans retain target-independent u64 geometry. This
        // consumer needs host-sized layout indices, so narrow only here and
        // reject rather than panicking on a narrower compiler host.
        let offsets = offsets
            .iter()
            .copied()
            .collect::<Option<Vec<_>>>()
            .expect("validated plan supplies every field");
        let size = usize::try_from(size).map_err(|_| {
            vec![Diagnostic::error(format!(
                "plan-laid value type `{}`: fixed size {size} cannot be represented on this compiler host",
                record.synthetic_name
            ))]
        })?;
        let align = usize::try_from(report.align).map_err(|_| {
            vec![Diagnostic::error(format!(
                "plan-laid value type `{}`: alignment {} cannot be represented on this compiler host",
                record.synthetic_name, report.align
            ))]
        })?;
        layouts.push(PlanLaidLayout {
            data_name: record.synthetic_name.clone(),
            data_symbol,
            field_symbols,
            schema_symbol,
            schema_field_symbols,
            policy_symbol,
            policy_plan_machine_symbol,
            validated_layout: report.clone(),
            offsets,
            bit_fields,
            integer_fields,
            repeated_fields,
            size,
            align,
        });
    }

    typed.plan_laid_layouts = layouts;
    Ok(())
}

fn stored_integer_write_is_total(
    typed: &TypedTrees,
    field_type: psi_typed_trees::types::TypeReferenceHandle,
    stored_width: u64,
    interpretation: psi_layout_plans::IntegerInterpretation,
) -> bool {
    let Some(primitive) = typed.primitive_type_reference(field_type) else {
        return false;
    };
    let (admitted_minimum, admitted_maximum) = if let Some(range) =
        psi_typed_trees::wire::scalar_representation_range(typed, field_type)
    {
        (i128::from(range.minimum), i128::from(range.maximum))
    } else {
        let width = primitive.scalar_byte_size().unwrap_or(0) * 8;
        if width == 0 {
            return false;
        }
        if primitive.is_signed_integer() {
            (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
        } else {
            (0, (1i128 << width) - 1)
        }
    };
    let (stored_minimum, stored_maximum) = match interpretation {
        psi_layout_plans::IntegerInterpretation::Signed => (
            -(1i128 << (stored_width - 1)),
            (1i128 << (stored_width - 1)) - 1,
        ),
        psi_layout_plans::IntegerInterpretation::Unsigned => (0, (1i128 << stored_width) - 1),
    };
    admitted_minimum >= stored_minimum && admitted_maximum <= stored_maximum
}
