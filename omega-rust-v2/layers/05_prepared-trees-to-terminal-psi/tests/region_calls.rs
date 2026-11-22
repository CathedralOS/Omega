//! A call of a machine with region binders selects that machine's one body and
//! names, beside the call, what fills each of the callee's binders: a region
//! binder of the caller, or the operands that carry a region the caller has no
//! binder for.
#[allow(dead_code)]
#[path = "obligation_outputs/pipeline.rs"]
mod pipeline;
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    AggregateRegionSlot, BlockTerminator, CallArgument, CallRegion, CallRegions, CallSite, CallTarget, FieldInitializer, LoweredData, LoweredField,
    LoweredMachine, LoweredPackage, MachineRegion, OperationKind, OperationResult, Parameter, PlaceId, PlaceOrigin, PlaceProjection, ProjectedPlace,
    PsiOperation, ReferenceAccess, ReturnEdge, ValueOperand,
};
use psi_handoff_consumer::{CarriedLoan, RegionAtCall, applied_regions, regions_at_call};

const FILE: &str = pipeline::FILE;

/// `first` returns a borrow of its formal's field at its one region.
const CELL: &str = "data Cell {value:u8;}\nmachine first<'a>(input:@'a Cell)->@'a u8 {@input.value}\n";
/// `Schema` stores a view at its lifetime. `forward` and `size` take it by value.
const SCHEMA: &str = "data Schema<'t> [copy] {storage:@'t [u8];}\n\
    machine forward<'t>(schema:Schema<'t>)->Schema<'t> {schema}\n\
    machine size<'t>(schema:Schema<'t>)->u64 {7}\n";

fn direct(
    parameters: &[usize],
    result: bool,
) -> MachineRegion {
    MachineRegion {
        reference_parameters: parameters.to_vec(),
        reference_result: result,
        ..Default::default()
    }
}

fn aggregate(
    parameters: &[(usize, usize)],
    result: &[usize],
) -> MachineRegion {
    MachineRegion {
        aggregate_parameters: parameters
            .iter()
            .map(|&(parameter, binder)| AggregateRegionSlot { parameter, binder })
            .collect(),
        aggregate_result: result.to_vec(),
        ..Default::default()
    }
}

/// The one `Schema` row, whatever region is applied to it.
fn schema(package: &LoweredPackage) -> &LoweredData {
    package
        .data
        .iter()
        .find(|data| data.declaration.path == "Schema")
        .expect("the package lowers `Schema`")
}

/// The loan the reader reports when the operand at `parameter` stores, in
/// `field`, the descriptor copied from `backing`.
fn loan(
    parameter: usize,
    field: &LoweredField,
    backing: PlaceId,
) -> CarriedLoan<'_> {
    CarriedLoan {
        parameter,
        field: Some(&field.declaration),
        backing: ProjectedPlace {
            root: backing,
            path: Vec::new(),
        },
    }
}

fn calls(machine: &LoweredMachine) -> Vec<&PsiOperation> {
    machine
        .body
        .operations
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .collect()
}

/// `relay` passes its own formal to `first` at its own region. The source can
/// write the lifetime argument or leave it to typing, and the caller can have
/// a second binder. Psi names `first`'s one body and the binder the call applies.
#[test]
fn a_call_names_the_callers_region_that_fills_the_callees_region() {
    let cases = [
        ("machine relay<'u>(cell:@'u Cell)->@'u u8 {first::<'u>(cell)}", 0, vec![direct(&[0], true)]),
        ("machine relay<'u>(cell:@'u Cell)->@'u u8 {first(cell)}", 0, vec![direct(&[0], true)]),
        // `'u` reaches only `other`. The operand and the result are at `'v`, the second binder.
        (
            "machine relay<'u,'v>(other:@'u Cell, cell:@'v Cell)->@'v u8 {first::<'v>(cell)}",
            1,
            vec![direct(&[0], false), direct(&[1], true)],
        ),
    ];
    for (relay, ordinal, regions) in cases {
        let source = format!("{CELL}{relay}\n");
        let lowered = pipeline::lower(&source);
        let package = &lowered.package;
        assert!(package.machine_templates.is_empty(), "{relay}");
        let (first, relay) = (lowered.machine("first"), lowered.machine("relay"));
        assert_eq!(first.regions, [direct(&[0], true)]);
        assert_eq!(relay.regions, regions);
        // One body each: a region is not a static argument of either machine.
        assert!(first.arguments.is_empty() && relay.arguments.is_empty());
        assert!(first.body.call_regions.is_empty(), "`first` calls nothing");

        let [call] = calls(relay)[..] else { panic!("{:?}", relay.body.operations) };
        let OperationKind::Call { callee, arguments, .. } = &call.kind else {
            unreachable!()
        };
        assert_eq!(*callee, CallTarget::Machine(first.id));
        // The operand lends the referent of the formal that carries the applied region.
        let [CallArgument::Borrow { place, access }] = arguments.as_slice() else {
            panic!("{arguments:?}")
        };
        assert_eq!(Parameter::Structural(place.root), relay.body.parameters[regions.len() - 1]);
        assert_eq!((place.path.as_slice(), *access), (&[PlaceProjection::Referent][..], ReferenceAccess::Shared));

        let site = CallSite::Operation(call.id);
        assert_eq!(
            relay.body.call_regions,
            [CallRegions {
                call: site,
                arguments: vec![CallRegion::Binder(ordinal)]
            }]
        );
        let (selected, applied) = applied_regions(package, relay, site).unwrap();
        assert_eq!(selected.declaration, first.declaration);
        let [region] = applied.as_slice() else { panic!("{applied:?}") };
        spec_corpus::observe_handoff!(psi "psi-output/applied-caller-region", assert_eq!(
            (region.callee, region.ordinal, region.caller),
            (&first.regions[0], ordinal, &relay.regions[ordinal])
        ));
        // The callee's region reaches its result, and the caller returns that result
        // at the region the call applied.
        assert!(region.callee.reference_result && region.caller.reference_result);
        let OperationResult::Structural(result) = call.result else {
            panic!("{:?}", call.result)
        };
        let [ReturnEdge::Structural { source: returned, .. }] = pipeline::returns(&relay.body)[..] else {
            panic!()
        };
        assert_eq!(returned, result);
    }
}

/// The lifetime argument must be the region the operand is at. Typing refuses
/// the other binder, so lowering never chooses between them.
#[test]
fn typing_refuses_a_region_argument_the_operand_is_not_at() {
    let source = format!("{CELL}machine relay<'u,'v>(other:@'u Cell, cell:@'v Cell)->@'v u8 {{first::<'u>(cell)}}\n");
    let package = sources::resolve(&[(FILE, &source)]);
    let error = omega_preparation::prepare_source_package(&package).expect_err("`cell` is at `'v`");
    assert_eq!(error.message, r"conflicting type argument app\relay<parameter 0>");
}

/// A region inside a record is applied the same way. The result of one call
/// is the operand of the next, and a caller's caller applies its own binder.
#[test]
fn a_record_region_is_applied_through_a_chain_of_calls() {
    let source = format!(
        "{SCHEMA}machine relay<'u,'v>(other:Schema<'u>, schema:Schema<'v>)->u64 {{\n    let kept = forward::<'v>(schema);\n    size::<'v>(kept)\n}}\n\
         machine outer<'w>(schema:Schema<'w>)->u64 {{relay::<'w,'w>(schema, schema)}}\n"
    );
    let lowered = pipeline::lower(&source);
    let package = &lowered.package;
    assert!(package.machine_templates.is_empty());
    let [forward, size, relay, outer] = ["forward", "size", "relay", "outer"].map(|name| lowered.machine(name));
    assert_eq!(forward.regions, [aggregate(&[(0, 0)], &[0])]);
    assert_eq!(relay.regions, [aggregate(&[(0, 0)], &[]), aggregate(&[(1, 0)], &[])]);

    let [kept, sized] = calls(relay)[..] else {
        panic!("{:?}", relay.body.operations)
    };
    for (call, callee) in [(kept, forward), (sized, size)] {
        let (selected, applied) = applied_regions(package, relay, CallSite::Operation(call.id)).unwrap();
        assert_eq!(selected.id, callee.id);
        // Both calls apply `'v`, the binder `schema` fills, and never `'u`.
        assert_eq!(applied.iter().map(|region| region.ordinal).collect::<Vec<_>>(), [1]);
    }
    // `size` receives the record `forward` returned, not the formal.
    let OperationResult::Structural(forwarded) = kept.result else {
        panic!("{:?}", kept.result)
    };
    let OperationKind::Call { arguments, .. } = &sized.kind else { unreachable!() };
    assert_eq!(arguments.as_slice(), [CallArgument::CopyWhole(forwarded)]);

    // `outer` fills both of `relay`'s regions with its one binder.
    let [call] = calls(outer)[..] else { panic!("{:?}", outer.body.operations) };
    let (selected, applied) = applied_regions(package, outer, CallSite::Operation(call.id)).unwrap();
    assert_eq!(selected.id, relay.id);
    assert_eq!(applied.iter().map(|region| region.ordinal).collect::<Vec<_>>(), [0, 0]);
}

/// A terminal call has no operation to carry the row. Each edge carries its own.
#[test]
fn a_terminal_call_names_its_regions_on_its_edge() {
    let source = format!(
        "{SCHEMA}machine relay<'u>(schema:Schema<'u>, wide:bool)->u64 {{\n    transition wide {{ true -> size::<'u>(schema), false -> size::<'u>(schema), }}\n}}\n"
    );
    let lowered = pipeline::lower(&source);
    let (size, relay) = (lowered.machine("size"), lowered.machine("relay"));
    let edges: Vec<_> = relay
        .body
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            BlockTerminator::TerminalCall { id, .. } => Some(CallSite::Edge(*id)),
            _ => None,
        })
        .collect();
    assert_eq!(edges.len(), 2);
    assert_eq!(
        relay.body.call_regions,
        edges
            .iter()
            .map(|&call| CallRegions {
                call,
                arguments: vec![CallRegion::Binder(0)]
            })
            .collect::<Vec<_>>()
    );
    for edge in edges {
        let (selected, applied) = applied_regions(&lowered.package, relay, edge).unwrap();
        assert_eq!((selected.id, applied.len()), (size.id, 1));
    }
}

/// `start` declares no region binder and builds a `Schema` from its formal.
const START: &str = "machine start(bytes:@[u8])->u64 {\n    let held = Schema{storage: bytes};\n    size(held)\n}\n";

/// Typing infers the region of `held` where the record is built. No binder of
/// `start` names that region, so the call's row says its operands carry it.
/// From Psi alone the reader follows the operand to the record's
/// establishment, and from the field stored at the record's lifetime to the
/// descriptor copied into it: `start`'s own formal.
#[test]
fn a_region_the_callers_body_inferred_is_carried_by_the_calls_operands() {
    let source = format!("{SCHEMA}{START}");
    let lowered = pipeline::lower(&source);
    let package = &lowered.package;
    assert!(package.machine_templates.is_empty());
    let (size, start) = (lowered.machine("size"), lowered.machine("start"));
    // Lowering mints no region for `start` and binds the formal to none.
    assert!(start.regions.is_empty() && start.arguments.is_empty() && start.body.lifetime_bindings.is_empty());
    assert_eq!(size.regions, [aggregate(&[(0, 0)], &[])]);
    let body = &start.body;
    let Parameter::Structural(bytes) = body.parameters[0] else {
        panic!("{:?}", body.parameters)
    };
    assert_eq!(body.places[bytes.index()].origin, PlaceOrigin::Parameter);

    // The record stores the formal's own descriptor in the field at its lifetime.
    let stored = &schema(package).fields[0];
    assert_eq!(stored.lifetime, Some(0));
    let [call] = calls(start)[..] else { panic!("{:?}", body.operations) };
    let OperationKind::Call { callee, arguments, .. } = &call.kind else {
        unreachable!()
    };
    assert_eq!(*callee, CallTarget::Machine(size.id));
    let [CallArgument::CopyWhole(held)] = arguments.as_slice() else {
        panic!("{arguments:?}")
    };
    assert_eq!(
        pipeline::place_producer(body, *held).kind,
        OperationKind::EstablishRecord {
            fields: vec![FieldInitializer {
                field: stored.declaration.clone(),
                operand: ValueOperand::CopyWhole(bytes),
            }]
        }
    );

    let site = CallSite::Operation(call.id);
    assert_eq!(
        body.call_regions,
        [CallRegions {
            call: site,
            arguments: vec![CallRegion::Operands]
        }]
    );
    let (selected, regions) = regions_at_call(package, start, site).unwrap();
    assert_eq!(selected.id, size.id);
    let [RegionAtCall::Operands { callee, carried }] = regions.as_slice() else {
        panic!("{regions:?}")
    };
    spec_corpus::observe_handoff!(psi "psi-output/operand-carried-call-region", assert_eq!(
        (*callee, carried.as_slice()),
        (&size.regions[0], &[loan(0, stored, bytes)][..])
    ));
    // The row is not a binder of the caller, and the binder-only reader says so.
    assert!(applied_regions(package, start, site).is_err());
}

/// One call can apply a declared binder to one region of its callee and leave
/// another to its operands. A record a call returns at an inferred region is
/// still traced to the descriptor it was built from.
#[test]
fn an_inferred_region_stays_apart_from_a_declared_binder_and_follows_a_returned_record() {
    let source = format!(
        "{SCHEMA}machine pair<'x,'y>(left:Schema<'x>, right:Schema<'y>)->u64 {{7}}\n\
         machine mixed<'u>(kept:Schema<'u>, bytes:@[u8])->u64 {{\n    let held = Schema{{storage: bytes}};\n    pair::<'u>(kept, held)\n}}\n\
         machine twice(bytes:@[u8])->u64 {{\n    let held = Schema{{storage: bytes}};\n    let again = forward(held);\n    size(again)\n}}\n"
    );
    let lowered = pipeline::lower(&source);
    let package = &lowered.package;
    assert!(package.machine_templates.is_empty());
    let [forward, size, pair, mixed, twice] = ["forward", "size", "pair", "mixed", "twice"].map(|name| lowered.machine(name));
    let stored = &schema(package).fields[0];

    // `'u` fills `pair`'s first region by name. The second is only what `held` carries.
    assert_eq!(mixed.regions, [aggregate(&[(0, 0)], &[])]);
    let Parameter::Structural(bytes) = mixed.body.parameters[1] else {
        panic!("{:?}", mixed.body.parameters)
    };
    let [call] = calls(mixed)[..] else { panic!("{:?}", mixed.body.operations) };
    let site = CallSite::Operation(call.id);
    assert_eq!(
        mixed.body.call_regions,
        [CallRegions {
            call: site,
            arguments: vec![CallRegion::Binder(0), CallRegion::Operands]
        }]
    );
    let (selected, regions) = regions_at_call(package, mixed, site).unwrap();
    assert_eq!(selected.id, pair.id);
    let [
        RegionAtCall::Binder(named),
        RegionAtCall::Operands { callee, carried },
    ] = regions.as_slice()
    else {
        panic!("{regions:?}")
    };
    assert_eq!((named.callee, named.ordinal, named.caller), (&pair.regions[0], 0, &mixed.regions[0]));
    assert_eq!((*callee, carried.as_slice()), (&pair.regions[1], &[loan(1, stored, bytes)][..]));

    // `forward` returns its operand's region, so `size` reads `bytes` through that call.
    assert!(twice.regions.is_empty());
    let Parameter::Structural(bytes) = twice.body.parameters[0] else {
        panic!("{:?}", twice.body.parameters)
    };
    let [forwarded, sized] = calls(twice)[..] else {
        panic!("{:?}", twice.body.operations)
    };
    for (call, callee) in [(forwarded, forward), (sized, size)] {
        let (selected, regions) = regions_at_call(package, twice, CallSite::Operation(call.id)).unwrap();
        assert_eq!(selected.id, callee.id);
        let [RegionAtCall::Operands { carried, .. }] = regions.as_slice() else {
            panic!("{regions:?}")
        };
        assert_eq!(carried.as_slice(), [loan(0, stored, bytes)]);
    }
    let OperationResult::Structural(returned) = forwarded.result else {
        panic!("{:?}", forwarded.result)
    };
    let OperationKind::Call { arguments, .. } = &sized.kind else { unreachable!() };
    assert_eq!(arguments.as_slice(), [CallArgument::CopyWhole(returned)]);
}

/// Two records built from different views are at two inferred regions. Typing
/// refuses to give one callee region both, so lowering never merges them.
#[test]
fn typing_refuses_one_callee_region_for_records_at_two_inferred_regions() {
    let source = format!(
        "{SCHEMA}machine both<'t>(left:Schema<'t>, right:Schema<'t>)->u64 {{7}}\n\
         machine two(a:@[u8], b:@[u8])->u64 {{\n    let x = Schema{{storage: a}};\n    let y = Schema{{storage: b}};\n    both(x, y)\n}}\n"
    );
    let package = sources::resolve(&[(FILE, &source)]);
    let error = omega_preparation::prepare_source_package(&package).expect_err("`x` and `y` are at different regions");
    // The refusal is at the callee's name in the call.
    let call = sources::unique_span(&source, "both(x, y)");
    assert_eq!(
        (error.span.get_start_offset(), error.span.get_end_offset()),
        (call.start, call.start + "both".len())
    );
    assert!(
        error
            .message
            .starts_with(r"no matching overload for both(app\Schema<lifetime "),
        "{}",
        error.message
    );
}

/// The reader accepts the published row and refuses each misstatement of it:
/// a binder `start` does not have, a missing row, a second row, and an
/// operand-carried claim for a record the caller received at its own binder.
#[test]
fn a_reader_refuses_a_row_that_misstates_where_the_region_comes_from() {
    let source = format!("{SCHEMA}{START}machine relay<'u>(schema:Schema<'u>)->u64 {{size::<'u>(schema)}}\n");
    let mut lowered = pipeline::lower(&source);
    let index = |lowered: &pipeline::Lowered<'_>, name: &str| {
        lowered
            .package
            .machines
            .iter()
            .position(|machine| machine.declaration.path == name)
            .unwrap()
    };
    let (start, relay) = (index(&lowered, "start"), index(&lowered, "relay"));
    let read = |lowered: &pipeline::Lowered<'_>, machine: usize| {
        let machine = &lowered.package.machines[machine];
        let [call] = calls(machine)[..] else {
            panic!("{:?}", machine.body.operations)
        };
        regions_at_call(&lowered.package, machine, CallSite::Operation(call.id)).map(|(_, regions)| regions.len())
    };
    assert_eq!((read(&lowered, start), read(&lowered, relay)), (Ok(1), Ok(1)));

    let published = lowered.package.machines[start].body.call_regions.clone();
    let faults: [(&str, Vec<CallRegions>); 3] = [
        (
            "names region 0 of a caller with 0",
            vec![CallRegions {
                arguments: vec![CallRegion::Binder(0)],
                ..published[0].clone()
            }],
        ),
        ("has 0 region rows", vec![]),
        ("has 2 region rows", vec![published[0].clone(), published[0].clone()]),
    ];
    for (reason, rows) in faults {
        lowered.package.machines[start].body.call_regions = rows;
        let error = read(&lowered, start).expect_err(reason);
        assert!(error.contains(reason), "{error}");
    }
    lowered.package.machines[start].body.call_regions = published;
    assert_eq!(read(&lowered, start), Ok(1));

    // `relay` passes its own formal, which it did not build, at its own binder.
    lowered.package.machines[relay].body.call_regions[0].arguments = vec![CallRegion::Operands];
    let error = read(&lowered, relay).expect_err("the operand is a formal");
    assert!(error.contains("operand 0 was not established in this body"), "{error}");
}

/// A call of a machine without region binders carries no row, so a consumer
/// can tell "no regions" from a missing row.
#[test]
fn a_call_of_a_machine_without_regions_has_no_row() {
    let source = format!("{SCHEMA}machine seven()->u64 {{7}}\nmachine relay<'u>(schema:Schema<'u>)->u64 {{seven()}}\n");
    let lowered = pipeline::lower(&source);
    let (seven, relay) = (lowered.machine("seven"), lowered.machine("relay"));
    assert!(relay.body.call_regions.is_empty());
    let [call] = calls(relay)[..] else { panic!("{:?}", relay.body.operations) };
    let (selected, applied) = applied_regions(&lowered.package, relay, CallSite::Operation(call.id)).unwrap();
    assert_eq!((selected.id, applied.len()), (seven.id, 0));
}
