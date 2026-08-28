use super::*;

fn checked(source: &str) -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

#[test]
fn retains_canonical_semantic_permission_events() {
    let checked = checked(
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

    use psi_language_semantics::PermissionEventKind as Kind;
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
            Kind::AffineDrop,
        ]
    );
    assert!(
        events
            .iter()
            .all(|event| event.access == psi_language_semantics::PermissionAccess::Owned)
    );
    assert!(
        events[..4]
            .iter()
            .all(|event| event.multiplicity == psi_language_semantics::Multiplicity::Linear)
    );
    assert_eq!(
        events[4].multiplicity,
        psi_language_semantics::Multiplicity::Affine
    );
    let origin = events[0].provenance;
    let claim_identity = events[0].claim_identity;
    assert_ne!(
        origin,
        psi_language_semantics::PermissionProvenance::Unknown
    );
    assert_ne!(
        claim_identity,
        psi_language_semantics::PermissionClaimIdentity::Unknown
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
        events[4].provenance,
        psi_language_semantics::PermissionProvenance::Unknown,
        "affine cleanup without live debt must not invent establishment provenance"
    );
    assert_eq!(
        events[4].claim_identity,
        psi_language_semantics::PermissionClaimIdentity::Unknown,
        "no-debt affine cleanup must not invent a live claim identity"
    );
}

#[test]
fn method_form_by_value_self_records_terminal_consume() {
    let checked = checked(
        r#"
        pub data Receipt [linear] { code: i32; }
        boundary machine Receipt::complete(self) {}
        data Main {}

        machine Main::run(receipt: Receipt) {
            receipt.complete();
        }
        "#,
    );

    use psi_language_semantics::{PermissionAccess, PermissionEventKind, PermissionEventSource};
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
    let checked = checked(
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
    assert_ne!(
        *claim,
        psi_language_semantics::PermissionClaimIdentity::Unknown
    );
    assert!(
        checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .any(|(_, event)| event.claim_identity == *claim
                && event.kind == psi_language_semantics::PermissionEventKind::Establish
                && event.source == psi_language_semantics::PermissionEventSource::StateEntry)
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
                    psi_language_semantics::PermissionEventKind::Consume
                        | psi_language_semantics::PermissionEventKind::Transfer
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
    let checked = checked(
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

    let claim_variant = |claim: &psi_language_semantics::PermissionClaimIdentity| {
        checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .find(|event| {
                event.claim_identity == *claim
                    && event.kind == psi_language_semantics::PermissionEventKind::Establish
                    && event.source == psi_language_semantics::PermissionEventSource::StateEntry
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
                        psi_facts::PlaceSegment::Case { variant } => Some(*variant),
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
            psi_typed_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == "Live" =>
            {
                Some(variant.symbol)
            }
            _ => None,
        })
        .expect("Live variant");
    assert_eq!(claim_variant(live_claim), live_variant);
}

#[test]
fn common_case_guard_parameter_map_survives_a_diamond_join() {
    let checked = checked(
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
    let checked = checked(
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
                .filter(|segment| matches!(segment, psi_facts::PlaceSegment::Case { .. }))
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
    let checked = checked(
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
        .find(|event| event.kind == psi_language_semantics::PermissionEventKind::Establish)
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics =
        lower_typed_trees(typed).expect_err("implicit zero-fill does not establish a sum value");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `state::Live.receipt` has not been established")
    }));
}

#[test]
fn borrow_loans_share_the_permission_context_with_access_and_origin() {
    let checked = checked(
        r#"
        data Main { items: [i32; 2]; }

        machine observe(items: &[i32]) {}
        machine mutate(items: &mut [i32]) {}
        machine Main::run() -> i32 { 0 }

        machine Main::read(&self) {
            let view: &[i32] = self.items.as_slice();
            observe(view);
        }

        machine Main::write(&mut self) {
            let view: &mut [i32] = self.items.as_mut_slice();
            mutate(view);
        }
        "#,
    );

    use psi_language_semantics::{
        Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
        PermissionProvenance,
    };
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| event.access != PermissionAccess::Owned)
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 4, "each loan has a begin and release event");

    for access in [PermissionAccess::Shared, PermissionAccess::Exclusive] {
        let pair = events
            .iter()
            .copied()
            .filter(|event| event.access == access)
            .collect::<Vec<_>>();
        assert_eq!(pair.len(), 2);
        assert_eq!(pair[0].kind, PermissionEventKind::Establish);
        assert_eq!(pair[1].kind, PermissionEventKind::Consume);
        assert_eq!(pair[0].provenance, pair[1].provenance);
        assert_eq!(pair[0].claim_identity, pair[1].claim_identity);
        assert_ne!(pair[0].provenance, PermissionProvenance::Unknown);
        assert_ne!(pair[0].claim_identity, PermissionClaimIdentity::Unknown);
    }
    assert_ne!(events[0].claim_identity, events[2].claim_identity);
    assert_eq!(
        events
            .iter()
            .find(|event| event.access == PermissionAccess::Shared)
            .expect("shared loan")
            .multiplicity,
        Multiplicity::Unrestricted
    );
    assert_eq!(
        events
            .iter()
            .find(|event| event.access == PermissionAccess::Exclusive)
            .expect("exclusive loan")
            .multiplicity,
        Multiplicity::Affine
    );
}

#[test]
fn linear_judgment_reads_canonical_permission_events() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Main {}
        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let forwarded: Receipt = issued;
            Receipt::ack(forwarded);
            0
        }
        "#,
    );
    crate::checks::validate_linear_permission_events(&checked.typed, &checked.facts)
        .expect("semantic permission events are sufficient for the judgment");
}

#[test]
fn consuming_call_that_returns_an_obligation_transfers_its_origin() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::forward(self) -> Receipt { self }
        machine Receipt::ack(self) {}
        data Main {}
        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let returned: Receipt = Receipt::forward(issued);
            Receipt::ack(returned);
            0
        }
        "#,
    );

    use psi_language_semantics::{PermissionAccess, PermissionEventKind};
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
        events.iter().map(|event| event.kind).collect::<Vec<_>>(),
        [
            PermissionEventKind::Establish,
            PermissionEventKind::Transfer,
            PermissionEventKind::Establish,
            PermissionEventKind::Consume,
        ]
    );
    assert!(
        events
            .iter()
            .all(|event| event.provenance == events[0].provenance)
    );
}

#[test]
fn state_call_result_preserves_a_locally_created_obligation_origin() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Main {}
        machine Main::issue(&mut self) -> Receipt {
            let issued: Receipt = Receipt { code: 7 };
            transition { _ -> issued }
        }
        machine Main::run(&mut self) -> i32 {
            let returned: Receipt = self.issue();
            Receipt::ack(returned);
            0
        }
        "#,
    );

    use psi_language_semantics::{PermissionAccess, PermissionEventKind};
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
        events.iter().map(|event| event.kind).collect::<Vec<_>>(),
        [
            PermissionEventKind::Establish,
            PermissionEventKind::Transfer,
            PermissionEventKind::Establish,
            PermissionEventKind::Consume,
        ]
    );
    assert!(
        events
            .iter()
            .all(|event| event.provenance == events[0].provenance),
        "the state-call result must not mint a caller-side origin: {events:#?}"
    );
    assert!(
        events
            .iter()
            .all(|event| event.claim_identity == events[0].claim_identity),
        "the state-call result must preserve the callee claim identity: {events:#?}"
    );
}

#[test]
fn state_call_result_maps_multiple_claims_by_unique_output_path() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::issue(&mut self) -> Pair {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let pair: Pair = Pair { left: left, right: right };
            transition { _ -> pair }
        }
        machine Main::run(&mut self) -> i32 {
            let returned: Pair = self.issue();
            let left: Receipt = returned.left;
            let right: Receipt = returned.right;
            Receipt::ack(left);
            Receipt::ack(right);
            0
        }
        "#,
    );

    use psi_language_semantics::{PermissionAccess, PermissionClaimIdentity, PermissionEventKind};
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| event.access == PermissionAccess::Owned)
        .collect::<Vec<_>>();
    let run_symbol = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .find(|state| state.name.as_str() == "run")
        .map(|state| state.symbol)
        .expect("run state");
    let caller_establishments = events
        .iter()
        .copied()
        .filter(|event| {
            event.kind == PermissionEventKind::Establish
                && matches!(
                    event.source,
                    psi_language_semantics::PermissionEventSource::Statement { statement_index: 0 }
                )
        })
        .filter(|event| event.state_symbol == run_symbol)
        .collect::<Vec<_>>();
    assert_eq!(caller_establishments.len(), 2);
    assert_ne!(
        caller_establishments[0].claim_identity,
        caller_establishments[1].claim_identity
    );
    assert!(
        caller_establishments
            .iter()
            .all(|event| event.claim_identity != PermissionClaimIdentity::Unknown)
    );
    for establishment in caller_establishments {
        assert_eq!(
            events
                .iter()
                .filter(|event| event.claim_identity == establishment.claim_identity)
                .count(),
            8,
            "each callee claim must remain independently conserved through the caller: {events:#?}"
        );
    }
}

#[test]
fn state_call_result_maps_direct_aggregate_constructor_fields() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::issue(&mut self) -> Pair {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            transition { _ -> Pair { left: left, right: right } }
        }
        machine Main::run(&mut self) -> i32 {
            let returned: Pair = self.issue();
            let left: Receipt = returned.left;
            let right: Receipt = returned.right;
            Receipt::ack(left);
            Receipt::ack(right);
            0
        }
        "#,
    );

    use psi_language_semantics::{PermissionAccess, PermissionEventKind};
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| event.access == PermissionAccess::Owned)
        .collect::<Vec<_>>();
    let (issue_symbol, run_symbol) = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .fold((None, None), |(issue, run), state| {
            match state.name.as_str() {
                "issue" => (Some(state.symbol), run),
                "run" => (issue, Some(state.symbol)),
                _ => (issue, run),
            }
        });
    let issue_symbol = issue_symbol.expect("issue state");
    let run_symbol = run_symbol.expect("run state");
    let callee_transfers = events
        .iter()
        .copied()
        .filter(|event| {
            event.state_symbol == issue_symbol
                && event.kind == PermissionEventKind::Transfer
                && event.obligation_live
        })
        .collect::<Vec<_>>();
    let caller_establishments = events
        .iter()
        .copied()
        .filter(|event| {
            event.state_symbol == run_symbol
                && event.kind == PermissionEventKind::Establish
                && matches!(
                    event.source,
                    psi_language_semantics::PermissionEventSource::Statement { statement_index: 0 }
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(callee_transfers.len(), 2);
    assert_eq!(caller_establishments.len(), 2);
    assert_ne!(
        caller_establishments[0].claim_identity,
        caller_establishments[1].claim_identity
    );
    assert!(caller_establishments.iter().all(|establishment| {
        callee_transfers
            .iter()
            .any(|transfer| transfer.claim_identity == establishment.claim_identity)
    }));
}

#[test]
fn state_call_result_consumes_checked_opaque_multi_output_map() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::pack(left: Receipt, right: Receipt) -> Pair {
            transition { _ -> Pair { left: left, right: right } }
        }
        machine Main::issue(&mut self) -> Pair {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            transition { _ -> Main::pack(left, right) }
        }
        machine Main::forward(&mut self) -> Pair {
            transition { _ -> (self.issue()) }
        }
        machine Main::run(&mut self) -> i32 {
            let returned: Pair = self.forward();
            let left: Receipt = returned.left;
            let right: Receipt = returned.right;
            Receipt::ack(left);
            Receipt::ack(right);
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("checked outcome maps should compose");
    use psi_language_semantics::PermissionEventKind;
    let maps = checked
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .map(|(_, map)| map)
        .collect::<Vec<_>>();
    let (pack_symbol, issue_symbol, forward_symbol, run_symbol) = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .fold(
            (None, None, None, None),
            |(pack, issue, forward, run), state| match state.name.as_str() {
                "pack" => (Some(state.symbol), issue, forward, run),
                "issue" => (pack, Some(state.symbol), forward, run),
                "forward" => (pack, issue, Some(state.symbol), run),
                "run" => (pack, issue, forward, Some(state.symbol)),
                _ => (pack, issue, forward, run),
            },
        );
    let pack_symbol = pack_symbol.expect("pack state");
    let issue_symbol = issue_symbol.expect("issue state");
    let forward_symbol = forward_symbol.expect("forward state");
    let run_symbol = run_symbol.expect("run state");
    assert_eq!(
        maps.iter()
            .find(|map| map.state_symbol == pack_symbol)
            .map(|map| map.entries.count()),
        Some(2),
    );
    assert_eq!(
        maps.iter()
            .find(|map| map.state_symbol == forward_symbol)
            .map(|map| map.entries.count()),
        Some(2),
    );
    assert_eq!(
        maps.iter()
            .find(|map| map.state_symbol == issue_symbol)
            .map(|map| map.entries.count()),
        Some(2),
    );
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let issue_transfers = events
        .iter()
        .copied()
        .filter(|event| {
            event.state_symbol == issue_symbol
                && event.kind == PermissionEventKind::Transfer
                && event.obligation_live
        })
        .collect::<Vec<_>>();
    let run_establishments = events
        .iter()
        .copied()
        .filter(|event| {
            event.state_symbol == run_symbol
                && event.kind == PermissionEventKind::Establish
                && matches!(
                    event.source,
                    psi_language_semantics::PermissionEventSource::Statement { statement_index: 0 }
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(issue_transfers.len(), 2);
    assert_eq!(run_establishments.len(), 2);
    assert_ne!(
        run_establishments[0].claim_identity,
        run_establishments[1].claim_identity,
    );
    assert!(run_establishments.iter().all(|establishment| {
        issue_transfers
            .iter()
            .any(|transfer| transfer.claim_identity == establishment.claim_identity)
    }));
}

#[test]
fn permission_producer_reconstructs_transfers_from_typed_flow() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Main {}
        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let forwarded: Receipt = issued;
            Receipt::ack(forwarded);
            0
        }
        "#,
    );
    let expected = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event.clone())
        .collect::<Vec<_>>();

    let mut facts = checked.facts.clone();
    facts.flow.ownership.permissions = Default::default();
    crate::checks::record_permission_events(&checked.typed, &mut facts);
    let actual = facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event.clone())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn permission_producer_reconstructs_affine_cleanup_from_typed_flow() {
    let checked = checked(
        r#"
        data Box { value: i32; }
        data Main {}
        machine Main::run() -> i32 { 0 }
        machine Main::consume(input: Box) -> i32 {
            let first: Box = Box { value: 1 };
            let second: Box = Box { value: 2 };
            0
        }
        "#,
    );
    let expected = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.kind == psi_language_semantics::PermissionEventKind::AffineDrop)
                .then_some(event.root)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        expected.len(),
        3,
        "two locals and one owned parameter clean up"
    );

    let mut facts = checked.facts.clone();
    facts.flow.ownership.permissions = Default::default();
    crate::checks::record_permission_events(&checked.typed, &mut facts);
    let actual = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.kind == psi_language_semantics::PermissionEventKind::AffineDrop)
                .then_some(event.root)
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn nested_conditional_payload_extraction_preserves_its_origin() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data ReceiptState {
            case Empty;
            case Live(receipt: Receipt);
        }
        data Main {}
        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let state: ReceiptState = ReceiptState::Live { receipt: issued };
            let extracted: Receipt = state.receipt;
            Receipt::ack(extracted);
            0
        }
        "#,
    );

    use psi_language_semantics::{PermissionAccess, PermissionEventKind};
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
        events.iter().map(|event| event.kind).collect::<Vec<_>>(),
        [
            PermissionEventKind::Establish,
            PermissionEventKind::Transfer,
            PermissionEventKind::Establish,
            PermissionEventKind::Transfer,
            PermissionEventKind::Establish,
            PermissionEventKind::Consume,
        ]
    );
    assert!(
        events
            .iter()
            .all(|event| event.provenance == events[0].provenance),
        "nested transfers must conserve one origin: {events:#?}"
    );
}

#[test]
fn generic_conditional_payload_substitution_preserves_linear_debt() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Outcome<T> {
            case Empty;
            case Returned(value: T);
        }
        data Main {}
        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let outcome: Outcome<Receipt> = Outcome::Returned { value: issued };
            let extracted: Receipt = outcome.value;
            Receipt::ack(extracted);
            0
        }
        "#,
    );

    use psi_language_semantics::{PermissionAccess, PermissionEventKind};
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
        events.iter().map(|event| event.kind).collect::<Vec<_>>(),
        [
            PermissionEventKind::Establish,
            PermissionEventKind::Transfer,
            PermissionEventKind::Establish,
            PermissionEventKind::Transfer,
            PermissionEventKind::Establish,
            PermissionEventKind::Consume,
        ]
    );
    assert!(events.iter().all(|event| event.obligation_live));
    assert!(
        events
            .iter()
            .all(|event| event.provenance == events[0].provenance),
        "generic substitution must conserve the payload origin: {events:#?}"
    );
}

#[test]
fn generic_conditional_empty_case_establishes_without_debt() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        data Outcome<T> {
            case Empty;
            case Returned(value: T);
        }
        data Main {}
        machine Main::run() -> i32 {
            let outcome: Outcome<Receipt> = Outcome::Empty;
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
        .find(|event| event.kind == psi_language_semantics::PermissionEventKind::Establish)
        .expect("generic conditional establishment event");
    assert!(!event.obligation_live);
}

#[test]
fn active_case_frontier_preserves_independent_payload_claims() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data ReceiptState {
            case Empty;
            case Pair(left: Receipt, right: Receipt);
            case Single(value: Receipt);
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let state: ReceiptState = ReceiptState::Pair {
                left: left,
                right: right,
            };
            let extracted_left: Receipt = state.left;
            let extracted_right: Receipt = state.right;
            Receipt::ack(extracted_left);
            Receipt::ack(extracted_right);
            0
        }
        "#,
    );

    use psi_language_semantics::{PermissionAccess, PermissionEventKind};
    let state_symbol = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .flat_map(|state| checked.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            psi_typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "state" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("state local");
    let establishments = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.access == PermissionAccess::Owned
                && event.kind == PermissionEventKind::Establish
                && event.obligation_live
                && event.root == psi_facts::PlaceRoot::Symbol(state_symbol)
        })
        .collect::<Vec<_>>();
    assert_eq!(establishments.len(), 2);
    assert!(establishments.iter().all(|event| {
        matches!(
            checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments),
            [
                psi_facts::PlaceSegment::Case { .. },
                psi_facts::PlaceSegment::Field { .. }
            ]
        )
    }));
    assert_ne!(
        establishments[0].claim_identity,
        establishments[1].claim_identity
    );
}

#[test]
fn active_case_partial_move_leaves_same_case_sibling_live() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data ReceiptState {
            case Empty;
            case Pair(left: Receipt, right: Receipt);
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let state: ReceiptState = ReceiptState::Pair {
                left: left,
                right: right,
            };
            let extracted: Receipt = state.left;
            Receipt::ack(extracted);
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("the active-case sibling remains live");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `state::Pair.right` reaches scope exit")
    }));
}

#[test]
fn active_case_rejects_duplicate_payload_move() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data ReceiptState {
            case Empty;
            case Live(receipt: Receipt);
        }
        data Main {}
        machine Main::run() -> i32 {
            let receipt: Receipt = Receipt { code: 1 };
            let state: ReceiptState = ReceiptState::Live { receipt: receipt };
            let first: Receipt = state.receipt;
            let duplicate: Receipt = state.receipt;
            Receipt::ack(first);
            Receipt::ack(duplicate);
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("one payload claim cannot move twice");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `state::Live.receipt` was already transferred")
    }));
}

#[test]
fn active_case_result_map_omits_proven_inactive_alternatives() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data ReceiptState {
            case Empty;
            case Pair(left: Receipt, right: Receipt);
            case Single(value: Receipt);
        }
        data Main {}
        machine Main::pack(receipt: Receipt) -> ReceiptState {
            ReceiptState::Single { value: receipt }
        }
        machine Main::run() -> i32 {
            let receipt: Receipt = Receipt { code: 1 };
            let state: ReceiptState = Main::pack(receipt);
            let extracted: Receipt = state.value;
            Receipt::ack(extracted);
            0
        }
        "#,
    );

    let pack_symbol = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .find(|state| state.name.as_str() == "pack")
        .map(|state| state.symbol)
        .expect("pack state");
    let map = checked
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .map(|(_, map)| map)
        .find(|map| map.state_symbol == pack_symbol)
        .expect("active-case outcome map");
    let entries = checked
        .facts
        .flow
        .ownership
        .claim_outcome_entries
        .span_or_empty(map.entries);
    assert_eq!(entries.len(), 1);
    assert!(matches!(
        checked
            .facts
            .flow
            .ownership
            .segments
            .span_or_empty(entries[0].output_segments),
        [
            psi_facts::PlaceSegment::Case { .. },
            psi_facts::PlaceSegment::Field { .. }
        ]
    ));
}

#[test]
fn nested_linear_record_extraction_stays_conservative_without_field_algebra() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair [linear] {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let pair: Pair = Pair { left: left, right: right };
            let extracted: Receipt = pair.left;
            Receipt::ack(extracted);
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("partial linear-record extraction needs per-field resource accounting");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `pair` reaches scope exit")
    }));
}

#[test]
fn transparent_record_frontier_preserves_independent_field_origins() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let pair: Pair = Pair { left: left, right: right };
            let forwarded: Pair = pair;
            let extracted_left: Receipt = forwarded.left;
            let extracted_right: Receipt = forwarded.right;
            Receipt::ack(extracted_left);
            Receipt::ack(extracted_right);
            0
        }
        "#,
    );

    use psi_language_semantics::{PermissionAccess, PermissionEventKind};
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| event.access == PermissionAccess::Owned)
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 16);

    let pair_symbol = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .flat_map(|state| checked.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            psi_typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "pair" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("pair local");
    let pair_establishments = events
        .iter()
        .copied()
        .filter(|event| {
            event.kind == PermissionEventKind::Establish
                && event.root == psi_facts::PlaceRoot::Symbol(pair_symbol)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        pair_establishments.len(),
        2,
        "transparent records establish one frontier entry per contained linear field"
    );
    assert!(pair_establishments.iter().all(|event| {
        checked
            .facts
            .flow
            .ownership
            .segments
            .span_or_empty(event.segments)
            .len()
            == 1
    }));
    assert_ne!(
        pair_establishments[0].provenance, pair_establishments[1].provenance,
        "constructing an aggregate must not collapse independent field lineages"
    );
    assert_ne!(
        pair_establishments[0].claim_identity, pair_establishments[1].claim_identity,
        "constructing an aggregate must not collapse independent field claims"
    );
    for establishment in pair_establishments {
        assert_eq!(
            events
                .iter()
                .filter(|event| event.provenance == establishment.provenance)
                .count(),
            8,
            "each source claim must map through its field and extracted local independently"
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.claim_identity == establishment.claim_identity)
                .count(),
            8,
            "each source claim identity must survive aggregate and local transfers"
        );
    }
}

#[test]
fn transparent_record_entry_claims_share_lineage_but_have_distinct_identities() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 { 0 }
        machine consume(pair: Pair) {
            let left: Receipt = pair.left;
            let right: Receipt = pair.right;
            Receipt::ack(left);
            Receipt::ack(right);
        }
        "#,
    );

    use psi_language_semantics::{
        PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    };
    let entries = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.source == PermissionEventSource::StateEntry
                && event.kind == PermissionEventKind::Establish
        })
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].provenance, entries[1].provenance);
    assert_ne!(entries[0].claim_identity, entries[1].claim_identity);
    assert!(
        entries
            .iter()
            .all(|event| event.claim_identity != PermissionClaimIdentity::Unknown)
    );
}

#[test]
fn transparent_record_partial_move_leaves_sibling_obligation_live() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let pair: Pair = Pair { left: left, right: right };
            let extracted: Receipt = pair.left;
            Receipt::ack(extracted);
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics =
        lower_typed_trees(typed).expect_err("the untouched sibling remains an obligation");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `pair.right` reaches scope exit")
    }));
}

#[test]
fn nominal_drop_rejects_direct_partial_move() {
    let source = r#"
        data Leaf { value: i32; }
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        data Main {}
        machine Main::run() {
            let wrapper: Wrapper = Wrapper { leaf: Leaf { value: 1 } };
            let extracted: Leaf = wrapper.leaf;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("a nominal drop machine requires its whole valid receiver");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot partially move a value of `Wrapper`")
    }));
}

#[test]
fn nominal_drop_rejects_move_below_nested_prefix() {
    let source = r#"
        data Leaf { value: i32; }
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        data Outer { wrapper: Wrapper; }
        data Main {}
        machine Main::run() {
            let outer: Outer = Outer {
                wrapper: Wrapper { leaf: Leaf { value: 1 } },
            };
            let extracted: Leaf = outer.wrapper.leaf;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("every proper nominal-drop prefix retains whole-value entitlement");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot partially move a value of `Wrapper`")
    }));
}

#[test]
fn nominal_drop_rejects_move_below_generic_prefix() {
    let source = r#"
        data Leaf { value: i32; }
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        data Box<T> { value: T; }
        data Main {}
        machine Main::run() {
            let boxed: Box<Wrapper> = Box {
                value: Wrapper { leaf: Leaf { value: 1 } },
            };
            let extracted: Leaf = boxed.value.leaf;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("generic substitution preserves a nested nominal drop entitlement");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot partially move a value of `Wrapper`")
    }));
}

#[test]
fn nominal_drop_rejects_partial_move_from_self() {
    let source = r#"
        data Leaf { value: i32; }
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        machine Wrapper::take(&mut self) {
            let extracted: Leaf = self.leaf;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("normalized self roots retain their attached nominal drop");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot partially move a value of `Wrapper`")
    }));
}

#[test]
fn nominal_drop_allows_explicit_consuming_decomposition() {
    checked(
        r#"
        data Leaf { value: i32; }
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        machine Wrapper::into_leaf(self) -> Leaf { self.leaf }
        "#,
    );
}

#[test]
fn nominal_drop_allows_whole_value_move() {
    checked(
        r#"
        data Leaf { value: i32; }
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        data Main {}
        machine Main::run() {
            let wrapper: Wrapper = Wrapper { leaf: Leaf { value: 1 } };
            let moved: Wrapper = wrapper;
        }
        "#,
    );
}

#[test]
fn nominal_drop_allows_moving_nested_value_whole() {
    checked(
        r#"
        data Leaf { value: i32; }
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        data Outer { wrapper: Wrapper; }
        data Main {}
        machine Main::run() {
            let outer: Outer = Outer {
                wrapper: Wrapper { leaf: Leaf { value: 1 } },
            };
            let moved: Wrapper = outer.wrapper;
        }
        "#,
    );
}

#[test]
fn nominal_drop_allows_copying_primitive_field() {
    checked(
        r#"
        data Wrapper { value: i32; }
        machine Wrapper::drop(&mut self) {}
        data Main {}
        machine Main::run() {
            let wrapper: Wrapper = Wrapper { value: 1 };
            let copied: i32 = wrapper.value;
        }
        "#,
    );
}

#[test]
fn nominal_drop_allows_owned_production_into_self_field() {
    checked(
        r#"
        data Leaf { value: i32; }
        data LeafFactory {}
        boundary operator LeafFactory::create() -> Leaf;
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        machine Wrapper::replace(&mut self) {
            self.leaf = LeafFactory::create();
        }
        "#,
    );
}

#[test]
fn transparent_affine_record_allows_partial_move() {
    checked(
        r#"
        data Leaf { value: i32; }
        data Pair { left: Leaf; right: Leaf; }
        data Main {}
        machine Main::run() {
            let pair: Pair = Pair {
                left: Leaf { value: 1 },
                right: Leaf { value: 2 },
            };
            let extracted: Leaf = pair.left;
        }
        "#,
    );
}

#[test]
fn transparent_record_rejects_duplicate_field_move() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let pair: Pair = Pair { left: left, right: right };
            let first: Receipt = pair.left;
            let duplicate: Receipt = pair.left;
            let remaining: Receipt = pair.right;
            Receipt::ack(first);
            Receipt::ack(duplicate);
            Receipt::ack(remaining);
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("one field claim cannot move twice");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `pair.left` was already transferred")
    }));
}

#[test]
fn fixed_array_partial_move_leaves_sibling_obligation_live() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let receipts: [Receipt; 2] = [left, right];
            let first: Receipt = receipts[0];
            Receipt::ack(first);
            let second: Receipt = receipts[1];
            Receipt::ack(second);
            0
        }
        "#,
    );

    use psi_language_semantics::{PermissionAccess, PermissionEventKind};
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| event.access == PermissionAccess::Owned)
        .collect::<Vec<_>>();
    let receipt_establishments = events
        .iter()
        .copied()
        .filter(|event| {
            event.kind == PermissionEventKind::Establish
                && event.obligation_live
                && matches!(
                    checked
                        .facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(event.segments),
                    [psi_facts::PlaceSegment::FixedIndex { .. }]
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(receipt_establishments.len(), 2);
    let indices = receipt_establishments
        .iter()
        .map(|event| {
            let [psi_facts::PlaceSegment::FixedIndex { index }] = checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
            else {
                unreachable!("fixed array claim path")
            };
            *index
        })
        .collect::<Vec<_>>();
    assert_eq!(indices, [0, 1]);
    assert_ne!(
        receipt_establishments[0].claim_identity,
        receipt_establishments[1].claim_identity
    );
}

#[test]
fn fixed_array_rejects_duplicate_literal_index_move() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let receipts: [Receipt; 2] = [left, right];
            let first: Receipt = receipts[0];
            let duplicate: Receipt = receipts[0];
            Receipt::ack(first);
            Receipt::ack(duplicate);
            let second: Receipt = receipts[1];
            Receipt::ack(second);
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics =
        lower_typed_trees(typed).expect_err("the same fixed element cannot move twice");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("receipts[0]")
                && diagnostic
                    .message
                    .contains("already transferred or consumed")
        }),
        "expected a duplicate fixed-index move diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn fixed_array_state_result_maps_claims_by_literal_index() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Main {}
        machine Main::pack(left: Receipt, right: Receipt) -> [Receipt; 2] {
            [left, right]
        }
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let receipts: [Receipt; 2] = Main::pack(left, right);
            let first: Receipt = receipts[0];
            let second: Receipt = receipts[1];
            Receipt::ack(first);
            Receipt::ack(second);
            0
        }
        "#,
    );

    let pack_symbol = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .find(|state| state.name.as_str() == "pack")
        .map(|state| state.symbol)
        .expect("pack state");
    let map = checked
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .map(|(_, map)| map)
        .find(|map| map.state_symbol == pack_symbol)
        .expect("fixed-array outcome map");
    let entries = checked
        .facts
        .flow
        .ownership
        .claim_outcome_entries
        .span_or_empty(map.entries);
    assert_eq!(entries.len(), 2);
    for (expected, entry) in entries.iter().enumerate() {
        assert_eq!(
            checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(entry.output_segments),
            [psi_facts::PlaceSegment::FixedIndex { index: expected }]
        );
    }
}

#[test]
fn transparent_record_sibling_assignment_transfers_the_source_claim() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let pair: Pair = Pair { left: left, right: right };
            let old_left: Receipt = pair.left;
            Receipt::ack(old_left);
            pair.left = pair.right;
            let forwarded: Receipt = pair.left;
            Receipt::ack(forwarded);
            let duplicate: Receipt = pair.right;
            Receipt::ack(duplicate);
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics =
        lower_typed_trees(typed).expect_err("assigning from a sibling must transfer its claim");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `pair.right` was already transferred")
    }));
}

#[test]
fn nested_generic_transparent_record_retains_the_concrete_claim_path() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Box<T> { value: T; }
        data Envelope<T> { boxed: Box<T>; }
        data Main {}
        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let boxed: Box<Receipt> = Box { value: issued };
            let envelope: Envelope<Receipt> = Envelope { boxed: boxed };
            let extracted: Receipt = envelope.boxed.value;
            Receipt::ack(extracted);
            0
        }
        "#,
    );

    let envelope_symbol = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .flat_map(|state| checked.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            psi_typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "envelope" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("envelope local");
    let establishment = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .find(|event| {
            event.kind == psi_language_semantics::PermissionEventKind::Establish
                && event.root == psi_facts::PlaceRoot::Symbol(envelope_symbol)
        })
        .expect("nested generic frontier establishment");
    assert_eq!(
        checked
            .facts
            .flow
            .ownership
            .segments
            .span_or_empty(establishment.segments)
            .len(),
        2,
        "the concrete claim must remain nested under both generic record fields"
    );
}
