//! Stack realization tests: arrival contexts, entry stack epochs and closures.

use super::{
    ArrivalContextId, ArrivalContextRealization, ArrivalContextStackDomain, EntryStack,
    EntryStackEpoch, EntryStackRealization, EntryStackStage, InstalledEntryFactIdentity,
    Preemption, StackDomainRef, StackOccupancy, X86_64ArrivalMechanism, X86_64GateKind,
    X86_64HardwareStackSelection, X86_64InstalledArrivalContext, X86_64InstalledHardwareEntryFacts,
    X86_64TargetProfileIdentity, derive_x86_64_hardware_arrival,
    validate_entry_stack_domain_closure, validate_entry_stack_realization,
    validate_x86_64_installed_hardware_entry_facts,
};

fn installed_identity() -> InstalledEntryFactIdentity {
    InstalledEntryFactIdentity {
        target_profile: X86_64TargetProfileIdentity::LONG_MODE_INTERRUPT_GATES,
        artifact: 0x20,
        installed_code: 0x30,
        entry: 0x31,
        entry_offset: 0,
        boundary_plan_report_fingerprint: 0x40,
        boundary_plan_commitment: [0x40; 32],
    }
}

fn x86_context(
    id: u64,
    interrupted_privilege: u8,
    entry_privilege: u8,
    stack_selection: X86_64HardwareStackSelection,
    mechanism: X86_64ArrivalMechanism,
) -> X86_64InstalledArrivalContext {
    X86_64InstalledArrivalContext {
        context: ArrivalContextId::new(id).expect("context identity"),
        mechanism,
        interrupted_privilege,
        entry_privilege,
        stack_selection,
        nesting: Preemption::Nestable { maximum_depth: 2 },
    }
}

fn x86_facts(
    vector: u8,
    boundary_stack: EntryStack,
    contexts: Vec<X86_64InstalledArrivalContext>,
) -> X86_64InstalledHardwareEntryFacts {
    X86_64InstalledHardwareEntryFacts {
        identity: installed_identity(),
        vector,
        gate: X86_64GateKind::Interrupt,
        boundary_stack,
        contexts,
    }
}

fn context(value: u64, occupancy: Vec<StackOccupancy>) -> ArrivalContextRealization {
    ArrivalContextRealization {
        context: ArrivalContextId::new(value).expect("context identity"),
        epochs: vec![EntryStackEpoch {
            stage: EntryStackStage::Body,
            active_domain: StackDomainRef::Interrupted,
            occupancy_by_domain: occupancy,
            nesting: Preemption::Nestable { maximum_depth: 2 },
        }],
    }
}

#[test]
fn canonical_identity_ignores_context_and_occupancy_input_order() {
    let interrupted = StackOccupancy {
        domain: StackDomainRef::Interrupted,
        bytes: 24,
        alignment: 8,
    };
    let dedicated = StackOccupancy {
        domain: StackDomainRef::Dedicated { class: 7 },
        bytes: 16,
        alignment: 16,
    };
    let first = validate_entry_stack_realization(EntryStackRealization {
        contexts: vec![
            context(2, vec![interrupted, dedicated]),
            context(1, vec![dedicated, interrupted]),
        ],
    })
    .expect("first realization");
    let second = validate_entry_stack_realization(EntryStackRealization {
        contexts: vec![
            context(1, vec![interrupted, dedicated]),
            context(2, vec![dedicated, interrupted]),
        ],
    })
    .expect("second realization");

    assert_eq!(first, second);
    assert_eq!(first.report_fingerprint(), second.report_fingerprint());
}

#[test]
fn structural_validation_rejects_open_or_malformed_realizations() {
    let valid_context = context(
        1,
        vec![StackOccupancy {
            domain: StackDomainRef::Interrupted,
            bytes: 8,
            alignment: 8,
        }],
    );
    let cases = [
        (
            EntryStackRealization::default(),
            "no admissible arrival context",
        ),
        (
            EntryStackRealization {
                contexts: vec![valid_context.clone(), valid_context.clone()],
            },
            "repeats an arrival-context identity",
        ),
        (
            EntryStackRealization {
                contexts: vec![ArrivalContextRealization {
                    context: ArrivalContextId::new(1).expect("identity"),
                    epochs: vec![EntryStackEpoch {
                        stage: EntryStackStage::Body,
                        active_domain: StackDomainRef::ProviderSelected,
                        occupancy_by_domain: Vec::new(),
                        nesting: Preemption::Masked,
                    }],
                }],
            },
            "unresolved provider-selected active stack domain",
        ),
        (
            EntryStackRealization {
                contexts: vec![ArrivalContextRealization {
                    context: ArrivalContextId::new(1).expect("identity"),
                    epochs: vec![EntryStackEpoch {
                        stage: EntryStackStage::Body,
                        active_domain: StackDomainRef::Interrupted,
                        occupancy_by_domain: Vec::new(),
                        nesting: Preemption::ProviderDefined,
                    }],
                }],
            },
            "unresolved provider-defined nesting",
        ),
    ];
    for (realization, expected) in cases {
        let error = validate_entry_stack_realization(realization)
            .expect_err("malformed realization must reject");
        assert!(error.0.contains(expected), "{}", error.0);
    }
}

#[test]
fn context_domain_closure_is_exact_and_fixed_stacks_cannot_drift() {
    let context = |id, domain| ArrivalContextStackDomain {
        context: ArrivalContextId::new(id).expect("context identity"),
        domain,
    };
    let selected = validate_entry_stack_domain_closure(
        EntryStack::ProviderSelected,
        vec![
            context(2, StackDomainRef::Dedicated { class: 3 }),
            context(1, StackDomainRef::Interrupted),
        ],
    )
    .expect("provider-selected domains may vary by context");
    assert_eq!(selected.contexts()[0].context.get(), 1);
    assert_eq!(selected.contexts()[1].context.get(), 2);

    let error = validate_entry_stack_domain_closure(
        EntryStack::Interrupted,
        vec![context(1, StackDomainRef::Dedicated { class: 3 })],
    )
    .expect_err("fixed interrupted stack cannot close to a dedicated domain");
    assert!(error.0.contains("differs from the fixed boundary"));
}

#[test]
fn epoch_order_and_body_cardinality_are_closed() {
    let epoch = |stage| EntryStackEpoch {
        stage,
        active_domain: StackDomainRef::Interrupted,
        occupancy_by_domain: Vec::new(),
        nesting: Preemption::Masked,
    };
    let wrong_order = EntryStackRealization {
        contexts: vec![ArrivalContextRealization {
            context: ArrivalContextId::new(1).expect("identity"),
            epochs: vec![epoch(EntryStackStage::Body), epoch(EntryStackStage::Enter)],
        }],
    };
    assert!(
        validate_entry_stack_realization(wrong_order)
            .expect_err("enter after body must reject")
            .0
            .contains("noncanonical")
    );

    let two_bodies = EntryStackRealization {
        contexts: vec![ArrivalContextRealization {
            context: ArrivalContextId::new(1).expect("identity"),
            epochs: vec![epoch(EntryStackStage::Body), epoch(EntryStackStage::Body)],
        }],
    };
    assert!(
        validate_entry_stack_realization(two_bodies)
            .expect_err("two body epochs must reject")
            .0
            .contains("noncanonical")
    );
}

#[test]
fn x86_same_privilege_arrival_is_derived_on_the_interrupted_stack() {
    for (vector, mechanism, expected_bytes) in [
        (14, X86_64ArrivalMechanism::SoftwareInterrupt, 24),
        (14, X86_64ArrivalMechanism::Exception, 32),
    ] {
        let installed = validate_x86_64_installed_hardware_entry_facts(x86_facts(
            vector,
            EntryStack::Interrupted,
            vec![x86_context(
                1,
                0,
                0,
                X86_64HardwareStackSelection::Current,
                mechanism,
            )],
        ))
        .expect("same-CPL installation facts");
        let derived =
            derive_x86_64_hardware_arrival(&installed).expect("sealed same-CPL target derivation");
        let context = &derived.realization().realization().contexts[0];

        assert_eq!(context.epochs.len(), 1);
        assert_eq!(context.epochs[0].stage, EntryStackStage::Body);
        assert_eq!(context.epochs[0].active_domain, StackDomainRef::Interrupted);
        assert_eq!(
            context.epochs[0].occupancy_by_domain,
            vec![StackOccupancy {
                domain: StackDomainRef::Interrupted,
                bytes: expected_bytes,
                alignment: 8,
            }]
        );
        assert_eq!(
            derived.body_domains().contexts(),
            &[ArrivalContextStackDomain {
                context: ArrivalContextId::new(1).expect("identity"),
                domain: StackDomainRef::Interrupted,
            }]
        );
    }
}

#[test]
fn x86_privilege_and_ist_switches_derive_complete_dedicated_frames() {
    for (stack_selection, vector, mechanism, expected_bytes) in [
        (
            X86_64HardwareStackSelection::PrivilegeTransition { dedicated_class: 7 },
            32,
            X86_64ArrivalMechanism::ExternalInterrupt,
            40,
        ),
        (
            X86_64HardwareStackSelection::PrivilegeTransition { dedicated_class: 7 },
            14,
            X86_64ArrivalMechanism::Exception,
            48,
        ),
        (
            X86_64HardwareStackSelection::InterruptStackTable {
                slot: 3,
                dedicated_class: 7,
            },
            32,
            X86_64ArrivalMechanism::ExternalInterrupt,
            40,
        ),
        (
            X86_64HardwareStackSelection::InterruptStackTable {
                slot: 3,
                dedicated_class: 7,
            },
            14,
            X86_64ArrivalMechanism::Exception,
            48,
        ),
    ] {
        let (interrupted_privilege, entry_privilege) = match stack_selection {
            X86_64HardwareStackSelection::PrivilegeTransition { .. } => (3, 0),
            X86_64HardwareStackSelection::InterruptStackTable { .. } => (0, 0),
            X86_64HardwareStackSelection::Current => unreachable!(),
        };
        let installed = validate_x86_64_installed_hardware_entry_facts(x86_facts(
            vector,
            EntryStack::Dedicated { class: 7 },
            vec![x86_context(
                1,
                interrupted_privilege,
                entry_privilege,
                stack_selection,
                mechanism,
            )],
        ))
        .expect("hardware-switched installation facts");
        let derived = derive_x86_64_hardware_arrival(&installed)
            .expect("sealed hardware-switch target derivation");
        let epoch = &derived.realization().realization().contexts[0].epochs[0];

        assert_eq!(epoch.active_domain, StackDomainRef::Dedicated { class: 7 });
        assert_eq!(epoch.occupancy_by_domain[0].bytes, expected_bytes);
    }
}

#[test]
fn x86_mixed_contexts_are_canonical_and_context_complete() {
    let same = x86_context(
        1,
        0,
        0,
        X86_64HardwareStackSelection::Current,
        X86_64ArrivalMechanism::Exception,
    );
    let transition = x86_context(
        2,
        3,
        0,
        X86_64HardwareStackSelection::PrivilegeTransition { dedicated_class: 9 },
        X86_64ArrivalMechanism::Exception,
    );
    let first = validate_x86_64_installed_hardware_entry_facts(x86_facts(
        14,
        EntryStack::ProviderSelected,
        vec![transition, same],
    ))
    .expect("mixed installed contexts");
    let second = validate_x86_64_installed_hardware_entry_facts(x86_facts(
        14,
        EntryStack::ProviderSelected,
        vec![same, transition],
    ))
    .expect("same mixed contexts in canonical order");

    assert_eq!(first, second);
    assert_eq!(first.report_fingerprint(), second.report_fingerprint());
    let derived = derive_x86_64_hardware_arrival(&first).expect("mixed derivation");
    assert_eq!(derived.realization().realization().contexts.len(), 2);
    assert_eq!(
        derived
            .realization()
            .realization()
            .contexts
            .iter()
            .flat_map(|context| &context.epochs)
            .flat_map(|epoch| &epoch.occupancy_by_domain)
            .map(|occupancy| occupancy.bytes)
            .max(),
        Some(48),
        "mutually exclusive same-CPL/transition contexts contribute their maximum arrival frame",
    );
    assert_eq!(
        derived.body_domains().contexts(),
        &[
            ArrivalContextStackDomain {
                context: ArrivalContextId::new(1).expect("identity"),
                domain: StackDomainRef::Interrupted,
            },
            ArrivalContextStackDomain {
                context: ArrivalContextId::new(2).expect("identity"),
                domain: StackDomainRef::Dedicated { class: 9 },
            },
        ]
    );
}

#[test]
fn x86_installation_validation_fails_closed_on_invalid_exact_facts() {
    let current = x86_context(
        1,
        0,
        0,
        X86_64HardwareStackSelection::Current,
        X86_64ArrivalMechanism::ExternalInterrupt,
    );
    let mut absent_identity = x86_facts(32, EntryStack::Interrupted, vec![current]);
    absent_identity.identity.artifact = 0;
    let mut absent_boundary_commitment = x86_facts(32, EntryStack::Interrupted, vec![current]);
    absent_boundary_commitment.identity.boundary_plan_commitment = [0; 32];
    let duplicate = x86_facts(32, EntryStack::Interrupted, vec![current, current]);
    let reserved_external = x86_facts(
        14,
        EntryStack::Interrupted,
        vec![x86_context(
            1,
            0,
            0,
            X86_64HardwareStackSelection::Current,
            X86_64ArrivalMechanism::ExternalInterrupt,
        )],
    );
    let bad_privilege = x86_facts(
        32,
        EntryStack::Interrupted,
        vec![x86_context(
            1,
            4,
            0,
            X86_64HardwareStackSelection::Current,
            X86_64ArrivalMechanism::ExternalInterrupt,
        )],
    );
    let missing_switch = x86_facts(
        32,
        EntryStack::ProviderSelected,
        vec![x86_context(
            1,
            3,
            0,
            X86_64HardwareStackSelection::Current,
            X86_64ArrivalMechanism::ExternalInterrupt,
        )],
    );
    let fake_transition = x86_facts(
        32,
        EntryStack::Dedicated { class: 2 },
        vec![x86_context(
            1,
            0,
            0,
            X86_64HardwareStackSelection::PrivilegeTransition { dedicated_class: 2 },
            X86_64ArrivalMechanism::ExternalInterrupt,
        )],
    );
    let bad_ist = x86_facts(
        32,
        EntryStack::Dedicated { class: 2 },
        vec![x86_context(
            1,
            0,
            0,
            X86_64HardwareStackSelection::InterruptStackTable {
                slot: 0,
                dedicated_class: 2,
            },
            X86_64ArrivalMechanism::ExternalInterrupt,
        )],
    );
    let fixed_domain_drift = x86_facts(
        32,
        EntryStack::Interrupted,
        vec![x86_context(
            1,
            0,
            0,
            X86_64HardwareStackSelection::InterruptStackTable {
                slot: 1,
                dedicated_class: 2,
            },
            X86_64ArrivalMechanism::ExternalInterrupt,
        )],
    );

    for (facts, expected) in [
        (absent_identity, "absent exact identity"),
        (absent_boundary_commitment, "absent exact identity"),
        (duplicate, "repeats an arrival-context identity"),
        (reserved_external, "classifies reserved vector"),
        (bad_privilege, "outside 0..=3"),
        (missing_switch, "without a hardware stack switch"),
        (fake_transition, "without changing privilege"),
        (bad_ist, "invalid IST slot"),
        (fixed_domain_drift, "differs from the fixed boundary"),
    ] {
        let error = validate_x86_64_installed_hardware_entry_facts(facts)
            .expect_err("invalid x86 installation facts must reject");
        assert!(error.0.contains(expected), "{}", error.0);
    }
}

#[test]
fn x86_derived_identity_binds_every_exact_installation_fact_and_revalidates() {
    let facts = x86_facts(
        32,
        EntryStack::Interrupted,
        vec![x86_context(
            1,
            0,
            0,
            X86_64HardwareStackSelection::Current,
            X86_64ArrivalMechanism::ExternalInterrupt,
        )],
    );
    let installed =
        validate_x86_64_installed_hardware_entry_facts(facts.clone()).expect("baseline facts");
    let baseline = derive_x86_64_hardware_arrival(&installed).expect("baseline derivation");

    let mut changed = facts;
    changed.gate = X86_64GateKind::Trap;
    let changed =
        validate_x86_64_installed_hardware_entry_facts(changed).expect("changed exact gate facts");
    let changed = derive_x86_64_hardware_arrival(&changed).expect("changed derivation");
    assert_ne!(baseline.report_fingerprint(), changed.report_fingerprint());

    let mut tampered = installed;
    tampered.facts.vector = 33;
    let error = derive_x86_64_hardware_arrival(&tampered)
        .expect_err("post-validation fact tampering must fail closed");
    assert!(error.0.contains("canonical identity revalidation"));
}
