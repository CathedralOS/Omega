use crate::tests::front_end::checked_program;

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
