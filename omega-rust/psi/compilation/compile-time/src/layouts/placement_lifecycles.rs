//! Whole-value placement follows actual prepared calls, not the public
//! requirement roster or Resident annotations. Geometry is reusable; every
//! supplying occurrence owes fresh declaration authority before evaluation.
//! These records establish neither backing nor successful initialization.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::typed_trees::{PlacementLifecycleOperation, PlacementLifecyclePlan};
use typed_trees::{FrozenTypedTrees, TypedTrees};

use crate::machine_execution::admission::{BuildTimeAdmissionPlan, BuildTimeInvocationCustody};

mod demands;

pub fn retain_placement_lifecycle_plans(
    program: FrozenTypedTrees,
    authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
) -> Result<FrozenTypedTrees, Vec<Diagnostic>> {
    let mut program = program.into_inner();
    let demands = demands::collect(&mut program)?;
    if demands.is_empty() {
        return Ok(program.into());
    }
    let admission = BuildTimeAdmissionPlan::infer(&program, None);
    for demand in demands {
        let source = demand.sources[0];
        let diagnostic = |reason| vec![Diagnostic::error(reason).with_source_span(source)];
        let policy = demand.policy;
        let schema = demand.schema;
        let mut policies = program.machines().iter().filter(|machine| {
            machine.attached_data_symbol == policy
                && machine.name.as_str().rsplit("::").next() == Some("plan")
        });
        let (Some(policy_machine), None) = (policies.next(), policies.next()) else {
            return Err(diagnostic(
                "resident lifecycle policy must select one attached plan machine".into(),
            ));
        };
        for source in &demand.sources {
            admit(
                &program,
                &admission,
                policy_machine.symbol,
                *source,
                authority,
            )?;
        }
        let policy_machine_symbol = policy_machine.symbol;
        let schema_definition = program
            .data_definition_by_symbol(schema)
            .ok_or_else(|| diagnostic("resident lifecycle lost its schema declaration".into()))?;
        if demand.operation == PlacementLifecycleOperation::View {
            validate_nonresident_schema(&program, schema).map_err(diagnostic)?;
        }
        let retained_placement = program
            .placed_view_plans
            .iter()
            .find(|view| {
                view.policy_symbol == policy
                    && view.schema_symbol == schema
                    && view.policy_plan_machine_symbol == policy_machine_symbol
            })
            .map(|view| &view.placement)
            .or_else(|| {
                program
                    .placement_lifecycle_plans
                    .iter()
                    .find_map(|(_, row)| match row {
                        PlacementLifecyclePlan::Evaluated {
                            policy: retained_policy,
                            schema: retained_schema,
                            policy_machine: retained_machine,
                            placement,
                            ..
                        } if *retained_policy == policy
                            && *retained_schema == schema
                            && *retained_machine == policy_machine_symbol =>
                        {
                            Some(placement)
                        }
                        _ => None,
                    })
            });
        let placement = match retained_placement {
            Some(placement) => placement.clone(),
            None => super::access_plans::compute_selected_placement_plan(
                &program,
                policy_machine,
                schema_definition,
                // Lifecycle geometry is evaluated after source completion,
                // outside its retained accounting ledger. Ordinary evaluation
                // still enforces admission and its logical work ceiling.
                &Default::default(),
            )
            .map_err(diagnostic)?,
        };
        let invocations = program
            .placement_policy_invocations
            .insert_many(demand.sources);
        program
            .placement_lifecycle_plans
            .append(PlacementLifecyclePlan::Evaluated {
                callable: demand.callable,
                operation: demand.operation,
                policy,
                schema,
                policy_machine: policy_machine_symbol,
                invocations,
                placement,
            });
    }
    Ok(program.into())
}

/// A profile establishes access, not predicates about bytes. Record-wide
/// predicates are erased before Terminal publication, so reject them here,
/// including predicates nested inside represented records and array elements.
/// Terminal independently checks the remaining total-decoding shape. Borrowing
/// an affine schema does not create an owned value; only its represented fields
/// must be copyable, since reading them cannot manufacture content custody.
fn validate_nonresident_schema(program: &TypedTrees, schema: SymbolHandle) -> Result<(), String> {
    let mut records = vec![schema];
    let mut visited = Vec::new();
    while let Some(symbol) = records.pop() {
        if visited.contains(&symbol) {
            continue;
        }
        visited.push(symbol);
        let Some(data) = program.data_definition_by_symbol(symbol) else {
            continue;
        };
        if !data.where_facts.is_empty() || data.zero_gated {
            return Err(format!(
                "nonresident placement View cannot establish record predicates for `{}`; independent content-validity evidence is required",
                program.symbols.name(symbol),
            ));
        }
        let mut represented_types = Vec::new();
        for field in program.live_data_fields(program.data_members(data)) {
            if program.type_multiplicity(field.type_reference)
                != language_semantics::Multiplicity::Unrestricted
            {
                return Err(format!(
                    "nonresident placement View cannot acquire non-copy field `{}`; initialization or existing resident custody is required",
                    field.name,
                ));
            }
            represented_types.push(field.type_reference);
        }
        while let Some(type_reference) = represented_types.pop() {
            use typed_trees::types::TypeReferenceNode;
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Named { symbol, .. } => records.push(*symbol),
                TypeReferenceNode::FixedArray { element_type, .. } => {
                    represented_types.push(*element_type);
                }
                TypeReferenceNode::Constrained { .. } => {
                    return Err("nonresident placement View cannot establish field qualifications; independent content-validity evidence is required".into());
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Retained language judgments do not retain the current package permissions.
pub fn admit_placement_lifecycle_plans(
    program: &TypedTrees,
    authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
) -> Result<(), Vec<Diagnostic>> {
    if program
        .placement_lifecycle_plans
        .iter()
        .all(|(_, row)| matches!(row, PlacementLifecyclePlan::Absent))
    {
        return Ok(());
    }
    let admission = BuildTimeAdmissionPlan::infer(program, None);
    for (_, row) in program.placement_lifecycle_plans.iter() {
        let PlacementLifecyclePlan::Evaluated {
            policy_machine,
            invocations,
            ..
        } = row
        else {
            continue;
        };
        let sources = program
            .placement_policy_invocations
            .span_or_empty(*invocations);
        if sources.is_empty() || sources.len() != invocations.len() {
            return Err(vec![Diagnostic::error(
                "resident lifecycle plan lost its supplying invocations",
            )]);
        }
        for source in sources {
            admit(program, &admission, *policy_machine, *source, authority)?;
        }
    }
    Ok(())
}

fn admit(
    program: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    policy: SymbolHandle,
    source: source::SourceSpan,
    authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
) -> Result<(), Vec<Diagnostic>> {
    let machine = program.machine(policy).ok_or_else(|| {
        vec![
            Diagnostic::error("resident lifecycle lost its policy machine")
                .with_source_span(source),
        ]
    })?;
    admission
        .require_invocation_with_authority(
            program,
            machine,
            BuildTimeInvocationCustody::Source(source),
            authority,
        )
        .map_err(|rejection| {
            vec![
                Diagnostic::error(rejection.reason)
                    .with_source_span(rejection.source_span.unwrap_or(source)),
            ]
        })
}

fn core_lifecycle_operation(
    program: &TypedTrees,
    target: SymbolHandle,
) -> Option<PlacementLifecycleOperation> {
    let application = program.closed_requirement_application(target)?;
    if !is_core_layout_declaration(program, application.declaring_trait)
        || program.symbols.name(application.declaring_trait) != "PlacementLifecycle"
        || !is_core_layout_declaration(program, application.template)
    {
        return None;
    }
    match program
        .symbols
        .name(application.template)
        .rsplit("::")
        .next()?
    {
        "initialize" => Some(PlacementLifecycleOperation::Initialize),
        "move_out" => Some(PlacementLifecycleOperation::MoveOut),
        _ => None,
    }
}

fn is_core_layout_declaration(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    program
        .symbols
        .symbol_source_span(symbol)
        .and_then(|span| program.symbols.source_file(span))
        .is_some_and(|source| {
            source.origin == source::SourceOrigin::Toolchain
                && source.path.strip_prefix(&source.package_root).ok()
                    == Some(std::path::Path::new("layout.omg"))
        })
}
