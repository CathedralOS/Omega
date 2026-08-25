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
use std::sync::Arc;

use psi_access_plans::PlacementPlanId;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::DataSupplyMode;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::item::{DataField, DataMember, Item};
use psi_syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use psi_typed_trees::TypedTrees;

mod plan_installation;
mod record_synthesis;

use plan_installation::install_placed_view_plan;
use record_synthesis::{
    accessor_operations, retire_accessor_templates, synthesize_exact_records,
    synthesize_probe_records,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedViewRecord {
    pub synthetic_name: String,
    pub policy_machine: String,
    pub schema_data: String,
    pub normalized_placement: PlacementPlanId,
    pub invocation_sources: Vec<psi_source::SourceSpan>,
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
    invocation_sources: Vec<psi_source::SourceSpan>,
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
    desugar_placed_views_with_optional_sources(syntax, None, None)
}

pub(crate) fn desugar_placed_views_with_optional_sources(
    syntax: &mut SyntaxTrees,
    sources: Option<Arc<psi_source::SourceMap>>,
    selection_authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
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
    let resolved = crate::lower_probe_with_optional_sources(&probe, sources)?;
    let mut typed =
        psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .map_err(|diagnostic| vec![diagnostic])?;
    crate::evaluate_const_array_lengths_with_authority(&mut typed, selection_authority.clone())?;
    crate::evaluate_const_domain_facts_with_authority(&mut typed, selection_authority.clone())?;
    crate::compute_plan_laid_layouts_with_authority(
        &mut typed,
        &probe_plan_laid,
        selection_authority.clone(),
    )?;

    let mut plans = BTreeMap::new();
    for application in &applications {
        let policy_machine = format!("{}::plan", application.policy);
        admit_policy_invocations(
            &typed,
            &policy_machine,
            &application.invocation_sources,
            selection_authority.clone(),
        )
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "placed view `{}`: {reason}",
                application.synthetic_name
            ))]
        })?;
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
                invocation_sources: application.invocation_sources,
            }
        })
        .collect())
}

pub fn validate_placed_view_plans(
    typed: &mut TypedTrees,
    records: &[PlacedViewRecord],
) -> Result<(), Vec<Diagnostic>> {
    validate_placed_view_plans_with_authority(typed, records, None)
}

pub fn validate_placed_view_plans_with_authority(
    typed: &mut TypedTrees,
    records: &[PlacedViewRecord],
    selection_authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    for record in records {
        admit_policy_invocations(
            typed,
            &record.policy_machine,
            &record.invocation_sources,
            selection_authority.clone(),
        )
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "placed view `{}`: {reason}",
                record.synthetic_name
            ))]
        })?;
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

fn admit_policy_invocations(
    typed: &TypedTrees,
    policy_machine: &str,
    invocation_sources: &[psi_source::SourceSpan],
    selection_authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), String> {
    let Some(selection_authority) = selection_authority else {
        return Ok(());
    };
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == policy_machine)
        .ok_or_else(|| format!("no machine named `{policy_machine}` exists"))?;
    let admission = crate::BuildTimeAdmissionPlan::infer_with_selection_authority(
        typed,
        Some(selection_authority),
    );
    for source in invocation_sources {
        admission.require_common_floor_for_invocation(
            typed,
            machine,
            crate::BuildTimeInvocationCustody::Source(*source),
        )?;
    }
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
        let Some((policy, policy_source)) = named_argument(syntax, *policy_handle) else {
            diagnostics.push(Diagnostic::error(
                "the first `Placed` argument must be a nominal placement-policy data name",
            ));
            continue;
        };
        let Some((schema, _)) = named_argument(syntax, *schema_handle) else {
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
        if let Some(application) = applications
            .iter_mut()
            .find(|application: &&mut Application| application.synthetic_name == synthetic_name)
        {
            if !application.invocation_sources.contains(&policy_source) {
                application.invocation_sources.push(policy_source);
            }
        } else {
            applications.push(Application {
                synthetic_name,
                policy,
                schema,
                invocation_sources: vec![policy_source],
            });
        }
    }
    if diagnostics.is_empty() {
        Ok((applications, rewrites, schemas))
    } else {
        Err(diagnostics)
    }
}

fn named_argument(
    syntax: &SyntaxTrees,
    handle: TypeReferenceHandle,
) -> Option<(String, psi_source::SourceSpan)> {
    match syntax.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => Some((name.as_str().to_owned(), name.source_span())),
        _ => None,
    }
}
