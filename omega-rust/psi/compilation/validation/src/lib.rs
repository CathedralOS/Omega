/*
We decide whether a typed Omega program means what it says. `validate_program`
and `prepare_specialized_program` check declarations, machine calls, effects,
contracts, arithmetic domains, and value custody over `TypedTrees`, and
`CheckingContext` / `CheckingInput` give every later reader the same immutable
program plus its invocation memos. Successful validation is a source judgment,
not a portable proof certificate.
*/

/*
We keep one coordinator and four domain families. `program_validation.rs` runs
declaration checks, machine and state validation, and result publication in
order; `frozen_program_and_queries.rs` owns the immutable checking context and
the shared read-only queries. The families are declarations (`declarations_core`,
`operators`, `traits`), machine calls (`machine_calls_core`, `calls`,
`call_cycles`, `effect_inference`, `machine_parameters_and_bindings`,
`reference_result_custody`, `structural_call_custody`), proof contracts
(`proof_contracts_core`, `arithmetic_domains`, `contract_entailment`,
`domains_and_predicates`, `float_and_mathematical_contracts`, `quotients`), and
value custody (`value_custody_core`, `access_views_and_borrows`,
`completed_storage_and_conservation`, `expression_types_and_fields`,
`literals_and_struct_literals`, `places`, `recasts`, `type_references_and_wire`,
`borrow_and_reflection`). The `declarations`, `machine_calls`,
`proof_contracts`, and `value_custody` facade modules below keep the module
paths every checker already names, so a flat file layout costs no call site.
Checking only reads typed tables: preparation may rewrite them, checking may
not, and no thread or program address selects a memo.
*/

/*
We sit between typing and checked facts. `symbol-resolved-trees-to-typed-trees`
(Psi 03) produces the `TypedTrees` we read; `typed-trees-to-typed-trees` (Psi 04)
borrows our queries while preparing and specializing, `typed-trees-to-checked-trees`
(Psi 05) runs full validation and publishes checked facts, and `proof`,
`checked-trees-to-lowered-psi`, `lowered-psi-to-terminal-psi`,
`checked-interpreter`, `representation-planning`, build evaluation, and the
compiler driver reuse the same read-only judgments.
*/

mod access_views_and_borrows;
pub(crate) mod arithmetic_domains;
mod borrow_and_reflection;
pub(crate) mod call_cycles;
pub(crate) mod calls;
mod completed_storage_and_conservation;
pub(crate) mod contract_entailment;
mod declarations_core;
mod domains_and_predicates;
pub(crate) mod effect_inference;
pub mod erased_actuals;
mod expression_types_and_fields;
mod float_and_mathematical_contracts;
mod frozen_program_and_queries;
mod literals_and_struct_literals;
mod machine_calls_core;
mod machine_parameters_and_bindings;
pub(crate) mod operators;
pub(crate) mod places;
mod program_validation;
mod proof_contracts_core;
pub(crate) mod quotients;
pub(crate) mod recasts;
pub mod reference_result_custody;
pub(crate) mod structural_call_custody;
pub mod trace_flags;
pub(crate) mod traits;
mod type_references_and_wire;
mod value_custody_core;

pub use borrow_and_reflection::{borrow_contracts, reflection};
pub(crate) use frozen_program_and_queries::{frozen_program, queries};

pub(crate) mod declarations {
    pub use crate::declarations_core::{
        data_queries, declaration_visibility, definition_facts, selection_authority,
        selection_resolution, source_completion, standard_declarations, state_signatures, symbols,
        transitions, type_aliases,
    };
    pub(crate) use crate::operators;
    pub(crate) use crate::traits;
}

pub(crate) mod machine_calls {
    pub(crate) use crate::call_cycles;
    pub(crate) use crate::calls;
    pub(crate) use crate::effect_inference;
    pub(crate) use crate::machine_calls_core::record_reference;
    pub use crate::machine_calls_core::{
        assembly_services, attached_method_bounds, boundary_calls, callable_overloads,
        compiler_operations, contract_dependencies, contract_identities, denotational_calls,
        effects, fact_call_projections, fact_only_citations, graph_components, invocations,
        machine_data, machine_specialization_identity, result_overloads,
        static_machine_call_contracts, terminal_transitions,
    };
    pub use crate::machine_parameters_and_bindings::{
        machine_parameters, static_arguments, static_bindings,
    };
    pub use crate::reference_result_custody;
    pub(crate) use crate::structural_call_custody;
}

pub(crate) mod proof_contracts {
    pub(crate) use crate::arithmetic_domains;
    pub(crate) use crate::contract_entailment;
    pub use crate::domains_and_predicates::{
        bound_expression_meaning, collection_predicates, default_domains, domains,
    };
    pub use crate::float_and_mathematical_contracts::{
        float_projection_bindings, float_projection_invocations, mathematical_applications,
    };
    pub use crate::proof_contracts_core::{
        carrier_contents, contract_argument_root, contract_results, contract_structure,
        domain_weakening, expression_owners, immutable_integer_bounds, instruction_effects,
        parameter_domains, parameter_expression_meaning, proof_embeddings, proof_facts,
        proof_only_faces, properties, proposition_context, proposition_entailment,
        qualification_evidence, relevance, scalar_boundary_guarantees, signature_ensures,
        slice_ranking,
    };
    pub(crate) use crate::quotients;
}

pub(crate) mod value_custody {

    pub use crate::access_views_and_borrows::{
        access_atomic_observing_results, access_views, write_only_borrows,
    };
    pub use crate::completed_storage_and_conservation::{
        completed_storage, content_conservation, content_projections,
    };
    pub use crate::expression_types_and_fields::{expression_types, field_domains};
    pub use crate::literals_and_struct_literals::{literals, struct_literals};
    pub(crate) use crate::places;
    pub(crate) use crate::recasts;
    pub use crate::type_references_and_wire::{type_references, wire};
    pub use crate::value_custody_core::{
        activation_storage, atomic_fences, atomic_operations, byte_observations, claim_frontier,
        cleanup, cleanup_selection, collect_executable_value_origins, const_expressions, constants,
        content_classes, data, destructure, dispatch_extraction, intrinsic_boundaries, locals,
        logical_matches, membership_evidence, owned_value_source, partial_affine_residuals,
        permission_provenance, plan_laid, qualification_casts, receiving_values, reference_access,
        reference_local_establishment, reference_shells, resident_content, scalar_case_constructor,
        scalar_representation_range, source_liveness, static_inline_storage, storage_contents,
        structural_identity, transition_operand_moves, type_properties, type_shapes,
        visit_executable_value_occurrences, visit_statement_value_occurrences,
    };

    pub(crate) mod pattern_components {
        //! Receiving slots preserve a case view's component and reference carrier.

        fn reference_alias_type(
            program: &dyn crate::CheckingInput,
            reference: typed_trees::types::TypeReferenceHandle,
        ) -> bool {
            reference.is_valid()
                && matches!(
                    program.type_reference_table.type_reference(
                        program
                            .type_reference_table
                            .unconstrained_type_reference(reference)
                    ),
                    typed_trees::types::TypeReferenceNode::Reference { .. }
                )
        }

        pub(crate) fn report_delivery(
            program: &dyn crate::CheckingInput,
            value: typed_trees::expression::ExpressionHandle,
            destination: typed_trees::types::TypeReferenceHandle,
            expected: Option<language_core::PatternComponent>,
            owner: &str,
            diagnostics: &mut Vec<diagnostics::Diagnostic>,
        ) -> bool {
            if !value.is_valid() {
                return false;
            }
            let actual = program.pattern_component(value);
            if actual.is_none()
                || expected.is_none()
                || actual != expected
                || (actual == Some(language_core::PatternComponent::Case)
                    && !reference_alias_type(program, destination))
            {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
            "{owner} cannot recover a whole data value or change the component of a case-only view"
        )));
                return true;
            }
            false
        }
    }
}

pub mod compiler_evaluation {
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
    ) -> Result<Vec<semantic_vocabulary::compiler_evaluation::DeclarationAuthorityRequest>, String>
    {
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
                return Err(
                    "compiler-evaluation declaration-selection requirement is absent".into(),
                );
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
                .is_some_and(|definition| {
                    definition.properties.multiplicity == Multiplicity::Linear
                })
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

    pub fn checked_dependencies(
        checked: &checked_trees::CheckedTrees,
    ) -> checked_trees::CheckedCompilerEvaluationDependencies {
        let ranks = &checked.facts.termination.rank_arrivals;
        let mut components = arena::Arena::new();
        let mut members = arena::Arena::new();
        let mut calls = arena::Arena::new();
        for (_, component) in ranks.runtime_components.iter() {
            let bodies = ranks.runtime_members.span_or_empty(component.members);
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
                components
                    .insert(checked_trees::CheckedCompilerEvaluationComponent { members, calls });
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
}

pub mod product_entries {
    use diagnostics::Diagnostic;
    use semantic_vocabulary::ProgramEntryVisibleParameters;
    use typed_trees::TypedTrees;
    use typed_trees::product_operand::ProductEntryDeclaration;

    pub(crate) mod root_bindings {
        use diagnostics::Diagnostic;
        use language_semantics::declaration_selection::{
            ExecutedRootBinding, RootBindingSelection,
        };
        use typed_trees::TypedTrees;

        use super::RootBinding;

        pub fn collect_root_bindings(
            typed: &TypedTrees,
            executed: &[ExecutedRootBinding],
        ) -> Result<Vec<RootBinding>, Vec<Diagnostic>> {
            let typed = typed.product_declarations();
            let mut bindings: Vec<RootBinding> = Vec::with_capacity(executed.len());
            let mut diagnostics = Vec::new();
            for binding in executed {
                let slot = binding.slot.clone();
                let (implementation, implementation_symbol) = match &binding.implementation {
                    RootBindingSelection::Described(described) => {
                        if slot != described.slot {
                            diagnostics.push(
                        Diagnostic::error(format!(
                            "root binding slot `{slot}` does not match the product description's slot `{}`",
                            described.slot
                        ))
                        .with_source_span(binding.source_span),
                    );
                            continue;
                        }
                        let selected = described
                            .declaration
                            .rejoin(
                                &typed.symbols,
                                typed.machines().iter().map(|machine| machine.symbol),
                            )
                            .map_err(|reason| {
                                vec![
                                    Diagnostic::error(reason).with_source_span(binding.source_span),
                                ]
                            })?;
                        (described.machine_name.clone(), selected)
                    }
                    RootBindingSelection::AuthoredPath(implementation) => {
                        let implementation = implementation.clone();
                        let occurrence =
                            typed.symbols.rejoin_product_occurrence(binding.source_span);
                        let qualified = implementation.split_once("::");
                        let dependency = qualified.and_then(|(alias, _)| {
                            typed.symbols.product_dependency_target(occurrence, alias)
                        });
                        let declaration_path = match (qualified, dependency) {
                            (Some((_, path)), Some(_)) => path,
                            _ => implementation.as_str(),
                        };
                        let mut candidates = typed
                            .machines()
                            .iter()
                            .filter(|machine| {
                                ((dependency.is_none()
                                    && machine.name.as_str() == declaration_path)
                                    || typed.symbols.display_path(machine.symbol, "::")
                                        == declaration_path)
                                    && match dependency {
                                        Some(package) => {
                                            machine.is_public
                                                && typed
                                                    .symbols
                                                    .symbol_product_package_identity(machine.symbol)
                                                    == Some(package)
                                        }
                                        None => typed
                                            .symbols
                                            .symbol_source_span(machine.symbol)
                                            .is_some_and(|span| {
                                                typed
                                                    .symbols
                                                    .same_product_package_instance(occurrence, span)
                                            }),
                                    }
                            })
                            .filter(|machine| {
                                typed.symbols.source_reference_can_describe_product_symbol(
                                    occurrence,
                                    machine.symbol,
                                )
                            })
                            .collect::<Vec<_>>();
                        if candidates.iter().any(|machine| {
                            typed.symbols.display_path(machine.symbol, "::") == declaration_path
                        }) {
                            candidates.retain(|machine| {
                                typed.symbols.display_path(machine.symbol, "::") == declaration_path
                            });
                        }
                        match candidates.as_slice() {
                            [machine] => (implementation, machine.symbol),
                            [] => {
                                let probe =
                                    qualified.map_or(implementation.as_str(), |(_, path)| path);
                                let message = if typed.machines().iter().any(|machine| {
                                    machine.name.as_str() == probe
                                        || typed.symbols.display_path(machine.symbol, "::") == probe
                                }) {
                                    format!(
                                        "root binding names `{implementation}`, which is not a product declaration visible from this build source's package; borrowing Build does not grant the caller's product namespace"
                                    )
                                } else {
                                    format!(
                                        "root binding names unknown entry machine `{implementation}` in this build source's package"
                                    )
                                };
                                diagnostics.push(
                                    Diagnostic::error(message)
                                        .with_source_span(binding.source_span),
                                );
                                continue;
                            }
                            _ => {
                                diagnostics.push(
                            Diagnostic::error(format!(
                                "root binding `{implementation}` is ambiguous within its package"
                            ))
                            .with_source_span(binding.source_span),
                        );
                                continue;
                            }
                        }
                    }
                };
                if let Some(existing) = bindings.iter().find(|binding| binding.slot == slot) {
                    diagnostics.push(Diagnostic::error(format!(
                "root slot `{slot}` is already bound to `{}`; it cannot also bind `{implementation}`", existing.implementation
            )).with_source_span(binding.source_span));
                    continue;
                }
                bindings.push(RootBinding {
                    slot,
                    implementation,
                    implementation_symbol,
                });
            }
            if diagnostics.is_empty() {
                Ok(bindings)
            } else {
                Err(diagnostics)
            }
        }
    }
    pub use root_bindings::collect_root_bindings;

    pub use language_semantics::declaration_selection::RootBinding;

    pub fn validate_source_compatibility(
        typed: &TypedTrees,
        declaration: ProductEntryDeclaration<'_>,
        slot: semantic_vocabulary::ProgramEntrySourceContract,
    ) -> Result<(), Vec<Diagnostic>> {
        let declaration =
            ProductEntryDeclaration::from_declared_owner(typed, declaration.machine_symbol())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "ProgramEntry source declaration has no exact complete product owner",
                    )]
                })?;
        let machine_name = declaration.name;
        let Some(entry) = declaration.entry else {
            return Err(vec![Diagnostic::error(format!(
                "entry machine `{machine_name}` has no executable entry state"
            ))]);
        };

        let mut diagnostics = Vec::new();
        if declaration.erased_requires_inputs().next().is_some() {
            diagnostics.push(Diagnostic::error(format!(
            "ProgramEntry cannot supply erased requires evidence to entry machine `{machine_name}`"
        )));
        }
        if !declaration.type_parameters.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "entry machine `{machine_name}` is generic; a root slot must bind one exact machine"
            )));
        }
        if entry.return_type.is_valid() {
            diagnostics.push(Diagnostic::error(format!(
                "entry machine `{machine_name}` returns a value, but `ProgramEntry` has no result"
            )));
        }

        let parameters = entry.parameters;
        let visible = visible_entry_parameters(typed, parameters);
        match slot.visible_parameters {
            ProgramEntryVisibleParameters::None if !visible.is_empty() => {
                diagnostics.push(Diagnostic::error(format!(
                "hosted `ProgramEntry` exposes no arrival parameters, but `{machine_name}` declares `{}`",
                visible
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect::<Vec<_>>()
                    .join("`, `")
            )));
            }
            ProgramEntryVisibleParameters::ImageAndInitialStorage if visible.len() != 2 => {
                diagnostics.push(Diagnostic::error(format!(
                "target schema `{:?}` exposes exactly image and initial-storage roots, but `{machine_name}` declares {} visible parameter{}",
                slot.schema,
                visible.len(),
                if visible.len() == 1 { "" } else { "s" },
            )));
            }
            _ => {}
        }

        if slot.visible_parameters == ProgramEntryVisibleParameters::ImageAndInitialStorage
            && visible.len() == 2
        {
            match core_arrival_requirement_contract(typed, slot.semantic_arrival_requirement) {
            Ok(required) if required.parameters.len() == visible.len() => {
                for (index, (actual, required)) in
                    visible.iter().zip(required.parameters.iter()).enumerate()
                {
                    if typed.normalized_type_identity(actual.type_reference) != required.identity
                        || actual.is_const != required.is_const
                        || actual.is_mutable != required.is_mutable
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "target root slot `{}::{}` requires visible parameter {index} ({}) to have exact type `{}`, but entry machine `{machine_name}` declares `{}`",
                            slot.owner,
                            slot.slot_name,
                            ["image", "initial storage"][index],
                            required.display,
                            typed.display_type_reference_with_constraints(actual.type_reference),
                        )));
                    }
                }
            }
            Ok(required) => diagnostics.push(Diagnostic::error(format!(
                "target root slot `{}::{}` selects arrival requirement `{}` with {} visible parameters, but its target schema declares {}",
                slot.owner,
                slot.slot_name,
                slot.semantic_arrival_requirement,
                required.parameters.len(),
                visible.len(),
            ))),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
        }

        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        Ok(())
    }

    pub struct ArrivalRequirementParameterType {
        pub identity: typed_trees::type_identity::NormalizedTypeIdentity,
        pub display: String,
        pub is_const: bool,
        pub is_mutable: bool,
    }

    pub struct ArrivalRequirementContract {
        pub signature: symbols::SymbolHandle,
        pub requirement_identity: String,
        pub parameters: Vec<ArrivalRequirementParameterType>,
        pub result_type_identity: Option<typed_trees::type_identity::NormalizedTypeIdentity>,
    }

    pub fn core_arrival_requirement_contract(
        typed: &TypedTrees,
        requirement: &str,
    ) -> Result<ArrivalRequirementContract, Diagnostic> {
        let Some((owner, _)) = requirement.split_once("::") else {
            return Err(Diagnostic::error(
                "target semantic arrival has no requirement owner",
            ));
        };
        let mut declarations = typed
            .symbols
            .source_files()
            .filter(|file| {
                file.origin == source::SourceOrigin::Toolchain
                    && file.dependency_scope == source::DependencyScope::Product
                    && file.path.strip_prefix(&file.package_root).ok()
                        == Some(std::path::Path::new("extent.omg"))
            })
            .flat_map(|file| {
                typed.traits().iter().filter(move |definition| {
                    typed
                        .symbols
                        .symbol_source_span(definition.symbol)
                        .is_some_and(|span| span.source_id == file.source_id)
                        && definition.name.as_str() == owner
                })
            });
        let Some(definition) = declarations.next() else {
            return Err(Diagnostic::error(format!(
                "target semantic arrival `{requirement}` has no exact core declaration"
            )));
        };
        if declarations.next().is_some() {
            return Err(Diagnostic::error(format!(
                "target semantic arrival `{requirement}` has multiple core declaration instances"
            )));
        }
        arrival_requirement_contract(typed, definition, requirement)
    }

    pub fn arrival_requirement_contract(
        typed: &TypedTrees,
        schema: &typed_trees::trait_definition::TraitDefinition,
        requirement: &str,
    ) -> Result<ArrivalRequirementContract, Diagnostic> {
        let Some((owner, method)) = requirement.split_once("::") else {
            return Err(Diagnostic::error(format!(
                "target entry arrival requirement `{requirement}` is not an exact `Trait::machine` identity"
            )));
        };
        let mut inherited = vec![schema];
        let mut position = 0;
        while let Some(definition) = inherited.get(position).copied() {
            for parent in typed.trait_requirements(definition) {
                if inherited
                    .iter()
                    .any(|candidate| candidate.symbol == parent.symbol)
                {
                    continue;
                }
                if let Some(parent) = typed.trait_definition_by_symbol(parent.symbol) {
                    inherited.push(parent);
                }
            }
            position += 1;
        }
        let definitions = inherited
            .into_iter()
            .filter(|definition| definition.is_boundary && definition.name.as_str() == owner)
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            return Err(Diagnostic::error(format!(
                "target entry arrival requirement `{requirement}` resolves to {} boundary trait declarations in the selected schema instead of exactly one",
                definitions.len()
            )));
        };
        let signatures = typed
            .trait_machine_signatures(definition)
            .iter()
            .filter(|signature| signature.name.as_str() == method)
            .collect::<Vec<_>>();
        let [signature] = signatures.as_slice() else {
            return Err(Diagnostic::error(format!(
                "target entry arrival requirement `{requirement}` resolves to {} machine declarations instead of exactly one",
                signatures.len()
            )));
        };
        Ok(ArrivalRequirementContract {
            signature: signature.symbol,
            requirement_identity: typed
                .trait_requirement_callable_identity_text(definition, signature),
            parameters: typed
                .state_signature_parameters(signature)
                .iter()
                .map(|parameter| ArrivalRequirementParameterType {
                    identity: typed.normalized_type_identity(parameter.type_reference),
                    display: typed
                        .display_type_reference_with_constraints(parameter.type_reference),
                    is_const: parameter.is_const,
                    is_mutable: parameter.is_mutable,
                })
                .collect(),
            result_type_identity: signature
                .return_type
                .is_valid()
                .then(|| typed.normalized_type_identity(signature.return_type)),
        })
    }

    pub fn visible_entry_parameters<'program>(
        typed: &TypedTrees,
        parameters: &'program [typed_trees::signature::StateParameter],
    ) -> Vec<&'program typed_trees::signature::StateParameter> {
        parameters
            .iter()
            .filter(|parameter| {
                typed
                    .access_view_plan_for_input_type_reference(parameter.type_reference)
                    .is_none()
            })
            .collect::<Vec<_>>()
    }
}

pub mod native_locators {
    pub(crate) mod vocabulary {
        use diagnostics::Diagnostic;
        use source::{SourceFile, SourceOrigin};
        use std::path::Path;
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::data::{DataDefinition, DataMember, TypeParameterKind};
        use typed_trees::types::{
            FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
        };

        #[derive(Debug, Clone, Copy)]
        pub struct NativeLocatorVocabulary {
            pub binding: SymbolHandle,
            pub dll_import: SymbolHandle,
        }

        pub fn vocabulary(
            typed: &TypedTrees,
            in_scope: impl Fn(SymbolHandle) -> bool,
        ) -> Result<NativeLocatorVocabulary, Vec<Diagnostic>> {
            let exact = |name: &str| {
                typed
                    .data_definitions()
                    .iter()
                    .filter(|definition| {
                        in_scope(definition.symbol)
                            && definition.name.as_str() == name
                            && definition.generic_instance.is_none()
                            && definition.is_public
                            && definition.supply_mode
                                == language_semantics::DataSupplyMode::CheckedShape
                            && exact_external_binding_source(typed, definition.symbol).is_some()
                    })
                    .collect::<Vec<_>>()
            };
            let bindings = exact("ForeignBinding");
            let imports = exact("DllImport");
            let ([binding], [dll_import]) = (bindings.as_slice(), imports.as_slice()) else {
                return Err(vec![Diagnostic::error(
                    "native locator requires the unique compiler-owned ForeignBinding and DllImport vocabulary",
                )]);
            };
            let binding_parameters = exact_width_parameters(typed, binding)?;
            let import_parameters = exact_width_parameters(typed, dll_import)?;
            validate_dll_import_shape(typed, dll_import, import_parameters)?;
            validate_binding_shape(typed, binding, binding_parameters, dll_import.symbol)?;
            Ok(NativeLocatorVocabulary {
                binding: binding.symbol,
                dll_import: dll_import.symbol,
            })
        }

        pub fn application_widths(
            typed: &TypedTrees,
            reference: TypeReferenceHandle,
        ) -> Result<[u64; 3], Vec<Diagnostic>> {
            let application = binding_application(typed, reference)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let TypeReferenceNode::Generic { base_symbol, .. } =
                typed.type_reference_table.type_reference(application)
            else {
                return Err(vec![Diagnostic::error(
                    "native locator lost its exact generic origin",
                )]);
            };
            let binding = exact_vocabulary(typed, *base_symbol, "ForeignBinding")?;
            let import_symbol = typed
                .data_members(binding)
                .first()
                .and_then(DataMember::as_variant)
                .and_then(|variant| typed.data_payload_fields(variant).first())
                .and_then(|field| {
                    match typed
                        .type_reference_table
                        .type_reference(field.type_reference)
                    {
                        TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
                        _ => None,
                    }
                })
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "compiler-owned ForeignBinding lost its exact DllImport payload",
                    )]
                })?;
            let import = exact_vocabulary(typed, import_symbol, "DllImport")?;
            let parameters = exact_width_parameters(typed, binding)?;
            let import_parameters = exact_width_parameters(typed, import)?;
            validate_dll_import_shape(typed, import, import_parameters)?;
            validate_binding_shape(typed, binding, parameters, import.symbol)?;
            binding_widths(typed, binding.symbol, reference)
                .map_err(|message| vec![Diagnostic::error(message)])
        }

        pub fn unused_coordinate_widths(
            typed: &TypedTrees,
            symbol: SymbolHandle,
            case: SymbolHandle,
        ) -> Option<[bool; 3]> {
            if let Ok(definition) = exact_vocabulary(typed, symbol, "DllImport") {
                let parameters = exact_width_parameters(typed, definition).ok()?;
                validate_dll_import_shape(typed, definition, parameters).ok()?;
                let ordinal = typed.data_members(definition).iter().position(|member| {
                    member
                        .as_variant()
                        .is_some_and(|variant| variant.symbol == case)
                })?;
                return [
                    [false, false, true],
                    [false, true, true],
                    [false; 3],
                    [false, false, true],
                ]
                .get(ordinal)
                .copied();
            }
            let definition = exact_vocabulary(typed, symbol, "ForeignBinding").ok()?;
            let widths = exact_width_parameters(typed, definition).ok()?;
            let import_symbol = typed
                .data_members(definition)
                .first()?
                .as_variant()
                .and_then(|variant| typed.data_payload_fields(variant).first())
                .and_then(|field| {
                    match typed
                        .type_reference_table
                        .type_reference(field.type_reference)
                    {
                        TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
                        _ => None,
                    }
                })?;
            let import = exact_vocabulary(typed, import_symbol, "DllImport").ok()?;
            validate_dll_import_shape(typed, import, exact_width_parameters(typed, import).ok()?)
                .ok()?;
            validate_binding_shape(typed, definition, widths, import_symbol).ok()?;
            let ordinal = typed.data_members(definition).iter().position(|member| {
                member
                    .as_variant()
                    .is_some_and(|variant| variant.symbol == case)
            })?;
            [[false; 3], [true; 3]].get(ordinal).copied()
        }

        fn exact_vocabulary<'a>(
            typed: &'a TypedTrees,
            symbol: SymbolHandle,
            name: &str,
        ) -> Result<&'a DataDefinition, Vec<Diagnostic>> {
            let definition = typed
                .unique_data_definition_by_symbol(symbol)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "native binding lost the exact `{name}` template declaration"
                    ))]
                })?;
            let admitted = typed.symbols.name(symbol) == name
                && definition.generic_instance.is_none()
                && definition.is_public
                && definition.supply_mode == language_semantics::DataSupplyMode::CheckedShape
                && typed
                    .symbols
                    .symbol_source_span(symbol)
                    .and_then(|span| typed.symbols.source_file(span))
                    .is_some_and(|file| {
                        file.origin == SourceOrigin::Toolchain
                            && file
                                .path
                                .ends_with(std::path::Path::new("core/external_binding.omg"))
                    });
            if !admitted {
                return Err(vec![Diagnostic::error(format!(
                    "native binding requires the exact compiler-owned `{name}` declaration"
                ))]);
            }
            Ok(definition)
        }

        fn binding_application(
            typed: &TypedTrees,
            reference: TypeReferenceHandle,
        ) -> Result<TypeReferenceHandle, String> {
            match typed.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, .. } => typed
                    .unique_data_definition_by_symbol(*symbol)
                    .and_then(|definition| definition.generic_instance)
                    .ok_or_else(|| {
                        "native locator must return a closed Binding application".to_owned()
                    }),
                TypeReferenceNode::Generic { .. } => Ok(reference),
                _ => Err("native locator must return a closed Binding application".to_owned()),
            }
        }

        fn exact_external_binding_source(
            typed: &TypedTrees,
            symbol: SymbolHandle,
        ) -> Option<&SourceFile> {
            let source = typed
                .symbols
                .symbol_source_span(symbol)
                .and_then(|span| typed.symbols.source_file(span))?;
            (source.origin == SourceOrigin::Toolchain
                && source
                    .path
                    .ends_with(Path::new("core/external_binding.omg")))
            .then_some(source)
        }

        fn exact_width_parameters(
            typed: &TypedTrees,
            definition: &DataDefinition,
        ) -> Result<[SymbolHandle; 3], Vec<Diagnostic>> {
            let parameters = typed.data_type_parameters(definition);
            if parameters.len() != 3 {
                return Err(vec![Diagnostic::error(format!(
                    "compiler-owned `{}` must retain exactly three width parameters",
                    definition.name
                ))]);
            }
            let names = ["ObjectLength", "SymbolLength", "VersionLength"];
            let mut symbols = [SymbolHandle::invalid(); 3];
            for (index, (parameter, expected_name)) in parameters.iter().zip(names).enumerate() {
                let TypeParameterKind::Const { type_reference } = parameter.kind else {
                    return Err(vec![Diagnostic::error(format!(
                        "compiler-owned `{}` parameter `{expected_name}` must remain const u64",
                        definition.name
                    ))]);
                };
                if parameter.name.as_str() != expected_name
                    || typed.primitive_type_reference(type_reference) != Some(PrimitiveType::U64)
                {
                    return Err(vec![Diagnostic::error(format!(
                        "compiler-owned `{}` parameter `{expected_name}` drifted from const u64",
                        definition.name
                    ))]);
                }
                symbols[index] = parameter.symbol;
            }
            Ok(symbols)
        }

        fn validate_dll_import_shape(
            typed: &TypedTrees,
            definition: &DataDefinition,
            widths: [SymbolHandle; 3],
        ) -> Result<(), Vec<Diagnostic>> {
            let members = typed.data_members(definition);
            let expected = [
                ("PeByName", &["library", "export"][..]),
                ("PeByOrdinal", &["library", "ordinal"][..]),
                ("ElfVersioned", &["object", "symbol", "version"][..]),
                ("MachODylibSymbol", &["install_name", "symbol"][..]),
            ];
            if members.len() != expected.len() {
                return Err(vec![Diagnostic::error(
                    "compiler-owned DllImport case set drifted",
                )]);
            }
            for (index, (member, (variant_name, field_names))) in
                members.iter().zip(expected).enumerate()
            {
                let Some(variant) = member.as_variant() else {
                    return Err(vec![Diagnostic::error(
                        "compiler-owned DllImport must remain a closed case sum",
                    )]);
                };
                let fields = typed.data_payload_fields(variant);
                if variant.name.as_str() != variant_name
                    || fields.len() != field_names.len()
                    || fields
                        .iter()
                        .zip(field_names)
                        .any(|(field, name)| field.name.as_str() != *name)
                {
                    return Err(vec![Diagnostic::error(format!(
                        "compiler-owned DllImport::{variant_name} shape drifted"
                    ))]);
                }
                match index {
                    0 => {
                        require_fixed_bytes(typed, fields[0].type_reference, widths[0])?;
                        require_fixed_bytes(typed, fields[1].type_reference, widths[1])?;
                    }
                    1 => {
                        require_fixed_bytes(typed, fields[0].type_reference, widths[0])?;
                        if typed.primitive_type_reference(fields[1].type_reference)
                            != Some(PrimitiveType::U16)
                        {
                            return Err(vec![Diagnostic::error(
                                "compiler-owned PeByOrdinal ordinal must remain u16",
                            )]);
                        }
                    }
                    2 => {
                        require_fixed_bytes(typed, fields[0].type_reference, widths[0])?;
                        require_fixed_bytes(typed, fields[1].type_reference, widths[1])?;
                        require_fixed_bytes(typed, fields[2].type_reference, widths[2])?;
                    }
                    3 => {
                        require_fixed_bytes(typed, fields[0].type_reference, widths[0])?;
                        require_fixed_bytes(typed, fields[1].type_reference, widths[1])?;
                    }
                    _ => unreachable!(),
                }
            }
            Ok(())
        }

        fn validate_binding_shape(
            typed: &TypedTrees,
            definition: &DataDefinition,
            widths: [SymbolHandle; 3],
            dll_import: SymbolHandle,
        ) -> Result<(), Vec<Diagnostic>> {
            let [
                DataMember::Variant(import_variant),
                DataMember::Variant(syscall_variant),
            ] = typed.data_members(definition)
            else {
                return Err(vec![Diagnostic::error(
                    "compiler-owned Binding must retain the exact DllImport and Syscall cases",
                )]);
            };
            let [field] = typed.data_payload_fields(import_variant) else {
                return Err(vec![Diagnostic::error(
                    "compiler-owned ForeignBinding::DllImport payload drifted",
                )]);
            };
            if import_variant.name.as_str() != "DllImport" || field.name.as_str() != "import" {
                return Err(vec![Diagnostic::error(
                    "compiler-owned ForeignBinding::DllImport names drifted",
                )]);
            }
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = typed
                .type_reference_table
                .type_reference(field.type_reference)
            else {
                return Err(vec![Diagnostic::error(
                    "compiler-owned Binding payload must remain an exact DllImport application",
                )]);
            };
            let arguments = typed
                .type_reference_table
                .type_reference_handles(*arguments);
            if *base_symbol != dll_import || arguments.len() != 3 {
                return Err(vec![Diagnostic::error(
                    "compiler-owned Binding payload generic identity drifted",
                )]);
            }
            for (argument, expected) in arguments.iter().zip(widths) {
                let Some(symbol) = typed
                    .type_reference_table
                    .type_reference(*argument)
                    .as_named_symbol()
                else {
                    return Err(vec![Diagnostic::error(
                        "compiler-owned Binding widths must pass through exact const binders",
                    )]);
                };
                if symbol != expected {
                    return Err(vec![Diagnostic::error(
                        "compiler-owned Binding width binder identity drifted",
                    )]);
                }
            }
            let [number] = typed.data_payload_fields(syscall_variant) else {
                return Err(vec![Diagnostic::error(
                    "compiler-owned ForeignBinding::Syscall payload drifted",
                )]);
            };
            if syscall_variant.name.as_str() != "Syscall"
                || number.name.as_str() != "number"
                || typed.primitive_type_reference(number.type_reference) != Some(PrimitiveType::U64)
            {
                return Err(vec![Diagnostic::error(
                    "compiler-owned ForeignBinding::Syscall must retain one u64 `number` field",
                )]);
            }
            Ok(())
        }

        fn require_fixed_bytes(
            typed: &TypedTrees,
            reference: TypeReferenceHandle,
            width: SymbolHandle,
        ) -> Result<(), Vec<Diagnostic>> {
            let TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::ConstParameter { symbol, .. },
            } = typed.type_reference_table.type_reference(reference)
            else {
                return Err(vec![Diagnostic::error(
                    "compiler-owned foreign coordinate must remain a const-sized byte array",
                )]);
            };
            if *symbol != width
                || typed.primitive_type_reference(*element_type) != Some(PrimitiveType::U8)
            {
                return Err(vec![Diagnostic::error(
                    "compiler-owned foreign coordinate width or element type drifted",
                )]);
            }
            Ok(())
        }

        pub fn binding_widths(
            typed: &TypedTrees,
            binding_symbol: SymbolHandle,
            return_type: TypeReferenceHandle,
        ) -> Result<[u64; 3], String> {
            let application = match typed.type_reference_table.type_reference(return_type) {
                TypeReferenceNode::Named { symbol, .. } => typed
                    .data_definition_by_symbol(*symbol)
                    .and_then(|definition| definition.generic_instance)
                    .ok_or_else(|| {
                        "ordinary external `via` producer must return a closed Binding application"
                            .to_owned()
                    })?,
                TypeReferenceNode::Generic { .. } => return_type,
                _ => {
                    return Err(
                        "ordinary external `via` producer must return a closed Binding application"
                            .to_owned(),
                    );
                }
            };
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = typed.type_reference_table.type_reference(application)
            else {
                return Err(
                    "ordinary external `via` producer return lost its generic Binding origin"
                        .to_owned(),
                );
            };
            let arguments = typed
                .type_reference_table
                .type_reference_handles(*arguments);
            if *base_symbol != binding_symbol || arguments.len() != 3 {
                return Err("ordinary external `via` producer must return the exact compiler-owned ForeignBinding<ObjectLength, SymbolLength, VersionLength>".to_owned());
            }
            let mut widths = [0u64; 3];
            for (index, argument) in arguments.iter().enumerate() {
                let TypeReferenceNode::Named { symbol, name } =
                    typed.type_reference_table.type_reference(*argument)
                else {
                    return Err(
                        "ordinary external `via` Binding widths must be closed decimal constants"
                            .to_owned(),
                    );
                };
                if symbol.is_valid()
                    || name.as_str().is_empty()
                    || !name.as_str().bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(
                        "ordinary external `via` Binding widths must be closed decimal constants"
                            .to_owned(),
                    );
                }
                widths[index] = name.as_str().parse::<u64>().map_err(|_| {
                    "ordinary external `via` Binding width does not fit u64".to_owned()
                })?;
            }
            Ok(widths)
        }
    }
    pub use vocabulary::{
        NativeLocatorVocabulary, application_widths, binding_widths, unused_coordinate_widths,
        vocabulary,
    };
}

pub use declarations::data_queries::data_field_is_requires_independent;
pub use declarations::traits::conformance_applications;

pub use queries::execution_projection;

pub use queries::product_queries;

pub use declarations::operators::{
    OperatorApplicationScope, ScopedOperatorApplication, ScopedOperatorResolution,
    build_operator_facts, derive_checked_operator_crash_contracts,
    derive_pre_flow_operator_selections, scoped_operator_application,
};
pub use declarations::traits::refinement_fit;
pub use declarations::traits::{
    ProviderApplicationAssociation, ProviderApplicationIdentity, provider_application_associations,
};
pub use declarations::traits::{
    ProviderRegionCorrespondence, closed_provider_region_correspondence,
    provider_region_correspondence,
};
pub use machine_calls::attached_method_bounds::validate_selected_attached_method_bounds;
pub use machine_calls::{compiler_operations, contract_identities};
pub use proof_contracts::contract_entailment::specification_calls::{
    DirectMemberCall, direct_member_call, invocation_is_fact_only,
};
pub use proof_contracts::default_domains::case_predicate_expressions;
pub use proof_contracts::qualification_evidence::boundary_qualification_authorization;
pub use proof_contracts::qualification_evidence::{
    BoundaryQualificationObligation, CallQualificationContract, CallQualificationSource,
    boundary_provider_qualification_obligations, call_qualification_source,
};
pub use proof_contracts::relevance::dispatch_subject_of_erased_binding;
pub use proof_contracts::{
    contract_argument_root, contract_structure, expression_owners, mathematical_applications,
};
pub use value_custody::destructure::record_pattern_subject;
pub use value_custody::field_domains::readable_fixed_array_elements;
pub use value_custody::partial_affine_residuals::{
    CaseSelectedCleanupPartition, CaseSelectedCleanupRegion, CleanupCaseCondition,
    CleanupCaseReachability, CleanupPartitionArm, CleanupPartitionNode, reconstruct,
    reconstruct_case_selected_cleanup_partition, reconstruct_case_selected_regions,
};
pub use value_custody::places::binding_observations::{
    place_has_immutable_denotation, stable_slice_descriptor_place,
    value_reads_have_immutable_denotation,
};
pub use value_custody::places::collection_operations;
pub use value_custody::places::declared_reference_access;
pub use value_custody::places::{
    applied_data_member_field, declaration_type_reference, declared_case_projection_field,
    state_root_type,
};
pub use value_custody::static_inline_storage::{
    runtime_inline_array_extent, validate_prepared_inline_storage,
};

pub use declarations::selection_resolution;
pub use declarations::source_completion;
pub use declarations::standard_declarations::{
    boundary_has_explicit_calling_policy, standard_calling_vocabularies,
};
use declarations::state_signatures::StateSignatureOwner;
pub use machine_calls::boundary_calls::validate_boundary_calls;
pub use machine_calls::calls::{
    SelectedCallArgument, SelectedCallArgumentError, authored_call_parameters, call_returns_unit,
    call_target_has_checked_body_contract, call_type_arguments,
    closed_requirement_dispatch_is_valid, machine_has_unit_result,
    named_conformance_target_requirement, nested_boundary_applications_compose,
    requirement_contract_signature, resolve_requirement_contract,
    result_initializer_call_is_admitted, selected_call_arguments,
    selected_call_result_domain_matches, selected_requirement_signature, target_family_members,
    unit_result_initializer_call_is_admitted, unit_return_call_is_admitted,
    unit_statement_call_is_admitted,
};
pub use machine_calls::fact_only_citations::{
    fact_only_theorem, is_fact_only_citation, is_fact_only_citation_call,
    is_fact_only_requirement_citation, validate_runtime_proof_citations,
};
pub use machine_calls::static_machine_call_contracts::{
    static_machine_call_binding_bytes, static_machine_parameter_contract_bytes,
    static_machine_template_reach_contract_bytes,
};
pub use machine_calls::structural_call_custody::{
    DeclarationPredicateRow, StateContractRow, StructuralQualificationContract,
    StructuralResultContract, borrowed_slice_view_result, declaration_facts_carried,
    finite_arrival_parameter, machine_declaration_predicates,
    reconstruct_structural_parameter_return_claims, restates_structural_parameter_qualification,
    result_membership_carrier, state_declaration_predicates, state_finite_predicate,
    structural_call_custody, structural_call_result_claims, structural_claim_path,
    structural_owned_path, structural_qualification_contract, structural_result_contract,
    structural_result_projected_qualifications, structural_result_qualification_contract,
    structural_result_qualifications, structural_state_contract_predicates,
    structural_storage_path, structural_transfer_storage_path, view_result_carrier_qualifications,
    zeroed_byte_projected_qualifications,
};
pub use proof_contracts::arithmetic_domains::domain_intervals::{
    declared_domain_predicate_bounds, exact_declared_domain_carrier_interval,
    exact_declared_domain_interval,
};
pub use proof_contracts::arithmetic_domains::float_return_postcondition;
pub use proof_contracts::arithmetic_domains::normal_return_float_predicates;
pub use proof_contracts::arithmetic_domains::plain_exact_widening_cast;
pub use proof_contracts::arithmetic_domains::{
    BigIntInterval, ExpressionInterval, IntegerBound, IntegerOrderingRelation, Interval,
    StatedOrderingPremise, expression_interval, folded_integer_binary, integer_binary_range,
    integer_bound_offset, integer_bound_terms_equal, premise_chain_proves, premise_proves,
    propagate_integer_upper_bounds, prove_integer_ordering,
};
pub use proof_contracts::arithmetic_domains::{
    SelectedScalarIntervalSource, carrier_integer_range, checked_integer_binary_kind,
    integer_range_contains, selected_boolean_decision, selected_scalar_interval,
};
pub use proof_contracts::collection_predicates::{
    PredicateBooleanSubject, PredicateCondition, PredicateIntegerOperand,
    PredicateIntegerReadSubject, PredicateIntegerSubject, exact_predicate_conjunction,
    exact_predicate_intervals, exact_predicate_ordering, length_member, predicate_integer_operand,
    predicate_ordering_consequences, premises_prove_expression,
};
pub use proof_contracts::contract_entailment::call_projection_identity::{
    captured_call_argument_index_matches, captured_call_index_value,
    captured_call_subject_identity, owned_contract_subject,
};
pub use proof_contracts::contract_entailment::completed_domains::{
    completed_value_proves_bound_domain_predicate, completed_value_proves_domain_predicate,
    completed_value_refutes_domain,
};
pub use proof_contracts::contract_entailment::completed_observations::completed_value_is_bounded_by_static;
pub use proof_contracts::contract_entailment::content_conservation::{
    CompletedConservationQueries, ConditionalContentGuarantee, ContentBackingAddressGoal,
    ContentConservationDependencies, ContentConservationImplication,
    completed_value_conservation_answers, completed_value_content_conservation_dependencies,
    completed_value_has_content_conservation_candidate, completed_value_preserves_entry_projection,
    completed_value_proves_backing_addresses, completed_value_proves_content_conservation,
};
pub(crate) use proof_contracts::contract_entailment::immutable_entry_boolean_projection;
pub use proof_contracts::contract_entailment::numeric_observations::{
    CapturedIntegerOperation, CapturedNumericObservations, CapturedScalarObservation,
    NumericObservationDemand, ScalarObservationDemand, captured_numeric_observations,
};
pub use proof_contracts::contract_entailment::ranking_range::{
    captured_collection_length, captured_integer_value, closed_collection_length,
    closed_collection_length_at_place, closed_collection_length_under_matches,
    unique_literal_field,
};
pub use proof_contracts::contract_entailment::structural_prefix::{
    private_invocation_entry_is_closed, with_private_invocation_inputs,
};
pub use proof_contracts::contract_entailment::{
    ImmutableEntryReadIdentity, immutable_entry_indexed_read_identity,
    immutable_entry_integer_projection, instantiate_immutable_entry_indexed_read_identity,
};
pub use proof_contracts::contract_results::{
    ReservedResultMember, ReservedResultPlace, callable_contract_scopes, reserved_result_owner,
    reserved_result_place,
};
pub use proof_contracts::domains::{
    integer_domain_membership, scalar_domain_definition_predicate, scalar_domain_local_predicate,
    scalar_domain_predicate, scalar_domain_value_predicate, scalar_field_domain_predicate,
    scalar_field_restriction, scalar_membership_carrier, scalar_membership_is_tag,
    scalar_membership_predicates, scalar_membership_value_carrier,
    scalar_membership_value_predicates, scalar_predicate_domains, scalar_type_routed_domains,
    scalar_type_tags, validate_domain_membership_carriers,
};
pub use proof_contracts::parameter_domains::parameter_domain_constraints;
pub use proof_contracts::parameter_expression_meaning::has_builtin_parameter_bound_expression_meaning;
pub use proof_contracts::signature_ensures::{
    collect_parameter_bound_comparisons, ensures_contract_expressions,
};
use typed_trees::types::TypeReferenceHandle;
pub use value_custody::access_atomic_observing_results;
pub use value_custody::byte_observations::{ByteObservationCarrier, comparable_byte_carrier_type};
pub use value_custody::claim_frontier::{
    ClaimFrontierClaim, DeclaredClaimFrontier, declared_claim_frontier, linear_claim_frontier,
    owned_receiver_claims, type_carries_linear_obligation,
};
pub use value_custody::constants::validate_constant_literal_fields;
pub use value_custody::content_classes::classify_program_content_classes;
pub use value_custody::content_conservation::{data_variant_name, push_structural_field};
pub use value_custody::dispatch_extraction::DispatchExtraction;
pub use value_custody::field_domains;
pub use value_custody::owned_value_source::{
    ownership_source_expression, plain_owned_value_source, value_preserving_cast_operand,
    whole_owned_value_source,
};
pub use value_custody::permission_provenance::expression_permission_provenance;
pub use value_custody::receiving_values::receiving_value;
pub use value_custody::reference_shells::{
    ReferenceShell, SliceViewShell, byte_sequence_carrier, reference_shell, slice_view_shell,
    stored_view_shell,
};
pub use value_custody::scalar_case_constructor::{
    ScalarCaseConstructor, StructuralCaseConstructor, is_scalar_case_value,
    scalar_case_constructor, scalar_case_value_source, structural_case_constructor,
};
pub use value_custody::scalar_representation_range::scalar_representation_range;
pub use value_custody::source_liveness::{
    statement_uses_symbol, transition_target_uses_symbol, transition_target_uses_symbol_except,
};
pub use value_custody::storage_contents::{
    ContentsRequirement, data_has_contents, has_contents, has_owned_or_shared_view_fields,
};
pub use value_custody::struct_literals::{
    ArrayWindowShape, array_window_shape, case_predicates_admit_zero, selected_construction_case,
    selected_construction_definition, selected_construction_field,
    selected_construction_field_type, validate_value_construction,
};
pub use value_custody::structural_identity::{
    logical_structural_type_identity, stored_structural_type_identity, stores_reference_shape,
    structural_identity_type, structural_storage_type, structural_type_identity,
};
pub use value_custody::transition_operand_moves::transition_operand_moves;
pub use value_custody::type_shapes::exact_constrained_shells;
pub use value_custody::wire::validate_wire_protocol;
pub use value_custody::{
    collect_executable_value_origins, visit_executable_value_occurrences,
    visit_statement_value_occurrences,
};

pub use access_plans::AtomicAccessOperation;
pub use value_custody::atomic_operations::{
    AtomicCarrierAccess, atomic_assignment_carrier, atomic_execution_children, atomic_load_carrier,
    atomic_once_carrier, atomic_once_operands, atomic_once_result_type, atomic_once_write_place,
    atomic_resident_type_reference, atomic_result_placeholder, atomic_value_operands,
    is_atomic_type, place_is_atomic_storage,
};
pub use value_custody::places::{
    AttachedStorageRoot, ResolvedArrayExtent, ScalarCoordinateProjection, StructuralCoordinate,
    StructuralScalarCoordinate, StructuralScalarField, StructuralSource, assignment_target_indexes,
    assignment_target_selectors, attached_storage_root, canonical_storage_label,
    collection_length_receiver, declaration_storage_label, declared_collection_extent,
    ensured_range_call_endpoints, exact_attached_field, exact_attached_storage_field,
    exact_data_member_field, exact_self_field, expression_names_attached_storage,
    fixed_array_type_extent, fixed_array_type_length, has_builtin_subslice_meaning,
    indexed_operation_has_builtin_meaning, mutable_byte_slice_parameter_element,
    place_has_builtin_coordinates, range_operator_establishes_endpoint_extent,
    resolved_array_extent, resolved_array_extent_at, resolved_projected_array_extent,
    saved_dispatch_call, saved_dispatch_subject, structural_atomic_field, structural_coordinate,
    structural_coordinate_in_type, structural_scalar_coordinate,
    structural_scalar_coordinate_in_type, structural_scalar_field,
    structural_scalar_field_at_coordinate, structural_scalar_field_at_element_coordinate,
    structural_scalar_field_in_parameters, structural_scalar_field_of_evaluated_receiver,
    structural_source, subslice_expression, value_element_has_builtin_coordinate,
    value_has_builtin_coordinates,
};

pub use proof_contracts::bound_expression_meaning::{
    applied_case_membership_variant, applied_declaration_case_membership_variant,
    exact_case_reference_owner, exact_domain_subject_type, has_builtin_binary_expression_meaning,
    has_builtin_bound_expression_meaning, has_builtin_decomposed_guard_meaning,
    has_builtin_domain_decomposed_guard_meaning, has_builtin_or_sealed_float_comparison_meaning,
    has_exact_case_membership_meaning, has_exact_data_case_membership_meaning,
    has_exact_declaration_case_membership_meaning, has_exact_domain_case_membership_meaning,
    has_exact_integer_domain_subject, match_result_unique_arm, sealed_float_comparison,
    sealed_float_comparison_declaration,
};
pub use proof_contracts::contract_entailment::declaration_projection_cases_hold;
pub use proof_contracts::proposition_context::logical_proposition_children;
pub use value_custody::intrinsic_boundaries::boundary_requirement_completion;
pub use value_custody::intrinsic_boundaries::checked_shape_data_declaration;
pub use value_custody::intrinsic_boundaries::exact_byte_read_result_type;
pub use value_custody::intrinsic_boundaries::exact_compiler_intrinsic_boundary_requirement;
pub use value_custody::intrinsic_boundaries::exact_external_leaf_boundary_requirement;
pub use value_custody::intrinsic_boundaries::is_canonical_process_exit_requirement;
pub use value_custody::intrinsic_boundaries::valid_boundary_completion_declaration;
pub use value_custody::intrinsic_boundaries::validate_boundary_completion_declarations;

pub use crate::declarations::declaration_visibility::validate_declaration_visibility;
pub use crate::declarations::selection_authority::{
    DeclarationSelectionAuthority, RecordedDeclarationAuthority, checked_selection_requirements,
    validate_authored_declaration_selections,
};
pub use crate::machine_calls::call_cycles::requirement_backing_machines;
pub use crate::machine_calls::call_cycles::{
    RankRelevance, RuntimeRankApplicationBinding, RuntimeRankBodyWitness, RuntimeRankComponents,
    RuntimeRankInput, RuntimeRankOperand, ValidatedProofCallComponent, ValidatedProofRankInterval,
    ValidatedProofRankingRelation, ValidatedProofRecursiveCallSite,
    ValidatedProofRecursiveComponent, ValidatedProofRecursiveEdge, ValidatedProofRecursiveMember,
    ValidatedProofRecursiveTransitionLane, ValidatedRuntimeIntegerRank,
    ValidatedRuntimeNaturalComponent, ValidatedRuntimeNaturalEdge, ValidatedRuntimeNaturalState,
    activation_target_state_index, infer_checked_proof_recursive_machines, internal_state_index,
    machine_call_dependency_symbols, prepare_runtime_rank_components, rank_relevance,
    retain_runtime_rank_component, retained_internal_rank_nonincrease,
    retained_runtime_natural_components, retained_runtime_recursive_components,
};
pub use crate::machine_calls::calls::{
    AssignmentStorageOrigins, AssignmentWriteTarget, CallFrameResolver, CallWriteOrigins,
    InvocationResultHome, LocalWriteOrigin, LocalWriteSource, RecastSource,
    ReferenceResultLeafOrigin, ReferenceResultTransport, ReferenceSlotContract,
    ReferenceViewFormation, ScalarStorageCoordinate, StoredReferenceCallContext,
    StoredReferenceDestinationKind, StoredReferenceEffect, StoredReferenceRelationRefusal,
    ValidatedSelfCallDescent, assignment_rebinds_parameter, assignment_stored_value_type,
    frame_coordinate_path, frame_length_coordinate_path, frame_paths_overlap, operand_call_plans,
    state_reference_parameter_binding_is_stable, storage_origin_types_match,
};
pub use crate::machine_calls::denotational_calls::{
    SpecificationCallRequest, admit_specification_calls, checked_value_call_candidate,
    expression_values_have_call_identity, normal_return_call_candidate, requirement_satisfiers,
    selected_operator_has_closed_value_denotation,
};
pub use crate::machine_calls::fact_call_projections::has_checked_denotational_call_identity;
pub use crate::proof_contracts::slice_ranking::slice_tail_strictly_decreases;
pub use crate::value_custody::cleanup::{
    data_cleanup_machine, nominal_cleanup_machine_for_type, nominal_cleanup_owner,
    nominal_cleanup_target, validate_reserved_cleanup_selections,
};
pub use crate::value_custody::content_conservation::{
    ContentBoundaryBodyRequirement, ContentConservationSourcePlan,
    boundary_body_content_requirements, content_conservation_plans,
};
pub use crate::value_custody::content_projections::{
    ContentAlgebraApplication, build_content_projection_plans, content_algebra_application,
    content_algebra_for_type,
};

pub use crate::declarations::symbols::TopLevelSymbols;
pub use crate::value_custody::cleanup::{
    data_requires_nominal_drop, has_no_code_cleanup_contents, has_no_owned_cleanup_obligations,
    has_plain_contents_without_nominal_cleanup, type_graph_requires_nominal_drop,
};
pub use crate::value_custody::data::{data_requires_establishment, type_requires_establishment};
pub use crate::value_custody::type_references::{
    StaticTypeArgumentOwner, scalar_type_refinement_predicates, scalar_value_domain_predicate,
    validate_closed_const_argument, validate_exact_const_value_encoding,
    validate_static_type_argument,
};
pub use declarations::definition_facts::build_definition_fact_plan;
pub use declarations::operators::{
    ValidatedBoundaryOperatorApplication, ValidatedBoundaryOperatorApplicationArgument,
    ValidatedBoundaryOperatorApplicationUseSite, canonical_closed_operator_realization_bytes,
    landed_integer_literal_type_reference, validate_closed_operator_application,
    validate_named_operator_application,
};
pub use declarations::operators::{
    build_operator_uses, checked_operator_candidate,
    collect_pre_specialization_operator_selections, comparison_operator_spelling,
    expression_type_reference_for_origin, indexed_operand_types,
    select_pending_domain_operator_meanings, token_bound_machine_call_target,
};
pub use declarations::traits::{
    DynamicImplementationSelection, DynamicSlotDemand, collect_dynamic_implementation_selections,
    collect_dynamic_slot_demands, compose_forwarded_trait_arguments, describe_dynamic_conformances,
    dynamic_call_demand, dynamic_expression_call_demand, generic_bound_operator_requirement,
    normalized_dynamic_row_identities, qualified_trait_scope,
    resolve_closed_requirement_application, resolve_dynamic_call_targets,
    resolve_selected_dynamic_call_targets, revalidate_machine_trait_conformances,
    revalidate_named_implementation, revalidate_top_level_requirement_realization,
};
pub use machine_calls::effect_inference::{
    declared_machine_invocations, declared_signature_invocations,
    fixed_installation_boundary_service_reach, has_self_forwarded_boundary_parameter,
    infer_operational_may, infer_operational_may_for_dependencies, infer_service_reaches,
    infer_synchronous_invocations, invocation_target_label, trait_signature_service_reach,
};
pub use machine_calls::effects::{
    AsmAuthorityAdmission, validate_asm_discharge, validate_behavior_plan,
};
pub use machine_calls::machine_parameters::infer_call_type_arguments;
pub use machine_calls::machine_parameters::{
    PreparedInstalledInterruptRegistration, PreparedStaticMachineSelections,
    StaticMachineContractObligation, ValidatedNominalMachineUse, ValidatedNominalMachineUseSite,
    ValidatedRequirementCallMachineSelection, ValidatedRequirementCallSpecialization,
    ValidatedRequirementCallTypeBinding, closed_static_call_type_bindings,
    validate_static_machine_selections,
};
pub use machine_calls::machine_specialization_identity::machine_specialization_matches_template_identity;
pub use machine_calls::result_overloads::{
    bind_float_conversion_casts, collect_expected_expression_calls, resolve_named_result_overloads,
};
pub use machine_calls::static_arguments;
pub use machine_calls::static_bindings;
pub use proof_contracts::default_domains::{
    OpenInvariantCrashSite, data_float_predicate, data_predicates_admit_zero,
    standing_facts_prove_requirement,
};
pub use proof_contracts::float_projection_bindings::semantic_operations::{
    exact_toolchain_float_semantic_contract, float_semantic_application_for_parameters,
};
pub use proof_contracts::float_projection_bindings::semantic_values::closed_float_semantic_integer_operand;
pub use proof_contracts::float_projection_bindings::semantic_values::exact_toolchain_float_format_const;
pub use proof_contracts::float_projection_bindings::{
    exact_toolchain_float_projection_contract, exact_toolchain_float_projection_primitive,
    float_boolean_semantic_contract, float_format_conversion_contract,
    float_integer_conversion_bounds, float_integer_conversion_contract,
    float_result_semantic_contract, float_saturation_divisor_parameter,
    integer_float_conversion_bounds, is_exact_toolchain_float_meaning_type,
    numeric_float_conversion_contract,
};
pub use proof_contracts::float_projection_invocations::exact_projection_operation as exact_toolchain_float_projection_operation;
pub use proof_contracts::properties::{
    DeclaredPropertyRequirement, OpaqueDataPropertyReceipt, OpaqueDataPropertyReceiptKind,
    TypeCarryRequirements, declared_property_requirements, effective_data_carry_policy,
    effective_type_carry_policy, effective_type_carry_requirements,
    substituted_type_satisfies_copy, type_satisfies_declared_property,
};
pub use proof_contracts::proposition_entailment::select_named_evidence_implementation;
pub use proof_contracts::quotients::{
    CheckedTerminationOracle, QuotientRequestAdmission, ValidatedQuotientFormation,
    ValidatedQuotientOperation, admit_checked_quotient_requests,
    checked_quotient_relation_identity, validate_quotient_formations,
};
pub use value_custody::constants::completed_integer_constant;
pub use value_custody::expression_types::argument_matches_type_reference_handle as checked_argument_matches_type_reference;
pub use value_custody::expression_types::borrow_selects_reference_storage;
pub use value_custody::expression_types::bounded_byte_buffer_capacity;
pub use value_custody::expression_types::match_subject_primitive_type;
pub use value_custody::expression_types::validate_match_dispatch;
pub use value_custody::expression_types::{
    arithmetic_result_type_reference, expression_result_type_reference,
    join_result_type_references, parameter_expression_result_type_reference,
    parameter_expression_result_type_reference_in_scope, scalar_observation_meaning,
};
pub use value_custody::expression_types::{
    structural_match_cases_are_covered, structural_match_pattern_case,
    structural_match_pattern_is_total, structural_match_subject_type,
};
pub use value_custody::literals::builtin_constant_array_projection_type;
pub use value_custody::literals::declared_constant_array_type;
pub use value_custody::literals::evaluate_anonymous_numeric_comparison;
pub use value_custody::literals::evaluate_anonymous_numeric_expression;
pub use value_custody::literals::has_anonymous_numeric_results;
pub use value_custody::literals::has_anonymous_operator_meaning;
pub use value_custody::literals::land_anonymous_integer_expression;
pub use value_custody::literals::land_float_literal_destinations;
pub use value_custody::literals::land_integer_value;
pub use value_custody::literals::select_anonymous_numeric_match_arm;
pub use value_custody::literals::validate_anonymous_numeric_formation;
pub use value_custody::literals::{
    ScalarArrayElement, ScalarArrayElements, closed_array_store_type,
    closed_constant_array_elements, closed_literal_array_elements, closed_record_scalar_projection,
    closed_scalar_place_projection, is_closed_primitive_array_type, peel_arithmetic_policy,
    scalar_array_elements, scalar_array_preserves_memberships,
};
pub use value_custody::logical_matches::{
    LogicalMatchBinding, LogicalRecordCapture, expression_has_closed_value_captures,
    logical_record_match_capture, logical_saved_subject_projection,
};
pub use value_custody::places::ownership_place_from_expression_in_state;
pub use value_custody::places::{assignment_value_type, declared_place_type_raw};
pub use value_custody::places::{
    declared_carriers_match, peel_declared_constraints, unwrapped_type_reference,
    zeroed_structural_parts,
};
pub use value_custody::recasts::{
    RecastLeafSegment, RecastRecordSourceLeaf, RecastRuntimeIndexGeometry, RecastScalarGeometry,
    RecastScalarLayout, RecastScalarProjection, ReferenceFormationInterpretation,
    ReferenceFormationSource, SliceRecastByteRegion, SliceRecastSource,
    ValidatedLiteralIndexedRecastFootprint, access_borrow_has_empty_write_frame,
    recast_record_source_leaf, recast_scalar_layout, recast_scalar_path_geometry,
    recast_scalar_path_projection, recast_scalar_projection, reference_formation_source,
    reference_view_storage, slice_recast_byte_region, slice_recast_source,
    validate_literal_indexed_recast_footprint, validate_runtime_indexed_recast_source,
};
pub use value_custody::resident_content::{
    ResidentOwnerQualifications, is_extent_declaration, resident_owner_qualifications,
};
pub use value_custody::type_references::normalize_open_index_expressions;
pub use value_custody::type_references::{
    closed_float_range_endpoint, closed_integer_range_bound, closed_integer_range_maximum,
    ieee_float_range_ordered, is_arithmetic_policy_only_integer, is_plain_scalar_storage,
    scalar_result_contract_is_represented, unrestricted_builtin_primitive,
};

pub use proof_contracts::arithmetic_domains::arrival_integer_expression_bounds;
pub use proof_contracts::arithmetic_domains::builtin_monotonic_integer_update_bounds;
pub use proof_contracts::arithmetic_domains::enforced_integer_type_bounds;
pub use proof_contracts::arithmetic_domains::evaluated_call_requires_proven;
pub use proof_contracts::arithmetic_domains::immutable_integer_expression_bounds;
pub use proof_contracts::arithmetic_domains::integer_widen_is_total;
pub use proof_contracts::arithmetic_domains::normal_return_integer_bounds;
pub use proof_contracts::arithmetic_domains::normal_return_length_collection;
pub use proof_contracts::arithmetic_domains::occurrence_integer_expression_fits_range;
pub use proof_contracts::arithmetic_domains::occurrence_integer_expressions_bounds;
pub use proof_contracts::arithmetic_domains::state_parameter_integer_interval;
pub use proof_contracts::arithmetic_domains::validate_ordered_requirement_call_totality;
pub use proof_contracts::arithmetic_domains::{OccurrenceIntegerBounds, occurrence_integer_bounds};
pub use proof_contracts::arithmetic_domains::{
    StoredIntegerReach, UnestablishedStoredEncoding, standing_integer_bounds,
    stored_integer_bounds, stored_integer_source_projections, stored_integer_source_reach,
};
pub use proof_contracts::arithmetic_domains::{
    occurrence_integer_expression_bounds, occurrence_integer_expression_recipe,
};
pub use proof_contracts::contract_entailment::arithmetic_judgment::IntegerDenotation as CapturedIntegerDenotation;
pub use proof_contracts::contract_entailment::call_requirements::selected_value_call_requirement_entailed;
pub use proof_contracts::contract_entailment::call_requirements::structural_call_requirement_entailed;
pub use proof_contracts::contract_entailment::collection_bounds::{
    CollectionBoundsDemands, CollectionBoundsObservations,
};
pub use proof_contracts::contract_entailment::exit_predicates::{
    NormalExitContract, normal_exit_proves,
};
pub use proof_contracts::contract_entailment::is_arm_pattern_marker;
pub use proof_contracts::contract_entailment::law_conformance::{
    InheritedRequirementApplication, inherited_requirement_proposition_application,
    inherited_requirement_proposition_label, inherited_satisfier_parameters,
};
pub use proof_contracts::contract_entailment::law_conformance::{
    MatchedLawGuarantee, matched_machine_law_guarantees,
};
pub use proof_contracts::contract_entailment::proof_integer::{
    anonymous_integer_math_literal, proof_integer_expression,
};
pub use proof_contracts::contract_entailment::ranking_range::{
    ArithmeticCallObservation, CapturedRankApplicationInput, CapturedRankEntry,
    RankArrivalCompatibility, RankArrivalCorrespondence, RankArrivalDemand, RankCoordinateBinding,
    RankCoordinateTransport, RankLocalCapture, RankPremiseCoordinate, RankPremiseInput,
    RankPremiseObservation, RankProtectedPath, RankReceivingCoordinate, RankStateBindings,
    RankSubjectCoordinate, RankValueDerivation, RankingRangeEdgeProof, RankingRangeEntryProof,
    RankingRangeMeasure, RankingRangePremises, RankingRangeState, StateEntryCorrespondence,
    arithmetic_entry_requirement_is_covered, discover_rank_arrival_correspondence,
    frame_preserves_rank_premises, protect_rank_premise, prove_arithmetic_call_requirement,
    prove_captured_arithmetic_call_requirement, prove_captured_ranking_range_entry,
    prove_captured_ranking_range_entry_with_formation, prove_ranking_range_edge,
    prove_ranking_range_entry, prove_ranking_range_entry_with_formation,
    prove_ranking_range_transition, rank_protected_path, ranking_coordinate_type,
    ranking_range_arrival_preserves_roles, ranking_range_premise_symbols,
    ranking_range_required_coordinates, ranking_range_state_premise_coordinates,
    ranking_slice_length_observation, select_rank_arrival_correspondence,
};
pub use proof_contracts::contract_entailment::ranking_range::{
    ComputationBodyShape, DeclaredInputRead, DeclaredInputShape, DeclaredScalarView,
    MeasureBodyShape, ProjectionStep, ScalarViewComputation, computation_body_shape,
    declared_input_shape, declared_input_type_matches, declared_scalar_view, find_declared_measure,
    identity_subject_matches, measure_body_shape, measure_constraints_cover_subject,
};
pub use proof_contracts::contract_entailment::refuted_requires::instantiated_value_call_requires_proven;
pub use proof_contracts::contract_entailment::scoped_arithmetic::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticComparison,
    ScopedArithmeticExpression, ScopedArithmeticHypothesis, ScopedArithmeticValue,
    ScopedCallCompletion, ScopedCallResultObservation, scoped_arithmetic_comparisons_unsatisfiable,
    scoped_arithmetic_implication, scoped_arithmetic_value_bounds,
    scoped_arithmetic_values_relation, scoped_call_result_observations,
    scoped_completed_call_guarantees, scoped_declared_integer_application_formation,
    scoped_integer_expression_formation,
};
pub use proof_contracts::contract_entailment::specification_calls::call_contract_parameter_position;
pub use proof_contracts::contract_entailment::statement_predicates::{
    EntryPredicateObservations, StatementPredicateObservations,
};
pub use proof_contracts::contract_entailment::strict_arithmetic::{
    StrictArithmeticBindingValue, StrictArithmeticExpressionBinding,
    StrictArithmeticImplicationJudgment, StrictArithmeticSymbolBinding,
    strict_arithmetic_expression_implication,
};
pub use proof_contracts::contract_entailment::structural_judgment::owned_snapshots::{
    CapturedOwnedSnapshot, captured_call_owned_snapshot, captured_entry_owned_snapshot,
};
pub use proof_contracts::contract_entailment::transparent_applications::{
    InheritedRequirementEdge, transparent_call_requirement_entailed,
    transparent_proposition_application_entailed,
};
pub use proof_contracts::default_domains::{
    case_requires_field_intervals, data_requires_field_intervals, impossible_variant,
};
pub use proof_contracts::float_projection_invocations::{
    ValidatedFloatMeaningEqualityProposition, ValidatedFloatMeaningProjectionInvocation,
    float_static_call_resolution,
};
pub use proof_contracts::immutable_integer_bounds::{
    ImmutableBoundLookup, immutable_integer_bound_sum, immutable_integer_bound_symbol_offset,
    immutable_integer_bound_value_symbol, indexed_integer_bound_root,
    mutable_integer_bound_storage_symbol, normalize_immutable_integer_bound_expression,
    normalize_immutable_integer_bound_to_usize, projected_integer_bound_root,
    projected_integer_bound_subject_type,
};
pub use proof_contracts::proof_embeddings::{
    ValidatedIntegerEmbeddingCall, integer_embedding_argument,
};

pub(crate) use optimization::digest_encoder;
pub use queries::expression_traversal;

pub(crate) use queries::layout_checking::layout_checking_targets;
pub use queries::layout_checking::{LayoutCheckingContext, pending_layout_geometry};

pub use queries::substitutions;
pub use queries::type_applications::DeclarationScopes;

pub use frozen_program::{
    CheckingContext, CheckingInput, CheckingMemos, CheckingProjection, proof_only_classification,
};
pub use machine_calls::effect_inference::PreparedEffectPlans;
pub use program_validation::{
    ContractEntailment, DeferredEntailment, DependencyBodyJudgment, ExactIntegerCastFact,
    FrozenDependencyBody, OpaquePropertyValidation, ProgramValidation, ProgramValidationFacts,
    ProvenContractExpressions, admit_prepared_program_effects, checking_thread_pool,
    dependency_body_read_totals, dependency_entailment_totals, judge_contract_entailment,
    prepare_specialized_program, statement_source_span,
    validate_checked_operator_realization_contract, validate_generic_machine_contract_entailment,
    validate_program,
};
pub(crate) use program_validation::{DependencyBodyRead, note_dependency_body_read};

pub use proof_contracts::scalar_boundary_guarantees::derive_scalar_boundary_guarantees;

pub use machine_calls::calls::{
    CallSite, CompletingTransfer, call_is_unconditional, call_receiver_parts, call_result_type,
    call_site_argument_expressions, call_site_evidence_arguments, call_target_entry_symbol,
    call_target_parameters, call_target_return_type, call_target_state, call_target_symbol,
    call_target_type_parameters, collect_call_sites_in_statement, completing_call_transfer,
    completing_transition_invocation, completing_transition_invocations,
    completing_transition_transfer, expression_normal_completion_possible, find_call_site,
    find_call_site_in_state, find_machine, find_machine_by_entry_state, find_machine_head,
    find_state, find_state_for_exit, find_state_in_machine, find_state_on_machine,
    invocation_output_parameter, machine_parameter_signature_symbol, machine_state_by_symbols,
    machine_state_count, normal_completion_possible, receiver_can_dispatch_to_machine,
    resolve_name_path_member_symbol, resolve_state_call_target,
    statement_call_can_dispatch_to_machine, statement_call_receiver_members,
    statement_call_receiver_path, terminal_callable_target, trait_machine_signature_symbol,
    transition_call_target, unique_dispatched_entry,
};
pub use queries::expression_traversal::{
    append_fact_expression_roots, collect_contract_facts, collect_expression_tree,
    collect_statement_expression_trees, collect_transition_target_expression_trees,
    executable_subtree_any, is_single_segment_name_path, single_segment_name_symbol,
};
pub use value_custody::places::{
    CanonicalPlace, RuntimeFieldSubject, StorageObservationTypePath,
    StorageObservationTypePathError, canonical_owned_place_type_reference,
    canonical_place_from_expression, canonical_place_from_expression_in_state,
    canonical_place_source_type_application, canonical_place_type_reference,
    canonical_qualification_path, collection_element_type_reference,
    contextual_canonical_place_from_expression, corresponding_loan_owner_path,
    corresponding_place_projection, data_member_symbol_by_name, declared_data_type,
    effective_member_symbol, expression_place_type_reference, expression_root_symbol,
    expression_type_reference_in_state, expression_type_symbol, first_valid_name_path_symbol,
    index_place_segment, loan_carrier_has_owned_storage, machine_symbol_from_type_reference_handle,
    nearest_scoped_local, observed_storage_type_references, owned_projection_type_reference,
    parameter_source_type_application, place_root_may_be_symbol,
    project_type_reference_from_segments, projected_statement_receiver_place,
    projected_statement_receiver_type_reference, push_field_place_segments,
    qualification_storage_path, qualification_storage_type_reference, receiver_projections,
    resolve_member_symbol_from_place, resolve_member_symbol_from_type_symbol,
    runtime_field_subject, source_type_application_from_segments, state_scoped_name_symbol,
    storage_operand, storage_place_from_expression_in_state, structural_member_identity,
    symbol_type_symbol, unique_data_definition_by_symbol, whole_collection_view_source,
};

pub(crate) fn classify_type_references<R: Clone, F>(
    program: &dyn crate::CheckingInput,
    classify: impl Fn(&dyn crate::CheckingInput, TypeReferenceHandle) -> R,
    with_rows: impl FnOnce(Vec<R>) -> F,
) -> F {
    let table = &program.type_reference_table;
    let highest = table
        .type_reference_rows()
        .map(|(handle, _)| handle.arena_index())
        .max()
        .unwrap_or(0);
    let mut rows = vec![classify(program, TypeReferenceHandle::invalid()); highest as usize + 1];
    for (handle, _) in table.type_reference_rows() {
        rows[handle.arena_index() as usize] = classify(program, handle);
    }
    with_rows(rows)
}

pub use value_custody::activation_storage::{
    ActivationStorageContract, ActivationStorageOperation,
    activation_accessor_has_empty_write_frame, activation_claim_byte_carrier,
    activation_storage_bound, exact_toolchain_activation_storage_contract,
    type_carries_activation_claim,
};
pub use value_custody::atomic_fences::{
    AtomicFenceContract, exact_toolchain_atomic_fence_contract,
};

pub use proof_contracts::instruction_effects::{
    InstructionEffectContract, instruction_effect_contract, instruction_effect_model,
};

pub use proof_contracts::arithmetic_domains::{
    DeclaredFloatFieldBound, declared_float_field_bounds,
};

pub use machine_calls::contract_dependencies::{
    independent_call_application, independent_call_site,
};
pub use value_custody::type_references::{
    check_closed_const_expression, normalize_closed_const_expression,
};

pub use value_custody::const_expressions::{
    ConstProjectionStep, ImmutableConstProjection, fixed_array_geometry, immutable_const_projection,
};

pub use value_custody::completed_storage;
pub use value_custody::places::storage_path_relations;

pub use value_custody::membership_evidence;

pub use machine_calls::terminal_transitions::normalize_terminal_transition_calls;

pub use proof_contracts::bound_expression_meaning::{
    case_membership_type_facts, static_type_boolean_equalities,
};

pub use value_custody::literals::land_owned_fixed_byte_literal;

pub use proof_contracts::contract_entailment::IntegerApplicationFormation;
