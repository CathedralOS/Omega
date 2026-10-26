use diagnostics::Diagnostic;
use layout_plans::LayoutPlacementReport;
use typed_trees::{
    PlanLaidBitField, PlanLaidBitFragment, PlanLaidGeometry, PlanLaidIntegerField, PlanLaidLayout,
    PlanLaidRealizations, PlanLaidRepeatedField, PlanLaidSchemaArgument, PlanLaidSchemaField,
    PlanLaidSchemaFieldKind, PlanLaidTargetLayout, TypedTrees,
};

use super::PlanLaidRecord;

enum ApplicationPrerequisites {
    Ready,
    LayoutQueries,
    Other(Vec<String>),
}

/// Bind source recipes once, before deciding which inputs can be evaluated.
/// A pending application is a real policy requirement, never ordinary packing.
fn bind_applications(
    typed: &mut TypedTrees,
    records: &[PlanLaidRecord],
) -> Result<Vec<symbols::SymbolHandle>, Vec<Diagnostic>> {
    let mut applications = Vec::with_capacity(records.len());
    for record in records {
        let missing = |role: &str| {
            vec![Diagnostic::error(format!(
                "plan-laid value type `{}` lost its exact {role}",
                record.synthetic_name
            ))]
        };
        let policy_name = record
            .policy_machine
            .strip_suffix("::plan")
            .unwrap_or(&record.policy_machine);
        let policy_symbol = typed
            .data_definition_by_name_str(policy_name)
            .map(|data| data.symbol)
            .ok_or_else(|| missing("nominal policy identity"))?;
        let policy_plan_machine_symbol = typed
            .realized_machine_named(&record.policy_machine)
            .map(|machine| machine.symbol)
            .ok_or_else(|| missing("policy plan machine"))?;
        let data = typed
            .data_definition_by_name_str(&record.synthetic_name)
            .ok_or_else(|| missing("synthesized data identity"))?;
        let data_symbol = data.symbol;
        let field_symbols = typed
            .live_data_fields(typed.data_members(data))
            .map(|field| field.symbol)
            .collect();
        let schema = typed
            .data_definition_by_name_str(&record.schema_data)
            .ok_or_else(|| missing("source schema identity"))?;
        let schema_symbol = schema.symbol;
        let schema_field_symbols = typed
            .live_data_fields(typed.data_members(schema))
            .map(|field| field.symbol)
            .collect();
        let application = PlanLaidLayout {
            data_name: record.synthetic_name.clone(),
            data_symbol,
            field_symbols,
            schema_symbol,
            schema_field_symbols,
            policy_symbol,
            policy_plan_machine_symbol,
            schema_argument: None,
            realizations: PlanLaidRealizations::Pending,
        };
        if let Some(previous) = typed.plan_laid_layout(data_symbol) {
            if previous.field_symbols != application.field_symbols
                || previous.schema_symbol != application.schema_symbol
                || previous.schema_field_symbols != application.schema_field_symbols
                || previous.policy_symbol != application.policy_symbol
                || previous.policy_plan_machine_symbol != application.policy_plan_machine_symbol
            {
                return Err(missing("retained policy/schema application binding"));
            }
        } else {
            typed.plan_laid_layouts.push(application);
        }
        applications.push(data_symbol);
    }
    Ok(applications)
}

/// Bind an application's own stored carrier once, while retaining the exact
/// authored operand. Physical consumers and catalog discovery then read that
/// association instead of independently recognizing `Policy<Schema>`.
fn bind_query_carriers(typed: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    use typed_trees::expression::ExpressionNode;
    use typed_trees::types::TypeReferenceNode;

    let expressions = typed_trees::references::collect_program_references(typed).expressions;
    for expression in expressions {
        let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
            continue;
        };
        let Some(query) = &call.layout_query else {
            continue;
        };
        let operand = query.operand.type_reference;
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            lifetime_arguments,
            ..
        } = typed.type_reference_table.type_reference(operand)
        else {
            continue;
        };
        let [schema] = typed
            .type_reference_table
            .type_reference_handles(*arguments)
        else {
            continue;
        };
        if !lifetime_arguments.is_empty() {
            continue;
        }
        let schema = typed.type_reference_symbol(*schema);
        let mut matches = typed.plan_laid_layouts.iter().filter(|layout| {
            layout.policy_symbol == *base_symbol && layout.schema_symbol == schema
        });
        let Some(layout) = matches.next() else {
            continue;
        };
        if matches.next().is_some() {
            return Err(vec![Diagnostic::error(
                "layout query has ambiguous policy/schema application custody",
            )]);
        }
        let declaration = typed
            .data_definition_by_symbol(layout.data_symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "layout query lost its bound application carrier",
                )]
            })?
            .clone();
        let carrier = typed
            .type_reference_table
            .find_named_type_reference(declaration.symbol)
            .unwrap_or_else(|| {
                typed.type_reference_table.insert(TypeReferenceNode::Named {
                    symbol: declaration.symbol,
                    name: declaration.name.clone(),
                })
            });
        if typed.data_application_definition(operand) != Some(&declaration) {
            typed.retain_data_application_definition(operand, declaration, true);
        }
        if typed
            .expression_table
            .call_node(expression)
            .and_then(|call| call.layout_query.as_ref())
            .is_some_and(|query| query.carrier != carrier)
        {
            let ExpressionNode::Call(call) = typed.expression_table.expression_mut(expression)
            else {
                unreachable!("query binding preserves the source call");
            };
            call.layout_query.as_mut().expect("formed query").carrier = carrier;
        }
    }
    Ok(())
}

/// Settle ready application inputs independently of unrelated query values.
/// Results immediately become prerequisites of later jobs in this same owner.
pub(crate) fn prepare_plan_laid_layouts_in(
    typed: &mut TypedTrees,
    records: &[PlanLaidRecord],
    authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &mut crate::build_time_evaluation::EvaluationDomain,
    queries: Option<&dyn crate::LayoutQueryPreparation>,
) -> Result<Vec<PlanLaidRecord>, Vec<Diagnostic>> {
    if records.is_empty() {
        bind_query_carriers(typed)?;
        return Ok(Vec::new());
    }
    let applications = bind_applications(typed, records)?;
    bind_query_carriers(typed)?;
    let library_contexts = queries
        .map(|queries| queries.library_contexts())
        .unwrap_or_default();
    let mut context_roots = queries
        .map(|queries| queries.context_roots())
        .transpose()?
        .unwrap_or_default();
    // Every authored body must form even when no image for its target is
    // requested. Portable bodies inherit the context of exact calling roots.
    for machine in typed.machines() {
        if queries.is_some()
            && let Some(target) = typed.machine_authored_target(machine)
        {
            context_roots.push((typed_trees::name::Identifier::from(target), machine.symbol));
        }
    }
    let mut pending: Vec<_> = records.iter().zip(applications).collect();
    loop {
        // Several slots may select the same body. Its dependency closure is
        // independent of the slot's context and is prepared once per root.
        let mut context_demands = Vec::new();
        for (_, root) in &context_roots {
            if !context_demands.iter().any(|(owner, _)| owner == root) {
                context_demands.push((
                    *root,
                    crate::syntax_probes::PreparedEvaluationInputs::domain_for_program(
                        typed,
                        &[*root],
                        &[],
                    ),
                ));
            }
        }
        // An open library declaration supplies capabilities only to its own
        // dependency closure. A private helper used exclusively by guarded
        // bodies inherits those guards; merely sharing the catalog does not
        // make its policy run in every physical context. Public declarations
        // remain open even when a guarded caller also reaches them. Unreached
        // private declarations still owe ordinary source formation.
        let library_demand = if library_contexts.is_empty() {
            None
        } else {
            let roots: Vec<_> = typed_trees::references::collect_program_declarations(typed)
                .into_iter()
                .filter(|symbol| domain.contains(*symbol))
                .filter(|symbol| {
                    !typed.machine(*symbol).is_some_and(|machine| {
                        typed.machine_authored_target(machine).is_some()
                            || machine.target_family_interface
                    })
                })
                .filter(|symbol| {
                    typed_trees::visibility::declaration_visibility(typed, *symbol)
                        .is_some_and(|visibility| visibility.is_public())
                        || !context_demands
                            .iter()
                            .any(|(_, demand)| demand.contains(*symbol))
                })
                .collect();
            Some(
                crate::syntax_probes::PreparedEvaluationInputs::domain_for_program(
                    typed,
                    &roots,
                    &[],
                ),
            )
        };
        let input_revision = typed.type_reference_table.revision();
        let previous_work = domain.work().clone();
        let mut waiting = Vec::new();
        let mut completed = false;
        for (record, application) in pending {
            let prerequisites =
                application_prerequisites(typed, application, authority.clone(), domain, queries)?;
            let contexts = match (prerequisites, queries) {
                (ApplicationPrerequisites::Ready, _) => Vec::new(),
                (ApplicationPrerequisites::LayoutQueries, Some(queries)) => {
                    let layout = typed
                        .plan_laid_layout(application)
                        .expect("bound application");
                    if typed
                        .target_machine_families
                        .iter()
                        .any(|family| family.canonical == layout.policy_plan_machine_symbol)
                    {
                        // An authored family already owns its mandatory input
                        // roster. Each member is prepared under its own guard.
                        Vec::new()
                    } else {
                        let mut contexts: Vec<_> = context_roots
                            .iter()
                            .filter(|(_, root)| {
                                context_demands.iter().any(|(owner, demand)| {
                                    owner == root && demand.contains(application)
                                })
                            })
                            .map(|(target, _)| target.clone())
                            .collect();
                        let mandatory = contexts.clone();
                        if library_demand
                            .as_ref()
                            .is_some_and(|demand| demand.contains(application))
                        {
                            let selected = domain.demanded_roots(
                                typed,
                                &[layout.schema_symbol, layout.policy_plan_machine_symbol],
                            );
                            let mut pending_inputs = false;
                            for context in &library_contexts {
                                // Query work is selected, but layout lookup
                                // still needs the admitted catalog's installed
                                // applications and authored family identities.
                                // A policy query's generic operand does not
                                // independently root the synthesized value type.
                                let mut inputs = domain.execution_program(typed);
                                match queries.prepare_context(
                                    &mut inputs,
                                    selected.work(),
                                    context.as_str(),
                                )? {
                                    crate::LayoutContextInputs::Ready => {
                                        contexts.push(context.clone())
                                    }
                                    crate::LayoutContextInputs::Pending => pending_inputs = true,
                                    crate::LayoutContextInputs::Unavailable {
                                        application: dependency,
                                    } => {
                                        if mandatory.contains(context) {
                                            return Err(vec![Diagnostic::error(format!(
                                                "layout `{}` requires context `{context}`, but input layout `{}` has no member there",
                                                record.synthetic_name,
                                                typed.symbols.name(dependency),
                                            ))]);
                                        }
                                    }
                                }
                            }
                            if pending_inputs {
                                waiting.push((record, application));
                                continue;
                            }
                            if contexts.is_empty() {
                                return Err(vec![Diagnostic::error(format!(
                                    "layout `{}` has no physical context providing its query inputs",
                                    record.synthetic_name,
                                ))]);
                            }
                        }
                        contexts.sort_by(|left, right| left.as_str().cmp(right.as_str()));
                        contexts.dedup();
                        if contexts.is_empty() {
                            waiting.push((record, application));
                            continue;
                        }
                        contexts
                    }
                }
                _ => {
                    waiting.push((record, application));
                    continue;
                }
            };
            if install_plan_laid_layout(
                typed,
                record,
                application,
                authority.clone(),
                domain,
                queries,
                &contexts,
            )? {
                completed = true;
            } else {
                waiting.push((record, application));
            }
        }
        let inputs_changed = typed.type_reference_table.revision() != input_revision
            || domain.work() != &previous_work;
        if (!completed && !inputs_changed) || waiting.is_empty() {
            return Ok(waiting
                .into_iter()
                .map(|(record, _)| record.clone())
                .collect());
        }
        pending = waiting;
    }
}

pub(crate) fn compute_plan_laid_layouts_in(
    typed: &mut TypedTrees,
    records: &[PlanLaidRecord],
    authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &mut crate::build_time_evaluation::EvaluationDomain,
) -> Result<(), Vec<Diagnostic>> {
    let pending = prepare_plan_laid_layouts_in(typed, records, authority.clone(), domain, None)?;
    if pending.is_empty() {
        return Ok(());
    }
    let mut diagnostics = Vec::new();
    for record in pending {
        let data = typed
            .data_definition_by_name_str(&record.synthetic_name)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "pending layout lost its bound declaration",
                )]
            })?;
        let application = data.symbol;
        let prerequisites =
            match application_prerequisites(typed, application, authority.clone(), domain, None)? {
                ApplicationPrerequisites::Ready => Vec::new(),
                ApplicationPrerequisites::LayoutQueries => {
                    vec!["layout-query values in the policy input closure".to_owned()]
                }
                ApplicationPrerequisites::Other(prerequisites) => prerequisites,
            };
        diagnostics.push(Diagnostic::error(format!(
            "plan-laid value type `{}` has unresolved prerequisites: {}",
            record.synthetic_name,
            prerequisites.join(", ")
        )));
    }
    Err(diagnostics)
}

fn install_plan_laid_layout(
    typed: &mut TypedTrees,
    record: &PlanLaidRecord,
    application: symbols::SymbolHandle,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &crate::build_time_evaluation::EvaluationDomain,
    queries: Option<&dyn crate::LayoutQueryPreparation>,
    contexts: &[typed_trees::name::Identifier],
) -> Result<bool, Vec<Diagnostic>> {
    let execution = domain.execution_program(typed);
    // One continuation owns only its recorded applications. Other cohorts'
    // validated rows and arena coordinates remain part of this same program.
    // Publish the complete replacement only after every new result validates.
    let mut layouts = typed.plan_laid_layouts.clone();
    let mut target_layouts = typed.plan_laid_target_layouts.clone();
    let mut context_rosters = typed.plan_laid_contexts.clone();
    let mut captured_schema_fields = typed.plan_laid_schema_fields.clone();
    let data_symbol = application;
    let application = typed.plan_laid_layout(data_symbol).ok_or_else(|| {
        vec![Diagnostic::error(
            "layout job lost its bound policy application",
        )]
    })?;
    let policy_symbol = application.policy_symbol;
    let policy_plan_machine_symbol = application.policy_plan_machine_symbol;
    let field_symbols = application.field_symbols.clone();
    let schema = typed
        .data_definition_by_symbol(application.schema_symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "plan-laid value type `{}` lost its exact source schema identity",
                record.synthetic_name
            ))]
        })?;
    let schema_symbol = schema.symbol;
    let schema_fields = typed
        .live_data_fields(typed.data_members(schema))
        .map(|field| (field.name.as_str().to_owned(), field.type_reference))
        .collect::<Vec<_>>();
    let schema_field_symbols = typed
        .live_data_fields(typed.data_members(schema))
        .map(|field| field.symbol)
        .collect::<Vec<_>>();
    super::super::schema_reflection::require_invariant_schema(typed, schema_symbol, &[]).map_err(
        |reason| {
            vec![Diagnostic::error(format!(
                "plan-laid value type `{}`: {reason}",
                record.synthetic_name
            ))]
        },
    )?;
    let family = typed
        .target_machine_families
        .iter()
        .find(|family| family.canonical == policy_plan_machine_symbol);
    if family.is_none()
        && typed
            .machine_by_symbol(policy_plan_machine_symbol)
            .is_some_and(|machine| machine.target_family_interface)
    {
        return Err(vec![Diagnostic::error(format!(
            "plan-laid value type `{}`: policy family lost its structural roster",
            record.synthetic_name
        ))]);
    }
    let guarded_schema = (family.is_some() || !contexts.is_empty())
        .then(|| {
            let pending = execution
                .plan_laid_layouts
                .iter()
                .filter(|layout| !layout.is_materialized())
                .map(|layout| layout.data_symbol)
                .collect::<Vec<_>>();
            super::super::schema_reflection::require_materialized_schema(
                &execution,
                schema_symbol,
                &pending,
            )?;
            prepare_family_schema(&execution, schema_symbol, &record.schema_data)
        })
        .transpose()
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "plan-laid value type `{}`: {reason}",
                record.synthetic_name
            ))]
        })?;
    let schema_argument = guarded_schema.as_ref().map(|(fields, _, retained)| {
        debug_assert_eq!(fields.len(), retained.len());
        PlanLaidSchemaArgument {
            abi_version: 1,
            fields: captured_schema_fields.insert_many(retained.iter().cloned()),
        }
    });
    let evaluate = |machine_symbol, target: Option<&str>| {
        let mut contextual;
        let execution = if let (Some(queries), Some(target)) = (queries, target) {
            contextual = execution.clone();
            let selected = domain.demanded_roots(typed, &[schema_symbol, machine_symbol]);
            match queries.prepare_context(&mut contextual, selected.work(), target)? {
                crate::LayoutContextInputs::Ready => {}
                crate::LayoutContextInputs::Pending => return Ok(None),
                crate::LayoutContextInputs::Unavailable { application } => {
                    return Err(vec![Diagnostic::error(format!(
                        "policy input layout `{}` has no geometry in required context `{target}`",
                        typed.symbols.name(application),
                    ))]);
                }
            }
            if crate::const_evaluation::data_layout_queries::pending_layout_queries_need_operator_selection(
                &contextual, selected.work(),
            ) {
                return Ok(None);
            }
            &contextual
        } else {
            &execution
        };
        let admission =
            crate::BuildTimeAdmissionPlan::infer(execution, selection_authority.clone());
        let custodies: Vec<_> = record
            .invocation_sources
            .iter()
            .map(|source| crate::BuildTimeInvocationCustody::Source(*source))
            .collect();
        let reflected;
        let (fields, fingerprint) = match &guarded_schema {
            Some((fields, fingerprint, _)) => (fields.as_slice(), *fingerprint),
            None => {
                reflected =
                    super::super::schema_reflection::schema_fields(execution, &record.schema_data)
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                (reflected.0.as_slice(), reflected.1)
            }
        };
        let report = super::super::layout_plans::compute_native_layout_plan_with_schema(
            execution,
            machine_symbol,
            &record.schema_data,
            fields,
            fingerprint,
            &admission,
            &custodies,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        install_geometry(execution, record, &schema_fields, report).map(Some)
    };
    let realizations =
        if let Some(family) = family {
            validate_family_roster(&execution, family).map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}`: {reason}",
                    record.synthetic_name
                ))]
            })?;
            let mut alternatives = Vec::with_capacity(family.members.len());
            for member in &family.members {
                let Some(geometry) = evaluate(member.machine, Some(member.target.as_str()))
                    .map_err(|diagnostics| {
                        diagnostics.into_iter().map(|diagnostic| {
                        Diagnostic::error(format!(
                            "plan-laid value type `{}` target `{}` member `{}`: {diagnostic}",
                            record.synthetic_name,
                            member.target,
                            execution.symbols.name(member.machine),
                        ))
                    }).collect::<Vec<_>>()
                    })?
                else {
                    return Ok(false);
                };
                alternatives.push(PlanLaidTargetLayout {
                    target: member.target.clone(),
                    machine: member.machine,
                    geometry,
                });
            }
            PlanLaidRealizations::TargetFamily(target_layouts.insert_many(alternatives))
        } else if !contexts.is_empty() {
            let mut alternatives = Vec::with_capacity(contexts.len());
            for context in contexts {
                let Some(geometry) = evaluate(policy_plan_machine_symbol, Some(context.as_str()))?
                else {
                    return Ok(false);
                };
                alternatives.push(PlanLaidTargetLayout {
                    target: context.clone(),
                    machine: policy_plan_machine_symbol,
                    geometry,
                });
            }
            PlanLaidRealizations::TargetContexts {
                contexts: context_rosters.insert_many(contexts.iter().cloned()),
                layouts: target_layouts.insert_many(alternatives),
            }
        } else {
            PlanLaidRealizations::Invariant(
                evaluate(policy_plan_machine_symbol, None)
                    .map_err(|diagnostics| {
                        diagnostics
                            .into_iter()
                            .map(|diagnostic| {
                                Diagnostic::error(format!(
                                    "plan-laid value type `{}`: {diagnostic}",
                                    record.synthetic_name
                                ))
                            })
                            .collect::<Vec<_>>()
                    })?
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "portable policy evaluation retained target inputs",
                        )]
                    })?,
            )
        };
    let layout = PlanLaidLayout {
        data_name: record.synthetic_name.clone(),
        data_symbol,
        field_symbols,
        schema_symbol,
        schema_field_symbols,
        policy_symbol,
        policy_plan_machine_symbol,
        schema_argument,
        realizations,
    };
    if let Some(previous) = layouts
        .iter_mut()
        .find(|previous| previous.data_symbol == data_symbol)
    {
        if previous.schema_symbol != layout.schema_symbol
            || previous.field_symbols != layout.field_symbols
            || previous.schema_field_symbols != layout.schema_field_symbols
            || previous.policy_symbol != layout.policy_symbol
            || previous.policy_plan_machine_symbol != layout.policy_plan_machine_symbol
        {
            return Err(vec![Diagnostic::error(format!(
                "plan-laid value type `{}` changed its retained application identity",
                record.synthetic_name
            ))]);
        }
        *previous = layout;
    } else {
        layouts.push(layout);
    }
    typed.plan_laid_layouts = layouts;
    typed.plan_laid_target_layouts = target_layouts;
    typed.plan_laid_contexts = context_rosters;
    typed.plan_laid_schema_fields = captured_schema_fields;
    Ok(true)
}

/// Read actual dependency edges, not the presence of a query elsewhere in the
/// source cohort. These descriptions explain pending work; they grant no
/// admission, and evaluation errors never become scheduling signals.
fn application_prerequisites(
    typed: &mut TypedTrees,
    application: symbols::SymbolHandle,
    authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &mut crate::build_time_evaluation::EvaluationDomain,
    queries: Option<&dyn crate::LayoutQueryPreparation>,
) -> Result<ApplicationPrerequisites, Vec<Diagnostic>> {
    let layout = typed.plan_laid_layout(application).ok_or_else(|| {
        vec![Diagnostic::error(
            "layout readiness lost its bound application",
        )]
    })?;
    let schema = layout.schema_symbol;
    let policy = layout.policy_plan_machine_symbol;
    let mut roots = vec![schema, policy];
    let policies = if let Some(family) = typed
        .target_machine_families
        .iter()
        .find(|family| family.canonical == policy)
    {
        family
            .members
            .iter()
            .map(|member| member.machine)
            .collect::<Vec<_>>()
    } else {
        vec![policy]
    };
    roots.extend(policies.iter().copied());
    let mut demanded = domain.demanded_roots(typed, &roots);
    let original_demand = demanded.clone();
    let mut pending = Vec::new();
    for dependency in &typed.plan_laid_layouts {
        if demanded.contains(dependency.data_symbol) && !dependency.is_materialized() {
            pending.push(format!("layout `{}`", dependency.data_name));
        }
    }
    for machine in typed.machines() {
        if demanded.contains(machine.symbol)
            && machine.body_status == typed_trees::machine::MachineBodyStatus::Deferred
        {
            pending.push(format!("body `{}`", machine.name));
        }
    }
    if !pending.is_empty() {
        return Ok(ApplicationPrerequisites::Other(pending));
    }
    if let Some(queries) = queries {
        queries.prepare(typed, demanded.work())?;
    }
    if crate::const_evaluation::const_lengths::evaluate_independent_lengths(
        typed,
        authority.clone(),
        &demanded,
    )? {
        pending.push("provider-dependent array extents".to_owned());
    }
    use crate::const_evaluation::{const_applications, const_domain_facts, range_endpoints};
    let selected = crate::SelectedBuildTimeOperators::default();
    if range_endpoints::pending_endpoint_calls_need_operator_selection(
        typed,
        authority.clone(),
        &demanded,
    )? {
        pending.push("selected range endpoints".to_owned());
    }
    if const_domain_facts::pending_memberships_need_operator_selection(
        typed,
        authority.clone(),
        &demanded,
    )? {
        pending.push("selected domain inputs".to_owned());
    }
    if const_applications::pending_const_applications_need_operator_selection(
        typed,
        authority.clone(),
        &demanded,
    )? {
        pending.push("selected const arguments".to_owned());
    }
    if !pending.is_empty() {
        return Ok(ApplicationPrerequisites::Other(pending));
    }
    // Reflection consumes the normalized schema, including computed ranges
    // and const arguments. Use their existing producers in their normal order;
    // an unevaluated constraint is never a full-width scalar permission.
    range_endpoints::evaluate_selected_range_endpoints(
        typed,
        authority.clone(),
        selected,
        &demanded,
    )?;
    const_domain_facts::evaluate_selected_domain_facts(
        typed,
        authority.clone(),
        selected,
        &demanded,
    )?;
    let previous_types = typed.type_reference_table.revision();
    const_applications::evaluate_selected_const_applications(
        typed,
        authority.clone(),
        selected,
        &mut demanded,
    )?;
    domain.retain_produced(&original_demand, &demanded);
    if typed.type_reference_table.revision() != previous_types {
        // New instances may introduce lengths, domains or further applications.
        // Revisit their exact input closure before reflecting any geometry.
        return Ok(ApplicationPrerequisites::Other(vec![
            "newly realized schema inputs".to_owned(),
        ]));
    }
    if let Some(queries) = queries {
        queries.prepare(typed, demanded.work())?;
    }
    let execution = demanded.execution_program(typed);
    let admission = crate::BuildTimeAdmissionPlan::infer(&execution, authority);
    let operators = validation::derive_pre_flow_operator_selections(&execution);
    for policy in policies {
        if admission.closure_needs_operator_selection(&execution, policy, &operators) {
            pending.push(format!(
                "selected meaning in `{}`",
                typed.symbols.name(policy)
            ));
        }
    }
    let layout_queries = crate::const_evaluation::data_layout_queries::pending_layout_queries_need_operator_selection(
        typed,
        demanded.work(),
    );

    let mut records = vec![schema];
    let mut visited = Vec::new();
    while let Some(symbol) = records.pop() {
        if visited.contains(&symbol) {
            continue;
        }
        visited.push(symbol);
        if let Some(layout) = typed.plan_laid_layout(symbol) {
            match layout.realizations {
                PlanLaidRealizations::Pending => {
                    pending.push(format!("layout `{}`", layout.data_name));
                    continue;
                }
                PlanLaidRealizations::TargetFamily(_)
                | PlanLaidRealizations::TargetContexts { .. } => {
                    return Err(vec![Diagnostic::error(format!(
                        "schema contains target-dependent layout `{}`; nested target-dependent schema arguments are not yet supported",
                        layout.data_name
                    ))]);
                }
                PlanLaidRealizations::Invariant(_) => {}
            }
        }
        let Some(data) = typed.data_definition_by_symbol(symbol) else {
            continue;
        };
        let mut members = typed
            .live_data_fields(typed.data_members(data))
            .map(|field| field.type_reference)
            .collect::<Vec<_>>();
        while let Some(member) = members.pop() {
            use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
            match typed.type_reference_table.type_reference(member) {
                TypeReferenceNode::Named { symbol, .. } => records.push(*symbol),
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    if !matches!(length, FixedArrayLength::Literal(_)) {
                        pending.push(format!("schema array extent at {member:?}"));
                    }
                    members.push(*element_type);
                }
                TypeReferenceNode::Constrained { base_type, .. } => members.push(*base_type),
                _ => {}
            }
        }
    }
    if pending.is_empty() {
        Ok(if layout_queries {
            ApplicationPrerequisites::LayoutQueries
        } else {
            ApplicationPrerequisites::Ready
        })
    } else {
        if layout_queries {
            pending.push("layout-query values in the policy input closure".to_owned());
        }
        Ok(ApplicationPrerequisites::Other(pending))
    }
}

/// The structural roster is authoritative, including deployment target names
/// outside the native target catalog. Never infer membership from spelling.
fn validate_family_roster(
    typed: &TypedTrees,
    family: &typed_trees::machine::TargetMachineFamily,
) -> Result<(), String> {
    let canonical = typed.machine_by_symbol(family.canonical).ok_or_else(|| {
        format!(
            "layout policy family `{}` lost its canonical interface",
            family.name
        )
    })?;
    let canonical_identity = typed.machine_callable_identity(canonical);
    if family.members.is_empty()
        || !canonical.target_family_interface
        || !canonical_identity.is_valid()
    {
        return Err(format!(
            "layout policy family `{}` has no complete structural roster",
            family.name
        ));
    }
    for (position, member) in family.members.iter().enumerate() {
        if family.members[..position]
            .iter()
            .any(|earlier| earlier.target == member.target || earlier.machine == member.machine)
        {
            return Err(format!(
                "layout policy family `{}` has duplicate target/member `{}`",
                family.name, member.target
            ));
        }
        let machine = typed.machine_by_symbol(member.machine).ok_or_else(|| {
            format!(
                "layout policy family `{}` target `{}` lost its exact member",
                family.name, member.target
            )
        })?;
        if member.target.as_str().is_empty()
            || machine.authored_target.as_ref() != Some(&member.target)
            || machine.target_family_interface
            || machine.purpose != family.purpose
            || machine.attached_data_symbol != canonical.attached_data_symbol
            || typed.machine_callable_identity(machine) != canonical_identity
            || typed.machine_entry_result_type_identity(machine)
                != typed.machine_entry_result_type_identity(canonical)
        {
            return Err(format!(
                "layout policy family `{}` target `{}` has an inconsistent member `{}`",
                family.name,
                member.target,
                typed.symbols.name(member.machine)
            ));
        }
    }
    Ok(())
}

/// Version 1 records the complete active record argument; unsupported ABI axes
/// reject before evaluation so omitted inputs can never masquerade as a fold.
fn prepare_family_schema(
    typed: &TypedTrees,
    schema_symbol: symbols::SymbolHandle,
    schema_name: &str,
) -> Result<
    (
        Vec<super::super::schema_reflection::SchemaFieldInfo>,
        u64,
        Vec<PlanLaidSchemaField>,
    ),
    String,
> {
    let schema = typed
        .data_definition_by_symbol(schema_symbol)
        .ok_or_else(|| "layout family lost its exact schema declaration".to_owned())?;
    if !schema.retired_identities.is_empty() {
        return Err(format!(
            "target-family layout schema `{schema_name}` has retired identities; retained Schema ABI v1 supports active record fields only"
        ));
    }
    if typed
        .data_members(schema)
        .iter()
        .any(|member| member.as_field().is_none())
    {
        return Err(format!(
            "target-family layout schema `{schema_name}` has cases; retained Schema ABI v1 supports records only"
        ));
    }
    let (fields, fingerprint) = super::super::schema_reflection::schema_fields(typed, schema_name)?;
    for field in typed.live_data_fields(typed.data_members(schema)) {
        if !fields.iter().any(|row| row.schema_field == field.symbol) {
            return Err(format!(
                "target-family layout schema `{schema_name}` field `{}` has zero-sized relevant storage; retained Schema ABI v1 requires every relevant field to have an active row",
                field.name
            ));
        }
    }
    let retained = fields.iter().map(|field| {
        let kind = match field.kind {
            "Scalar" => PlanLaidSchemaFieldKind::Scalar,
            "Nested" => PlanLaidSchemaFieldKind::Nested,
            kind => return Err(format!(
                "target-family layout schema `{schema_name}` field `{}` uses `{kind}`; retained Schema ABI v1 supports Scalar and Nested fields only",
                field.name
            )),
        };
        Ok(PlanLaidSchemaField {
            schema_field: field.schema_field,
            key: field.key,
            identity: field.identity,
            byte_size: field.size,
            byte_alignment: field.align,
            kind,
        })
    }).collect::<Result<Vec<_>, String>>()?;
    Ok((fields, fingerprint, retained))
}

fn install_geometry(
    typed: &TypedTrees,
    record: &PlanLaidRecord,
    schema_fields: &[(String, typed_trees::types::TypeReferenceHandle)],
    native_report: crate::layouts::layout_plans::NativeLayoutPlanReport,
) -> Result<PlanLaidGeometry, Vec<Diagnostic>> {
    let report = &native_report.layout;
    let size = report.size.ok_or_else(|| vec![Diagnostic::error(format!(
        "policy `{}` produced a dynamic plan: a dynamic plan cannot be a value type -- values need offsets, bytes need mints",
        record.policy_machine
    ))])?;
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
                let typed_trees::types::TypeReferenceNode::FixedArray {
                    length: typed_trees::types::FixedArrayLength::Literal(element_count),
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
                if entries
                    .iter()
                    .all(|entry| matches!(entry.placement, LayoutPlacementReport::Bits { .. })) =>
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
    Ok(PlanLaidGeometry {
        validated_layout: report.clone(),
        private_callback_demands: native_report.private_callback_demands,
        offsets,
        bit_fields,
        integer_fields,
        repeated_fields,
        size,
        align,
    })
}

fn stored_integer_write_is_total(
    typed: &TypedTrees,
    field_type: typed_trees::types::TypeReferenceHandle,
    stored_width: u64,
    interpretation: layout_plans::IntegerInterpretation,
) -> bool {
    let Some(primitive) = typed.primitive_type_reference(field_type) else {
        return false;
    };
    let (admitted_minimum, admitted_maximum) =
        if let Some(range) = validation::scalar_representation_range(typed, field_type) {
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
    let (stored_minimum, stored_maximum) = interpretation.stored_bounds(stored_width);
    admitted_minimum >= stored_minimum && admitted_maximum <= stored_maximum
}
