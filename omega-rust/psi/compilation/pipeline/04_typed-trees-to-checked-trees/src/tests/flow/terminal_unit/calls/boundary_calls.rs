use crate::tests::flow::terminal_unit::{
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralFieldType, Multiplicity, PrimitiveType,
    checked, machine_named, record_fields,
};
use checked_trees::CheckedUnitCallResult;

#[test]
fn loans_retain_linear_claims_until_the_owned_call() {
    let original = checked(
        r#"
        data Receipt [linear] { code: i32; }
        boundary requirement Receipt::settle(receipt: Receipt);
        machine Receipt::observe(receipt: &Receipt) {}
        machine Receipt::edit(receipt: &mut Receipt) {}
        machine Receipt::forward(receipt: Receipt) {
            Receipt::settle(receipt);
        }
        machine enter(mut receipt: Receipt) {
            Receipt::observe(&receipt);
            Receipt::edit(&mut receipt);
            Receipt::observe(&receipt);
            Receipt::forward(receipt);
        }
        "#,
    );
    let symbol = machine_named(&original, "enter");
    let plan = original
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(symbol)
        .unwrap_or_else(|| {
            panic!(
                "loans followed by an owned transfer retain a complete plan: {:?}",
                original
                    .machines()
                    .iter()
                    .map(|machine| (
                        &machine.name,
                        original
                            .facts
                            .flow
                            .terminal_unit_effects
                            .omission_for_machine(machine.symbol)
                    ))
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(plan.entry_claims.len(), 1);
    let calls = plan
        .operations
        .iter()
        .filter_map(CheckedUnitEffectOperationPlan::call)
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 4);
    for call in &calls[..3] {
        assert!(call.claim_transfers().is_empty());
    }
    assert_eq!(calls[3].claim_transfers().len(), 1);
    assert_eq!(
        calls[3].claim_transfers()[0].claim_identity,
        plan.entry_claims[0].claim_identity
    );
    assert_eq!(calls[3].claim_transfers()[0].argument_index, 0);

    // Moving the real transfer event onto the loan must not make the loan
    // consume its backing. The join still requires complete event coverage.
    let mut changed = original.clone();
    let transfer = changed
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| {
            (event.machine_symbol == symbol
                && event.kind == language_semantics::PermissionEventKind::Transfer
                && event.access == language_semantics::PermissionAccess::Owned)
                .then_some(handle)
        })
        .expect("owned call transfer event");
    let loan_call = changed
        .facts
        .flow
        .control
        .calls
        .iter()
        .find_map(|(_, call)| {
            (call.target_symbol == calls[0].target.state && call.statement_index == 0)
                .then_some(call)
        })
        .expect("first borrowed call");
    changed
        .facts
        .flow
        .ownership
        .permissions
        .get_mut(transfer)
        .source = language_semantics::PermissionEventSource::Call {
        statement_index: loan_call.statement_index,
        call_ordinal: loan_call.call_ordinal,
        target_symbol: loan_call.target_symbol,
    };
    let plans = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &changed.typed,
        &changed.facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &changed.facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &changed.facts.flow.terminal_structural_scalar_returns,
        },
    );
    assert!(plans.for_machine(symbol).is_none());
}

#[test]
fn loan_after_linear_transfer_is_rejected() {
    let result = crate::tests::front_end::checked_program_result(
        r#"
        data Receipt [linear] { code: i32; }
        boundary requirement Receipt::settle(receipt: Receipt);
        machine Receipt::observe(receipt: &Receipt) {}
        machine enter(receipt: Receipt) {
            Receipt::settle(receipt);
            Receipt::observe(&receipt);
        }
        "#,
    );
    assert!(result.is_err(), "a loan cannot revive a consumed claim");
}

#[test]
fn static_boundary_reaches_keep_every_direct_intrinsic_and_requirement_call() {
    // The requirement calls are direct: since c5843e4c43 a nominal binder
    // call (`Selected(0)` under `where machine Selected satisfies
    // Console::exit_process`) is an obligation for specialization, not an
    // executable boundary body, so an open generic entry retains no plan for
    // it and cannot carry the reach-conflict controls below.
    let original = checked(
        r#"
        pub boundary trait Console {
            machine write_byte(byte: i32) reaches Console;
            machine exit_process(code: i32) reaches Console;
            machine unused(code: i32) reaches Console;
        }
        pub data ConsoleNativeProvider {}
        boundary machine ConsoleNativeProvider::write_byte(byte: i32)
            satisfies Console::write_byte;
        data Root {}
        machine Root::enter()
        reaches Console {
            ConsoleNativeProvider::write_byte(1);
            ConsoleNativeProvider::write_byte(2);
            Console::write_byte(3);
            Console::exit_process(0);
            Console::exit_process(0);
        }
        "#,
    );
    let definition = original
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Console")
        .unwrap();
    let signatures = original.typed.trait_machine_signatures(definition);
    let requirement = |name: &str| {
        signatures
            .iter()
            .find(|signature| signature.name.as_str() == name)
            .unwrap()
            .symbol
    };
    let write = requirement("write_byte");
    let exit = requirement("exit_process");
    let unused = requirement("unused");
    let calls = original
        .facts
        .flow
        .control
        .calls
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 5);
    let retained = |plans: &checked_trees::CheckedUnitEffectPlans, symbol| {
        plans
            .boundary_machines
            .iter()
            .any(|plan| plan.machine == symbol)
    };
    let plans = &original.facts.flow.terminal_unit_effects;
    assert!(retained(plans, write));
    assert!(retained(plans, exit));
    assert!(
        !retained(plans, unused),
        "uncalled requirements stay absent"
    );
    for (call_ordinal, handle) in calls.iter().enumerate() {
        let mut changed = original.clone();
        let call = changed.facts.flow.control.calls.get_mut(*handle);
        let empty = language_semantics::ServiceReachRowTable::EMPTY_ROW;
        assert_ne!(call.service_reach.transitive, empty);
        call.service_reach.transitive = empty;
        let plans = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &changed.typed,
            &changed.facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.facts.flow.terminal_structural_scalar_returns,
            },
        );
        let (conflicted, independent) = if call_ordinal < 3 {
            (write, exit)
        } else {
            (exit, write)
        };
        assert!(
            !retained(&plans, conflicted),
            "call {call_ordinal} must not lose its conflicting reach"
        );
        assert!(
            retained(&plans, independent),
            "another requirement retains its own evidence"
        );
        assert!(!retained(&plans, unused));
    }
}

#[test]
fn intrinsic_leaf_with_a_wrong_signature_does_not_rejoin_its_requirement() {
    let checked = checked(
        r#"
        boundary trait Console { machine write_byte(byte: i64) reaches Console; }
        data ConsoleNativeProvider {}
        machine ConsoleNativeProvider::write_byte(byte: i64)
            satisfies Console::write_byte
            via ForeignBinding::CompilerIntrinsic;
        data Root {}
        machine Root::enter() reaches Console {
            ConsoleNativeProvider::write_byte(37);
        }
        "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "Root::enter"))
            .is_none(),
        "an intrinsic leaf outside the sealed catalog signature rejoined a boundary"
    );
}

/// A direct call to an evaluated-binding leaf is a boundary call on the leaf's
/// exact satisfied requirement, not an intrinsic and not an ordinary callee:
/// the leaf owns no body, and its foreign binding settles the requirement.
#[test]
fn evaluated_binding_leaf_calls_rejoin_their_exact_requirement() {
    let checked = checked(
        r#"
        boundary trait Console { machine exit_process(return_code: i32) reaches Console; }
        data ConsoleNativeProvider {}
        machine exit_binding() -> i32 {
            0
        }
        machine ConsoleNativeProvider::exit_process(return_code: i32)
            satisfies Console::exit_process
            via exit_binding();
        data Root {}
        machine Root::enter() reaches Console {
            ConsoleNativeProvider::exit_process(37);
        }
        "#,
    );
    let leaf = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "ConsoleNativeProvider::exit_process")
        .unwrap();
    let leaf_state = checked.machine_states(leaf)[0].symbol;
    let requirement = boundary_requirement(&checked, "Console", "exit_process");
    assert_eq!(
        validation::exact_compiler_intrinsic_boundary_requirement(&checked.typed, leaf_state),
        None,
        "an evaluated import is not a compiler intrinsic"
    );
    assert_eq!(
        validation::exact_external_leaf_boundary_requirement(&checked.typed, leaf_state),
        Some((requirement, leaf.attached_data_symbol))
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::enter"))
        .expect("the leaf call plans as a boundary call");
    assert!(matches!(
        plan.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::Call(call),
            CheckedUnitEffectOperationPlan::Complete { .. },
        ] if call.is_boundary()
            && *call.result() == CheckedUnitCallResult::Unit
            && call.target.machine == requirement
            && call.target.state == requirement
    ));
}

/// The DarwinClock shape: a free leaf with a scalar result stored into a
/// receiver field, called from a provider machine with no provider field.
/// The field store consumes a boundary scalar result; no scalar computation
/// embeds the bodyless leaf as a callee.
#[test]
fn evaluated_binding_leaf_scalar_results_bind_as_boundary_results() {
    let checked = checked(
        r#"
        boundary trait Clock {
            machine poll(fds: u64, count: u64, timeout: i32) -> i32;
            machine now(clock: u32) -> u64;
        }
        machine clock_binding() -> i32 {
            0
        }
        machine clock_poll(fds: u64, count: u64, timeout: i32) -> i32
            satisfies Clock::poll via clock_binding();
        machine clock_now(clock: u32) -> u64
            satisfies Clock::now via clock_binding();
        data Pacer { status: i32; timeout: i32; }
        machine Pacer::sleep(&mut self) {
            self.status = clock_poll(0, 0, self.timeout);
        }
        machine Pacer::wait(&mut self) reaches Clock {
            self.status = Clock::poll(0, 0, self.timeout);
        }
        data Reader {}
        machine Reader::read() -> u64 reaches Clock {
            let ticks: u64 = clock_now(6);
            ticks
        }
        "#,
    );
    let poll = boundary_requirement(&checked, "Clock", "poll");
    let now = boundary_requirement(&checked, "Clock", "now");
    let plans = &checked.facts.flow.terminal_unit_effects;
    // `Pacer::sleep` authors no reach: a leaf call propagates its requirement's
    // reach like an ordinary machine call. `Pacer::wait` calls the requirement
    // itself; both calls publish one reach, so the requirement keeps its plan.
    for requirement in [poll, now] {
        assert!(
            plans
                .boundary_machines
                .iter()
                .any(|plan| plan.machine == requirement),
            "each called requirement retains its boundary plan"
        );
    }
    let sleep = plans
        .for_machine(machine_named(&checked, "Pacer::sleep"))
        .expect("the field store plans through the boundary result");
    let [
        CheckedUnitEffectOperationPlan::Call(call),
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_),
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = sleep.operations.as_slice()
    else {
        panic!("expected boundary scalar call, field store, completion")
    };
    let Some(result) = call.scalar_result().filter(|_| call.is_boundary()) else {
        panic!("expected boundary scalar call, field store, completion")
    };
    assert_eq!(call.target.machine, poll);
    assert_eq!(result.primitive_type, PrimitiveType::I32);
    assert_eq!(call.scalar_arguments.len(), 3);
    let read = plans
        .for_machine(machine_named(&checked, "Reader::read"))
        .expect("the local binding plans through the boundary result");
    assert!(read.operations.iter().any(|operation| matches!(
        operation,
        CheckedUnitEffectOperationPlan::Call(call)
            if call.is_boundary()
                && call.target.machine == now
                && call
                    .scalar_result()
                    .is_some_and(|result| result.primitive_type == PrimitiveType::U64)
    )));
    for machine in ["clock_poll", "clock_now"] {
        assert!(
            checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "the bodyless leaf `{machine}` owns no scalar graph"
        );
    }
}

/// The libobjc/libSystem `const char*` shape: a requirement taking a shared
/// `&u8` plans one shared-borrow structural parameter, because the borrowed
/// byte stays the caller's referent. Before this plan existed, a provider
/// body's leaf call was omitted as "calls boundary ..., which has no boundary
/// plan". A shared loan of a linear referent still forms no plan.
#[test]
fn shared_unrestricted_references_plan_as_borrowed_boundary_parameters() {
    let checked = checked(
        r#"
        boundary trait CText {
            machine length(text: &u8) -> u64;
            machine hold(token: &Token) -> u64;
        }
        data Token [linear] { value: u64; }
        machine text_binding() -> i32 {
            0
        }
        machine c_length(text: &u8) -> u64
            satisfies CText::length via text_binding();
        machine c_hold(token: &Token) -> u64
            satisfies CText::hold via text_binding();
        data Words { bytes: [u8; 6]; }
        data Reader {}
        machine Reader::measure(text: &u8, words: &Words) -> u64 reaches CText {
            let forwarded: u64 = c_length(text);
            let element: u64 = c_length(&words.bytes[2]);
            element
        }
        machine Reader::inspect(token: &Token) -> u64 reaches CText {
            let held: u64 = c_hold(token);
            held
        }
        "#,
    );
    let length = boundary_requirement(&checked, "CText", "length");
    let hold = boundary_requirement(&checked, "CText", "hold");
    let plans = &checked.facts.flow.terminal_unit_effects;
    let plan = plans
        .boundary_for_machine(length)
        .expect("a shared `&u8` requirement retains its boundary plan");
    assert!(plan.scalar_parameters.is_empty());
    let [parameter] = plan.structural_parameters.as_slice() else {
        panic!("one borrowed structural parameter")
    };
    assert_eq!(
        parameter.access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
    );
    assert_eq!(parameter.multiplicity, Multiplicity::Unrestricted);
    assert!(parameter.qualifications.is_empty());
    assert!(
        plans.boundary_for_machine(hold).is_none(),
        "a shared loan of a linear referent stays outside the boundary plan"
    );
    let measure = plans
        .for_machine(machine_named(&checked, "Reader::measure"))
        .expect("both leaf calls rejoin the requirement's plan");
    assert_eq!(
        measure
            .operations
            .iter()
            .filter(|operation| matches!(
                operation,
                CheckedUnitEffectOperationPlan::Call(call)
                    if call.is_boundary() && call.target.machine == length
            ))
            .count(),
        2
    );
}

#[test]
fn named_operator_results_use_declared_types_in_field_assignments() {
    for primitive in ["f32", "f64", "i32"] {
        let checked = checked(&format!(
            "data Arithmetic {{}}
             boundary operator Arithmetic::identity(value: {primitive}) -> {primitive};
             data Counter {{ value: {primitive}; }}
             machine Counter::inspect(&mut self) {{
                 self.value = Arithmetic::identity(self.value);
             }}"
        ));
        let operator = checked.typed.operators().first().unwrap();
        assert_eq!(
            crate::flow::call_target_return_type(&checked.typed, operator.symbol),
            Some(operator.return_type),
            "{primitive}: an operator requirement owns its result type"
        );
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "Counter::inspect"))
            .expect("the ordinary assignment consumes the named requirement result");
        assert!(matches!(
            plan.operations.as_slice(),
            [
                CheckedUnitEffectOperationPlan::Call(call),
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_),
                CheckedUnitEffectOperationPlan::Complete { .. },
            ] if call.is_boundary()
                && call.scalar_result().is_some()
                && call.target.machine == operator.symbol
        ));
    }
}

fn boundary_requirement(
    checked: &checked_trees::CheckedTrees,
    trait_name: &str,
    requirement: &str,
) -> symbols::SymbolHandle {
    let definition = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == trait_name)
        .unwrap();
    checked
        .typed
        .trait_machine_signatures(definition)
        .iter()
        .find(|signature| signature.name.as_str() == requirement)
        .unwrap()
        .symbol
}

#[test]
fn retains_qualified_static_root_helper_port_and_boundary_settlement() {
    let checked = checked(
        r#"
        pub data Acknowledgement [linear] {
            root: u64;
            provider_execution: u64;
            invocation: u64;
            policy: u64;
            acknowledgement: u64;
        }

        pub domain Acknowledgement::Pending;

        boundary machine Acknowledgement::settle(self)
        reaches PortIo
        requires
            self in Acknowledgement::Pending
        ensures true;

        data Helper {}

        machine Helper::run(acknowledgement: Acknowledgement in Pending)
        reaches PortIo
        {
            asm { out 32, 7 }
            acknowledgement.settle();
        }

        data Root {}

        machine Root::enter(acknowledgement: Acknowledgement in Pending)
        reaches PortIo
        {
            Helper::run(acknowledgement);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root_symbol = machine_named(&checked, "enter");
    let helper_symbol = machine_named(&checked, "run");
    let settle_symbol = machine_named(&checked, "settle");
    let root = plans
        .for_machine(root_symbol)
        .expect("qualified static root plan");
    let helper = plans
        .for_machine(helper_symbol)
        .expect("qualified static helper plan");
    let settle = plans
        .boundary_for_machine(settle_symbol)
        .expect("boundary settlement plan");

    assert!(root.attachment_type_identity.is_none());
    assert!(helper.attachment_type_identity.is_none());
    assert_eq!(root.structural_parameters.len(), 1);
    assert_eq!(helper.structural_parameters.len(), 1);
    assert_eq!(
        root.structural_parameters[0].multiplicity,
        Multiplicity::Linear
    );
    assert_eq!(root.structural_parameters[0].qualifications.len(), 1);
    assert_eq!(root.entry_claims.len(), 1);
    assert!(root.entry_claims[0].path.is_empty());
    assert_eq!(helper.entry_claims.len(), 1);
    assert_eq!(settle.structural_parameters.len(), 1);
    assert_eq!(
        settle.structural_parameters[0].multiplicity,
        Multiplicity::Linear
    );
    assert_eq!(settle.domain_requirements.len(), 1);
    assert_eq!(settle.domain_requirements[0].argument_index, 0);
    assert!(root.service_reach.transitive.is_valid());
    assert!(helper.service_reach.direct.is_valid());
    assert!(helper.service_reach.transitive.is_valid());
    assert!(settle.service_reach.direct.is_valid());
    assert_ne!(root.contract_report_fingerprint, 0);
    assert_ne!(helper.contract_report_fingerprint, 0);
    assert_ne!(settle.contract_report_fingerprint, 0);

    let port_values = checked
        .facts
        .values
        .values
        .iter()
        .filter_map(|(_, value)| {
            matches!(
                value.origin,
                checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol,
                    state_symbol,
                    statement_index: 0,
                    role: checked_trees::CheckedValueStatementRole::CallArgument,
                } if machine_symbol == helper_symbol && state_symbol == helper.state
            )
            .then_some(value)
        })
        .collect::<Vec<_>>();
    assert_eq!(port_values.len(), 2);
    assert_eq!(port_values[0].primitive_type, Some(PrimitiveType::U16));
    assert_eq!(port_values[1].primitive_type, Some(PrimitiveType::U8));
    assert_eq!(
        port_values[0]
            .integer_range
            .as_ref()
            .and_then(|range| range.minimum.to_u64()),
        Some(32)
    );
    assert_eq!(
        port_values[1]
            .integer_range
            .as_ref()
            .and_then(|range| range.minimum.to_u64()),
        Some(7)
    );

    let acknowledgement = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity.contains("Acknowledgement"))
        .expect("acknowledgement shape");
    let acknowledgement_fields = record_fields(acknowledgement);
    assert_eq!(acknowledgement_fields.len(), 5);
    assert_eq!(
        acknowledgement_fields
            .iter()
            .map(|field| field.identity.as_str())
            .collect::<Vec<_>>(),
        [
            "root",
            "provider_execution",
            "invocation",
            "policy",
            "acknowledgement",
        ]
    );
    assert!(acknowledgement_fields.iter().all(|field| {
        field.field_type == CheckedUnitStructuralFieldType::Scalar(PrimitiveType::U64)
    }));

    assert_eq!(root.operations.len(), 2);
    match &root.operations[0] {
        CheckedUnitEffectOperationPlan::Call(call)
            if !call.is_boundary() && *call.result() == CheckedUnitCallResult::Unit =>
        {
            assert_eq!(call.coordinate.statement_index, 0);
            assert_eq!(call.coordinate.call_ordinal, 0);
            assert_eq!(call.target.machine, helper_symbol);
            assert_eq!(call.structural_arguments.len(), 1);
            assert_eq!(
                call.structural_arguments[0].source_parameter_index(),
                Some(0)
            );
            let claim_transfers = call.claim_transfers();
            assert_eq!(claim_transfers.len(), 1);
            assert_eq!(claim_transfers[0].argument_index, 0);
            assert_eq!(
                claim_transfers[0].claim_identity,
                root.entry_claims[0].claim_identity
            );
        }
        operation => panic!("unexpected root operation: {operation:?}"),
    }
    assert!(matches!(
        root.operations[1],
        CheckedUnitEffectOperationPlan::Complete {
            statement_index: 1,
            ..
        }
    ));

    assert_eq!(helper.operations.len(), 3);
    assert!(matches!(
        helper.operations[0],
        CheckedUnitEffectOperationPlan::PortWrite {
            coordinate: checked_trees::CheckedUnitCallCoordinate {
                statement_index: 0,
                call_ordinal: 0,
            },
            port: 32,
            value: 7,
            ..
        }
    ));
    match &helper.operations[1] {
        CheckedUnitEffectOperationPlan::Call(call)
            if call.is_boundary() && *call.result() == CheckedUnitCallResult::Unit =>
        {
            assert_eq!(call.coordinate.statement_index, 1);
            assert_eq!(call.coordinate.call_ordinal, 0);
            assert_eq!(call.target.machine, settle_symbol);
            assert_eq!(call.structural_arguments.len(), 1);
            let completion_receipts = call.claim_transfers();
            assert_eq!(completion_receipts.len(), 1);
            assert_eq!(completion_receipts[0].argument_index, 0);
            assert_eq!(
                completion_receipts[0].claim_identity,
                helper.entry_claims[0].claim_identity
            );
        }
        operation => panic!("unexpected helper operation: {operation:?}"),
    }
    assert!(matches!(
        helper.operations[2],
        CheckedUnitEffectOperationPlan::Complete {
            statement_index: 2,
            ..
        }
    ));
}
