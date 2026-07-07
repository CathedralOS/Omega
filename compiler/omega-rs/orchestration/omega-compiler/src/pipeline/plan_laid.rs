//! PLAN-LAID VALUE TYPES -- L4 of the LAYOUTS ladder
//! (design_briefs/programmable_layouts.md §5): a field `gdt: CLayout<Gdt>;`
//! applies a layout POLICY (ordinary data with an effect-free `plan` machine)
//! to a SCHEMA (a plain record of primitives) in type position. The value
//! behaves exactly like the schema type -- same fields, ZII, projections --
//! but its native in-memory placement comes from the validated plan instead
//! of the compiler's own packing.
//!
//! Two passes, both driven from the pipeline:
//!
//! 1. `desugar_plan_laid_value_types` (PRE-RESOLUTION, on the merged syntax
//!    trees): synthesizes `data CLayout<Gdt> { <schema fields> }` and rewrites
//!    the generic spelling to that plain name, so symbol resolution, typing,
//!    validation, proof, and the interpreter all see an ordinary record. The
//!    interpreter is name-keyed, so it needs nothing else.
//! 2. `compute_plan_laid_layouts` (POST-TYPING, after const-length
//!    substitution): evaluates the policy at build time through the existing
//!    L2/L3 pipeline (`compute_layout_plan` -- purity gate, plan validation),
//!    requires the plan be FULLY STATIC (a dynamic plan cannot be a value
//!    type: values need offsets, bytes need mints), and records the placement
//!    on `TypedTrees::plan_laid_layouts` for the native layout builder.
//!
//! v0 boundaries (documented, all clean errors): schemas are plain records of
//! primitives; the spelling is legal in FIELD type position (params/lets keep
//! the existing generic-type errors); construction is ZII + per-field writes
//! (a `CLayout<Gdt> { ... }` literal is not spellable).

use omega_core::arena::{Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{DataDefinition, DataMember, DataProperties, Item};
use omega_syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use omega_typed_trees::{PlanLaidLayout, TypedTrees};
use std::collections::{HashMap, HashSet};

/// One plan-laid instantiation discovered by the desugar: the synthesized
/// data definition plus the (policy, schema) pair whose validated plan will
/// dictate its layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlanLaidRecord {
    /// Name of the synthesized data definition (`CLayout<GdtEntryish>`).
    pub(crate) synthetic_name: String,
    /// Qualified policy machine (`CLayout::plan`).
    pub(crate) policy_machine: String,
    /// The schema data definition the plan places.
    pub(crate) schema_data: String,
}

struct IndexedData {
    has_type_parameters: bool,
    members: HandleSpan<DataMember>,
    properties: DataProperties,
}

struct PendingRewrite {
    type_reference: TypeReferenceHandle,
    synthetic_name: String,
}

/// Find `Policy<Schema>` spellings in FIELD type position where `Policy` is a
/// non-generic data definition with an attached `plan` machine, synthesize the
/// instance definitions, and rewrite the spellings. Returns the records the
/// post-typing plan pass needs.
pub(crate) fn desugar_plan_laid_value_types(
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
                        has_type_parameters: !definition.type_parameters.is_empty(),
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

    // Scan field type references for policy applications. Collection only --
    // mutation happens after the scan so the borrows stay simple.
    let mut diagnostics = Vec::new();
    let mut rewrites: Vec<PendingRewrite> = Vec::new();
    let mut records: Vec<PlanLaidRecord> = Vec::new();
    for item in syntax.root_items() {
        let Item::Data(definition) = item else {
            continue;
        };
        for member in syntax.tables.items.data_members(definition.members) {
            let DataMember::Field(field) = member else {
                continue;
            };
            let TypeReferenceNode::Generic {
                base_name,
                arguments,
            } = syntax.tables.type_references.type_reference(field.type_reference)
            else {
                continue;
            };
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
                type_reference: field.type_reference,
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
            type_parameters: HandleSpan::default(),
            properties: schema_info.properties,
            members: HandleSpan::from_parts(first, count),
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
pub(crate) fn compute_plan_laid_layouts(
    typed: &mut TypedTrees,
    records: &[PlanLaidRecord],
) -> Result<(), Vec<Diagnostic>> {
    if records.is_empty() {
        return Ok(());
    }

    let mut layouts = Vec::with_capacity(records.len());
    for record in records {
        let report = crate::pipeline::layout_plans::compute_layout_plan(
            typed,
            &record.policy_machine,
            &record.schema_data,
        )
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

        // Plan validation already proved offsets non-negative, the size
        // covering every field, and the alignment a positive power of two.
        layouts.push(PlanLaidLayout {
            data_name: record.synthetic_name.clone(),
            offsets: report
                .offsets
                .iter()
                .map(|&offset| {
                    usize::try_from(offset).expect("validated plan offsets are non-negative")
                })
                .collect(),
            size: usize::try_from(size).expect("validated fixed sizes are non-negative"),
            align: usize::try_from(report.align).expect("validated alignments are positive"),
        });
    }

    typed.plan_laid_layouts = layouts;
    Ok(())
}
