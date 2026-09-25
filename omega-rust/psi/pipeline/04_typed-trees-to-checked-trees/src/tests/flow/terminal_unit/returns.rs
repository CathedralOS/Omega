use super::{CheckedUnitStructuralTypeShape, Multiplicity};
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

#[test]
fn borrowed_case_getter_retains_ordered_refined_scalar_returns() {
    let checked = checked(
        r#"
        data MemoryAlignment [copy] {
            case Alignment1; case Alignment2; case Alignment4; case Alignment8;
        }
        machine MemoryAlignment::get_size_in_bytes(&self) -> u64 [1..=8] {
            transition self {
                MemoryAlignment::Alignment1 -> (1)
                MemoryAlignment::Alignment2 -> (2)
                MemoryAlignment::Alignment4 -> (4)
                MemoryAlignment::Alignment8 -> (8)
            }
        }
    "#,
    );
    let getter = machine_named(&checked, "get_size_in_bytes");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == getter)
        .expect("borrowed four-case getter retains ordinary ordered scalar completion");
    let control = plan
        .scalar_control
        .as_ref()
        .expect("ordered scalar result owner");
    let checked_trees::CheckedScalarStateTerminator::Guarded { arms, fallback } =
        &control.terminator
    else {
        panic!("ordered guards");
    };
    assert!(
        fallback.is_none(),
        "the final case is not an implicit wildcard"
    );
    let arms = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(*arms)
        .unwrap();
    assert_eq!(arms.len(), 4);
    assert!(
        arms.windows(2)
            .all(|pair| pair[0].guard_statement_ordinal + 1 == pair[1].guard_statement_ordinal)
    );
}

#[test]
fn ordered_scalar_guards_retain_explicit_fallback() {
    let checked = checked(
        r#"
        machine choose(input: u64) -> u64 {
            transition input {
                0 -> (1)
                1 -> (2)
                _ -> (3)
            }
        }
    "#,
    );
    let machine = machine_named(&checked, "choose");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine)
        .expect("ordinary guarded scalar body");
    let checked_trees::CheckedScalarStateTerminator::Guarded { arms, fallback } =
        &plan.scalar_control.as_ref().unwrap().terminator
    else {
        panic!("ordered guards");
    };
    let arms = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(*arms)
        .unwrap();
    assert_eq!(arms.len(), 2);
    assert!(
        matches!(fallback, Some(checked_trees::CheckedScalarBranchDestination::Return { statement_ordinal, is_continuation: false }) if *statement_ordinal == arms[1].guard_statement_ordinal + 1)
    );
}

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
fn exact_payloadless_case_return_keeps_its_ordinary_unit_plan() {
    let checked = checked(
        r#"
        data Outcome [copy] {
            case Success;
            case Failure;
        }
        data Root {}

        machine Root::choose() -> Outcome {
            Outcome::Success
        }
        "#,
    );

    let machine = machine_named(&checked, "choose");
    assert!(
        checked_trees::CheckedReturnPlan::for_machine(&checked.facts.flow, machine).is_none(),
        "a zero-input case constructor has no return family of its own"
    );
    let Some(checked_trees::CheckedUnitPlan::ComposedControl(plan)) =
        checked_trees::CheckedUnitPlan::for_machine(&checked.facts.flow, machine)
    else {
        panic!("the zero-input case constructor keeps the ordinary composed Unit plan")
    };
    let checked_trees::CheckedControlResultPlan::Structural(result) = &plan.result else {
        panic!("the constructed case is the plan's structural result")
    };
    assert_eq!(result.multiplicity, Multiplicity::Unrestricted);
    assert!(result.qualifications.is_empty());
    let [state] = plan.states.as_slice() else {
        panic!("one state constructs and returns the case")
    };
    assert!(state.structural_parameters.is_empty());
    assert!(state.scalar_parameters.is_empty());
    // The case is an ordinary structural value establishment followed by the
    // structural return of that one binding.
    let [
        checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result: established,
            calls,
            ..
        },
    ] = state.operations.as_slice()
    else {
        panic!("one ordinary structural value establishes the case")
    };
    assert!(calls.is_empty());
    assert_eq!(established.type_identity, result.type_identity);
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::ReturnStructural {
        result: returned,
    } = &state.terminator
    else {
        panic!("the state returns the established case")
    };
    assert_eq!(
        returned.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: established.binding_ordinal,
        }
    );

    let result_shape = checked
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .iter()
        .find(|shape| shape.identity == result.type_identity)
        .expect("the Unit roster retains the exact result shape");
    let CheckedUnitStructuralTypeShape::Sum { cases } = &result_shape.shape else {
        panic!("the result must remain a sum")
    };
    assert_eq!(
        cases
            .iter()
            .map(|case| (case.identity.as_str(), case.fields.len()))
            .collect::<Vec<_>>(),
        [("Success", 0), ("Failure", 0)]
    );
}

#[test]
fn guarded_payloadless_case_return_keeps_its_ordinary_unit_plan() {
    let checked = checked(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        data Outcome [copy] { case Success; case Failure; }
        data Root {}

        machine Root::guarded() -> Outcome
        ensures Outcome::Success -> { selected: ready(); true; }
        { selected = ConcreteEvidence; Outcome::Success }
        "#,
    );
    let machine = machine_named(&checked, "guarded");
    assert!(
        checked_trees::CheckedReturnPlan::for_machine(&checked.facts.flow, machine).is_none(),
        "result-case guarantees do not select a return family"
    );
    assert!(
        matches!(
            checked_trees::CheckedUnitPlan::for_machine(&checked.facts.flow, machine),
            Some(checked_trees::CheckedUnitPlan::ComposedControl(_))
        ),
        "result-case guarantees keep the ordinary composed Unit plan"
    );
}

#[test]
fn nested_structural_record_result_keeps_composed_plan() {
    let checked = checked(
        r#"
        data Pair { left: u64; right: u64; }
        data Boxed { pair: Pair; tag: u64; }
        machine choose_boxed(pick_left: bool) -> Boxed {
            transition pick_left {
                true -> left()
                _ -> right()
            }
            state left() -> Boxed { Boxed { pair: Pair { left: 1, right: 0 }, tag: 0 } }
            state right() -> Boxed { Boxed { pair: Pair { left: 2, right: 0 }, tag: 0 } }
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "choose_boxed"))
        .expect("a record result with a nested structural carrier stays composed");
    let checked_trees::CheckedControlResultPlan::Structural(result) = &plan.result else {
        panic!("the nested record result stays structural");
    };
    assert!(
        result.type_identity.contains("Boxed"),
        "the result names the authored record, found {}",
        result.type_identity
    );
}

#[test]
fn structural_payload_sum_result_keeps_composed_plan() {
    let checked = checked(
        r#"
        data Inner { value: u64; }
        data Outcome { case Valued(inner: Inner); case Empty; }
        machine choose_outcome(pick_value: bool) -> Outcome {
            transition pick_value {
                true -> first()
                _ -> second()
            }
            state first() -> Outcome { Outcome::Empty }
            state second() -> Outcome { Outcome::Empty }
        }
        machine choose_valued(pick_value: bool) -> Outcome {
            transition pick_value {
                true -> valued()
                _ -> empty()
            }
            state valued() -> Outcome { Outcome::Valued { inner: Inner { value: 3 } } }
            state empty() -> Outcome { Outcome::Empty }
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "choose_outcome"))
        .expect("a sum result with a structural payload case stays composed");
    let checked_trees::CheckedControlResultPlan::Structural(result) = &plan.result else {
        panic!("the structural-payload sum result stays structural");
    };
    assert!(
        result.type_identity.contains("Outcome"),
        "the result names the authored sum, found {}",
        result.type_identity
    );
    // A structural case payload establishes as an owned structural value: the
    // composed plan keeps the return as a structural result.
    let valued_plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "choose_valued"))
        .expect("a structural case payload stays composed");
    let checked_trees::CheckedControlResultPlan::Structural(valued_result) = &valued_plan.result
    else {
        panic!("the case-payload result stays structural");
    };
    assert!(
        valued_result.type_identity.contains("Outcome"),
        "the result names the authored sum, found {}",
        valued_result.type_identity
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
fn nested_record_field_record_result_still_declines_composed() {
    let checked = checked(
        r#"
        data Inner { value: u64; }
        data Framed { inner: Inner; tag: u64; }
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
            .is_none(),
        "an omitted non-scalar structural field stays outside the composed result signature"
    );
}

#[test]
fn shared_byte_view_member_record_result_composes() {
    let checked = checked(
        r#"
        data Handle<'e> { name: &'e [u8]; tag: u64; }
        machine Probe::pick<'e>(&self, s: &'e [u8], flag: bool) -> Handle<'e> {
            transition flag {
                true -> left(s)
                _ -> right(s)
            }
            state left(s: &'e [u8]) -> Handle<'e> { Handle { name: s, tag: 1 } }
            state right(s: &'e [u8]) -> Handle<'e> { Handle { name: s, tag: 2 } }
        }
        data Probe {}
    "#,
    );
    // A record result whose members are scalars plus shared byte views is an
    // owned carrier of view descriptors: each `&` member keeps its own
    // statically named loan while the record itself transfers whole.
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine_named(&checked, "pick"))
            .is_some(),
        "a shared byte-view member record result admits a composed signature"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(machine_named(&checked, "pick"))
            .is_none()
    );
}

#[test]
fn mutable_byte_view_member_record_result_still_declines() {
    let checked = checked(
        r#"
        data Handle<'e> { name: &'e mut [u8]; tag: u64; }
        machine Probe::pick<'e>(&self, s: &'e mut [u8], flag: bool) -> Handle<'e> {
            transition flag {
                true -> left(s)
                _ -> right(s)
            }
            state left(s: &'e mut [u8]) -> Handle<'e> { Handle { name: s, tag: 1 } }
            state right(s: &'e mut [u8]) -> Handle<'e> { Handle { name: s, tag: 2 } }
        }
        data Probe {}
    "#,
    );
    // An exclusive view member cannot ride a shared loan roster: `&mut`
    // carriers stay declined at the signature gate.
    let stage = checked
        .facts
        .flow
        .terminal_unit_effects
        .omission_for_machine(machine_named(&checked, "pick"))
        .map(|row| row.stage.clone());
    assert!(
        matches!(
            stage,
            Some(checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction { ref phase, .. })
                if *phase == "state graph: result signature"
        ),
        "a mutable byte-view member still declines at result signature, got {stage:?}"
    );
}

#[test]
fn view_member_result_machines_advance_past_signature() {
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
        data TrackableTask {
            progress: f32;
        }
        machine TrackableTask::get_name<'a>(&self) -> &'a [u8] {
            "task"
        }
        machine TrackableTask::get_task_identifier<'a>(&self) -> &'a [u8] {
            "id"
        }
        machine TrackableTask::get_task_handle<'a>(&self) -> TrackableTaskHandle<'a> {
            TrackableTaskHandle {
                name: self.get_name(),
                progress: self.progress,
                task_identifier: self.get_task_identifier()
            }
        }
        "#,
    );
    // The sweep-ledger shapes each advance past `state graph: result
    // signature` to their own deeper walls: none of these omissions may name
    // the signature gate again.
    for name in [
        "TrackableTaskHandle::clone",
        "TrackableTask::get_name",
        "TrackableTask::get_task_identifier",
        "TrackableTask::get_task_handle",
    ] {
        let machine = machine_named(&checked, name);
        let stage = checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(machine)
            .map(|row| row.stage.clone());
        assert!(
            !matches!(
                stage,
                Some(checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction { ref phase, .. })
                    if *phase == "state graph: result signature"
            ),
            "{name} still declines at result signature: {stage:?}"
        );
    }
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

#[test]
fn addr_member_parameter_projection_still_declines() {
    let checked = checked(
        r#"
        data Handle { pointer: addr; tag: u64; }
        machine Handle::clone<'a>(source: &'a Handle) -> Handle {
            Handle { pointer: source.pointer, tag: source.tag }
        }
        "#,
    );
    // An address carrier still has no admitted runtime observation: the
    // record tail keeps declining at the statement-sequence kind gate.
    let stage = checked
        .facts
        .flow
        .terminal_unit_effects
        .omission_for_machine(machine_named(&checked, "clone"))
        .map(|row| row.stage.clone());
    assert!(
        matches!(
            stage,
            Some(checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction { ref phase, .. })
                if *phase == "statement sequence: unsupported statement kind"
        ),
        "an addr member read still declines the statement sequence, got {stage:?}"
    );
}
