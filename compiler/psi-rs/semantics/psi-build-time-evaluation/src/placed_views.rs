//! Source `Placed<Policy, Schema>` derivation.
//!
//! Placement policies are evaluated from typed trees, while the accessor
//! fields selected by that policy must exist before ordinary symbol resolution
//! and typing. This pass follows the same two-pass discipline as const-generic
//! calls:
//!
//! 1. discover each concrete `Placed<P, T>` application;
//! 2. build a probe tree in which every schema field has the opaque,
//!    permissive `PlacedField<FieldType>` template;
//! 3. type the probe and evaluate `P::plan(T)` through the canonical placement
//!    evaluator;
//! 4. synthesize the authoritative placed record with only accepted fields and
//!    clone only the named accessor operations admitted for each field.
//!
//! Every field type is unique to `(policy, schema, field)`. Atomic accessor
//! names retain their underlying primitive class for existing atomic
//! instruction typing, while the installed typed-tree plan -- not that name --
//! owns the exact operation subset. A helper may accept an accessor without
//! receiving the whole view, while a field omitted by the normalized plan has
//! no member to project. Provider admission remains the only construction
//! route; the synthesized placed record contains opaque accessor fields and is
//! linear.

use std::collections::{BTreeMap, BTreeSet};

use psi_access_plans::{ExternalRead, FieldAccess, PlacementPlanId, ValidatedPlacementPlan};
use psi_arena::{Handle, HandleSpan};
use psi_diagnostics::Diagnostic;
use psi_language_semantics::{DataSupplyMode, Multiplicity};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{
    DataDefinition, DataField, DataMember, DataProperties, Item, Machine,
};
use psi_syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use psi_typed_trees::TypedTrees;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedViewRecord {
    pub synthetic_name: String,
    pub policy_machine: String,
    pub schema_data: String,
    pub normalized_placement: PlacementPlanId,
}

#[derive(Clone)]
struct SchemaRecord {
    fields: Vec<DataField>,
}

#[derive(Debug, Clone)]
struct Application {
    synthetic_name: String,
    policy: String,
    schema: String,
}

#[derive(Debug, Clone)]
struct PendingRewrite {
    type_reference: TypeReferenceHandle,
    synthetic_name: String,
}

type Discovery = (
    Vec<Application>,
    Vec<PendingRewrite>,
    BTreeMap<String, SchemaRecord>,
);

pub fn desugar_placed_views(
    syntax: &mut SyntaxTrees,
) -> Result<Vec<PlacedViewRecord>, Vec<Diagnostic>> {
    let (applications, rewrites, schemas) = discover_applications(syntax)?;
    if applications.is_empty() {
        // The generic boundary templates exist only as compiler input for
        // cloning exact accessor machines. Leaving them active when a program
        // has no `Placed<P, T>` application makes ordinary static trait
        // selection consider `PlacedField<T>` as a real generic `Readable`,
        // `DestructiveRead`, or `Writable` provider.
        retire_accessor_templates(syntax);
        return Ok(Vec::new());
    }

    let mut probe = syntax.clone();
    synthesize_probe_records(&mut probe, &applications, &rewrites, &schemas);
    let mut probe = psi_generic_instances::normalize_pre_resolution(probe)?;
    let probe_plan_laid = crate::desugar_plan_laid_value_types(&mut probe)?;
    let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&probe)?;
    let mut typed =
        psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .map_err(|diagnostic| vec![diagnostic])?;
    crate::evaluate_const_array_lengths(&mut typed)?;
    crate::evaluate_const_domain_facts(&mut typed)?;
    crate::compute_plan_laid_layouts(&mut typed, &probe_plan_laid)?;

    let mut plans = BTreeMap::new();
    for application in &applications {
        let policy_machine = format!("{}::plan", application.policy);
        let plan = crate::compute_placement_plan(&typed, &policy_machine, &application.schema)
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "placed view `{}`: {reason}",
                    application.synthetic_name
                ))]
            })?;
        plans.insert(application.synthetic_name.clone(), plan);
    }

    synthesize_exact_records(syntax, &applications, &rewrites, &schemas, &plans)?;
    Ok(applications
        .into_iter()
        .map(|application| {
            let plan = &plans[&application.synthetic_name];
            PlacedViewRecord {
                synthetic_name: application.synthetic_name,
                policy_machine: format!("{}::plan", application.policy),
                schema_data: application.schema,
                normalized_placement: plan.identity(),
            }
        })
        .collect())
}

pub fn validate_placed_view_plans(
    typed: &mut TypedTrees,
    records: &[PlacedViewRecord],
) -> Result<(), Vec<Diagnostic>> {
    for record in records {
        let plan =
            crate::compute_placement_plan(typed, &record.policy_machine, &record.schema_data)
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "placed view `{}`: {reason}",
                        record.synthetic_name
                    ))]
                })?;
        if plan.identity() != record.normalized_placement {
            return Err(vec![Diagnostic::error(format!(
                "placed view `{}` changed normalized placement identity between accessor derivation and the authoritative typed program",
                record.synthetic_name
            ))]);
        }
        install_placed_view_plan(typed, record, &plan)?;
    }
    Ok(())
}

fn install_placed_view_plan(
    typed: &mut TypedTrees,
    record: &PlacedViewRecord,
    placement: &ValidatedPlacementPlan,
) -> Result<(), Vec<Diagnostic>> {
    let policy_symbol = typed
        .data_definitions()
        .iter()
        .find(|definition| {
            definition.name.as_str()
                == record
                    .policy_machine
                    .strip_suffix("::plan")
                    .unwrap_or(&record.policy_machine)
        })
        .map(|definition| definition.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "placed view `{}` lost nominal policy `{}` after typing",
                record.synthetic_name, record.policy_machine
            ))]
        })?;
    let schema = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == record.schema_data)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "placed view `{}` lost schema `{}` after typing",
                record.synthetic_name, record.schema_data
            ))]
        })?;
    let schema_fields = typed
        .data_members(schema)
        .iter()
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => Some(field),
            psi_typed_trees::data::DataMember::Variant(_) => None,
        })
        .map(|field| (field.name.as_str().to_owned(), field))
        .collect::<BTreeMap<_, _>>();
    let view = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == record.synthetic_name)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "placed view `{}` lost its derived data definition after typing",
                record.synthetic_name
            ))]
        })?;
    let view_fields = typed
        .data_members(view)
        .iter()
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => {
                Some((field.name.as_str().to_owned(), field))
            }
            psi_typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect::<BTreeMap<_, _>>();
    let schema_symbol = schema.symbol;
    let data_symbol = view.symbol;

    let mut fields = Vec::new();
    for entry in placement.access().plan().entries() {
        if matches!(entry.access(), FieldAccess::Inaccessible) {
            continue;
        }
        let schema_field = schema_fields.get(entry.field()).copied().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "placed view `{}` lost schema field `{}` after typing",
                record.synthetic_name,
                entry.field()
            ))]
        })?;
        let view_field = view_fields.get(entry.field()).copied().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "placed view `{}` lost admitted field `{}` after typing",
                record.synthetic_name,
                entry.field()
            ))]
        })?;
        if schema_field.identity != view_field.identity {
            return Err(vec![Diagnostic::error(format!(
                "placed view `{}` field `{}` changed stable member identity during accessor derivation",
                record.synthetic_name,
                entry.field()
            ))]);
        }
        let accessor_name = typed
            .named_type_reference(view_field.type_reference)
            .map(|name| name.as_str().to_owned())
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "placed view `{}` field `{}` lost its nominal accessor type after typing",
                    record.synthetic_name,
                    entry.field()
                ))]
            })?;
        let accessor_type_symbol = typed
            .type_reference_table
            .type_symbol(view_field.type_reference);
        let accessor_data = typed
            .data_definitions()
            .iter()
            .filter(|definition| {
                if accessor_type_symbol.is_valid() {
                    definition.symbol == accessor_type_symbol
                } else {
                    // Atomic synthesized carriers currently retain their
                    // specialized nominal spelling while their operation law
                    // remains in the exact Atomic typed carrier.
                    matches!(entry.access(), FieldAccess::Atomic { .. })
                        && definition.name.as_str() == accessor_name
                }
            })
            .collect::<Vec<_>>();
        let [accessor_data] = accessor_data.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "placed view `{}` field `{}` must retain one exact generated accessor data definition",
                record.synthetic_name,
                entry.field()
            ))]);
        };
        let mut accessor_targets = Vec::new();
        for operation in accessor_operations(entry.access()) {
            let machines = typed
                .machines()
                .iter()
                .filter(|machine| {
                    machine.attached_data.as_ref().is_some_and(|attached| {
                        attached.as_str() == accessor_name
                            && machine
                                .name
                                .as_str()
                                .rsplit("::")
                                .next()
                                .is_some_and(|name| name == operation)
                    })
                })
                .collect::<Vec<_>>();
            let [machine] = machines.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "placed view `{}` field `{}` must retain one exact generated `{operation}` accessor machine",
                    record.synthetic_name,
                    entry.field()
                ))]);
            };
            let [state] = typed.machine_states(machine) else {
                return Err(vec![Diagnostic::error(format!(
                    "placed view `{}` field `{}` generated `{operation}` accessor must have one exact callable state",
                    record.synthetic_name,
                    entry.field()
                ))]);
            };
            accessor_targets.push(psi_typed_trees::typed_trees::PlacedAccessorTarget {
                operation: operation.to_owned(),
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
            });
        }
        fields.push(psi_typed_trees::typed_trees::PlacedFieldPlan {
            field_name: entry.field().to_owned(),
            member_identity: schema_field.identity,
            field_symbol: schema_field.symbol,
            accessor_name,
            accessor_data_symbol: accessor_data.symbol,
            accessor_targets,
            value_type: schema_field.type_reference,
            access: entry.access().clone(),
        });
    }
    typed
        .placed_view_plans
        .push(psi_typed_trees::typed_trees::PlacedViewPlan {
            data_name: record.synthetic_name.clone(),
            data_symbol,
            policy_name: record
                .policy_machine
                .strip_suffix("::plan")
                .unwrap_or(&record.policy_machine)
                .to_owned(),
            policy_symbol,
            schema_name: record.schema_data.clone(),
            schema_symbol,
            placement: placement.clone(),
            fields,
        });
    Ok(())
}

fn discover_applications(syntax: &SyntaxTrees) -> Result<Discovery, Vec<Diagnostic>> {
    let mut data = BTreeMap::new();
    let mut placement_policies = BTreeSet::new();
    for item in syntax.root_items() {
        match item {
            Item::Data(definition) => {
                let fields = syntax
                    .tables
                    .items
                    .data_members(definition.members)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) => Some(field.clone()),
                        DataMember::Variant(_) | DataMember::Retired(_) => None,
                    })
                    .collect::<Vec<_>>();
                data.insert(
                    definition.name.as_str().to_owned(),
                    (
                        definition.type_parameters.is_empty(),
                        definition.supply_mode,
                        fields,
                        syntax.tables.items.data_members(definition.members).len(),
                    ),
                );
            }
            Item::Machine(machine)
                if machine.attached_data.as_ref().is_some_and(|attached| {
                    machine.name.as_str() == format!("{}::plan", attached.as_str())
                }) =>
            {
                placement_policies.insert(
                    machine
                        .attached_data
                        .as_ref()
                        .expect("matched attached machine")
                        .as_str()
                        .to_owned(),
                );
            }
            _ => {}
        }
    }

    let mut diagnostics = Vec::new();
    let mut applications = Vec::new();
    let mut rewrites = Vec::new();
    let mut schemas = BTreeMap::new();
    for type_reference in syntax.tables.type_references.generic_nodes() {
        let TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } = syntax.tables.type_references.type_reference(type_reference)
        else {
            unreachable!("generic_nodes returned a non-generic node");
        };
        if base_name.as_str() != "Placed" {
            continue;
        }
        if !lifetime_arguments.is_empty() {
            diagnostics.push(Diagnostic::error(
                "`Placed<Policy, Schema>` takes no authored lifetime arguments",
            ));
            continue;
        }
        let argument_handles = syntax
            .tables
            .type_references
            .type_reference_handles(*arguments);
        let [policy_handle, schema_handle] = argument_handles else {
            diagnostics.push(Diagnostic::error(
                "`Placed` takes exactly a nominal placement policy and a plain schema",
            ));
            continue;
        };
        let Some(policy) = named_argument(syntax, *policy_handle) else {
            diagnostics.push(Diagnostic::error(
                "the first `Placed` argument must be a nominal placement-policy data name",
            ));
            continue;
        };
        let Some(schema) = named_argument(syntax, *schema_handle) else {
            diagnostics.push(Diagnostic::error(
                "the second `Placed` argument must be a plain schema data name",
            ));
            continue;
        };
        if !placement_policies.contains(&policy) {
            diagnostics.push(Diagnostic::error(format!(
                "`Placed<{policy}, {schema}>` names `{policy}`, but it has no attached `{policy}::plan` machine"
            )));
            continue;
        }
        let Some((plain, supply_mode, fields, member_count)) = data.get(&schema) else {
            diagnostics.push(Diagnostic::error(format!(
                "`Placed<{policy}, {schema}>` names an unknown schema `{schema}`"
            )));
            continue;
        };
        if !*plain
            || *supply_mode == DataSupplyMode::BoundaryOpaque
            || fields.is_empty()
            || fields.len() != *member_count
        {
            diagnostics.push(Diagnostic::error(format!(
                "placed schema `{schema}` must be a nonempty, non-generic transparent record"
            )));
            continue;
        }

        let synthetic_name = format!("Placed<{policy},{schema}>");
        rewrites.push(PendingRewrite {
            type_reference,
            synthetic_name: synthetic_name.clone(),
        });
        schemas
            .entry(schema.clone())
            .or_insert_with(|| SchemaRecord {
                fields: fields.clone(),
            });
        if !applications
            .iter()
            .any(|application: &Application| application.synthetic_name == synthetic_name)
        {
            applications.push(Application {
                synthetic_name,
                policy,
                schema,
            });
        }
    }
    if diagnostics.is_empty() {
        Ok((applications, rewrites, schemas))
    } else {
        Err(diagnostics)
    }
}

fn named_argument(syntax: &SyntaxTrees, handle: TypeReferenceHandle) -> Option<String> {
    match syntax.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => Some(name.as_str().to_owned()),
        _ => None,
    }
}

fn synthesize_probe_records(
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
        push_record(syntax, &application.synthetic_name, members);
    }
    rewrite_applications(syntax, rewrites);
}

fn synthesize_exact_records(
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
            syntax.push_root_item(Item::Data(DataDefinition {
                name: Identifier::generated(accessor_name.clone()),
                supply_mode: DataSupplyMode::BoundaryOpaque,
                lifetime_parameters: Vec::new(),
                type_parameters: HandleSpan::empty(),
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
        push_record(syntax, &application.synthetic_name, members);
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

fn retire_accessor_templates(syntax: &mut SyntaxTrees) {
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

fn accessor_operations(access: &FieldAccess) -> Vec<&'static str> {
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

fn push_record(syntax: &mut SyntaxTrees, name: &str, members: Vec<DataMember>) {
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
        name: Identifier::generated(name),
        supply_mode: DataSupplyMode::CheckedShape,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
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
