//! Provider plans derive from checked `satisfies` closures and are admitted
//! through the chapter-10 trust path. Own-package plans remain dev-active with
//! a standing warning until the final build grants them; lockfile receipts hash
//! normalized plan identity so a changed plan drifts. A unique covering
//! candidate may still supply the declaration-era default, while explicit
//! selection remains under slot-owner authority.

use omega_effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
use omega_typed_trees::TypedTrees;

/// Exact selected provider-plan input consumed by external-root construction.
///
/// The schema is retained beside the normalized plan identity so installation
/// can bind the source callable shape—including domain/carry-qualified
/// parameter types—to the same provider selection later carried by
/// `ProviderExecution` and per-invocation entry receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExternalRootProviderPlan {
    pub identity: omega_external_roots::ProviderPlanId,
    pub schema: ServiceSchema,
}

/// Retain the exact validated selection on the checked program. Provider
/// execution and compiler-generated helper machines consume this carrier;
/// neither may reconstruct a plan by scanning authored `satisfies` rows.
pub(crate) fn retain_selected_provider_plan_facts(
    checked: &mut omega_checked_trees::CheckedTrees,
    candidates: &[ProviderPlan],
    selected_names: &[String],
    root_grants: &[String],
) -> Result<(), Vec<omega_core::diagnostics::Diagnostic>> {
    let facts =
        omega_checked_trees::SelectedProviderPlanFacts::from_selection(candidates, selected_names)
            .map_err(|error| vec![omega_core::diagnostics::Diagnostic::error(error)])?;
    let granted_receipts = facts
        .plans()
        .iter()
        .filter(|plan| {
            root_grants
                .iter()
                .any(|grant| grant == &plan.name || grant == &plan.schema.trait_name)
        })
        .map(|plan| (plan.schema.trait_name.clone(), plan.identity_fingerprint()))
        .collect::<Vec<_>>();
    let receipt_updates = checked
        .facts
        .semantic
        .facts
        .iter()
        .filter(|(_, fact)| {
            fact.evidence.origin
                == omega_core::semantics::QualificationEvidenceOrigin::AdmittedReceipt
                && fact.evidence.receipt_identity == 0
                && fact.evidence.source_symbol.is_valid()
        })
        .filter_map(|(handle, fact)| {
            granted_receipts
                .iter()
                .find(|(boundary, _)| {
                    evidence_source_names_boundary(checked, fact.evidence.source_symbol, boundary)
                })
                .map(|(_, identity)| (handle, *identity))
        })
        .collect::<Vec<_>>();
    retain_selected_operator_provider_evidence(checked, candidates, &facts)?;
    checked.retain_selected_provider_plans(facts);
    for (fact, identity) in receipt_updates {
        checked
            .facts
            .semantic
            .facts
            .get_mut(fact)
            .evidence
            .receipt_identity = identity;
    }
    Ok(())
}

fn retain_selected_operator_provider_evidence(
    checked: &mut omega_checked_trees::CheckedTrees,
    candidates: &[ProviderPlan],
    selected: &omega_checked_trees::SelectedProviderPlanFacts,
) -> Result<(), Vec<omega_core::diagnostics::Diagnostic>> {
    // Validate selected operator plans independently of use-site discovery.
    // A malformed realization is invalid policy even when dead code happens
    // not to mention its requirement, and later annotation may consume only
    // plans that passed this gate.
    let mut diagnostics = Vec::new();
    for plan in selected.plans() {
        let operator = checked.typed.operators().iter().find(|operator| {
            operator.is_boundary
                && omega_typed_trees::operator::boundary_operator_requirement_identity(
                    &checked.typed,
                    operator,
                ) == plan.schema.trait_name
        });
        let Some(operator) = operator else {
            continue;
        };
        if let Err(diagnostic) =
            selected_operator_provider_identity(checked, candidates, selected, operator.symbol)
        {
            diagnostics.push(diagnostic);
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let spelled = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(handle, operator_use)| (handle, operator_use.selected_operator_symbol))
        .collect::<Vec<_>>();
    let named = checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(handle, operator_use)| (handle, operator_use.selected_operator_symbol))
        .collect::<Vec<_>>();
    for (handle, symbol) in spelled {
        match selected_operator_provider_identity(checked, candidates, selected, symbol) {
            Ok(Some(identity)) => {
                checked
                    .facts
                    .operators
                    .uses
                    .get_mut(handle)
                    .provider_plan_identity = identity;
            }
            Ok(None) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    for (handle, symbol) in named {
        match selected_operator_provider_identity(checked, candidates, selected, symbol) {
            Ok(Some(identity)) => {
                checked
                    .facts
                    .operators
                    .named_uses
                    .get_mut(handle)
                    .provider_plan_identity = identity;
            }
            Ok(None) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn selected_operator_provider_identity(
    checked: &omega_checked_trees::CheckedTrees,
    candidates: &[ProviderPlan],
    selected: &omega_checked_trees::SelectedProviderPlanFacts,
    operator_symbol: omega_core::symbols::SymbolHandle,
) -> Result<Option<u64>, omega_core::diagnostics::Diagnostic> {
    let Some(operator) = checked
        .typed
        .operators()
        .iter()
        .find(|operator| operator.symbol == operator_symbol)
    else {
        return Ok(None);
    };
    let slot = omega_typed_trees::operator::boundary_operator_requirement_identity(
        &checked.typed,
        operator,
    );
    if !candidates
        .iter()
        .any(|candidate| candidate.schema.trait_name == slot)
    {
        return Ok(None);
    }
    let Some(plan) = selected
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == slot)
    else {
        return Err(omega_core::diagnostics::Diagnostic::error(format!(
            "boundary operator `{slot}` has provider candidates but no exact selected ProviderPlan realization for this target"
        )));
    };
    let [row] = plan.rows.as_slice() else {
        return Err(omega_core::diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must contain exactly one realization row",
            plan.name,
        )));
    };
    let ProviderBinding::CompilerIntrinsic { name } = &row.binding else {
        return Err(omega_core::diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` uses unsupported bootstrap binding `{:?}`; the migrated target slice requires one compiler intrinsic",
            plan.name, row.binding,
        )));
    };
    let expected = expected_float_intrinsic(&checked.typed, operator).ok_or_else(|| {
        omega_core::diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` targets `{slot}`, which has no compiler-known migrated intrinsic",
            plan.name,
        ))
    })?;
    if name != &expected {
        return Err(omega_core::diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` binds `{name}`, but `{slot}` requires exact intrinsic `{expected}`",
            plan.name,
        )));
    }
    Ok(Some(plan.identity_fingerprint()))
}

fn expected_float_intrinsic(
    typed: &TypedTrees,
    operator: &omega_typed_trees::operator::OperatorDefinition,
) -> Option<String> {
    let path = typed.operator_path_members(operator.name);
    let [namespace, requirement] = path else {
        return None;
    };
    let parameters = typed.operator_parameters(operator);
    let (operation, primitive, expected_result) = match namespace.as_str() {
        "Float" => {
            let operation = match requirement.as_str() {
                "add" | "subtract" | "multiply" | "divide" | "equal" | "not_equal" | "less"
                | "less_or_equal" | "greater" | "greater_or_equal" => requirement.as_str(),
                _ => return None,
            };
            let [left, right] = parameters else {
                return None;
            };
            let primitive = typed.primitive_type_reference(left.type_reference)?;
            if typed.primitive_type_reference(right.type_reference) != Some(primitive) {
                return None;
            }
            let expected_result = if matches!(operation, "add" | "subtract" | "multiply" | "divide")
            {
                primitive
            } else {
                omega_typed_trees::types::PrimitiveType::Bool
            };
            (operation, primitive, expected_result)
        }
        "F32" | "F64" => {
            let operation = match requirement.as_str() {
                "minimum" | "maximum" => requirement.as_str(),
                "negate" | "square_root" | "classify" | "is_nan" | "is_finite" | "is_infinite"
                | "is_normal" | "is_subnormal" => requirement.as_str(),
                "multiply_then_add" => requirement.as_str(),
                _ => return None,
            };
            let expected_primitive = if namespace.as_str() == "F32" {
                omega_typed_trees::types::PrimitiveType::F32
            } else {
                omega_typed_trees::types::PrimitiveType::F64
            };
            match parameters {
                [value]
                    if matches!(
                        operation,
                        "negate"
                            | "square_root"
                            | "classify"
                            | "is_nan"
                            | "is_finite"
                            | "is_infinite"
                            | "is_normal"
                            | "is_subnormal"
                    ) =>
                {
                    if typed.primitive_type_reference(value.type_reference)
                        != Some(expected_primitive)
                    {
                        return None;
                    }
                }
                [left, right] if matches!(operation, "minimum" | "maximum") => {
                    if typed.primitive_type_reference(left.type_reference)
                        != Some(expected_primitive)
                        || typed.primitive_type_reference(right.type_reference)
                            != Some(expected_primitive)
                    {
                        return None;
                    }
                }
                [left, right, addend] if operation == "multiply_then_add" => {
                    if typed.primitive_type_reference(left.type_reference)
                        != Some(expected_primitive)
                        || typed.primitive_type_reference(right.type_reference)
                            != Some(expected_primitive)
                        || typed.primitive_type_reference(addend.type_reference)
                            != Some(expected_primitive)
                    {
                        return None;
                    }
                }
                _ => return None,
            }
            if operation == "classify" {
                if typed.display_type_reference(operator.return_type) != "FloatClass" {
                    return None;
                }
                let format = if expected_primitive == omega_typed_trees::types::PrimitiveType::F32 {
                    "f32"
                } else {
                    "f64"
                };
                return Some(format!("{}::classify.{format}", namespace.as_str()));
            }
            let expected_result = if matches!(
                operation,
                "is_nan" | "is_finite" | "is_infinite" | "is_normal" | "is_subnormal"
            ) {
                omega_typed_trees::types::PrimitiveType::Bool
            } else {
                expected_primitive
            };
            (operation, expected_primitive, expected_result)
        }
        _ => return None,
    };
    if typed.primitive_type_reference(operator.return_type) != Some(expected_result) {
        return None;
    }
    let format = match primitive {
        omega_typed_trees::types::PrimitiveType::F32 => "f32",
        omega_typed_trees::types::PrimitiveType::F64 => "f64",
        _ => return None,
    };
    Some(format!("{}::{operation}.{format}", namespace.as_str()))
}

fn evidence_source_names_boundary(
    checked: &omega_checked_trees::CheckedTrees,
    source_symbol: omega_core::symbols::SymbolHandle,
    boundary_name: &str,
) -> bool {
    if checked.typed.traits().iter().any(|definition| {
        definition.symbol == source_symbol
            && same_semantic_name(definition.name.as_str(), boundary_name)
    }) {
        return true;
    }
    checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_symbol)
        .is_some_and(|machine| {
            checked
                .typed
                .machine_trait_conformances(machine)
                .iter()
                .any(|conformance| same_semantic_name(conformance.name.as_str(), boundary_name))
        })
}

/// Resolve one external-root boundary slot only from the immutable provider
/// selection retained on the checked program. The returned ID is the exact
/// normalized `ProviderPlan` fingerprint consumed by root validation; source
/// declarations and unselected candidates are no longer in scope here.
pub fn selected_external_root_provider_plan_id(
    checked: &omega_checked_trees::CheckedTrees,
    boundary_trait: &str,
) -> Result<omega_external_roots::ProviderPlanId, omega_external_roots::ExternalRootDiagnostic> {
    selected_external_root_provider_plan(checked, boundary_trait).map(|selected| selected.identity)
}

/// Resolve one external-root boundary slot to the exact retained provider
/// identity and normalized source schema. Root-installation artifacts can
/// therefore report the authority-bearing inputs bound by the receipt chain
/// without re-reading source or trusting display names.
pub fn selected_external_root_provider_plan(
    checked: &omega_checked_trees::CheckedTrees,
    boundary_trait: &str,
) -> Result<SelectedExternalRootProviderPlan, omega_external_roots::ExternalRootDiagnostic> {
    let matches = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .filter(|plan| same_semantic_name(&plan.schema.trait_name, boundary_trait))
        .collect::<Vec<_>>();
    let [plan] = matches.as_slice() else {
        return Err(omega_external_roots::ExternalRootDiagnostic(
            match matches.len() {
                0 => format!(
                    "external-root boundary slot `{boundary_trait}` has no retained selected provider plan"
                ),
                count => format!(
                    "external-root boundary slot `{boundary_trait}` matches {count} retained selected provider plans"
                ),
            },
        ));
    };
    Ok(SelectedExternalRootProviderPlan {
        identity: omega_external_roots::ProviderPlanId::from_normalized_identity(
            plan.identity_fingerprint(),
        )?,
        schema: plan.schema.clone(),
    })
}

/// PRV4 order step (2): derive plans from explicit SATISFIES edges -- one
/// plan per (provider type, boundary trait, target), assembled only from
/// that provider's conformance closure. External leaves and checked adapters
/// attached to the same provider type join one plan. External leaves may be
/// free declarations; checked adapters must belong to a nominal provider type
/// so execution can only dispatch through a retained whole-provider selection.
/// Coverage never combines unrelated provider types. Coverage/signatures come from the typed schema
/// (signature refinement is enforced by the conformance checker on each
/// edge); the effect surface is the union of the SATISFIED requirements'
/// declared effects -- the requirement supplies the ceiling, never the
/// leaf. Selection v1: a slot whose (trait, target) has exactly one FULLY
/// COVERING derived plan selects it implicitly; ambiguity or partial
/// coverage is loud at the consumer (the trust report shows coverage).
pub(crate) fn derive_satisfies_plans(
    syntax_trees: &omega_syntax_trees::SyntaxTrees,
    typed: &TypedTrees,
    selected_target: Option<&str>,
) -> Vec<ProviderPlan> {
    let mut plans: Vec<ProviderPlan> = Vec::new();
    for item in syntax_trees.root_items() {
        let omega_syntax_trees::item::Item::Machine(machine) = item else {
            continue;
        };
        if machine.boundary {
            continue;
        }
        for clause in syntax_trees.items.satisfies_clauses(machine.satisfies) {
            if clause.requirement.as_ref().is_some_and(|requirement| {
                typed
                    .machines()
                    .iter()
                    .find(|candidate| candidate.name.as_str() == machine.name.as_str())
                    .and_then(|candidate| {
                        omega_typed_trees::operator::resolve_satisfied_boundary_operator(
                            typed,
                            candidate,
                            clause.trait_name.as_str(),
                            requirement.as_str(),
                        )
                    })
                    .is_some()
            }) {
                // Exact boundary-operator requirements use one overloaded
                // signature per provider slot; derive them below rather than
                // manufacturing an empty boundary-trait schema here.
                continue;
            }
            // A bodyless leaf carries `via`; a CHECKED ADAPTER is an
            // ordinary machine with a body and a requirement-named
            // satisfies edge (no via). Both contribute rows; whole-trait
            // conformances (no requirement) are the trait system's
            // ordinary business and derive nothing here.
            let Some(requirement) = clause.requirement.as_ref() else {
                continue;
            };
            let binding_kind = match (&clause.via, machine.bodyless) {
                (Some(binding), true) => Some(binding.clone()),
                (None, false) => {
                    // A CHECKED ADAPTER derives a plan row only over a
                    // BOUNDARY trait (a service schema). A plain trait's
                    // conformance -- including its service-reach ceiling -- is the
                    // existing trait machinery's business (the decision-20
                    // admission fixtures pin it) and derives nothing here.
                    let is_boundary_trait = typed.traits().iter().any(|definition| {
                        definition.is_boundary
                            && (definition.name.as_str() == clause.trait_name.as_str()
                                || definition
                                    .name
                                    .as_str()
                                    .rsplit("::")
                                    .next()
                                    .is_some_and(|leaf| leaf == clause.trait_name.as_str()))
                    });
                    if !is_boundary_trait {
                        continue;
                    }
                    None
                }
                _ => continue, // refused elsewhere (via rungs)
            };
            let binding = binding_kind.as_ref();
            let _ = &binding;
            // The selected target-machine marker is cleared before lowering
            // so the machine behaves ordinarily. Recover the deployment
            // dimension from compile selection for plan identity/selection;
            // otherwise a target-scoped leaf silently becomes a universal
            // provider after it is selected.
            let target = machine.target.as_ref().map_or_else(
                || selected_target.unwrap_or_default().to_owned(),
                |target| target.as_str().to_owned(),
            );
            let trait_leaf = clause.trait_name.as_str().to_owned();
            let provider_type = machine
                .attached_data
                .as_ref()
                .map(|name| name.as_str().to_owned())
                .unwrap_or_default();
            let plan_name = satisfies_plan_name(&target, &trait_leaf, &provider_type);
            let position = plans
                .iter()
                .position(|plan| plan.name == plan_name)
                .unwrap_or_else(|| {
                    let schema = typed
                        .traits()
                        .iter()
                        .find(|definition| {
                            definition.name.as_str() == trait_leaf
                                || definition
                                    .name
                                    .as_str()
                                    .rsplit("::")
                                    .next()
                                    .is_some_and(|leaf| leaf == trait_leaf)
                        })
                        .and_then(|definition| {
                            let arguments =
                                provider_boundary_arguments(typed, definition, &provider_type);
                            ServiceSchema::from_typed_instance(typed, definition, &arguments)
                        })
                        .unwrap_or_else(|| ServiceSchema {
                            trait_name: trait_leaf.clone(),
                            methods: Vec::new(),
                        });
                    plans.push(ProviderPlan {
                        name: plan_name.clone(),
                        provider_type: provider_type.clone(),
                        target: target.clone(),
                        schema,
                        rows: Vec::new(),
                        origin_package: String::new(),
                    });
                    plans.len() - 1
                });
            let plan = &mut plans[position];
            let row_binding = match binding {
                None => ProviderBinding::CheckedAdapter {
                    machine: machine.name.as_str().to_owned(),
                },
                Some(binding) => external_provider_binding(binding, &provider_type),
            };
            plan.rows.push(ProviderPlanRow {
                method: requirement.as_str().to_owned(),
                binding: row_binding,
            });
        }
    }
    plans.extend(derive_boundary_operator_plans(
        syntax_trees,
        typed,
        selected_target,
    ));
    plans
}

fn derive_boundary_operator_plans(
    syntax_trees: &omega_syntax_trees::SyntaxTrees,
    typed: &TypedTrees,
    selected_target: Option<&str>,
) -> Vec<ProviderPlan> {
    let mut plans = Vec::<ProviderPlan>::new();
    for item in syntax_trees.root_items() {
        let omega_syntax_trees::item::Item::Machine(machine) = item else {
            continue;
        };
        let Some(typed_machine) = typed
            .machines()
            .iter()
            .find(|candidate| candidate.name.as_str() == machine.name.as_str())
        else {
            continue;
        };
        for clause in syntax_trees.items.satisfies_clauses(machine.satisfies) {
            let (Some(requirement), Some(binding)) =
                (clause.requirement.as_ref(), clause.via.as_ref())
            else {
                continue;
            };
            let Some(operator) = omega_typed_trees::operator::resolve_satisfied_boundary_operator(
                typed,
                typed_machine,
                clause.trait_name.as_str(),
                requirement.as_str(),
            ) else {
                continue;
            };
            let Some(schema) = ServiceSchema::from_typed_operator(typed, operator) else {
                continue;
            };
            let target = machine.target.as_ref().map_or_else(
                || selected_target.unwrap_or_default().to_owned(),
                |target| target.as_str().to_owned(),
            );
            let provider_type = machine
                .attached_data
                .as_ref()
                .map(|name| name.as_str().to_owned())
                .unwrap_or_default();
            let plan_name = satisfies_plan_name(&target, &schema.trait_name, &provider_type);
            let position = plans
                .iter()
                .position(|plan| plan.name == plan_name)
                .unwrap_or_else(|| {
                    plans.push(ProviderPlan {
                        name: plan_name.clone(),
                        provider_type: provider_type.clone(),
                        target: target.clone(),
                        schema: schema.clone(),
                        rows: Vec::new(),
                        origin_package: String::new(),
                    });
                    plans.len() - 1
                });
            plans[position].rows.push(ProviderPlanRow {
                method: "realize".to_owned(),
                binding: external_provider_binding(binding, &provider_type),
            });
        }
    }
    plans
}

fn external_provider_binding(
    binding: &omega_syntax_trees::item::ExternalBinding,
    provider_type: &str,
) -> ProviderBinding {
    use omega_syntax_trees::item::ExternalBinding;

    match binding {
        ExternalBinding::Syscall { number } => ProviderBinding::Syscall {
            number: u32::try_from(*number).unwrap_or_default(),
        },
        ExternalBinding::DllImport { module, symbol } => ProviderBinding::Import {
            library: module.clone(),
            symbol: symbol.clone(),
        },
        ExternalBinding::CompilerIntrinsic { name } => {
            ProviderBinding::CompilerIntrinsic { name: name.clone() }
        }
        ExternalBinding::VtableSlot { index } => ProviderBinding::VtableSlot { index: *index },
        ExternalBinding::VtableField { field } => ProviderBinding::VtableField {
            table: provider_type.to_owned(),
            field: field.as_str().to_owned(),
        },
        ExternalBinding::TableFunction { field } => ProviderBinding::TableFunction {
            table: provider_type.to_owned(),
            field: field.as_str().to_owned(),
        },
    }
}

fn provider_boundary_arguments(
    typed: &TypedTrees,
    boundary: &omega_typed_trees::trait_definition::TraitDefinition,
    provider_type: &str,
) -> Vec<omega_typed_trees::types::TypeReferenceHandle> {
    typed
        .data_conformances()
        .iter()
        .find(|conformance| {
            same_semantic_name(conformance.type_name.as_str(), provider_type)
                && same_semantic_name(conformance.trait_name.as_str(), boundary.name.as_str())
        })
        .map(|conformance| {
            typed
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .to_vec()
        })
        .unwrap_or_default()
}

fn same_semantic_name(left: &str, right: &str) -> bool {
    left == right
        || (!left.contains("::") && right.rsplit("::").next().is_some_and(|leaf| leaf == left))
        || (!right.contains("::") && left.rsplit("::").next().is_some_and(|leaf| leaf == right))
}

/// The stable name shared by derivation, reports, selection, and backend row
/// extraction. External leaves may use the anonymous form; a real provider
/// type is deliberately visible in artifact identity.
pub(crate) fn satisfies_plan_name(target: &str, trait_name: &str, provider_type: &str) -> String {
    match (target.is_empty(), provider_type.is_empty()) {
        (true, true) => format!("satisfies::{trait_name}"),
        (false, true) => format!("{target}::satisfies::{trait_name}"),
        (true, false) => format!("{provider_type}::satisfies::{trait_name}"),
        (false, false) => format!("{target}::{provider_type}::satisfies::{trait_name}"),
    }
}

/// Validate every derived candidate before coverage and selection. A partial
/// candidate may wait for more conformances, but duplicate/stray rows and
/// malformed binding shapes are invalid in their own right. For checked
/// adapters, normalized service reach must also fit inside the satisfied
/// requirement's declared ceiling. Independent operational refinement is
/// validated by the machine-conformance checker that produced the candidate.
pub(crate) fn validate_provider_plan_candidates(
    typed: &TypedTrees,
    plans: &[omega_effects::provider_plan::ProviderPlan],
) -> Vec<omega_core::diagnostics::Diagnostic> {
    let mut diagnostics = Vec::new();
    let effect_plan = omega_effects::infer_operational_may(typed);
    let service_reach_plan = omega_effects::infer_service_reaches(typed, &effect_plan);
    let invocation_plan = omega_effects::infer_synchronous_invocations(typed);
    for plan in plans {
        diagnostics.extend(
            plan.validate_candidate_against_schema()
                .into_iter()
                .map(omega_core::diagnostics::Diagnostic::error),
        );
        for row in &plan.rows {
            match &row.binding {
                ProviderBinding::CheckedAdapter { machine } if plan.provider_type.is_empty() => {
                    diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                        "checked adapter `{machine}` for `{}::{}` has no nominal provider type; attach it as `machine ProviderType::{machine}(...) satisfies {}::{}` and select that provider for the boundary slot",
                        plan.schema.trait_name,
                        row.method,
                        plan.schema.trait_name,
                        row.method,
                    )));
                }
                ProviderBinding::VtableField { table, .. }
                | ProviderBinding::TableFunction { table, .. }
                    if table.is_empty() =>
                {
                    diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                        "external leaf for `{}::{}` uses a table field without an attached provider data type; declare it as `machine TableType::leaf(...) satisfies {}::{} via Binding::...`",
                        plan.schema.trait_name,
                        row.method,
                        plan.schema.trait_name,
                        row.method,
                    )));
                }
                _ => {}
            }
            let ProviderBinding::CheckedAdapter { machine } = &row.binding else {
                continue;
            };
            let Some(adapter) = typed
                .machines()
                .iter()
                .find(|candidate| candidate.name.as_str() == machine.as_str())
            else {
                continue;
            };
            let service_ceiling = plan
                .schema
                .methods
                .iter()
                .find(|method| method.name == row.method)
                .map(|method| method.service_reach.as_slice())
                .unwrap_or_default();
            let invocation_ceiling = plan
                .schema
                .methods
                .iter()
                .find(|method| method.name == row.method)
                .map(|method| method.synchronous_invocations.as_slice())
                .unwrap_or_default();
            let hidden_invocations = invocation_plan
                .for_machine(adapter.symbol)
                .into_iter()
                .flat_map(|summary| summary.inferred_transitive.iter().copied())
                .filter(|target| {
                    !is_self_forwarded_invocation(
                        typed,
                        adapter,
                        &plan.schema,
                        &row.method,
                        *target,
                    )
                })
                .filter_map(|target| invocation_service_name(typed, adapter, target))
                .filter(|target| !invocation_ceiling.contains(target))
                .collect::<Vec<_>>();
            if !hidden_invocations.is_empty() {
                diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                    "adapter `{}` does not refine `{}::{}`: its body may synchronously invoke boundary binding(s) [{}], but the requirement omits those `invokes` edges",
                    machine,
                    plan.schema.trait_name,
                    row.method,
                    hidden_invocations.join(", "),
                )));
            }
            let hidden_services = service_reach_plan
                .for_machine(adapter.symbol)
                .into_iter()
                .flat_map(|summary| service_reach_plan.services(summary.effective).iter())
                .filter_map(|service| typed.service_reaches.definition(*service))
                .map(|definition| definition.name.as_str())
                .filter(|name| {
                    !service_ceiling
                        .iter()
                        .any(|allowed| allowed.as_str() == *name)
                })
                .collect::<Vec<_>>();
            if !hidden_services.is_empty() {
                diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                    "adapter `{}` does not refine `{}::{}`: its body reaches boundary service(s) [{}] outside the requirement's declared service ceiling [{}] -- the satisfied requirement is the public contract; widen it or drop the service reach",
                    machine,
                    plan.schema.trait_name,
                    row.method,
                    hidden_services.join(", "),
                    service_ceiling.join(", "),
                )));
            }
        }
    }
    diagnostics
}

/// Reject a cycle in the direct synchronous graph realized by the concrete
/// provider selection. Reach closure is intentionally irrelevant: only a
/// selected method's authored `invokes` edges participate, and a missing
/// selected target cannot manufacture an edge.
pub(crate) fn validate_selected_synchronous_invocation_cycles(
    typed: &TypedTrees,
    plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_names: &[String],
) -> Result<(), Vec<omega_core::diagnostics::Diagnostic>> {
    let selected = selected_names
        .iter()
        .filter_map(|name| plans.iter().find(|plan| plan.name == *name))
        .collect::<Vec<_>>();
    let inferred = omega_effects::infer_synchronous_invocations(typed);
    let mut edges = vec![Vec::<usize>::new(); selected.len()];
    for (source_index, source) in selected.iter().enumerate() {
        for method in &source.schema.methods {
            let row = source.rows.iter().find(|row| row.method == method.name);
            let checked_targets = row.and_then(|row| {
                let ProviderBinding::CheckedAdapter { machine } = &row.binding else {
                    return None;
                };
                let adapter = typed
                    .machines()
                    .iter()
                    .find(|candidate| candidate.name.as_str() == machine)?;
                let summary = inferred.for_machine(adapter.symbol)?;
                Some(
                    summary
                        .inferred_transitive
                        .iter()
                        .filter(|target| {
                            !is_self_forwarded_invocation(
                                typed,
                                adapter,
                                &source.schema,
                                &method.name,
                                **target,
                            )
                        })
                        .filter_map(|target| invocation_service_name(typed, adapter, *target))
                        .collect::<Vec<_>>(),
                )
            });
            let target_names = checked_targets
                .as_deref()
                .unwrap_or(&method.synchronous_invocations);
            for target_name in target_names {
                if let Some(target_index) = selected.iter().position(|target| {
                    target.schema.trait_name == *target_name
                        || target
                            .schema
                            .trait_name
                            .rsplit("::")
                            .next()
                            .is_some_and(|leaf| leaf == target_name)
                }) && !edges[source_index].contains(&target_index)
                {
                    edges[source_index].push(target_index);
                }
            }
        }
    }

    let mut color = vec![0u8; selected.len()];
    let mut path = Vec::new();
    for start in 0..selected.len() {
        if color[start] == 0
            && let Some(cycle) = synchronous_cycle_from(start, &edges, &mut color, &mut path)
        {
            let names = cycle
                .iter()
                .map(|index| selected[*index].schema.trait_name.as_str())
                .chain(std::iter::once(
                    selected[cycle[0]].schema.trait_name.as_str(),
                ))
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(vec![omega_core::diagnostics::Diagnostic::error(format!(
                "selected providers realize a cyclic synchronous `invokes` graph: {names}; break one edge with a mailbox, queue, scheduler handoff, or other new activation",
            ))]);
        }
    }
    Ok(())
}

fn invocation_service_name(
    typed: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    target: omega_effects::InvocationTarget,
) -> Option<String> {
    let symbol = match target {
        omega_effects::InvocationTarget::Parameter(index) => typed
            .machine_states(machine)
            .first()
            .into_iter()
            .flat_map(|state| typed.state_parameters(state))
            .filter(|parameter| !parameter.is_self)
            .nth(index as usize)
            .map(|parameter| {
                typed
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                    .type_symbol(&typed.type_reference_table)
            })?,
        omega_effects::InvocationTarget::Service(symbol) => symbol,
    };
    typed
        .traits()
        .iter()
        .find(|definition| definition.is_boundary && definition.symbol == symbol)
        .map(|definition| definition.name.as_str().to_owned())
}

/// A selected checked adapter may take the satisfied boundary receiver as one
/// extra leading parameter. Calls made through that value stay within the
/// selected provider artifact, so composition erases them from the realized
/// component-boundary graph. Other boundary parameters remain ordinary
/// invocation targets and continue to refine the requirement ceiling.
fn is_self_forwarded_invocation(
    typed: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    schema: &omega_effects::provider_plan::ServiceSchema,
    method_name: &str,
    target: omega_effects::InvocationTarget,
) -> bool {
    let omega_effects::InvocationTarget::Parameter(0) = target else {
        return false;
    };
    let Some(method) = schema
        .methods
        .iter()
        .find(|method| method.name == method_name)
    else {
        return false;
    };
    let Some(boundary) = typed.traits().iter().find(|definition| {
        definition.is_boundary && same_semantic_name(definition.name.as_str(), &schema.trait_name)
    }) else {
        return false;
    };
    omega_effects::has_self_forwarded_boundary_parameter(
        typed,
        machine,
        boundary.symbol,
        method.parameter_count,
    )
}

fn synchronous_cycle_from(
    node: usize,
    edges: &[Vec<usize>],
    color: &mut [u8],
    path: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    color[node] = 1;
    path.push(node);
    for target in &edges[node] {
        if color[*target] == 0 {
            if let Some(cycle) = synchronous_cycle_from(*target, edges, color, path) {
                return Some(cycle);
            }
        } else if color[*target] == 1 {
            let start = path.iter().position(|member| member == target)?;
            return Some(path[start..].to_vec());
        }
    }
    path.pop();
    color[node] = 2;
    None
}

fn authored_name_matches(authored: &str, candidate: &str, exact_exists: bool) -> bool {
    authored == candidate
        || (!exact_exists
            && !authored.contains("::")
            && candidate
                .rsplit("::")
                .next()
                .is_some_and(|leaf| leaf == authored))
}

fn resolve_provider_selection_slots(
    slot_names: &[String],
    declarations: &[crate::pipeline::build_config::ProviderSelection],
    owner: &str,
) -> (
    Vec<(String, crate::pipeline::build_config::ProviderSelection)>,
    Vec<omega_core::diagnostics::Diagnostic>,
) {
    let mut resolved = Vec::new();
    let mut diagnostics = Vec::new();
    for declaration in declarations {
        let exact_exists = slot_names
            .iter()
            .any(|slot| slot == &declaration.boundary_trait);
        let matching_slots = slot_names
            .iter()
            .map(String::as_str)
            .filter(|slot| authored_name_matches(&declaration.boundary_trait, slot, exact_exists))
            .collect::<Vec<_>>();
        match matching_slots.as_slice() {
            [slot] => resolved.push(((*slot).to_owned(), declaration.clone())),
            [] => diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                "{owner} selects provider `{}` for unknown boundary slot `{}`; the slot must exist in the loaded dependency closure",
                declaration.provider_type, declaration.boundary_trait,
            ))),
            many => diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                "{owner} names ambiguous boundary slot `{}`; it matches {} -- qualify the slot type",
                declaration.boundary_trait,
                many.iter()
                    .map(|slot| format!("`{slot}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
            ))),
        }
    }
    (resolved, diagnostics)
}

/// PRV4c: select one fully covering provider type per applicable boundary
/// slot. An explicit build-root declaration wins over the selected target
/// package's ordinary default declaration. Without either, a unique covering
/// candidate supplies the declaration-era default. Rows are never selected
/// individually and partial candidates never combine.
pub(crate) fn select_provider_plan_names(
    plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_target: omega_target::NativeTarget,
    defaults: &[crate::pipeline::build_config::ProviderSelection],
    requested: &[crate::pipeline::build_config::ProviderSelection],
) -> Result<Vec<String>, Vec<omega_core::diagnostics::Diagnostic>> {
    // Target inertness (the fail-canary host-portability convention): a
    // plan scoped to a NON-selected target is inert and never collides --
    // only plans that RESOLVE to the selected target participate.
    let applies = |target: &str| -> bool {
        if target.is_empty() {
            return true; // portable: every target
        }
        omega_target::NativeTarget::from_omega_target_name(Some(target))
            .is_ok_and(|resolved| resolved == selected_target)
    };
    let mut diagnostics = Vec::new();
    let mut selected = Vec::new();
    let mut slot_names: Vec<String> = plans
        .iter()
        .filter(|plan| !plan.schema.methods.is_empty())
        .map(|plan| plan.schema.trait_name.clone())
        .collect();
    slot_names.sort_unstable();
    slot_names.dedup();

    // Resolve every authored slot path to ONE canonical loaded trait before
    // applying precedence. A short leaf is convenient when unique, but it is
    // not an identity: letting `Pick` match both `first::Pick` and
    // `second::Pick` would turn one type-per-slot declaration into two grants.
    // Canonicalization here also makes differently spelled aliases of the
    // same slot participate in duplicate detection.
    let (resolved_requests, request_diagnostics) =
        resolve_provider_selection_slots(&slot_names, requested, "build");
    diagnostics.extend(request_diagnostics);
    let (resolved_defaults, default_diagnostics) =
        resolve_provider_selection_slots(&slot_names, defaults, "target package");
    diagnostics.extend(default_diagnostics);
    for slot_name in &slot_names {
        let declarations = resolved_requests
            .iter()
            .filter(|(slot, _)| slot == slot_name)
            .map(|(_, declaration)| declaration)
            .collect::<Vec<_>>();
        if declarations.len() > 1 {
            diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                "build declares provider selection for slot `{slot_name}` more than once: {}",
                declarations
                    .iter()
                    .map(|declaration| format!(
                        "`{} -> {}`",
                        declaration.boundary_trait, declaration.provider_type
                    ))
                    .collect::<Vec<_>>()
                    .join(", "),
            )));
        }
    }

    for slot_name in slot_names {
        let explicit = resolved_requests
            .iter()
            .find(|(slot, _)| *slot == slot_name)
            .map(|(_, selection)| selection);
        let slot_defaults: Vec<_> = resolved_defaults
            .iter()
            .filter(|(slot, _)| *slot == slot_name)
            .map(|(_, selection)| selection)
            .collect();
        let candidates: Vec<&ProviderPlan> = plans
            .iter()
            .filter(|plan| plan.schema.trait_name == slot_name && applies(&plan.target))
            .collect();
        let covering: Vec<&ProviderPlan> = candidates
            .iter()
            .copied()
            .filter(|plan| plan.covers_schema())
            .collect();

        let selected_declaration = if let Some(explicit) = explicit {
            // A slot-owner override intentionally replaces every target
            // default for this slot, including a default whose provider is
            // absent from the selected dependency closure.
            Some(("build", explicit))
        } else if let Some(first) = slot_defaults.first().copied() {
            // Compare target defaults by the canonical candidate type when a
            // spelling resolves uniquely. `Provider` and `pkg::Provider`
            // naming the same loaded type are one default, not a conflict.
            // Ambiguous or absent spellings stay distinct here and receive
            // the more specific candidate diagnostic below.
            let mut distinct_provider_types: Vec<String> = slot_defaults
                .iter()
                .map(|selection| {
                    let exact_exists = candidates
                        .iter()
                        .any(|plan| plan.provider_type == selection.provider_type);
                    let matching = candidates
                        .iter()
                        .copied()
                        .filter(|plan| {
                            authored_name_matches(
                                &selection.provider_type,
                                &plan.provider_type,
                                exact_exists,
                            )
                        })
                        .collect::<Vec<_>>();
                    match matching.as_slice() {
                        [plan] => plan.provider_type.clone(),
                        _ => selection.provider_type.clone(),
                    }
                })
                .collect();
            distinct_provider_types.sort_unstable();
            distinct_provider_types.dedup();
            if distinct_provider_types.len() > 1 {
                diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                    "slot `{slot_name}` has conflicting target-package defaults: {} -- a target supplies at most one default provider type per slot",
                    distinct_provider_types
                        .iter()
                        .map(|provider| format!("`{provider}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                )));
                continue;
            }
            Some(("target package", first))
        } else {
            None
        };

        if let Some((owner, declaration)) = selected_declaration {
            let exact_exists = candidates
                .iter()
                .any(|plan| plan.provider_type == declaration.provider_type);
            let matching: Vec<&ProviderPlan> = candidates
                .iter()
                .copied()
                .filter(|plan| {
                    authored_name_matches(
                        &declaration.provider_type,
                        &plan.provider_type,
                        exact_exists,
                    )
                })
                .collect();
            match matching.as_slice() {
                [plan] if plan.covers_schema() => selected.push(plan.name.clone()),
                [plan] => diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                    "{owner} selects provider `{}` for slot `{slot_name}`, but candidate `{}` is partial ({}/{}) and cannot be selected",
                    declaration.provider_type,
                    plan.name,
                    plan.rows.len(),
                    plan.schema.methods.len(),
                ))),
                [] => {
                    let exact_exists = plans.iter().any(|plan| {
                        plan.schema.trait_name == slot_name
                            && plan.provider_type == declaration.provider_type
                    });
                    let wrong_target = plans.iter().any(|plan| {
                        plan.schema.trait_name == slot_name
                            && authored_name_matches(
                                &declaration.provider_type,
                                &plan.provider_type,
                                exact_exists,
                            )
                    });
                    diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                        "{owner} selects provider `{}` for slot `{slot_name}`, but no {}candidate exists in the loaded dependency closure",
                        declaration.provider_type,
                        if wrong_target { "selected-target " } else { "" },
                    )));
                }
                _ => diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                    "{owner} selection `{}` for slot `{slot_name}` resolves to multiple provider candidates; qualify the provider type",
                    declaration.provider_type,
                ))),
            }
            continue;
        }

        match covering.as_slice() {
            [] => {}
            [plan] => selected.push(plan.name.clone()),
            many => {
                let count = if many.len() == 2 {
                    "two".to_owned()
                } else {
                    many.len().to_string()
                };
                diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                    "slot `{slot_name}` has {count} covering provider plans for the selected target: {} -- choose one in build.omg with `b.select_provider<{slot_name}, ProviderType>();`",
                    many.iter()
                        .map(|plan| format!("`{}` [{:016x}]", plan.name, plan.identity_fingerprint()))
                        .collect::<Vec<_>>()
                        .join(", "),
                )));
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(selected)
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection_plan(name: &str, methods: &[&str], rows: &[&str]) -> ProviderPlan {
        ProviderPlan {
            name: name.to_owned(),
            provider_type: name.to_owned(),
            target: String::new(),
            schema: ServiceSchema {
                trait_name: "Pair".to_owned(),
                methods: methods
                    .iter()
                    .map(|method| omega_effects::provider_plan::ServiceMethod {
                        name: (*method).to_owned(),
                        parameter_count: 0,
                        parameter_type_identities: Vec::new(),
                        entry_claims: Vec::new(),
                        has_result: false,
                        result_type_identity: None,
                        service_reach: vec!["Pair".to_owned()],
                        synchronous_invocations: Vec::new(),
                        may_suspend: false,
                        may_block: false,
                        calling_plan_fingerprint: None,
                    })
                    .collect(),
            },
            rows: rows
                .iter()
                .map(|method| ProviderPlanRow {
                    method: (*method).to_owned(),
                    binding: ProviderBinding::VtableSlot { index: 0 },
                })
                .collect(),
            origin_package: String::new(),
        }
    }

    #[test]
    fn selected_synchronous_invocation_graph_rejects_cycles_only_after_selection() {
        let mut alpha = selection_plan("alpha", &["run"], &["run"]);
        alpha.schema.trait_name = "Alpha".to_owned();
        alpha.schema.methods[0].synchronous_invocations = vec!["Beta".to_owned()];
        let mut beta = selection_plan("beta", &["run"], &["run"]);
        beta.schema.trait_name = "Beta".to_owned();
        beta.schema.methods[0].synchronous_invocations = vec!["Alpha".to_owned()];

        validate_selected_synchronous_invocation_cycles(
            &TypedTrees::default(),
            &[alpha.clone(), beta.clone()],
            &["alpha".to_owned()],
        )
        .expect("an unselected potential return edge is not realized");

        let diagnostics = validate_selected_synchronous_invocation_cycles(
            &TypedTrees::default(),
            &[alpha, beta],
            &["alpha".to_owned(), "beta".to_owned()],
        )
        .expect_err("the selected Alpha -> Beta -> Alpha graph must reject");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cyclic synchronous `invokes` graph")
                && diagnostic.message.contains("Alpha -> Beta -> Alpha")
        }));
    }

    #[test]
    fn implicit_selection_never_combines_partial_candidates() {
        let plans = vec![
            selection_plan("FirstProvider", &["first", "second"], &["first"]),
            selection_plan("SecondProvider", &["first", "second"], &["second"]),
        ];
        assert_eq!(
            select_provider_plan_names(&plans, omega_target::NativeTarget::host(), &[], &[])
                .expect("partial candidates are reportable, not ambiguous"),
            Vec::<String>::new(),
            "two partial candidates are not one provider"
        );
    }

    #[test]
    fn implicit_selection_returns_the_unique_covering_candidate() {
        let plans = vec![
            selection_plan(
                "CompleteProvider",
                &["first", "second"],
                &["first", "second"],
            ),
            selection_plan("PartialProvider", &["first", "second"], &["first"]),
        ];
        assert_eq!(
            select_provider_plan_names(&plans, omega_target::NativeTarget::host(), &[], &[])
                .expect("one covering candidate selects"),
            vec!["CompleteProvider".to_owned()]
        );
    }

    #[test]
    fn external_root_bridge_requires_one_exact_retained_boundary_slot() {
        let mut first = selection_plan("FirstProvider", &["run"], &["run"]);
        first.schema.trait_name = "first::Pair".into();
        let mut second = selection_plan("SecondProvider", &["run"], &["run"]);
        second.schema.trait_name = "second::Pair".into();
        let facts = omega_checked_trees::SelectedProviderPlanFacts::from_selection(
            &[first.clone(), second],
            &["FirstProvider".into(), "SecondProvider".into()],
        )
        .expect("distinct qualified boundary slots may both be selected");
        let mut checked = omega_checked_trees::CheckedTrees::default();
        checked.retain_selected_provider_plans(facts);

        assert_eq!(
            selected_external_root_provider_plan_id(&checked, "first::Pair")
                .expect("qualified slot resolves")
                .normalized_identity(),
            first.identity_fingerprint()
        );
        assert!(
            selected_external_root_provider_plan_id(&checked, "Pair")
                .expect_err("an ambiguous leaf slot must reject")
                .0
                .contains("matches 2 retained selected provider plans")
        );
    }

    #[test]
    fn granted_selected_plan_attaches_receipt_to_matching_admitted_fact() {
        let boundary_symbol = omega_core::symbols::SymbolHandle::from_arena_index(7);
        let subject_symbol = omega_core::symbols::SymbolHandle::from_arena_index(8);
        let domain_symbol = omega_core::symbols::SymbolHandle::from_arena_index(9);
        let mut checked = omega_checked_trees::CheckedTrees::default();
        checked
            .typed
            .push_trait_definition(omega_typed_trees::trait_definition::TraitDefinition {
                symbol: boundary_symbol,
                is_boundary: true,
                name: omega_typed_trees::name::Identifier::generated("Pair"),
                ..Default::default()
            });
        let place = checked.facts.semantic.append_symbol_place(subject_symbol);
        let fact = checked.facts.semantic.append_fact(omega_facts::Fact {
            place: omega_facts::FactPlace::Place(place),
            point: omega_facts::ProgramPoint::Global,
            origin: omega_facts::FactOrigin::CallEnsures,
            evidence: omega_facts::QualificationEvidence::from_origin(
                omega_core::semantics::QualificationEvidenceOrigin::AdmittedReceipt,
                boundary_symbol,
            ),
            payload: omega_facts::FactPayload::DomainMembership {
                value: Default::default(),
                domain: Default::default(),
                domain_symbol,
            },
        });
        let selected = selection_plan("FirstProvider", &["first"], &["first"]);
        let identity = selected.identity_fingerprint();

        retain_selected_provider_plan_facts(
            &mut checked,
            std::slice::from_ref(&selected),
            &["FirstProvider".to_owned()],
            &["Pair".to_owned()],
        )
        .expect("selected granted provider plan");

        assert_eq!(
            checked
                .facts
                .semantic
                .facts
                .get(fact)
                .evidence
                .receipt_identity,
            identity
        );
    }

    #[test]
    fn explicit_selection_resolves_covering_ambiguity_by_provider_type() {
        let plans = vec![
            selection_plan("FirstProvider", &["first"], &["first"]),
            selection_plan("SecondProvider", &["first"], &["first"]),
        ];
        let selected = select_provider_plan_names(
            &plans,
            omega_target::NativeTarget::host(),
            &[],
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pair".to_owned(),
                provider_type: "SecondProvider".to_owned(),
            }],
        )
        .expect("the build root owns the slot choice");
        assert_eq!(selected, vec!["SecondProvider".to_owned()]);
    }

    #[test]
    fn unqualified_selection_refuses_an_ambiguous_boundary_slot_leaf() {
        let mut first = selection_plan("FirstProvider", &["choose"], &["choose"]);
        first.schema.trait_name = "first::Pick".to_owned();
        let mut second = selection_plan("SecondProvider", &["choose"], &["choose"]);
        second.schema.trait_name = "second::Pick".to_owned();

        let diagnostics = select_provider_plan_names(
            &[first, second],
            omega_target::NativeTarget::host(),
            &[],
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pick".to_owned(),
                provider_type: "FirstProvider".to_owned(),
            }],
        )
        .expect_err("one short slot name must not grant two qualified slots");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("ambiguous boundary slot `Pick`")
                    && diagnostic.message.contains("`first::Pick`")
                    && diagnostic.message.contains("`second::Pick`")
            }),
            "expected a qualification diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn exact_slot_name_wins_over_a_qualified_leaf_fallback() {
        let exact = selection_plan("ExactProvider", &["choose"], &["choose"]);
        let mut qualified = selection_plan("QualifiedProvider", &["choose"], &[]);
        qualified.schema.trait_name = "package::Pair".to_owned();

        let selected = select_provider_plan_names(
            &[exact, qualified],
            omega_target::NativeTarget::host(),
            &[],
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pair".to_owned(),
                provider_type: "ExactProvider".to_owned(),
            }],
        )
        .expect("an exact canonical slot name outranks leaf fallback");
        assert_eq!(selected, vec!["ExactProvider".to_owned()]);
    }

    #[test]
    fn exact_provider_name_wins_over_a_qualified_leaf_fallback() {
        let exact = selection_plan("exact-plan", &["choose"], &["choose"]);
        let mut qualified = selection_plan("qualified-plan", &["choose"], &["choose"]);
        qualified.provider_type = "package::exact-plan".to_owned();

        let selected = select_provider_plan_names(
            &[exact, qualified],
            omega_target::NativeTarget::host(),
            &[],
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pair".to_owned(),
                provider_type: "exact-plan".to_owned(),
            }],
        )
        .expect("an exact canonical provider name outranks leaf fallback");
        assert_eq!(selected, vec!["exact-plan".to_owned()]);
    }

    #[test]
    fn canonical_slot_resolution_catches_duplicate_selection_spellings() {
        let mut plan = selection_plan("FirstProvider", &["choose"], &["choose"]);
        plan.schema.trait_name = "package::Pick".to_owned();

        let diagnostics = select_provider_plan_names(
            &[plan],
            omega_target::NativeTarget::host(),
            &[],
            &[
                crate::pipeline::build_config::ProviderSelection {
                    boundary_trait: "Pick".to_owned(),
                    provider_type: "FirstProvider".to_owned(),
                },
                crate::pipeline::build_config::ProviderSelection {
                    boundary_trait: "package::Pick".to_owned(),
                    provider_type: "SecondProvider".to_owned(),
                },
            ],
        )
        .expect_err("one canonical slot cannot be selected twice through aliases");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("provider selection for slot `package::Pick` more than once")),
            "expected canonical duplicate-slot diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn target_default_refuses_an_ambiguous_boundary_slot_leaf() {
        let mut first = selection_plan("FirstProvider", &["choose"], &["choose"]);
        first.schema.trait_name = "first::Pick".to_owned();
        let mut second = selection_plan("SecondProvider", &["choose"], &["choose"]);
        second.schema.trait_name = "second::Pick".to_owned();

        let diagnostics = select_provider_plan_names(
            &[first, second],
            omega_target::NativeTarget::host(),
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pick".to_owned(),
                provider_type: "FirstProvider".to_owned(),
            }],
            &[],
        )
        .expect_err("a target default must name one canonical slot");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("target package names ambiguous boundary slot `Pick`")
        }));
    }

    #[test]
    fn explicit_selection_refuses_partial_provider() {
        let plans = vec![selection_plan(
            "PartialProvider",
            &["first", "second"],
            &["first"],
        )];
        let diagnostics = select_provider_plan_names(
            &plans,
            omega_target::NativeTarget::host(),
            &[],
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pair".to_owned(),
                provider_type: "PartialProvider".to_owned(),
            }],
        )
        .expect_err("selection never manufactures missing rows");
        assert!(diagnostics[0].message.contains("is partial"));
    }

    #[test]
    fn target_default_resolves_covering_ambiguity() {
        let plans = vec![
            selection_plan("FirstProvider", &["first"], &["first"]),
            selection_plan("SecondProvider", &["first"], &["first"]),
        ];
        let selected = select_provider_plan_names(
            &plans,
            omega_target::NativeTarget::host(),
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pair".to_owned(),
                provider_type: "FirstProvider".to_owned(),
            }],
            &[],
        )
        .expect("the selected target package supplies the slot default");
        assert_eq!(selected, vec!["FirstProvider".to_owned()]);
    }

    #[test]
    fn target_default_aliases_of_one_provider_do_not_conflict() {
        let mut plan = selection_plan("package-provider", &["first"], &["first"]);
        plan.provider_type = "package::FirstProvider".to_owned();
        let selected = select_provider_plan_names(
            &[plan],
            omega_target::NativeTarget::host(),
            &[
                crate::pipeline::build_config::ProviderSelection {
                    boundary_trait: "Pair".to_owned(),
                    provider_type: "FirstProvider".to_owned(),
                },
                crate::pipeline::build_config::ProviderSelection {
                    boundary_trait: "Pair".to_owned(),
                    provider_type: "package::FirstProvider".to_owned(),
                },
            ],
            &[],
        )
        .expect("aliases of one canonical provider type are one target default");
        assert_eq!(selected, vec!["package-provider".to_owned()]);
    }

    #[test]
    fn build_override_wins_over_target_default() {
        let plans = vec![
            selection_plan("FirstProvider", &["first"], &["first"]),
            selection_plan("SecondProvider", &["first"], &["first"]),
        ];
        let selected = select_provider_plan_names(
            &plans,
            omega_target::NativeTarget::host(),
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pair".to_owned(),
                provider_type: "FirstProvider".to_owned(),
            }],
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pair".to_owned(),
                provider_type: "SecondProvider".to_owned(),
            }],
        )
        .expect("the build root owns the final slot choice");
        assert_eq!(selected, vec!["SecondProvider".to_owned()]);
    }

    #[test]
    fn conflicting_target_defaults_are_loud() {
        let plans = vec![
            selection_plan("FirstProvider", &["first"], &["first"]),
            selection_plan("SecondProvider", &["first"], &["first"]),
        ];
        let diagnostics = select_provider_plan_names(
            &plans,
            omega_target::NativeTarget::host(),
            &[
                crate::pipeline::build_config::ProviderSelection {
                    boundary_trait: "Pair".to_owned(),
                    provider_type: "FirstProvider".to_owned(),
                },
                crate::pipeline::build_config::ProviderSelection {
                    boundary_trait: "Pair".to_owned(),
                    provider_type: "SecondProvider".to_owned(),
                },
            ],
            &[],
        )
        .expect_err("a target has one default provider per slot");
        assert!(
            diagnostics[0]
                .message
                .contains("conflicting target-package defaults")
        );
    }

    #[test]
    fn table_field_leaf_requires_an_attached_layout_owner() {
        let mut plan = selection_plan("field-leaf", &["first"], &[]);
        plan.provider_type.clear();
        plan.rows.push(ProviderPlanRow {
            method: "first".to_owned(),
            binding: ProviderBinding::VtableField {
                table: String::new(),
                field: "first".to_owned(),
            },
        });

        let diagnostics = validate_provider_plan_candidates(&TypedTrees::default(), &[plan]);

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("without an attached provider data type")
        );
    }

    #[test]
    fn checked_adapter_requires_a_nominal_provider_type() {
        let mut plan = selection_plan("free-adapter", &["first"], &[]);
        plan.provider_type.clear();
        plan.rows.push(ProviderPlanRow {
            method: "first".to_owned(),
            binding: ProviderBinding::CheckedAdapter {
                machine: "first_adapter".to_owned(),
            },
        });

        let diagnostics = validate_provider_plan_candidates(&TypedTrees::default(), &[plan]);

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("has no nominal provider type")
        );
    }

    #[test]
    fn checked_adapter_rejects_symbol_resolved_service_widening() {
        let source = r#"
            boundary trait Queryable {
                machine query();
            }

            boundary trait Readable {
                machine read(queryable: &mut Queryable);
            }

            data Provider {}

            machine Provider::read(queryable: &mut Queryable)
            satisfies Readable::read {
                queryable.query();
            }
        "#;
        let tokens = omega_source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize");
        let syntax =
            omega_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse provider");
        let resolved = omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve provider");
        let typed =
            omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type provider");
        let plans = derive_satisfies_plans(&syntax, &typed, None);

        let diagnostics = validate_provider_plan_candidates(&typed, &plans);

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("boundary service(s) [Queryable]")
                && diagnostic
                    .message
                    .contains("declared service ceiling [Readable]")
        }));
    }
}
