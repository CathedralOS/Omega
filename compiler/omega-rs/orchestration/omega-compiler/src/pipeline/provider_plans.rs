//! Provider plans derive from checked `satisfies` closures and are admitted
//! through the chapter-10 trust path. Own-package plans remain dev-active with
//! a standing warning until the final build grants them; lockfile receipts hash
//! normalized plan identity so a changed plan drifts. Implicit selection
//! consumes only a unique covering candidate, while explicit selection remains
//! under slot-owner authority.

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
/// attached to the same provider type join one plan; legacy free machines
/// retain one anonymous compatibility candidate until PRV4f. Coverage never
/// combines unrelated provider types. Coverage/signatures come from the typed schema
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
            use omega_syntax_trees::item::ExternalBinding;
            let row_binding = match binding {
                None => ProviderBinding::CheckedAdapter {
                    machine: machine.name.as_str().to_owned(),
                },
                Some(binding) => match binding {
                    ExternalBinding::Syscall { number } => ProviderBinding::Syscall {
                        number: u32::try_from(*number).unwrap_or_default(),
                    },
                    ExternalBinding::DllImport { module, symbol } => ProviderBinding::Import {
                        library: module.clone(),
                        symbol: symbol.clone(),
                    },
                    ExternalBinding::VtableSlot { index } => {
                        ProviderBinding::VtableSlot { index: *index }
                    }
                    ExternalBinding::VtableField { field } => ProviderBinding::VtableField {
                        table: provider_type.clone(),
                        field: field.as_str().to_owned(),
                    },
                    ExternalBinding::TableFunction { field } => ProviderBinding::TableFunction {
                        table: provider_type.clone(),
                        field: field.as_str().to_owned(),
                    },
                },
            };
            plan.rows.push(ProviderPlanRow {
                method: requirement.as_str().to_owned(),
                binding: row_binding,
            });
        }
    }
    plans
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
/// extraction. The anonymous form preserves the free-machine migration bridge;
/// a real provider type is deliberately visible in artifact identity.
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
    for plan in plans {
        diagnostics.extend(
            plan.validate_candidate_against_schema()
                .into_iter()
                .map(omega_core::diagnostics::Diagnostic::error),
        );
        for row in &plan.rows {
            match &row.binding {
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

/// PRV4c: select one fully covering provider type per applicable boundary
/// slot. An explicit build-root declaration wins over the selected target
/// package's ordinary default declaration. Without either, a unique covering
/// candidate remains the compatibility fallback until PRV4f removes the
/// legacy provider surfaces. Rows are never selected individually and partial
/// candidates never combine.
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
    let name_matches = |authored: &str, candidate: &str| -> bool {
        authored == candidate
            || (!authored.contains("::")
                && candidate
                    .rsplit("::")
                    .next()
                    .is_some_and(|leaf| leaf == authored))
    };
    let mut selected = Vec::new();
    let mut slot_names: Vec<&str> = plans
        .iter()
        .filter(|plan| !plan.schema.methods.is_empty())
        .map(|plan| plan.schema.trait_name.as_str())
        .collect();
    for request in requested {
        if !slot_names
            .iter()
            .any(|slot| name_matches(&request.boundary_trait, slot))
        {
            diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                "build selects provider `{}` for unknown boundary slot `{}`; the slot must exist in the loaded dependency closure",
                request.provider_type, request.boundary_trait,
            )));
        }
    }
    for default in defaults {
        if !slot_names
            .iter()
            .any(|slot| name_matches(&default.boundary_trait, slot))
        {
            diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                "target package defaults provider `{}` for unknown boundary slot `{}`; the slot must exist in the loaded dependency closure",
                default.provider_type, default.boundary_trait,
            )));
        }
    }
    slot_names.sort_unstable();
    slot_names.dedup();

    for slot_name in slot_names {
        let explicit = requested
            .iter()
            .find(|selection| name_matches(&selection.boundary_trait, slot_name));
        let slot_defaults: Vec<_> = defaults
            .iter()
            .filter(|selection| name_matches(&selection.boundary_trait, slot_name))
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
            let mut distinct_provider_types: Vec<&str> = slot_defaults
                .iter()
                .map(|selection| selection.provider_type.as_str())
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
            let matching: Vec<&ProviderPlan> = candidates
                .iter()
                .copied()
                .filter(|plan| name_matches(&declaration.provider_type, &plan.provider_type))
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
                    let wrong_target = plans.iter().any(|plan| {
                        plan.schema.trait_name == slot_name
                            && name_matches(&declaration.provider_type, &plan.provider_type)
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
