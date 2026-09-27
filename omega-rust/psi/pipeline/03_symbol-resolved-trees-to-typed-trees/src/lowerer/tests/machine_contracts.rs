#[test]
fn typed_snapshots_publish_only_normalized_service_reach() {
    let source = r#"
        boundary trait Console {
            machine write_line(text: &[u8])
            reaches Console;
        }

        machine emit(text: &[u8])
        reaches Console
        {
        }
    "#;
    let typed = crate::front_end::typed_program(source);
    let snapshot = typed.snapshot();
    let [machine] = snapshot.roots.machines.as_slice() else {
        panic!("one typed machine snapshot");
    };

    assert_eq!(machine.service_reach, ["Console"]);
    let [trait_definition] = snapshot.roots.traits.as_slice() else {
        panic!("one typed trait snapshot");
    };
    let [signature] = trait_definition.machines.as_slice() else {
        panic!("one typed trait-machine snapshot");
    };
    assert_eq!(signature.service_reach, ["Console"]);
    assert!(!signature.service_reach_is_installation_bound);
}

#[test]
fn authored_premise_through_a_generic_leaf_still_names_a_declared_member() {
    // The application replays its own declaration only: `scheduler` is a
    // `Context` member, not a `Box` member, so this premise still has no
    // exact field path and must not mint one.
    for requirement in [
        "requires b.scheduler in WeakFair",
        "requires b.item.missing in WeakFair",
    ] {
        let source = format!(
            r#"
            data Main {{}}
            machine Main::run(&mut self) {{}}
            pub data SchedulerHandle [copy] {{}}
            pub data Context {{ scheduler: SchedulerHandle; }}
            pub data Box<T> {{ item: T; }}
            pub domain SchedulerHandle::WeakFair
            satisfies ProgressProfile
            established by SchedulerAdmission::grant;
            pub boundary trait SchedulerAdmission {{
                machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
            }}
            pub machine wait_boxed(b: &Box<Context>)
            {requirement}
            terminates;
            -> u64 {{ 0 }}
        "#
        );
        let diagnostic = crate::front_end::typed_program_result(&source)
            .expect_err("a non-member premise must still fail normalization");
        assert!(
            diagnostic
                .message
                .contains("must name one identity-preserving parameter or field path"),
            "{requirement}: {}",
            diagnostic.message
        );
    }
}

#[test]
fn typed_snapshot_publishes_normalized_termination_witness() {
    let source = r#"
        machine countdown(remaining: u64)
        terminates by remaining -> Nat::Descending;
        {
        }
    "#;
    let typed = crate::front_end::typed_program(source);
    let snapshot = typed.snapshot();
    let [machine] = snapshot.roots.machines.as_slice() else {
        panic!("one typed machine snapshot");
    };
    let witness = machine
        .termination_witness
        .as_ref()
        .expect("normalized ranking witness");

    assert_eq!(witness.subjects, ["remaining"]);
    assert_eq!(witness.view_path, "Nat::Descending");
    assert!(witness.view_arguments.is_empty());
    assert!(witness.rank_range.is_none());
}

#[test]
fn typed_snapshot_retains_trait_owned_operator_token() {
    let source = r#"
        trait Ranked<T> {
            operator < compare(left: T, right: T) -> bool;
        }
    "#;
    let typed = crate::front_end::typed_program(source);
    let snapshot = typed.snapshot();
    let [trait_definition] = snapshot.roots.traits.as_slice() else {
        panic!("one trait snapshot expected");
    };
    let [requirement] = trait_definition.machines.as_slice() else {
        panic!("one trait requirement expected");
    };

    assert_eq!(requirement.spelling, Some("<"));
}

#[test]
fn proof_fact_indexed_application_rejects_a_wrong_argument_count() {
    let source = r#"
        data Extent {}
        data Slot {}
        data SlotPlacement {}
        domain<P, T> Extent::Resident<P, T>;

        trait ResidentStorage {
            machine place(storage: Extent) -> Extent
            ensures result in Resident<SlotPlacement>;
        }
    "#;
    let error = crate::front_end::typed_program_result(source)
        .expect_err("one argument cannot apply a two-index family");
    assert!(
        error.message.contains(
            "domain family `Resident` requires 2 closed index argument(s), but 1 were supplied"
        ),
        "{}",
        error.message
    );
}
