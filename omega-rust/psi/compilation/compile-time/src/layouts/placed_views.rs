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

use crate::build_time_evaluation::EvaluationDomain;
use access_plans::PlacementPlanId;
use diagnostics::Diagnostic;
use language_semantics::DataSupplyMode;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{DataField, DataMember, Item};
use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use typed_trees::TypedTrees;

mod applied_results;
mod plan_installation;
mod record_synthesis;
mod template_applications;

use template_applications::scoped_template_applications;

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
    pub invocation_sources: Vec<source::SourceSpan>,
    template_source: source::SourceSpan,
    policy_source: source::SourceSpan,
    policy_machine_source: source::SourceSpan,
    schema_source: source::SourceSpan,
}

/// Compilation-local declaration custody. Coordinates never become a normalized
/// placement or ABI identity; generated labels only distinguish resolver roots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct DeclarationKey(usize, usize, usize);

impl From<source::SourceSpan> for DeclarationKey {
    fn from(source: source::SourceSpan) -> Self {
        Self(source.source_id.0, source.span.start, source.span.end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ApplicationKey {
    template: DeclarationKey,
    policy: DeclarationKey,
    schema: DeclarationKey,
}

#[derive(Clone)]
struct SchemaRecord {
    fields: Vec<DataField>,
}

#[derive(Clone)]
struct DataCandidate {
    name: String,
    source: source::SourceSpan,
    is_public: bool,
    plain: bool,
    supply_mode: DataSupplyMode,
    fields: Vec<DataField>,
    member_count: usize,
}

#[derive(Clone)]
struct PolicyMachineCandidate {
    machine_source: source::SourceSpan,
}

#[derive(Debug, Clone)]
struct Application {
    key: ApplicationKey,
    origin: TypeReferenceHandle,
    synthetic_name: String,
    policy: String,
    schema: String,
    template_source: source::SourceSpan,
    policy_source: source::SourceSpan,
    policy_machine_source: source::SourceSpan,
    schema_source: source::SourceSpan,
    generated_is_public: bool,
    invocation_sources: Vec<source::SourceSpan>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingRewrite {
    pub(crate) type_reference: TypeReferenceHandle,
    pub(crate) synthetic_name: String,
}

type Discovery = (
    Vec<Application>,
    Vec<PendingRewrite>,
    BTreeMap<DeclarationKey, SchemaRecord>,
);

pub fn desugar_placed_views(
    syntax: &mut SyntaxTrees,
    context: crate::BuildTimeSources<'_>,
) -> Result<Vec<PlacedViewRecord>, Vec<Diagnostic>> {
    let crate::BuildTimeSources {
        sources,
        source_scoped_top_level_bindings,
        selection_authority,
    } = context;
    let owner_requests = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine) => machine
                .attached_data
                .as_ref()
                .map(|owner| (machine.name.source_span(), owner.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let owner_declarations =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::select_data_declarations(
            syntax,
            sources.clone(),
            source_scoped_top_level_bindings.to_vec(),
            &owner_requests
                .iter()
                .map(|(_, owner)| owner.clone())
                .collect::<Vec<_>>(),
        )?;
    let selected_owners = owner_requests
        .into_iter()
        .zip(owner_declarations)
        .map(|((machine, _), owner)| (machine, owner))
        .collect::<Vec<_>>();
    let mut selected_arguments = applied_results::retain_applied_results(
        syntax,
        sources.clone(),
        source_scoped_top_level_bindings,
    )?;
    // Header selection retains nominal custody without executing a body.
    // Generated result demands already carry their selected template identity.
    let requests = syntax
        .type_references
        .generic_nodes()
        .into_iter()
        .filter_map(|reference| {
            let TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } = syntax.type_references.type_reference(reference)
            else {
                return None;
            };
            Some((
                reference,
                base_name.clone(),
                syntax
                    .type_references
                    .type_reference_handles(*arguments)
                    .to_vec(),
            ))
        })
        .collect::<Vec<_>>();
    let names = requests
        .iter()
        .flat_map(|(reference, base, arguments)| {
            let mut names = vec![(*reference, base.clone())];
            for argument in arguments {
                if let TypeReferenceNode::Named(name) =
                    syntax.type_references.type_reference(*argument)
                {
                    names.push((*argument, name.clone()));
                }
            }
            names
        })
        .filter(|(reference, _)| {
            !selected_arguments
                .iter()
                .any(|selected| selected.reference == *reference)
        })
        .collect::<Vec<_>>();
    let declarations =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::select_data_declarations(
            syntax,
            sources.clone(),
            source_scoped_top_level_bindings.to_vec(),
            &names
                .iter()
                .map(|(_, name)| name.clone())
                .collect::<Vec<_>>(),
        )?;
    for ((reference, _), declaration) in names.into_iter().zip(declarations) {
        selected_arguments.push(applied_results::SelectedArgument {
            reference,
            declaration,
        });
    }
    let (applications, rewrites, schemas) = discover_applications(
        syntax,
        sources.as_deref(),
        selection_authority.as_deref(),
        &selected_arguments,
        &selected_owners,
    )?;
    if applications.is_empty() {
        // The generic boundary templates exist only as compiler input for
        // cloning exact accessor machines. Leaving them active when a program
        // has no `Placed<P, T>` application makes ordinary static trait
        // selection consider `PlacedField<T>` as a real generic `Readable`,
        // `DestructiveRead`, or `Writable` provider.
        retire_accessor_templates(syntax, sources.as_deref());
        return Ok(Vec::new());
    }

    let mut probe = syntax.clone();
    synthesize_probe_records(
        &mut probe,
        &applications,
        &rewrites,
        &schemas,
        sources.as_deref(),
    )?;
    let mut probe = crate::syntax_probes::normalize_generic_data(
        probe,
        sources.clone(),
        source_scoped_top_level_bindings,
        None,
    )?;
    let probe_plan_laid = crate::desugar_plan_laid_value_types(&mut probe)?;
    let frontier = syntax_trees_to_symbol_resolved_trees::resolve_deferred(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: probe,
            sources: sources.clone(),
            top_level_bindings: source_scoped_top_level_bindings.to_vec(),
        },
    )?;
    let (mut typed, domain, selected_layouts) =
        prepare_placement_probe(frontier, &applications, &probe_plan_laid)?;
    crate::const_evaluation::const_lengths::evaluate_const_array_lengths_in(
        &mut typed,
        selection_authority.clone(),
        &domain,
    )?;
    crate::const_evaluation::const_domain_facts::evaluate_selected_domain_facts(
        &mut typed,
        selection_authority.clone(),
        crate::SelectedBuildTimeOperators::default(),
        &domain,
    )?;
    super::plan_laid::compute_plan_laid_layouts_in(
        &mut typed,
        &selected_layouts,
        selection_authority.clone(),
        &domain,
    )?;
    let execution = domain.execution_program(&typed);

    let mut plans = BTreeMap::new();
    for application in &applications {
        let (policy_machine, schema) = selected_placement_declarations(
            &execution,
            application.policy_machine_source,
            application.schema_source,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        admit_policy_invocations(
            &execution,
            policy_machine,
            &application.invocation_sources,
            selection_authority.clone(),
        )
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "placed view `{}`: {reason}",
                application.synthetic_name
            ))]
        })?;
        let plan = super::access_plans::compute_selected_placement_plan(
            &execution,
            policy_machine,
            schema,
        )
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "placed view `{}`: {reason}",
                application.synthetic_name
            ))]
        })?;
        plans.insert(application.key, plan);
    }

    synthesize_exact_records(
        syntax,
        &applications,
        &rewrites,
        &schemas,
        &plans,
        sources.as_deref(),
    )?;
    Ok(applications
        .into_iter()
        .map(|application| {
            let plan = &plans[&application.key];
            PlacedViewRecord {
                synthetic_name: application.synthetic_name,
                policy_machine: format!("{}::plan", application.policy),
                schema_data: application.schema,
                normalized_placement: plan.identity(),
                invocation_sources: application.invocation_sources,
                template_source: application.template_source,
                policy_source: application.policy_source,
                policy_machine_source: application.policy_machine_source,
                schema_source: application.schema_source,
            }
        })
        .collect())
}

/// Reflection consumes a schema declaration even when the policy's generic
/// `Schema` parameter gives no ordinary call edge to that declaration. Pending
/// plan-laid records likewise contribute policy dependencies before their typed
/// layout rows exist. Close those edges from selected producers only; unrelated
/// bodies remain in the immutable frontier, not in the executable probe.
fn prepare_placement_probe(
    frontier: syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
    applications: &[Application],
    records: &[crate::PlanLaidRecord],
) -> Result<(TypedTrees, EvaluationDomain, Vec<crate::PlanLaidRecord>), Vec<Diagnostic>> {
    let mut machines = applications
        .iter()
        .map(|application| application.policy_machine_source)
        .collect::<Vec<_>>();
    let mut declarations = Vec::new();
    for source in applications
        .iter()
        .flat_map(|application| [application.policy_source, application.schema_source])
    {
        let mut candidates = frontier
            .trees()
            .roots
            .data_definitions
            .iter()
            .filter(|data| {
                frontier.trees().symbols.symbol_source_span(data.symbol) == Some(source)
            });
        let (Some(declaration), None) = (candidates.next(), candidates.next()) else {
            return Err(vec![
                Diagnostic::error("placed-view probe lost its unique reflected declaration")
                    .with_source_span(source),
            ]);
        };
        if !declarations.contains(&declaration.symbol) {
            declarations.push(declaration.symbol);
        }
    }
    let mut selected = Vec::new();
    loop {
        let (typed, domain) = crate::syntax_probes::prepare_evaluation(
            frontier.clone(),
            &machines,
            &[],
            &declarations,
        )?;
        let previous_count = selected.len();
        for record in records {
            if selected.contains(record) || !domain.contains_data(&typed, &record.synthetic_name)? {
                continue;
            }
            let mut policies = typed
                .machines()
                .iter()
                .filter(|machine| machine.name.as_str() == record.policy_machine);
            let (Some(policy), None) = (policies.next(), policies.next()) else {
                return Err(vec![Diagnostic::error(format!(
                    "plan-laid probe `{}` lost its unique policy declaration",
                    record.synthetic_name,
                ))]);
            };
            let mut schemas = typed
                .data_definitions()
                .iter()
                .filter(|definition| definition.name.as_str() == record.schema_data);
            let (Some(schema), None) = (schemas.next(), schemas.next()) else {
                return Err(vec![Diagnostic::error(format!(
                    "plan-laid probe `{}` lost its unique schema declaration",
                    record.synthetic_name,
                ))]);
            };
            let Some(policy_source) = typed.symbols.symbol_source_span(policy.symbol) else {
                return Err(vec![Diagnostic::error(
                    "plan-laid probe lost its policy source",
                )]);
            };
            if !machines.contains(&policy_source) {
                machines.push(policy_source);
            }
            // A normalized schema can be generated and have no authored span.
            // These records were synthesized before the retained frontier was
            // indexed, so their exact declaration symbols are stable roots.
            if !declarations.contains(&schema.symbol) {
                declarations.push(schema.symbol);
            }
            selected.push(record.clone());
        }
        if selected.len() == previous_count {
            return Ok((typed, domain, selected));
        }
    }
}

pub(crate) fn validate_placed_view_plans_in(
    typed: &mut TypedTrees,
    records: &[PlacedViewRecord],
    selection_authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &crate::build_time_evaluation::EvaluationDomain,
) -> Result<(), Vec<Diagnostic>> {
    if records.is_empty() {
        return Ok(());
    }
    let execution = domain.execution_program(typed);
    for record in records {
        let (policy_machine, schema) = selected_placement_declarations(
            &execution,
            record.policy_machine_source,
            record.schema_source,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        admit_policy_invocations(
            &execution,
            policy_machine,
            &record.invocation_sources,
            selection_authority.clone(),
        )
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "placed view `{}`: {reason}",
                record.synthetic_name
            ))]
        })?;
        let plan = super::access_plans::compute_selected_placement_plan(
            &execution,
            policy_machine,
            schema,
        )
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

fn selected_placement_declarations(
    typed: &TypedTrees,
    policy_source: source::SourceSpan,
    schema_source: source::SourceSpan,
) -> Result<
    (
        &typed_trees::machine::Machine,
        &typed_trees::data::DataDefinition,
    ),
    String,
> {
    let mut machines = typed
        .machines()
        .iter()
        .filter(|machine| typed.symbols.symbol_source_span(machine.symbol) == Some(policy_source));
    let (Some(machine), None) = (machines.next(), machines.next()) else {
        return Err("placed-view evaluation lost its unique selected policy machine".to_owned());
    };
    let mut schemas = typed
        .data_definitions()
        .iter()
        .filter(|schema| typed.symbols.symbol_source_span(schema.symbol) == Some(schema_source));
    let (Some(schema), None) = (schemas.next(), schemas.next()) else {
        return Err(
            "placed-view evaluation lost its unique selected schema declaration".to_owned(),
        );
    };
    Ok((machine, schema))
}

fn admit_policy_invocations(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    invocation_sources: &[source::SourceSpan],
    selection_authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), String> {
    let Some(selection_authority) = selection_authority else {
        return Ok(());
    };
    let admission = crate::BuildTimeAdmissionPlan::infer(typed, Some(selection_authority));
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
    sources: Option<&source::SourceMap>,
    selection_authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    selected_arguments: &[applied_results::SelectedArgument],
    selected_owners: &[(source::SourceSpan, Result<source::SourceSpan, String>)],
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
    let deferred = scoped_template_applications(syntax, selected_owners);
    for type_reference in syntax.tables.type_references.generic_nodes() {
        if deferred.contains(&type_reference) {
            continue;
        }
        let TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } = syntax.tables.type_references.type_reference(type_reference)
        else {
            unreachable!("generic_nodes returned a non-generic node");
        };
        let template = selected_arguments
            .iter()
            .find(|selected| selected.reference == type_reference)
            .and_then(|selected| selected.declaration.as_ref().ok().copied());
        let canonical = template
            .and_then(|declaration| sources.and_then(|sources| sources.file_at(declaration)))
            .is_some_and(|source| {
                source.origin == source::SourceOrigin::Toolchain
                    && source.path.strip_prefix(&source.package_root).ok()
                        == Some(std::path::Path::new("layout.omg"))
            })
            && template.is_some_and(|declaration| {
                data.iter().any(|candidate| {
                    candidate.source == declaration
                        && candidate.name.rsplit("::").next() == Some("Placed")
                })
            });
        if !canonical && (sources.is_some() || base_name.as_str() != "Placed") {
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
        let Some((mut policy, policy_source)) = named_argument(syntax, *policy_handle) else {
            diagnostics.push(Diagnostic::error(
                "the first `Placed` argument must be a nominal placement-policy data name",
            ));
            continue;
        };
        let Some((mut schema, schema_use_source)) = named_argument(syntax, *schema_handle) else {
            diagnostics.push(Diagnostic::error(
                "the second `Placed` argument must be a plain schema data name",
            ));
            continue;
        };
        let mut selection_failed = false;
        for (handle, reference) in [
            (*policy_handle, policy_source),
            (*schema_handle, schema_use_source),
        ] {
            match selected_arguments
                .iter()
                .find(|selected| selected.reference == handle)
            {
                Some(selected) => {
                    if let Err(reason) = &selected.declaration {
                        diagnostics
                            .push(Diagnostic::error(reason.clone()).with_source_span(reference));
                        selection_failed = true;
                    }
                }
                None if sources.is_some() => {
                    diagnostics.push(
                        Diagnostic::error(
                            "closed placement operand has no retained nominal selection",
                        )
                        .with_source_span(reference),
                    );
                    selection_failed = true;
                }
                None => {}
            }
        }
        if selection_failed {
            continue;
        }
        let policy_candidates = data
            .iter()
            .filter(|candidate| {
                selected_arguments
                    .iter()
                    .find(|selected| selected.reference == *policy_handle)
                    .map_or(sources.is_none() && candidate.name == policy, |selected| {
                        selected
                            .declaration
                            .as_ref()
                            .is_ok_and(|declaration| candidate.source == *declaration)
                    })
            })
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
        policy = policy_data.name.clone();
        let policy_machines = placement_policies
            .iter()
            .filter(|candidate| {
                selected_owners.iter().any(|(machine, owner)| {
                    *machine == candidate.machine_source
                        && owner
                            .as_ref()
                            .is_ok_and(|owner| *owner == policy_data.source)
                }) && sources.is_none_or(|sources| {
                    sources.same_checked_instance(policy_data.source, candidate.machine_source)
                })
            })
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
            .filter(|candidate| {
                selected_arguments
                    .iter()
                    .find(|selected| selected.reference == *schema_handle)
                    .map_or(sources.is_none() && candidate.name == schema, |selected| {
                        selected
                            .declaration
                            .as_ref()
                            .is_ok_and(|declaration| candidate.source == *declaration)
                    })
            })
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
        schema = schema_data.name.clone();
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

        let key = ApplicationKey {
            template: template.unwrap_or_default().into(),
            policy: policy_data.source.into(),
            schema: schema_data.source.into(),
        };
        // Repeated uses share only an exact declaration tuple. A same-spelled
        // declaration in another checked instance gets a separate resolver
        // root; the label is not its authority or normalized plan identity.
        let synthetic_name = if let Some(application) = applications
            .iter()
            .find(|application: &&Application| application.key == key)
        {
            application.synthetic_name.clone()
        } else {
            let base = format!("Placed<{policy},{schema}>");
            if applications
                .iter()
                .any(|application| application.synthetic_name == base)
            {
                format!("{base}#{}", applications.len())
            } else {
                base
            }
        };
        rewrites.push(PendingRewrite {
            type_reference,
            synthetic_name: synthetic_name.clone(),
        });
        schemas.entry(key.schema).or_insert_with(|| SchemaRecord {
            fields: schema_data.fields.clone(),
        });
        if let Some(application) = applications
            .iter_mut()
            .find(|application: &&mut Application| application.key == key)
        {
            if !application.invocation_sources.contains(&policy_source) {
                application.invocation_sources.push(policy_source);
            }
        } else {
            applications.push(Application {
                key,
                origin: type_reference,
                synthetic_name,
                policy,
                schema,
                template_source: template.unwrap_or_default(),
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
    sources: Option<&source::SourceMap>,
    left: source::SourceSpan,
    right: source::SourceSpan,
) -> bool {
    sources.is_none_or(|sources| sources.same_package(left, right))
}

fn declaration_is_visible(
    sources: Option<&source::SourceMap>,
    requester: source::SourceSpan,
    declaration: source::SourceSpan,
    is_public: bool,
) -> bool {
    is_public || same_package(sources, requester, declaration)
}

fn require_declaration_selection_authority(
    sources: Option<&source::SourceMap>,
    authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    requester: source::SourceSpan,
    declaration: source::SourceSpan,
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
    if requester_file.origin == source::SourceOrigin::Toolchain
        || declaration_file.origin == source::SourceOrigin::Toolchain
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
) -> Option<(String, source::SourceSpan)> {
    match syntax.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => Some((name.as_str().to_owned(), name.source_span())),
        _ => None,
    }
}
