use std::collections::BTreeMap;

use psi_access_plans::{AccessExposure, ExternalRead, FieldAccess, ValidatedPlacementPlan};
use psi_arena::{Handle, HandleSpan};
use psi_diagnostics::Diagnostic;
use psi_language_semantics::{DataSupplyMode, Multiplicity};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{
    DataDefinition, DataField, DataMember, DataProperties, Item, Machine,
};
use psi_syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::{Application, PendingRewrite, SchemaRecord};

pub(super) fn synthesize_probe_records(
    syntax: &mut SyntaxTrees,
    applications: &[Application],
    rewrites: &[PendingRewrite],
    schemas: &BTreeMap<String, SchemaRecord>,
) {
    for application in applications {
        let members = schemas[&application.schema]
            .fields
            .iter()
            .map(|field| {
                let arguments = syntax
                    .tables
                    .type_references
                    .insert_type_reference_handles([field.type_reference]);
                let accessor = syntax
                    .tables
                    .type_references
                    .insert_generic(Identifier::generated("PlacedField"), arguments);
                DataMember::Field(DataField {
                    identity: field.identity,
                    name: field.name.clone(),
                    relevance: field.relevance,
                    type_reference: accessor,
                })
            })
            .collect::<Vec<_>>();
        push_record(
            syntax,
            &application.synthetic_name,
            members,
            application.generated_is_public,
        );
    }
    rewrite_applications(syntax, rewrites);
}

pub(super) fn synthesize_exact_records(
    syntax: &mut SyntaxTrees,
    applications: &[Application],
    rewrites: &[PendingRewrite],
    schemas: &BTreeMap<String, SchemaRecord>,
    plans: &BTreeMap<String, ValidatedPlacementPlan>,
) -> Result<(), Vec<Diagnostic>> {
    let template = syntax.clone();
    let template_machines = template
        .root_items()
        .filter_map(|item| {
            let Item::Machine(machine) = item else {
                return None;
            };
            (machine
                .attached_data
                .as_ref()
                .map(|identifier| identifier.as_str())
                == Some("PlacedField"))
            .then_some(machine.clone())
        })
        .collect::<Vec<_>>();

    for application in applications {
        let plan = &plans[&application.synthetic_name];
        let schema = &schemas[&application.schema];
        let mut members = Vec::new();
        for field in &schema.fields {
            let Some(entry) = plan
                .access()
                .plan()
                .entries()
                .iter()
                .find(|entry| entry.field() == field.name.as_str())
            else {
                return Err(vec![Diagnostic::error(format!(
                    "placed view `{}` lost canonical schema field `{}`",
                    application.synthetic_name,
                    field.name.as_str()
                ))]);
            };
            if matches!(entry.access(), FieldAccess::Inaccessible) {
                continue;
            }

            let accessor_name = accessor_name(syntax, application, field, entry.access())?;
            let is_exported = access_is_exported(entry.access());
            syntax.push_root_item(Item::Data(DataDefinition {
                name: Identifier::generated(accessor_name.clone()),
                // A published shell must be well-formed even when one field's
                // operations are binding-private. The opaque carrier itself
                // grants no operation; cloned machine visibility and the
                // exact installed AccessExposure row retain that authority.
                is_public: application.generated_is_public,
                supply_mode: DataSupplyMode::BoundaryOpaque,
                lifetime_parameters: Vec::new(),
                type_parameters: HandleSpan::empty(),
                generic_instance: None,
                properties: DataProperties::default(),
                where_facts: HandleSpan::empty(),
                members: HandleSpan::empty(),
                quotient: None,
            }));
            for operation in accessor_operations(entry.access()) {
                let Some(machine) = template_machines.iter().find(|machine| {
                    machine
                        .name
                        .as_str()
                        .rsplit("::")
                        .next()
                        .is_some_and(|name| name == operation)
                }) else {
                    return Err(vec![Diagnostic::error(format!(
                        "compiler accessor template `PlacedField::{operation}` is unavailable"
                    ))]);
                };
                clone_accessor_machine(
                    syntax,
                    &template,
                    machine,
                    &accessor_name,
                    field.type_reference,
                    is_exported && application.generated_is_public,
                );
            }
            let field_type = syntax
                .tables
                .type_references
                .insert_named(Identifier::generated(accessor_name));
            members.push(DataMember::Field(DataField {
                identity: field.identity,
                name: field.name.clone(),
                relevance: field.relevance,
                type_reference: field_type,
            }));
        }
        push_record(
            syntax,
            &application.synthetic_name,
            members,
            application.generated_is_public,
        );
    }
    rewrite_applications(syntax, rewrites);
    retire_accessor_templates(syntax);
    Ok(())
}

fn accessor_name(
    syntax: &SyntaxTrees,
    application: &Application,
    field: &DataField,
    access: &FieldAccess,
) -> Result<String, Vec<Diagnostic>> {
    let base = if matches!(access, FieldAccess::Atomic { .. }) {
        match syntax
            .tables
            .type_references
            .type_reference(field.type_reference)
        {
            TypeReferenceNode::Named(name) => match name.as_str() {
                "bool" => "AtomicBool#PlacedField",
                "u32" => "AtomicU32#PlacedField",
                "u64" => "AtomicU64#PlacedField",
                _ => {
                    return Err(vec![Diagnostic::error(format!(
                        "placed atomic field `{}` in `{}` requires schema type `bool`, `u32`, or `u64`",
                        field.name.as_str(),
                        application.synthetic_name
                    ))]);
                }
            },
            _ => {
                return Err(vec![Diagnostic::error(format!(
                    "placed atomic field `{}` in `{}` requires a plain schema type `bool`, `u32`, or `u64`",
                    field.name.as_str(),
                    application.synthetic_name
                ))]);
            }
        }
    } else {
        "PlacedField"
    };
    Ok(format!(
        "{base}<{},{},{}>",
        application.policy,
        application.schema,
        field.name.as_str()
    ))
}

pub(super) fn retire_accessor_templates(syntax: &mut SyntaxTrees) {
    let templates = syntax
        .root_item_handles()
        .iter()
        .filter_map(|handle| {
            let Item::Machine(machine) = syntax.root_item(*handle) else {
                return None;
            };
            (machine
                .attached_data
                .as_ref()
                .is_some_and(|attached| attached.as_str() == "PlacedField"))
            .then_some((*handle, machine.clone()))
        })
        .collect::<Vec<_>>();
    for (handle, mut machine) in templates {
        machine.target = Some(Identifier::generated("#compiler_placed_template"));
        syntax
            .tables
            .items
            .replace_item(handle, Item::Machine(machine));
    }
}

pub(super) fn accessor_operations(access: &FieldAccess) -> Vec<&'static str> {
    match access {
        FieldAccess::Inaccessible | FieldAccess::Atomic { .. } => Vec::new(),
        FieldAccess::Stable { read, write, .. } => {
            let mut operations = Vec::new();
            if *read {
                operations.push("read");
            }
            if *write {
                operations.push("write");
            }
            operations
        }
        FieldAccess::External { read, write, .. } => {
            let mut operations = Vec::new();
            match read {
                ExternalRead::None => {}
                ExternalRead::Read => operations.push("read"),
                ExternalRead::Take => operations.push("take"),
            }
            if *write {
                operations.push("write");
            }
            operations
        }
    }
}

fn clone_accessor_machine(
    syntax: &mut SyntaxTrees,
    template: &SyntaxTrees,
    machine: &Machine,
    accessor_name: &str,
    field_type: TypeReferenceHandle,
    is_public: bool,
) {
    let watermark = syntax.tables.type_references.node_count();
    let Item::Machine(mut clone) = syntax.copy_item_from(template, &Item::Machine(machine.clone()))
    else {
        unreachable!("copied accessor template remains a machine");
    };
    let operation = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .expect("accessor template operation");
    clone.name = Identifier::generated(format!("{accessor_name}::{operation}"));
    clone.attached_data = Some(Identifier::generated(accessor_name));
    clone.is_public = is_public;
    clone.type_parameters = HandleSpan::empty();
    let replacement = syntax
        .tables
        .type_references
        .type_reference(field_type)
        .clone();
    for (handle, name) in syntax.tables.type_references.named_nodes_from(watermark) {
        if name == "T" {
            syntax
                .tables
                .type_references
                .replace_type_reference(handle, replacement.clone());
        }
    }
    syntax.push_root_item(Item::Machine(clone));
}

fn access_is_exported(access: &FieldAccess) -> bool {
    matches!(
        access,
        FieldAccess::Stable {
            exposure: AccessExposure::Exported,
            ..
        } | FieldAccess::External {
            exposure: AccessExposure::Exported,
            ..
        } | FieldAccess::Atomic {
            exposure: AccessExposure::Exported,
            ..
        }
    )
}

fn push_record(syntax: &mut SyntaxTrees, name: &str, members: Vec<DataMember>, is_public: bool) {
    let mut first = Handle::invalid();
    let mut count = 0u32;
    for member in members {
        let handle = syntax.tables.items.append_data_member(member);
        if count == 0 {
            first = handle;
        }
        count += 1;
    }
    syntax.push_root_item(Item::Data(DataDefinition {
        // The shell is compiler-owned rather than a declaration published by
        // either input package. Discovery has already retained the exact
        // policy/schema identities and enforced their visibility; field
        // operation visibility is governed separately by AccessExposure.
        name: Identifier::generated(name),
        is_public,
        supply_mode: DataSupplyMode::CheckedShape,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        generic_instance: None,
        properties: DataProperties {
            multiplicity: Multiplicity::Linear,
            carry: None,
        },
        where_facts: HandleSpan::empty(),
        members: HandleSpan::from_parts(first, count),
        quotient: None,
    }));
}

fn rewrite_applications(syntax: &mut SyntaxTrees, rewrites: &[PendingRewrite]) {
    for rewrite in rewrites {
        syntax.tables.type_references.replace_type_reference(
            rewrite.type_reference,
            TypeReferenceNode::Named(Identifier::generated(rewrite.synthetic_name.as_str())),
        );
    }
}
