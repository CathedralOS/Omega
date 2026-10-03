//! ATOMIC-MEMORY-MODEL: every serial atomic operation lowers to one Terminal
//! `AtomicAccess` event and executes from the serialized module, so decode
//! and independent verification stand between the lowered spelling and the
//! observed values. Each body reports the instruction-observed prior and the
//! following resident through a boundary effect; the atomic cell is a scalar
//! field of the borrowed receiver, supplied as a structural input.
//!
//! The rejection controls mutate the lowered event and check that the
//! verifier refuses an illegal ordering, a forged plain-cell identity, a mistyped or
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

/// One body over `cell.counter`, reporting each value it names.
fn source(body: &str) -> String {
    format!(
        "boundary trait Report {{ machine value(v: u32) reaches Report; }}
        data Cell {{ counter: AtomicU32; }}
        machine Cell::run(cell: &Cell) reaches Report {{
            {body}
        }}"
    )
}

fn lower(body: &str) -> lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(&source(body));
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Cell::run"),
    )
    .unwrap_or_else(|error| panic!("{body}: {error:#?}"))
}

/// Run the serialized module with `counter` initially `initial`.
fn interpret(
    module: &TerminalModule,
    proof: &terminal_psi::ProofBundle,
    initial: u128,
) -> Result<terminal_interpreter::MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let machine = &module.machines[0];
    let receiver = machine
        .structural_parameters
        .iter()
        .next()
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
    interpret_terminal_artifact_measured(
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
    )
}

/// Return every reported value from a Unit body.
fn run(
    module: &TerminalModule,
    proof: &terminal_psi::ProofBundle,
    initial: u128,
) -> Result<Vec<u128>, TerminalArtifactInterpretError> {
    let execution = interpret(module, proof, initial)?;
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
    let body = "cell.counter.store(42, Publish);
                let seen: u32 = cell.counter.load(Receive);
                Report::value(seen);
                cell.counter.store(43, GlobalOrder);
                let again: u32 = cell.counter.load(GlobalOrder);
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
fn scalar_completion_covers_effects_and_their_result_bindings() {
    for observation in [
        "cell.counter.fetch_or(5, ReceivePublish)",
        "cell.counter.swap(15, ReceivePublish)",
        "cell.counter.load(Receive)",
    ] {
        let source = format!(
            "data Cell {{ counter: AtomicU32; }}
             machine Cell::mark(cell: &Cell) -> u32 {{
                 cell.counter.store(10, Publish);
                 let prior: u32 = {observation};
                 transition {{ _ -> (prior) }}
             }}"
        );
        let checked = crate::front_end::checked_program(&source);
        let lowered = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("Cell::mark"),
        )
        .unwrap_or_else(|error| panic!("{observation}: {error:#?}"));
        assert_eq!(
            interpret(&lowered.semantic_module, &lowered.proof_bundle, 0)
                .expect("serialized scalar completion executes")
                .value(),
            TerminalExecutionResult::Scalar(u32_value(10)),
            "{observation}: the store precedes the returned observation"
        );

        let mut omitted = checked.clone();
        let mut removed = false;
        for machine in &mut omitted.facts.flow.terminal_unit_effects.machines {
            machine.operations.retain(|operation| {
                let observing = matches!(operation,
                    checked_trees::CheckedUnitEffectOperationPlan::AtomicAccess(access)
                        if access.result.is_some());
                removed |= observing;
                !observing
            });
        }
        assert!(
            removed,
            "the observing effect is in the ordinary operation roster"
        );
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &omitted,
                TerminalMachineSelection::Name("Cell::mark"),
            )
            .is_err(),
            "{observation}: neither an effect nor its binding may disappear"
        );
    }
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
            "let prior: u32 in Wrapping = cell.counter.{method}({operand}, ReceivePublish);
             Report::value(prior as u32);
             let now: u32 = cell.counter.load(NoOrdering);
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
    let body = "let prior: u32 = cell.counter.swap(42, ReceivePublish);
                Report::value(prior);
                let now: u32 = cell.counter.load(NoOrdering);
                Report::value(now);";
    assert_eq!(execute(body, 10), [10, 42]);
}

#[test]
fn compare_exchange_keeps_both_orderings_and_exchanges_only_on_a_match() {
    let body = "let prior: u32 in Wrapping = cell.counter.compare_exchange(10, 99, ReceivePublish, Receive);
                Report::value(prior as u32);
                let now: u32 = cell.counter.load(NoOrdering);
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
        "let prior: u32 in Wrapping = cell.counter.fetch_add(5, NoOrdering);
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

    // Equal-width plain scalar storage cannot acquire atomic authority by
    // borrowing it shared or relabeling the event.
    let error = mutated(|module| {
        for declaration in &mut module.structural_types {
            if let StructuralTypeShape::Record { fields } = &mut declaration.shape {
                for field in fields {
                    if let terminal_psi::StructuralFieldType::Atomic(scalar) = field.field_type {
                        field.field_type = terminal_psi::StructuralFieldType::Scalar(scalar);
                    }
                }
            }
        }
    });
    assert_eq!(refusal(error), AtomicAccessRefusal::UnresolvedField);

    let error = mutated(|module| {
        for parameter in module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.structural_parameters)
        {
            parameter.access = StructuralAccess::WriteOnlyBorrow;
        }
    });
    assert_eq!(refusal(error), AtomicAccessRefusal::UnreadableLocation);

    // Replacing an atomic observation with an ordinary field load would lose
    // the event's ordering. Shared authority cannot admit that substitution.
    let error = mutated(|module| {
        let operation = atomic_operation(module);
        let OperationKind::AtomicAccess { place, field, .. } = operation.kind else {
            unreachable!("selected an atomic event");
        };
        operation.kind = OperationKind::IntegerStructuralField {
            source: place,
            path: Vec::new(),
            field,
        };
    });
    assert!(matches!(
        error,
        ModuleError::InvalidIntegerStructuralField { .. }
    ));

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

#[test]
fn owned_element_view_descriptors_do_not_authorize_plain_atomic_snapshots() {
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, StructuralFieldId, StructuralTypeId,
    };
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType,
        StructuralTypeDeclaration,
    };

    for nested in [false, true] {
        let error = mutated(|module| {
            let cell = module.machines[0].structural_parameters[0].structural_type;
            let next_identity = module
                .structural_types
                .iter()
                .map(|declaration| declaration.id.get())
                .max()
                .unwrap()
                + 1;
            let view = StructuralTypeId::new(next_identity).unwrap();
            module.structural_types.push(StructuralTypeDeclaration {
                id: view,
                identity: "borrowed cells".into(),
                shape: StructuralTypeShape::ElementView { element: cell },
            });
            let (root, path) = if nested {
                let wrapper = StructuralTypeId::new(next_identity + 1).unwrap();
                let field = StructuralFieldId::new(1).unwrap();
                module.structural_types.push(StructuralTypeDeclaration {
                    id: wrapper,
                    identity: "owned view container".into(),
                    shape: StructuralTypeShape::Record {
                        fields: vec![StructuralFieldDeclaration {
                            id: field,
                            identity: "cells".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(view),
                        }],
                    },
                });
                (
                    wrapper,
                    vec![
                        CanonicalStructuralPathSegment::Field(field),
                        CanonicalStructuralPathSegment::FixedIndex(0),
                    ],
                )
            } else {
                (view, vec![CanonicalStructuralPathSegment::FixedIndex(0)])
            };
            let parameter = &mut module.machines[0].structural_parameters[0];
            parameter.structural_type = root;
            parameter.access = StructuralAccess::Owned;

            let operation = atomic_operation(module);
            let OperationKind::AtomicAccess { place, field, .. } = operation.kind else {
                unreachable!("selected an atomic event");
            };
            operation.kind = OperationKind::IntegerStructuralField {
                source: place,
                path,
                field,
            };
        });
        assert!(
            matches!(error, ModuleError::InvalidIntegerStructuralField { .. }),
            "owned descriptor (nested={nested}) must not own borrowed atomic backing: {error:?}"
        );
    }
}

/// Lowering rejoins every planned event to its authored carrier: a plan that
/// substitutes the operation, an ordering, the field, an operand, or the
/// prior's binding statement refuses instead of emitting a different event.
#[test]
fn lowering_rejects_a_plan_that_drifts_from_its_authored_carrier() {
    use checked_trees::{
        CheckedAtomicEvent, CheckedAtomicReadModifyWrite, CheckedUnitEffectOperationPlan,
    };
    let body = "cell.counter.store(10, Publish);
                let prior: u32 in Wrapping = cell.counter.fetch_add(5, ReceivePublish);
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
                *value = checked_trees::CheckedCallScalarArgument::Pure(
                    checked_trees::CheckedScalarExpression::IntegerLiteral {
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
