use std::collections::BTreeMap;

use access_plans::{AccessExposure, FieldAccess, ValidatedPlacementPlan};
use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use language_semantics::{DataSupplyMode, Multiplicity};
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{DataDefinition, DataField, DataMember, DataProperties, Item, Machine};
use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::{Application, ApplicationKey, DeclarationKey, PendingRewrite, SchemaRecord};

/// A synthesized declaration's symbol provenance: the authored `Placed<P, T>`
/// use-site that derived it. Compiler-owned shells are source-free text, but
/// package review and Terminal retention resolve a managed owner through the
/// name's retained span — borrowing the invocation occurrence pins the shell
/// to the package that spelled the application instead of leaving a
/// provenance-free orphan no review can own.
fn generated_identifier(name: impl Into<String>, provenance: source::SourceSpan) -> Identifier {
    Identifier::new(name.into(), provenance)
}

fn application_provenance(application: &Application) -> source::SourceSpan {
    application
        .invocation_sources
        .first()
        .copied()
        .unwrap_or_default()
}

pub(super) fn synthesize_probe_records(
    syntax: &mut SyntaxTrees,
    applications: &[Application],
    rewrites: &[PendingRewrite],
    schemas: &BTreeMap<DeclarationKey, SchemaRecord>,
    sources: Option<&source::SourceMap>,
) -> Result<(), Vec<Diagnostic>> {
    let templates = syntax
        .root_items()
        .filter_map(|item| item.as_data())
        .filter(|data| {
            data.name.as_str().rsplit("::").next() == Some("PlacedField")
                && sources.is_none_or(|sources| {
                    sources
                        .file_at(data.name.source_span())
                        .is_some_and(|source| {
                            source.origin == source::SourceOrigin::Toolchain
                                && source.path.strip_prefix(&source.package_root).ok()
                                    == Some(std::path::Path::new("layout.omg"))
                        })
                })
        })
        .map(|data| data.name.clone())
        .collect::<Vec<_>>();
    let [template] = templates.as_slice() else {
        return Err(vec![Diagnostic::error(
            "placed probe requires one exact sealed accessor template",
        )]);
    };
    for application in applications {
        let members = schemas[&application.key.schema]
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
                    .insert_generic(template.clone(), arguments);
                DataMember::Field(DataField {
                    identity: field.identity,
                    name: field.name.clone(),
                    relevance: field.relevance,
                    type_reference: accessor,
                })
            })
            .collect::<Vec<_>>();
        let origin_node = syntax
            .type_references
            .type_reference(application.origin)
            .clone();
        let origin = syntax.type_references.insert(origin_node);
        push_record(
            syntax,
            origin,
            &application.synthetic_name,
            members,
            application.generated_is_public,
            application_provenance(application),
        );
    }
    rewrite_applications(syntax, rewrites);
    Ok(())
}

pub(super) fn synthesize_exact_records(
    syntax: &mut SyntaxTrees,
    applications: &[Application],
    rewrites: &[PendingRewrite],
    schemas: &BTreeMap<DeclarationKey, SchemaRecord>,
    plans: &BTreeMap<ApplicationKey, ValidatedPlacementPlan>,
    sources: Option<&source::SourceMap>,
) -> Result<(), Vec<Diagnostic>> {
    let template = syntax.clone();
    let template_machines = template
        .root_items()
        .filter_map(|item| {
            let machine = item.as_machine()?;
            sealed_accessor_template(machine, sources).then_some(machine.clone())
        })
        .collect::<Vec<_>>();

    for application in applications {
        let plan = &plans[&application.key];
        let schema = &schemas[&application.key.schema];
        let mut members = Vec::new();
        for field in &schema.fields {
            let entry = plan
                .access()
                .plan()
                .entries()
                .iter()
                .find(|entry| entry.field() == field.name.as_str());
            if field.relevance.is_erased() && entry.is_none() {
                // Schema reflection deliberately omits erased semantic fields.
                // They remain available to the custody-agreement checker but
                // must not acquire a synthesized physical accessor.
                continue;
            }
            let Some(entry) = entry else {
                return Err(vec![Diagnostic::error(format!(
                    "placed view `{}` lost canonical schema field `{}`",
                    application.synthetic_name,
                    field.name.as_str()
                ))]);
            };
            if matches!(entry.access(), FieldAccess::Inaccessible) {
                continue;
            }

            let provenance = application_provenance(application);
            let accessor_name = accessor_name(syntax, application, field, entry.access())?;
            let is_exported = access_is_exported(entry.access());
            syntax.push_root_item(Item::Data(DataDefinition {
                name: generated_identifier(accessor_name.clone(), provenance),
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
                    provenance,
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
        let origin_node = syntax
            .type_references
            .type_reference(application.origin)
            .clone();
        let origin = syntax.type_references.insert(origin_node);
        push_record(
            syntax,
            origin,
            &application.synthetic_name,
            members,
            application.generated_is_public,
            application_provenance(application),
        );
    }
    rewrite_applications(syntax, rewrites);
    retire_accessor_templates(syntax, sources);
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
        "{base}<{},{}>",
        application.synthetic_name,
        field.name.as_str()
    ))
}

fn sealed_accessor_template(machine: &Machine, sources: Option<&source::SourceMap>) -> bool {
    machine
        .attached_data
        .as_ref()
        .is_some_and(|attached| attached.as_str() == "PlacedField")
        && sources.is_none_or(|sources| {
            sources
                .file_at(machine.name.source_span())
                .is_some_and(|source| {
                    source.origin == source::SourceOrigin::Toolchain
                        && source.path.strip_prefix(&source.package_root).ok()
                            == Some(std::path::Path::new("layout.omg"))
                })
        })
}

pub(super) fn retire_accessor_templates(
    syntax: &mut SyntaxTrees,
    sources: Option<&source::SourceMap>,
) {
    // These generic declarations are elaboration inputs, not unrealized target
    // bodies. Retire their declaration roots after cloning exact accessors;
    // a synthetic target tag would keep them in all-target source checking and
    // package disclosure. Item handles for all surviving declarations stay
    // unchanged, including the generated accessors and authored target members.
    let mut retained_roots = Vec::with_capacity(syntax.root_item_count());
    for handle in syntax.root_item_handles() {
        let is_template = matches!(syntax.root_item(*handle), Item::Machine(machine)
            if sealed_accessor_template(machine, sources));
        if !is_template {
            retained_roots.push(*handle);
        }
    }
    syntax.roots.items = retained_roots;
}

pub(super) fn accessor_operations(access: &FieldAccess) -> Vec<&'static str> {
    access.operation_names()
}

fn clone_accessor_machine(
    syntax: &mut SyntaxTrees,
    template: &SyntaxTrees,
    machine: &Machine,
    accessor_name: &str,
    field_type: TypeReferenceHandle,
    is_public: bool,
    provenance: source::SourceSpan,
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
    clone.name = generated_identifier(format!("{accessor_name}::{operation}"), provenance);
    clone.attached_data = Some(generated_identifier(accessor_name, provenance));
    clone.is_public = is_public;
    clone.type_parameters = HandleSpan::empty();
    // The template spells its subject `placed_field: &PlacedField<T>`: the
    // subject position binds the accessor's OWN attached data, so the copied
    // application rewrites to the opaque accessor identity -- not to the
    // field's `T` argument -- before the ordinary `Named(T)` substitution
    // below fills the field type everywhere else. Preserve the template's
    // explicit parameter role: its trait requirement substitutes `Self`
    // with this exact accessor type, without introducing a receiver.
    let subject = TypeReferenceNode::Named(generated_identifier(accessor_name, provenance));
    for (handle, base_name) in syntax.tables.type_references.generic_nodes_from(watermark) {
        if base_name == "PlacedField" {
            syntax
                .tables
                .type_references
                .replace_type_reference(handle, subject.clone());
        }
    }
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

fn push_record(
    syntax: &mut SyntaxTrees,
    origin: TypeReferenceHandle,
    name: &str,
    members: Vec<DataMember>,
    is_public: bool,
    provenance: source::SourceSpan,
) {
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
        // Its name still borrows the authored use-site span so package review
        // and hermetic retention can resolve the consuming package as owner.
        name: generated_identifier(name, provenance),
        is_public,
        supply_mode: DataSupplyMode::CheckedShape,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        generic_instance: Some(origin),
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
