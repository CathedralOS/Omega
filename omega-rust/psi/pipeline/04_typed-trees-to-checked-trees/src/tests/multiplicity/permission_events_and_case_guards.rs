use crate::tests::front_end::{checked_program, checked_program_result};

#[test]
fn retains_canonical_semantic_permission_events() {
    let checked = checked_program(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Box { value: i32; }
        data Main {}

        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let forwarded: Receipt = issued;
            Receipt::ack(forwarded);
            let affine: Box = Box { value: 1 };
            0
        }
        "#,
    );

    use language_semantics::PermissionEventKind as Kind;
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let kinds = events.iter().map(|event| event.kind).collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            Kind::Establish,
            Kind::Transfer,
            Kind::Establish,
            Kind::Consume,
            Kind::Establish,
            Kind::AffineDrop,
        ]
    );
    assert!(
        events
            .iter()
            .all(|event| event.access == language_semantics::PermissionAccess::Owned)
    );
    assert!(
        events[..4]
            .iter()
            .all(|event| event.multiplicity == language_semantics::Multiplicity::Linear)
    );
    assert!(
        events[4..]
            .iter()
            .all(|event| event.multiplicity == language_semantics::Multiplicity::Affine)
    );
    let origin = events[0].provenance;
    let claim_identity = events[0].claim_identity;
    assert_ne!(origin, language_semantics::PermissionProvenance::Unknown);
    assert_ne!(
        claim_identity,
        language_semantics::PermissionClaimIdentity::Unknown
    );
    assert!(
        events[..4].iter().all(|event| event.provenance == origin),
        "transfers preserve one obligation origin rather than minting a new one per binding"
    );
    assert!(
        events[..4]
            .iter()
            .all(|event| event.claim_identity == claim_identity),
        "transfers preserve one claim identity rather than minting a new claim per binding"
    );
    assert_eq!(
        events[5].provenance, events[4].provenance,
        "affine cleanup must preserve its establishment provenance"
    );
    assert_eq!(
        events[5].claim_identity, events[4].claim_identity,
        "affine cleanup must preserve its established claim identity"
    );
}

#[test]
fn method_form_by_value_self_records_terminal_consume() {
    let checked = checked_program(
        r#"
        pub data Receipt [linear] { code: i32; }
        boundary machine Receipt::complete(self) {}
        data Main {}

        machine Main::run(receipt: Receipt) {
            receipt.complete();
        }
        "#,
    );

    use language_semantics::{PermissionAccess, PermissionEventKind, PermissionEventSource};
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| event.access == PermissionAccess::Owned)
        .collect::<Vec<_>>();
    assert_eq!(
        events.len(),
        2,
        "one establishment and one consume: {events:#?}"
    );
    assert_eq!(events[0].kind, PermissionEventKind::Establish);
    assert_eq!(events[1].kind, PermissionEventKind::Consume);
    assert!(matches!(
        events[1].source,
        PermissionEventSource::Call { .. }
    ));
    assert_eq!(events[1].claim_identity, events[0].claim_identity);
    assert_eq!(events[1].provenance, events[0].provenance);
}

#[test]
fn explicit_crash_retains_definitely_live_linear_frontier_without_cleanup() {
    let checked = checked_program(
        r#"
        data Receipt [linear] { code: i32; }
        data MaybeReceipt {
            case Empty;
            case Live(receipt: Receipt);
        }

        machine abandon(receipt: Receipt) -> i32
        crashes Abort
        {
            crash Abort;
        }

        machine abandon_maybe(receipt: MaybeReceipt) -> i32
        crashes Abort
        {
            crash Abort;
        }
        "#,
    );

    let crash_site = |machine_name: &str| {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .expect("crashing machine");
        let [site] = checked
            .facts
            .contract_plans
            .for_machine(machine.symbol)
            .expect("machine contract plan")
            .crash
            .checked_sites()
        else {
            panic!("crashing machine should have one checked crash site")
        };
        site
    };

    let [claim] = crash_site("abandon").frontier_lower_bound() else {
        panic!("the definitely-live receipt must enter the crash frontier")
    };
    assert_ne!(*claim, language_semantics::PermissionClaimIdentity::Unknown);
    assert!(
        checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .any(|(_, event)| event.claim_identity == *claim
                && event.kind == language_semantics::PermissionEventKind::Establish
                && event.source == language_semantics::PermissionEventSource::StateEntry)
    );
    assert!(
        !checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .any(|(_, event)| event.claim_identity == *claim
                && matches!(
                    event.kind,
                    language_semantics::PermissionEventKind::Consume
                        | language_semantics::PermissionEventKind::Transfer
                )),
        "crash records abandonment; it must not invent cleanup or consumption"
    );
    assert!(
        crash_site("abandon_maybe")
            .frontier_lower_bound()
            .is_empty(),
        "an unknown active sum case is not a definitely-live lower-bound claim"
    );
}

#[test]
fn multi_hop_case_guard_promotes_only_the_proven_conditional_crash_claim() {
    let checked = checked_program(
        r#"
        data Receipt [linear] { code: i32; }
        data MaybeReceipt {
            case Empty;
            case Live(receipt: Receipt);
        }

        machine route(choice: MaybeReceipt) -> i32
        crashes Abort
        {
            transition choice {
                MaybeReceipt::Live -> relay(choice)
                MaybeReceipt::Empty -> 0
            }

            state relay(pending: MaybeReceipt) -> i32 {
                transition { _ -> crash_live(pending) }
            }

            state crash_live(choice: MaybeReceipt) -> i32 {
                crash Abort;
            }
        }
        "#,
    );

    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "route")
        .expect("routing machine");
    let plan = &checked
        .facts
        .contract_plans
        .for_machine(machine.symbol)
        .expect("machine contract plan")
        .crash;
    let frontier_for = |state_name: &str| {
        let state = checked
            .machine_states(machine)
            .iter()
            .find(|state| state.name.as_str() == state_name)
            .expect("crash state");
        let site = plan
            .checked_sites()
            .iter()
            .find(|site| site.location().state() == state.symbol)
            .expect("checked crash site");
        site.frontier_lower_bound()
    };

    let [live_claim] = frontier_for("crash_live") else {
        panic!("the live-case guard should prove exactly one payload claim")
    };

    let claim_variant = |claim: &language_semantics::PermissionClaimIdentity| {
        checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .find(|event| {
                event.claim_identity == *claim
                    && event.kind == language_semantics::PermissionEventKind::Establish
                    && event.source == language_semantics::PermissionEventSource::StateEntry
            })
            .and_then(|event| {
                checked
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments)
                    .iter()
                    .find_map(|segment| match segment {
                        facts::PlaceSegment::Case { variant } => Some(*variant),
                        _ => None,
                    })
            })
            .expect("frontier claim should retain its case path")
    };
    let live_variant = checked
        .data_definitions()
        .iter()
        .flat_map(|definition| checked.data_members(definition))
        .find_map(|member| match member {
            typed_trees::data::DataMember::Variant(variant) if variant.name.as_str() == "Live" => {
                Some(variant.symbol)
            }
            _ => None,
        })
        .expect("Live variant");
    assert_eq!(claim_variant(live_claim), live_variant);
}

#[test]
fn common_case_guard_parameter_map_survives_a_diamond_join() {
    let checked = checked_program(
        r#"
        data Receipt [linear] { code: i32; }
        data MaybeReceipt {
            case Empty;
            case Live(receipt: Receipt);
        }

        machine route(choice: MaybeReceipt, branch: bool) -> i32
        crashes Abort
        {
            transition choice {
                MaybeReceipt::Live -> split(choice, branch)
                MaybeReceipt::Empty -> 0
            }

            state split(pending: MaybeReceipt, branch: bool) -> i32 {
                transition branch {
                    true -> left(pending)
                    _ -> right(pending)
                }
            }

            state left(pending: MaybeReceipt) -> i32 {
                transition { _ -> crash_live(pending) }
            }

            state right(pending: MaybeReceipt) -> i32 {
                transition { _ -> crash_live(pending) }
            }

            state crash_live(choice: MaybeReceipt) -> i32 {
                crash Abort;
            }
        }
        "#,
    );

    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "route")
        .expect("routing machine");
    let crash_state = checked
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == "crash_live")
        .expect("joined crash state");
    let site = checked
        .facts
        .contract_plans
        .for_machine(machine.symbol)
        .expect("machine contract plan")
        .crash
        .checked_sites()
        .iter()
        .find(|site| site.location().state() == crash_state.symbol)
        .expect("checked crash site");
    assert_eq!(
        site.frontier_lower_bound().len(),
        1,
        "the identical composed case proof on both diamond edges must survive the meet"
    );
}

#[test]
fn nested_case_membership_proves_every_conditional_crash_claim_segment() {
    let checked = checked_program(
        r#"
        data Receipt [linear] { code: i32; }
        data InnerReceipt {
            case Empty;
            case Live(receipt: Receipt);
        }
        data OuterReceipt {
            case Empty;
            case Wrapped(inner: InnerReceipt);
        }

        machine nested(choice: OuterReceipt) -> i32
        crashes Abort
        {
            transition choice {
                OuterReceipt::Wrapped -> inspect(choice)
                OuterReceipt::Empty -> 0
            }

            state inspect(pending: OuterReceipt) -> i32 {
                transition pending.inner {
                    InnerReceipt::Live -> crash_live(pending)
                    InnerReceipt::Empty -> 0
                }
            }

            state crash_live(choice: OuterReceipt) -> i32 {
                crash Abort;
            }
        }

        machine outer_only(choice: OuterReceipt) -> i32
        crashes Abort
        {
            transition choice {
                OuterReceipt::Wrapped -> crash_wrapped(choice)
                OuterReceipt::Empty -> 0
            }

            state crash_wrapped(choice: OuterReceipt) -> i32 {
                crash Abort;
            }
        }
        "#,
    );

    let frontier_for = |machine_name: &str, state_name: &str| {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .expect("routing machine");
        let state = checked
            .machine_states(machine)
            .iter()
            .find(|state| state.name.as_str() == state_name)
            .expect("crash state");
        checked
            .facts
            .contract_plans
            .for_machine(machine.symbol)
            .expect("machine contract plan")
            .crash
            .checked_sites()
            .iter()
            .find(|site| site.location().state() == state.symbol)
            .expect("checked crash site")
            .frontier_lower_bound()
    };

    let [nested_claim] = frontier_for("nested", "crash_live") else {
        panic!("both nested case guards should prove one payload claim")
    };
    let case_count = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .find(|event| event.claim_identity == *nested_claim)
        .map(|event| {
            checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
                .iter()
                .filter(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
                .count()
        })
        .expect("nested claim event");
    assert_eq!(case_count, 2);
    assert!(
        frontier_for("outer_only", "crash_wrapped").is_empty(),
        "proving only the outer case must not expose an unknown inner payload claim"
    );
}

#[test]
fn empty_conditional_sum_records_establishment_without_payload_debt() {
    let checked = checked_program(
        r#"
        data Receipt [linear] { code: i32; }
        data ReceiptState {
            case Empty;
            case Live(receipt: Receipt);
        }
        data Main {}
        machine Main::run() -> i32 {
            let state: ReceiptState = ReceiptState::Empty;
            0
        }
        "#,
    );
    let event = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .find(|event| event.kind == language_semantics::PermissionEventKind::Establish)
        .expect("conditional establishment event");
    assert!(!event.obligation_live);
}

#[test]
fn uninitialized_conditional_sum_cannot_be_moved_as_an_empty_value() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        data ReceiptState {
            case Empty;
            case Live(receipt: Receipt);
        }
        machine ReceiptState::settle(self) {}
        data Main {}
        machine Main::run() -> i32 {
            let state: ReceiptState;
            ReceiptState::settle(state);
            0
        }
    "#;
    let diagnostics = checked_program_result(source)
        .expect_err("implicit zero-fill does not establish a sum value");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `state::Live.receipt` has not been established")
    }));
}
