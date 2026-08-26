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

use std::collections::BTreeMap;
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
    policy_source: psi_source::SourceSpan,
    policy_machine_source: psi_source::SourceSpan,
    schema_source: psi_source::SourceSpan,
}

#[derive(Clone)]
struct SchemaRecord {
    fields: Vec<DataField>,
}

#[derive(Clone)]
struct DataCandidate {
    name: String,
    source: psi_source::SourceSpan,
    is_public: bool,
    plain: bool,
    supply_mode: DataSupplyMode,
    fields: Vec<DataField>,
    member_count: usize,
}

#[derive(Clone)]
struct PolicyMachineCandidate {
    policy: String,
    machine_source: psi_source::SourceSpan,
}

#[derive(Debug, Clone)]
struct Application {
    synthetic_name: String,
    policy: String,
    schema: String,
    policy_source: psi_source::SourceSpan,
    policy_machine_source: psi_source::SourceSpan,
    schema_source: psi_source::SourceSpan,
    generated_is_public: bool,
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
    let (applications, rewrites, schemas) =
        discover_applications(syntax, sources.as_deref(), selection_authority.as_deref())?;
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
                policy_source: application.policy_source,
                policy_machine_source: application.policy_machine_source,
                schema_source: application.schema_source,
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

fn discover_applications(
    syntax: &SyntaxTrees,
    sources: Option<&psi_source::SourceMap>,
    selection_authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
) -> Result<Discovery, Vec<Diagnostic>> {
    let mut data = Vec::new();
    let mut placement_policies = Vec::new();
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
                data.push(DataCandidate {
                    name: definition.name.as_str().to_owned(),
                    source: definition.name.source_span(),
                    is_public: definition.is_public,
                    plain: definition.type_parameters.is_empty(),
                    supply_mode: definition.supply_mode,
                    fields,
                    member_count: syntax.tables.items.data_members(definition.members).len(),
                });
            }
            Item::Machine(machine)
                if machine.attached_data.as_ref().is_some_and(|attached| {
                    machine.name.as_str() == format!("{}::plan", attached.as_str())
                }) =>
            {
                placement_policies.push(PolicyMachineCandidate {
                    policy: machine
                        .attached_data
                        .as_ref()
                        .expect("matched attached machine")
                        .as_str()
                        .to_owned(),
                    machine_source: machine.name.source_span(),
                });
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
        let Some((schema, schema_use_source)) = named_argument(syntax, *schema_handle) else {
            diagnostics.push(Diagnostic::error(
                "the second `Placed` argument must be a plain schema data name",
            ));
            continue;
        };
        let policy_candidates = data
            .iter()
            .filter(|candidate| candidate.name == policy)
            .collect::<Vec<_>>();
        let [policy_data] = policy_candidates.as_slice() else {
            diagnostics.push(
                Diagnostic::error(if policy_candidates.is_empty() {
                    format!("`Placed<{policy}, {schema}>` names an unknown placement policy `{policy}`")
                } else {
                    format!("`Placed<{policy}, {schema}>` cannot select one exact placement policy `{policy}`")
                })
                .with_source_span(policy_source),
            );
            continue;
        };
        let policy_machines = placement_policies
            .iter()
            .filter(|candidate| candidate.policy == policy)
            .collect::<Vec<_>>();
        let [policy_machine] = policy_machines.as_slice() else {
            diagnostics.push(
                Diagnostic::error(format!(
                    "`Placed<{policy}, {schema}>` must select one exact `{policy}::plan` machine"
                ))
                .with_source_span(policy_source),
            );
            continue;
        };
        if !same_package(sources, policy_data.source, policy_machine.machine_source) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "`Placed<{policy}, {schema}>` cannot pair placement policy `{policy}` with a `{policy}::plan` machine from another package"
                ))
                .with_source_span(policy_source),
            );
            continue;
        }
        let schema_candidates = data
            .iter()
            .filter(|candidate| candidate.name == schema)
            .collect::<Vec<_>>();
        let [schema_data] = schema_candidates.as_slice() else {
            diagnostics.push(
                Diagnostic::error(if schema_candidates.is_empty() {
                    format!("`Placed<{policy}, {schema}>` names an unknown schema `{schema}`")
                } else {
                    format!(
                        "`Placed<{policy}, {schema}>` cannot select one exact schema `{schema}`"
                    )
                })
                .with_source_span(schema_use_source),
            );
            continue;
        };
        if !declaration_is_visible(
            sources,
            policy_source,
            policy_data.source,
            policy_data.is_public,
        ) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "`Placed<{policy}, {schema}>` cannot use private placement policy `{policy}` from another package"
                ))
                .with_source_span(policy_source),
            );
            continue;
        }
        if !declaration_is_visible(
            sources,
            schema_use_source,
            schema_data.source,
            schema_data.is_public,
        ) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "`Placed<{policy}, {schema}>` cannot use private schema `{schema}` from another package"
                ))
                .with_source_span(schema_use_source),
            );
            continue;
        }
        if let Err(reason) = require_declaration_selection_authority(
            sources,
            selection_authority,
            policy_source,
            policy_data.source,
            "placement policy",
        ) {
            diagnostics.push(Diagnostic::error(reason).with_source_span(policy_source));
            continue;
        }
        if let Err(reason) = require_declaration_selection_authority(
            sources,
            selection_authority,
            schema_use_source,
            schema_data.source,
            "placed schema",
        ) {
            diagnostics.push(Diagnostic::error(reason).with_source_span(schema_use_source));
            continue;
        }
        // `Placed<P, S>` is erased before ordinary authored type-selection
        // capture. Until that erasure retains interface exposure directly,
        // require both nominal inputs to be publishable even for a local use;
        // otherwise a public signature could launder a private declaration
        // through the source-free compiler shell.
        if !policy_data.is_public {
            diagnostics.push(
                Diagnostic::error(format!(
                    "placement policy `{policy}` used by `Placed<{policy}, {schema}>` must be public"
                ))
                .with_source_span(policy_source),
            );
            continue;
        }
        if !schema_data.is_public {
            diagnostics.push(
                Diagnostic::error(format!(
                    "schema `{schema}` used by `Placed<{policy}, {schema}>` must be public"
                ))
                .with_source_span(schema_use_source),
            );
            continue;
        }
        if !schema_data.plain
            || schema_data.supply_mode == DataSupplyMode::BoundaryOpaque
            || schema_data.fields.is_empty()
            || schema_data.fields.len() != schema_data.member_count
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
                fields: schema_data.fields.clone(),
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
                policy_source: policy_data.source,
                policy_machine_source: policy_machine.machine_source,
                schema_source: schema_data.source,
                generated_is_public: policy_data.is_public && schema_data.is_public,
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

fn same_package(
    sources: Option<&psi_source::SourceMap>,
    left: psi_source::SourceSpan,
    right: psi_source::SourceSpan,
) -> bool {
    sources.is_none_or(|sources| sources.same_package(left, right))
}

fn declaration_is_visible(
    sources: Option<&psi_source::SourceMap>,
    requester: psi_source::SourceSpan,
    declaration: psi_source::SourceSpan,
    is_public: bool,
) -> bool {
    is_public || same_package(sources, requester, declaration)
}

fn require_declaration_selection_authority(
    sources: Option<&psi_source::SourceMap>,
    authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    requester: psi_source::SourceSpan,
    declaration: psi_source::SourceSpan,
    context: &str,
) -> Result<(), String> {
    let Some(authority) = authority else {
        return Ok(());
    };
    let Some(sources) = sources else {
        return Err(format!(
            "{context} selection lacks compiler-owned source/package provenance"
        ));
    };
    let Some(requester_file) = sources.file_at(requester) else {
        return Err(format!(
            "{context} selection lacks requesting source/package provenance"
        ));
    };
    let Some(declaration_file) = sources.file_at(declaration) else {
        return Err(format!(
            "{context} selection lacks declaration source/package provenance"
        ));
    };
    if requester_file.origin == psi_source::SourceOrigin::Toolchain
        || declaration_file.origin == psi_source::SourceOrigin::Toolchain
    {
        return Ok(());
    }
    let Some(requester_package) = requester_file.package_identity else {
        return Err(format!(
            "{context} selection has user source without reconciled requesting package custody"
        ));
    };
    let Some(declaration_package) = declaration_file.package_identity else {
        return Err(format!(
            "{context} selection has user declaration without reconciled package custody"
        ));
    };
    if authority.allows_declaration_selection(requester_package, declaration_package) {
        return Ok(());
    }
    Err(format!(
        "{context} selects package {} from package {} without direct dependency authority",
        authority.package_label(declaration_package),
        authority.package_label(requester_package),
    ))
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
