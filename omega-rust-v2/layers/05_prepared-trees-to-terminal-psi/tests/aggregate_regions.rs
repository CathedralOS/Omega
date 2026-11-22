//! A machine region can sit inside a record. `forward<'t>(schema: Schema<'t>) -> Schema<'t>`
//! has one executable body for every `'t` when this package lowers `Schema`:
//! the machine publishes which of the record's lifetime binders its region
//! fills on each parameter and on the result. A record this package does not
//! lower, and a body that needs a region elsewhere, stay with the open template.
#[allow(dead_code)]
#[path = "obligation_outputs/pipeline.rs"]
mod pipeline;
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    AggregateRegionSlot, LoweredData, MachineRegion, MachineTemplateBinder, OperationKind, OperationResult, Parameter, PlaceProjection, ReferenceAccess,
    ReturnEdge, TemplateBinder, TypeShape, ValueOperand,
};
use omega_typed::ObligationKind;

const FILE: &str = pipeline::FILE;
const FORWARDED: &str = include_str!("fixtures/aggregate_regions/forwarded_record.omg");
const NESTED: &str = include_str!("fixtures/aggregate_regions/nested_record.omg");
const REFERRED: &str = include_str!("fixtures/aggregate_regions/referred_record.omg");
const UNRESTRICTED: &str = include_str!("fixtures/domain_access/unrestricted/access_plan.omg");

fn region(
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

fn data<'a>(
    lowered: &'a pipeline::Lowered<'_>,
    path: &str,
) -> &'a LoweredData {
    lowered
        .package
        .data
        .iter()
        .find(|data| data.declaration.path == path)
        .unwrap_or_else(|| panic!("no lowered data {path}"))
}

#[test]
fn a_forwarded_record_publishes_one_body_and_the_binders_each_region_fills() {
    let lowered = pipeline::lower(FORWARDED);
    let package = &lowered.package;
    // Every machine has an executable body, and none remains an open template.
    assert!(package.machine_templates.is_empty(), "{:?}", package.machine_templates);
    assert_eq!(package.machines.len(), 5);

    // The records keep their binders, and each stored view names the binder it lives at.
    let (schema, span, route) = (data(&lowered, "Schema"), data(&lowered, "Span"), data(&lowered, "Route"));
    assert_eq!((schema.lifetime_count, span.lifetime_count, route.lifetime_count), (1, 2, 1));
    assert_eq!(schema.fields[0].lifetime, Some(0));
    assert_eq!((span.fields[0].lifetime, span.fields[1].lifetime), (Some(0), Some(1)));
    assert_eq!(route.cases[0].fields[0].lifetime, Some(0));
    for view in schema
        .fields
        .iter()
        .chain(&span.fields)
        .chain(&route.cases[0].fields)
    {
        assert_eq!(package.types.get(view.carrier), Some(&TypeShape::SharedByteView));
    }

    // `(parameter, binder)` slots and result binders, per region in declaration order,
    // then the record each parameter holds and the formal the body returns.
    let expected: [(&str, Vec<MachineRegion>, &[&LoweredData], (usize, &str)); 5] = [
        ("forward", vec![region(&[(0, 0)], &[0])], &[schema], (0, "schema")),
        // `'a` reaches only `left`. The result is at `'b`, which only `right` carries.
        ("pick", vec![region(&[(0, 0)], &[]), region(&[(1, 0)], &[0])], &[schema, schema], (1, "right")),
        ("keep", vec![region(&[(0, 0)], &[0]), region(&[(0, 1)], &[1])], &[span], (0, "span")),
        // One region fills both binders of the same parameter and no result.
        ("fold", vec![region(&[(0, 0), (0, 1)], &[])], &[span], (1, "count")),
        ("route", vec![region(&[(0, 0)], &[0])], &[route], (0, "route")),
    ];
    let mut returned_loans = lowered.obligations("ReturnBorrow").into_iter();
    for (name, regions, records, (returned, tail)) in expected {
        let machine = lowered.machine(name);
        let body = &machine.body;
        assert_eq!(machine.regions, regions, "{name}");
        // A region is not a static argument: the one body is not a specialization.
        assert!(machine.arguments.is_empty() && body.lifetime_bindings.is_empty(), "{name}");
        // Each record parameter is the declaration's one row, whatever region is applied to it.
        for (position, record) in records.iter().enumerate() {
            assert_eq!(machine.parameter_types[position], record.carrier, "{name}");
            assert_eq!(package.types.get(record.carrier), Some(&TypeShape::Nominal(record.id)));
        }
        // A consumer reads each slot against the record at that parameter or result.
        for region in &machine.regions {
            for slot in &region.aggregate_parameters {
                assert!(slot.binder < records[slot.parameter].lifetime_count, "{name}");
            }
        }
        let returns = pipeline::returns(body);
        let [edge] = returns.as_slice() else { panic!("{name}: {returns:?}") };
        match *edge {
            ReturnEdge::Structural { source, result } => {
                // The body returns the formal the source names, so the checker can
                // compare that formal's slots with the result's.
                assert_eq!(Parameter::Structural(source), body.parameters[returned], "{name}");
                assert_eq!(Some(body.places[result.index()].ty), machine.result_type, "{name}");
                assert_eq!(machine.result_type, Some(records[returned].carrier), "{name}");
                let result_binders = machine
                    .regions
                    .iter()
                    .flat_map(|region| &region.aggregate_result);
                assert!(
                    result_binders
                        .into_iter()
                        .all(|&binder| binder < records[returned].lifetime_count),
                    "{name}"
                );
                // Typing hands this return to the lifetime checker at the same expression.
                let loan = returned_loans
                    .next()
                    .unwrap_or_else(|| panic!("{name} returns a record that can carry a loan"));
                assert!(
                    matches!(&loan.kind, ObligationKind::ReturnBorrow { machine: owner } if owner.path == machine.declaration.path),
                    "{name}"
                );
                assert_eq!(lowered.spelling(&loan.site), tail, "{name}");
            }
            ReturnEdge::Scalar(value) => {
                assert_eq!((name, Parameter::Scalar(value)), ("fold", body.parameters[returned]));
            }
            other => panic!("{name}: {other:?}"),
        }
    }
    assert!(returned_loans.next().is_none());
}

#[test]
fn typing_refuses_a_result_at_another_region_before_lowering_sees_it() {
    // Lowering publishes the declared relation and the returned formal. It never
    // receives a body that returns `left` at `'b`, so it has nothing to reconcile.
    let swapped = FORWARDED.replace("    right\n", "    left\n");
    assert_ne!(swapped, FORWARDED);
    let package = sources::resolve(&[(FILE, &swapped)]);
    let error = omega_preparation::prepare_source_package(&package).expect_err("the result is at `'b`");
    let at = swapped.find("    left\n").unwrap() + 4;
    assert_eq!((error.span.get_start_offset(), error.span.get_end_offset()), (at, at + "left".len()));
    assert_eq!(
        error.message,
        r"expected app\Schema<app\pick<parameter 1>>, found app\Schema<app\pick<parameter 0>>"
    );
}

#[test]
fn a_record_that_stores_a_reference_to_a_record_at_its_region_is_forwarded_whole() {
    let lowered = pipeline::lower(REFERRED);
    let package = &lowered.package;
    // Every record is concrete data and every machine has an executable body.
    assert!(package.data_templates.is_empty() && package.machine_templates.is_empty());
    let (bytes, schema, pair) = (data(&lowered, "Bytes"), data(&lowered, "Schema"), data(&lowered, "Pair"));
    assert_eq!((bytes.lifetime_count, schema.lifetime_count, pair.lifetime_count), (1, 1, 2));

    // The field's row is the shared reference to the one row of `Bytes`.
    // Beside it the field keeps two separate facts: the binder of its own
    // record the reference is held at, and the binder of its own record that
    // fills each binder of `Bytes`.
    let referred = TypeShape::NominalReference {
        access: ReferenceAccess::Shared,
        referent: bytes.id,
    };
    let [storage] = schema.fields.as_slice() else { panic!("{:?}", schema.fields) };
    assert_eq!(package.types.get(storage.carrier), Some(&referred));
    assert_eq!((storage.lifetime, storage.aggregate_lifetimes.as_slice()), (Some(0), &[0][..]));
    // `first: @'r Bytes<'v>` in a record over `'r, 'v`: held at binder 0, and
    // binder 1 fills the one binder of `Bytes`. `count: u64` names neither.
    let [first, count] = pair.fields.as_slice() else {
        panic!("{:?}", pair.fields)
    };
    assert_eq!(package.types.get(first.carrier), Some(&referred));
    assert_eq!((first.lifetime, first.aggregate_lifetimes.as_slice()), (Some(0), &[1][..]));
    assert_eq!(first.aggregate_lifetimes.len(), bytes.lifetime_count);
    assert_eq!((count.lifetime, count.aggregate_lifetimes.as_slice()), (None, &[][..]));

    let expected: [(&str, &LoweredData, Vec<MachineRegion>, &str); 2] = [
        ("forward", schema, vec![region(&[(0, 0)], &[0])], "schema"),
        // `keep<'p, 'q>(pair: Pair<'q, 'p>)`: `'p` fills binder 1, the region
        // of the record referred to, and `'q` binder 0, where the reference is held.
        ("keep", pair, vec![region(&[(0, 1)], &[1]), region(&[(0, 0)], &[0])], "pair"),
    ];
    let loans = lowered.obligations("ReturnBorrow");
    for (name, record, regions, tail) in expected {
        let machine = lowered.machine(name);
        let body = &machine.body;
        assert_eq!(machine.regions, regions, "{name}");
        assert!(machine.arguments.is_empty() && body.lifetime_bindings.is_empty(), "{name}");
        assert_eq!(
            (machine.parameter_types.as_slice(), machine.result_type),
            (&[record.carrier][..], Some(record.carrier)),
            "{name}"
        );
        // The body runs no operation and returns its formal. The stored
        // reference travels inside the record and nothing crosses it.
        assert!(body.operations.is_empty(), "{name}: {:?}", body.operations);
        let returns = pipeline::returns(body);
        let [ReturnEdge::Structural { source, result }] = returns.as_slice() else {
            panic!("{name}: {returns:?}")
        };
        assert_eq!(Parameter::Structural(*source), body.parameters[0], "{name}");
        assert_eq!(body.places[result.index()].ty, record.carrier, "{name}");
        // Typing hands this return to the lifetime checker at the returned formal.
        let returned: Vec<_> = loans
            .iter()
            .filter(|loan| matches!(&loan.kind, ObligationKind::ReturnBorrow { machine: owner } if owner.path == name))
            .collect();
        let [loan] = returned.as_slice() else { panic!("{name}: {returned:?}") };
        assert_eq!(lowered.spelling(&loan.site), tail, "{name}");
    }
}

#[test]
fn a_body_that_reads_through_the_stored_reference_names_that_crossing() {
    // The twin of `forward`. The same record arrives, and the body copies out
    // the record behind its stored reference instead of returning the formal.
    // Lowering does not judge whether the view that `Bytes` holds may be read
    // through an incoming reference. It publishes the read as what it is, so
    // the checker meets the crossing and no row or slot stands in for it.
    let read = format!("{REFERRED}\npublic machine inner<'t>(schema: Schema<'t>) -> Bytes<'t> {{\n    $schema.storage\n}}\n");
    let lowered = pipeline::lower(&read);
    let (bytes, schema) = (data(&lowered, "Bytes"), data(&lowered, "Schema"));
    let machine = lowered.machine("inner");
    let body = &machine.body;
    let Parameter::Structural(formal) = body.parameters[0] else {
        panic!("{:?}", body.parameters)
    };
    // The one operation copies from the formal through the stored field and
    // then through the reference stored there, into a place of `Bytes`' row.
    let [operation] = body.operations.as_slice() else {
        panic!("{:?}", body.operations)
    };
    let (OperationKind::CopyStructural { source }, OperationResult::Structural(copy)) = (&operation.kind, operation.result) else {
        panic!("{operation:?}")
    };
    assert_eq!(source.root, formal);
    assert_eq!(
        source.path,
        [
            PlaceProjection::Field(schema.fields[0].declaration.clone()),
            PlaceProjection::Referent
        ]
    );
    assert_eq!(body.places[copy.index()].ty, bytes.carrier);
    // The body returns that copy, not the formal.
    let returns = pipeline::returns(body);
    let [ReturnEdge::Structural { source: returned, .. }] = returns.as_slice() else {
        panic!("{returns:?}")
    };
    assert_eq!(*returned, copy);
    assert_ne!(copy, formal);
    // The slots state the declared types of the formal and the result, as
    // they do for `forward`, whose body in the same package still crosses nothing.
    assert_eq!(machine.regions, [region(&[(0, 0)], &[0])]);
    assert!(lowered.machine("forward").body.operations.is_empty());
}

#[test]
fn typing_refuses_a_result_that_trades_the_reference_region_for_the_referent_region() {
    // `Pair<'q, 'p>` holds its reference at `'q` and refers to a record at
    // `'p`. A result of `Pair<'p, 'q>` would trade the two, so the two facts
    // of the field are not interchangeable, and lowering never sees this body.
    let traded = REFERRED.replace("(pair: Pair<'q, 'p>) -> Pair<'q, 'p> {", "(pair: Pair<'q, 'p>) -> Pair<'p, 'q> {");
    assert_ne!(traded, REFERRED);
    let package = sources::resolve(&[(FILE, &traded)]);
    let error = omega_preparation::prepare_source_package(&package).expect_err("the result trades the two regions");
    let at = traded.find("    pair\n").unwrap() + 4;
    assert_eq!((error.span.get_start_offset(), error.span.get_end_offset()), (at, at + "pair".len()));
    assert_eq!(
        error.message,
        r"expected app\Pair<app\keep<parameter 0>, app\keep<parameter 1>>, found app\Pair<app\keep<parameter 1>, app\keep<parameter 0>>"
    );
}

#[test]
fn typing_refuses_a_stored_reference_with_no_written_lifetime_before_lowering_sees_it() {
    // The twin of the forwarded record: only the lifetime on the reference
    // itself is gone. Borrow-carrying data declares the lifetimes of its
    // views, so typing refuses the field, and lowering never receives a
    // record with no binder to publish for it.
    let unwritten = REFERRED.replace("storage: @'t Bytes<'t>;", "storage: @Bytes<'t>;");
    assert_ne!(unwritten, REFERRED);
    let package = sources::resolve(&[(FILE, &unwritten)]);
    let error = omega_preparation::prepare_source_package(&package).expect_err("the stored reference names no lifetime");
    let at = unwritten.find("storage: @Bytes<'t>;").unwrap() + "storage: ".len();
    assert_eq!((error.span.get_start_offset(), error.span.get_end_offset()), (at, at + "@Bytes<'t>".len()));
    assert_eq!(error.message, "a stored reference must name its lifetime");

    // The ownership control: with the lifetime written, the same record keeps
    // its row and `forward` its one body, and the field names where it is held.
    let lowered = pipeline::lower(REFERRED);
    assert!(lowered.package.data_templates.is_empty() && lowered.package.machine_templates.is_empty());
    let stored = &data(&lowered, "Schema").fields[0];
    assert_eq!((stored.lifetime, stored.aggregate_lifetimes.as_slice()), (Some(0), &[0][..]));
    assert_eq!(lowered.machine("forward").regions, [region(&[(0, 0)], &[0])]);
}

/// The descriptor a rebuilt record's field at `position` holds: the operand of
/// the establishment that `returned` came from, with the operation that
/// produced that operand.
fn rebuilt_field(
    body: &omega_lowering::MachineBody,
    returned: omega_lowering::PlaceId,
    position: usize,
) -> (&omega_lowering::DeclarationId, &OperationKind, omega_lowering::TypeId) {
    let OperationKind::EstablishRecord { fields } = &pipeline::place_producer(body, returned).kind else {
        panic!("the returned record is not established in this body")
    };
    let field = &fields[position];
    let ValueOperand::CopyWhole(view) = field.operand else {
        panic!("{:?}", field.operand)
    };
    (&field.field, &pipeline::place_producer(body, view).kind, body.places[view.index()].ty)
}

/// A copy of the descriptor stored in `field` of the record at `root`.
fn stored_copy(
    root: omega_lowering::PlaceId,
    field: &omega_lowering::LoweredField,
) -> OperationKind {
    OperationKind::CopyStructural {
        source: omega_lowering::ProjectedPlace {
            root,
            path: vec![PlaceProjection::Field(field.declaration.clone())],
        },
    }
}

#[test]
fn rebuilding_a_record_from_its_stored_view_copies_the_stored_descriptor() {
    // The body reads the stored view. That value is at `'t` and has no slot of
    // its own: the body copies the descriptor out of the formal's field, and
    // the copy names its backing. The machine keeps the forwarded twin's one
    // body and region slots.
    let rebuilt = FORWARDED.replace("{\n    schema\n}", "{\n    Schema::<'t>{storage: schema.storage}\n}");
    assert_ne!(rebuilt, FORWARDED);
    let lowered = pipeline::lower(&rebuilt);
    let package = &lowered.package;
    assert!(package.machine_templates.is_empty(), "{:?}", package.machine_templates);
    assert_eq!(package.machines.len(), 5);
    let schema = data(&lowered, "Schema");
    let stored = &schema.fields[0];
    let forward = lowered.machine("forward");
    let body = &forward.body;
    assert_eq!(forward.regions, [region(&[(0, 0)], &[0])]);
    // Still no specialization, and nothing binds or applies a region by name.
    assert!(forward.arguments.is_empty() && body.lifetime_bindings.is_empty() && body.call_regions.is_empty());
    let Parameter::Structural(formal) = body.parameters[0] else {
        panic!("{:?}", body.parameters)
    };

    let returns = pipeline::returns(body);
    let [ReturnEdge::Structural { source, result }] = returns.as_slice() else {
        panic!("{returns:?}")
    };
    assert_eq!(
        (body.places[source.index()].ty, body.places[result.index()].ty),
        (schema.carrier, schema.carrier)
    );
    assert_eq!(forward.result_type, Some(schema.carrier));
    spec_corpus::observe_handoff!(psi "psi-output/rebuilt-stored-view", assert_eq!(
        (rebuilt_field(body, *source, 0), stored.lifetime),
        ((&stored.declaration, &stored_copy(formal, stored), stored.carrier), Some(0))
    ));
    // Typing hands the returned record to the lifetime checker at the construction.
    let loans = lowered.obligations("ReturnBorrow");
    let returned: Vec<_> = loans
        .iter()
        .filter(|loan| matches!(&loan.kind, ObligationKind::ReturnBorrow { machine } if machine.path == "forward"))
        .collect();
    let [loan] = returned.as_slice() else { panic!("{returned:?}") };
    assert_eq!(lowered.spelling(&loan.site), "Schema::<'t>{storage: schema.storage}");
}

#[test]
fn a_rebuilt_record_takes_each_region_from_the_field_its_source_names() {
    // `pick` rebuilds its result from `right`, the only formal at `'b`. `keep`
    // rebuilds both fields, each from the field at the same region. Every
    // copy names its own backing, so two regions never share one.
    let rebuilt = FORWARDED
        .replace("{\n    right\n}", "{\n    Schema::<'b>{storage: right.storage}\n}")
        .replace("{\n    span\n}", "{\n    Span::<'x, 'y>{head: span.head, tail: span.tail}\n}");
    assert_eq!(rebuilt.matches("::<'").count(), 2);
    let lowered = pipeline::lower(&rebuilt);
    assert!(lowered.package.machine_templates.is_empty());
    let (schema, span) = (data(&lowered, "Schema"), data(&lowered, "Span"));
    let returned = |body: &omega_lowering::MachineBody| {
        let returns = pipeline::returns(body);
        let [ReturnEdge::Structural { source, .. }] = returns.as_slice() else {
            panic!("{returns:?}")
        };
        *source
    };

    let pick = lowered.machine("pick");
    assert_eq!(pick.regions, [region(&[(0, 0)], &[]), region(&[(1, 0)], &[0])]);
    let Parameter::Structural(right) = pick.body.parameters[1] else {
        panic!("{:?}", pick.body.parameters)
    };
    assert_eq!(
        rebuilt_field(&pick.body, returned(&pick.body), 0),
        (&schema.fields[0].declaration, &stored_copy(right, &schema.fields[0]), schema.fields[0].carrier)
    );

    let keep = lowered.machine("keep");
    assert_eq!(keep.regions, [region(&[(0, 0)], &[0]), region(&[(0, 1)], &[1])]);
    let Parameter::Structural(formal) = keep.body.parameters[0] else {
        panic!("{:?}", keep.body.parameters)
    };
    for (position, field) in span.fields.iter().enumerate() {
        assert_eq!(field.lifetime, Some(position));
        assert_eq!(
            rebuilt_field(&keep.body, returned(&keep.body), position),
            (&field.declaration, &stored_copy(formal, field), field.carrier)
        );
    }

    // Typing refuses a field built from the view at the other region, so
    // lowering never has two regions to reconcile.
    for (machine, written, crossed) in [
        ("pick", "storage: right.storage", "storage: left.storage"),
        ("keep", "head: span.head, tail: span.tail", "head: span.tail, tail: span.head"),
    ] {
        let source = rebuilt.replace(written, crossed);
        assert_ne!(source, rebuilt);
        let package = sources::resolve(&[(FILE, &source)]);
        let error = omega_preparation::prepare_source_package(&package).expect_err("the view is at another region");
        let from = source.find(crossed).unwrap();
        assert!(
            (from..from + crossed.len()).contains(&error.span.get_start_offset()) && error.message.starts_with("expected @[u8] at "),
            "{machine}: {} at {}",
            error.message,
            error.span.get_start_offset()
        );
    }
}

#[test]
fn a_record_behind_a_second_reference_stays_an_open_template() {
    // One reference, shared or exclusive, refers to the record's one row and
    // has a slot: `poke<'r>(h: @mutable Holder<'r>)` names `'r` as
    // `look<'r>(h: @Holder<'r>)` does. A reference to a reference has none.
    for (formal, closed) in [
        ("@Schema<'t>", true),
        ("@mutable Schema<'t>", true),
        ("@@Schema<'t>", false),
    ] {
        let source = format!("{FORWARDED}\nmachine peek<'t>(schema: {formal}) -> u64 {{\n    7\n}}\n");
        let lowered = pipeline::lower(&source);
        let templates: Vec<_> = lowered
            .package
            .machine_templates
            .iter()
            .filter(|template| template.declaration.path == "peek")
            .collect();
        let bodies = lowered
            .package
            .machines
            .iter()
            .filter(|machine| machine.declaration.path == "peek")
            .count();
        assert_eq!((templates.len(), bodies), if closed { (0, 1) } else { (1, 0) }, "{formal}");
        if let [template] = templates.as_slice() {
            assert!(
                matches!(template.parameters.as_slice(), [MachineTemplateBinder::Type(TemplateBinder::Lifetime)]),
                "{formal}"
            );
        }
    }
}

#[test]
fn a_body_that_calls_a_machine_without_regions_is_one_closed_body() {
    let source = format!("{FORWARDED}\nmachine relay<'u>(schema: Schema<'u>) -> u64 {{\n    seven()\n}}\nmachine seven() -> u64 {{\n    7\n}}\n");
    let lowered = pipeline::lower(&source);
    assert_eq!(lowered.machine("relay").regions, [region(&[(0, 0)], &[])]);
    assert!(lowered.package.machine_templates.is_empty());
}

#[test]
fn a_forwarded_toolchain_record_keeps_its_defining_identity_and_machine_region() {
    let source = "use omega\\language\\core\\reflect;\npublic machine forward<'t>(schema: TypeSchema<'t>) -> TypeSchema<'t> {\n    schema\n}\n";
    let lowered = pipeline::lower(source);
    let machine = lowered.machine("forward");
    let schema = data(&lowered, r"language\core\reflect\TypeSchema");
    assert_eq!(schema.declaration.package, omega_toolchain::TOOLCHAIN_PACKAGE);
    assert_eq!(machine.regions, [region(&[(0, 0)], &[0])]);
    assert_eq!(machine.parameter_types.as_slice(), &[schema.carrier]);
    assert_eq!(machine.result_type, Some(schema.carrier));
}

#[test]
fn a_policy_calls_the_executable_constructor_in_its_defining_package() {
    let source = UNRESTRICTED.replace("domain i32::Locked access Rule;", "");
    assert_ne!(source, UNRESTRICTED);
    let lowered = pipeline::lower(&source);
    let plan = lowered.machine("Rule::plan");
    let constructor = lowered.machine(r"language\core\access\new_plan");
    assert_eq!(constructor.declaration.package, omega_toolchain::TOOLCHAIN_PACKAGE);
    assert_eq!(plan.regions, [region(&[(0, 0)], &[0])]);
    let [call] = plan.body.call_regions.as_slice() else { panic!("expected the region of the one constructor call") };
    let (selected, regions) = psi_handoff_consumer::applied_regions(&lowered.package, plan, call.call).unwrap();
    assert_eq!(selected.id, constructor.id);
    assert_eq!(regions.iter().map(|region| region.ordinal).collect::<Vec<_>>(), [0]);
}

#[test]
fn a_nested_record_publishes_the_owner_binder_that_fills_each_inner_binder() {
    let lowered = pipeline::lower(NESTED);
    let package = &lowered.package;
    // Every record is concrete data and both machines have executable bodies.
    assert!(package.data_templates.is_empty() && package.machine_templates.is_empty());
    assert_eq!((package.data.len(), package.machines.len()), (6, 3));
    let (bytes, schema, span, ledger) = (
        data(&lowered, "Bytes"),
        data(&lowered, "Schema"),
        data(&lowered, "Span"),
        data(&lowered, "Ledger"),
    );
    assert_eq!(
        (bytes.lifetime_count, schema.lifetime_count, span.lifetime_count, ledger.lifetime_count),
        (1, 1, 2, 2)
    );

    // A stored view names its binder. A stored record names, for each of its
    // own binders, the binder of the enclosing record that fills it.
    let stored = |record: &LoweredData| -> Vec<(Option<usize>, Vec<usize>)> {
        record
            .fields
            .iter()
            .map(|field| (field.lifetime, field.aggregate_lifetimes.clone()))
            .collect()
    };
    assert_eq!(stored(bytes), [(Some(0), vec![])]);
    assert_eq!(stored(schema), [(None, vec![0])]);
    // `swapped: Span<'y, 'x>`, `twice: Span<'x, 'x>`, `rows: [Schema<'y>; 2]`, `count: u64`.
    assert_eq!(
        stored(ledger),
        [
            (None, vec![1, 0]),
            (None, vec![0, 0]),
            (None, vec![1]),
            (None, vec![])
        ]
    );

    // The field's row is the inner record's one row, whichever binders fill it.
    assert_eq!(schema.fields[0].carrier, bytes.carrier);
    assert_eq!((ledger.fields[0].carrier, ledger.fields[1].carrier), (span.carrier, span.carrier));
    assert_eq!(
        package.types.get(ledger.fields[2].carrier),
        Some(&TypeShape::FixedArray {
            element: schema.carrier,
            length: 2
        })
    );
    // Each listed binder belongs to the enclosing record, one per inner binder.
    for (outer, inner) in [
        (schema, bytes),
        (ledger, span),
        (ledger, span),
        (ledger, schema),
    ]
    .into_iter()
    .zip([
        &schema.fields[0],
        &ledger.fields[0],
        &ledger.fields[1],
        &ledger.fields[2],
    ])
    .map(|((outer, inner), field)| ((outer, field), inner))
    {
        let (outer, field) = outer;
        assert_eq!(field.aggregate_lifetimes.len(), inner.lifetime_count);
        assert!(
            field
                .aggregate_lifetimes
                .iter()
                .all(|&binder| binder < outer.lifetime_count)
        );
    }

    // The machine region reaches the stored view through both relations:
    // `forward`'s `'t` fills `Schema`'s binder 0, which fills `Bytes`'s binder 0.
    let forward = lowered.machine("forward");
    assert_eq!(forward.regions, [region(&[(0, 0)], &[0])]);
    assert_eq!((forward.parameter_types[0], forward.result_type), (schema.carrier, Some(schema.carrier)));
    // `audit<'p, 'q>(ledger: Ledger<'q, 'p>)`: `'p` fills binder 1 and `'q` binder 0.
    let audit = lowered.machine("audit");
    assert_eq!(audit.regions, [region(&[(0, 1)], &[1]), region(&[(0, 0)], &[0])]);
    for machine in [forward, audit] {
        let returns = pipeline::returns(&machine.body);
        let [ReturnEdge::Structural { source, .. }] = returns.as_slice() else {
            panic!("{returns:?}")
        };
        assert_eq!(Parameter::Structural(*source), machine.body.parameters[0]);
    }
}

#[test]
fn a_field_that_names_a_binder_elsewhere_keeps_its_record_a_template() {
    let templates = |source: &str| -> (Vec<String>, Vec<String>) {
        let lowered = pipeline::lower(source);
        let paths = |ids: Vec<&omega_lowering::DeclarationId>| ids.into_iter().map(|id| id.path.clone()).collect();
        (
            paths(
                lowered
                    .package
                    .data_templates
                    .iter()
                    .map(|data| &data.declaration)
                    .collect(),
            ),
            paths(
                lowered
                    .package
                    .data
                    .iter()
                    .filter(|data| data.declaration.package == "app")
                    .map(|data| &data.declaration)
                    .collect(),
            ),
        )
    };
    let bytes = "data Bytes<'t> [copy] {view:@'t [u8];}";
    let names = |paths: &[&str]| paths.iter().map(|path| path.to_string()).collect::<Vec<_>>();
    // The control: the inner record held by value at the owner's binder.
    assert_eq!(
        templates(&format!("{bytes} data Held<'t> {{inner:Bytes<'t>;}}")),
        (names(&[]), names(&["Bytes", "Held"]))
    );
    // Behind one shared reference that names the binder it is held at, the
    // row records that binder and the referent's.
    assert_eq!(
        templates(&format!("{bytes} data Held<'t> {{inner:@'t Bytes<'t>;}}")),
        (names(&[]), names(&["Bytes", "Held"]))
    );
    // With no lifetime on the reference itself nothing says where it is held.
    // Typing refuses that field, so the record never reaches lowering as a template.
    let unwritten = format!("{bytes} data Held<'t> {{inner:@Bytes<'t>;}}");
    let package = sources::resolve(&[(FILE, &unwritten)]);
    let error = omega_preparation::prepare_source_package(&package).expect_err("the stored reference names no lifetime");
    let at = unwritten.find("@Bytes<'t>").unwrap();
    assert_eq!((error.span.get_start_offset(), error.span.get_end_offset()), (at, at + "@Bytes<'t>".len()));
    assert_eq!(error.message, "a stored reference must name its lifetime");
    // An exclusive reference, a second reference and an array of references
    // have no such row either.
    for inner in [
        "@'t mutable Bytes<'t>",
        "@'t @'t Bytes<'t>",
        "[@'t Bytes<'t>; 2]",
    ] {
        assert_eq!(
            templates(&format!("{bytes} data Held<'t> {{inner:{inner};}}")),
            (names(&["Held"]), names(&["Bytes"])),
            "{inner}"
        );
    }
    // A record that holds a template is a template too.
    assert_eq!(
        templates(&format!("{bytes} data Held<'t> {{inner:[@'t Bytes<'t>; 2];}} data Outer<'t> {{held:Held<'t>;}}")),
        (names(&["Held", "Outer"]), names(&["Bytes"]))
    );
    // The toolchain's record of the same shape belongs to another package.
    assert_eq!(
        templates("use omega\\language\\core\\reflect;\ndata Held<'t> {inner:TypeSchema<'t>;}"),
        (names(&["Held"]), names(&[]))
    );
}

#[test]
fn a_direct_reference_machine_keeps_its_closed_body_when_its_body_applies_a_region() {
    // Explicit and inferred applications of the inner machine preserve the
    // same caller binder. A plain read has no call and applies no region.
    let cell = "data Cell {value:u8;}\nmachine inner<'r>(cell:@'r Cell)->u8 {\n    cell.value\n}\n";
    let direct = [MachineRegion {
        reference_parameters: vec![0],
        ..Default::default()
    }];
    for body in ["inner(cell)", "inner::<'a>(cell)", "cell.value"] {
        let source = format!("{cell}machine outer<'a>(cell:@'a Cell)->u8 {{\n    {body}\n}}\n");
        let lowered = pipeline::lower(&source);
        let inner = lowered.machine("inner");
        let outer = lowered.machine("outer");
        assert!(lowered.package.machine_templates.is_empty(), "{body}");
        assert_eq!(inner.regions, direct, "{body}");
        assert_eq!(outer.regions, direct, "{body}");
        if body == "cell.value" {
            assert!(outer.body.call_regions.is_empty());
        } else {
            let [call] = outer.body.call_regions.as_slice() else { panic!("expected one call in {body}") };
            let (callee, applied) = psi_handoff_consumer::applied_regions(&lowered.package, outer, call.call).unwrap();
            assert_eq!(callee.id, inner.id);
            assert_eq!(applied.iter().map(|region| region.ordinal).collect::<Vec<_>>(), [0]);
        }
    }
}

#[test]
fn a_constructor_copies_the_schema_out_of_a_shared_reference_into_a_nested_plan() {
    let lowered = pipeline::lower(NESTED);
    let package = &lowered.package;
    let (schema, slot, plan) = (data(&lowered, "Schema"), data(&lowered, "Slot"), data(&lowered, "Plan"));
    // `Plan<'t>` passes `'t` to its schema and to each slot, and a full slot passes it to its schema.
    assert_eq!((slot.lifetime_count, plan.lifetime_count), (1, 1));
    let passed: Vec<_> = plan
        .fields
        .iter()
        .map(|field| field.aggregate_lifetimes.as_slice())
        .collect();
    assert_eq!(passed, [&[0][..], &[0], &[]]);
    assert_eq!(slot.cases[1].fields[0].aggregate_lifetimes, [0]);
    assert_eq!(slot.cases[1].fields[0].carrier, schema.carrier);
    assert_eq!(
        package.types.get(plan.fields[1].carrier),
        Some(&TypeShape::FixedArray {
            element: slot.carrier,
            length: 2
        })
    );
    assert_eq!(plan.requires.len(), 1);

    // `build<'t>(schema: @Schema<'t>) -> Plan<'t>`: the formal refers to the
    // record's one row, and `'t` fills binder 0 of that record and of the result.
    let build = lowered.machine("build");
    assert_eq!(build.regions, [region(&[(0, 0)], &[0])]);
    assert_eq!(
        package.types.get(build.parameter_types[0]),
        Some(&TypeShape::NominalReference {
            access: ReferenceAccess::Shared,
            referent: schema.id
        })
    );
    assert_eq!(build.result_type, Some(plan.carrier));

    // The body copies the referent, builds the plan around that copy and returns it.
    let body = &build.body;
    let Parameter::Structural(formal) = body.parameters[0] else {
        panic!("{:?}", body.parameters)
    };
    let copies: Vec<_> = body
        .operations
        .iter()
        .filter_map(|operation| match (&operation.kind, operation.result) {
            (OperationKind::CopyStructural { source }, OperationResult::Structural(copy)) => Some((source, copy)),
            _ => None,
        })
        .collect();
    let [(source, copy)] = copies.as_slice() else { panic!("{copies:?}") };
    assert_eq!((source.root, source.path.as_slice()), (formal, &[PlaceProjection::Referent][..]));
    assert_eq!(body.places[copy.index()].ty, schema.carrier);
    let built: Vec<_> = body
        .operations
        .iter()
        .filter_map(|operation| match (&operation.kind, operation.result) {
            (OperationKind::EstablishRecord { fields }, OperationResult::Structural(place)) => Some((fields, place)),
            _ => None,
        })
        .collect();
    let [(fields, built)] = built.as_slice() else { panic!("{built:?}") };
    assert_eq!(
        fields.iter().map(|field| &field.field).collect::<Vec<_>>(),
        plan.fields
            .iter()
            .map(|field| &field.declaration)
            .collect::<Vec<_>>()
    );
    assert_eq!(fields[0].operand, ValueOperand::CopyWhole(*copy));
    let returns = pipeline::returns(body);
    let [ReturnEdge::Structural { source, result }] = returns.as_slice() else {
        panic!("{returns:?}")
    };
    assert_eq!((*source, body.places[result.index()].ty), (*built, plan.carrier));
    // Typing hands the returned plan to the lifetime checker at the construction.
    let loans = lowered.obligations("ReturnBorrow");
    let returned: Vec<_> = loans
        .iter()
        .filter(|loan| matches!(&loan.kind, ObligationKind::ReturnBorrow { machine } if machine.path == "build"))
        .collect();
    let [loan] = returned.as_slice() else { panic!("{returned:?}") };
    assert_eq!(lowered.spelling(&loan.site), "Plan::<'t> { schema: $schema, .. }");
}
