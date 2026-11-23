//! Execute the closed bodies an ordinary package lowers to. `build` has the
//! shape of the toolchain's `access\new_plan`, and `plan` the shape of a
//! selected policy's `Rule::plan`: it borrows its schema and calls the
//! constructor. [Fixture](../../layers/05_prepared-trees-to-terminal-psi/tests/fixtures/aggregate_regions/nested_record.omg)
use crate::{Argument, Ceilings, ChargeSite, CheckedBody, MeterStop, Reason, Stop, Value, admit};
use omega_lowering::{DeclarationId, LoweredMachine, LoweredPackage};
use omega_resolver::{ParsedSource, StageSource};
use std::sync::Arc;

const NESTED: &str = include_str!("../../layers/05_prepared-trees-to-terminal-psi/tests/fixtures/aggregate_regions/nested_record.omg");
/// A selected policy's plan, written over the fixture's own records.
const PLAN: &str = "public machine plan<'t>(schema: Schema<'t>) -> Plan<'t> {\n    build::<'t>(@schema)\n}\n";
/// The toolchain's `Issuance` and a constructor that moves it into its plan,
/// with one caller for each captured policy case.
const ISSUED: &str = "public data Issuance<'t> {
    case Only(routes: [Schema<'t>]);
    case Unrestricted;
}
public data Issued<'t>
    requires self.count <= 2
{
    schema: Schema<'t>;
    issuance: Issuance<'t>;
    slots: [Slot<'t>; 2];
    count: u64;
}
public machine issue<'t>(schema: @Schema<'t>, issuance: Issuance<'t>) -> Issued<'t> {
    Issued::<'t> { schema: $schema, issuance: issuance, .. }
}
public machine unrestricted<'t>(schema: Schema<'t>) -> Issued<'t> {
    issue::<'t>(@schema, Issuance::Unrestricted{})
}
public machine nobody<'t>(schema: Schema<'t>) -> Issued<'t> {
    issue::<'t>(@schema, Issuance::Only{routes: []})
}
";

fn lower(text: &str) -> LoweredPackage {
    let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
    let source = ParsedSource {
        logical_path: "plan.omg",
        text,
        tokens: &tokens,
        tree: &tree,
    };
    let package = omega_toolchain::resolve_source_package(&[StageSource { package: "app", source }]).unwrap();
    let prepared = omega_preparation::prepare_source_package(&package).unwrap();
    omega_lowering::lower_package(&prepared).unwrap()
}

fn machine<'unit>(
    unit: &'unit LoweredPackage,
    path: &str,
) -> &'unit LoweredMachine {
    let found: Vec<_> = unit
        .machines
        .iter()
        .filter(|machine| machine.declaration.path == path)
        .collect();
    let [machine] = found.as_slice() else { panic!("one closed body of {path}") };
    machine
}

/// Stands in for the checker's proof of each named body of this unit.
fn checked<'unit>(
    unit: &'unit LoweredPackage,
    paths: &[&str],
) -> Vec<CheckedBody<'unit>> {
    paths
        .iter()
        .map(|path| CheckedBody::new(unit, machine(unit, path)))
        .collect()
}

fn data(
    unit: &LoweredPackage,
    path: &str,
) -> omega_lowering::DataId {
    unit.data
        .iter()
        .find(|data| data.declaration.path == path)
        .unwrap_or_else(|| panic!("no data {path}"))
        .id
}

/// `Schema { storage: Bytes { view } }` over the given frozen backing.
fn schema(
    unit: &LoweredPackage,
    backing: &Arc<[u8]>,
) -> Value {
    Value::Record {
        data: data(unit, "Schema"),
        fields: vec![Value::Record {
            data: data(unit, "Bytes"),
            fields: vec![Value::Bytes(Arc::clone(backing))],
        }],
    }
}

fn entry(
    unit: &LoweredPackage,
    path: &str,
) -> DeclarationId {
    machine(unit, path).declaration.clone()
}

/// The view a result's schema field holds.
fn stored_view(value: &Value) -> &Arc<[u8]> {
    let Value::Record { fields, .. } = value else { panic!("a record: {value:?}") };
    let Value::Record { fields: schema, .. } = &fields[0] else {
        panic!("a schema: {fields:?}")
    };
    let Value::Record { fields: bytes, .. } = &schema[0] else {
        panic!("its bytes: {schema:?}")
    };
    let Value::Bytes(view) = &bytes[0] else { panic!("a view: {bytes:?}") };
    view
}

#[test]
fn the_constructor_runs_on_the_sponsors_schema_and_returns_the_plan_it_establishes() {
    let unit = lower(NESTED);
    let admitted = admit(&unit, &entry(&unit, "build"), &checked(&unit, &["build"])).unwrap();
    // Copy, two empty slots, their array, the zero count, the record, and the return edge.
    assert_eq!(admitted.certified_work(), 7);
    let backing: Arc<[u8]> = Arc::from(&b"schema image"[..]);
    let argument = Arc::new(schema(&unit, &backing));
    let completed = admitted
        .execute(vec![Argument::Shared(Arc::clone(&argument))], Ceilings::default())
        .unwrap();
    let plan = completed.result.as_ref().unwrap();
    let empty = Value::Case {
        data: data(&unit, "Slot"),
        case: 0,
        fields: vec![],
    };
    assert_eq!(
        plan,
        &Value::Record {
            data: data(&unit, "Plan"),
            fields: vec![
                schema(&unit, &backing),
                Value::Array(vec![empty.clone(), empty]),
                Value::Integer(0.into())
            ],
        }
    );
    // The plan views the sponsor's own backing, not a copy of its bytes.
    assert!(Arc::ptr_eq(stored_view(plan), &backing));
    let usage = &completed.usage;
    assert_eq!((usage.work_schedule, usage.cell_schedule, usage.work_used), (1, 1, admitted.certified_work()));
    // The sponsor's schema (1), the frame and its reference (2), the copy (1),
    // two case tags (2), the count (1), then the plan's copied schema and count (2).
    assert_eq!(usage.peak_live_cells, 9);
    let build = machine(&unit, "build");
    let sites: Vec<_> = usage
        .sites
        .iter()
        .map(|row| (row.machine, row.site, row.executions))
        .collect();
    let mut expected: Vec<_> = build
        .body
        .operations
        .iter()
        .map(|operation| (build.id, ChargeSite::Operation(operation.id), 1))
        .collect();
    let omega_lowering::BlockTerminator::Return { id, .. } = &build.body.blocks[0].terminator else {
        panic!("a return")
    };
    expected.push((build.id, ChargeSite::Edge(*id), 1));
    assert_eq!(sites, expected);
}

#[test]
fn the_caller_borrows_its_schema_and_the_usage_covers_both_bodies() {
    let source = format!("{NESTED}\n{PLAN}");
    let unit = lower(&source);
    let admitted = admit(&unit, &entry(&unit, "plan"), &checked(&unit, &["plan", "build"])).unwrap();
    assert_eq!(
        admitted
            .reached()
            .map(|machine| machine.declaration.path.as_str())
            .collect::<Vec<_>>(),
        ["plan", "build"]
    );
    // The call and the caller's return, then the constructor's seven.
    assert_eq!(admitted.certified_work(), 9);
    let backing: Arc<[u8]> = Arc::from(&b"schema image"[..]);
    let completed = admitted
        .execute(vec![Argument::Owned(schema(&unit, &backing))], Ceilings::default())
        .unwrap();
    let plan = completed.result.as_ref().unwrap();
    assert!(Arc::ptr_eq(stored_view(plan), &backing));
    assert_eq!((completed.usage.work_used, completed.usage.peak_live_cells), (9, 10));
    // The same value the constructor returns when the sponsor lends the schema itself.
    let direct = admit(&unit, &entry(&unit, "build"), &checked(&unit, &["build"])).unwrap();
    let lent = direct
        .execute(vec![Argument::Shared(Arc::new(schema(&unit, &backing)))], Ceilings::default())
        .unwrap();
    assert_eq!(completed.result, lent.result);
    assert_eq!(completed.usage.sites.len(), 9);
    assert!(completed.usage.sites.iter().all(|row| row.executions == 1));
}

#[test]
fn each_policy_case_moves_its_own_issuance_into_the_plan() {
    let source = format!("{NESTED}\n{ISSUED}");
    let unit = lower(&source);
    let backing: Arc<[u8]> = Arc::from(&b"schema image"[..]);
    let issuance = data(&unit, "Issuance");
    let run = |caller: &str| {
        let admitted = admit(&unit, &entry(&unit, caller), &checked(&unit, &[caller, "issue"])).unwrap();
        let completed = admitted
            .execute(vec![Argument::Owned(schema(&unit, &backing))], Ceilings::default())
            .unwrap();
        assert_eq!(completed.usage.work_used, admitted.certified_work());
        let Some(Value::Record { fields, .. }) = completed.result else {
            panic!("an issued plan")
        };
        (fields[1].clone(), completed.usage)
    };
    let (unrestricted, open) = run("unrestricted");
    let (nobody, closed) = run("nobody");
    // `Only` is the first case and carries an empty route list. `Unrestricted` carries nothing.
    assert_eq!(
        unrestricted,
        Value::Case {
            data: issuance,
            case: 1,
            fields: vec![]
        }
    );
    assert_eq!(
        nobody,
        Value::Case {
            data: issuance,
            case: 0,
            fields: vec![Value::Array(vec![])]
        }
    );
    // The empty list is one more operation. Its stored length is one cell in
    // the caller's frame and one more in the case that copies the list.
    assert_eq!((open.work_used, closed.work_used), (11, 12));
    assert_eq!((open.peak_live_cells, closed.peak_live_cells), (11, 13));
}

#[test]
fn a_ceiling_decides_whether_a_result_is_obtained_and_never_which() {
    let source = format!("{NESTED}\n{PLAN}");
    let unit = lower(&source);
    let admitted = admit(&unit, &entry(&unit, "plan"), &checked(&unit, &["plan", "build"])).unwrap();
    let backing: Arc<[u8]> = Arc::from(&b"schema image"[..]);
    let run = |ceilings| admitted.execute(vec![Argument::Owned(schema(&unit, &backing))], ceilings);
    let free = run(Ceilings::default()).unwrap();
    let exact = Ceilings {
        work: Some(9),
        live_cells: Some(10),
    };
    assert_eq!(run(exact).unwrap(), free);

    // One unit short: the constructor has returned, and the caller's own
    // return edge is the charge that cannot be paid.
    let Err(Stop::Resource { stop, at }) = run(Ceilings { work: Some(8), ..exact }) else {
        panic!("exhaustion")
    };
    assert_eq!(
        stop,
        MeterStop::WorkExhausted {
            used: 8,
            remaining: 0,
            unpaid: 1
        }
    );
    assert_eq!(at.chain.iter().map(|id| id.path.as_str()).collect::<Vec<_>>(), ["plan"]);
    assert!(matches!(at.site, Some(ChargeSite::Edge(_))));
    // Two units short stops inside the constructor, at its return edge.
    let Err(Stop::Resource { at, .. }) = run(Ceilings { work: Some(7), ..exact }) else {
        panic!("exhaustion")
    };
    assert_eq!(at.chain.iter().map(|id| id.path.as_str()).collect::<Vec<_>>(), ["plan", "build"]);
    assert!(matches!(at.site, Some(ChargeSite::Edge(_))));
    // A zero budget is a budget: the first operation is refused.
    let Err(Stop::Resource { stop, at }) = run(Ceilings { work: Some(0), ..exact }) else {
        panic!("exhaustion")
    };
    assert_eq!(
        stop,
        MeterStop::WorkExhausted {
            used: 0,
            remaining: 0,
            unpaid: 1
        }
    );
    assert_eq!(at.chain.len(), 1);

    // One cell short: the plan record needs two more cells than the ceiling leaves.
    let Err(Stop::Resource { stop, at }) = run(Ceilings { live_cells: Some(9), ..exact }) else {
        panic!("ceiling")
    };
    assert_eq!(
        stop,
        MeterStop::LiveCells {
            live: 8,
            requested: 2,
            ceiling: 9
        }
    );
    let build = machine(&unit, "build");
    assert_eq!(at.site, Some(ChargeSite::Operation(build.body.operations[5].id)));
    assert_eq!(at.source.as_ref(), build.body.diagnostics.operations.get(5));
    let message = run(Ceilings { work: Some(8), ..exact })
        .unwrap_err()
        .to_string();
    assert_eq!(
        message,
        "work budget exhausted: 8 used, 0 remaining; the next charge requires 1 (schedule 1) in `plan` at edge 1"
    );
}

#[test]
fn admission_needs_checker_evidence_for_every_reached_body_of_this_unit() {
    let source = format!("{NESTED}\n{PLAN}");
    let unit = lower(&source);
    let plan = entry(&unit, "plan");
    let refused = |evidence: &[CheckedBody<'_>]| {
        let refusal = admit(&unit, &plan, evidence).unwrap_err();
        (refusal.machine.path.clone(), refusal.reason)
    };
    assert_eq!(refused(&[]), ("plan".to_owned(), Reason::Unchecked));
    assert_eq!(refused(&checked(&unit, &["plan"])), ("build".to_owned(), Reason::Unchecked));
    assert_eq!(refused(&checked(&unit, &["build"])), ("plan".to_owned(), Reason::Unchecked));
    // An equal body lowered again is another unit. Its evidence does not apply here.
    let twin = lower(&source);
    assert_eq!(twin, unit);
    assert_eq!(refused(&checked(&twin, &["plan", "build"])), ("plan".to_owned(), Reason::Unchecked));
    let message = admit(&unit, &plan, &[]).unwrap_err().to_string();
    assert_eq!(message, "`plan` has no checked body evidence");
    // A declaration with no closed body in the unit is not an entry.
    let absent = unit.data[0].declaration.clone();
    assert_eq!(admit(&unit, &absent, &[]).unwrap_err().reason, Reason::UnknownMachine);
}

#[test]
fn a_false_data_predicate_stops_the_invocation_without_a_value() {
    // The same constructor over a plan whose zero count is not allowed. The
    // evidence claims the body was checked; the executor's own check of the
    // established value disagrees and publishes nothing.
    let source = NESTED.replace("requires self.count <= 2", "requires self.count >= 1");
    assert_ne!(source, NESTED);
    let unit = lower(&source);
    let admitted = admit(&unit, &entry(&unit, "build"), &checked(&unit, &["build"])).unwrap();
    let backing: Arc<[u8]> = Arc::from(&b"schema image"[..]);
    let stop = admitted
        .execute(vec![Argument::Shared(Arc::new(schema(&unit, &backing)))], Ceilings::default())
        .unwrap_err();
    let Stop::Inconsistent { what, at } = &stop else { panic!("{stop:?}") };
    assert_eq!(*what, "a data predicate is false for the established value");
    let build = machine(&unit, "build");
    assert_eq!(at.site, Some(ChargeSite::Operation(build.body.operations[5].id)));
}

#[test]
fn an_argument_must_be_a_completed_value_of_its_formal() {
    let unit = lower(NESTED);
    let admitted = admit(&unit, &entry(&unit, "build"), &checked(&unit, &["build"])).unwrap();
    let backing: Arc<[u8]> = Arc::from(&b"schema image"[..]);
    let run = |arguments| admitted.execute(arguments, Ceilings::default()).unwrap_err();
    assert_eq!(run(vec![]), Stop::Arity { expected: 1, supplied: 0 });
    // The formal is a shared reference, so the schema cannot be given away.
    assert_eq!(run(vec![Argument::Owned(schema(&unit, &backing))]), Stop::Argument { parameter: 0 });
    // A record of another declaration, and a schema missing its bytes.
    let other = Value::Record {
        data: data(&unit, "Bytes"),
        fields: vec![Value::Bytes(Arc::clone(&backing))],
    };
    assert_eq!(run(vec![Argument::Shared(Arc::new(other))]), Stop::Argument { parameter: 0 });
    let hollow = Value::Record {
        data: data(&unit, "Schema"),
        fields: vec![Value::Integer(0.into())],
    };
    assert_eq!(run(vec![Argument::Shared(Arc::new(hollow))]), Stop::Argument { parameter: 0 });
}

#[test]
fn bodies_outside_the_straight_line_fragment_are_refused_before_anything_runs() {
    let refusal = |extra: &str, name: &str, evidence: &[&str]| {
        let source = format!("{NESTED}\n{extra}");
        let unit = lower(&source);
        let refusal = admit(&unit, &entry(&unit, name), &checked(&unit, evidence)).unwrap_err();
        (refusal.machine.path.clone(), refusal.reason, refusal.to_string())
    };
    // A branch selects between two results.
    let (machine, reason, _) = refusal(
        "machine pick(flag: bool) -> u64 {\n    match flag { true -> 1u64, false -> 2u64, }\n}\n",
        "pick",
        &["pick"],
    );
    assert_eq!((machine.as_str(), reason), ("pick", Reason::NotStraightLine));
    // Arithmetic is not an admitted operation, whatever its policy.
    let (machine, reason, message) = refusal("machine next(n: u64) -> u64 {\n    n +% 1u64\n}\n", "next", &["next"]);
    assert_eq!(machine, "next");
    assert!(matches!(reason, Reason::Unsupported(_)), "{reason:?}");
    assert_eq!(message, "`next` uses an integer operation, which this executor does not run");
    // A precondition needs evidence at the invocation, and the executor accepts none.
    let (machine, reason, _) = refusal("machine guarded(n: u64) -> u64\n    requires n > 0u64\n{\n    n\n}\n", "guarded", &["guarded"]);
    assert_eq!((machine.as_str(), reason), ("guarded", Reason::Precondition));
    // The refusal names the callee that stops its caller.
    let (machine, reason, _) = refusal(
        "machine next(n: u64) -> u64 {\n    n +% 1u64\n}\nmachine relay(n: u64) -> u64 {\n    next(n)\n}\n",
        "relay",
        &["relay", "next"],
    );
    assert!(matches!((machine.as_str(), reason), ("next", Reason::Unsupported(_))), "{machine}: {reason:?}");
}

#[test]
fn an_entry_cannot_return_a_reference_into_the_invocation() {
    // Lowering publishes no closed body that returns a reference to a record
    // yet, so retype the fixture's `forward`, which returns its formal, to take
    // and return the shared schema reference that `build` takes.
    let mut unit = lower(NESTED);
    let reference = machine(&unit, "build").parameter_types[0];
    let position = unit
        .machines
        .iter()
        .position(|machine| machine.declaration.path == "forward")
        .unwrap();
    let forward = &mut unit.machines[position];
    forward.parameter_types[0] = reference;
    forward.result_type = Some(reference);
    for place in &mut forward.body.places {
        place.ty = reference;
    }
    // A body the executor runs, but as an entry it would hand the sponsor a
    // reference instead of a snapshot of values.
    let refusal = admit(&unit, &entry(&unit, "forward"), &checked(&unit, &["forward"])).unwrap_err();
    assert_eq!((refusal.machine.path.as_str(), refusal.reason), ("forward", Reason::EscapingReference));
    assert_eq!(refusal.to_string(), "`forward` returns a value that can hold a reference into the invocation");
    // The unchanged body returns its schema by value and is admitted.
    let unit = lower(NESTED);
    assert!(admit(&unit, &entry(&unit, "forward"), &checked(&unit, &["forward"])).is_ok());
}
