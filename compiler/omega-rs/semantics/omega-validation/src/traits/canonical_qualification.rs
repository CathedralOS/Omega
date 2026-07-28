use crate::type_references::type_references_match;
use omega_core::diagnostics::Diagnostic;
use omega_core::semantics::{
    DomainEstablishmentRoute, DomainPredicateBody, TerminationGuarantee, TerminationInterface,
    TraitConformanceSemanticRole, TraitSemanticRole,
};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::signature::SignatureContractKind;
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use omega_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use std::collections::{HashSet, VecDeque};

pub(crate) fn validate_core_qualification_trait(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for definition in program.traits().iter().filter(|definition| {
        definition.semantic_role == TraitSemanticRole::RepresentationQualification
    }) {
        let parameters = program.trait_type_parameters(definition);
        let requirements = program.trait_machine_signatures(definition);
        let shape_is_exact = parameters.len() == 1
            && parameters[0].name.as_str() == "Q"
            && requirements.len() == 1
            && requirements[0].name.as_str() == "qualify"
            && requirements[0].terminates_guarantee
            && program
                .service_reach_rows
                .services(requirements[0].service_reach_row)
                .is_empty()
            && !requirements[0].suspends
            && !requirements[0].blocks
            && program.state_signature_parameters(&requirements[0]).len() == 1
            && {
                let input = &program.state_signature_parameters(&requirements[0])[0];
                !input.is_self
                    && !input.is_mutable
                    && named_type_is(program, input.type_reference, "Self", None)
            }
            && named_type_is(
                program,
                requirements[0].return_type,
                "Q",
                Some(parameters[0].symbol),
            );
        if !shape_is_exact {
            diagnostics.push(Diagnostic::error(
                "toolchain core trait `RepresentationQualification<Q>` has an invalid \
                 declaration: it must contain only `machine qualify(value: Self) -> Q \
                 terminates;` with an empty operational contract"
                    .to_owned(),
            ));
        }
    }
}

pub(crate) fn validate_canonical_qualification_conformance(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for conformance in program.machine_trait_conformances(machine) {
        let TraitConformanceSemanticRole::RepresentationQualification {
            domain,
            home_authorized,
        } = conformance.semantic_role
        else {
            continue;
        };
        let label = format!(
            "machine `{}` canonical representation qualification",
            machine.name
        );
        let [qualified_type] = program
            .type_reference_table
            .type_reference_handles(conformance.arguments)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "{label} must supply exactly one qualified type argument `Q`"
            )));
            continue;
        };
        let Some((carrier, qualified_domain)) = exact_qualified_type(program, *qualified_type)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "{label} requires `Q` to add exactly one normalized declared domain to its carrier"
            )));
            continue;
        };
        if !domain.is_valid() || qualified_domain != domain {
            diagnostics.push(Diagnostic::error(format!(
                "{label} does not identify one bodyless domain whose carrier is `Q` without its qualification"
            )));
            continue;
        }
        if !home_authorized {
            diagnostics.push(Diagnostic::error(format!(
                "{label} is declared outside the domain-owning package; third-party \
                 satisfiers are named-only and cannot open another package's domain"
            )));
        }
        let Some(domain_definition) = program
            .domain_definitions()
            .iter()
            .find(|candidate| candidate.symbol == domain)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "{label} references an unknown normalized domain"
            )));
            continue;
        };
        if domain_definition.predicate_body != DomainPredicateBody::Bodyless {
            diagnostics.push(Diagnostic::error(format!(
                "{label} cannot establish bodyful domain `{}`; its predicate must be proved at each use",
                domain_definition.name
            )));
            continue;
        }
        if !type_references_match(program, carrier, domain_definition.target_type) {
            diagnostics.push(Diagnostic::error(format!(
                "{label} changes carrier type: erasing `Q` must yield the domain carrier exactly"
            )));
        }
        let has_route = domain_definition.establishment_routes.iter().any(|route| {
            matches!(
                route,
                DomainEstablishmentRoute::CanonicalQualification { satisfier }
                    if *satisfier == machine.symbol
            )
        });
        if home_authorized && !has_route {
            diagnostics.push(Diagnostic::error(format!(
                "{label} was not retained as the domain's normalized establishment route"
            )));
        }

        let Some(entry) = program.machine_states(machine).first() else {
            diagnostics.push(Diagnostic::error(format!("{label} has no entry state")));
            continue;
        };
        let parameters = program.state_parameters(entry);
        if parameters.len() != 1
            || parameters[0].is_self
            || parameters[0].is_mutable
            || matches!(
                program
                    .type_reference_table
                    .type_reference(parameters[0].type_reference),
                TypeReferenceNode::Reference { .. }
            )
            || !type_references_match(program, parameters[0].type_reference, carrier)
        {
            diagnostics.push(Diagnostic::error(format!(
                "{label} must take exactly one immutable by-value input of the unqualified carrier"
            )));
        }
        if !type_references_match(program, entry.return_type, *qualified_type) {
            diagnostics.push(Diagnostic::error(format!(
                "{label} must return its exact qualified type argument `Q`"
            )));
        }
        if !program.machine_owned_data(machine).is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "{label} cannot own or mutate machine data"
            )));
        }
        if !program.machine_effects(machine).is_empty()
            || !program
                .service_reach_rows
                .services(machine.service_reach_row)
                .is_empty()
            || machine.suspends
            || machine.blocks
        {
            diagnostics.push(Diagnostic::error(format!(
                "{label} must have an empty service reach and cannot suspend or block"
            )));
        }
        if !matches!(
            machine.termination_plan.interface,
            TerminationInterface::Published(TerminationGuarantee::EventualTerminal {
                ref premises
            }) if premises.is_empty()
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "{label} must inherit an unconditional `terminates` guarantee"
            )));
        }
        if program.machine_contracts(machine).iter().any(|contract| {
            contract.kind == SignatureContractKind::Requires && !contract.facts.is_empty()
        }) || program.machine_states(machine).iter().any(|state| {
            program.state_contracts(state).iter().any(|contract| {
                contract.kind == SignatureContractKind::Requires && !contract.facts.is_empty()
            })
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "{label} cannot require a use-site precondition"
            )));
        }
        if parameters.len() == 1 {
            validate_unchanged_lineage(
                program,
                machine,
                entry.symbol,
                parameters[0].symbol,
                diagnostics,
            );
        }
    }
}

fn exact_qualified_type(
    program: &TypedTrees,
    qualified: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, SymbolHandle)> {
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(qualified)
    else {
        return None;
    };
    let [TypeConstraintNode::Domain(domain)] =
        program.type_reference_table.constraints(*constraints)
    else {
        return None;
    };
    atomic_domain_symbol(program, domain.symbol).map(|domain| (*base_type, domain))
}

fn atomic_domain_symbol(program: &TypedTrees, domain: SymbolHandle) -> Option<SymbolHandle> {
    fn expand(
        program: &TypedTrees,
        domain: SymbolHandle,
        stack: &mut Vec<SymbolHandle>,
        output: &mut Vec<SymbolHandle>,
    ) {
        if !domain.is_valid() || stack.contains(&domain) {
            return;
        }
        let Some(definition) = program
            .domain_definitions()
            .iter()
            .find(|candidate| candidate.symbol == domain)
        else {
            return;
        };
        let Some(alias) = definition.alias.as_ref() else {
            if !output.contains(&domain) {
                output.push(domain);
            }
            return;
        };
        stack.push(domain);
        for constituent in &alias.constituents {
            expand(program, constituent.domain_symbol, stack, output);
        }
        stack.pop();
    }

    let mut atoms = Vec::new();
    expand(program, domain, &mut Vec::new(), &mut atoms);
    let [atom] = atoms.as_slice() else {
        return None;
    };
    Some(*atom)
}

fn named_type_is(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    expected_name: &str,
    expected_symbol: Option<SymbolHandle>,
) -> bool {
    matches!(
        program.type_reference_table.type_reference(type_reference),
        TypeReferenceNode::Named { symbol, name }
            if name.as_str() == expected_name
                && expected_symbol.is_none_or(|expected| *symbol == expected)
    )
}

fn validate_unchanged_lineage(
    program: &TypedTrees,
    machine: &Machine,
    entry_symbol: SymbolHandle,
    input_symbol: SymbolHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let proof_only = omega_typed_trees::proof_only::classify(program);
    let mut queue = VecDeque::from([(entry_symbol, vec![input_symbol.arena_index()])]);
    let mut visited = HashSet::new();
    let mut reached_states = HashSet::new();
    let mut returned = false;
    let mut reported = HashSet::new();

    while let Some((state_symbol, incoming_lineage)) = queue.pop_front() {
        let key = (state_symbol.arena_index(), incoming_lineage.clone());
        if !visited.insert(key) {
            continue;
        }
        let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
        else {
            continue;
        };
        reached_states.insert(state.symbol.arena_index());
        let mut lineage = incoming_lineage.into_iter().collect::<HashSet<_>>();

        for parameter in program.state_parameters(state) {
            if parameter.is_mutable {
                push_once(
                    diagnostics,
                    &mut reported,
                    "mutable-parameter",
                    format!(
                        "machine `{}` canonical representation qualification cannot use mutable state parameters",
                        machine.name
                    ),
                );
            }
        }

        let statements = program.statement_table.statements(state.statement_nodes);
        for (statement_index, statement) in statements.iter().enumerate() {
            match statement {
                StatementNode::LocalData(local) => {
                    if local.is_mutable {
                        push_once(
                            diagnostics,
                            &mut reported,
                            "mutable-local",
                            format!(
                                "machine `{}` canonical representation qualification cannot mutate locals",
                                machine.name
                            ),
                        );
                    }
                    if expression_retains_lineage(program, local.initial_value, &lineage) {
                        lineage.insert(local.symbol.arena_index());
                    } else if !expression_is_proof_call(
                        program,
                        machine,
                        local.initial_value,
                        &proof_only,
                    ) {
                        push_once(
                            diagnostics,
                            &mut reported,
                            "runtime-local",
                            format!(
                                "machine `{}` canonical representation qualification may only alias the input or cite proof-only machines; transformation and reconstruction are forbidden",
                                machine.name
                            ),
                        );
                    }
                }
                StatementNode::Call(call)
                    if statement_call_is_proof(program, machine, call, &proof_only) => {}
                StatementNode::Transition(transition) => {
                    if !matches!(transition.guard, TransitionGuardNode::Always)
                        || transition.continuation.is_valid()
                    {
                        push_once(
                            diagnostics,
                            &mut reported,
                            "runtime-control",
                            format!(
                                "machine `{}` canonical representation qualification cannot branch, trap, fail, abort, or carry continuation control",
                                machine.name
                            ),
                        );
                    }
                    match program.statement_table.transition_target(transition.target) {
                        TransitionTargetNode::Value(value) => {
                            returned = true;
                            if !expression_retains_lineage(program, *value, &lineage) {
                                push_once(
                                    diagnostics,
                                    &mut reported,
                                    "changed-result",
                                    format!(
                                        "machine `{}` canonical representation qualification must return the unchanged input value; transformed or reconstructed results are forbidden",
                                        machine.name
                                    ),
                                );
                            }
                        }
                        TransitionTargetNode::Named { path, arguments } => {
                            let Some(target) = program
                                .machine_states(machine)
                                .iter()
                                .find(|candidate| candidate.symbol == path.symbol)
                            else {
                                continue;
                            };
                            let target_parameters = program.state_parameters(target);
                            let arguments = program.statement_table.expression_handles(*arguments);
                            if arguments.len() != target_parameters.len() {
                                continue;
                            }
                            let mut next = Vec::new();
                            for (argument, parameter) in arguments.iter().zip(target_parameters) {
                                if expression_retains_lineage(program, *argument, &lineage) {
                                    next.push(parameter.symbol.arena_index());
                                } else if !matches!(
                                    program.expression_table.expression(*argument),
                                    ExpressionNode::Name(_)
                                ) {
                                    push_once(
                                        diagnostics,
                                        &mut reported,
                                        "runtime-transition",
                                        format!(
                                            "machine `{}` canonical representation qualification may forward aliases between proof states but cannot compute transition arguments",
                                            machine.name
                                        ),
                                    );
                                }
                            }
                            queue.push_back((target.symbol, next));
                        }
                        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {
                            push_once(
                                diagnostics,
                                &mut reported,
                                "abnormal-terminal",
                                format!(
                                    "machine `{}` canonical representation qualification must return the unchanged input on every path",
                                    machine.name
                                ),
                            );
                        }
                    }
                }
                StatementNode::Expression(expression)
                    if statement_index + 1 == statements.len() && state.return_type.is_valid() =>
                {
                    returned = true;
                    if !expression_retains_lineage(program, *expression, &lineage) {
                        push_once(
                            diagnostics,
                            &mut reported,
                            "changed-result",
                            format!(
                                "machine `{}` canonical representation qualification must return the unchanged input value; transformed or reconstructed results are forbidden",
                                machine.name
                            ),
                        );
                    }
                }
                StatementNode::AssemblyFact(_)
                | StatementNode::Assignment(_)
                | StatementNode::Call(_)
                | StatementNode::Expression(_) => {
                    push_once(
                        diagnostics,
                        &mut reported,
                        "runtime-behavior",
                        format!(
                            "machine `{}` canonical representation qualification cannot perform runtime work, mutation, assembly, trap, failure, or abort",
                            machine.name
                        ),
                    );
                }
            }
        }
    }

    if !returned {
        push_once(
            diagnostics,
            &mut reported,
            "no-return",
            format!(
                "machine `{}` canonical representation qualification has no unchanged-value return path",
                machine.name
            ),
        );
    }
    if reached_states.len() != program.machine_states(machine).len() {
        push_once(
            diagnostics,
            &mut reported,
            "unreachable-state",
            format!(
                "machine `{}` canonical representation qualification contains a state outside its checked unchanged-value lineage",
                machine.name
            ),
        );
    }
}

fn expression_retains_lineage(
    program: &TypedTrees,
    expression: ExpressionHandle,
    lineage: &HashSet<u32>,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    lineage.contains(&path.symbol.arena_index())
}

fn statement_call_is_proof(
    program: &TypedTrees,
    machine: &Machine,
    call: &omega_typed_trees::statement::TableCall,
    proof_only: &omega_typed_trees::proof_only::ProofOnlyClassification,
) -> bool {
    call.receiver.is_empty()
        && called_free_machine(program, call.target_symbol).is_some_and(|callee| {
            !std::ptr::eq(callee, machine) && proof_only.is_proof_machine(program, callee)
        })
}

fn expression_is_proof_call(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
    proof_only: &omega_typed_trees::proof_only::ProofOnlyClassification,
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    !call.receiver.is_valid()
        && called_free_machine(program, call.target_symbol).is_some_and(|callee| {
            !std::ptr::eq(callee, machine) && proof_only.is_proof_machine(program, callee)
        })
}

fn called_free_machine(program: &TypedTrees, target_symbol: SymbolHandle) -> Option<&Machine> {
    program.machines().iter().find(|candidate| {
        candidate.attached_data.is_none()
            && (candidate.symbol == target_symbol
                || program
                    .machine_states(candidate)
                    .iter()
                    .any(|state| state.symbol == target_symbol))
    })
}

fn push_once(
    diagnostics: &mut Vec<Diagnostic>,
    reported: &mut HashSet<&'static str>,
    key: &'static str,
    message: String,
) {
    if reported.insert(key) {
        diagnostics.push(Diagnostic::error(message));
    }
}
