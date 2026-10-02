//! Endpoint custody is retained before executable range admission.
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;
use checked_trees::CheckedScalarExpressionRole;

#[test]
fn shared_array_ranges_retain_backing_and_endpoint_evidence() {
    use checked_trees::{
        CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument,
        CheckedStorageRoot, CheckedUnitStructuralArgumentSourcePlan,
    };
    let checked = lower_typed_trees(
        typed_program(
            "data Buffer { values: [i32; 3]; }
             data Indirect { values: &[i32; 3]; }
             machine count(values: &[i32]) -> u64 { values.len }
             machine local() -> u64 {
                 let values: [i32; 3] = [11, 22, 33];
                 count(&values[1..3])
             }
             machine empty() -> u64 {
                 let values: [i32; 0] = [];
                 count(&values[..])
             }
             machine parameter(values: &[i32; 3]) -> u64 { count(&values[1..3]) }
             machine field(buffer: &Buffer) -> u64 { count(&buffer.values[1..3]) }
             machine indirect(buffer: &Indirect) -> u64 { count(&buffer.values[1..3]) }",
        ),
        &CheckingRequest::settled(),
    )
    .expect("array ranges retain exact shared loans");
    let computations = &checked.facts.values.scalar_computations;
    let mut roots = Vec::new();
    for (_, computation) in computations.nodes.iter() {
        let CheckedScalarComputationKind::Call {
            structural_arguments,
            ..
        } = computation.kind
        else {
            continue;
        };
        for argument in computations
            .structural_arguments
            .span_or_empty(structural_arguments)
        {
            let CheckedScalarComputationStructuralArgument::Place(argument) = argument else {
                continue;
            };
            let CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                root,
                start,
                end,
                ..
            } = &argument.source
            else {
                continue;
            };
            roots.push((*root, argument.path.len(), start.is_some(), end.is_some()));
        }
    }
    assert_eq!(
        roots.len(),
        4,
        "four direct backing operands use view ranges; a reference field needs a referent crossing: {roots:?}"
    );
    assert!(roots.iter().any(|(root, path, start, end)| matches!(
        root,
        CheckedStorageRoot::OwnedLocal { .. }
    ) && *path == 0
        && *start
        && *end));
    assert!(roots.iter().any(|(root, path, start, end)| matches!(
        root,
        CheckedStorageRoot::OwnedLocal { .. }
    ) && *path == 0
        && !*start
        && !*end));
    assert!(
        roots.iter().any(
            |(root, path, _, _)| matches!(root, CheckedStorageRoot::Parameter { .. }) && *path == 0
        )
    );
    assert!(
        roots.iter().any(
            |(root, path, _, _)| matches!(root, CheckedStorageRoot::Parameter { .. }) && *path == 1
        )
    );
}

#[test]
fn byte_subslice_full_view_retains_an_ordinary_checked_call_plan() {
    let checked = lower_typed_trees(
        typed_program(
            "boundary trait Host { machine write(bytes: &[u8]); }
         data Relay {}
         machine Relay::write(bytes: &[u8]) reaches Host { Host::write(bytes); }
         data Helper {}
         machine Helper::write(bytes: &[u8]) reaches Host {
            Relay::write(bytes[..]);
            Host::write(bytes);
         }",
        ),
        &CheckingRequest::settled(),
    )
    .expect("full exclusive byte view checks without extra bounds");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|name| name.as_str() == "Helper")
        })
        .expect("helper");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine.symbol)
        .expect("full view retains the callable helper");
    let Some(call) = plan.operations[0].call().filter(|call| {
        !call.is_boundary() && *call.result() == checked_trees::CheckedUnitCallResult::Unit
    }) else {
        panic!("ordinary Relay call")
    };
    assert!(matches!(
        call.structural_arguments[0].source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            root: checked_trees::CheckedStorageRoot::Parameter { index: 0 },
            start: None,
            end: None,
            ..
        }
    ));
    assert!(
        checked
            .facts
            .values
            .scalar_expressions
            .source_bindings
            .iter()
            .all(|(_, binding)| binding.state != plan.state
                || !matches!(
                    binding.role,
                    CheckedScalarExpressionRole::SubsliceStart { .. }
                        | CheckedScalarExpressionRole::SubsliceEnd { .. }
                ))
    );
}
