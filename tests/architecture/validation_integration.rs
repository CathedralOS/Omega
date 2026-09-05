use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use validation::{validate_behavior_plan, validate_program, validate_static_machine_selections};

fn typed_program_from_source(source: &str) -> typed_trees::TypedTrees {
    let source = format!("data Main {{}} machine Main::run(&mut self) {{}} {source}");
    let tokens = Lexer::new(&source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("resolve should succeed");
    lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed")
}

#[test]
fn closed_conformance_validates_its_exact_inline_and_reference_rows() {
    let typed = typed_program_from_source(
        r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
            machine Self::rank_value(&self) -> u32;
        }

        data Card {}
        machine Card::stable_rank_value(&self) -> u32 {
            transition { _ -> (0) }
        }

        PowerOrder: Card satisfies Ranked {
            machine before(&self, other: &Card) -> bool {
                transition { _ -> (false) }
            }
            Ranked::rank_value = Card::stable_rank_value;
        }
        "#,
    );

    validate_program(&typed).expect("the closed row map should validate without ambient lookup");
}

#[test]
fn explicit_routed_progress_profile_validates() {
    let typed = typed_program_from_source(
        r#"
        data SchedulerHandle {}
        domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;

        boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
        }
        "#,
    );

    validate_program(&typed).expect("an opaque boundary-routed progress profile should validate");
}

#[test]
fn provider_progress_schema_retains_the_exact_authorized_establishment_route() {
    let typed = typed_program_from_source(
        r#"
        boundary trait SchedulerRuntime {
            machine wait(&self)
            requires self in WeakFair
            terminates;
        }
        domain SchedulerRuntime::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerRuntime) -> SchedulerRuntime in WeakFair;
        }
        "#,
    );
    validate_program(&typed).expect("progress contracts should validate");

    let runtime = typed
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "SchedulerRuntime")
        .expect("runtime boundary trait");
    let admission = typed
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "SchedulerAdmission")
        .expect("admission boundary trait");
    let grant = typed
        .trait_machine_signatures(admission)
        .iter()
        .find(|requirement| requirement.name.as_str() == "grant")
        .expect("grant requirement");
    let grant_identity = typed
        .normalized_trait_requirement_overload_identity(admission, grant)
        .identity();
    let schema = effects::provider_plan::ServiceSchema::from_typed(&typed, runtime)
        .expect("runtime provider schema");
    let premise = &schema.methods[0].termination_premises[0];

    assert_eq!(premise.profile, "SchedulerRuntime::WeakFair");
    assert_eq!(premise.establishment_routes.len(), 1);
    assert_eq!(
        premise.establishment_routes[0].kind,
        effects::provider_plan::ServiceProgressEstablishmentRouteKind::BoundaryRequirement
    );
    assert_eq!(
        premise.establishment_routes[0].requirement_identity,
        grant_identity
    );
}

#[test]
fn progress_profile_rejects_predicates_missing_routes_and_checked_routes() {
    let cases = [
        (
            r#"
            data SchedulerHandle {}
            domain SchedulerHandle::WeakFair
            satisfies ProgressProfile
            requires true
            established by SchedulerAdmission::grant;
            boundary trait SchedulerAdmission {
                machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
            }
            "#,
            "must be predicate-free",
        ),
        (
            r#"
            data SchedulerHandle {}
            domain SchedulerHandle::WeakFair
            satisfies ProgressProfile;
            "#,
            "requires at least one exact `established by` boundary requirement",
        ),
        (
            r#"
            data SchedulerHandle {}
            domain SchedulerHandle::WeakFair
            satisfies ProgressProfile
            established by SchedulerAdmission::grant;
            trait SchedulerAdmission {
                machine grant() -> SchedulerHandle in WeakFair;
            }
            "#,
            "may be established only by exact boundary trait requirements",
        ),
    ];

    for (source, expected) in cases {
        let typed = typed_program_from_source(source);
        let diagnostics = validate_program(&typed).expect_err("profile shape must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "expected `{expected}`, got: {diagnostics:?}"
        );
    }
}

#[test]
fn progress_profile_termination_premise_normalizes_subject_and_profile() {
    let typed = typed_program_from_source(
        r#"
        data SchedulerHandle {}
        domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
        }
        boundary trait SchedulerRuntime {
            machine wait(scheduler: SchedulerHandle)
            requires scheduler in WeakFair
            terminates;
        }
        "#,
    );

    validate_program(&typed).expect("a bodyless requirement may publish the normalized schema");
    let profile = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "SchedulerHandle::WeakFair")
        .expect("profile domain");
    let runtime = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "SchedulerRuntime")
        .expect("runtime trait");
    let wait = typed
        .trait_machine_signatures(runtime)
        .iter()
        .find(|signature| signature.name.as_str() == "wait")
        .expect("wait requirement");
    let [scheduler] = typed.state_signature_parameters(wait) else {
        panic!("wait should have one parameter")
    };
    let language_semantics::TerminationGuarantee::Terminates { premises } =
        &wait.termination_guarantee
    else {
        panic!("wait should publish termination")
    };
    let [premise] = premises.as_slice() else {
        panic!("wait should publish one premise")
    };
    assert_eq!(premise.profile, profile.semantic_id);
    assert_eq!(premise.subject.root, scheduler.symbol);
    assert!(premise.subject.projections.is_empty());
}

#[test]
fn checked_machine_may_publish_a_conservative_progress_schema_without_using_it() {
    let typed = typed_program_from_source(
        r#"
        data SchedulerHandle {}
        domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
        }
        data Worker {}
        machine Worker::wait(&self, scheduler: SchedulerHandle) -> u64
        requires scheduler in WeakFair
        terminates
        {
            0
        }
        "#,
    );

    validate_program(&typed)
        .expect("an unused authored premise may conservatively strengthen the public contract");
}

#[test]
fn checked_progress_call_instantiates_and_covers_the_exact_public_subject() {
    let typed = typed_program_from_source(
        r#"
        data SchedulerHandle {}
        domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair
            ensures result in SchedulerHandle::WeakFair
            terminates;
        }
        boundary trait SchedulerRuntime {
            machine wait(scheduler: SchedulerHandle)
            requires scheduler in WeakFair
            terminates;
        }
        machine process(
            runtime: &mut SchedulerRuntime,
            scheduler: SchedulerHandle
        )
        requires scheduler in WeakFair
        terminates
        {
            runtime.wait(scheduler);
        }
        "#,
    );

    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("the selected call premise should match the authored public schema");
    let process = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "process")
        .expect("process machine");
    let plan = checked
        .facts
        .termination
        .for_machine(process.symbol)
        .expect("checked termination plan");
    let language_semantics::TerminationGuarantee::Terminates { premises } = &plan.checked_summary
    else {
        panic!("process should retain a checked termination summary")
    };
    assert_eq!(premises.len(), 1);
}

#[test]
fn checked_progress_retains_provider_receiver_as_build_bound_demand() {
    let typed = typed_program_from_source(
        r#"
        boundary trait SchedulerRuntime {
            machine wait(&self)
            requires self in WeakFair
            terminates;
        }
        domain SchedulerRuntime::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerRuntime) -> SchedulerRuntime in WeakFair;
        }
        machine helper(runtime: SchedulerRuntime in WeakFair)
        {
            runtime.wait();
        }
        machine process(runtime: SchedulerRuntime in WeakFair)
        terminates
        {
            helper(runtime);
        }
        "#,
    );

    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("provider-receiver progress must remain a composition demand");
    let process = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "process")
        .expect("process machine");
    let [demand] = checked
        .facts
        .termination
        .build_bound_for_machine(process.symbol)
    else {
        panic!("process should retain one exact build-bound demand")
    };
    assert_eq!(demand.provider_service_identity, "SchedulerRuntime");
    assert_eq!(demand.profile_identity, "SchedulerRuntime::WeakFair");
    assert!(
        demand
            .requirement_identity
            .contains("SchedulerRuntime::wait")
    );
    assert!(demand.subject_projections.is_empty());
    assert!(
        checked
            .symbols
            .display_path(demand.origin.machine, "::")
            .contains("helper")
    );
}

#[test]
fn admitted_provider_receiver_receipt_removes_build_bound_demand() {
    let typed = typed_program_from_source(
        r#"
        boundary trait SchedulerRuntime {
            machine wait(&self)
            requires self in WeakFair
            terminates;
        }
        domain SchedulerRuntime::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerRuntime) -> SchedulerRuntime in WeakFair
            ensures result in SchedulerRuntime::WeakFair
            terminates;
        }
        machine process(
            admission: &mut SchedulerAdmission,
            runtime: SchedulerRuntime
        )
        terminates
        {
            let granted: SchedulerRuntime in WeakFair = admission.grant(runtime);
            granted.wait();
        }
        "#,
    );

    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("the exact local receipt should discharge provider progress");
    let process = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "process")
        .expect("process machine");
    assert!(
        checked
            .facts
            .termination
            .build_bound_for_machine(process.symbol)
            .is_empty()
    );
}

#[test]
fn checked_progress_call_rejects_an_unpublished_subject_dependency() {
    let typed = typed_program_from_source(
        r#"
        data SchedulerHandle {}
        domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair
            ensures result in SchedulerHandle::WeakFair
            terminates;
        }
        boundary trait SchedulerRuntime {
            machine wait(scheduler: SchedulerHandle)
            requires scheduler in WeakFair
            terminates;
        }
        machine process(
            runtime: &mut SchedulerRuntime,
            scheduler: SchedulerHandle in WeakFair
        )
        terminates
        {
            runtime.wait(scheduler);
        }
        "#,
    );

    let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect_err("a parameter qualification must not silently become a public progress schema");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("published termination contract does not cover that exact subject")
    }));
}

#[test]
fn private_progress_dependencies_substitute_through_the_exact_helper_call() {
    let typed = typed_program_from_source(
        r#"
        data SchedulerHandle {}
        domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair
            terminates;
        }
        boundary trait SchedulerRuntime {
            machine wait(scheduler: SchedulerHandle)
            requires scheduler in WeakFair
            terminates;
        }
        machine helper(
            runtime: &mut SchedulerRuntime,
            scheduler: SchedulerHandle
        )
        requires scheduler in WeakFair
        {
            runtime.wait(scheduler);
        }
        machine process(
            runtime: &mut SchedulerRuntime,
            scheduler: SchedulerHandle
        )
        requires scheduler in WeakFair
        terminates
        {
            helper(runtime, scheduler);
        }
        "#,
    );

    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("a private helper should forward its exact derived premise by position");
}

#[test]
fn measured_entry_back_edge_retains_its_checked_termination_summary() {
    let typed = typed_program_from_source(
        r#"
        machine countdown(remaining: u64) -> u64
        terminates by remaining;
        {
            transition remaining > 0 {
                true -> countdown(remaining - 1)
                false -> 0
            }
        }
        machine run(remaining: u64) -> u64
        terminates
        {
            countdown(remaining)
        }
        "#,
    );

    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("the measured entry back-edge should validate");
    let countdown = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "countdown")
        .expect("countdown machine");
    let plan = checked
        .facts
        .termination
        .for_machine(countdown.symbol)
        .expect("checked termination plan");
    assert_eq!(
        plan.checked_summary,
        language_semantics::TerminationGuarantee::Terminates {
            premises: Vec::new(),
        }
    );
    assert_eq!(
        plan.implementation_witness
            .as_ref()
            .expect("measured recursion witness")
            .view_path,
        "Nat::Descending"
    );
    let run = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .expect("run machine");
    assert_eq!(
        checked
            .facts
            .termination
            .for_machine(run.symbol)
            .expect("wrapper termination plan")
            .checked_summary,
        language_semantics::TerminationGuarantee::Terminates {
            premises: Vec::new(),
        }
    );
}

#[test]
fn measured_entry_back_edge_retains_exact_progress_subject_lineage() {
    let typed = typed_program_from_source(
        r#"
        data SchedulerHandle {}
        domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
        }
        boundary trait SchedulerRuntime {
            machine wait(scheduler: SchedulerHandle)
            requires scheduler in WeakFair
            terminates;
        }
        machine countdown(
            runtime: &mut SchedulerRuntime,
            scheduler: SchedulerHandle,
            remaining: u64
        )
        requires scheduler in WeakFair
        terminates by remaining;
        {
            runtime.wait(scheduler);
            transition remaining > 0 {
                true -> countdown(runtime, scheduler, remaining - 1)
                false -> 0
            }
        }
        "#,
    );

    let countdown = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "countdown")
        .expect("countdown machine");
    let scheduler_symbol = typed
        .machine_states(countdown)
        .first()
        .and_then(|state| {
            typed
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.name.as_str() == "scheduler")
        })
        .expect("scheduler parameter")
        .symbol;
    let weak_fair = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "SchedulerHandle::WeakFair")
        .expect("weak-fair progress profile")
        .semantic_id;

    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("the measured entry back-edge should preserve its exact progress subject");
    let countdown = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "countdown")
        .expect("countdown machine");
    let plan = checked
        .facts
        .termination
        .for_machine(countdown.symbol)
        .expect("checked termination plan");
    let language_semantics::TerminationGuarantee::Terminates { premises } = &plan.checked_summary
    else {
        panic!("countdown should retain a checked termination summary")
    };
    let [premise] = premises.as_slice() else {
        panic!("countdown should retain exactly one progress premise")
    };
    assert_eq!(premise.profile, weak_fair);
    assert_eq!(premise.subject.root, scheduler_symbol);
    assert!(premise.subject.projections.is_empty());
}

#[test]
fn admitted_local_progress_receipt_discharges_the_selected_call_premise() {
    let typed = typed_program_from_source(
        r#"
        data SchedulerHandle {}
        domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair
            ensures result in SchedulerHandle::WeakFair
            terminates;
        }
        boundary trait SchedulerRuntime {
            machine wait(scheduler: SchedulerHandle)
            requires scheduler in WeakFair
            terminates;
        }
        machine process(
            admission: &mut SchedulerAdmission,
            runtime: &mut SchedulerRuntime,
            scheduler: SchedulerHandle
        )
        terminates
        {
            let granted: SchedulerHandle in WeakFair = admission.grant(scheduler);
            runtime.wait(granted);
        }
        "#,
    );

    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("the exact locally admitted receipt should discharge the wait premise");
    let process = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "process")
        .expect("process machine");
    let plan = checked
        .facts
        .termination
        .for_machine(process.symbol)
        .expect("checked termination plan");
    assert_eq!(
        plan.checked_summary,
        language_semantics::TerminationGuarantee::Terminates {
            premises: Vec::new(),
        }
    );
}

#[test]
fn progress_subject_identity_threads_through_named_state_transitions() {
    let typed = typed_program_from_source(
        r#"
        data SchedulerHandle {}
        domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair
            ensures result in SchedulerHandle::WeakFair
            terminates;
        }
        boundary trait SchedulerRuntime {
            machine wait(scheduler: SchedulerHandle)
            requires scheduler in WeakFair
            terminates;
        }
        machine process(
            runtime: &mut SchedulerRuntime,
            scheduler: SchedulerHandle
        )
        requires scheduler in WeakFair
        terminates
        {
            transition { _ -> waiting(runtime, scheduler) }

            state waiting(
                runtime: &mut SchedulerRuntime,
                scheduler: SchedulerHandle in WeakFair
            ) {
                runtime.wait(scheduler);
            }
        }
        "#,
    );

    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("the transition should preserve the exact public progress subject");
    let process = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "process")
        .expect("process machine");
    let plan = checked
        .facts
        .termination
        .for_machine(process.symbol)
        .expect("checked termination plan");
    let language_semantics::TerminationGuarantee::Terminates { premises } = &plan.checked_summary
    else {
        panic!("process should retain a checked termination summary")
    };
    assert_eq!(premises.len(), 1);
}

#[test]
fn progress_subject_alternatives_across_state_predecessors_remain_explicit() {
    let typed = typed_program_from_source(
        r#"
        data SchedulerHandle {}
        domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair
            ensures result in SchedulerHandle::WeakFair
            terminates;
        }
        boundary trait SchedulerRuntime {
            machine wait(scheduler: SchedulerHandle)
            requires scheduler in WeakFair
            terminates;
        }
        machine process(
            runtime: &mut SchedulerRuntime,
            first: SchedulerHandle,
            second: SchedulerHandle,
            choose_first: bool
        )
        requires first in WeakFair
        requires second in WeakFair
        terminates
        {
            transition choose_first {
                true -> waiting(runtime, first)
                false -> waiting(runtime, second)
            }

            state waiting(
                runtime: &mut SchedulerRuntime,
                scheduler: SchedulerHandle in WeakFair
            ) {
                runtime.wait(scheduler);
            }
        }
        "#,
    );

    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("both exact predecessor subjects are covered by the public contract");
    let process = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "process")
        .expect("process machine");
    let plan = checked
        .facts
        .termination
        .for_machine(process.symbol)
        .expect("checked termination plan");
    let language_semantics::TerminationGuarantee::Terminates { premises } = &plan.checked_summary
    else {
        panic!("process should retain a checked termination summary")
    };
    assert_eq!(premises.len(), 2);
}

#[test]
fn inherited_progress_schema_substitutes_implementation_parameter_identity() {
    let typed = typed_program_from_source(
        r#"
        data SchedulerHandle {}
        domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
        }
        boundary trait SchedulerRuntime {
            machine wait(scheduler: SchedulerHandle) -> u64
            requires scheduler in WeakFair
            terminates;
        }
        machine wait_impl(scheduler: SchedulerHandle) -> u64
        satisfies SchedulerRuntime::wait
        {
            0
        }
        "#,
    );

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "wait_impl")
        .expect("implementation machine");
    let [parameter] = typed
        .machine_states(machine)
        .first()
        .map(|state| typed.state_parameters(state))
        .expect("implementation state")
    else {
        panic!("implementation should have one parameter")
    };
    let language_semantics::TerminationInterface::Published(
        language_semantics::TerminationGuarantee::Terminates { premises },
    ) = &machine.termination_plan.interface
    else {
        panic!("implementation should inherit termination")
    };
    let [premise] = premises.as_slice() else {
        panic!("implementation should inherit one premise")
    };
    assert_eq!(premise.subject.root, parameter.symbol);
}

#[test]
fn closed_conformance_rejects_an_incompatible_inline_row() {
    let typed = typed_program_from_source(
        r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }

        data Card {}
        PowerOrder: Card satisfies Ranked {
            machine before(&self, other: i32) -> bool {
                transition { _ -> (false) }
            }
        }
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("the retained inline row has the wrong signature");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not satisfy trait `Ranked` machine `before`")
    }));
}

#[test]
fn closed_conformance_validates_same_named_result_overloads() {
    let typed = typed_program_from_source(
        r#"
        trait Converter {
            machine Self::convert(&self, value: i32) -> i32 {
                value
            }
            machine Self::convert(&self, value: i32) -> i32 in Saturating {
                value
            }
        }

        data Item {}
        Primary: Item satisfies Converter {
            machine convert(&self, value: i32) -> i32 in Saturating {
                transition { _ -> (value) }
            }
        }
        "#,
    );

    validate_program(&typed)
        .expect("each exact result overload should retain its own checked conformance row");
}

#[test]
fn witness_proposition_requires_a_direct_carrierless_trait_interface() {
    let typed = typed_program_from_source(
        r#"
        trait Evidence {
            machine cite(value: i32) ensures value == value;
        }

        proposition witnessed(value: i32) evidence Evidence;
        "#,
    );

    validate_program(&typed).expect("a subjectless checked trait is a carrierless interface");
}

#[test]
fn witness_proposition_rejects_data_as_its_evidence_interface() {
    let typed = typed_program_from_source(
        r#"
        data Evidence {}
        proposition witnessed(value: i32) evidence Evidence;
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("ordinary data must not stand in for an evidence interface");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("evidence `Evidence` is not a trait interface")
    }));
}

#[test]
fn witness_proposition_rejects_carrier_bearing_trait_interface() {
    let typed = typed_program_from_source(
        r#"
        trait Evidence {
            machine cite(&self);
        }

        proposition witnessed(value: i32) evidence Evidence;
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("an evidence interface must not depend on a carrier instance");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("is not carrierless")
            && diagnostic.message.contains("carrier-dependent parameter")
    }));
}

#[test]
fn witness_proposition_declaration_rejects_selected_dynamic_evidence() {
    let typed = typed_program_from_source(
        r#"
        trait Evidence {
            machine cite(value: i32) ensures value == value;
        }

        proposition witnessed(value: i32) evidence dyn Evidence;
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("the declaration names an interface, not one selected evidence value");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("names selected dynamic evidence `dyn Evidence`")
    }));
}

#[test]
fn witness_proposition_rejects_boundary_evidence_interface() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Evidence {
            machine cite(value: i32) ensures value == value;
        }

        proposition witnessed(value: i32) evidence Evidence;
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("an externally executed service is not carrierless proof evidence");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("is not carrierless")
            && diagnostic
                .message
                .contains("boundary traits describe externally executed services")
    }));
}

#[test]
fn witness_proposition_checks_evidence_trait_generic_arity() {
    let typed = typed_program_from_source(
        r#"
        trait Evidence<Carrier> {
            machine cite(value: Carrier) ensures value == value;
        }

        proposition witnessed(value: i32) evidence Evidence;
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("the evidence interface must apply the complete trait telescope");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "evidence trait `Evidence` expects 1 generic argument(s), but the interface supplies 0",
        )
    }));
}

#[test]
fn witness_proposition_accepts_bound_generic_evidence_interface() {
    let typed = typed_program_from_source(
        r#"
        trait Evidence<Carrier> {
            machine cite(value: Carrier) ensures value == value;
        }

        proposition witnessed<Carrier>(value: Carrier) evidence Evidence<Carrier>;
        "#,
    );

    validate_program(&typed)
        .expect("the proposition telescope may bind the evidence interface telescope");
}

#[test]
fn named_contract_evidence_rejects_fact_only_propositions() {
    let typed = typed_program_from_source(
        r#"
        proposition visible(value: i32);

        machine consume(value: i32)
        requires proof: visible(value)
        {
        }
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("fact-only propositions have no evidence term");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("named requires evidence `proof` binds fact-only proposition `visible`")
    }));
}

#[test]
fn named_contract_evidence_rejects_boolean_facts() {
    let typed = typed_program_from_source(
        r#"
        machine consume(value: i32)
        requires proof: value == value
        {
        }
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("a Boolean fact has no projectable evidence term");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "named requires evidence `proof` must bind exactly one proposition application",
        )
    }));
}

#[test]
fn named_contract_evidence_rejects_domain_memberships() {
    let typed = typed_program_from_source(
        r#"
        data Token {}
        domain Token::Issued;

        machine consume(value: Token)
        requires proof: value in Token::Issued
        {
        }
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("a domain membership has no projectable evidence term");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "named requires evidence `proof` must bind exactly one proposition application",
        )
    }));
}

#[test]
fn named_contract_evidence_rejects_transparent_boolean_propositions() {
    let typed = typed_program_from_source(
        r#"
        proposition positive(value: i32) = value > 0;

        machine consume(value: i32)
        requires proof: positive(value)
        {
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a transparent Boolean proposition has no nominal evidence endpoint");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "named requires evidence `proof` does not resolve to one nominal proposition endpoint",
        )
    }));
}

#[test]
fn proposition_contract_requires_exact_normalized_application() {
    let typed = typed_program_from_source(
        r#"
        proposition related(left: i32, right: i32);
        proposition self_related(value: i32) = related(value, value);

        machine consume(left: i32, right: i32)
        requires related(left, right)
        {
        }

        machine caller(value: i32)
        requires self_related(value)
        {
            consume(value, value);
        }
        "#,
    );

    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("a transparent alias must establish its normalized expansion");
}

#[test]
fn proposition_contract_rejects_different_application() {
    let typed = typed_program_from_source(
        r#"
        proposition related(left: i32, right: i32);

        machine consume(left: i32, right: i32)
        requires related(left, right)
        {
        }

        machine caller(left: i32, right: i32)
        requires related(left, left)
        {
            consume(left, right);
        }
        "#,
    );

    let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect_err("a different proposition argument tuple must not be accepted");
    assert!(diagnostics.iter().any(|diagnostic| {
        (diagnostic
            .message
            .contains("cannot prove requires contract")
            || diagnostic
                .message
                .contains("is not established at the call"))
            && diagnostic.message.contains("proposition:fact:related")
    }));
}

#[test]
fn transparent_boolean_proposition_normalizes_to_boolean_fact() {
    let typed = typed_program_from_source(
        r#"
        proposition positive(value: i32) = value > 0;

        machine consume(value: i32)
        requires positive(value)
        {
        }

        machine caller(value: i32)
        requires value > 0
        {
            consume(value);
        }
        "#,
    );

    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("a transparent Boolean proposition must normalize to its Boolean fact");
}

#[test]
fn transparent_proposition_alias_cycle_rejects() {
    let typed = typed_program_from_source(
        r#"
        proposition first(value: i32) = second(value);
        proposition second(value: i32) = first(value);
        "#,
    );

    let diagnostics = validate_program(&typed).expect_err("transparent aliases must be acyclic");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("participates in an alias cycle")
    }));
}

#[test]
fn bodyless_checked_machine_cannot_invent_proposition_ensure() {
    let typed = typed_program_from_source(
        r#"
        proposition related(left: i32, right: i32);

        machine invented(value: i32)
        ensures related(value, value)
        {
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("an ordinary empty body must not invent a primitive proposition");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot establish proposition ensure")
    }));
}

#[test]
fn checked_machine_may_forward_required_proposition() {
    let typed = typed_program_from_source(
        r#"
        proposition related(left: i32, right: i32);

        machine forwarded(left: i32, right: i32)
        requires related(left, right)
        ensures related(left, right)
        {
        }
        "#,
    );

    validate_program(&typed).expect("a required proposition may be forwarded unchanged");
}

#[test]
fn checked_machine_may_cite_accepted_proposition_axiom() {
    let typed = typed_program_from_source(
        r#"
        pub proposition reflexive(value: i32);

        boundary machine accepted_reflexivity(value: i32)
        ensures reflexive(value);

        machine cite(value: i32)
        ensures reflexive(value)
        {
            accepted_reflexivity(value);
        }
        "#,
    );

    validate_program(&typed).expect("an accepted proposition axiom may be cited explicitly");
}

#[test]
fn proposition_application_rejects_wrong_value_argument_type() {
    let typed = typed_program_from_source(
        r#"
        proposition integer_fact(value: i32);

        machine bad()
        requires integer_fact(true)
        {
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a proposition argument must match its declared parameter type");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("argument 1 does not match")
            && diagnostic.message.contains("parameter `value` type `i32`")
    }));
}

#[test]
fn generic_proposition_parameter_validates_its_authored_application_signature() {
    let typed = typed_program_from_source(
        r#"
        trait Reflexive<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
            machine prove(value: C) ensures Relation(value, value);
        }
        "#,
    );

    validate_program(&typed)
        .expect("a generic proposition fact should validate against its authored signature");
}

#[test]
fn generic_proposition_parameter_rejects_wrong_value_argument_type() {
    let typed = typed_program_from_source(
        r#"
        trait Reflexive<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
            machine bad(value: C) ensures Relation(value, true);
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a generic proposition argument must match its authored parameter type");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("argument 2 does not match")
            && diagnostic.message.contains("parameter `right` type `C`")
    }));
}

#[test]
fn concrete_proposition_family_substitutes_into_trait_application() {
    let typed = typed_program_from_source(
        r#"
        data Carrier {}
        proposition related(left: Carrier, right: Carrier);

        trait RelationKind<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
        }

        CarrierRelationKind: Carrier satisfies RelationKind<Carrier, related>;
        "#,
    );

    validate_program(&typed)
        .expect("a proposition declaration with the substituted signature should be accepted");
}

#[test]
fn trait_proposition_slot_rejects_a_data_type_argument() {
    let typed = typed_program_from_source(
        r#"
        data Carrier {}

        trait RelationKind<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
        }

        CarrierRelationKind: Carrier satisfies RelationKind<Carrier, Carrier>;
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a proposition-family slot must not accept a runtime data type");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "argument `Carrier` for proposition parameter `Relation` is not a proposition family",
        )
    }));
}

#[test]
fn concrete_proposition_family_rejects_incompatible_signature() {
    let typed = typed_program_from_source(
        r#"
        data Carrier {}
        proposition related(left: Carrier, right: bool);

        trait RelationKind<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
        }

        CarrierRelationKind: Carrier satisfies RelationKind<Carrier, related>;
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a concrete proposition family must match the substituted signature");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("proposition family `related`")
            && diagnostic.message.contains("value parameter 2")
            && diagnostic
                .message
                .contains("requires `Carrier` after substitution")
    }));
}

#[test]
fn trait_composition_forwards_a_proposition_family_parameter() {
    let typed = typed_program_from_source(
        r#"
        trait Reflexive<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
        }

        trait Equivalence<C, proposition Relation>: Reflexive<C, Relation>
        where proposition Relation(left: C, right: C);
        {
        }
        "#,
    );

    validate_program(&typed)
        .expect("a composed trait should forward its proposition-family parameter exactly");
}

#[test]
fn indexed_carrier_family_substitutes_a_relation_telescope() {
    let typed = typed_program_from_source(
        r#"
        data Index {}

        data Stream<machine Sequence>
        where machine Sequence(index: Index) -> Index;
        {
            case Empty;
            case More(tail: Stream<Sequence>);
        }

        proposition stream_related<machine Left, machine Right>(
            left: Stream<Left>,
            right: Stream<Right>
        );

        trait RelationKind<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
        }

        StreamRelationKind: Stream satisfies RelationKind<Stream, stream_related>;
        "#,
    );

    validate_program(&typed)
        .expect("the relation should instantiate one fresh carrier telescope per representative");
}

#[test]
fn indexed_relation_requires_a_fresh_telescope_per_representative() {
    let typed = typed_program_from_source(
        r#"
        data Index {}

        data Stream<machine Sequence>
        where machine Sequence(index: Index) -> Index;
        {
            case Empty;
            case More(tail: Stream<Sequence>);
        }

        proposition aliases_index<machine Shared, machine Unused>(
            left: Stream<Shared>,
            right: Stream<Shared>
        );

        trait RelationKind<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
        }

        StreamRelationKind: Stream satisfies RelationKind<Stream, aliases_index>;
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("each representative must receive its own carrier index telescope");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("proposition family `aliases_index`")
            && diagnostic.message.contains("value parameter 2")
    }));
}

#[test]
fn proposition_law_conformance_substitutes_the_selected_family() {
    let typed = typed_program_from_source(
        r#"
        data Carrier {}
        proposition related(left: Carrier, right: Carrier);

        trait Reflexive<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
            machine reflexive(value: C) ensures Relation(value, value);
        }

        machine reflexive(value: Carrier)
        satisfies Reflexive<Carrier, related>::reflexive
        requires related(value, value)
        ensures related(value, value)
        {
        }
        "#,
    );

    validate_program(&typed)
        .expect("the proof machine should satisfy the substituted proposition law exactly");
}

#[test]
fn proposition_law_conformance_rejects_a_different_ensures() {
    let typed = typed_program_from_source(
        r#"
        data Carrier {}
        proposition related(left: Carrier, right: Carrier);
        proposition unrelated(left: Carrier, right: Carrier);

        trait Reflexive<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
            machine reflexive(value: C) ensures Relation(value, value);
        }

        machine reflexive(value: Carrier)
        satisfies Reflexive<Carrier, related>::reflexive
        requires unrelated(value, value)
        ensures unrelated(value, value)
        {
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a proof of another proposition must not satisfy the selected relation law");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("proves no ensures matching proposition law")
            && diagnostic.message.contains("proposition:fact:related")
    }));
}

#[test]
fn closed_conformance_rejects_a_realization_that_does_not_prove_its_law() {
    let typed = typed_program_from_source(
        r#"
        data Carrier {}
        proposition related(left: Carrier, right: Carrier);
        proposition unrelated(left: Carrier, right: Carrier);

        trait Reflexive<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
            machine reflexive(value: C) ensures Relation(value, value);
        }

        Primary: Carrier satisfies Reflexive<Carrier, related> {
            machine reflexive(value: Carrier)
            requires unrelated(value, value)
            ensures unrelated(value, value)
            {
            }
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a closed row must prove the exact substituted trait law");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("proves no ensures matching proposition law")
            && diagnostic.message.contains("proposition:fact:related")
    }));
}

#[test]
fn bodyless_conformance_rejects_an_attached_machine_that_does_not_prove_its_law() {
    let typed = typed_program_from_source(
        r#"
        data Carrier {}
        proposition related(left: Carrier, right: Carrier);
        proposition unrelated(left: Carrier, right: Carrier);

        trait Reflexive<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
            machine reflexive(value: C) ensures Relation(value, value);
        }

        machine Carrier::reflexive(value: Carrier)
        requires unrelated(value, value)
        ensures unrelated(value, value)
        {
        }
        Primary: Carrier satisfies Reflexive<Carrier, related>;
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("an attached realization must prove the exact substituted trait law");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("proves no ensures matching proposition law")
            && diagnostic.message.contains("proposition:fact:related")
    }));
}

#[test]
fn proposition_law_conformance_rejects_a_same_spelled_foreign_endpoint() {
    let mut typed = typed_program_from_source(
        r#"
        data Carrier {}
        proposition related(left: Carrier, right: Carrier);
        proposition unrelated(left: Carrier, right: Carrier);

        trait Reflexive<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
            machine reflexive(value: C) ensures Relation(value, value);
        }

        machine reflexive(value: Carrier)
        satisfies Reflexive<Carrier, related>::reflexive
        requires unrelated(value, value)
        ensures unrelated(value, value)
        {
        }
        "#,
    );
    let proposition_span = typed.roots.propositions;
    let propositions = typed
        .tables
        .propositions
        .span_mut_or_empty(proposition_span);
    let [related, unrelated] = propositions else {
        panic!("related and unrelated proposition declarations")
    };
    unrelated.name = related.name.clone();

    let diagnostics = validate_program(&typed).expect_err(
        "same display spelling must not let another proposition symbol discharge the law",
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("proves no ensures matching proposition law")
    }));
}

#[test]
fn indexed_proposition_law_synthesizes_representative_binders() {
    let typed = typed_program_from_source(
        r#"
        data Index {}

        data Stream<machine Sequence>
        where machine Sequence(index: Index) -> Index;
        {
            case Empty;
            case More(tail: Stream<Sequence>);
        }

        proposition stream_related<machine Left, machine Right>(
            left: Stream<Left>,
            right: Stream<Right>
        );

        trait Reflexive<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
            machine reflexive(value: C) ensures Relation(value, value);
        }

        machine reflexive<machine Sequence>(value: Stream<Sequence>)
        where machine Sequence(index: Index) -> Index;
        satisfies Reflexive<Stream, stream_related>::reflexive
        requires stream_related<Sequence, Sequence>(value, value)
        ensures stream_related<Sequence, Sequence>(value, value)
        {
        }
        "#,
    );

    validate_program(&typed).expect(
        "the indexed reflexive law should synthesize the representative's carrier telescope",
    );
}

#[test]
fn indexed_binary_law_uses_fresh_binders_for_each_representative() {
    let typed = typed_program_from_source(
        r#"
        data Index {}

        data Stream<machine Sequence>
        where machine Sequence(index: Index) -> Index;
        {
            case Empty;
            case More(tail: Stream<Sequence>);
        }

        proposition stream_related<machine Left, machine Right>(
            left: Stream<Left>,
            right: Stream<Right>
        );

        trait Paired<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
            machine paired(left: C, right: C) ensures Relation(left, right);
        }

        machine paired<machine Left, machine Right>(
            left: Stream<Left>,
            right: Stream<Right>
        )
        where machine Left(index: Index) -> Index;
        where machine Right(index: Index) -> Index;
        satisfies Paired<Stream, stream_related>::paired
        requires stream_related<Left, Right>(left, right)
        ensures stream_related<Left, Right>(left, right)
        {
        }
        "#,
    );

    validate_program(&typed)
        .expect("a binary law should retain independent representative index packs");
}

#[test]
fn indexed_binary_law_rejects_swapped_representative_binders() {
    let typed = typed_program_from_source(
        r#"
        data Index {}

        data Stream<machine Sequence>
        where machine Sequence(index: Index) -> Index;
        {
            case Empty;
            case More(tail: Stream<Sequence>);
        }

        proposition stream_related<machine Left, machine Right>(
            left: Stream<Left>,
            right: Stream<Right>
        );

        trait Paired<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
            machine paired(left: C, right: C) ensures Relation(left, right);
        }

        machine paired<machine Left, machine Right>(
            left: Stream<Left>,
            right: Stream<Right>
        )
        where machine Left(index: Index) -> Index;
        where machine Right(index: Index) -> Index;
        satisfies Paired<Stream, stream_related>::paired
        requires stream_related<Right, Left>(left, right)
        ensures stream_related<Right, Left>(left, right)
        {
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("swapping independent carrier binders must not prove the indexed law");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("proves no ensures matching proposition law")
    }));
}

#[test]
fn indexed_binary_law_rejects_a_reused_representative_telescope() {
    let typed = typed_program_from_source(
        r#"
        data Index {}

        data Stream<machine Sequence>
        where machine Sequence(index: Index) -> Index;
        {
            case Empty;
            case More(tail: Stream<Sequence>);
        }

        proposition stream_related<machine Left, machine Right>(
            left: Stream<Left>,
            right: Stream<Right>
        );

        trait Paired<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
            machine paired(left: C, right: C) ensures Relation(left, right);
        }

        machine paired<machine Shared>(
            left: Stream<Shared>,
            right: Stream<Shared>
        )
        where machine Shared(index: Index) -> Index;
        satisfies Paired<Stream, stream_related>::paired
        requires stream_related<Shared, Shared>(left, right)
        ensures stream_related<Shared, Shared>(left, right)
        {
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("each representative must receive a fresh carrier telescope");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("expected 2 callable generic parameter(s), got 1")
    }));
}

#[test]
fn local_dynamic_value_rejects_boundary_trait() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Service {
            machine ping();
        }

        machine inspect(service: &dyn Service) {}
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("a boundary trait is not a local dyn surface");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "uses boundary trait `Service` as a local dynamic value; local dynamic descriptors cannot cross a replaceable component boundary",
        )
    }));
}

#[test]
fn local_dynamic_value_rejects_unbound_generic_trait() {
    let typed = typed_program_from_source(
        r#"
        trait Projection<T> {
            machine project(&self) -> T;
        }

        machine inspect(projection: &dyn Projection) {}
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("a generic dyn surface must bind its trait parameters");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "uses generic trait `Projection` as an unbound dynamic value; bind its 1 generic parameter(s)",
        )
    }));
}

#[test]
fn local_dynamic_call_rejects_self_outside_receiver_without_rejecting_trait() {
    let typed = typed_program_from_source(
        r#"
        trait Comparable {
            machine compare(&self, other: &Self);
        }

        machine compare_erased(left: &dyn Comparable, right: &dyn Comparable) {
            left.compare(right);
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("Self outside the receiver is absent from the dyn surface");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "requirement `Comparable::compare` is absent from `dyn Comparable`: `Self` appears outside the borrowed receiver",
        )
    }));
}

#[test]
fn local_dynamic_call_keeps_eligible_sibling_available() {
    let typed = typed_program_from_source(
        r#"
        trait Mixed {
            machine run(&self);
            machine compare(&self, other: &Self);
        }

        machine run_erased(value: &dyn Mixed) {
            value.run();
        }
        "#,
    );

    let mixed = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Mixed")
        .expect("Mixed trait");
    let surface = typed
        .dynamic_signature_surface(mixed)
        .map(|requirement| requirement.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(surface, vec!["run"]);

    validate_program(&typed)
        .expect("an ineligible sibling must not remove an eligible dyn requirement");
}

#[test]
fn local_dynamic_call_rejects_self_result() {
    let typed = typed_program_from_source(
        r#"
        trait Cloneable {
            machine clone(&self) -> Self;
        }

        machine clone_erased(value: &dyn Cloneable) {
            value.clone();
        }
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("a Self result is absent from the dyn surface");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "requirement `Cloneable::clone` is absent from `dyn Cloneable`: `Self` appears in the result type",
        )
    }));
}

#[test]
fn local_dynamic_coercion_retains_one_complete_nominal_conformance() {
    let typed = typed_program_from_source(
        r#"
        trait Marker {}
        data Item {}
        Primary: Item satisfies Marker {}

        machine erase(item: Item) {
            let erased: &dyn Marker = &item as &dyn Marker;
        }
        "#,
    );

    validate_program(&typed).expect("one complete conformance is selected uniquely");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("checked lowering retains the exact selection");
    let [selection] = checked.facts.dynamic_conformances.selections.as_slice() else {
        panic!("one dynamic conformance selection");
    };
    assert_eq!(checked.symbols.name(selection.source_data), "Item");
    assert_eq!(checked.symbols.name(selection.target_trait), "Marker");
    assert_eq!(
        selection
            .conformance
            .map(|symbol| checked.symbols.name(symbol)),
        Some("Primary")
    );
    assert!(selection.occurrence.is_valid());
}

#[test]
fn local_dynamic_coercion_retains_closed_conformance_rows() {
    let typed = typed_program_from_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }
        data Item {}
        Primary: Item satisfies Shape {
            machine code(&self) -> i32 {
                transition { _ -> (7) }
            }
        }

        machine erase(item: Item) {
            let erased: &dyn Shape = &item as &dyn Item::Primary;
        }
        "#,
    );

    validate_program(&typed).expect("the closed conformance should license local dyn selection");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("checked lowering should retain the selected row map");
    let [selection] = checked.facts.dynamic_conformances.selections.as_slice() else {
        panic!("one dynamic conformance selection");
    };
    let [row] = selection.rows.as_slice() else {
        panic!("one normalized dynamic row");
    };
    assert_eq!(checked.symbols.name(row.declaring_trait), "Shape");
    assert_eq!(checked.symbols.name(row.requirement), "code");
    assert_eq!(
        checked.symbols.name(row.realization_machine),
        "Item::Primary::code"
    );
    assert_eq!(checked.symbols.name(row.realization_state), "code");
}

#[test]
fn local_dynamic_coercion_retains_an_instantiated_trait_default_row() {
    let typed = typed_program_from_source(
        r#"
        trait Shape {
            machine touch(&self) {}
        }
        data Item {}
        Primary: Item satisfies Shape {}

        machine erase(item: Item) {
            let erased: &dyn Shape = &item as &dyn Item::Primary;
        }
        "#,
    );

    validate_program(&typed).expect("the instantiated default row should validate");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("checked dyn facts should retain the instantiated default row");
    let [selection] = checked.facts.dynamic_conformances.selections.as_slice() else {
        panic!("one dynamic conformance selection");
    };
    let [row] = selection.rows.as_slice() else {
        panic!("one normalized dynamic row");
    };
    assert_eq!(format!("{:?}", row.source), "TraitDefault");
    assert_eq!(
        checked.symbols.name(row.realization_machine),
        "Item::Primary::Shape::touch"
    );
    assert_eq!(checked.symbols.name(row.realization_state), "touch");
}

#[test]
fn local_dynamic_coercion_retains_each_result_overload_row() {
    let typed = typed_program_from_source(
        r#"
        trait Converter {
            machine Self::convert(&self, value: i32) -> i32 { value }
            machine Self::convert(&self, value: i32) -> i32 in Saturating { value }
        }
        data Item {}
        Primary: Item satisfies Converter {}

        machine erase(item: Item) {
            let erased: &dyn Converter = &item as &dyn Item::Primary;
        }
        "#,
    );

    validate_program(&typed).expect("both exact overload rows should license dyn selection");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("checked dyn facts should retain both exact overload rows");
    let [selection] = checked.facts.dynamic_conformances.selections.as_slice() else {
        panic!("one dynamic conformance selection");
    };
    assert_eq!(selection.rows.len(), 2);
    assert_ne!(selection.rows[0].requirement, selection.rows[1].requirement);
    assert_ne!(
        selection.rows[0].realization_state,
        selection.rows[1].realization_state
    );
}

#[test]
fn closed_conformance_instantiates_an_inherited_generic_trait_default() {
    let typed = typed_program_from_source(
        r#"
        trait Parent<T> {
            machine Self::touch(&self, value: T) {}
        }
        trait Child: Parent<i32> {}
        data Item {}
        Primary: Item satisfies Child {}
        "#,
    );

    validate_program(&typed)
        .expect("the inherited default should be instantiated with Parent<i32>");
    let conformance = typed.conformances().iter().next().expect("one conformance");
    let [row] = typed
        .closed_conformance_rows(conformance)
        .expect("closed rows")
    else {
        panic!("one inherited default row");
    };
    assert_eq!(row.declaring_trait_name.as_str(), "Parent");
    assert_eq!(row.requirement_name.as_str(), "touch");
    assert!(row.realization_machine.is_valid());
    assert!(row.realization_state.is_valid());
}

#[test]
fn bare_local_dynamic_coercion_rejects_ambiguous_conformances() {
    let typed = typed_program_from_source(
        r#"
        trait Marker {
            machine code(&self) -> i32;
        }
        data Item {}
        First: Item satisfies Marker {
            machine code(&self) -> i32 { 1 }
        }
        Second: Item satisfies Marker {
            machine code(&self) -> i32 { 2 }
        }

        machine erase(item: Item) -> i32 {
            let erased: &dyn Marker = &item as &dyn Marker;
            erased.code()
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a bare dynamic coercion cannot choose between conformances");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "local dynamic coercion from `Item` to `dyn Marker` has 2 complete nominal conformances; select one exact named conformance",
        )
    }));
}

#[test]
fn bare_dynamic_parameter_argument_rejects_ambiguous_conformances() {
    let typed = typed_program_from_source(
        r#"
        trait Shape { machine code(&self) -> i32; }
        data Item {}
        First: Item satisfies Shape {
            machine code(&self) -> i32 { 1 }
        }
        Second: Item satisfies Shape {
            machine code(&self) -> i32 { 2 }
        }

        machine dispatch(erased: &dyn Shape) -> i32 { erased.code() }
        machine run(item: Item) -> i32 { dispatch(&item) }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a bare dynamic argument must not silently select one conformance");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "call to `dispatch` cannot pass `Item` to bare dynamic parameter `erased`: 2 complete closed conformances to `Shape` are available; declare the parameter with one exact named dynamic conformance"
        )),
        "expected an explicit conformance-selection diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn exact_named_dynamic_parameter_resolves_argument_conformance() {
    let typed = typed_program_from_source(
        r#"
        trait Shape { machine code(&self) -> i32; }
        data Item {}
        First: Item satisfies Shape {
            machine code(&self) -> i32 { 1 }
        }
        Second: Item satisfies Shape {
            machine code(&self) -> i32 { 2 }
        }

        machine dispatch(erased: &dyn Item::First) -> i32 { erased.code() }
        machine run(item: Item) -> i32 { dispatch(&item) }
        "#,
    );

    validate_program(&typed)
        .expect("an exact named dynamic parameter selects one conformance at the call boundary");
}

#[test]
fn selected_local_dynamic_value_passes_to_a_compatible_bare_parameter() {
    let typed = typed_program_from_source(
        r#"
        trait Shape { machine code(&self) -> i32; }
        data Item {}
        First: Item satisfies Shape {
            machine code(&self) -> i32 { 1 }
        }
        Second: Item satisfies Shape {
            machine code(&self) -> i32 { 2 }
        }

        machine dispatch(erased: &dyn Shape) -> i32 { erased.code() }
        machine run(item: Item) -> i32 {
            let erased: &dyn Shape = &item as &dyn Item::First;
            dispatch(erased)
        }
        "#,
    );

    validate_program(&typed)
        .expect("an already-selected local descriptor should pass to the same bare trait surface");
}

#[test]
fn selected_local_dynamic_value_does_not_pass_to_a_different_bare_trait() {
    let typed = typed_program_from_source(
        r#"
        trait Shape { machine code(&self) -> i32; }
        trait Other { machine code(&self) -> i32; }
        data Item {}
        First: Item satisfies Shape {
            machine code(&self) -> i32 { 1 }
        }
        OtherImpl: Item satisfies Other {
            machine code(&self) -> i32 { 2 }
        }

        machine dispatch(erased: &dyn Other) -> i32 { erased.code() }
        machine run(item: Item) -> i32 {
            let erased: &dyn Shape = &item as &dyn Item::First;
            dispatch(erased)
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a descriptor for one trait must not be rebound as another trait");
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
        "cannot pass dynamic value to bare parameter `erased` without one earlier exact compatible local conformance selection"
    )), "unexpected diagnostics: {diagnostics:?}");
}

#[test]
fn named_local_dynamic_coercion_selects_one_exact_conformance() {
    let typed = typed_program_from_source(
        r#"
        trait Marker {}
        data Item {}
        First: Item satisfies Marker {}
        Second: Item satisfies Marker {}

        machine erase(item: Item) {
            let erased: &dyn Marker = &item as &dyn Item::First;
        }
        "#,
    );

    let erase = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "erase")
        .expect("authored erase machine");
    let cast_target = typed
        .machine_states(erase)
        .iter()
        .flat_map(|state| typed.statement_table.statements(state.statement_nodes))
        .find_map(|statement| {
            let typed_trees::statement::StatementNode::LocalData(local) = statement else {
                return None;
            };
            let typed_trees::expression::ExpressionNode::Borrow(borrow) =
                typed.expression_table.expression(local.initial_value)
            else {
                return None;
            };
            let typed_trees::expression::ExpressionNode::Cast(cast) =
                typed.expression_table.expression(borrow.target)
            else {
                return None;
            };
            Some(cast.target_type)
        })
        .expect("named dynamic cast target");
    let identity = typed.normalized_type_identity(cast_target);
    assert!(
        identity.as_str().contains("conformance(First)"),
        "named dynamic identity: {identity}"
    );

    validate_program(&typed).expect("an exact named conformance resolves ambiguity");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("checked lowering retains the exact named selection");
    let [selection] = checked.facts.dynamic_conformances.selections.as_slice() else {
        panic!("one named dynamic conformance selection");
    };
    assert_eq!(checked.symbols.name(selection.source_data), "Item");
    assert_eq!(checked.symbols.name(selection.target_trait), "Marker");
    assert_eq!(
        selection
            .conformance
            .map(|symbol| checked.symbols.name(symbol)),
        Some("First")
    );
}

#[test]
fn dynamic_statement_call_retains_exact_inherited_requirement_symbol() {
    let typed = typed_program_from_source(
        r#"
        trait Parent {
            machine ping(&self) {}
        }
        trait Child: Parent {}
        data Item {}
        Primary: Item satisfies Child {}

        machine run(item: Item) {
            let erased: &dyn Child = &item as &dyn Item::Primary;
            erased.ping();
        }
        "#,
    );

    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("an inherited dynamic requirement should resolve exactly");
    let [selection] = checked.facts.dynamic_conformances.selections.as_slice() else {
        panic!("one selected conformance");
    };
    let row = selection
        .rows
        .iter()
        .find(|row| checked.symbols.name(row.requirement) == "ping")
        .expect("inherited ping row");
    let run = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .expect("run machine");
    let call = checked
        .machine_states(run)
        .iter()
        .flat_map(|state| checked.statement_table.statements(state.statement_nodes))
        .find_map(|statement| {
            let typed_trees::statement::StatementNode::Call(call) = statement else {
                return None;
            };
            (call.target.as_str() == "ping").then_some(call)
        })
        .expect("dynamic statement call");
    assert_eq!(call.target_symbol, row.requirement);
    assert_eq!(checked.symbols.name(row.declaring_trait), "Parent");
}

#[test]
fn dynamic_call_rejects_ambiguous_inherited_requirement_spelling() {
    let mut typed = typed_program_from_source(
        r#"
        trait Left {
            machine ping(&self);
        }
        trait Right {
            machine ping(&self);
        }
        trait Both: Left + Right {}
        data Item {}

        machine Item::left_ping(&self) {}
        machine Item::right_ping(&self) {}

        Primary: Item satisfies Both {
            Left::ping = Item::left_ping;
            Right::ping = Item::right_ping;
        }

        machine run(item: Item) {
            let erased: &dyn Both = &item as &dyn Item::Primary;
            erased.ping();
        }
        "#,
    );

    let left_ping = typed
        .traits()
        .iter()
        .find(|trait_definition| trait_definition.name.as_str() == "Left")
        .and_then(|trait_definition| typed.trait_machine_signatures(trait_definition).first())
        .map(|requirement| requirement.symbol)
        .expect("Left::ping requirement");
    let run_statements = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .and_then(|machine| typed.machine_states(machine).first())
        .map(|state| state.statement_nodes)
        .expect("run statements");
    let call = typed
        .statement_table
        .statements_mut(run_statements)
        .iter_mut()
        .find_map(|statement| {
            let typed_trees::statement::StatementNode::Call(call) = statement else {
                return None;
            };
            (call.target.as_str() == "ping").then_some(call)
        })
        .expect("dynamic statement call");
    call.target_symbol = left_ping;

    let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect_err("a provisional inherited symbol cannot resolve an ambiguous leaf spelling");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "dynamic call `Both::ping` is ambiguous across inherited requirements: Left::ping, Right::ping",
        )
    }));
}

#[test]
fn named_local_dynamic_coercion_rejects_unknown_selection() {
    let source = r#"
        trait Marker {}
        data Item {}
        Primary: Item satisfies Marker {}

        machine erase(item: Item) {
            let erased: &dyn Marker = &item as &dyn Item::Missing;
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostics =
        lower_syntax_trees(&syntax).expect_err("a named dynamic selection must resolve exactly");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("dynamic coercion selects unknown named conformance `Item::Missing`")),
        "expected unknown named conformance diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn named_local_dynamic_coercion_rejects_wrong_source_carrier() {
    let typed = typed_program_from_source(
        r#"
        trait Marker {}
        data Item {}
        data Other {}
        Primary: Item satisfies Marker {}

        machine erase(other: Other) {
            let erased: &dyn Item::Primary = &other as &dyn Item::Primary;
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a named conformance cannot erase a value of another carrier");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "local dynamic coercion from `Other` to `dyn Marker` cannot use named conformance `Item::Primary`",
        )
    }));
}

#[test]
fn local_dynamic_coercion_rejects_missing_conformance() {
    let typed = typed_program_from_source(
        r#"
        trait Marker {
            machine code(&self) -> i32;
        }
        data Item {}

        machine erase(item: Item) -> i32 {
            let erased: &dyn Marker = &item as &dyn Marker;
            erased.code()
        }
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("a dynamic coercion needs a nominal conformance");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "local dynamic coercion from `Item` to `dyn Marker` has no complete nominal conformance",
        )
    }));
}

#[test]
fn local_dynamic_coercion_rejects_bodyless_static_conformance() {
    let typed = typed_program_from_source(
        r#"
        trait Marker {
            machine code(&self) -> i32;
        }
        data Item {}
        machine Item::code(&self) -> i32 { 1 }
        Primary: Item satisfies Marker;

        machine erase(item: Item) -> i32 {
            let erased: &dyn Marker = &item as &dyn Item::Primary;
            erased.code()
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a bodyless static conformance has no authoritative dynamic row map");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "named conformance `Item::Primary` is bodyless and cannot license local dynamic dispatch; declare its complete row map with a conformance block",
        )
    }));
}

#[test]
fn same_conformance_local_dynamic_rebind_retains_both_exact_selections() {
    let typed = typed_program_from_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }
        data Item {}
        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { 1 }
        }
        machine dispatch(value: &dyn Shape) -> i32 { value.code() }

        machine choose(first: Item, second: Item) -> i32 {
            let mut erased: &dyn Shape = &first as &dyn Item::Primary;
            erased = &second as &dyn Item::Primary;
            dispatch(erased)
        }
        "#,
    );

    validate_program(&typed).expect("same-conformance dynamic rebind");
    let selections = validation::collect_dynamic_conformance_selections(&typed)
        .expect("exact rebind selections");
    assert_eq!(selections.len(), 2);
    assert_eq!(selections[0].binding, selections[1].binding);
    assert_eq!(selections[0].target_trait, selections[1].target_trait);
    assert_eq!(selections[0].conformance, selections[1].conformance);
    assert_ne!(selections[0].source_symbol, selections[1].source_symbol);
    assert!(selections[0].statement_index < selections[1].statement_index);
}

#[test]
fn direct_call_through_same_conformance_rebound_dynamic_local_is_admitted() {
    let typed = typed_program_from_source(
        r#"
        trait Shape { machine code(&self) -> i32; }
        data Item {}
        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { 1 }
        }
        machine choose(first: Item, second: Item) -> i32 {
            let mut erased: &dyn Shape = &first as &dyn Item::Primary;
            erased = &second as &dyn Item::Primary;
            erased.code()
        }
        "#,
    );

    validate_program(&typed).expect("exact same-conformance rebound direct call");
}

#[test]
fn dynamic_rebind_without_an_exact_cast_cannot_reuse_the_initializer_selection() {
    let typed = typed_program_from_source(
        r#"
        trait Shape { machine code(&self) -> i32; }
        data Item {}
        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { 1 }
        }
        machine dispatch(value: &dyn Shape) -> i32 { value.code() }
        machine choose(first: Item, second: Item) -> i32 {
            let mut erased: &dyn Shape = &first as &dyn Item::Primary;
            erased = &second;
            dispatch(erased)
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("an unselected assignment must not leave stale descriptor authority");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires an exact direct-place named-conformance cast")
    }));
}

#[test]
fn different_conformance_local_dynamic_rebind_retains_both_exact_selections() {
    let typed = typed_program_from_source(
        r#"
        trait Shape { machine code(&self) -> i32; }
        data Item {}
        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { 1 }
        }
        Secondary: Item satisfies Shape {
            machine code(&self) -> i32 { 2 }
        }

        machine choose(first: Item, second: Item) -> i32 {
            let mut erased: &dyn Shape = &first as &dyn Item::Primary;
            erased = &second as &dyn Item::Secondary;
            erased.code()
        }
        "#,
    );

    validate_program(&typed).expect("changed-conformance dynamic rebind");
    let selections = validation::collect_dynamic_conformance_selections(&typed)
        .expect("exact changed-conformance selections");
    assert_eq!(selections.len(), 2);
    assert_eq!(selections[0].binding, selections[1].binding);
    assert_eq!(selections[0].target_trait, selections[1].target_trait);
    assert_ne!(selections[0].conformance, selections[1].conformance);
    assert_ne!(selections[0].source_symbol, selections[1].source_symbol);
    assert!(selections[0].statement_index < selections[1].statement_index);
}

#[test]
fn named_whole_trait_conformance_survives_typing() {
    let typed = typed_program_from_source(
        r#"
        trait Marker {}
        data Item {}
        Primary: Item satisfies Marker;
        "#,
    );

    let [conformance] = typed.conformances() else {
        panic!("one conformance");
    };
    assert_eq!(
        conformance.carrier_name().map(|name| name.as_str()),
        Some("Item")
    );
    assert_eq!(conformance.trait_name.as_str(), "Marker");
    assert_eq!(
        conformance.alias.as_ref().map(|alias| alias.as_str()),
        Some("Primary")
    );
    assert!(conformance.symbol.is_valid());
    assert_eq!(
        typed.symbols.get(conformance.symbol).kind,
        symbols::SymbolKind::Conformance
    );
    assert_eq!(typed.symbols.name(conformance.symbol), "Primary");
    assert_eq!(
        typed
            .symbols
            .name(typed.symbols.get(conformance.symbol).parent),
        "root"
    );
    validate_program(&typed).expect("one named conformance should validate");
}

#[test]
fn duplicate_named_whole_trait_conformance_rejects() {
    let typed = typed_program_from_source(
        r#"
        trait Left {}
        trait Right {}
        data Item {}
        Primary: Item satisfies Left;
        Primary: Item satisfies Right;
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("a conformance path must be unique within its type");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("data `Item` declares conformance name `Primary` more than once")
    }));
}

#[test]
fn generic_conformance_bounds_survive_typing_and_resolve_exact_selection() {
    let typed = typed_program_from_source(
        r#"
        trait Marker {}
        trait Projection<Message> {}
        data Item {}
        Primary: Item satisfies Marker;

        machine inspect<T, Message>(value: &T)
        where
            T satisfies Projection<Message>,
            Message satisfies Item::Primary
        {}
        "#,
    );

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .expect("generic machine");
    let [ordinary, named] = machine.conformance_bounds.as_slice() else {
        panic!("two typed conformance bounds");
    };
    assert!(ordinary.subject.is_valid());
    assert_eq!(typed.symbols.name(ordinary.carrier), "Projection");
    assert_eq!(ordinary.arguments.len(), 1);
    assert!(named.subject.is_valid());
    let selected = named
        .selected_conformance_symbol()
        .expect("named conformance symbol");
    assert_eq!(typed.symbols.name(selected), "Primary");
    assert_eq!(
        typed.symbols.get(selected).kind,
        symbols::SymbolKind::Conformance
    );
    validate_program(&typed).expect("resolved conformance bounds should validate");
}

#[test]
fn generic_conformance_bound_rejects_unknown_subject() {
    let typed = typed_program_from_source(
        r#"
        trait Marker {}
        machine inspect<T>(value: &T)
        where U satisfies Marker
        {}
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("a bound subject must be a declared type parameter");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("conformance bound names unknown type parameter `U`")
    }));
}

#[test]
fn generic_conformance_bound_rejects_unknown_named_selection() {
    let typed = typed_program_from_source(
        r#"
        trait Marker {}
        data Item {}
        machine inspect<T>(value: &T)
        where T satisfies Item::Missing
        {}
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("a named bound must select a declared conformance");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("conformance bound selects unknown conformance `Item::Missing`")
    }));
}

#[test]
fn exact_requirement_edge_label_does_not_create_a_named_conformance() {
    let typed = typed_program_from_source(
        r#"
        trait Marker { machine Self::mark(&self); }
        data Item {}
        machine Item::mark(&self) satisfies Marker::mark as LocalMarker {}
        machine inspect<T>(value: &T)
        where T satisfies Item::LocalMarker
        {}
        "#,
    );

    assert!(typed.conformances().is_empty());
    let diagnostics = validate_program(&typed)
        .expect_err("an exact-edge grouping label cannot satisfy a whole-trait bound");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("conformance bound selects unknown conformance `Item::LocalMarker`")
    }));
}

#[test]
fn generic_trait_header_conformance_bound_survives_typing() {
    let typed = typed_program_from_source(
        r#"
        trait CallingPolicy {}
        trait Calling<C>
        where C satisfies CallingPolicy
        {}
        "#,
    );

    let trait_definition = typed
        .traits()
        .iter()
        .find(|trait_definition| trait_definition.name.as_str() == "Calling")
        .expect("generic trait");
    let [bound] = trait_definition.conformance_bounds.as_slice() else {
        panic!("one typed trait header bound");
    };
    assert!(bound.subject.is_valid());
    assert_eq!(typed.symbols.name(bound.carrier), "CallingPolicy");
    validate_program(&typed).expect("resolved trait header bound should validate");
}

#[test]
fn generic_trait_header_conformance_bound_rejects_unknown_subject() {
    let typed = typed_program_from_source(
        r#"
        trait CallingPolicy {}
        trait Calling<C>
        where Missing satisfies CallingPolicy
        {}
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a trait bound subject must be a declared type parameter");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("trait `Calling` conformance bound names unknown type parameter `Missing`")
    }));
}

#[test]
fn concrete_trait_application_discharge_header_conformance_bound() {
    let typed = typed_program_from_source(
        r#"
        trait CallingPolicy {}
        data Convention {}
        ConventionCallingPolicy: Convention satisfies CallingPolicy;

        trait Calling<C>
        where C satisfies CallingPolicy
        {}

        data Device {}
        DeviceCalling: Device satisfies Calling<Convention>;
        "#,
    );

    validate_program(&typed)
        .expect("the argument's nominal conformance should discharge the trait header bound");
}

#[test]
fn concrete_trait_application_rejects_unmet_header_conformance_bound() {
    let typed = typed_program_from_source(
        r#"
        trait CallingPolicy {}
        data Convention {}

        trait Calling<C>
        where C satisfies CallingPolicy
        {}

        data Device {}
        DeviceCalling: Device satisfies Calling<Convention>;
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a concrete trait argument must satisfy its header obligation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "conformance `Device satisfies Calling` does not meet trait `Calling` header obligation `C satisfies CallingPolicy` for argument `Convention`",
        )
    }));
}

#[test]
fn generic_trait_application_uses_enclosing_conformance_bound() {
    let typed = typed_program_from_source(
        r#"
        trait CallingPolicy {}
        trait Calling<C>
        where C satisfies CallingPolicy
        {}

        machine inspect<T, C>(value: &T)
        where
            C satisfies CallingPolicy,
            T satisfies Calling<C>
        {}
        "#,
    );

    validate_program(&typed)
        .expect("an enclosing generic bound should discharge the applied trait header bound");
}

#[test]
fn generic_trait_header_obligation_substitutes_nested_arguments() {
    let typed = typed_program_from_source(
        r#"
        data Envelope<T> {}
        trait PolicyFor<T> {}
        trait Routed<C, Message>
        where C satisfies PolicyFor<Envelope<Message>>
        {}

        machine inspect<T, C, Message>(value: &T)
        where
            C satisfies PolicyFor<Envelope<Message>>,
            T satisfies Routed<C, Message>
        {}
        "#,
    );

    validate_program(&typed)
        .expect("nested header arguments should substitute the applied trait parameters");
}

#[test]
fn generic_trait_header_exact_obligation_uses_enclosing_named_bound() {
    let typed = typed_program_from_source(
        r#"
        trait Marker {}
        data Item {}
        Primary: Item satisfies Marker;

        trait Selected<C>
        where C satisfies Item::Primary
        {}

        machine inspect<T, C>(value: &T)
        where
            C satisfies Item::Primary,
            T satisfies Selected<C>
        {}
        "#,
    );

    validate_program(&typed)
        .expect("the exact enclosing evidence should discharge an exact header obligation");
}

#[test]
fn generic_trait_application_rejects_missing_enclosing_conformance_bound() {
    let typed = typed_program_from_source(
        r#"
        trait CallingPolicy {}
        trait Calling<C>
        where C satisfies CallingPolicy
        {}

        machine inspect<T, C>(value: &T)
        where T satisfies Calling<C>
        {}
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("an applied generic trait must prove its header obligation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "machine `inspect` conformance bound `T satisfies Calling` does not meet trait `Calling` header obligation `C satisfies CallingPolicy` for argument `C`",
        )
    }));
}

#[test]
fn trait_parent_application_uses_child_header_bound() {
    let typed = typed_program_from_source(
        r#"
        trait CallingPolicy {}
        trait Calling<C>
        where C satisfies CallingPolicy
        {}

        trait Routed<C>: Calling<C>
        where C satisfies CallingPolicy
        {}
        "#,
    );

    validate_program(&typed)
        .expect("a child trait header bound should discharge its parent's header obligation");
}

#[test]
fn trait_parent_application_rejects_missing_child_header_bound() {
    let typed = typed_program_from_source(
        r#"
        trait CallingPolicy {}
        trait Calling<C>
        where C satisfies CallingPolicy
        {}

        trait Routed<C>: Calling<C> {}
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a parent application must prove its generic trait header obligation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "trait `Routed` parent `Calling` does not meet trait `Calling` header obligation `C satisfies CallingPolicy` for argument `C`",
        )
    }));
}

#[test]
fn generic_bound_authorizes_requirement_call_in_body() {
    let typed = typed_program_from_source(
        r#"
        trait Incrementable {
            machine increment(&mut self);
        }
        machine step<T>(subject: &mut T)
        where T satisfies Incrementable
        {
            subject.increment();
        }
        "#,
    );

    validate_program(&typed).expect("the bound trait requirement should authorize the call");
}

#[test]
fn generic_call_searches_all_conformance_bounds() {
    let typed = typed_program_from_source(
        r#"
        trait Observable {
            machine observe(&self) -> i32;
        }
        trait Incrementable {
            machine increment(&mut self);
        }
        machine step<T>(subject: &mut T)
        where T satisfies Observable, T satisfies Incrementable
        {
            subject.increment();
        }
        "#,
    );

    validate_program(&typed)
        .expect("a requirement on any declared bound should authorize the call");
}

#[test]
fn generic_bound_arguments_specialize_requirement_parameters() {
    let typed = typed_program_from_source(
        r#"
        trait Encoder<Message> {
            machine encode(&self, out: &mut Message);
        }
        machine encode<T, M>(subject: &T, out: &mut M)
        where T satisfies Encoder<M>
        {
            subject.encode(out);
        }
        "#,
    );

    validate_program(&typed)
        .expect("the bound argument M should instantiate Encoder's Message parameter");
}

#[test]
fn generic_bound_arguments_reject_wrong_requirement_argument_type() {
    let typed = typed_program_from_source(
        r#"
        trait Encoder<Message> {
            machine encode(&self, out: &mut Message);
        }
        data Expected {}
        data Wrong {}
        machine encode<T>(subject: &T, out: &mut Wrong)
        where T satisfies Encoder<Expected>
        {
            subject.encode(out);
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("the concrete bound argument must instantiate the requirement signature");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "argument `out` for bounded trait requirement `Encoder::encode` does not match",
        )
    }));
}

#[test]
fn generic_call_rejects_ambiguous_bound_requirements() {
    let typed = typed_program_from_source(
        r#"
        trait First {
            machine inspect(&self) -> i32;
        }
        trait Second {
            machine inspect(&self) -> i32;
        }
        machine inspect<T>(subject: &T) -> i32
        where T satisfies First, T satisfies Second
        {
            subject.inspect()
        }
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("same-named requirements need an exact selection");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("generic call `inspect` is ambiguous across its conformance bounds")
    }));
}

#[test]
fn generic_call_rejects_unconstrained_subject() {
    let typed = typed_program_from_source(
        r#"
        machine step<T>(subject: &mut T) {
            subject.increment();
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a call through an unconstrained generic parameter must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot call `increment` through unconstrained generic parameter `T`")
    }));
}

#[test]
fn generic_call_rejects_requirement_absent_from_bound_trait() {
    let typed = typed_program_from_source(
        r#"
        trait Incrementable {
            machine increment(&mut self);
        }
        machine step<T>(subject: &mut T)
        where T satisfies Incrementable
        {
            subject.reset();
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a bound only authorizes requirements in its trait surface");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("trait `Incrementable`, which has no requirement `reset`")
    }));
}

#[test]
fn named_generic_bound_authorizes_its_selected_trait_surface() {
    let typed = typed_program_from_source(
        r#"
        trait Ranked {
            machine rank(&self) -> i32;
        }
        data Card {}
        machine Card::rank(&self) -> i32 satisfies Ranked::rank { 1 }
        PowerOrder: Card satisfies Ranked;

        machine rank_selected<C>(card: &C) -> i32
        where C satisfies Card::PowerOrder
        {
            card.rank()
        }
        "#,
    );

    validate_program(&typed).expect("the exact named conformance should expose its trait surface");
}

#[test]
fn exact_qualification_may_publish_one_checked_content_projection() {
    let typed = typed_program_from_source(
        r#"
        data Unit {}
        data CountedQuantity<U> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }
        data Extent [linear] { base: u64; length: u64; }
        domain Extent::Granted;

        machine Granted::content(extent: &Extent) -> CountedQuantity<Unit>
        satisfies Content<CountedQuantity<Unit>>::project
        {
            CountedQuantity { magnitude: extent.length }
        }
        "#,
    );

    validate_program(&typed)
        .expect("one owner-attached checked content projection should validate");
}

#[test]
fn content_projection_rejects_non_owner_machine_home() {
    let typed = typed_program_from_source(
        r#"
        data Unit {}
        data CountedQuantity<U> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }
        data Extent [linear] { base: u64; length: u64; }
        domain Extent::Granted;

        machine Foreign::content(extent: &Extent) -> CountedQuantity<Unit>
        satisfies Content<CountedQuantity<Unit>>::project
        {
            CountedQuantity { magnitude: extent.length }
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a foreign machine home must not reinterpret another qualification");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("is not attached to the exact qualification it projects")
    }));
}

#[test]
fn exact_qualification_rejects_duplicate_content_projections() {
    let typed = typed_program_from_source(
        r#"
        data Unit {}
        data CountedQuantity<U> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }
        data Extent [linear] { base: u64; length: u64; }
        domain Extent::Granted;

        machine Granted::content(extent: &Extent) -> CountedQuantity<Unit>
        satisfies Content<CountedQuantity<Unit>>::project
        {
            CountedQuantity { magnitude: extent.length }
        }

        machine Granted::again(extent: &Extent) -> CountedQuantity<Unit>
        satisfies Content<CountedQuantity<Unit>>::project
        {
            CountedQuantity { magnitude: extent.length }
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("an exact qualification must publish only one content projection");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("publishes more than one `Content<A>` projection")
    }));
}

#[test]
fn content_projection_rejects_user_defined_algebra() {
    let typed = typed_program_from_source(
        r#"
        data Shape { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }
        data Extent [linear] { length: u64; }
        domain Extent::Granted;

        machine Granted::content(extent: &Extent) -> Shape
        satisfies Content<Shape>::project
        {
            Shape { magnitude: extent.length }
        }
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("content algebra vocabulary must remain closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("must select compiler-owned `IntervalSet<CoordinateSpace>`")
    }));
}

#[test]
fn content_projection_rejects_arbitrary_helper_call() {
    let typed = typed_program_from_source(
        r#"
        data Unit {}
        data CountedQuantity<U> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }
        data Extent [linear] { length: u64; }
        domain Extent::Granted;

        machine hidden(extent: &Extent) -> u64 {
            extent.length
        }

        machine Granted::content(extent: &Extent) -> CountedQuantity<Unit>
        satisfies Content<CountedQuantity<Unit>>::project
        {
            CountedQuantity { magnitude: hidden(extent) }
        }
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("an arbitrary helper call must not enter projection");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("is outside the closed projection fragment")
    }));
}

#[test]
fn content_projection_rejects_non_linear_carrier() {
    let typed = typed_program_from_source(
        r#"
        data Unit {}
        data CountedQuantity<U> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }
        data Counter { remaining: u64; }
        domain Counter::Available;

        machine Available::content(counter: &Counter) -> CountedQuantity<Unit>
        satisfies Content<CountedQuantity<Unit>>::project
        {
            CountedQuantity { magnitude: counter.remaining }
        }
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("content accounting must require a linear claim");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("carrier `Counter` is not linear")
            && diagnostic.message.contains("owned linear claims")
    }));
}

#[test]
fn boundary_requirement_may_authorize_its_exact_qualified_result() {
    let typed = typed_program_from_source(
        r#"
        data Token {}
        domain Token::Issued
        established by TokenIssuer::issue;

        boundary trait TokenIssuer {
            machine issue() -> Token
            ensures
                result in Token::Issued;
        }
        "#,
    );

    validate_program(&typed).expect("exact boundary result qualification should validate");
}

#[test]
fn boundary_requirement_cannot_admit_an_argument_qualification() {
    let typed = typed_program_from_source(
        r#"
        data Token {}
        domain Token::Issued;

        boundary trait TokenIssuer {
            machine issue(candidate: Token) -> Token
            ensures
                candidate in Token::Issued;
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a boundary requirement may authorize only its exact result");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("may admit domain `Token::Issued` only for its exact `result`")
    }));
}

#[test]
fn boundary_requirement_result_must_match_the_domain_carrier() {
    let typed = typed_program_from_source(
        r#"
        data Token {}
        data Receipt {}
        domain Token::Issued;

        boundary trait TokenIssuer {
            machine issue() -> Receipt
            ensures
                result in Token::Issued;
        }
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("the admitted result must use the domain carrier");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("result carrier `Receipt` does not match domain target `Token`")
    }));
}

#[test]
fn accepted_machine_cannot_directly_admit_domain_membership() {
    let typed = typed_program_from_source(
        r#"
        data Token {}
        domain Token::Issued;

        boundary machine issue() -> Token
        ensures
            result in Token::Issued;
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("direct accepted qualification must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("external machine `issue` cannot directly admit domain membership")
    }));
}

#[test]
fn declared_lifetime_parameters_survive_lowering_and_validate() {
    let typed = typed_program_from_source(
        r#"
        data View<'buf> {
            body: &'buf i32;
        }

        machine borrow<'call>(value: &'call i32) -> &'call i32 {
            value
        }
        "#,
    );

    let view = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "View")
        .expect("View");
    assert_eq!(view.lifetime_parameters.len(), 1);
    assert_eq!(view.lifetime_parameters[0].as_str(), "buf");

    let borrow = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "borrow")
        .expect("borrow");
    assert_eq!(borrow.lifetime_parameters.len(), 1);
    assert_eq!(borrow.lifetime_parameters[0].as_str(), "call");
    validate_program(&typed).expect("declared lifetime tags should validate");
}

#[test]
fn explicit_lifetime_arguments_survive_typed_lowering_and_validate() {
    let typed = typed_program_from_source(
        r#"
        data View<'buf, T> {
            body: &'buf T;
        }

        machine consume<'call>(value: View<'call, i32>) {}
        "#,
    );

    let consume = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "consume")
        .expect("consume");
    let entry = &typed.machine_states(consume)[0];
    let parameter = &typed.state_parameters(entry)[0];
    let typed_trees::types::TypeReferenceNode::Generic {
        base_name,
        lifetime_arguments,
        arguments,
        ..
    } = typed
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        panic!("data should retain an erased lifetime application");
    };
    assert_eq!(base_name.as_str(), "View");
    assert_eq!(lifetime_arguments[0].as_str(), "call");
    assert_eq!(
        typed
            .type_reference_table
            .type_reference_handles(*arguments)
            .len(),
        1
    );
    validate_program(&typed).expect("declared lifetime application should validate");
}

#[test]
fn undeclared_and_wrong_arity_lifetime_arguments_reject() {
    let typed = typed_program_from_source(
        r#"
        data Pair<'left, 'right> {
            left: &'left i32;
            right: &'right i32;
        }

        machine bad<'call>(value: Pair<'ghost>) {}
        "#,
    );
    let diagnostics =
        validate_program(&typed).expect_err("invalid lifetime application must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("uses undeclared lifetime argument `'ghost'")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("expected 2 lifetime arguments but got 1")
    }));
}

#[test]
fn undeclared_lifetime_tag_rejects() {
    let typed = typed_program_from_source(
        r#"
        data Bad {
            body: &'ghost i32;
        }
        "#,
    );
    let diagnostics = validate_program(&typed).expect_err("undeclared lifetime tag must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("uses undeclared lifetime `'ghost'")
    }));
}

#[test]
fn carry_policy_survives_lowering_and_derives_through_transparent_data() {
    let typed = typed_program_from_source(
        r#"
        data Inner { value: i32; }
        data Outer [carry(
            suspension: allowed,
            cpu: any,
            thread: any,
            address: movable,
        )] { inner: Inner; }
        "#,
    );
    let outer = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .expect("Outer");
    assert_eq!(
        outer.properties.carry,
        Some(language_semantics::CarryPolicy::PERMISSIVE)
    );
    validate_program(&typed).expect("transparent scalar aggregate should derive permissive carry");
}

#[test]
fn compiler_carry_permissions_are_subject_polymorphic_and_portable_is_canonical() {
    let typed = typed_program_from_source(
        r#"
        data Carrier {
            explicit: u64 in Carry::AcrossSuspend
                           & Carry::AnyCpu
                           & Carry::AnyThread
                           & Carry::MovableAddress;
            portable: u64 in Carry::Portable;
        }
        "#,
    );
    validate_program(&typed).expect("compiler-owned carry permissions classify any carrier");

    let carrier = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Carrier")
        .expect("Carrier");
    let fields = typed
        .data_members(carrier)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        typed.normalized_type_identity(fields[0].type_reference),
        typed.normalized_type_identity(fields[1].type_reference),
    );
}

#[test]
fn unknown_compiler_carry_permission_rejects_with_closed_vocabulary() {
    let typed = typed_program_from_source(
        r#"
        data Carrier {
            value: u64 in Carry::Anywhere;
        }
        "#,
    );
    let diagnostics =
        validate_program(&typed).expect_err("the compiler-owned carry vocabulary is closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("unknown compiler carry permission `Carry::Anywhere`")
    }));
}

#[test]
fn transparent_domain_alias_may_expand_compiler_carry_portable() {
    let typed = typed_program_from_source(
        r#"
        data Token {}
        domain Token::Portable = Carry::Portable;
        machine carry(token: Token in Token::Portable) -> Token {
            transition { _ -> token }
        }
        "#,
    );

    validate_program(&typed).expect("a declared alias may bundle compiler carry permissions");
    let carry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "carry")
        .expect("carry machine");
    let token = typed
        .state_parameters(
            typed
                .machine_states(carry)
                .first()
                .expect("carry entry state"),
        )
        .iter()
        .find(|parameter| parameter.name.as_str() == "token")
        .expect("token parameter");
    let typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } = typed
        .type_reference_table
        .type_reference(token.type_reference)
    else {
        panic!("portable alias should remain a constrained type");
    };
    let names = typed
        .type_reference_table
        .constraints(*constraints)
        .iter()
        .map(|constraint| match constraint {
            typed_trees::types::TypeConstraintNode::Domain(domain) => {
                domain.name.as_str().to_owned()
            }
            other => panic!("portable alias atom became {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        language_semantics::CarryPermission::ALL.map(|permission| permission.name().to_owned())
    );
}

#[test]
fn transparent_domain_alias_rejects_unknown_compiler_carry_atom() {
    let typed = typed_program_from_source(
        r#"
        data Token {}
        domain Token::Anywhere = Carry::Anywhere;
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("aliases cannot extend the compiler carry vocabulary");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("unknown compiler carry permission `Carry::Anywhere`")
    }));
}

#[test]
fn authored_carry_floor_rejects_a_stricter_stored_borrow() {
    let typed = typed_program_from_source(
        r#"
        data Bad [carry(
            suspension: allowed,
            cpu: any,
            thread: any,
            address: movable,
        )] { borrowed: &i32; }
        "#,
    );
    let diagnostics =
        validate_program(&typed).expect_err("borrow must fail permissive carry floor");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("declares carry policy")
            && diagnostic.message.contains("field `borrowed`")
    }));
}

#[test]
fn carry_bound_rejects_a_stricter_generic_argument() {
    let typed = typed_program_from_source(
        r#"
        data Envelope<T [carry(
            suspension: allowed,
            cpu: any,
            thread: any,
            address: movable,
        )]> { value: T; }
        data Borrowed { value: &i32; }
        data UsesEnvelope { value: Envelope<Borrowed>; }
        "#,
    );
    let diagnostics =
        validate_program(&typed).expect_err("borrow type must not satisfy permissive carry bound");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("type parameter `T") && diagnostic.message.contains("carry(")
    }));
}

#[test]
fn concrete_generic_carry_derives_from_substituted_arguments() {
    let typed = typed_program_from_source(
        r#"
        data Envelope<T> { value: T; }
        data Nested<T> { value: Envelope<T>; }
        data Concrete [carry(
            suspension: allowed,
            cpu: any,
            thread: any,
            address: movable,
        )] { value: Nested<i32>; }
        "#,
    );
    validate_program(&typed)
        .expect("a concrete transparent instantiation should derive from its argument");
}

#[test]
fn concrete_generic_carry_keeps_restrictive_substituted_arguments() {
    let typed = typed_program_from_source(
        r#"
        data Envelope<T> { value: T; }
        data Borrowed { value: &i32; }
        data Bad [carry(
            suspension: allowed,
            cpu: any,
            thread: any,
            address: movable,
        )] { value: Envelope<Borrowed>; }
        "#,
    );
    let diagnostics = validate_program(&typed)
        .expect_err("a concrete borrowed argument must remain carry-restrictive");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("data `Bad` declares carry policy")
            && diagnostic.message.contains("field `value`")
    }));
}

#[test]
fn quotient_rejects_an_authored_empty_equivalence_lookalike() {
    let typed = typed_program_from_source(
        r#"
        data Carrier {}
        proposition equivalent(a: Carrier, b: Carrier);
        trait Equivalence<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
        }
        FakeEquivalence: satisfies Equivalence<Carrier, equivalent> {}
        data Quotient = Carrier % equivalent
        where equivalent satisfies Equivalence<Carrier, equivalent> as FakeEquivalence;
        "#,
    );
    let diagnostics = validate_program(&typed)
        .expect_err("a local same-name empty trait must not license quotient formation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("sealed toolchain `Equivalence` declaration")
            && diagnostic.message.contains("authored lookalike")
    }));
}

#[test]
fn quotient_rejects_missing_named_equivalence_selection() {
    let typed = typed_program_from_source(
        r#"
        data Carrier { case Zero; case Next(prev: Carrier); }
        proposition equivalent(a: Carrier, b: Carrier);
        data Quotient = Carrier % equivalent;
        "#,
    );
    let diagnostics = validate_program(&typed).expect_err("the exact named selection is mandatory");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires one exact named `Equivalence` conformance")
    }));
}

#[test]
fn quotient_rejects_boolean_relation_even_when_its_shape_matches() {
    let typed = typed_program_from_source(
        r#"
        data Carrier { case Zero; case Next(prev: Carrier); }
        machine equivalent(a: Carrier, b: Carrier) -> bool {
            transition { _ -> (true) }
        }
        data Quotient = Carrier % equivalent;
        "#,
    );
    let diagnostics = validate_program(&typed)
        .expect_err("an executable decision machine is not a quotient proposition identity");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("must resolve to one exact proposition family")
    }));
}

#[test]
fn quotient_construction_is_carrier_only() {
    let typed = typed_program_from_source(
        r#"
        data Carrier { case Zero; case Next(prev: Carrier); }
        machine equivalent(a: Carrier, b: Carrier) -> bool {
            transition { _ -> (true) }
        }
        machine equivalent_reflexive(a: Carrier)
        ensures equivalent(a, a) { }
        machine equivalent_symmetric(a: Carrier, b: Carrier)
        requires equivalent(a, b);
        ensures equivalent(b, a) { }
        machine equivalent_transitive(a: Carrier, b: Carrier, c: Carrier)
        requires equivalent(a, b), equivalent(b, c)
        ensures equivalent(a, c) { }
        data Quotient = Carrier % equivalent;
        machine bad() -> Quotient {
            transition { _ -> (42 as Quotient) }
        }
        "#,
    );
    let diagnostics = validate_program(&typed).expect_err("non-carrier mint must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("quotient construction is carrier-only")
    }));
}

#[test]
fn quotient_equality_requires_the_declared_relation_fact() {
    let typed = typed_program_from_source(
        r#"
        data Carrier { case Zero; case Next(prev: Carrier); }
        machine equivalent(a: Carrier, b: Carrier) -> bool {
            transition { _ -> (true) }
        }
        machine equivalent_reflexive(a: Carrier)
        ensures equivalent(a, a) { }
        machine equivalent_symmetric(a: Carrier, b: Carrier)
        requires equivalent(a, b);
        ensures equivalent(b, a) { }
        machine equivalent_transitive(a: Carrier, b: Carrier, c: Carrier)
        requires equivalent(a, b), equivalent(b, c)
        ensures equivalent(a, c) { }
        data Quotient = Carrier % equivalent;
        machine bad_congruence(a: Carrier, b: Carrier)
        ensures (a as Quotient) == (b as Quotient)
        {
        }
        "#,
    );
    let diagnostics = validate_program(&typed).expect_err("unrelated representatives must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove quotient equality")
            && diagnostic
                .message
                .contains("equivalent(left_carrier, right_carrier)")
    }));
}

#[test]
fn static_machine_argument_refines_authored_generic_contract() {
    let typed = typed_program_from_source(
        r#"
        data Card {}
        machine Card::power(value: &Card) {}

        machine map<T, machine F>(value: &T)
        where machine F(value: &T)
        {}

        machine caller(card: &Card) {
            map<Card::power>(card);
        }
        "#,
    );
    validate_program(&typed).expect("matching static machine contract should validate");
}

#[test]
fn static_machine_argument_need_not_republish_a_tautological_postcondition() {
    let typed = typed_program_from_source(
        r#"
        data Token {}

        machine selected(value: &Token) {}

        machine invoke<machine F>(value: &Token)
        where machine F(value: &Token) ensures true;
        {}

        machine caller(value: &Token) {
            invoke<selected>(value);
        }
        "#,
    );

    validate_static_machine_selections(&typed)
        .expect("`ensures true` is the postcondition identity");
}

#[test]
fn static_machine_argument_requires_a_published_termination_guarantee() {
    let typed = typed_program_from_source(
        r#"
        machine selected(value: u64) -> u64
        terminates by value;
        {
            transition value > 0 {
                true -> selected(value - 1)
                false -> 0
            }
        }

        machine invoke<machine F>(value: u64) -> u64
        where machine F(value: u64) -> u64 terminates;
        {
            F(value)
        }

        machine caller(value: u64) -> u64 {
            invoke<selected>(value)
        }
        "#,
    );

    let diagnostics = validate_static_machine_selections(&typed)
        .expect_err("a private ranking witness must not publish a termination promise");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("the requirement guarantees termination")
    }));
}

#[test]
fn static_machine_argument_accepts_a_published_termination_guarantee() {
    let typed = typed_program_from_source(
        r#"
        machine selected(value: u64) -> u64
        terminates;
        terminates by value;
        {
            transition value > 0 {
                true -> selected(value - 1)
                false -> 0
            }
        }

        machine invoke<machine F>(value: u64) -> u64
        where machine F(value: u64) -> u64 terminates;
        {
            F(value)
        }

        machine caller(value: u64) -> u64 {
            invoke<selected>(value)
        }
        "#,
    );

    validate_static_machine_selections(&typed)
        .expect("an explicit published termination guarantee should satisfy the slot");
}

#[test]
fn static_machine_argument_rejects_inferred_suspension_above_slot_ceiling() {
    let typed = typed_program_from_source(
        r#"
        data Token {}
        boundary machine suspend_source(value: &Token) suspends;

        machine work(value: &Token) {
            suspend suspend_source(value);
        }

        machine invoke<machine F>(value: &Token)
        where machine F(value: &Token)
        {}

        machine caller(value: &Token) {
            invoke<work>(value);
        }
        "#,
    );

    let diagnostics = validate_static_machine_selections(&typed)
        .expect_err("an inferred suspending provider must not widen a negative slot ceiling");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("may suspend")
            && diagnostic.message.contains("requirement omits `suspends;`")
    }));
}

#[test]
fn static_machine_argument_rejects_blocking_independently_from_suspension() {
    let typed = typed_program_from_source(
        r#"
        data Token {}
        boundary machine blocking_source(value: &Token) blocks;

        machine work(value: &Token) {
            block blocking_source(value);
        }

        machine invoke<machine F>(value: &Token)
        where machine F(value: &Token) suspends;
        {}

        machine caller(value: &Token) {
            invoke<work>(value);
        }
        "#,
    );

    let diagnostics = validate_static_machine_selections(&typed)
        .expect_err("allowing suspension must not silently allow blocking");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("may block")
            && diagnostic.message.contains("requirement omits `blocks;`")
    }));
    assert!(!diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("may suspend")
            && diagnostic.message.contains("requirement omits `suspends;`")
    }));
}

#[test]
fn static_machine_argument_admits_provider_within_both_operational_ceilings() {
    let typed = typed_program_from_source(
        r#"
        data Token {}
        boundary machine waiting_source(value: &Token) suspends; blocks;

        machine work(value: &Token) {
            suspend block waiting_source(value);
        }

        machine invoke<machine F>(value: &Token)
        where machine F(value: &Token) suspends; blocks;
        {}

        machine caller(value: &Token) {
            invoke<work>(value);
        }
        "#,
    );

    validate_static_machine_selections(&typed)
        .expect("a provider within both pinned axes should validate");
}

#[test]
fn static_machine_argument_rejects_symbol_resolved_service_widening() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Readable {
            machine read();
        }

        boundary trait Queryable {
            machine query();
        }

        machine work(service: &mut Queryable)
        reaches Queryable
        {}

        machine invoke<machine F>(service: &mut Queryable)
        where machine F(service: &mut Queryable) reaches Readable;
        {}

        machine caller(service: &mut Queryable) {
            invoke<work>(service);
        }
        "#,
    );

    let diagnostics = validate_static_machine_selections(&typed)
        .expect_err("a provider must not widen a symbol-resolved service ceiling");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("service reach `Queryable` exceeds its authored ceiling")
    }));
}

#[test]
fn static_machine_argument_admits_service_reach_within_parent_closure() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Readable {
            machine read();
        }

        boundary trait Filesystem: Readable {
            machine inspect();
        }

        machine work(service: &mut Filesystem) {
            service.read();
        }

        machine invoke<machine F>(service: &mut Filesystem)
        where machine F(service: &mut Filesystem)
        reaches Filesystem
        invokes service;
        {}

        machine caller(service: &mut Filesystem) {
            invoke<work>(service);
        }
        "#,
    );

    validate_static_machine_selections(&typed)
        .expect("a child service ceiling includes its boundary-service parents");
}

#[test]
fn recursive_private_operational_inference_reaches_independent_fixed_points() {
    let typed = typed_program_from_source(
        r#"
        boundary trait WaitingSource {
            machine wait() suspends; blocks;
        }

        data A {}
        data B {}

        machine A::step(&mut self, b: &mut B, source: &mut WaitingSource) {
            b.step(self, source);
        }

        machine B::step(&mut self, a: &mut A, source: &mut WaitingSource) {
            a.step(self, source);
            suspend block source.wait();
        }
        "#,
    );
    let operations = flow_effects::infer_operational_may(&typed);

    for name in ["A::step", "B::step"] {
        let symbol = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .symbol;
        let summary = operations
            .machines()
            .iter()
            .find(|summary| summary.symbol == symbol)
            .unwrap_or_else(|| panic!("operational summary for {name}"));
        assert!(summary.body_may_suspend, "{name} should infer suspension");
        assert!(summary.body_may_block, "{name} should infer blocking");
        assert!(summary.transitive_may_suspend);
        assert!(summary.transitive_may_block);
    }
}

#[test]
fn published_operational_omission_is_a_negative_ceiling() {
    let typed = typed_program_from_source(
        r#"
        boundary trait WaitingSource {
            machine wait() suspends; blocks;
        }

        data Published { source: WaitingSource; }
        boundary machine Published::entry(&mut self) {
            suspend block self.source.wait();
        }
        "#,
    );
    let operations = flow_effects::infer_operational_may(&typed);
    let diagnostics = validate_behavior_plan(&typed, &operations)
        .expect_err("published omission must reject inferred operational behavior");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("omits `suspends;`")
            && diagnostic.message.contains("body may suspend")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("omits `blocks;`")
            && diagnostic.message.contains("body may block")
    }));
}

#[test]
fn ordinary_public_machine_reach_omission_is_a_negative_ceiling() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Host {
            machine ping() reaches Host;
        }

        pub machine public_api() {
            Host::ping();
        }
        "#,
    );
    let operations = flow_effects::infer_operational_may(&typed);
    let diagnostics = validate_behavior_plan(&typed, &operations)
        .expect_err("ordinary public omission must reject inferred service reach");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("public_api")
            && diagnostic.message.contains("undeclared service")
            && diagnostic.message.contains("Host")
    }));
}

#[test]
fn checked_provider_rejects_undeclared_synchronous_invocation() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Handler {
            machine handle();
        }

        boundary trait EventSource {
            machine fire(handler: &mut Handler);
        }

        data EventProvider {}

        machine EventProvider::fire_checked(handler: &mut Handler)
        satisfies EventSource::fire
        {
            handler.handle();
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a checked provider may not hide a synchronous callback edge");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not refine `EventSource::fire`")
            && diagnostic.message.contains("omits `invokes handler;`")
    }));
}

#[test]
fn published_machine_rejects_omitted_synchronous_invocation() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Handler {
            machine handle();
        }

        data Published {}
        boundary machine Published::entry(&mut self, handler: &mut Handler) {
            handler.handle();
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("a published machine may not omit an inferred synchronous edge");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("Published::entry")
            && diagnostic.message.contains("omits `invokes handler;`")
    }));
}

#[test]
fn invocation_declarations_reject_unknown_and_duplicate_bindings() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Handler {
            machine handle();
        }

        boundary trait EventSource {
            machine fire(handler: &mut Handler)
            invokes missing;
            invokes handler;
            invokes handler;
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("invocation declarations must name unique boundary bindings");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("`missing` is neither one of its boundary-binding parameters")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("declares `invokes handler;` more than once")
    }));
}

#[test]
fn checked_provider_infers_declared_invocation_through_local_helper() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Handler {
            machine handle();
        }

        boundary trait EventSource {
            machine fire(handler: &mut Handler)
            invokes handler;
        }

        machine forward(handler: &mut Handler) {
            handler.handle();
        }

        data EventProvider {}

        machine EventProvider::fire_checked(handler: &mut Handler)
        satisfies EventSource::fire
        {
            forward(handler);
        }
        "#,
    );

    validate_program(&typed)
        .expect("the declared callback ceiling admits the helper-forwarded direct edge");
    let plan = flow_effects::infer_synchronous_invocations(&typed);
    let provider = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "EventProvider::fire_checked")
        .expect("provider machine");
    assert_eq!(
        plan.for_machine(provider.symbol)
            .expect("provider invocation summary")
            .inferred_transitive,
        vec![flow_effects::InvocationTarget::Parameter(0)]
    );
}

#[test]
fn checked_provider_normalizes_self_forwarded_receiver_before_refinement() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Handler {
            machine handle();
        }

        boundary trait EventSource {
            machine fire(handler: &mut Handler)
            invokes handler;
        }

        machine forward(handler: &mut Handler) {
            handler.handle();
        }

        data EventProvider {}

        machine EventProvider::fire_checked(source: EventSource, handler: &mut Handler)
        satisfies EventSource::fire
        {
            forward(handler);
        }
        "#,
    );

    validate_program(&typed).expect(
        "composition removes the forwarded provider receiver and shifts the callback to requirement parameter 0",
    );
    let plan = flow_effects::infer_synchronous_invocations(&typed);
    let provider = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "EventProvider::fire_checked")
        .expect("provider machine");
    assert_eq!(
        plan.for_machine(provider.symbol)
            .expect("provider invocation summary")
            .inferred_transitive,
        vec![flow_effects::InvocationTarget::Parameter(1)]
    );
}

#[test]
fn checked_provider_infers_attached_boundary_field_as_direct_target() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Alpha {
            machine alpha() invokes Beta;
        }
        boundary trait Beta {
            machine beta() invokes Alpha;
        }
        data AlphaProvider { beta: Beta; }
        machine AlphaProvider::alpha_checked(&mut self) satisfies Alpha::alpha {
            self.beta.beta();
        }
        data BetaProvider { alpha: Alpha; }
        machine BetaProvider::beta_checked(&mut self) satisfies Beta::beta {
            self.alpha.alpha();
        }
        "#,
    );
    let plan = flow_effects::infer_synchronous_invocations(&typed);
    let labels = |name: &str| {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("checked provider");
        plan.for_machine(machine.symbol)
            .expect("invocation summary")
            .inferred_direct
            .iter()
            .map(|target| flow_effects::invocation_target_label(&typed, machine, *target))
            .collect::<Vec<_>>()
    };
    assert_eq!(labels("AlphaProvider::alpha_checked"), ["Beta"]);
    assert_eq!(labels("BetaProvider::beta_checked"), ["Alpha"]);
}

#[test]
fn invocation_inference_does_not_alias_same_named_foreign_fields_to_self() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Alpha {
            machine alpha();
        }
        boundary trait Beta {
            machine beta();
        }
        data Other { binding: Beta; }
        data Provider { binding: Alpha; }
        machine Provider::run(&self, other: &Other) {
            other.binding.beta();
        }
        "#,
    );
    let plan = flow_effects::infer_synchronous_invocations(&typed);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Provider::run")
        .expect("provider machine");
    let labels = plan
        .for_machine(machine.symbol)
        .expect("invocation summary")
        .inferred_direct
        .iter()
        .map(|target| flow_effects::invocation_target_label(&typed, machine, *target))
        .collect::<Vec<_>>();
    assert_eq!(labels, ["Beta"]);
}

#[test]
fn invocation_and_operational_inference_visit_transition_arguments() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Handler {
            machine value() -> i32
            blocks;
        }

        machine run(handler: &mut Handler) {
            transition { _ -> done(handler.value()) }

            state done(value: i32) {}
        }
        "#,
    );
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .expect("run machine");
    let invocations = flow_effects::infer_synchronous_invocations(&typed);
    assert_eq!(
        invocations
            .for_machine(machine.symbol)
            .expect("run invocation summary")
            .inferred_transitive,
        vec![flow_effects::InvocationTarget::Parameter(0)]
    );
    let operations = flow_effects::infer_operational_may(&typed);
    let operational = operations
        .machines()
        .iter()
        .find(|summary| summary.symbol == machine.symbol)
        .expect("run operational summary");
    assert!(operational.body_may_block);
}

#[test]
fn static_machine_argument_rejects_callable_shape_mismatch() {
    let typed = typed_program_from_source(
        r#"
        data Card {}
        machine Card::power(value: u64) {}

        machine map<T, machine F>(value: &T)
        where machine F(value: &T)
        {}

        machine caller(card: &Card) {
            map<Card::power>(card);
        }
        "#,
    );
    let diagnostics = validate_program(&typed).expect_err("shape mismatch must fail");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("parameter 0 expects `&T`, got `u64`")
    }));
}

#[test]
fn generic_call_rejects_missing_static_machine_argument() {
    let typed = typed_program_from_source(
        r#"
        data Card {}

        machine map<T, machine F>(value: &T)
        where machine F(value: &T)
        {}

        machine caller(card: &Card) {
            map(card);
        }
        "#,
    );
    let diagnostics = validate_program(&typed).expect_err("missing selection must fail");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires 1 static machine argument(s), got 0")
    }));
}

#[test]
fn static_machine_argument_rejects_stronger_precondition() {
    let typed = typed_program_from_source(
        r#"
        data Card {}
        machine Card::power(value: &Card)
        requires value == value
        {}

        machine map<T, machine F>(value: &T)
        where machine F(value: &T)
        {}

        machine caller(card: &Card) {
            map<Card::power>(card);
        }
        "#,
    );
    let diagnostics = validate_program(&typed).expect_err("stronger precondition must fail");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires facts are not a conservative refinement")
    }));
}

#[test]
fn generic_body_call_is_checked_against_machine_parameter_contract() {
    let typed = typed_program_from_source(
        r#"
        data Card {}

        machine apply<T, machine F>(value: &T)
        where machine F(item: &T)
        {
            F(value);
        }
        "#,
    );
    validate_program(&typed).expect("generic body should use the authored callable contract");
}

#[test]
fn higher_order_machine_parameter_refines_and_forwards_distinct_schema() {
    let typed = typed_program_from_source(
        r#"
        data Index {
            case Zero;
            case Next(previous: Index);
        }

        data Stream<machine S>
        where machine S(index: Index) -> Index;
        {
            case Empty;
            case More(sample: Index, tail: Stream<S>);
        }

        boundary machine sample(index: Index) -> Index;

        machine identity_schema<machine Chosen>(value: Stream<Chosen>) -> Stream<Chosen>
        where machine Chosen(index: Index) -> Index;
        {
            value
        }

        machine forward_schema<machine Schema, machine Selected>(value: Stream<Selected>) -> Stream<Selected>
        where machine Schema<machine Inner>(value: Stream<Inner>) -> Stream<Inner>
        where machine Inner(index: Index) -> Index;
        where machine Selected(index: Index) -> Index;
        {
            Schema<Selected>(value)
        }

        machine accepts_concrete(value: Stream<sample>) -> Stream<sample> {
            forward_schema<identity_schema, sample>(value)
        }
        "#,
    );

    let forward = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward_schema")
        .expect("forward_schema");
    let selected = typed
        .machine_type_parameters(forward)
        .iter()
        .find(|parameter| parameter.name.as_str() == "Selected")
        .expect("Selected");
    let call = typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "Schema" =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("Schema<Selected> call");
    assert_eq!(call.machine_arguments[0].symbol, selected.symbol);
    validate_static_machine_selections(&typed)
        .expect("higher-order static schema should refine and validate");
}

#[test]
fn nested_machine_argument_rejects_before_uninstantiated_shape_can_be_used() {
    let typed = typed_program_from_source(
        r#"
        boundary machine sample(value: u64) -> u64;

        machine inspect<machine Operation>() -> u64
        where machine Operation<machine Inner>(value: u64) -> u64
        where machine Inner(value: u64) -> u64;
        {
            0
        }

        machine identity<machine Selected>(value: u64) -> u64
        where machine Selected(value: u64) -> u64;
        {
            value
        }

        machine use() -> u64 {
            inspect<identity<sample>>()
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("an applied machine family must not validate as its generic base declaration");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("nested machine application; recursive specialization identity")
    }));
}

#[test]
fn generic_body_call_rejects_argument_outside_machine_parameter_contract() {
    let typed = typed_program_from_source(
        r#"
        data Card {}

        machine apply<T, machine F>(value: &T)
        where machine F(item: &T)
        {
            F(7);
        }
        "#,
    );
    let diagnostics =
        validate_program(&typed).expect_err("generic body call shape must be checked");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("argument `item`")
            && diagnostic.message.contains("expects `&T`")
    }));
}

#[test]
fn generic_machine_signature_may_forward_type_parameters_into_generic_data() {
    let typed = typed_program_from_source(
        r#"
        data Outcome<T> {
            case Empty;
            case Returned(value: T);
        }

        machine wrap<T>(value: T) -> Outcome<T> {
            Outcome::Returned { value: value }
        }
        "#,
    );

    validate_program(&typed)
        .expect("an in-scope machine type parameter should remain valid inside generic data");
}

#[test]
fn machine_parameterized_data_must_be_proof_only() {
    let typed = typed_program_from_source(
        r#"
        data RuntimeCallback<machine F>
        where machine F(value: u64) -> u64;
        {
            value: u64;
        }
        "#,
    );

    let diagnostics = validate_program(&typed)
        .expect_err("finite-layout data must not be indexed by a static machine");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("parameterized by a static machine but has a runtime layout")
    }));
}

#[test]
fn machine_parameter_cannot_be_stored_as_a_data_field() {
    let typed = typed_program_from_source(
        r#"
        data ProofFamily<machine F>
        where machine F(value: u64) -> u64;
        {
            case End;
            case More(callback: F, tail: ProofFamily<F>);
        }
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("a static machine parameter is not a value type");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("uses machine parameter `F` as a stored/runtime type")
    }));
}

#[test]
fn validates_main_entry_surface_from_source_pipeline() {
    let source = r#"
    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    assert_eq!(typed.machines().len(), 1);
    assert_eq!(typed.machines()[0].name.as_str(), "Main::main");
    assert_eq!(typed.machine_states(&typed.machines()[0]).len(), 1);
    assert_eq!(
        typed.machine_states(&typed.machines()[0])[0].name.as_str(),
        "main"
    );
    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn validates_local_state_call_arguments_from_source_pipeline() {
    let source = r#"
    data Main {
    }

    machine Main::main(&mut self) {
        take_non_negative(0);

        state take_non_negative(
            &mut self,
            value: u32[exact, non_negative]
        ) {
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let entry = typed
        .machine_states(&typed.machines()[0])
        .iter()
        .find(|state| state.name.as_str() == "main")
        .expect("entry state");
    let call_argument_count = typed
        .statement_table
        .statements(entry.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::Call(call) => Some(call.arguments.len()),
            typed_trees::statement::StatementNode::Expression(expression) => {
                let typed_trees::expression::ExpressionNode::Call(call) =
                    typed.expression_table.expression(*expression)
                else {
                    return None;
                };
                Some(call.arguments.len())
            }
            _ => None,
        })
        .expect("expected call statement");
    assert_eq!(call_argument_count, 1);
    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn rejects_unknown_trait_machine_service_reaches() {
    let source = r#"
    boundary trait Console {
        machine write_line(text: &[u8])
        reaches
            stdoutish;
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostics = lower_syntax_trees(&syntax_trees)
        .expect_err("unknown service reach must not enter resolved trees");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("unknown boundary service `stdoutish`")),
        "expected unknown service diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_unknown_domain_membership_in_domain_body() {
    let source = r#"
    data Player {
    }

    domain Player::Alive
    requires
        self in Player::Valid

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject domain");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("domain `Player::Alive` references unknown domain `Player::Valid`")),
        "expected unknown domain diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn repeated_normalized_domain_name_requires_equal_semantics() {
    let matching = typed_program_from_source(
        r#"
        domain [u8; 8]::Utf8
        requires
            valid_utf8(self);
        domain [u8; 16]::Utf8
        requires
            valid_utf8(self);
        "#,
    );
    validate_program(&matching)
        .expect("equal capacity specializations should share their normalized domain name");

    let divergent = typed_program_from_source(
        r#"
        domain [u8; 8]::Utf8
        requires
            valid_utf8(self);
        domain [u8; 16]::Utf8
        requires
            no_nul(self);
        "#,
    );
    let diagnostics =
        validate_program(&divergent).expect_err("different semantics under one name must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("declared more than once with different normalized semantics")
    }));
}

#[test]
fn domain_constraints_resolve_same_short_name_by_carrier() {
    let typed = typed_program_from_source(
        r#"
        data Holder {
            signed: i64 in Tagged;
            unsigned: u64 in Tagged;
        }
        domain i64::Tagged;
        domain u64::Tagged
        requires
            self >= 0;
        "#,
    );
    validate_program(&typed)
        .expect("each short domain name should retain its carrier-specific identity");
}

#[test]
fn declared_domain_constraint_with_missing_normalized_identity_fails_closed() {
    let mut typed = typed_program_from_source(
        r#"
        data Holder { value: i64 in Tagged; }
        domain i64::Tagged;
        "#,
    );
    let (_, constraints) = typed
        .type_reference_table
        .constrained_type_references()
        .into_iter()
        .find(|(_, constraints)| {
            typed
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| {
                    matches!(
                        constraint,
                        typed_trees::types::TypeConstraintNode::Domain(_)
                    )
                })
        })
        .expect("declared domain constraint");
    let domain = typed
        .type_reference_table
        .constraints_mut(constraints)
        .iter_mut()
        .find_map(|constraint| match constraint {
            typed_trees::types::TypeConstraintNode::Domain(domain) => Some(domain),
            _ => None,
        })
        .expect("domain constraint");
    domain.symbol = symbols::SymbolHandle::invalid();

    let diagnostics = validate_program(&typed)
        .expect_err("a declared-domain constraint cannot fall back to global name lookup");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("no `domain` named `Tagged` is declared")
    }));
}

#[test]
fn empty_domain_explicit_as_requires_neither_predicate_nor_user_satisfier() {
    let typed = typed_program_from_source(
        r#"
        domain i64::Km;

        machine qualify(raw: i64) {
            let distance: i64 = raw as i64 in Km;
        }
        "#,
    );
    validate_program(&typed)
        .expect("an empty domain is explicitly and vacuously qualified by compiler-derived `as`");
}

#[test]
fn distinct_semantic_roles_compose_in_one_domain_chain() {
    let typed = typed_program_from_source(
        r#"
        domain i32::Degrees;

        operator + i32::Degrees::add(left: i32, right: i32) -> i32;

        data Holder {
            value: i32 in Degrees & Wrapping;
        }
        "#,
    );
    validate_program(&typed)
        .expect("a denotation/dimension domain and an arithmetic policy should compose");
}

#[test]
fn duplicate_denotation_role_contributions_are_rejected() {
    let typed = typed_program_from_source(
        r#"
        domain i32::Degrees;

        operator i32::Degrees::add(left: i32, right: i32) -> i32;

        domain i32::Radians;

        operator i32::Radians::combine(left: i32, right: i32) -> i32;

        data Holder {
            value: i32 in Degrees & Radians;
        }
        "#,
    );
    let diagnostics =
        validate_program(&typed).expect_err("one domain chain cannot select two denotations");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("conflicting `denotation_dimension` semantic-role contributions")
    }));
}

fn validate_contract_source(source: &str) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");
    validate_program(&typed)
}

#[test]
fn boundary_out_param_ensures_seeds_the_value_env() {
    // R4 witness mint: `fw.get_size(&mut self.n)` with `ensures size <= 8`
    // leaves `self.n` in [0, 8], so the exact-overflow proof on `n + 1`
    // discharges without a guard.
    validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 8;
    }
    data Main { fw: Firmware; n: u32; m: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        self.m = self.n + 1;
    }
    "#,
    )
    .expect("the ensures-seeded bound should discharge the exact-overflow proof");
}

#[test]
fn boundary_out_param_without_ensures_stays_unproven() {
    // The negative rail: the same shape without the ensures keeps the
    // decision-17 refusal (the call clears the env; nothing re-seeds).
    let diagnostics = validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32);
    }
    data Main { fw: Firmware; n: u32; m: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        self.m = self.n + 1;
    }
    "#,
    )
    .expect_err("without the ensures the addition must stay unproven");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("may overflow `u32`")),
        "expected the exact-arithmetic refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_out_param_ensures_dies_at_the_next_write() {
    // Env facts are flow-scoped: writing the place kills the seeded bound.
    let diagnostics = validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 8;
    }
    data Main { fw: Firmware; n: u32; other: u32; m: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        self.n = self.other;
        self.m = self.n + 1;
    }
    "#,
    )
    .expect_err("the rebound place must lose the ensures fact");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("may overflow `u32`")),
        "expected an exact-arithmetic refusal after the rebind, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_ensures_witness_discharges_recast_footprint() {
    // R4 recast route (the M2 memory-map mini-shape): `ensures size <= 8`
    // on the boundary out-param bounds the transition argument, so the
    // callee's recast footprint `8 + size(u32) <= 12` proves with no guard
    // and no declared range on `off`.
    validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 8;
    }
    data Main { fw: Firmware; buf: [u8; 12]; n: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        transition { _ -> read(self.n) }
        state read(&mut self, off: u32) {
            let v: &u32 = &self.buf[off] as &u32;
        }
    }
    "#,
    )
    .expect("the ensures witness should discharge the recast footprint");
}

#[test]
fn boundary_ensures_witness_too_wide_refuses_recast_footprint() {
    // `ensures size <= 9` admits offset 9: a 4-byte view reads bytes 9..13
    // of a 12-byte region -- refused with the footprint named.
    let diagnostics = validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 9;
    }
    data Main { fw: Firmware; buf: [u8; 12]; n: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        transition { _ -> read(self.n) }
        state read(&mut self, off: u32) {
            let v: &u32 = &self.buf[off] as &u32;
        }
    }
    "#,
    )
    .expect_err("a bound past the footprint must refuse");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("would read past the buffer")),
        "expected the footprint refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_ensures_witness_survives_unrelated_intervening_call() {
    // The boundary receiver and the caller's `n` field are distinct places.
    // A second call that receives neither `n` nor its owner cannot erase the
    // first call's established bound.
    validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 8;
        machine poke();
    }
    data Main { fw: Firmware; buf: [u8; 12]; n: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        self.fw.poke();
        transition { _ -> read(self.n) }
        state read(&mut self, off: u32) {
            let v: &u32 = &self.buf[off] as &u32;
        }
    }
    "#,
    )
    .expect("an unreachable caller field must retain its witness");
}

#[test]
fn boundary_ensures_witness_dies_when_intervening_call_borrows_place_mutably() {
    let diagnostics = validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 8;
        machine poke(size: &mut u32);
    }
    data Main { fw: Firmware; buf: [u8; 12]; n: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        self.fw.poke(&mut self.n);
        transition { _ -> read(self.n) }
        state read(&mut self, off: u32) {
            let v: &u32 = &self.buf[off] as &u32;
        }
    }
    "#,
    )
    .expect_err("an explicit exclusive argument must kill the witness");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("recast")),
        "expected a recast refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_ensures_witness_survives_unrelated_internal_call() {
    validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 8;
    }
    data Main { fw: Firmware; buf: [u8; 12]; n: u32; other: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        self.touch_other();
        transition { _ -> read(self.n) }
        state read(&mut self, off: u32) {
            let v: &u32 = &self.buf[off] as &u32;
        }
    }
    machine Main::touch_other(&mut self) {
        self.other = 1;
    }
    "#,
    )
    .expect("an internal call with a disjoint may-write set must preserve the witness");
}

#[test]
fn boundary_ensures_witness_dies_when_internal_call_writes_place() {
    let diagnostics = validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_size(size: &mut u32)
        ensures size <= 8;
    }
    data Main { fw: Firmware; buf: [u8; 12]; n: u32; }
    machine Main::main(&mut self) {
        self.fw.get_size(&mut self.n);
        self.touch_n();
        transition { _ -> read(self.n) }
        state read(&mut self, off: u32) {
            let v: &u32 = &self.buf[off] as &u32;
        }
    }
    machine Main::touch_n(&mut self) {
        self.n = 1;
    }
    "#,
    )
    .expect_err("an internal may-write must invalidate the witness");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("recast")),
        "expected a recast refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_survives_transitive_disjoint_boundary_frame() {
    validate_contract_source(
        r#"
    boundary trait Sensor {
        machine sample(value: &mut u32)
        ensures value <= 8;
    }
    boundary trait Device {
        machine ping();
    }
    data Worker { device: Device; }
    machine Worker::touch_device(&mut self) {
        self.device.ping();
    }
    data Main { sensor: Sensor; worker: Worker; n: u32; buf: [u8; 12]; }
    machine Main::main(&mut self) {
        self.sensor.sample(&mut self.n);
        self.worker.touch_device();
        transition { _ -> read(self.n) }
        state read(&mut self, off: u32) {
            let value: &u32 = &self.buf[off] as &u32;
        }
    }
    "#,
    )
    .expect("an internal wrapper's exact nested boundary frame must preserve a disjoint witness");
}

#[test]
fn boundary_witness_dies_through_transitive_boundary_out_argument() {
    let diagnostics = validate_contract_source(
        r#"
    boundary trait Sensor {
        machine sample(value: &mut u32)
        ensures value <= 8;
    }
    boundary trait Device {
        machine overwrite(value: &mut u32);
    }
    data Worker { device: Device; }
    machine Worker::overwrite_external(&mut self, value: &mut u32) {
        self.device.overwrite(value);
    }
    data Main { sensor: Sensor; worker: Worker; n: u32; buf: [u8; 12]; }
    machine Main::main(&mut self) {
        self.sensor.sample(&mut self.n);
        self.worker.overwrite_external(&mut self.n);
        transition { _ -> read(self.n) }
        state read(&mut self, off: u32) {
            let value: &u32 = &self.buf[off] as &u32;
        }
    }
    "#,
    )
    .expect_err("a transitive boundary out-argument must invalidate the overlapping witness");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("recast")),
        "expected the recast refusal, got {diagnostics:#?}"
    );
}

#[test]
fn dependent_entry_fact_survives_unrelated_internal_call() {
    validate_contract_source(
        r#"
    data Main { count: u32; other: u32; remaining: u32; }
    machine Main::main(&mut self) { }
    machine Main::read(&mut self, i: u32 [0..=self.count]) {
        self.touch_other();
        self.remaining = self.count - i;
    }
    machine Main::touch_other(&mut self) {
        self.other = 1;
    }
    "#,
    )
    .expect("a disjoint internal call must preserve the dependent entry fact");
}

#[test]
fn dependent_entry_fact_dies_when_internal_call_writes_bound_field() {
    let diagnostics = validate_contract_source(
        r#"
    data Main { count: u32; remaining: u32; }
    machine Main::main(&mut self) { }
    machine Main::read(&mut self, i: u32 [0..=self.count]) {
        self.touch_count();
        self.remaining = self.count - i;
    }
    machine Main::touch_count(&mut self) {
        self.count = 1;
    }
    "#,
    )
    .expect_err("a call writing the bound field must invalidate the dependent fact");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("may overflow `u32`")),
        "expected the exact-arithmetic refusal, got {diagnostics:#?}"
    );
}

#[test]
fn symbolic_walk_recast_footprint_discharges() {
    // M2 gap 4b, the full chain: the walk state re-enters itself with
    // `offset + desc_size` under the guard `offset + desc_size + desc_size
    // <= map_size`; map_size's upper witness (<= 64) and desc_size's lower
    // witness (>= 8) resolve symbolically through the per-edge meet
    // (self-forwarding edges preserve entry bounds), so the loop edge
    // bounds the offset at 64 - 8 = 56 and the Desc footprint 56 + 8 <= 64
    // discharges. The entry edge is the constant 0.
    validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_memory_map(map_size: &mut u32, desc_size: &mut u32)
        ensures map_size <= 64 && desc_size >= 8;
    }
    data Desc { a: u32; b: u32; }
    data Main { fw: Firmware; buf: [u8; 64]; map_size: u32; desc_size: u32; }
    machine Main::main(&mut self) {
        self.fw.get_memory_map(&mut self.map_size, &mut self.desc_size);
        transition { _ -> walk(self.map_size, self.desc_size, 0) }
        state walk(&mut self, map_size: u32 in Trapping, desc_size: u32 in Trapping, offset: u32 in Trapping) {
            let d: &Desc = &self.buf[offset] as &Desc;
            transition offset + desc_size + desc_size <= map_size {
                true -> walk(map_size, desc_size, offset + desc_size)
                _ -> done()
            }
        }
        state done(&mut self) { }
    }
    "#,
    )
    .expect("the symbolic walk chain should discharge the recast footprint");
}

#[test]
fn boundary_ensures_equalities_couple_symbolic_recast_witnesses() {
    // R1/R4: the boundary supplies bounds through equal out-parameters rather
    // than directly on the values consumed by the walk. Equality transports
    // both the upper map-size witness and lower descriptor-size witness.
    validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_memory_map(
            map_size: &mut u32,
            desc_size: &mut u32,
            map_limit: &mut u32,
            desc_floor: &mut u32
        )
        ensures map_size == map_limit && map_limit <= 64
            && desc_size == desc_floor && desc_floor >= 8;
    }
    data Desc { a: u32; b: u32; }
    data Main {
        fw: Firmware;
        buf: [u8; 64];
        map_size: u32;
        desc_size: u32;
        map_limit: u32;
        desc_floor: u32;
    }
    machine Main::main(&mut self) {
        self.fw.get_memory_map(
            &mut self.map_size,
            &mut self.desc_size,
            &mut self.map_limit,
            &mut self.desc_floor
        );
        transition { _ -> walk(self.map_size, self.desc_size, 0) }
        state walk(
            &mut self,
            map_size: u32 in Trapping,
            desc_size: u32 in Trapping,
            offset: u32 in Trapping
        ) {
            let d: &Desc = &self.buf[offset] as &Desc;
            transition offset + desc_size + desc_size <= map_size {
                true -> walk(map_size, desc_size, offset + desc_size)
                _ -> done()
            }
        }
        state done(&mut self) { }
    }
    "#,
    )
    .expect("equal out-parameters should transport both symbolic bounds");
}

#[test]
fn symbolic_walk_guard_route_discharges() {
    // The owner-corrected HONEST spelling (Cathedral f0b7572): no trait
    // ensures (a vouch would be false under BUFFER_TOO_SMALL) -- a
    // post-call SANITY GUARD establishes both witnesses on the entry
    // edge, and the same symbolic chain discharges.
    validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_memory_map(map_size: &mut u32, desc_size: &mut u32);
    }
    data Desc { a: u32; b: u32; }
    data Main { fw: Firmware; buf: [u8; 64]; map_size: u32; desc_size: u32; }
    machine Main::main(&mut self) {
        self.fw.get_memory_map(&mut self.map_size, &mut self.desc_size);
        transition self.map_size <= 64 && self.desc_size >= 8 {
            true -> walk(self.map_size, self.desc_size, 0)
            _ -> bad()
        }
        state walk(&mut self, map_size: u32 in Trapping, desc_size: u32 in Trapping, offset: u32 in Trapping) {
            let d: &Desc = &self.buf[offset] as &Desc;
            transition offset + desc_size + desc_size <= map_size {
                true -> walk(map_size, desc_size, offset + desc_size)
                _ -> done()
            }
        }
        state done(&mut self) { }
        state bad(&mut self) { }
    }
    "#,
    )
    .expect("the guard-route walk chain should discharge the recast footprint");
}

#[test]
fn symbolic_walk_recast_wide_witness_refuses() {
    // `map_size <= 65` drags the loop-edge bound to 57; 57 + 8 > 64.
    let diagnostics = validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_memory_map(map_size: &mut u32, desc_size: &mut u32)
        ensures map_size <= 65 && desc_size >= 8;
    }
    data Desc { a: u32; b: u32; }
    data Main { fw: Firmware; buf: [u8; 64]; map_size: u32; desc_size: u32; }
    machine Main::main(&mut self) {
        self.fw.get_memory_map(&mut self.map_size, &mut self.desc_size);
        transition { _ -> walk(self.map_size, self.desc_size, 0) }
        state walk(&mut self, map_size: u32 in Trapping, desc_size: u32 in Trapping, offset: u32 in Trapping) {
            let d: &Desc = &self.buf[offset] as &Desc;
            transition offset + desc_size + desc_size <= map_size {
                true -> walk(map_size, desc_size, offset + desc_size)
                _ -> done()
            }
        }
        state done(&mut self) { }
    }
    "#,
    )
    .expect_err("a witness past the footprint must refuse");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("would read past the buffer")),
        "expected the footprint refusal, got {diagnostics:#?}"
    );
}

#[test]
fn symbolic_walk_weak_guard_spelling_refuses() {
    // The `< map_size` spelling bounds the next descriptor's START, not
    // its END: bound 63, 63 + 8 > 64 -- the exact tail overrun the design
    // note calls out; the chain must refuse it.
    let diagnostics = validate_contract_source(
        r#"
    boundary trait Firmware {
        machine get_memory_map(map_size: &mut u32, desc_size: &mut u32)
        ensures map_size <= 64 && desc_size >= 8;
    }
    data Desc { a: u32; b: u32; }
    data Main { fw: Firmware; buf: [u8; 64]; map_size: u32; desc_size: u32; }
    machine Main::main(&mut self) {
        self.fw.get_memory_map(&mut self.map_size, &mut self.desc_size);
        transition { _ -> walk(self.map_size, self.desc_size, 0) }
        state walk(&mut self, map_size: u32 in Trapping, desc_size: u32 in Trapping, offset: u32 in Trapping) {
            let d: &Desc = &self.buf[offset] as &Desc;
            transition offset + desc_size < map_size {
                true -> walk(map_size, desc_size, offset + desc_size)
                _ -> done()
            }
        }
        state done(&mut self) { }
    }
    "#,
    )
    .expect_err("the weak guard spelling admits a tail overrun and must refuse");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("would read past the buffer")),
        "expected the footprint refusal, got {diagnostics:#?}"
    );
}

#[test]
fn bracket_range_is_requires_sugar_for_ensures() {
    // R1 bracket-as-sugar (ch12): `k: u64 [0..=8]` IS `requires k >= 0 &&
    // k <= 8` -- the entailment engine assumes it, so `result <= 9`
    // proves for `result = k + 1` with no spelled requires.
    validate_contract_source(
        r#"
    data Main { }
    machine Main::pick(&mut self, k: u64 [0..=8]) -> u64
    ensures
        result <= 9
    {
        transition { _ -> (k + 1) }
    }
    machine Main::main(&mut self) {
        let v: u64 = self.pick(3);
    }
    "#,
    )
    .expect("the bracket range should discharge the ensures");
}

#[test]
fn bracket_range_sugar_does_not_over_prove() {
    // k = 8 makes k + 1 = 9 > 8: the tighter ensures must stay unproven.
    let diagnostics = validate_contract_source(
        r#"
    data Main { }
    machine Main::pick(&mut self, k: u64 [0..=8]) -> u64
    ensures
        result <= 8
    {
        transition { _ -> (k + 1) }
    }
    machine Main::main(&mut self) {
        let v: u64 = self.pick(3);
    }
    "#,
    )
    .expect_err("an ensures the range cannot support must refuse");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
        "expected the ensures refusal, got {diagnostics:#?}"
    );
}

#[test]
fn refutes_constant_false_ensures_on_empty_proof_machine() {
    let diagnostics = validate_contract_source(
        r#"
    machine false_constant()
    ensures
        1nat + 1nat == 3nat
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect_err("constant-false ensures should be refuted");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "ensures contract proof fact `1 + 1 == 3` is disproved by constant arithmetic"
            )),
        "expected constant refutation diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn refutes_strict_order_asymmetry_between_requires_and_ensures() {
    let diagnostics = validate_contract_source(
        r#"
    machine false_asymmetry(i: u64, j: u64)
    requires
        i < j
    ensures
        j < i
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect_err("asymmetric strict-order ensures should be refuted");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "ensures contract proof fact `j < i` is disproved: the requires contract entails its negation"
        )),
        "expected asymmetry refutation diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn proves_entailment_ladder_on_empty_proof_machines() {
    validate_contract_source(
        r#"
    machine sum_in_range(a: i32, b: i32)
    requires
        a in 1..=10
        b in 1..=10
    ensures
        a + b in 2..=20
    {
    }

    machine congruence_add(a: u64, b: u64)
    requires
        a == b
    ensures
        a + 1 == b + 1
    {
    }

    machine square_in_range(a: i32)
    requires
        a in 0..=10
    ensures
        a * a in 0..=100
    {
    }

    machine antisymmetry(a: u64, b: u64)
    requires
        a <= b
        b <= a
    ensures
        a == b
    {
    }

    machine remainder_bound(a: u64)
    ensures
        a % 2 in 0..=1
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect("the entailment ladder's true theorems must prove");
}

#[test]
fn rejects_unprovable_in_language_ensures() {
    let diagnostics = validate_contract_source(
        r#"
    machine sum_corner_too_tight(a: i32, b: i32)
    requires
        a in 1..=10
        b in 1..=10
    ensures
        a + b in 2..=19
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect_err("an unprovable in-language ensures must be rejected");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove ensures contract proof fact")),
        "expected cannot-prove diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn stands_down_on_out_of_language_contracts() {
    // `min(a, b)` is an unknown call shape: the engine cannot read it, so an
    // unprovable ensures must be ACCEPTED unchecked rather than rejected.
    let typed = typed_program_from_source(
        r#"
    machine uses_unknown_call(a: u64, b: u64)
    requires
        min(a, b) >= 1
    ensures
        a >= 1
    {
    }

    "#,
    );
    validate_program(&typed).expect("contracts outside the engine's language stand down");
    let stand_downs = validation::collect_contract_entailment_stand_downs(&typed);
    let [stand_down] = stand_downs.as_slice() else {
        panic!("one exact stand-down row")
    };
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == stand_down.machine_symbol)
        .expect("stand-down machine");
    assert_eq!(machine.name.as_str(), "uses_unknown_call");
    assert_eq!(stand_down.contract_index, 1);
    assert_eq!(stand_down.fact_index, 0);
    assert_eq!(
        stand_down.reason,
        validation::ContractEntailmentStandDownReason::OutsideEntailmentLanguage
    );
}

#[test]
fn accepted_boundary_claim_is_trust_not_a_proof_stand_down() {
    let typed = typed_program_from_source(
        r#"
    boundary machine accepted_unknown(a: u64, b: u64)
    ensures
        min(a, b) >= 1;
    "#,
    );
    validate_program(&typed).expect("accepted claim remains in the trust lane");
    assert!(validation::collect_contract_entailment_stand_downs(&typed).is_empty());
}

#[test]
fn unrecognized_bodied_contract_is_an_admission_stand_down() {
    let typed = typed_program_from_source(
        r#"
    machine unchecked_body(a: u64)
    ensures
        a >= 0
    {
        let copy: u64 = a;
    }
    "#,
    );
    validate_program(&typed).expect("ordinary validation preserves its historical stand-down");
    let stand_downs = validation::collect_contract_entailment_stand_downs(&typed);
    let [stand_down] = stand_downs.as_slice() else {
        panic!("one exact body-shape stand-down row")
    };
    assert_eq!(
        stand_down.reason,
        validation::ContractEntailmentStandDownReason::UnrecognizedInductiveBody
    );
}

#[test]
fn bodyful_boundary_contract_is_checked_not_accepted_supply() {
    let typed = typed_program_from_source(
        r#"
    boundary machine checked_boundary(a: u64)
    ensures
        a >= 0
    {
        let copy: u64 = a;
    }
    "#,
    );
    validate_program(&typed).expect("ordinary boundary validation should succeed");
    let stand_downs = validation::collect_contract_entailment_stand_downs(&typed);
    let [stand_down] = stand_downs.as_slice() else {
        panic!("one exact bodyful-boundary stand-down row")
    };
    assert_eq!(
        stand_down.reason,
        validation::ContractEntailmentStandDownReason::UnrecognizedInductiveBody
    );
}

#[test]
fn accepts_vacuous_ensures_under_contradictory_requires() {
    // Contradictory hypotheses entail anything (the `absurd` case), so the
    // refutation pass must stand down rather than reject a vacuous truth.
    validate_contract_source(
        r#"
    machine vacuous_truth(i: u64, j: u64)
    requires
        i < j
        j < i
    ensures
        j < i
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect("vacuously true contract should not be refuted");
}

#[test]
fn refutation_skips_machines_with_body_statements() {
    // A body could establish facts beyond the requires set; the refutation
    // pass only reasons about empty-body proof artifacts. The constant-false
    // ensures here is still wrong, but rejecting it is the entailment
    // engine's job, not this pass's.
    validate_contract_source(
        r#"
    data Counter {
        value: i32;
    }

    machine Counter::touch(&mut self)
    ensures
        1nat + 1nat == 3nat
    {
        self.value = 1;
    }

    data Main {
        counter: Counter;
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect("non-empty bodies are outside the refutation pass");
}

#[test]
fn proves_inductive_theorem_via_recursive_contract_and_decrease() {
    // Ladder rung L7: the accumulator Gauss sum. The recursive arm assumes
    // the machine's own ensures for (n - 1, acc + n) -- justified by the
    // discharged strict decrease of `n` under the n > 0 guard -- and the base
    // arm grounds `result := acc` under n == 0. Wrapping is intentional: this
    // is a polynomial ring identity over every u64 input, whereas the same
    // universal Exact contract would make false global no-overflow claims.
    validate_contract_source(
        r#"
    data Main {
    }

    machine Main::main(&mut self) {
        let total: u64 in Wrapping = self.gauss_sum(4, 0);
    }

    machine Main::gauss_sum(&mut self, n: u64 in Wrapping, acc: u64 in Wrapping) -> u64 in Wrapping
    terminates by n;
    ensures
        result * 2 == acc * 2 + n * (n + 1)
    {
        transition n > 0 {
            true -> self.gauss_sum(n - 1, acc + n)
            false -> acc
        }
    }
    "#,
    )
    .expect("the inductive gauss-sum theorem must prove");
}

#[test]
fn refutes_inductive_false_twin_on_the_base_arm() {
    // The off-by-one twin's inductive step is self-consistent; the base arm
    // (n == 0, result := acc) collapses to 0 == 1 and must be disproved.
    let diagnostics = validate_contract_source(
        r#"
    data Main {
    }

    machine Main::main(&mut self) {
        let total: u64 in Wrapping = self.gauss_sum(4, 0);
    }

    machine Main::gauss_sum(&mut self, n: u64 in Wrapping, acc: u64 in Wrapping) -> u64 in Wrapping
    terminates by n;
    ensures
        result * 2 == acc * 2 + n * (n + 1) + 1
    {
        transition n > 0 {
            true -> self.gauss_sum(n - 1, acc + n)
            false -> acc
        }
    }
    "#,
    )
    .expect_err("the off-by-one inductive twin must be disproved on its base arm");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "is disproved on the transition arm guarded by `n > 0 == false`: the arm's facts entail its negation"
        )),
        "expected base-arm refutation diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_inductive_step_false_twin_on_the_recursive_arm() {
    // Base case true, step false: the instantiated hypothesis misses the
    // goal, every fact is in-language and the decrease IS discharged, so the
    // recursive arm's unproven obligation rejects.
    let diagnostics = validate_contract_source(
        r#"
    data Main {
    }

    machine Main::main(&mut self) {
        let total: u64 in Wrapping = self.gauss_sum(4, 0);
    }

    machine Main::gauss_sum(&mut self, n: u64 in Wrapping, acc: u64 in Wrapping) -> u64 in Wrapping
    terminates by n;
    ensures
        result * 2 == acc * 2 + n * (n + 3)
    {
        transition n > 0 {
            true -> self.gauss_sum(n - 1, acc + n)
            false -> acc
        }
    }
    "#,
    )
    .expect_err("the step-false inductive twin must be rejected on its recursive arm");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "cannot prove ensures contract proof fact `result * 2 == acc * 2 + n * n + 3` on the transition arm guarded by `n > 0 == true`"
        )),
        "expected recursive-arm cannot-prove diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn no_induction_hypothesis_without_a_decreases_claim() {
    // SOUNDNESS GATE: without `terminates by ...;` there is no
    // well-founded measure, so the recursive arm gets no induction
    // hypothesis. The goal cannot prove, but the missing hypothesis (not the
    // theorem) may be at fault, so the engine stands down rather than
    // rejecting -- the theorem is accepted UNCHECKED, exactly the bodied
    // status quo.
    validate_contract_source(
        r#"
    data Main {
    }

    machine Main::main(&mut self) {
        let total: u64 in Wrapping = self.gauss_sum(4, 0);
    }

    machine Main::gauss_sum(&mut self, n: u64 in Wrapping, acc: u64 in Wrapping) -> u64 in Wrapping
    ensures
        result * 2 == acc * 2 + n * (n + 1)
    {
        transition n > 0 {
            true -> self.gauss_sum(n - 1, acc + n)
            false -> acc
        }
    }
    "#,
    )
    .expect("no decreases claim means no hypothesis and no rejection");
}

#[test]
fn no_induction_hypothesis_when_the_call_site_decrease_is_undischarged() {
    // The decreases clause names `n`, but the recursive call passes `n`
    // unchanged: the strict decrease fails at that exact call site, so no
    // hypothesis enters and the arm cannot drive a rejection. (The
    // machine-level termination pass independently rejects this program; the
    // entailment engine must not pile a false ensures rejection on top.)
    let diagnostics = validate_contract_source(
        r#"
    data Main {
    }

    machine Main::main(&mut self) {
        let total: u64 in Wrapping = self.gauss_sum(4, 0);
    }

    machine Main::gauss_sum(&mut self, n: u64 in Wrapping, acc: u64 in Wrapping) -> u64 in Wrapping
    terminates by n;
    ensures
        result * 2 == acc * 2 + n * (n + 1)
    {
        transition n > 0 {
            true -> self.gauss_sum(n, acc + n)
            false -> acc
        }
    }
    "#,
    )
    .err()
    .unwrap_or_default();
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ensures contract proof fact")),
        "an undischarged call-site decrease must not produce an ensures rejection, got {diagnostics:#?}"
    );
}

#[test]
fn accepts_true_theorems_on_empty_proof_machines() {
    validate_contract_source(
        r#"
    machine pythagorean_three_four_five()
    ensures
        3nat * 3nat + 4nat * 4nat == 5nat * 5nat
    {
    }

    machine less_than_transitive(a: u64, b: u64, c: u64)
    requires
        a < b
        b < c
    ensures
        a < c
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect("true theorems should pass the refutation pass");
}

#[test]
fn rejects_unknown_domain_membership_in_contract() {
    let source = r#"
    data Player {
    }

    boundary trait Renderer {
        machine draw(player: Player)
        requires
            player in Player::Drawable;
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject domain");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "trait `Renderer` state `draw` requires contract references unknown domain `Player::Drawable`"
        )),
        "expected unknown contract domain diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_non_boolean_shaped_proof_fact() {
    let source = r#"
    data Player {
    }

    domain Player::Weird
    requires
        1 + 2

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject fact");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("domain `Player::Weird` proof fact `1 + 2` is not boolean-shaped")),
        "expected non-boolean proof fact diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_domain_import_with_different_target_type() {
    let source = r#"
    data Player {
    }

    data Enemy {
    }

    domain Enemy::Valid
    requires
        true

    domain Player::Alive
    requires
        self in Enemy::Valid

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject import");
    let has_target_mismatch = diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "domain `Player::Alive` imports `Enemy::Valid` but they classify different types",
        )
    });
    assert!(
        has_target_mismatch,
        "expected domain target mismatch diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_domain_import_cycles() {
    let source = r#"
    data Player {
    }

    domain Player::Alive
    requires
        self in Player::Valid

    domain Player::Valid
    requires
        self in Player::Alive

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject cycle");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "domain membership cycle: Player::Alive -> Player::Valid -> Player::Alive",
            )
        }),
        "expected domain cycle diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_unknown_transparent_alias_constituent() {
    let typed = typed_program_from_source(
        r#"
        data Socket {}
        domain Socket::Usable =
            Socket::Connected & Socket::Missing;
        domain Socket::Connected
        requires
            true;
        "#,
    );

    let diagnostics = validate_program(&typed).expect_err("unknown alias constituent must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("domain alias `Socket::Usable` references unknown domain `Socket::Missing`")
    }));
}

#[test]
fn rejects_transparent_alias_constituent_with_different_carrier() {
    let typed = typed_program_from_source(
        r#"
        data Socket {}
        data Session {}
        domain Socket::Connected
        requires
            true;
        domain Session::Authenticated
        requires
            true;
        domain Socket::Usable =
            Socket::Connected & Session::Authenticated;
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("cross-carrier alias constituent must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "domain alias `Socket::Usable` includes `Session::Authenticated` but they classify different types",
        )
    }));
}

#[test]
fn rejects_transparent_domain_alias_cycles() {
    let typed = typed_program_from_source(
        r#"
        data Socket {}
        domain Socket::Usable = Socket::Ready;
        domain Socket::Ready = Socket::Usable;
        "#,
    );

    let diagnostics = validate_program(&typed).expect_err("alias cycle must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("domain alias cycle: Socket::Usable -> Socket::Ready -> Socket::Usable")
    }));
}

#[test]
fn rejects_public_alias_that_publishes_private_constituent() {
    let typed = typed_program_from_source(
        r#"
        data Socket {}
        domain Socket::Connected
        requires
            true;
        pub domain Socket::Usable = Socket::Connected;
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("public alias cannot expose private atom");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "public domain alias `Socket::Usable` cannot publish private constituent `Socket::Connected`",
        )
    }));
}

#[test]
fn rejects_machine_service_reaches_outside_trait_ceiling() {
    let source = r#"
    boundary trait Filesystem {}

    boundary trait Console {
        machine write_line(text: &[u8])
        reaches
            Console;
    }

    data ConsoleImpl {
    }

    machine ConsoleImpl::write_line(text: &[u8]) satisfies Console::write_line
    reaches
        Console + Filesystem
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject extra effect");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("service `Filesystem` is not allowed by the trait requirement")),
        "expected service ceiling diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn accepts_machine_service_reaches_within_trait_ceiling() {
    let source = r#"
    boundary trait Console {
        machine write_line(text: &[u8])
        reaches
            Console;
    }

    data ConsoleImpl {
    }

    machine ConsoleImpl::write_line(text: &[u8]) satisfies Console::write_line
    reaches
        Console
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn accepts_machine_service_reaches_below_trait_ceiling() {
    let source = r#"
    boundary trait Console {
        machine write_line(text: &[u8])
        reaches
            Console;
    }

    data TestConsole {
    }

    machine TestConsole::write_line(text: &[u8]) satisfies Console::write_line {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn accepts_nominal_boundary_adapter_with_forwarded_service_receiver() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Console {
            machine write(text: &[u8])
            reaches
                Console;
        }

        data ConsoleProvider {}

        machine ConsoleProvider::write(console: Console, text: &[u8])
        satisfies Console::write
        reaches
            Console
        {
        }
        "#,
    );

    validate_program(&typed)
        .expect("a nominal checked adapter may receive the boundary receiver explicitly");
}

#[test]
fn compiler_intrinsic_may_retain_a_compiler_owned_safe_carrier_surface() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Console {
            machine read_line(out_line: &mut [u8])
            reaches
                Console;
        }

        machine console_read_line(out_line: &mut [u8])
        satisfies Console::read_line
        via Binding::CompilerIntrinsic;
        "#,
    );

    validate_program(&typed)
        .expect("a compiler intrinsic owns the safe carrier lowering rather than a foreign ABI");
}

#[test]
fn foreign_import_still_rejects_a_private_safe_carrier_surface() {
    let typed = typed_program_from_source(
        r#"
        boundary trait Console {
            machine read_line(out_line: &mut [u8])
            reaches
                Console;
        }

        machine console_read_line(out_line: &mut [u8])
        satisfies Console::read_line
        via Binding::DllImport("host", "read_line");
        "#,
    );

    let diagnostics =
        validate_program(&typed).expect_err("a foreign ABI cannot inherit the safe slice layout");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot use safe slice `&mut [u8]` as a default-native parameter")
    }));
}

#[test]
fn rejects_published_service_ceiling_below_reached_services() {
    let source = r#"
    boundary trait Input {
        machine read_line(out: &mut [u8; 32]);
    }

    boundary trait Output {
    }

    data Main {
        input: Input;
    }

    machine Main::main(&mut self)
    reaches
        Output
    {
        let line: [u8; 32];
        self.input.read_line(&mut line);
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let operations = flow_effects::infer_operational_may(&typed);
    let diagnostics =
        validate_behavior_plan(&typed, &operations).expect_err("service ceiling should fail");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("publishes service reach `Output`")
                && diagnostic
                    .message
                    .contains("reaches undeclared service `Input`")
        }),
        "expected normalized service ceiling diagnostic, got {diagnostics:#?}"
    );
}

/// Tests relocated from `effects` when that crate moved into the
/// `representations` layer. They exercise the effect/capability *analysis*
/// passes, which require building a `TypedTrees` through the front-of-pipeline
/// lowering crates. Those pipeline crates are dev-dependencies here (a
/// semantics crate may depend up into pipeline for tests), but must not be
/// dev-dependencies of `effects` itself, since that would re-introduce a
/// `representations -> pipeline` upward edge.
mod effects_analysis {
    use effects::{
        BoundaryCallCoordinate, BoundaryProviderApproval, BoundaryProviderApprovalRegistry,
        audit_boundary_provider_calls, build_boundary_provider_approval_registry,
    };
    use flow_effects::{infer_operational_may, infer_service_reaches};
    use typed_trees::TypedTrees;

    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use tokens_to_syntax_trees::parse_syntax_trees;

    fn lower(source: &str) -> TypedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
        lower_symbol_resolved_trees(&resolved).expect("type")
    }

    fn lower_checked_calls(source: &str) -> (TypedTrees, Vec<BoundaryCallCoordinate>) {
        let checked = typed_trees_to_checked_trees::lower_typed_trees(lower(source))
            .expect("checked lowering");
        let flow = &checked.facts.flow.control;
        let mut calls = Vec::new();
        for (_, state) in flow.states.iter() {
            for call in flow.calls.span_or_empty(state.calls) {
                let edges = checked
                    .facts
                    .flow
                    .boundaries
                    .edges
                    .span_or_empty(call.boundary_edges);
                if edges.is_empty() {
                    let direct = checked
                        .typed
                        .traits()
                        .iter()
                        .filter(|definition| definition.is_boundary)
                        .flat_map(|definition| {
                            checked
                                .typed
                                .trait_machine_signatures(definition)
                                .iter()
                                .filter(move |signature| signature.symbol == call.target_symbol)
                                .map(move |signature| (definition.symbol, signature.symbol))
                        })
                        .collect::<Vec<_>>();
                    let [(boundary_trait_symbol, boundary_signature_symbol)] = direct.as_slice()
                    else {
                        continue;
                    };
                    calls.push(BoundaryCallCoordinate {
                        machine_symbol: state.machine_symbol,
                        state_symbol: state.state_symbol,
                        target_state_symbol: call.target_symbol,
                        boundary_trait_symbol: *boundary_trait_symbol,
                        boundary_signature_symbol: *boundary_signature_symbol,
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                    });
                    continue;
                }
                calls.extend(edges.iter().map(|edge| BoundaryCallCoordinate {
                    machine_symbol: state.machine_symbol,
                    state_symbol: state.state_symbol,
                    target_state_symbol: call.target_symbol,
                    boundary_trait_symbol: edge.boundary_trait_symbol,
                    boundary_signature_symbol: edge.boundary_signature_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                }));
            }
        }
        (checked.typed, calls)
    }

    #[test]
    fn propagates_canonical_service_reach_to_call_sites() {
        let source = r#"
        boundary trait Console {
            machine write_line(text: &[u8]);
        }

        data ConsoleImpl {
        }

        machine ConsoleImpl::write_line(text: &[u8]) satisfies Console::write_line
        reaches
            Console
        {
        }

        data Main {
            console: ConsoleImpl;
        }

        machine Main::main(&mut self) {
            self.console.write_line("hello");
        }
        "#;

        let typed = lower(source);
        let operations = infer_operational_may(&typed);
        let services = infer_service_reaches(&typed, &operations);
        let console = typed
            .service_reaches
            .id_for_name("Console")
            .expect("Console service");

        let main_machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("main machine");
        let main_services = services
            .for_machine(main_machine.symbol)
            .expect("main service summary");
        assert_eq!(
            services.services(main_services.inferred_transitive),
            &[console]
        );
        let main_state = services
            .states_for(main_services)
            .first()
            .expect("main state");
        let call = services.calls_for(main_state).first().expect("call");
        assert_eq!(services.services(call.inferred_transitive), &[console]);
    }

    #[test]
    fn provider_approval_ignores_empty_and_rejects_unknown_call_coordinates() {
        let program = TypedTrees::default();
        let registry = build_boundary_provider_approval_registry(&program);

        assert!(
            audit_boundary_provider_calls(
                &program,
                Vec::<BoundaryCallCoordinate>::new(),
                &registry,
            )
            .is_empty()
        );
        let unknown = symbols::SymbolHandle::from_arena_index(3);
        let unapproved = audit_boundary_provider_calls(
            &program,
            [BoundaryCallCoordinate {
                machine_symbol: symbols::SymbolHandle::from_arena_index(1),
                state_symbol: symbols::SymbolHandle::from_arena_index(2),
                target_state_symbol: unknown,
                boundary_trait_symbol: unknown,
                boundary_signature_symbol: symbols::SymbolHandle::from_arena_index(4),
                statement_index: 5,
                call_ordinal: 6,
            }],
            &registry,
        );
        assert_eq!(unapproved.len(), 1);
        assert_eq!(unapproved[0].boundary_trait_symbol, unknown);
    }

    #[test]
    fn provider_approval_audits_each_same_site_boundary_edge_by_exact_trait() {
        let approved = symbols::SymbolHandle::from_arena_index(1);
        let denied = symbols::SymbolHandle::from_arena_index(2);
        let signature = symbols::SymbolHandle::from_arena_index(3);
        let machine = symbols::SymbolHandle::from_arena_index(4);
        let state = symbols::SymbolHandle::from_arena_index(5);
        let target = symbols::SymbolHandle::from_arena_index(6);
        let registry = BoundaryProviderApprovalRegistry::with_providers(vec![
            BoundaryProviderApproval::new(approved, true),
            BoundaryProviderApproval::new(denied, false),
        ]);
        let coordinate = |boundary_trait_symbol| BoundaryCallCoordinate {
            machine_symbol: machine,
            state_symbol: state,
            target_state_symbol: target,
            boundary_trait_symbol,
            boundary_signature_symbol: signature,
            statement_index: 7,
            call_ordinal: 8,
        };

        let unapproved = audit_boundary_provider_calls(
            &TypedTrees::default(),
            [coordinate(approved), coordinate(denied)],
            &registry,
        );
        assert_eq!(unapproved.len(), 1);
        assert_eq!(unapproved[0].boundary_trait_symbol, denied);
        assert_eq!(unapproved[0].statement_index, 7);
        assert_eq!(unapproved[0].call_ordinal, 8);
    }

    #[test]
    fn duplicate_boundary_registry_identity_is_denied_without_or_laundering() {
        for (second_symbol, second_name) in [
            (symbols::SymbolHandle::from_arena_index(1), "Console"),
            (symbols::SymbolHandle::from_arena_index(2), "Console"),
        ] {
            let first_symbol = symbols::SymbolHandle::from_arena_index(1);
            let mut program = TypedTrees::default();
            for (symbol, name) in [(first_symbol, "Console"), (second_symbol, second_name)] {
                program.push_trait_definition(typed_trees::trait_definition::TraitDefinition {
                    symbol,
                    is_boundary: true,
                    name: typed_trees::name::Identifier::generated(name),
                    ..Default::default()
                });
            }

            let registry = build_boundary_provider_approval_registry(&program);
            assert!(
                registry
                    .providers()
                    .iter()
                    .all(|provider| !provider.approved),
                "duplicate exact symbol or exact authored name must deny every row: {registry:?}",
            );
            assert_eq!(
                registry.authorize_boundary_call(first_symbol),
                effects::BoundaryCallApproval::Unapproved,
            );
        }
    }

    #[test]
    fn abstract_boundary_provider_is_authorized() {
        let (program, calls) = lower_checked_calls(
            r#"
            boundary trait Console {
                machine write_line(text: &[u8]);
            }

            data Main {
                console: Console;
            }

            machine Main::main(&mut self) {
                self.console.write_line("hello");
            }
            "#,
        );
        let registry = build_boundary_provider_approval_registry(&program);
        let unapproved = audit_boundary_provider_calls(&program, calls, &registry);
        assert!(
            unapproved.is_empty(),
            "abstract provider should be approved"
        );
    }

    #[test]
    fn exact_requirement_adapter_does_not_revoke_boundary_provider_approval() {
        let (program, calls) = lower_checked_calls(
            r#"
            boundary trait Console {
                machine write_line(text: &[u8])
                reaches
                    Console;
            }

            data ConsoleAdapter {}

            machine ConsoleAdapter::write_line(text: &[u8])
            satisfies Console::write_line
            reaches
                Console
            {
            }

            data Main {
                console: ConsoleAdapter;
            }

            machine Main::main(&mut self) {
                self.console.write_line("hello");
            }
            "#,
        );
        let registry = build_boundary_provider_approval_registry(&program);
        let console = program
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "Console")
            .expect("Console boundary");

        assert!(
            registry
                .provider(console.symbol)
                .expect("Console approval")
                .approved,
            "an exact requirement adapter is a supply edge, not a whole-trait implementation",
        );
        assert!(
            audit_boundary_provider_calls(&program, calls, &registry).is_empty(),
            "calls through the exact adapter should retain boundary provider approval",
        );
    }

    #[test]
    fn in_package_host_provider_is_unapproved() {
        let (program, calls) = lower_checked_calls(
            r#"
            boundary trait LocalFiles {
                machine write_bytes(path: &[u8]);
            }

            data Disk {
            }

            DiskLocalFiles: Disk satisfies LocalFiles;

            machine Disk::write_bytes(path: &[u8]) satisfies LocalFiles::write_bytes {
            }

            data Main {
                disk: Disk;
            }

            machine Main::main(&mut self) {
                self.disk.write_bytes("/etc/passwd");
            }
            "#,
        );
        let registry = build_boundary_provider_approval_registry(&program);
        let unapproved = audit_boundary_provider_calls(&program, calls, &registry);
        assert_eq!(
            unapproved.len(),
            1,
            "in-package host provider should be flagged as unapproved"
        );
        let local_files = program
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "LocalFiles")
            .expect("LocalFiles boundary");
        assert_eq!(
            unapproved[0].boundary_trait_symbol, local_files.symbol,
            "approval failure must name the exact boundary capability",
        );
        let main = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("Main::main machine");
        let main_state = program
            .machine_states(main)
            .first()
            .expect("Main::main state");
        assert_eq!(unapproved[0].machine_symbol, main.symbol);
        assert_eq!(unapproved[0].state_symbol, main_state.symbol);
        assert_eq!(unapproved[0].statement_index, 0);
        assert_eq!(unapproved[0].call_ordinal, 0);
    }

    #[test]
    fn in_package_boundary_provider_needs_no_lowercase_effect_to_be_unapproved() {
        let (program, calls) = lower_checked_calls(
            r#"
            boundary trait LocalClock {
                machine tick(token: i32);
            }

            data FakeClock {}

            FakeClockLocalClock: FakeClock satisfies LocalClock;

            machine FakeClock::tick(token: i32) satisfies LocalClock::tick {}

            data Main {
                clock: FakeClock;
            }

            machine Main::main(&mut self) {
                self.clock.tick(0);
            }
            "#,
        );
        let registry = build_boundary_provider_approval_registry(&program);
        let local_clock = program
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "LocalClock")
            .expect("LocalClock boundary");
        assert!(
            !registry
                .provider(local_clock.symbol)
                .expect("LocalClock approval")
                .approved
        );
        let call_targets = calls
            .iter()
            .map(|call| call.target_state_symbol)
            .collect::<Vec<_>>();
        let unapproved = audit_boundary_provider_calls(&program, calls, &registry);

        assert_eq!(unapproved.len(), 1, "call targets: {call_targets:?}");
        assert_eq!(unapproved[0].boundary_trait_symbol, local_clock.symbol);
    }
}

mod structural_entailment {
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use tokens_to_syntax_trees::parse_syntax_trees;
    use validation::validate_program;

    /// Run source through parse -> resolve -> typed -> validate. `Nat` is
    /// declared inline (the judge is parametric over any recursive proof
    /// data, so no bundled import is needed). Returns the joined diagnostic
    /// text on failure, or the empty string on success.
    fn validate(body: &str) -> Result<(), String> {
        let source = format!(
            "data Nat {{ case Zero; case Succ(prev: Nat); }}\n\
             data Main {{}}\n\
             machine Main::main(&mut self) {{}}\n\
             machine add(a: Nat, b: Nat) -> Nat terminates by a; {{\n\
             transition a {{ Nat::Zero -> (b) Nat::Succ {{ prev }} -> Nat::Succ {{ prev: add(prev, b) }} }}\n\
             }}\n\
             {body}\n"
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering");
        validate_program(&typed).map_err(|diagnostics| {
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        })
    }

    #[test]
    fn reflexivity_proves() {
        validate("machine refl(a: Nat) ensures a == a {}")
            .expect("a == a should prove structurally");
    }

    #[test]
    fn constructor_injectivity_proves() {
        validate(
            "machine inj(a: Nat, b: Nat) \
             requires (Nat::Succ { prev: a }) == (Nat::Succ { prev: b }); \
             ensures a == b; {}",
        )
        .expect("Succ injectivity should prove");
    }

    #[test]
    fn case_disjointness_refutes() {
        let error =
            validate("machine bad(a: Nat) requires a == Nat::Zero; ensures a != Nat::Zero; {}")
                .expect_err("a != Zero under a == Zero should refute");
        assert!(
            error.contains("disproved structurally"),
            "expected a structural disproof, got: {error}"
        );
    }

    #[test]
    fn ground_compute_proves() {
        validate(
            "machine two() ensures \
             (add(Nat::Succ { prev: Nat::Zero }, Nat::Succ { prev: Nat::Zero })) \
             == (Nat::Succ { prev: Nat::Succ { prev: Nat::Zero } }) {}",
        )
        .expect("1 + 1 == 2 should prove by unfolding");
    }

    #[test]
    fn false_ground_compute_refutes() {
        let error = validate(
            "machine bad() ensures \
             (add(Nat::Succ { prev: Nat::Zero }, Nat::Zero)) == Nat::Zero {}",
        )
        .expect_err("add(1, 0) == 0 should refute");
        assert!(
            error.contains("disproved structurally"),
            "expected a structural disproof, got: {error}"
        );
    }

    #[test]
    fn structural_induction_proves() {
        validate(
            "machine right_id(a: Nat) -> Nat terminates by a; \
             ensures result == a { \
             transition a { Nat::Zero -> Nat::Zero \
             Nat::Succ { prev } -> Nat::Succ { prev: right_id(prev) } } }",
        )
        .expect("right identity by induction should prove");
    }

    #[test]
    fn false_inductive_claim_refutes() {
        let error = validate(
            "machine bad(a: Nat) -> Nat terminates by a; \
             ensures result == Nat::Zero { \
             transition a { Nat::Zero -> Nat::Zero \
             Nat::Succ { prev } -> Nat::Succ { prev: bad(prev) } } }",
        )
        .expect_err("a false inductive claim should refute");
        assert!(
            error.contains("disproved structurally"),
            "expected a structural disproof, got: {error}"
        );
    }

    #[test]
    fn application_equation_law_proves() {
        validate(
            "machine succ_law(a: Nat, b: Nat) -> Nat terminates by a; \
             ensures (add(a, Nat::Succ { prev: b })) == (Nat::Succ { prev: add(a, b) }) { \
             transition a { Nat::Zero -> Nat::Zero \
             Nat::Succ { prev } -> Nat::Succ { prev: succ_law(prev, b) } } }",
        )
        .expect("the successor-shift law should prove by induction");
    }

    #[test]
    fn seq_length_cons_proves() {
        // Generic recursive Seq<T>: the judge is parametric over constructor
        // names, so length(Cons(x, s)) == Succ(length(s)) proves by one
        // unfold, the tail's length staying symbolic on both sides.
        validate(
            "data Seq { case Empty; case Cons(head: Nat, tail: Seq); } \
             machine length(s: Seq) -> Nat terminates by s; { \
             transition s { Seq::Empty -> Nat::Zero \
             Seq::Cons { head, tail } -> Nat::Succ { prev: length(tail) } } } \
             machine length_cons(x: Nat, s: Seq) \
             ensures (length(Seq::Cons { head: x, tail: s })) \
             == (Nat::Succ { prev: length(s) }); {}",
        )
        .expect("length_cons should prove by unfolding");
    }

    #[test]
    fn lemma_citation_proves() {
        // right_id proves `result == a`; a caller cites it (its inductive
        // body never finitely unfolds for a symbolic argument).
        validate(
            "machine right_id(a: Nat) -> Nat terminates by a; \
             ensures result == a { \
             transition a { Nat::Zero -> Nat::Zero \
             Nat::Succ { prev } -> Nat::Succ { prev: right_id(prev) } } } \
             machine cite(a: Nat) -> Nat ensures result == a { \
             transition { _ -> (right_id(a)) } }",
        )
        .expect("citing a proven functional ensures should prove");
    }

    /// The law-shaped lemma every statement-citation test consumes:
    /// `result == a` (functional) plus `add(a, Zero) == a` (the LAW an
    /// explicit citation delivers), both by one induction.
    const RIGHT_ID_LAW: &str = "machine right_id(a: Nat) -> Nat \
         terminates by a; \
         ensures result == a; (add(a, Nat::Zero)) == a; { \
         transition a { Nat::Zero -> Nat::Zero \
         Nat::Succ { prev } -> Nat::Succ { prev: right_id(prev) } } }";

    #[test]
    fn law_conjunct_proves_alongside_functional_ensures() {
        validate(RIGHT_ID_LAW).expect("both conjuncts should prove by the same induction");
    }

    #[test]
    fn statement_citation_proves() {
        // ch10 "Citing Proofs" / the settled proof-citation rule: the bare
        // statement call injects right_id's law instantiated at `b`.
        validate(
            &(RIGHT_ID_LAW.to_owned()
                + " machine cite_zero(b: Nat) ensures (add(b, Nat::Zero)) == b { \
               right_id(b); }"),
        )
        .expect("a statement citation should deliver the law");
    }

    #[test]
    fn uncited_law_fact_fences_and_names_the_missing_citation() {
        // Nothing is global, nothing applies silently: the same goal
        // WITHOUT the citation statement must fence -- and the settled
        // ergonomics mitigation names the lemma whose ensures
        // shape-matches it, with the exact operands.
        let error = validate(
            &(RIGHT_ID_LAW.to_owned()
                + " machine uncited(b: Nat) ensures (add(b, Nat::Zero)) == b {}"),
        )
        .expect_err("an uncited law fact should fence");
        assert!(
            error.contains("no entailment tier judges yet"),
            "expected the N3 fence, got: {error}"
        );
        assert!(
            error.contains("note: `right_id` proves this shape -- cite it: `right_id(b);`"),
            "expected the citation suggestion, got: {error}"
        );
    }

    #[test]
    fn requires_bearing_citation_rejected() {
        // SITE DISCHARGE negative: the citer has no hypothesis establishing
        // the callee's instantiated requires, so the citation refuses and
        // names the undischarged fact.
        let error = validate(
            "machine sym(a: Nat, b: Nat) requires a == b; ensures b == a; {} \
             machine conditional_cite(x: Nat, y: Nat) ensures y == x { \
             sym(x, y); }",
        )
        .expect_err(
            "citing a requires-bearing lemma without establishing its requires should reject",
        );
        assert!(
            error.contains("is not established at this citation site"),
            "expected the site-discharge refusal, got: {error}"
        );
    }

    #[test]
    fn requires_bearing_citation_discharged() {
        // SITE DISCHARGE positive: the citer's own requires establish the
        // callee's instantiated requires, so the citation injects the
        // conditional ensures and the goal proves.
        validate(
            "machine sym(a: Nat, b: Nat) requires a == b; ensures b == a; {} \
             machine conditional_cite(x: Nat, y: Nat) requires x == y; \
             ensures y == x { sym(x, y); }",
        )
        .expect("a citation whose requires are established at the site should prove");
    }

    /// The successor-shift law, the step case's citation material.
    const SUCC_LAW: &str = "machine add_succ_law(a: Nat, b: Nat) -> Nat \
         terminates by a; \
         ensures (add(a, Nat::Succ { prev: b })) == (Nat::Succ { prev: add(a, b) }) { \
         transition a { Nat::Zero -> Nat::Zero \
         Nat::Succ { prev } -> Nat::Succ { prev: add_succ_law(prev, b) } } }";

    /// COMMUTATIVITY (N3 rung 2): induction on `a` with per-arm SUB-STATE
    /// proofs -- the base case cites right_id's law at `b`, the step case
    /// cites add_succ_law AT THE CASE PAYLOAD (only reachable from a
    /// sub-state's frame) and takes the IH from its self-application.
    const ADD_COMM: &str = "machine add_comm(a: Nat, b: Nat) -> Nat \
         terminates by a; \
         ensures (add(a, b)) == (add(b, a)) { \
         transition a { Nat::Zero -> base_case(b) \
         Nat::Succ { prev } -> step_case(prev, b) } \
         state base_case(b: Nat) -> Nat { \
         right_id(b); \
         transition { _ -> (b) } } \
         state step_case(prev: Nat, b: Nat) -> Nat { \
         add_succ_law(b, prev); \
         transition { _ -> Nat::Succ { prev: add_comm(prev, b) } } } }";

    #[test]
    fn commutativity_proves_with_per_arm_citations() {
        validate(&(RIGHT_ID_LAW.to_owned() + " " + SUCC_LAW + " " + ADD_COMM))
            .expect("add_comm should prove by induction with per-arm citations");
    }

    #[test]
    fn citing_a_multi_state_lemma_delivers_its_law() {
        // The consumer face of rung 2: add_comm has THREE states (entry +
        // two sub-proofs); a citation reads only the ENTRY signature.
        // Pinned after a real regression -- the single-state `[entry]`
        // gate silently dropped multi-state callees' equations.
        validate(
            &(RIGHT_ID_LAW.to_owned()
                + " "
                + SUCC_LAW
                + " "
                + ADD_COMM
                + " machine use_comm(x: Nat, y: Nat) \
                   ensures (add(x, y)) == (add(y, x)) { \
                   add_comm(x, y); }"),
        )
        .expect("citing the proven add_comm should deliver its law");
    }

    #[test]
    fn commutativity_step_without_citation_fences() {
        // Drop the step case's citation: the IH alone cannot bridge
        // `add(b, Succ(prev))`, so the claim must fence, proving the
        // per-arm citation is load-bearing.
        let uncited = ADD_COMM.replace("add_succ_law(b, prev); ", "");
        let error = validate(&(RIGHT_ID_LAW.to_owned() + " " + SUCC_LAW + " " + &uncited))
            .expect_err("the step case without its citation should fence");
        assert!(
            error.contains("no entailment tier judges yet"),
            "expected the N3 fence, got: {error}"
        );
    }

    #[test]
    fn false_commutativity_shaped_claim_refutes() {
        // Same sub-state shape, wrong theorem: add(a, b) == add(b, b)
        // must refute on the base arm (add(Zero, b) == b vs add(b, b)).
        let error = validate(
            &(RIGHT_ID_LAW.to_owned()
                + " "
                + SUCC_LAW
                + " machine bad_comm(a: Nat, b: Nat) -> Nat \
                   terminates by a; \
                   ensures (add(a, b)) == (add(b, b)) { \
                   transition a { Nat::Zero -> base_case(b) \
                   Nat::Succ { prev } -> step_case(prev, b) } \
                   state base_case(b: Nat) -> Nat { \
                   right_id(b); \
                   transition { _ -> (b) } } \
                   state step_case(prev: Nat, b: Nat) -> Nat { \
                   add_succ_law(b, prev); \
                   transition { _ -> Nat::Succ { prev: bad_comm(prev, b) } } } }"),
        )
        .expect_err("a false comm-shaped claim should not prove");
        assert!(
            error.contains("no entailment tier judges yet")
                || error.contains("disproved structurally"),
            "expected a fence or refutation, got: {error}"
        );
    }

    #[test]
    fn nondescending_substate_recursion_rejected() {
        // The sub-state parameter is bound from `b` (not a payload of the
        // measure), so the self-call through it must reject.
        let error = validate(
            "machine bad(a: Nat, b: Nat) -> Nat \
             terminates by a; \
             ensures (add(a, b)) == (add(a, b)) { \
             transition a { Nat::Zero -> done(b) \
             Nat::Succ { prev } -> spin(b, b) } \
             state done(b: Nat) -> Nat { \
             transition { _ -> (b) } } \
             state spin(prev: Nat, b: Nat) -> Nat { \
             transition { _ -> Nat::Succ { prev: bad(prev, b) } } } }",
        )
        .expect_err("recursion through a non-descending sub-state parameter should reject");
        assert!(
            error.contains("cannot prove the measure"),
            "expected the descent error, got: {error}"
        );
    }

    /// Multiplication (the harness prelude only carries `add`).
    const MUL: &str = "machine mul(a: Nat, b: Nat) -> Nat \
         terminates by a; { \
         transition a { Nat::Zero -> Nat::Zero \
         Nat::Succ { prev } -> (add(b, mul(prev, b))) } }";

    /// Associativity: pure induction, no citations -- both sides normalize
    /// by unfolding plus the IH rewrite.
    const ADD_ASSOC: &str = "machine add_assoc(a: Nat, b: Nat, c: Nat) -> Nat \
         terminates by a; \
         ensures (add(add(a, b), c)) == (add(a, add(b, c))) { \
         transition a { Nat::Zero -> Nat::Zero \
         Nat::Succ { prev } -> Nat::Succ { prev: add_assoc(prev, b, c) } } }";

    const MUL_ZERO_RIGHT: &str = "machine mul_zero_right(a: Nat) -> Nat \
         terminates by a; \
         ensures (mul(a, Nat::Zero)) == Nat::Zero { \
         transition a { Nat::Zero -> Nat::Zero \
         Nat::Succ { prev } -> Nat::Succ { prev: mul_zero_right(prev) } } }";

    /// The right-successor law for mul. Its step case is the citation
    /// CHOREOGRAPHY stress: three reducing rewrites (comm, assoc, comm)
    /// normalize `add(b, add(prev, m))` onto the goal's other side --
    /// explicit Dafny-style proof text, no search.
    const MUL_SUCC_RIGHT: &str = "machine mul_succ_right(a: Nat, b: Nat) -> Nat \
         terminates by a; \
         ensures (mul(a, Nat::Succ { prev: b })) == (add(a, mul(a, b))) { \
         transition a { Nat::Zero -> Nat::Zero \
         Nat::Succ { prev } -> step_msr(prev, b) } \
         state step_msr(prev: Nat, b: Nat) -> Nat { \
         add_comm(b, add(prev, mul(prev, b))); \
         add_assoc(prev, mul(prev, b), b); \
         add_comm(mul(prev, b), b); \
         transition { _ -> Nat::Succ { prev: mul_succ_right(prev, b) } } } }";

    /// Multiplication commutes: base cites mul_zero_right, step cites
    /// mul_succ_right at (b, prev) plus the IH.
    const MUL_COMM: &str = "machine mul_comm(a: Nat, b: Nat) -> Nat \
         terminates by a; \
         ensures (mul(a, b)) == (mul(b, a)) { \
         transition a { Nat::Zero -> base_mc(b) \
         Nat::Succ { prev } -> step_mc(prev, b) } \
         state base_mc(b: Nat) -> Nat { \
         mul_zero_right(b); \
         transition { _ -> Nat::Zero } } \
         state step_mc(prev: Nat, b: Nat) -> Nat { \
         mul_succ_right(b, prev); \
         transition { _ -> Nat::Succ { prev: mul_comm(prev, b) } } } }";

    fn lemma_zoo() -> String {
        [
            RIGHT_ID_LAW,
            SUCC_LAW,
            ADD_COMM,
            MUL,
            ADD_ASSOC,
            MUL_ZERO_RIGHT,
            MUL_SUCC_RIGHT,
            MUL_COMM,
        ]
        .join(" ")
    }

    #[test]
    fn add_assoc_proves_without_citations() {
        validate(&(RIGHT_ID_LAW.to_owned() + " " + ADD_ASSOC))
            .expect("associativity should prove by pure induction");
    }

    #[test]
    fn mul_zero_right_proves() {
        validate(&(MUL.to_owned() + " " + MUL_ZERO_RIGHT))
            .expect("the right annihilator should prove by induction");
    }

    #[test]
    fn mul_succ_right_proves_with_citation_choreography() {
        validate(&lemma_zoo())
            .expect("the right-successor law should prove via comm/assoc/comm citations");
    }

    #[test]
    fn mul_comm_proves() {
        validate(&lemma_zoo()).expect("mul_comm should prove citing the mul laws");
    }

    #[test]
    fn mul_succ_right_without_choreography_fences() {
        // Dropping the three rearrangement citations strands the step case:
        // the IH alone cannot bridge the summand order. Since rearrange-mode
        // (2026-07-18) this test ALSO pins the license discipline: the comm
        // and assoc lemmas are IN SCOPE in this very program, but no
        // CommutativeSemiring conformance is declared, so the rearrange tier
        // must NOT fire -- explicit conformance, never scope-sniffing.
        let uncited = MUL_SUCC_RIGHT
            .replace("add_comm(b, add(prev, mul(prev, b))); ", "")
            .replace("add_assoc(prev, mul(prev, b), b); ", "")
            .replace("add_comm(mul(prev, b), b); ", "");
        let error = validate(
            &(RIGHT_ID_LAW.to_owned()
                + " "
                + SUCC_LAW
                + " "
                + ADD_COMM
                + " "
                + MUL
                + " "
                + ADD_ASSOC
                + " "
                + MUL_ZERO_RIGHT
                + " "
                + &uncited),
        )
        .expect_err("the step case without its choreography should fence");
        assert!(
            error.contains("no entailment tier judges yet"),
            "expected the N3 fence, got: {error}"
        );
    }

    #[test]
    fn seq_append_laws_prove() {
        // Right identity and associativity of append -- both pure
        // inductions on the first sequence, no citations.
        validate(
            "data Seq { case Empty; case Cons(head: Nat, tail: Seq); } \
             machine append(s: Seq, t: Seq) -> Seq terminates by s; { \
             transition s { Seq::Empty -> (t) \
             Seq::Cons { head, tail } -> Seq::Cons { head: head, tail: append(tail, t) } } } \
             machine append_empty_right(s: Seq) -> Seq terminates by s; \
             ensures (append(s, Seq::Empty)) == s { \
             transition s { Seq::Empty -> Seq::Empty \
             Seq::Cons { head, tail } -> Seq::Cons { head: head, tail: append_empty_right(tail) } } } \
             machine append_assoc(s: Seq, t: Seq, u: Seq) -> Seq terminates by s; \
             ensures (append(append(s, t), u)) == (append(s, append(t, u))) { \
             transition s { Seq::Empty -> Seq::Empty \
             Seq::Cons { head, tail } -> Seq::Cons { head: head, tail: append_assoc(tail, t, u) } } }",
        )
        .expect("the append laws should prove by induction");
    }

    #[test]
    fn seq_length_append_proves() {
        // The Seq zoo's first COMPOSED law: length distributes over append,
        // by induction on the first sequence -- generic recursive data,
        // cross-machine unfolding (append inside length inside add), the
        // IH as a rewrite. No citations needed: both sides normalize.
        validate(
            "data Seq { case Empty; case Cons(head: Nat, tail: Seq); } \
             machine length(s: Seq) -> Nat terminates by s; { \
             transition s { Seq::Empty -> Nat::Zero \
             Seq::Cons { head, tail } -> Nat::Succ { prev: length(tail) } } } \
             machine append(s: Seq, t: Seq) -> Seq terminates by s; { \
             transition s { Seq::Empty -> (t) \
             Seq::Cons { head, tail } -> Seq::Cons { head: head, tail: append(tail, t) } } } \
             machine length_append(s: Seq, t: Seq) -> Nat terminates by s; \
             ensures (length(append(s, t))) == (add(length(s), length(t))) { \
             transition s { Seq::Empty -> Nat::Zero \
             Seq::Cons { head, tail } -> Nat::Succ { prev: length_append(tail, t) } } }",
        )
        .expect("length_append should prove by induction");
    }

    #[test]
    fn citation_with_wrong_operands_does_not_prove_the_goal() {
        // The citation instantiates at ITS operands only -- citing the law
        // at `b` says nothing about the unrelated `c`. (Citing at `b` DOES
        // serve goals one compute step away, like `add(Succ(b), Zero) ==
        // Succ(b)` -- the Succ arm unfolds and exposes exactly the cited
        // instance; that is sound derivation, not leakage.)
        let error = validate(
            &(RIGHT_ID_LAW.to_owned()
                + " machine wrong_operand(b: Nat, c: Nat) \
               ensures (add(c, Nat::Zero)) == c { \
               right_id(b); }"),
        )
        .expect_err("a citation at the wrong operands should not discharge the goal");
        assert!(
            error.contains("no entailment tier judges yet"),
            "expected the N3 fence, got: {error}"
        );
    }
}

mod provider_plan {
    use effects::build_boundary_provider_approval_registry;
    use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use tokens_to_syntax_trees::parse_syntax_trees;

    fn typed(source: &str) -> typed_trees::TypedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        lower_symbol_resolved_trees(&resolved).expect("typed lowering")
    }

    #[test]
    fn service_schema_reifies_a_boundary_trait() {
        // PRV2: the schema derives from the typed boundary TraitDefinition
        // -- names, parameter counts (receiver excluded), result presence,
        // canonical service reach, and independent operational ceilings.
        let program = typed(
            "boundary trait Console {\n\
             machine write_line(text: &[u8])\n\
             reaches\n\
                 Console\n\
             suspends;\n\
             blocks;\n\
             machine exit_process(return_code: i32);\n\
             }\n\
             data Main { console: Console; }\n\
             machine Main::main(&mut self) {}\n",
        );
        let console = program
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "Console")
            .expect("Console trait");
        let schema =
            ServiceSchema::from_typed(&program, console).expect("boundary trait has a schema");
        assert_eq!(schema.trait_name, "Console");
        assert_eq!(schema.methods.len(), 2);
        assert_eq!(schema.methods[0].name, "write_line");
        assert_eq!(schema.methods[0].parameter_count, 1);
        assert!(!schema.methods[0].has_result);
        assert_eq!(schema.methods[0].service_reach, vec!["Console".to_owned()]);
        assert!(schema.methods[0].may_suspend);
        assert!(schema.methods[0].may_block);
        assert_eq!(schema.methods[1].name, "exit_process");
        assert_eq!(schema.methods[1].service_reach, vec!["Console".to_owned()]);
        assert!(!schema.methods[1].may_suspend);
        assert!(!schema.methods[1].may_block);
    }

    #[test]
    fn trait_parents_expand_requirements_without_laundering_policy_into_service_reach() {
        let program = typed(
            "trait Calling<C> {\n\
             machine policy_probe();\n\
             }\n\
             data X64Convention {}\n\
             boundary trait Device {\n\
             machine read()\n\
             reaches\n\
                 Device;\n\
             }\n\
             boundary trait Timer: Device + Calling<X64Convention> {\n\
             machine tick();\n\
             }\n\
             data Main {}\n\
             machine Main::main(&mut self) {}\n",
        );
        let timer = program
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "Timer")
            .expect("Timer trait");

        let parents = program.trait_requirements(timer);
        assert_eq!(parents.len(), 2);
        assert_eq!(
            program.trait_composition_kind(&parents[0]),
            Some(typed_trees::trait_definition::TraitCompositionKind::ServiceReach)
        );
        assert_eq!(
            program.trait_composition_kind(&parents[1]),
            Some(typed_trees::trait_definition::TraitCompositionKind::Policy)
        );

        let schema = ServiceSchema::from_typed(&program, timer).expect("boundary schema");
        assert_eq!(
            schema
                .methods
                .iter()
                .map(|method| method.name.as_str())
                .collect::<Vec<_>>(),
            ["read", "policy_probe", "tick"]
        );
        let method = |name: &str| {
            schema
                .methods
                .iter()
                .find(|method| method.name == name)
                .expect("schema method")
        };
        assert_eq!(method("read").service_reach, ["Device"]);
        assert!(
            method("policy_probe").service_reach.is_empty(),
            "ordinary policy parents must not mint service identities",
        );
        assert_eq!(method("tick").service_reach, ["Device", "Timer"]);

        let registry = build_boundary_provider_approval_registry(&program);
        let timer_provider = registry.provider(timer.symbol).expect("Timer provider");
        assert!(timer_provider.approved);
    }

    #[test]
    fn fingerprint_is_presentation_invariant() {
        // PRV2: identity excludes presentation -- reordering rows keeps the
        // fingerprint; changing a binding changes it.
        let schema = ServiceSchema {
            trait_name: "Console".to_owned(),
            trait_package_identity: None,
            methods: Vec::new(),
        };
        let row = |method: &str, number: i64| ProviderPlanRow {
            method: method.to_owned(),
            requirement_identity: String::new(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::Syscall { number },
        };
        let plan = |rows: Vec<ProviderPlanRow>| ProviderPlan {
            name: "p".to_owned(),
            provider_type: "P".to_owned(),
            provider_type_package_identity: None,
            target: "t".to_owned(),
            schema: schema.clone(),
            rows,
            origin_package_identity: None,
            origin_package: "omega::language::std".to_owned(),
        };
        let forward = plan(vec![row("a", 1), row("b", 2)]);
        let reversed = plan(vec![row("b", 2), row("a", 1)]);
        assert_eq!(forward.report_fingerprint(), reversed.report_fingerprint());
        let changed = plan(vec![row("a", 9), row("b", 2)]);
        assert_ne!(forward.report_fingerprint(), changed.report_fingerprint());
    }
}
