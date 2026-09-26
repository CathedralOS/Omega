use super::Multiplicity;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

#[test]
fn retains_exact_payloadless_guarded_identity_call() {
    let checked = checked(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        data Outcome [copy] { case Success; case Failure; }
        data Root {}

        machine Root::produce() -> Outcome
        ensures Outcome::Success -> { selected: ready(); }
        { selected = ConcreteEvidence; Outcome::Success }

        machine Root::caller() -> Outcome {
            let saved: Outcome = Root::produce();
            transition saved {
                Outcome::Success { ; selected: local } -> saved
                Outcome::Failure { } -> saved
            }
        }
        "#,
    );
    let caller = machine_named(&checked, "caller");
    let producer = machine_named(&checked, "produce");
    let plan = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_for_machine(caller)
        .expect("the exhaustive identity arms should retain one guarded call plan");
    assert_eq!(plan.target_machine, producer);
    assert_eq!(plan.call.statement_index, 0);
    assert_eq!(plan.call.call_ordinal, 0);
    assert_eq!(plan.result.multiplicity, Multiplicity::Unrestricted);
    assert!(plan.result.qualifications.is_empty());
    let selected = plan
        .selected_evidence
        .first()
        .expect("the named guarded row should retain its selected caller term");
    assert_eq!(selected.arm_statement_index, 3);
    assert_eq!(
        checked
            .facts
            .proof
            .evidence_terms
            .get(selected.selected_term)
            .name,
        "local"
    );
}

#[test]
fn guarded_payloadless_identity_call_rejects_foreign_attachment() {
    let checked = checked(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        data Outcome [copy] { case Success; case Failure; }
        data Root {}
        data Other {}

        machine Root::produce() -> Outcome
        ensures Outcome::Success -> { selected: ready(); }
        { selected = ConcreteEvidence; Outcome::Success }

        machine Other::caller() -> Outcome {
            let saved: Outcome = Root::produce();
            transition saved {
                Outcome::Success { ; selected: local } -> saved
                Outcome::Failure { } -> saved
            }
        }
        "#,
    );
    let caller = machine_named(&checked, "caller");
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_call_returns
            .payloadless_guarded_for_machine(caller)
            .is_none(),
        "the bounded call carrier must not forge the callee attachment onto a foreign caller"
    );
}

/// A static-requirement-dispatched call names the satisfier's private
/// realization as its `target_symbol`; the specialized caller spelled the
/// public requirement. The guarded plan would otherwise bind the private
/// realization as the callee the caller runs.
#[test]
fn guarded_payloadless_identity_call_rejects_static_requirement_dispatch() {
    let checked = checked(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        data Outcome [copy] { case Success; case Failure; }
        trait Producer {
            machine Self::produce(&self) -> Outcome;
        }
        data Token {}
        TokenProducer: Token satisfies Producer {
            machine produce(&self) -> Outcome
            ensures Outcome::Success -> { selected: ready(); }
            { selected = ConcreteEvidence; Outcome::Success }
        }
        machine invoke<Element, Order: Element satisfies Producer>(item: &Element) -> Outcome {
            let saved: Outcome = Order::produce(item);
            transition saved {
                Outcome::Success { } -> saved
                Outcome::Failure { } -> saved
            }
        }
        machine caller(token: &Token) -> Outcome { invoke<Token, TokenProducer>(token) }
        "#,
    );
    let template = machine_named(&checked, "invoke");
    let instance = checked
        .machine_specializations
        .iter()
        .find(|specialization| specialization.template == template)
        .expect("one selected `invoke<Token, TokenProducer>` instance")
        .instance;
    let producer = machine_named(&checked, "produce");
    let dispatched = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == instance)
        .into_iter()
        .flat_map(|machine| checked.machine_states(machine))
        .flat_map(|state| checked.statement_table.statements(state.statement_nodes))
        .filter_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local) => Some(local.initial_value),
            _ => None,
        })
        .filter_map(
            |expression| match checked.expression_table.expression(expression) {
                typed_trees::expression::ExpressionNode::Call(call) => Some(call),
                _ => None,
            },
        )
        .find(|call| call.static_requirement_dispatch.is_some())
        .expect("the specialized body retains the dispatched requirement call");
    assert!(
        checked
            .machines()
            .iter()
            .any(|machine| machine.symbol == producer
                && checked
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == dispatched.target_symbol)),
        "the rewritten target names the private realization"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_call_returns
            .payloadless_guarded_for_machine(instance)
            .is_none(),
        "a dispatched requirement call is not an ordinary guarded value-call return"
    );
}

#[test]
fn scalar_array_field_record_result_composes_with_zeroed_omission() {
    let checked = checked(
        r#"
        data Framed { bytes: [u8; 4]; tag: u64; }
        machine choose_framed(pick_left: bool) -> Framed {
            transition pick_left {
                true -> left()
                _ -> right()
            }
            state left() -> Framed { Framed { tag: 1 } }
            state right() -> Framed { Framed { tag: 2 } }
        }
    "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine_named(&checked, "choose_framed"))
            .is_some(),
        "an omitted scalar-array field composes through the zeroed fill"
    );
}

#[test]
fn float_member_record_tail_statement_plans() {
    let checked = checked(
        r#"
        data TrackableTaskHandle<'a> {
            name: &'a [u8];
            progress: f32;
            task_identifier: &'a [u8];
        }
        machine TrackableTaskHandle::clone<'a>(source: &'a TrackableTaskHandle<'a>) -> TrackableTaskHandle<'a> {
            TrackableTaskHandle { name: source.name, progress: source.progress, task_identifier: source.task_identifier }
        }
        "#,
    );
    // A floating member read through a structural parameter projection names
    // the same statically known storage an integer leaf does: the record
    // tail admits the clone literal as a statement-sequence structural root.
    let machine = machine_named(&checked, "clone");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(machine)
            .is_none(),
        "an f32 member read must not decline the statement sequence"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .is_some(),
        "the clone literal tail admits an ordinary statement-sequence plan"
    );
}
