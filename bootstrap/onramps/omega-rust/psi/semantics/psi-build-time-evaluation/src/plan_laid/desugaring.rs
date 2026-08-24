use std::collections::{HashMap, HashSet};

use psi_arena::{Handle, HandleSpan};
use psi_diagnostics::Diagnostic;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{DataDefinition, DataMember, DataProperties, Item};
use psi_syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::PlanLaidRecord;

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
            is_public: false,
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
