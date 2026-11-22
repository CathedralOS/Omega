//! A `[T]` held by value is an immutable sequence of completed elements. The
//! core access policy stores two of them: `Issuance::Only.routes` and
//! `AccessCallerRule.callers`. These tests lower ordinary programs of that
//! shape, and of every other element form, and read the published Psi with
//! the independent consumer: the sequence's exact element row, its stored
//! length and each ordered operand
//! (`wiki/spec/terminal-psi/structural_access.md#complete-structural-array-construction`).
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    BlockTerminator, CallArgument, CleanupAction, FieldInitializer, LoweredMachine, LoweredPackage, MachineBody, OperationKind, OperationResult, Parameter,
    PlaceId, PlaceOrigin, PsiOperation, ReferenceAccess, ReturnEdge, ScalarLiteral, ScalarType, TypeId, TypeShape, ValueId, ValueOperand, ValueOrigin,
    ViewElement,
};
use psi_handoff_consumer::{ElementSource, established_sequences, forwarded_sequence, nominal_row, returned_sequence, sequence_element};

const ROUTE: &str = "data Route { id:u32; }";
const FILE: &str = "sequences.omg";

fn lower(source: &str) -> LoweredPackage {
    sources::lower(&[(FILE, source)]).unwrap()
}

fn machine<'a>(
    package: &'a LoweredPackage,
    name: &str,
) -> &'a LoweredMachine {
    package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == name)
        .unwrap_or_else(|| panic!("machine {name}"))
}

/// The one by-value sequence row of the package and the `Route` data it holds.
fn route_sequence(package: &LoweredPackage) -> TypeId {
    let rows: Vec<_> = package
        .types
        .rows()
        .filter(|(_, shape)| matches!(shape, TypeShape::ElementSequence { .. }))
        .collect();
    let [(ty, TypeShape::ElementSequence { element })] = rows.as_slice() else {
        panic!("one sequence row, found {rows:?}")
    };
    assert_eq!(nominal_row(package, *element).unwrap().declaration.path, "Route");
    *ty
}

/// The operation that produced an element's place.
fn produced<'a>(source: &ElementSource<'a>) -> &'a PsiOperation {
    match source {
        ElementSource::Produced(operation) => operation,
        other => panic!("the element is an operation result, found {other:?}"),
    }
}

fn constant(
    body: &MachineBody,
    value: ValueId,
) -> &ScalarLiteral {
    let ValueOrigin::Operation(id) = body.values[value.index()].origin else {
        panic!("the value is an operation result")
    };
    let OperationKind::Constant(literal) = &body.operations[id.index()].kind else {
        panic!("the value is a constant")
    };
    literal
}

/// The `id` a `Route { id: n }` element was constructed with.
fn route_id(
    body: &MachineBody,
    producer: &PsiOperation,
) -> u32 {
    let OperationKind::EstablishRecord { fields } = &producer.kind else {
        panic!("the element is a constructed record, found {:?}", producer.kind)
    };
    let [
        FieldInitializer {
            field,
            operand: ValueOperand::Scalar(value),
        },
    ] = fields.as_slice()
    else {
        panic!("one scalar field")
    };
    assert_eq!(field.path, "Route::id");
    let ScalarLiteral::Integer(id) = constant(body, *value) else {
        panic!("an integer field")
    };
    u32::try_from(id).unwrap()
}

#[test]
fn an_empty_literal_and_its_two_record_twin_differ_only_in_their_ordered_roster() {
    let empty = lower(&format!("{ROUTE} machine routes()->[Route]{{ [] }}"));
    let two = lower(&format!("{ROUTE} machine routes()->[Route]{{ [Route{{id:4}}, Route{{id:9}}] }}"));
    // Both programs declare the same types: the empty literal needs no fixed
    // array and no borrowed view to stand in for it.
    assert_eq!(empty.types, two.types);
    let rows: Vec<_> = empty.types.rows().collect();
    let [
        (_, TypeShape::PrimitiveScalar(_)),
        (route, TypeShape::Nominal(_)),
        (_, sequence),
    ] = rows.as_slice()
    else {
        panic!("a scalar, the record and the sequence, found {rows:?}")
    };
    // The sequence names the record's own row as its element.
    assert_eq!(**sequence, TypeShape::ElementSequence { element: *route });

    let none = returned_sequence(&empty, "routes").unwrap();
    assert_eq!(nominal_row(&empty, none.element).unwrap().declaration.path, "Route");
    assert_eq!(Some(none.ty), machine(&empty, "routes").result_type);
    assert!(none.elements.is_empty());
    // The whole body is the construction: nothing supplies a placeholder element.
    let [only] = machine(&empty, "routes").body.operations.as_slice() else {
        panic!("one operation")
    };
    assert_eq!(only.kind, OperationKind::EstablishStructuralArray { elements: vec![] });

    let pair = returned_sequence(&two, "routes").unwrap();
    assert_eq!(nominal_row(&two, pair.element).unwrap().declaration.path, "Route");
    assert_eq!(pair.ty, none.ty);
    assert_eq!(pair.elements.len(), 2);
    let body = &machine(&two, "routes").body;
    let ids: Vec<_> = pair
        .sources
        .iter()
        .map(|source| route_id(body, produced(source)))
        .collect();
    spec_corpus::observe_handoff!(psi "psi-output/immutable-record-sequence", assert_eq!((none.elements.len(), ids.as_slice()), (0, &[4, 9][..])));
    assert_eq!(
        body.operations.last().unwrap().kind,
        OperationKind::EstablishStructuralArray {
            elements: pair.elements.to_vec()
        }
    );
}

#[test]
fn element_calls_run_once_in_authored_order_before_the_sequence_is_published() {
    let package = lower(&format!(
        "{ROUTE} machine route(id:u32)->Route{{ Route{{id:id}} }} machine routes()->[Route]{{ [route(7), route(2), route(5)] }}"
    ));
    let sequence = returned_sequence(&package, "routes").unwrap();
    let body = &machine(&package, "routes").body;
    let callee = machine(&package, "route").id;
    let mut arguments = Vec::new();
    for producer in sequence.sources.iter().map(produced) {
        let OperationKind::Call {
            callee: omega_lowering::CallTarget::Machine(target),
            arguments: supplied,
            ..
        } = &producer.kind
        else {
            panic!("each element is a call result, found {:?}", producer.kind)
        };
        assert_eq!(*target, callee);
        let [CallArgument::Scalar(value)] = supplied.as_slice() else {
            panic!("one scalar argument")
        };
        arguments.push(constant(body, *value).clone());
    }
    assert_eq!(arguments, [7, 2, 5].map(|id| ScalarLiteral::Integer(id.into())));
    let calls: Vec<_> = body
        .operations
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .map(|operation| operation.id)
        .collect();
    assert_eq!(
        calls,
        sequence
            .sources
            .iter()
            .map(|source| produced(source).id)
            .collect::<Vec<_>>()
    );
}

#[test]
fn owned_elements_move_into_the_sequence_and_leave_no_cleanup() {
    let package = lower(&format!("{ROUTE} machine routes(first:Route, second:Route)->[Route]{{ [second, first] }}"));
    let sequence = returned_sequence(&package, "routes").unwrap();
    let body = &machine(&package, "routes").body;
    let [Parameter::Structural(first), Parameter::Structural(second)] = body.parameters[..] else {
        panic!("two owned records")
    };
    let moved: Vec<_> = sequence
        .sources
        .iter()
        .map(|source| match &produced(source).kind {
            OperationKind::MoveStructural { source } if source.path.is_empty() => source.root,
            other => panic!("each element moves a whole formal, found {other:?}"),
        })
        .collect();
    assert_eq!(moved, [second, first]);
}

#[test]
fn a_sum_payload_and_a_record_field_store_the_same_sequence_type() {
    let source = format!(
        "{ROUTE}
         data Issuance {{ case Only(routes:[Route]); case Unrestricted; }}
         data CallerRule {{ region:u32; callers:[Route]; }}
         machine closed()->Issuance{{ Issuance::Only{{routes:[]}} }}
         machine default_closed()->Issuance{{ Issuance::Only{{..}} }}
         machine listed()->Issuance{{ Issuance::Only{{routes:[Route{{id:1}}, Route{{id:2}}]}} }}
         machine rule()->CallerRule{{ CallerRule{{region:3, callers:[Route{{id:8}}]}} }}"
    );
    let package = lower(&source);
    let sequence = route_sequence(&package);
    let data = |path: &str| {
        package
            .data
            .iter()
            .find(|data| data.declaration.path == path)
            .unwrap()
    };
    let [only, unrestricted] = data("Issuance").cases.as_slice() else {
        panic!("two cases")
    };
    assert!(unrestricted.fields.is_empty());
    let [routes] = only.fields.as_slice() else { panic!("one payload field") };
    assert_eq!(
        (routes.declaration.path.as_str(), routes.carrier, routes.lifetime),
        ("Issuance::Only::routes", sequence, None)
    );
    let [_, callers] = data("CallerRule").fields.as_slice() else {
        panic!("two fields")
    };
    assert_eq!(
        (callers.declaration.path.as_str(), callers.carrier, callers.lifetime),
        ("CallerRule::callers", sequence, None)
    );
    assert_eq!(
        nominal_row(&package, sequence_element(&package, sequence).unwrap())
            .unwrap()
            .declaration
            .path,
        "Route"
    );

    // Each constructor stores the one sequence its initializer established, by whole-value copy.
    for (owner, field, ids) in [
        ("closed", "Issuance::Only::routes", &[][..]),
        ("default_closed", "Issuance::Only::routes", &[][..]),
        ("listed", "Issuance::Only::routes", &[1, 2][..]),
        ("rule", "CallerRule::callers", &[8][..]),
    ] {
        let body = &machine(&package, owner).body;
        let established = established_sequences(&package, owner).unwrap();
        let [stored] = established.as_slice() else {
            panic!("{owner} establishes one sequence")
        };
        assert_eq!(stored.ty, sequence);
        let found: Vec<_> = stored
            .sources
            .iter()
            .map(|source| route_id(body, produced(source)))
            .collect();
        assert_eq!(found, ids, "{owner}");
        let fields = match &body.operations.last().unwrap().kind {
            OperationKind::EstablishCase { case, fields } => {
                assert_eq!(case.path, "Issuance::Only");
                fields
            }
            OperationKind::EstablishRecord { fields } => fields,
            other => panic!("{owner} ends in its construction, found {other:?}"),
        };
        let operand = &fields
            .iter()
            .find(|initializer| initializer.field.path == field)
            .unwrap()
            .operand;
        assert_eq!(*operand, ValueOperand::CopyWhole(stored.place), "{owner}");
    }

    // Discarding a record that stores a sequence is one whole discard: the sequence owes no action of its own.
    let dropped = lower(&format!(
        "{ROUTE} data CallerRule {{ region:u32; callers:[Route]; }} machine forget(rule:CallerRule){{ }}"
    ));
    let body = &machine(&dropped, "forget").body;
    let [Parameter::Structural(rule)] = body.parameters[..] else {
        panic!("one record")
    };
    let [block] = body.blocks.as_slice() else { panic!("one block") };
    assert!(matches!(&block.terminator, BlockTerminator::Return { result: ReturnEdge::Unit, cleanup, .. } if *cleanup == [CleanupAction::DiscardRoot(rule)]));
}

#[test]
fn a_fixed_array_of_slots_holds_rules_that_store_sequences() {
    // The core `AccessPlan` keeps `readers: [AccessRuleSlot; 32]`, and a slot's
    // rule stores its callers as a sequence. This is that nesting, two slots wide.
    let source = format!(
        "{ROUTE}
         data CallerRule {{ region:u32; callers:[Route]; }}
         data Slot {{ case Empty; case Rule(rule:CallerRule); }}
         machine slots()->[Slot;2]{{ [Slot::Empty{{}}, Slot::Rule{{rule:CallerRule{{region:1, callers:[Route{{id:2}}]}}}}] }}"
    );
    let package = lower(&source);
    let sequence = route_sequence(&package);
    let machine = machine(&package, "slots");
    let body = &machine.body;
    let slot = package
        .data
        .iter()
        .find(|data| data.declaration.path == "Slot")
        .unwrap();
    assert_eq!(
        package.types.get(machine.result_type.unwrap()),
        Some(&TypeShape::FixedArray {
            element: slot.carrier,
            length: 2
        })
    );
    let [callers] = established_sequences(&package, "slots")
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(callers.ty, sequence);
    assert_eq!(route_id(body, produced(&callers.sources[0])), 2);
    // The sequence is complete before the rule that stores it, and the array takes both slots in order.
    let cases: Vec<_> = body
        .operations
        .iter()
        .filter_map(|operation| match (&operation.kind, operation.result) {
            (OperationKind::EstablishCase { case, .. }, OperationResult::Structural(place)) => Some((case.path.as_str(), place)),
            _ => None,
        })
        .collect();
    let [("Slot::Empty", empty), ("Slot::Rule", rule)] = cases.as_slice() else {
        panic!("the two slots in authored order, found {cases:?}")
    };
    assert_eq!(
        body.operations.last().unwrap().kind,
        OperationKind::EstablishStructuralArray { elements: vec![*empty, *rule] }
    );
    assert!(established_sequences(&package, "slots").unwrap()[0].place.get() < rule.get());
}

/// The place a single-block machine returns.
fn returned_place(body: &MachineBody) -> PlaceId {
    let [block] = body.blocks.as_slice() else { panic!("one block") };
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source, .. },
        cleanup,
        ..
    } = &block.terminator
    else {
        panic!("a structural return")
    };
    assert!(cleanup.is_empty());
    *source
}

#[test]
fn a_sequence_travels_by_value_through_formals_locals_calls_and_joins() {
    let source = format!(
        "{ROUTE}
         machine pass(routes:[Route])->[Route]{{ routes }}
         machine rebind(routes:[Route])->[Route]{{ let first:[Route]=routes; let again:[Route]=routes; again }}
         machine forward()->[Route]{{ pass([Route{{id:6}}]) }}
         machine choose(listed:bool)->[Route]{{ match listed {{ true -> [Route{{id:1}}], false -> [], }} }}"
    );
    let package = lower(&source);
    let sequence = route_sequence(&package);

    // A formal and a result of the sequence type are structural places of that one row.
    for owner in ["pass", "rebind"] {
        let machine = machine(&package, owner);
        assert_eq!((machine.parameter_types.as_slice(), machine.result_type), (&[sequence][..], Some(sequence)));
        let [Parameter::Structural(formal)] = machine.body.parameters[..] else {
            panic!("a structural formal")
        };
        assert_eq!(
            machine.body.places[formal.index()],
            omega_lowering::Place {
                ty: sequence,
                origin: PlaceOrigin::Parameter
            }
        );
        // Returning or rebinding the formal copies nothing and establishes nothing.
        assert!(machine.body.operations.is_empty(), "{owner}");
        assert_eq!(returned_place(&machine.body), formal);
        assert_eq!(
            returned_sequence(&package, owner).unwrap_err(),
            "the returned sequence is not established in this body"
        );
    }

    // A call receives the whole sequence by copy and returns one of the same type.
    let forward = &machine(&package, "forward").body;
    let [argument] = established_sequences(&package, "forward")
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(route_id(forward, produced(&argument.sources[0])), 6);
    let call = forward.operations.last().unwrap();
    let OperationKind::Call { arguments, .. } = &call.kind else {
        panic!("the body ends in its call")
    };
    assert_eq!(arguments.as_slice(), [CallArgument::CopyWhole(argument.place)]);
    let OperationResult::Structural(result) = call.result else {
        panic!("a structural call result")
    };
    assert_eq!(forward.places[result.index()].ty, sequence);
    assert_eq!(returned_place(forward), result);

    // Each arm of a match establishes its own complete sequence and returns it.
    let lengths: Vec<_> = established_sequences(&package, "choose")
        .unwrap()
        .iter()
        .map(|established| established.elements.len())
        .collect();
    assert_eq!(lengths, [1, 0]);
}

#[test]
fn a_borrowed_view_and_a_fixed_array_stay_distinct_from_a_sequence() {
    let source = format!(
        "{ROUTE}
         machine owned(routes:[Route])->[Route]{{ routes }}
         machine viewed(routes:@[Route])->@[Route]{{ routes }}
         machine pair()->[Route;2]{{ [Route{{id:1}}, Route{{id:2}}] }}"
    );
    let package = lower(&source);
    let sequence = route_sequence(&package);
    let route = nominal_row(&package, sequence_element(&package, sequence).unwrap()).unwrap();

    let viewed = machine(&package, "viewed").result_type.unwrap();
    assert_ne!(viewed, sequence);
    assert_eq!(
        package.types.get(viewed),
        Some(&TypeShape::SharedElementView {
            element: ViewElement::Nominal(route.id)
        })
    );
    assert_eq!(machine(&package, "viewed").parameter_types, [viewed]);
    assert_eq!(sequence_element(&package, viewed).unwrap_err(), "a borrowed view is not a by-value sequence");

    // The fixed array uses the same establishing operation under its declared extent.
    let pair = machine(&package, "pair");
    let fixed = pair.result_type.unwrap();
    assert_eq!(
        package.types.get(fixed),
        Some(&TypeShape::FixedArray {
            element: route.carrier,
            length: 2
        })
    );
    assert!(matches!(&pair.body.operations.last().unwrap().kind, OperationKind::EstablishStructuralArray { elements } if elements.len() == 2));
    assert_eq!(
        returned_sequence(&package, "pair").unwrap_err(),
        "a fixed array has a declared extent, not a stored length"
    );
    assert!(established_sequences(&package, "pair").unwrap().is_empty());

    // Typing keeps the three apart before lowering: none converts to another.
    for (body, refusal) in [
        ("machine mix(routes:@[Route])->[Route]{ routes }", r"expected [app\Route], found @[app\Route]"),
        ("machine mix(routes:[Route])->@[Route]{ routes }", r"expected @[app\Route], found [app\Route]"),
        (
            "machine mix(routes:[Route;2])->[Route]{ routes }",
            r"expected [app\Route], found [app\Route; 2]",
        ),
    ] {
        let source = format!("{ROUTE} {body}");
        let package = sources::resolve(&[(FILE, &source)]);
        let Err(error) = omega_preparation::prepare_source_package(&package) else {
            panic!("typing refuses `{body}`")
        };
        assert_eq!(error.message, refusal);
    }
}

#[test]
fn a_linear_element_or_one_that_needs_cleanup_is_invalid_in_a_sequence() {
    // The sequence form admits no linear claim and no executable cleanup
    // (structural_access.md, complete structural array construction), so these
    // programs are invalid: a linear record, a record that holds one, and a
    // record with a selected cleanup.
    let refusal = "an immutable sequence cannot hold an element that is linear or needs cleanup";
    for (declarations, element) in [
        ("data Token [linear] { id:u32; }", "data Token [linear] { id:u32; }"),
        ("data Token [linear] { id:u32; } data Holder { token:Token; }", "data Holder { token:Token; }"),
        (
            "data Handle dropped by release { id:u32; } machine release(handle:Handle){ let Handle{id as _}=handle; }",
            "data Handle dropped by release { id:u32; }",
        ),
    ] {
        let name = element.split_whitespace().nth(1).unwrap();
        // The formal alone is refused: no construction is needed to reach the element rule.
        let source = format!("{declarations} machine keep(given:[{name}])->[{name}]{{ given }}");
        sources::refuse(&[(FILE, &source)], FILE, sources::unique_span(&source, element), refusal);
    }
    // Without the linear attribute the same formal is a sequence row.
    let plain = lower("data Token { id:u32; } machine keep(given:[Token])->[Token]{ given }");
    let kept = machine(&plain, "keep").result_type.unwrap();
    assert_eq!(
        nominal_row(&plain, sequence_element(&plain, kept).unwrap())
            .unwrap()
            .declaration
            .path,
        "Token"
    );
}

/// The value a scalar element's home was established from.
fn homed(source: &ElementSource<'_>) -> ValueId {
    match source {
        ElementSource::Homed(value) => *value,
        other => panic!("the element is a homed scalar, found {other:?}"),
    }
}

/// The one row of `package` with this shape.
fn row(
    package: &LoweredPackage,
    shape: TypeShape,
) -> TypeId {
    let found: Vec<_> = package
        .types
        .rows()
        .filter(|(_, listed)| **listed == shape)
        .map(|(id, _)| id)
        .collect();
    let [id] = found.as_slice() else {
        panic!("one row {shape:?}, found {found:?}")
    };
    *id
}

const U32: ScalarType = ScalarType::Integer { signed: false, width: 32 };
const U8: ScalarType = ScalarType::Integer { signed: false, width: 8 };

#[test]
fn a_scalar_sequence_homes_each_element_in_authored_order() {
    let source = "machine none()->[u32]{ [] }
         machine listed()->[u32]{ [3, 9] }
         machine computed(first:u32)->[u32]{ [first, first + 1] }
         machine pass(given:[u32])->[u32]{ given }";
    let package = lower(source);
    let scalar = row(&package, TypeShape::PrimitiveScalar(U32));
    let sequence = row(&package, TypeShape::ElementSequence { element: scalar });

    let none = returned_sequence(&package, "none").unwrap();
    assert_eq!((none.ty, none.element, none.elements.len()), (sequence, scalar, 0));

    // Each literal element is a constant in its own home, in authored order.
    let listed = returned_sequence(&package, "listed").unwrap();
    let body = &machine(&package, "listed").body;
    let values: Vec<_> = listed
        .sources
        .iter()
        .map(|source| constant(body, homed(source)).clone())
        .collect();
    assert_eq!((listed.ty, listed.element), (sequence, scalar));
    assert_eq!(values, [3, 9].map(|value| ScalarLiteral::Integer(value.into())));

    // A computed element is homed from its own value: the formal, then the sum.
    let computed = returned_sequence(&package, "computed").unwrap();
    let body = &machine(&package, "computed").body;
    let [Parameter::Scalar(first)] = body.parameters[..] else {
        panic!("one scalar formal")
    };
    let [from_formal, from_sum] = [0, 1].map(|ordinal| homed(&computed.sources[ordinal]));
    assert_eq!(from_formal, first);
    let ValueOrigin::Operation(sum) = body.values[from_sum.index()].origin else {
        panic!("the second element is computed")
    };
    assert!(matches!(&body.operations[sum.index()].kind, OperationKind::Integer { operands, .. } if operands.first() == Some(&first)));

    assert_eq!(forwarded_sequence(&package, "pass"), Ok(sequence));
}

#[test]
fn a_byte_sequence_is_an_element_sequence_and_never_a_byte_view() {
    let package = lower(
        "machine bytes()->[u8]{ [1, 2] }
         machine pass(given:[u8])->[u8]{ given }
         machine view(given:@[u8])->@[u8]{ given }",
    );
    let byte = row(&package, TypeShape::PrimitiveScalar(U8));
    let sequence = row(&package, TypeShape::ElementSequence { element: byte });
    let bytes = returned_sequence(&package, "bytes").unwrap();
    let body = &machine(&package, "bytes").body;
    let values: Vec<_> = bytes
        .sources
        .iter()
        .map(|source| constant(body, homed(source)).clone())
        .collect();
    assert_eq!((bytes.ty, bytes.element), (sequence, byte));
    assert_eq!(values, [1, 2].map(|value| ScalarLiteral::Integer(value.into())));
    assert_eq!(forwarded_sequence(&package, "pass"), Ok(sequence));

    // The borrowed `@[u8]` keeps its own descriptor kind. Neither row names the other.
    let view = machine(&package, "view").result_type.unwrap();
    assert_eq!(package.types.get(view), Some(&TypeShape::SharedByteView));
    assert_ne!(view, sequence);
    assert_eq!(sequence_element(&package, view).unwrap_err(), "a borrowed view is not a by-value sequence");
}

#[test]
fn a_sequence_of_shared_references_holds_each_descriptor_in_order() {
    let package = lower(&format!(
        "{ROUTE}
         machine listed(one:@Route, two:@Route)->[@Route]{{ [two, one] }}
         machine pass(given:[@Route])->[@Route]{{ given }}"
    ));
    let route = package
        .data
        .iter()
        .find(|data| data.declaration.path == "Route")
        .unwrap();
    let reference = row(
        &package,
        TypeShape::NominalReference {
            referent: route.id,
            access: ReferenceAccess::Shared,
        },
    );
    let sequence = row(&package, TypeShape::ElementSequence { element: reference });

    // Each element is the formal's own descriptor, so the loan it carries stays visible.
    let listed = returned_sequence(&package, "listed").unwrap();
    assert_eq!((listed.ty, listed.element), (sequence, reference));
    assert!(matches!(listed.sources.as_slice(), [ElementSource::Formal(1), ElementSource::Formal(0)]));
    assert_eq!(forwarded_sequence(&package, "pass"), Ok(sequence));
}

#[test]
fn arrays_and_sequences_nest_with_exact_element_rows() {
    let source = format!(
        "{ROUTE}
         machine pairs()->[[Route;2]]{{ [[Route{{id:1}}, Route{{id:2}}], [Route{{id:3}}, Route{{id:4}}]] }}
         machine lists()->[[Route]]{{ [[], [Route{{id:5}}]] }}
         machine slots()->[[Route];2]{{ [[Route{{id:6}}], []] }}
         machine pass_pairs(given:[[Route;2]])->[[Route;2]]{{ given }}
         machine pass_lists(given:[[Route]])->[[Route]]{{ given }}
         machine pass_slots(given:[[Route];2])->[[Route];2]{{ given }}"
    );
    let package = lower(&source);
    let route = package
        .data
        .iter()
        .find(|data| data.declaration.path == "Route")
        .unwrap()
        .carrier;
    let pair = row(&package, TypeShape::FixedArray { element: route, length: 2 });
    let routes = row(&package, TypeShape::ElementSequence { element: route });
    let pairs = row(&package, TypeShape::ElementSequence { element: pair });
    let lists = row(&package, TypeShape::ElementSequence { element: routes });
    let slots = row(&package, TypeShape::FixedArray { element: routes, length: 2 });
    // Every layer follows the row of its element.
    assert!(route.get() < pair.get() && route.get() < routes.get());
    assert!(pair.get() < pairs.get() && routes.get() < lists.get() && routes.get() < slots.get());

    // A sequence of fixed arrays: each element is one complete array.
    let established = returned_sequence(&package, "pairs").unwrap();
    let body = &machine(&package, "pairs").body;
    assert_eq!((established.ty, established.element), (pairs, pair));
    let ids: Vec<Vec<u32>> = established
        .sources
        .iter()
        .map(|source| {
            let OperationKind::EstablishStructuralArray { elements } = &produced(source).kind else {
                panic!("each element is a constructed array")
            };
            elements
                .iter()
                .map(|place| {
                    let PlaceOrigin::Operation(id) = body.places[place.index()].origin else {
                        panic!("a constructed record")
                    };
                    route_id(body, &body.operations[id.index()])
                })
                .collect()
        })
        .collect();
    assert_eq!(ids, [[1, 2], [3, 4]]);

    // A sequence of sequences: the inner ones are complete first, then held in order.
    let nested = established_sequences(&package, "lists").unwrap();
    let body = &machine(&package, "lists").body;
    let [empty, single, outer] = nested.as_slice() else {
        panic!("two inner sequences and the outer one, found {nested:?}")
    };
    assert_eq!((empty.ty, empty.elements.len(), single.ty), (routes, 0, routes));
    assert_eq!(route_id(body, produced(&single.sources[0])), 5);
    assert_eq!((outer.ty, outer.element, outer.elements), (lists, routes, &[empty.place, single.place][..]));
    assert_eq!(returned_sequence(&package, "lists").unwrap().place, outer.place);

    // A fixed array of sequences: the array takes each sequence's place under its declared extent.
    let inner = established_sequences(&package, "slots").unwrap();
    let slots_machine = machine(&package, "slots");
    let [first, second] = inner.as_slice() else {
        panic!("two sequences, found {inner:?}")
    };
    assert_eq!((first.elements.len(), second.elements.len()), (1, 0));
    assert_eq!(route_id(&slots_machine.body, produced(&first.sources[0])), 6);
    assert_eq!(slots_machine.result_type, Some(slots));
    assert_eq!(
        slots_machine.body.operations.last().unwrap().kind,
        OperationKind::EstablishStructuralArray {
            elements: vec![first.place, second.place]
        }
    );
    assert_eq!(
        returned_sequence(&package, "slots").unwrap_err(),
        "a fixed array has a declared extent, not a stored length"
    );

    assert_eq!(forwarded_sequence(&package, "pass_pairs"), Ok(pairs));
    assert_eq!(forwarded_sequence(&package, "pass_lists"), Ok(lists));
    let pass_slots = machine(&package, "pass_slots");
    assert_eq!((pass_slots.parameter_types.as_slice(), pass_slots.result_type), (&[slots][..], Some(slots)));
    assert!(pass_slots.body.operations.is_empty());
}

#[test]
fn an_element_keeps_its_callable_contract_and_its_carry_policy() {
    // A record that stores a callable: the sequence names the record's row, and
    // that row still names the callable's contract.
    let hooks = lower(
        "data Step=machine()->u8 reaches none crashes none;
         data Hook { run:Step; }
         machine listed(one:Hook, two:Hook)->[Hook]{ [two, one] }
         machine pass(given:[Hook])->[Hook]{ given }",
    );
    let sequence = forwarded_sequence(&hooks, "pass").unwrap();
    let hook = nominal_row(&hooks, sequence_element(&hooks, sequence).unwrap()).unwrap();
    let [run] = hook.fields.as_slice() else { panic!("one field") };
    let Some(TypeShape::Callable(contract)) = hooks.types.get(run.carrier) else {
        panic!("the field is a callable descriptor")
    };
    assert!(hooks.callable_contracts.get(contract.index()).is_some());
    // Each owned record moves into the sequence whole, with its descriptor inside.
    let listed = returned_sequence(&hooks, "listed").unwrap();
    let body = &machine(&hooks, "listed").body;
    let [Parameter::Structural(one), Parameter::Structural(two)] = body.parameters[..] else {
        panic!("two owned records")
    };
    let moved: Vec<_> = listed
        .sources
        .iter()
        .map(|source| match &produced(source).kind {
            OperationKind::MoveStructural { source } if source.path.is_empty() => source.root,
            other => panic!("each element moves a whole formal, found {other:?}"),
        })
        .collect();
    assert_eq!((listed.ty, moved.as_slice()), (sequence, &[two, one][..]));

    // A record with a carry policy: the element row publishes the same policy
    // the record has outside any sequence. Nothing here judges it.
    let mut published = Vec::new();
    for policy in [
        "suspension:allowed,cpu:any,thread:any,address:movable,",
        "suspension:forbidden,cpu:same,thread:any,address:stable,",
    ] {
        let declaration = format!("data Pass [carry({policy})] {{ id:u32; }}");
        let alone = lower(&format!("{declaration} machine unused()->u32{{ 0 }}"));
        let held = lower(&format!("{declaration} machine keep(given:[Pass])->[Pass]{{ given }}"));
        let carry = &alone
            .data
            .iter()
            .find(|data| data.declaration.path == "Pass")
            .unwrap()
            .carry;
        assert!(carry.is_some());
        let sequence = forwarded_sequence(&held, "keep").unwrap();
        let element = nominal_row(&held, sequence_element(&held, sequence).unwrap()).unwrap();
        assert_eq!((element.declaration.path.as_str(), &element.carry), ("Pass", carry));
        published.push(carry.clone());
    }
    assert_ne!(published[0], published[1]);
}
