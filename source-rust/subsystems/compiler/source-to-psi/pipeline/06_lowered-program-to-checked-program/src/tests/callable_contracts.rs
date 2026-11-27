//! Construction refinement is independently replayed against the current header.

use super::*;
use terminal_psi::{
    BoundaryMachineResult, ContractClause, CrashRouteBucket, CrashRouteGuard,
    EvidenceArgumentSubject, EvidenceCallableOwner, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceInterfaceIdentity, EvidenceTermDeclaration, MachineContract,
    OutcomeSpecificCallArgumentSubstitution, PropositionApplicationIdentity,
    PropositionDeclaration, PropositionEvidence, TerminalCallableContract,
    TerminalCallableEnvelope, TerminalCallableEvidenceLane, TerminalCallableTermination,
    TerminalMachineDescriptor, TerminalMachineDescriptorOrigin, TerminalMachineResult,
    TerminalModule,
};
use terminal_semantics::calls::callable_refinement::{
    CallableRefinementGoalKind, callable_contract_refinement_goals, callable_refinement_context,
    descriptor_refinement_goals, validate_callable_evidence_contracts_in,
    validate_descriptor_refinement_evidence,
};

fn subject_bound(result: u64, parameter: u64) -> Proposition {
    Proposition::LessOrEqual(formal(result, u32_type()), formal(parameter, u32_type()))
}

fn refinement_module() -> TerminalModule {
    let mut selected = machine(vec![u32_type()], vec![returns(slot(0, u32_type()))]);
    selected.requires = vec![at_least(1, 3)];
    selected.ensures = vec![subject_bound(2, 1)];
    let (elaboration, verdict) = run(vec![selected]);
    for (_, result) in verdict.machines.get(id(1)).unwrap().verdicts.iter() {
        assert!(
            result.is_ok(),
            "selected body must establish its own contract: {result:?}"
        );
    }
    let mut module = elaboration.module;
    // These implication controls have no source declaration. Publication uses
    // the separate source-owned fixture below.
    for machine in &mut module.machines {
        machine.declaration = None;
    }
    module.execution.callables.machine_envelopes.clear();
    let selected = &module.machines[0];
    assert_eq!(selected.parameters[0].id, id(1));
    assert_eq!(selected.result.scalar().unwrap().id, id(2));
    let parameter = terminal_psi::ValueDeclaration {
        id: id(10),
        ..selected.parameters[0]
    };
    let result = terminal_psi::ValueDeclaration {
        id: id(11),
        ..selected.result.scalar().unwrap()
    };
    let parameter_order = selected.parameter_order.clone();
    let mut identities = module.identities.clone().into_builder();
    let identity = identities.intern("callable-refinement-test-contract");
    let application = identities.intern("callable-refinement-test-selection");
    module.identities = identities.build();
    let mut proof = MachineContract::empty(id(10));
    proof.parameters = vec![parameter];
    proof.parameter_order = parameter_order.clone();
    proof.result = TerminalMachineResult::Scalar(result);
    proof.requires = vec![at_least(10, 5)];
    proof.ensures = vec![ContractClause {
        obligation: id(90),
        proposition: subject_bound(11, 10),
    }];
    module.execution.callables.contracts.push(TerminalCallableContract {
        id: id(1),
        identity,
        scalar_parameters: vec![parameter],
        parameter_order,
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Scalar(result.value_type()),
        proof_result: TerminalMachineResult::Scalar(result),
        entry_claim_paths: Vec::new(),
        result_claim_paths: Vec::new(),
        result_reference_regions: Vec::new(),
        proof,
        evidence_contract: Vec::new(),
        envelope: TerminalCallableEnvelope {
            service_reach: Vec::new(),
            invocations: Vec::new(),
            suspends: false,
            blocks: false,
            termination: TerminalCallableTermination::NotGuaranteed,
        },
    });
    let goals = callable_contract_refinement_goals(&module, id(1), id(1)).unwrap();
    let mut next_obligation = 100;
    let obligations = goals
        .iter()
        .map(|_| {
            let obligation = id(next_obligation);
            next_obligation += 1;
            obligation
        })
        .collect();
    module
        .execution.callables
        .descriptors
        .push(TerminalMachineDescriptor {
            id: id(1),
            contract: id(1),
            machine: id(1),
            origin: TerminalMachineDescriptorOrigin {
                application,
                conformance: None,
                requirement: None,
            },
            refinement_obligations: obligations,
            refinement_evidence: Vec::new(),
        });
    module
}

/// The finalizer receives a real source-issued nominal publication. Logical
/// implication controls above deliberately use source-free abstract bodies.
fn declared_refinement_module() -> TerminalModule {
    use crate::source_inputs::SourceMap;
    use crate::source_state::TypedTrees;
    use crate::source_state::callable_values::RuntimeCallableContract;
    use crate::source_state::expression::NamePath;
    use crate::source_state::machine::Machine;
    use crate::source_state::name::Identifier;
    use crate::source_state::signature::{StateParameter, StateSignature};
    use crate::source_state::state::State;
    use crate::source_state::statement::{StatementNode, TableTransition, TransitionTargetNode};
    use crate::source_state::types::TypeReferenceNode;
    use crate::source_symbols::{SymbolTableBuilder, builtin_type_symbols};
    use source_admission::SourceOrigin;
    use source_text::{SourceSpan, Span};
    use std::path::PathBuf;
    use std::sync::Arc;
    use symbols::{BuiltinTypeAtom, SymbolKind, SymbolNameRef};

    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]);
    let mut sources = SourceMap::default();
    let source = sources
        .add_with_metadata(
            PathBuf::from("package/selected.omg"),
            "Selected entry amount Callback".to_owned(),
            PathBuf::from("package"),
            package,
            SourceOrigin::User,
        )
        .source_id;
    let name = |start, end| SymbolNameRef::Source(SourceSpan::new(source, Span::new(start, end)));
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = builder.insert_children(
        root,
        builtin_type_symbols().into_iter().chain([
            (SymbolKind::Machine, name(0, 8)),
            (SymbolKind::CallableContract, name(22, 30)),
        ]),
    );
    let u32_atom = BuiltinTypeAtom::ALL
        .into_iter()
        .find(|atom| {
            atom.fixed_integer()
                .is_some_and(|carrier| !carrier.is_signed() && carrier.width() == 32)
        })
        .expect("the builtin roster contains unsigned 32-bit integers");
    let u32_symbol = declarations.handle_at(u32_atom.ordinal() as u32);
    let selected = declarations.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
    let callback = declarations.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 1);
    let entry = builder
        .insert_children(selected, [(SymbolKind::State, name(9, 14))])
        .handle_at(0);
    let amount = builder
        .insert_children(entry, [(SymbolKind::Parameter, name(15, 21))])
        .handle_at(0);
    let callback_amount = builder
        .insert_children(callback, [(SymbolKind::Parameter, name(15, 21))])
        .handle_at(0);
    let mut typed = TypedTrees {
        symbols: builder.finish(),
        ..Default::default()
    };
    let no_services = typed.service_reach_rows.intern(Vec::new());
    let carrier = typed.type_reference_table.insert(TypeReferenceNode::Named {
        symbol: u32_symbol,
        name: Identifier::generated("u32"),
    });
    let parameters = typed.state_parameters.insert_many([StateParameter {
        symbol: amount,
        name: Identifier::generated("amount"),
        type_reference: carrier,
        ..Default::default()
    }]);
    let result = typed.expression_table.insert_name_path(&NamePath::resolved(
        vec![Identifier::generated("amount")],
        amount,
        amount,
    ));
    let target = typed
        .statement_table
        .insert_transition_target(TransitionTargetNode::Value(result));
    let statements = typed
        .statement_table
        .insert_statements([StatementNode::Transition(TableTransition {
            target,
            ..Default::default()
        })]);
    let states = typed.machine_states.insert_many([State {
        symbol: entry,
        name: Identifier::generated("entry"),
        parameters,
        return_type: carrier,
        statement_nodes: statements,
        ..Default::default()
    }]);
    typed.push_machine(Machine {
        symbol: selected,
        name: Identifier::generated("Selected"),
        states,
        service_reach_row: no_services,
        declares_no_crash_route: true,
        ..Default::default()
    });
    let parameters = typed.state_parameters.insert_many([StateParameter {
        symbol: callback_amount,
        name: Identifier::generated("amount"),
        type_reference: carrier,
        ..Default::default()
    }]);
    let required = typed
        .runtime_callable_contracts
        .insert(RuntimeCallableContract {
            signature: StateSignature {
                symbol: callback,
                name: Identifier::generated("Callback"),
                parameters,
                return_type: carrier,
                service_reach_row: no_services,
                declares_no_crash_route: true,
                ..Default::default()
            },
            lexical_owner: root,
            resolved_source: None,
        });
    let required_identity = typed
        .runtime_callable_contract_identity(required, &[])
        .unwrap();
    let elaborated = crate::lowering::elaborate(
        &typed,
        crate::lowering::Compilation {
            declaration_metadata: &[],
            supplies: &[],

            entries: &[selected],
            subjects: &[package],
            purpose: source_provenance::DependencyScope::Product,
            selected: &[],
            calling: None,
            providers: None,
            bindings: &[],
        },
    );
    let verdict = crate::judge(elaborated).unwrap();
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("the source-owned selected body must be accepted: {verdict:?}");
    };
    assert!(closure.elaboration.undeclared.is_empty());
    let mut module = closure.elaboration.module;
    assert!(module.machines[0].declaration.is_some());
    assert!(module.execution.callables.machine_envelope(id(1)).is_some());
    let selected = &module.machines[0];
    let mut proof = selected.contract.clone();
    proof.id = id(2);
    let mut identities = module.identities.clone().into_builder();
    let identity = identities.intern(&required_identity);
    let application = identities.intern("source-owned-selected-application");
    module.identities = identities.build();
    let envelope = module.execution.callables.machine_envelope(id(1)).unwrap().clone();
    module.execution.callables.contracts.push(TerminalCallableContract {
        id: id(1),
        identity,
        scalar_parameters: selected.parameters.clone(),
        parameter_order: selected.parameter_order.clone(),
        structural_parameters: selected.structural_parameters.clone(),
        result: BoundaryMachineResult::Scalar(selected.result.scalar().unwrap().value_type()),
        proof_result: selected.result.clone(),
        proof,
        entry_claim_paths: Vec::new(),
        result_claim_paths: Vec::new(),
        result_reference_regions: Vec::new(),
        evidence_contract: Vec::new(),
        envelope,
    });
    let questions = callable_contract_refinement_goals(&module, id(1), id(1)).unwrap();
    module
        .execution.callables
        .descriptors
        .push(TerminalMachineDescriptor {
            id: id(1),
            contract: id(1),
            machine: id(1),
            origin: TerminalMachineDescriptorOrigin {
                application,
                conformance: None,
                requirement: None,
            },
            refinement_obligations: questions
                .iter()
                .enumerate()
                .map(|(position, _)| id(100 + position as u64))
                .collect(),
            refinement_evidence: Vec::new(),
        });
    module
}

fn derive_refinements(
    module: &TerminalModule,
) -> Result<Vec<terminal_psi::CertificateEnvelope>, String> {
    let context = callable_refinement_context(module, id(1))?;
    let required = &module.execution.callables.contracts[0];
    let mut declarations = required
        .scalar_parameters
        .iter()
        .chain(&required.proof.erased_scalar_formals)
        .chain(required.proof_result.scalar_ref())
        .map(|value| (value.id, value.scalar_type))
        .collect::<Vec<_>>();
    declarations.sort_by_key(|(id, _)| *id);
    descriptor_refinement_goals(module, id(1))?
        .into_iter()
        .map(|(id, goal)| {
            crate::derive_contract_obligation(
                terminal_check_tables::RefusalSite::Program,
                &context,
                id,
                &goal.proposition,
                &declarations,
                &mut Default::default(),
            )
            .map_err(|refused| refused.detail)
        })
        .collect()
}

#[test]
fn a_malformed_contract_formal_is_a_checker_defect() {
    let refusal = crate::derive_contract_obligation(
        terminal_check_tables::RefusalSite::Program,
        &semantic_vocabulary::PropositionContext::default(),
        id(1),
        &Proposition::Truth,
        &[(id(1), u32_type()), (id(1), u32_type())],
        &mut Default::default(),
    )
    .unwrap_err();
    assert_eq!(
        refusal.reason,
        terminal_check_tables::RefusalReason::ForeignIdentity
    );
}

#[test]
fn callable_refinement_proves_input_weakening_and_remaps_result_formals() {
    let mut module = refinement_module();
    let goals = descriptor_refinement_goals(&module, id(1)).unwrap();
    assert_eq!(
        goals.iter().map(|(_, goal)| &goal.kind).collect::<Vec<_>>(),
        [
            &CallableRefinementGoalKind::Input,
            &CallableRefinementGoalKind::NormalOutput
        ]
    );
    module.execution.callables.descriptors[0].refinement_evidence = derive_refinements(&module).unwrap();
    assert_eq!(
        validate_descriptor_refinement_evidence(&module)
            .unwrap()
            .len(),
        goals.len()
    );
    let mut missing = module.clone();
    missing.execution.callables.descriptors[0].refinement_evidence.pop();
    assert!(validate_descriptor_refinement_evidence(&missing).is_err());
    let mut aliased = module.clone();
    aliased.execution.callables.descriptors[0].refinement_obligations[1] = id(100);
    assert!(validate_descriptor_refinement_evidence(&aliased).is_err());
    let mut changed = module;
    changed.execution.callables.contracts[0].proof.requires = vec![at_least(10, 2)];
    assert!(derive_refinements(&changed).is_err());
    assert!(validate_descriptor_refinement_evidence(&changed).is_err());
}

#[test]
fn checker_finalizes_descriptors_and_refuses_mutated_retained_certificates() {
    let mut module = declared_refinement_module();
    crate::finalize_callable_refinements(&mut module, &mut Default::default()).unwrap();
    assert!(
        !module.execution.callables.descriptors[0]
            .refinement_evidence
            .is_empty()
    );
    let retained = module.execution.callables.descriptors[0].refinement_evidence.clone();
    crate::finalize_callable_refinements(&mut module, &mut Default::default()).unwrap();
    assert_eq!(
        module.execution.callables.descriptors[0].refinement_evidence,
        retained
    );
    module.execution.callables.descriptors[0].refinement_evidence[0]
        .proof
        .conclusion = Proposition::Falsehood;
    assert!(
        crate::finalize_callable_refinements(&mut module, &mut Default::default(),)
            .is_err()
    );
}

#[test]
fn checker_refuses_operational_strengthening_before_publication() {
    let mut module = declared_refinement_module();
    crate::finalize_callable_refinements(&mut module, &mut Default::default()).unwrap();
    module.execution.callables.machine_envelopes[0]
        .as_mut()
        .unwrap()
        .envelope
        .blocks = true;
    module.machines[0]
        .declaration
        .as_mut()
        .unwrap()
        .published_may_block = true;
    let refusal =
        crate::finalize_callable_refinements(&mut module, &mut Default::default())
            .unwrap_err();
    assert_eq!(
        refusal.detail,
        "selected machine may block outside the callable contract"
    );

    let mut abstract_form = refinement_module();
    abstract_form.execution.callables.descriptors.clear();
    assert!(abstract_form.machines[0].declaration.is_none());
    crate::finalize_callable_refinements(&mut abstract_form, &mut Default::default())
        .unwrap();
}

#[test]
fn callable_refinement_refuses_weaker_outputs_and_unpermitted_crashes() {
    let mut module = refinement_module();
    module.execution.callables.descriptors[0].refinement_evidence = derive_refinements(&module).unwrap();
    let mut weaker = module.clone();
    weaker.machines[0].contract.ensures[0].proposition = at_most(2, u32::MAX.into());
    assert!(derive_refinements(&weaker).is_err());
    assert!(validate_descriptor_refinement_evidence(&weaker).is_err());
    module.machines[0]
        .contract
        .crash_routes
        .push(CrashRouteBucket {
            cause: semantic_vocabulary::CrashCause::Trap,
            alternatives: vec![CrashRouteGuard::Truth],
        });
    let goals = callable_contract_refinement_goals(&module, id(1), id(1)).unwrap();
    assert!(matches!(
        goals.last().unwrap().kind,
        CallableRefinementGoalKind::Crash { .. }
    ));
    assert!(validate_descriptor_refinement_evidence(&module).is_err());
    // Allocate the new question too: refusal must be logical, not only a stale count.
    module.execution.callables.descriptors[0]
        .refinement_obligations
        .push(id(102));
    assert!(derive_refinements(&module).is_err());
}

fn install_roster(module: &mut TerminalModule) {
    let mut identities = module.identities.clone().into_builder();
    let interface = EvidenceInterfaceIdentity {
        trait_identity: identities.intern("proof-witness"),
        arguments: vec!["u32".into()],
        requirements: Vec::new(),
    };
    module.identities = identities.build();
    let declaration = PropositionDeclaration {
        id: id(1),
        name: "Bound".into(),
        binders: Vec::new(),
        parameter_types: vec!["u32".into()],
        evidence: PropositionEvidence::Witness {
            evidence_type: "proof-witness[u32]".into(),
        },
    };
    module.proofs.proposition_declarations.push(declaration.clone());
    for (position, kind) in [
        EvidenceContractLaneKind::Requires,
        EvidenceContractLaneKind::Ensures,
    ]
    .into_iter()
    .enumerate()
    {
        let subjects = match kind {
            EvidenceContractLaneKind::Requires => vec![EvidenceArgumentSubject::Parameter(
                OutcomeSpecificCallArgumentSubstitution {
                    argument_position: 0,
                    parameter_position: 0,
                    structural_path: None,
                },
            )],
            EvidenceContractLaneKind::Ensures => vec![EvidenceArgumentSubject::Result {
                argument_position: 0,
            }],
        };
        let application = PropositionApplicationIdentity {
            id: id(position as u64 + 10),
            declaration: declaration.id,
            binder_arguments: Vec::new(),
            arguments: vec![format!("formal-{position}")],
            evidence_interface: Some(interface.clone()),
        };
        let term = EvidenceTermDeclaration {
            id: id(position as u64 + 1),
            proposition: application.id,
            interface: interface.clone(),
        };
        module.execution.callables.contracts[0]
            .evidence_contract
            .push(TerminalCallableEvidenceLane {
                kind,
                position: 0,
                declaration: declaration.clone(),
                application: application.clone(),
                term: term.clone(),
                argument_subjects: subjects.clone(),
            });
        module.proofs.proposition_applications.push(application);
        module.proofs.evidence_terms.push(term.clone());
        module.proofs.evidence_contract_lanes.push(EvidenceContractLane {
            owner: EvidenceCallableOwner::Runtime(id(1)),
            kind,
            position: 0,
            term: term.id,
            output_field: (kind == EvidenceContractLaneKind::Ensures).then(|| "$0".into()),
            argument_subjects: subjects,
        });
    }
}

fn roster_valid(module: &TerminalModule) -> bool {
    validate_callable_evidence_contracts_in(
        &module.execution.callables,
        &module.proofs.proposition_declarations,
        &module.proofs.proposition_applications,
        &module.proofs.evidence_terms,
        &module.proofs.evidence_contract_lanes,
    )
    .is_ok()
}

#[test]
fn runtime_evidence_roster_rejoins_full_meaning_and_exact_subjects() {
    let mut module = refinement_module();
    install_roster(&mut module);
    assert!(roster_valid(&module));
    let mut missing = module.clone();
    missing.proofs.evidence_contract_lanes.pop();
    assert!(!roster_valid(&missing));
    let mut duplicate = module.clone();
    duplicate
        .proofs.evidence_contract_lanes
        .push(duplicate.proofs.evidence_contract_lanes[0].clone());
    assert!(!roster_valid(&duplicate));
    let mut subject = module.clone();
    subject.proofs.evidence_contract_lanes[0].argument_subjects[0] =
        EvidenceArgumentSubject::Parameter(OutcomeSpecificCallArgumentSubstitution {
            argument_position: 0,
            parameter_position: 1,
            structural_path: None,
        });
    assert!(!roster_valid(&subject));
    let mut interface = module.clone();
    interface.proofs.evidence_terms[0].interface.arguments[0] = "u64".into();
    assert!(!roster_valid(&interface));
    let mut application = module;
    application.proofs.proposition_applications[0].arguments[0] = "another-formal".into();
    assert!(!roster_valid(&application));
}

/// A block acknowledgement keeps the ordinary lexical callee. A terminal
/// call can select a nested machine, but a local sharing its name wins in
/// both terminal and ordinary call position.
#[test]
fn acknowledged_terminal_paths_keep_nominal_targets_and_scalar_calls_stay_invalid() {
    let source = |local: &str, body: &str| {
        format!(
            r#"
machine build_entry(value: u32) -> u32 {{
    let {local}: u64 = 1;
    {body}
    machine root(value: u32) -> u32 {{ value }}
}}
"#
        )
    };
    let positive = source("other", "transition { _ -> block root(value) }");
    let verdict = super::build_purpose::judge_build(&positive).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("acknowledged nested transfer must be judged: {verdict:?}");
    };
    accepted.publish_invocation().unwrap();

    for body in [
        "transition { _ -> block root(value) }",
        "let result: u32 = block root(value); result",
    ] {
        let negative = source("root", body);
        assert_scalar_callee_refused(&negative);
    }
}

fn assert_scalar_callee_refused(negative: &str) {
    let mut sources = crate::source_inputs::SourceMap::default();
    let source_id = sources
        .add_checked_instance(
            "package/build.omg".into(),
            negative,
            "package".into(),
            semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]),
            source_admission::SourceOrigin::User,
            source_admission::SourceResolutionStratum::Base,
            source_provenance::DependencyScope::Build,
        )
        .source_id;
    let tokens = crate::frontend_fixture::Lexer::new(negative)
        .tokenize()
        .unwrap();
    let syntax =
        crate::frontend_fixture::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let mut request = crate::resolution::ResolutionRequest::new(syntax);
    request.sources = Some(std::sync::Arc::new(sources));
    let resolved = crate::resolution::resolve(request).unwrap();
    let error = crate::typing::lower_symbol_resolved_trees(&resolved).unwrap_err();
    assert_eq!(
        error.message,
        "the evaluated call operand is not a runtime callable"
    );
}

/// Inference, opaque contracts and boundary headers must discharge the same
/// marker obligation. A provisional private summary cannot erase it.
#[test]
fn acknowledgements_use_completed_named_selected_and_boundary_envelopes() {
    for (label, source) in [
        (
            "inferred named",
            r#"
machine wait(value: u32) -> u32 blocks crashes none { value }
machine relay(value: u32) -> u32 crashes none { block wait(value) }
machine build_entry(value: u32) -> u32 crashes none { ACK relay(value) }
"#,
        ),
        (
            "opaque callable",
            r#"
data BlockingOperation = machine(value: u32) -> u32 reaches none blocks crashes none;
machine build_entry(operation: BlockingOperation, value: u32) -> u32 crashes none {
    ACK operation(value)
}
"#,
        ),
        (
            "boundary",
            r#"
boundary trait Probe {
    machine read(value: u32) -> u32 reaches none blocks crashes none;
}
machine build_entry(value: u32) -> u32 crashes none { ACK Probe::read(value) }
"#,
        ),
    ] {
        let valid = source.replace("ACK", "block");
        let verdict = super::build_purpose::judge_build(&valid).unwrap();
        let RouteVerdict::Accepted(accepted) = verdict else {
            panic!("{label} with its actual acknowledgement must be judged: {verdict:?}");
        };
        let artifact = accepted.publish_invocation().unwrap();
        assert!(
            artifact
                .artifact
                .view()
                .module()
                .execution.callables
                .machine_envelopes
                .iter()
                .flatten()
                .any(|row| row.envelope.blocks),
            "{label} must retain blocking"
        );
        let mut missing_audit = accepted.elaboration.clone();
        let mut changed = 0;
        for machine in &mut missing_audit.module.machines {
            for formation in &mut machine.call_formations {
                if formation.acknowledgement.acknowledges_block {
                    changed += 1;
                    formation.acknowledgement.acknowledges_block = false;
                }
            }
        }
        assert!(
            changed > 0,
            "{label} must have its actual call acknowledgement"
        );
        let refusal = crate::operational::judge(&missing_audit).unwrap_err();
        assert!(
            matches!(refusal.reason, RefusalReason::Violation),
            "{refusal:?}"
        );
        assert_eq!(
            refusal.detail, "call may block and requires a block acknowledgement",
            "{label}"
        );
        let missing = source.replace("ACK", "");
        let verdict = super::build_purpose::judge_build(&missing).unwrap();
        let RouteVerdict::Rejected {
            formations,
            violations,
            unproved,
            unestablished,
            conflicts,
            exceeded,
        } = verdict
        else {
            panic!("{label} must reject the missing acknowledgement: {verdict:?}");
        };
        assert!(
            formations.is_empty()
                && unproved.is_empty()
                && unestablished.is_empty()
                && conflicts.is_empty()
                && exceeded.is_empty()
        );
        assert_eq!(violations.len(), 1, "{label}: {violations:?}");
        assert_eq!(
            violations[0].message(),
            "call may block and requires a block acknowledgement in machine `build_entry`"
        );
    }
}

#[test]
fn redundant_suspend_markers_still_require_a_complete_source_position() {
    let legal = r#"
machine pure(value: u32) -> u32 crashes none { value }
machine build_entry(value: u32) -> u32 crashes none {
    let result: u32 = suspend pure(value);
    result
}
"#;
    assert!(matches!(
        super::build_purpose::judge_build(legal).unwrap(),
        RouteVerdict::Accepted(_)
    ));
    let nested = r#"
machine pure(value: u32) -> u32 crashes none { value }
machine build_entry(value: u32) -> u32 crashes none {
    pure(suspend pure(value))
}
"#;
    let verdict = super::build_purpose::judge_build(nested).unwrap();
    let RouteVerdict::Rejected {
        formations,
        violations,
        unproved,
        unestablished,
        conflicts,
        exceeded,
    } = verdict
    else {
        panic!("a nested suspend marker must reject source formation: {verdict:?}");
    };
    assert!(
        formations.is_empty()
            && unproved.is_empty()
            && unestablished.is_empty()
            && conflicts.is_empty()
            && exceeded.is_empty()
    );
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert_eq!(
        violations[0].message(),
        "a suspend-marked call must be a complete statement, simple let initializer, transition subject or terminal expression"
    );
}
