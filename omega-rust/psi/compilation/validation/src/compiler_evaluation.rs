//! Source adapter for the shared compiler-evaluation floor.
//!
//! This only projects an already checked declaration. It selects no providers,
//! replays no contracts, and does not authorize an invocation.

use language_semantics::{MachineSupplyMode, Multiplicity};
use semantic_vocabulary::compiler_evaluation::{
    CompilerEvaluationDeclaration, CompilerEvaluationEffects,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;

pub fn checked_declaration(
    checked: &checked_trees::CheckedTrees,
    machine: &Machine,
    selections: &arena::Arena<checked_trees::CheckedSelectionRequirement>,
    sidecars: &typed_trees::references::MachineSidecarIndex,
) -> Result<CompilerEvaluationDeclaration, String> {
    let facts = &checked.facts;
    let reach = facts
        .service_reaches
        .for_machine(machine.symbol)
        .ok_or("checked compiler-evaluation declaration has no service-reach summary")?;
    let operational = facts
        .operational
        .machines()
        .iter()
        .find(|row| row.symbol == machine.symbol)
        .ok_or("checked compiler-evaluation declaration has no operational summary")?;
    let termination = facts
        .termination
        .rank_arrivals
        .judgment_for_body(machine.symbol, machine.states)
        .ok_or(
            "checked compiler-evaluation declaration has no exact local body termination judgment",
        )?;
    let mut services = Vec::new();
    for service in facts.service_reaches.rows.services(reach.effective) {
        let definition = facts
            .service_reaches
            .services
            .definition(*service)
            .ok_or("checked compiler-evaluation declaration reaches an unknown service")?;
        services.push(definition.name.as_str().to_owned());
    }
    let typed = &checked.typed;
    let states = typed.machine_states(machine);
    let entry = states
        .iter()
        .find(|state| {
            state.name.as_str()
                == machine
                    .name
                    .as_str()
                    .rsplit("::")
                    .next()
                    .unwrap_or_default()
        })
        .or_else(|| states.first());
    let has_requires = |contracts: &[typed_trees::signature::SignatureContract]| {
        contracts.iter().any(|contract| {
            contract.kind == typed_trees::signature::SignatureContractKind::Requires
        })
    };
    Ok(CompilerEvaluationDeclaration {
        name: machine.name.as_str().to_owned(),
        custody: symbol_custody(typed, machine.symbol),
        effects: CompilerEvaluationEffects {
            services,
            may_suspend: operational.transitive_may_suspend,
            may_block: operational.transitive_may_block,
        },
        local_terminates: typed.layout_target_facts_result(machine.symbol)?.is_some()
            || (machine.supply_mode == MachineSupplyMode::CheckedBody
                && termination.local_terminates
                && termination.diagnostic.is_empty()),
        entry_requires: has_requires(typed.machine_contracts(machine))
            || entry.is_none_or(|entry| has_requires(typed.state_contracts(entry))),
        linear_carrier: linear_carrier_violation(typed, machine),
        static_application: if machine.structural_type_equations_pending {
            Some(format!(
                "machine `{}` retains an undischarged structural type equation",
                machine.name
            ))
        } else if !machine.type_parameters.is_empty() {
            Some(format!(
                "machine `{}` retains an unspecialized static application",
                machine.name
            ))
        } else {
            None
        },
        selections: selection_requests(checked, machine, selections, sidecars),
    })
}

pub fn symbol_custody(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
) -> semantic_vocabulary::compiler_evaluation::DeclarationCustody {
    use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
    if let Some(package) = program.symbols.symbol_package_identity(symbol) {
        return DeclarationCustody::Package(package);
    }
    match program.symbols.symbol_source_origin(symbol) {
        Some(source::SourceOrigin::Toolchain) => DeclarationCustody::Toolchain,
        Some(source::SourceOrigin::User) => DeclarationCustody::UnownedUser,
        None => DeclarationCustody::Missing,
    }
}

fn selection_requests(
    checked: &checked_trees::CheckedTrees,
    machine: &Machine,
    selections: &arena::Arena<checked_trees::CheckedSelectionRequirement>,
    sidecars: &typed_trees::references::MachineSidecarIndex,
) -> Result<Vec<semantic_vocabulary::compiler_evaluation::DeclarationAuthorityRequest>, String> {
    let references =
        typed_trees::references::collect_machine_references(&checked.typed, machine, sidecars);
    let mut requests = Vec::new();
    let mut occurrences = references
        .authored_selection_occurrences
        .into_iter()
        .collect::<Vec<_>>();
    occurrences.sort_unstable();
    for occurrence in occurrences {
        let rows = selections.storage_slice();
        let position = rows
            .binary_search_by_key(
                &occurrence.ordinal(),
                checked_trees::CheckedSelectionRequirement::occurrence_ordinal,
            )
            .map_err(
                |_| "compiler-evaluation body lost a finalized declaration-selection occurrence",
            )?;
        let selection = &rows[position];
        let checked_trees::CheckedSelectionRequirement::Selection {
            excluded, request, ..
        } = selection
        else {
            return Err("compiler-evaluation declaration-selection requirement is absent".into());
        };
        if *excluded {
            return Err("compiler-evaluation body contains a declaration-selection occurrence outside Product authority coverage".into());
        }
        if let Some(request) = request.as_ref().map_err(Clone::clone)?
            && !requests.contains(request)
        {
            requests.push(request.clone());
        }
    }
    requests.sort_by_key(|request| (request.requester, request.owner));
    Ok(requests)
}

pub fn linear_carrier_violation(program: &TypedTrees, machine: &Machine) -> Option<String> {
    let describe = |context: &str, type_reference| {
        (program.type_multiplicity(type_reference)
            == Multiplicity::Linear)
            .then(|| {
                format!(
                    "{context} has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
                    program.display_type_reference(type_reference),
                    machine.name.as_str()
                )
            })
    };

    if let Some(attached_data) = machine.attached_data.as_ref()
        && program
            .attached_data_definition(machine)
            .is_some_and(|definition| definition.properties.multiplicity == Multiplicity::Linear)
    {
        return Some(format!(
            "machine instance `{}` has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
            machine.name,
            attached_data,
            machine.name.as_str()
        ));
    }

    for owned in program.machine_owned_data(machine) {
        if let Some(violation) = describe(
            &format!("machine-owned value `{}`", owned.name),
            owned.type_reference,
        ) {
            return Some(violation);
        }
    }

    for state in program.machine_states(machine) {
        for parameter in program.state_parameters(state) {
            if let Some(violation) = describe(
                &format!("state `{}` parameter `{}`", state.name, parameter.name),
                parameter.type_reference,
            ) {
                return Some(violation);
            }
        }
        if let Some(violation) =
            describe(&format!("state `{}` result", state.name), state.return_type)
        {
            return Some(violation);
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            let Some(local) = statement.as_local_data() else {
                continue;
            };
            if let Some(violation) = describe(
                &format!("state `{}` local `{}`", state.name, local.name),
                local.type_reference,
            ) {
                return Some(violation);
            }
        }
    }

    None
}

/// Retain the source checker's accepted whole-body decisions and still-partial
/// condition owners. Lowering only remaps these exact coordinates; it never
/// rediscovers a recursion proof or a Source dependency.
pub fn checked_dependencies(
    checked: &checked_trees::CheckedTrees,
) -> checked_trees::CheckedCompilerEvaluationDependencies {
    let ranks = &checked.facts.termination.rank_arrivals;
    let mut components = arena::Arena::new();
    let mut members = arena::Arena::new();
    let mut calls = arena::Arena::new();
    for (_, component) in ranks.runtime_components.iter() {
        let bodies = ranks.runtime_members.span_or_empty(component.members);
        // Productive recursion may have no diagnostic but still lack an
        // ordinary termination proof. Every exact body judgment is required.
        if bodies.is_empty()
            || !component.diagnostic.is_empty()
            || !bodies.iter().all(|member| {
                ranks
                    .judgment_for_body(member.machine, member.body)
                    .is_some_and(|judgment| {
                        judgment.local_terminates && judgment.diagnostic.is_empty()
                    })
                    && {
                        let mut current = checked
                            .machines()
                            .iter()
                            .filter(|machine| machine.symbol == member.machine);
                        current
                            .next()
                            .is_some_and(|machine| machine.states == member.body)
                            && current.next().is_none()
                    }
            })
        {
            continue;
        }
        let retained_edges = ranks.runtime_edges.span_or_empty(component.edges);
        let mut body_calls = Vec::new();
        let mut complete = true;
        for member in bodies {
            let Some(machine) = checked
                .facts
                .operational
                .machines()
                .iter()
                .find(|machine| machine.symbol == member.machine)
            else {
                complete = false;
                break;
            };
            let mut ordinal = 0u32;
            for state in checked
                .facts
                .operational
                .states
                .span_or_empty(machine.states)
            {
                for call in checked.facts.operational.calls.span_or_empty(state.calls) {
                    if bodies
                        .iter()
                        .any(|body| body.machine == call.target_machine_symbol)
                    {
                        if !retained_edges.iter().any(|edge| {
                            edge.source == state.symbol
                                && edge.target == call.target_state_symbol
                                && usize::try_from(edge.statement_ordinal).ok()
                                    == Some(call.statement_index)
                        }) {
                            complete = false;
                        }
                        body_calls.push(checked_trees::CheckedCompilerEvaluationCall {
                            caller: member.machine,
                            call_ordinal: ordinal,
                            callee: call.target_machine_symbol,
                        });
                    }
                    let Some(next) = ordinal.checked_add(1) else {
                        complete = false;
                        break;
                    };
                    ordinal = next;
                }
            }
        }
        if complete {
            let members = members.insert_many(bodies.iter().cloned());
            let calls = calls.insert_many(body_calls);
            components.insert(checked_trees::CheckedCompilerEvaluationComponent { members, calls });
        }
    }
    let mut pending_conditions = arena::Arena::new();
    pending_conditions.insert_many(pending_condition_owners(checked));
    checked_trees::CheckedCompilerEvaluationDependencies {
        recursive_components: components,
        recursive_members: members,
        recursive_calls: calls,
        pending_conditions,
    }
}

fn pending_condition_owners(checked: &checked_trees::CheckedTrees) -> Vec<SymbolHandle> {
    let mut owners = Vec::new();
    for plan in &checked.facts.flow.terminal_unit_effects.boundary_machines {
        if !plan.pending_scalar_requires.is_empty() || !plan.pending_scalar_recipes.is_empty() {
            owners.extend([plan.machine, plan.state]);
            for family in &checked.typed.target_machine_families {
                if family.canonical == plan.machine {
                    owners.extend(family.members.iter().map(|member| member.machine));
                }
            }
        }
    }
    owners.extend(
        checked
            .facts
            .constant_recipe_obligations
            .iter()
            .filter(|obligation| obligation.is_pending())
            .map(|obligation| obligation.owner()),
    );
    owners.extend(
        checked
            .facts
            .constant_predicate_obligations
            .iter()
            .filter(|obligation| obligation.is_pending())
            .map(|obligation| obligation.caller()),
    );
    owners.extend(
        checked
            .facts
            .integer_range_obligations
            .iter()
            .filter(|obligation| obligation.is_pending())
            .map(|obligation| obligation.owner()),
    );
    owners.sort_by_key(|owner| (owner.arena_index(), owner.generation()));
    owners.dedup();
    owners
}
