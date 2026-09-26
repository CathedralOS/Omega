//! ATOMIC-MEMORY-MODEL: every serial atomic operation lowers to one Terminal
//! `AtomicAccess` event and executes from the serialized module, so decode
//! and independent verification stand between the lowered spelling and the
//! observed values. Each body reports the instruction-observed prior and the
//! following resident through a boundary effect; the atomic cell is a scalar
//! field of the borrowed receiver, supplied as a structural input.
//!
//! The rejection controls mutate the lowered event and check that the
//! verifier refuses an illegal ordering, a shared-borrow write, a mistyped or
//! dropped result, a store that claims a result, an undeclared field, and a
//! runtime-selected location; the codec refuses to publish any of them.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::{
    AcceptTerminalEffects, TerminalArtifactInterpretError, TerminalEffect, TerminalExecutionResult,
    TerminalScalarValue, TerminalStructuralInputs, TerminalStructuralScalarFieldValue,
    TerminalStructuralValue, interpret_terminal_artifact_measured,
};
use terminal_psi::{
    AtomicAccessEvent, AtomicReadModifyWrite, MemoryOrdering, OperationKind, OperationResult,
    StructuralAccess, StructuralTypeShape, TerminalModule,
};
use terminal_verifier::{AtomicAccessRefusal, ModuleError};

fn u32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 32).expect("u32")
}

fn u32_value(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: u32_type(),
        value: IntegerValue::Unsigned(value),
    }
}

/// One body over `self.counter`, reporting each value it names.
fn source(body: &str) -> String {
    format!(
        "boundary trait Report {{ machine value(v: u32) reaches Report; }}
        data Cell {{ counter: AtomicU32; }}
        machine Cell::run(&mut self) reaches Report {{
            {body}
        }}"
    )
}

fn lower(body: &str) -> checked_trees_to_lowered_psi::lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(&source(body));
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Cell::run"),
    )
    .unwrap_or_else(|error| panic!("{body}: {error:#?}"))
}

/// Run the serialized module with `counter` initially `initial`, returning
/// every reported value.
fn run(
    module: &TerminalModule,
    proof: &terminal_psi::ProofBundle,
    initial: u128,
) -> Result<Vec<u128>, TerminalArtifactInterpretError> {
    let machine = &module.machines[0];
    let receiver = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.is_self)
        .expect("the body retains its receiver");
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == receiver.structural_type)
        .expect("receiver type");
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        panic!("Cell is a record");
    };
    let counter = fields
        .iter()
        .find(|field| field.identity == "counter")
        .expect("counter field");
    let execution = interpret_terminal_artifact_measured(
        &encode_module(module).expect("canonical semantic bytes"),
        &encode_proof_section(module, proof).expect("canonical proof bytes"),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: receiver.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            scalar_fields: &[TerminalStructuralScalarFieldValue {
                argument_index: 0,
                path: Vec::new(),
                field: counter.id,
                value: u32_value(initial),
            }],
            ..Default::default()
        },
        &mut AcceptTerminalEffects,
    )?;
    assert_eq!(execution.value(), TerminalExecutionResult::Unit);
    Ok(execution
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("reported value");
            };
            let [
                TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(value),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("one u32 report");
            };
            *value
        })
        .collect())
}

fn execute(body: &str, initial: u128) -> Vec<u128> {
    let lowered = lower(body);
    run(&lowered.semantic_module, &lowered.proof_bundle, initial)
        .unwrap_or_else(|error| panic!("{body}: {error:#?}"))
}

/// The atomic events of the lowered entry machine, in block order.
fn events(module: &TerminalModule) -> Vec<AtomicAccessEvent> {
    module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::AtomicAccess { event, .. } => Some(event),
            _ => None,
        })
        .collect()
}

#[test]
fn store_and_load_carry_their_orderings_and_round_trip_the_value() {
    let body = "self.counter.store(42, Publish);
                let seen: u32 = self.counter.load(Receive);
                Report::value(seen);
                self.counter.store(43, GlobalOrder);
                let again: u32 = self.counter.load(GlobalOrder);
                Report::value(again);";
    let lowered = lower(body);
    assert!(matches!(
        events(&lowered.semantic_module).as_slice(),
        [
            AtomicAccessEvent::Store {
                ordering: MemoryOrdering::Publish,
                ..
            },
            AtomicAccessEvent::Load {
                ordering: MemoryOrdering::Receive
            },
            AtomicAccessEvent::Store {
                ordering: MemoryOrdering::GlobalOrder,
                ..
            },
            AtomicAccessEvent::Load {
                ordering: MemoryOrdering::GlobalOrder
            },
        ]
    ));
    assert_eq!(execute(body, 0), [42, 43]);
}

#[test]
fn every_fetch_returns_the_observed_prior_and_wraps_at_the_field_width() {
    for (method, operand, operation, initial, stored) in [
        ("fetch_add", 5, AtomicReadModifyWrite::FetchAdd, 10, 15),
        (
            "fetch_add",
            1,
            AtomicReadModifyWrite::FetchAdd,
            4_294_967_295,
            0,
        ),
        ("fetch_sub", 12, AtomicReadModifyWrite::FetchSub, 42, 30),
        (
            "fetch_sub",
            31,
            AtomicReadModifyWrite::FetchSub,
            30,
            4_294_967_295,
        ),
        ("fetch_and", 11, AtomicReadModifyWrite::FetchAnd, 14, 10),
        ("fetch_or", 5, AtomicReadModifyWrite::FetchOr, 10, 15),
        ("fetch_xor", 6, AtomicReadModifyWrite::FetchXor, 10, 12),
    ] {
        let body = format!(
            "let prior: u32 in Wrapping = self.counter.{method}({operand}, ReceivePublish);
             Report::value(prior as u32);
             let now: u32 = self.counter.load(NoOrdering);
             Report::value(now);"
        );
        let lowered = lower(&body);
        assert!(
            matches!(
                events(&lowered.semantic_module).as_slice(),
                [
                    AtomicAccessEvent::ReadModifyWrite {
                        operation: lowered_operation,
                        ordering: MemoryOrdering::ReceivePublish,
                        ..
                    },
                    AtomicAccessEvent::Load { .. },
                ] if *lowered_operation == operation
            ),
            "{method}: one read-modify-write event, never a read, arithmetic and store"
        );
        assert_eq!(
            run(&lowered.semantic_module, &lowered.proof_bundle, initial).unwrap(),
            [initial, stored],
            "{method}({operand}) over {initial}"
        );
    }
}

#[test]
fn swap_returns_the_displaced_resident() {
    let body = "let prior: u32 = self.counter.swap(42, ReceivePublish);
                Report::value(prior);
                let now: u32 = self.counter.load(NoOrdering);
                Report::value(now);";
    assert_eq!(execute(body, 10), [10, 42]);
}

#[test]
fn compare_exchange_keeps_both_orderings_and_exchanges_only_on_a_match() {
    let body = "let prior: u32 in Wrapping = self.counter.compare_exchange(10, 99, ReceivePublish, Receive);
                Report::value(prior as u32);
                let now: u32 = self.counter.load(NoOrdering);
                Report::value(now);";
    let lowered = lower(body);
    assert!(matches!(
        events(&lowered.semantic_module).as_slice(),
        [
            AtomicAccessEvent::CompareExchange {
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
                ..
            },
            AtomicAccessEvent::Load { .. },
        ]
    ));
    // Success: the observed prior equals `expected`, and the cell exchanges.
    assert_eq!(
        run(&lowered.semantic_module, &lowered.proof_bundle, 10).unwrap(),
        [10, 99]
    );
    // Failure: the observed prior differs, and the cell is unchanged.
    assert_eq!(
        run(&lowered.semantic_module, &lowered.proof_bundle, 7).unwrap(),
        [7, 7]
    );
}

/// Lower one fetch body, rewrite it, and return the verifier's refusal.
fn mutated(mutate: impl FnOnce(&mut TerminalModule)) -> ModuleError {
    let lowered = lower(
        "let prior: u32 in Wrapping = self.counter.fetch_add(5, NoOrdering);
         Report::value(prior as u32);",
    );
    let mut module = lowered.semantic_module;
    terminal_verifier::validate_module(&module).expect("the unmutated event verifies");
    mutate(&mut module);
    // The codec refuses to publish an unverifiable module at all.
    assert!(encode_module(&module).is_err());
    terminal_verifier::validate_module(&module)
        .map(|_| ())
        .expect_err("the mutated event must not verify")
}

fn atomic_operation(module: &mut TerminalModule) -> &mut terminal_psi::Operation {
    module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::AtomicAccess { .. }))
        .expect("the body lowers one atomic event")
}

fn event(module: &mut TerminalModule) -> &mut AtomicAccessEvent {
    let OperationKind::AtomicAccess { event, .. } = &mut atomic_operation(module).kind else {
        unreachable!("atomic_operation selects an atomic event")
    };
    event
}

fn refusal(error: ModuleError) -> AtomicAccessRefusal {
    match error {
        ModuleError::InvalidAtomicAccess { refusal, .. } => refusal,
        other => panic!("expected an atomic-access refusal, got {other:?}"),
    }
}

#[test]
fn verification_rejects_substituted_atomic_evidence() {
    // A load that publishes is not a source-legal ordering.
    let error = mutated(|module| {
        *event(module) = AtomicAccessEvent::Load {
            ordering: MemoryOrdering::Publish,
        };
    });
    assert_eq!(refusal(error), AtomicAccessRefusal::IllegalOrdering);

    // A compare-exchange failure that exceeds its success ordering.
    let error = mutated(|module| {
        let AtomicAccessEvent::ReadModifyWrite { operand, .. } = *event(module) else {
            unreachable!("the body lowers a fetch")
        };
        *event(module) = AtomicAccessEvent::CompareExchange {
            expected: operand,
            replacement: operand,
            success: MemoryOrdering::Receive,
            failure: MemoryOrdering::GlobalOrder,
        };
    });
    assert_eq!(refusal(error), AtomicAccessRefusal::IllegalOrdering);

    // A modifying event through a shared borrow: no atomic-cell identity
    // authorizes mutation through a shared loan yet.
    let error = mutated(|module| {
        for parameter in module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.structural_parameters)
            .filter(|parameter| parameter.is_self)
        {
            parameter.access = StructuralAccess::SharedBorrow;
        }
    });
    assert_eq!(refusal(error), AtomicAccessRefusal::UnwritableLocation);

    // The observed prior carries the field's own type, and an observing
    // event cannot drop it.
    let error = mutated(|module| {
        let OperationResult::Scalar(result) = &mut atomic_operation(module).result else {
            unreachable!("a fetch defines its prior")
        };
        result.scalar_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"));
    });
    assert_eq!(refusal(error), AtomicAccessRefusal::ResultShapeMismatch);
    let error = mutated(|module| atomic_operation(module).result = OperationResult::Unit);
    assert_eq!(refusal(error), AtomicAccessRefusal::ResultShapeMismatch);

    // A store claims no result.
    let error = mutated(|module| {
        let AtomicAccessEvent::ReadModifyWrite { operand, .. } = *event(module) else {
            unreachable!("the body lowers a fetch")
        };
        *event(module) = AtomicAccessEvent::Store {
            value: operand,
            ordering: MemoryOrdering::NoOrdering,
        };
    });
    assert_eq!(refusal(error), AtomicAccessRefusal::ResultShapeMismatch);

    // A field the record does not declare, and a runtime-selected location.
    let error = mutated(|module| {
        let OperationKind::AtomicAccess { field, .. } = &mut atomic_operation(module).kind else {
            unreachable!("atomic_operation selects an atomic event")
        };
        *field = semantic_vocabulary::StructuralFieldId::new(field.get() + 97).expect("field");
    });
    assert_eq!(refusal(error), AtomicAccessRefusal::UnresolvedField);
    let error = mutated(|module| {
        let AtomicAccessEvent::ReadModifyWrite { operand, .. } = *event(module) else {
            unreachable!("the body lowers a fetch")
        };
        let OperationKind::AtomicAccess { path, .. } = &mut atomic_operation(module).kind else {
            unreachable!("atomic_operation selects an atomic event")
        };
        path.push(terminal_psi::StructuralPathSegment::RuntimeIndex {
            index: operand,
            obligation: semantic_vocabulary::ObligationId::new(999).expect("obligation"),
        });
    });
    assert_eq!(refusal(error), AtomicAccessRefusal::RuntimeSelectedLocation);
}

/// Lowering rejoins every planned event to its authored carrier: a plan that
/// substitutes the operation, an ordering, the field, an operand, or the
/// prior's binding statement refuses instead of emitting a different event.
#[test]
fn lowering_rejects_a_plan_that_drifts_from_its_authored_carrier() {
    use typed_trees_to_checked_trees::checked_trees::{
        CheckedAtomicEvent, CheckedAtomicReadModifyWrite, CheckedUnitEffectOperationPlan,
    };
    let body = "self.counter.store(10, Publish);
                let prior: u32 in Wrapping = self.counter.fetch_add(5, ReceivePublish);
                Report::value(prior as u32);";
    let checked = crate::front_end::checked_program(&source(body));
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Cell::run"),
    )
    .expect("the authored plan lowers");
    let mutations: [(&str, fn(&mut CheckedUnitEffectOperationPlan)); 6] = [
        ("store ordering", |operation| {
            if let CheckedUnitEffectOperationPlan::AtomicAccess(access) = operation
                && let CheckedAtomicEvent::Store { ordering, .. } = &mut access.event
            {
                *ordering = MemoryOrdering::GlobalOrder;
            }
        }),
        ("fetch operation", |operation| {
            if let CheckedUnitEffectOperationPlan::AtomicAccess(access) = operation
                && let CheckedAtomicEvent::ReadModifyWrite {
                    operation: fetch, ..
                } = &mut access.event
            {
                *fetch = CheckedAtomicReadModifyWrite::FetchSub;
            }
        }),
        ("fetch as swap", |operation| {
            if let CheckedUnitEffectOperationPlan::AtomicAccess(access) = operation
                && let CheckedAtomicEvent::ReadModifyWrite {
                    ordering, operand, ..
                } = &access.event
            {
                access.event = CheckedAtomicEvent::Swap {
                    ordering: *ordering,
                    value: operand.clone(),
                };
            }
        }),
        ("field", |operation| {
            if let CheckedUnitEffectOperationPlan::AtomicAccess(access) = operation {
                access.field_identity = String::from("other");
            }
        }),
        ("operand", |operation| {
            if let CheckedUnitEffectOperationPlan::AtomicAccess(access) = operation
                && let CheckedAtomicEvent::Store { value, .. } = &mut access.event
            {
                *value = typed_trees_to_checked_trees::checked_trees::CheckedCallScalarArgument::Pure(
                    typed_trees_to_checked_trees::checked_trees::CheckedScalarExpression::IntegerLiteral {
                        literal: numerics::literals::IntegerLiteral::zero(),
                    },
                );
            }
        }),
        ("prior binding", |operation| {
            if let CheckedUnitEffectOperationPlan::AtomicAccess(access) = operation
                && let Some(result) = &mut access.result
            {
                result.statement_index = result.statement_index.wrapping_sub(1);
            }
        }),
    ];
    for (name, mutate) in mutations {
        let mut changed = checked.clone();
        let mut touched = false;
        for operation in changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.operations)
            .filter(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::AtomicAccess(_))
            })
        {
            let before = operation.clone();
            mutate(operation);
            touched |= *operation != before;
        }
        assert!(touched, "{name}: the mutation applies to one planned event");
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Cell::run"),
            )
            .is_err(),
            "{name}: a drifted atomic plan must not lower"
        );
    }
}
