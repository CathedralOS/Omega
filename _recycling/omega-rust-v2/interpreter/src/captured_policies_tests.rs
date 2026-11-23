//! Execute the two captured access-policy programs themselves: the closed
//! `Rule::plan` of each fixture and the toolchain's `access\new_plan` it
//! calls, in the unit the selecting package lowers. The evidence is the
//! checker's own proof of that closed pair, and the argument is the schema
//! image preparation froze for the selected domain.
use crate::{Argument, Ceilings, CheckedBody, MeterStop, Stop, Value, admit};
use omega_lowering::{LoweredPackage, PolicyConstructionInputs};
use omega_resolver::{ParsedSource, StageSource};
use std::sync::Arc;

const UNRESTRICTED: &str = include_str!("../../layers/05_prepared-trees-to-terminal-psi/tests/fixtures/domain_access/unrestricted/access_plan.omg");
const EMPTY_ISSUERS: &str = include_str!("../../layers/05_prepared-trees-to-terminal-psi/tests/fixtures/domain_access/empty-issuers/policy.omg");

/// The selecting package's closed unit and the inputs preparation froze for it.
fn selected(text: &str) -> (omega_lowering::PolicyUnit, omega_lowering::SelectedPolicyDefinition, PolicyConstructionInputs) {
    let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
    let source = ParsedSource {
        logical_path: "policy.omg",
        text,
        tokens: &tokens,
        tree: &tree,
    };
    let mut package = omega_toolchain::resolve_source_package(&[StageSource { package: "app", source }]).unwrap();
    package.bind_package_key("app", [0x42; 32]).unwrap();
    let prepared = omega_preparation::prepare_source_package(&package).unwrap();
    let [selection] = prepared.typed().access_policies.as_slice() else {
        panic!("one selected policy")
    };
    let unit = omega_lowering::lower_selected_policy_unit(&prepared, selection)
        .unwrap()
        .expect("the selected plan has a closed body");
    let definition = omega_lowering::lower_selected_policy_definition(&prepared, selection).unwrap();
    let inputs = omega_lowering::lower_policy_construction_inputs(&prepared, selection).unwrap();
    (unit, definition, inputs)
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

fn fields<'value>(
    value: &'value Value,
    unit: &LoweredPackage,
    path: &str,
) -> &'value [Value] {
    match value {
        Value::Record { data: id, fields } if *id == data(unit, path) => fields,
        other => panic!("expected a {path} record, found {other:?}"),
    }
}

/// Run one fixture and return its plan's issuance field with the usage.
fn run(
    text: &str,
    ceilings: Ceilings,
) -> Result<(Value, crate::Usage, u64), Stop> {
    let (unit, definition, inputs) = selected(text);
    let constructor = omega_checker::check_record_constructor(&unit, &inputs.constructor).unwrap();
    let call = omega_checker::check_selected_policy_call(&definition, &constructor).unwrap();
    let closed = omega_checker::check_closed_policy_call(&call).unwrap();
    let evidence = [
        CheckedBody::new(closed.unit(), closed.plan()),
        CheckedBody::new(closed.unit(), closed.constructor()),
    ];
    let admitted = admit(&unit, &closed.plan().declaration, &evidence).unwrap();
    assert_eq!(
        admitted
            .reached()
            .map(|machine| machine.declaration.path.as_str())
            .collect::<Vec<_>>(),
        ["Rule::plan", r"language\core\access\new_plan"]
    );
    let schema = Value::Record {
        data: data(&unit, r"language\core\reflect\TypeSchema"),
        fields: vec![Value::Bytes(Arc::clone(&inputs.schema.storage))],
    };
    let completed = admitted.execute(vec![Argument::Owned(schema.clone())], ceilings)?;
    let certified = admitted.certified_work();
    assert_eq!(completed.usage.work_used, certified);

    let plan = completed.result.expect("the policy returns its plan");
    let plan = fields(&plan, &unit, r"language\core\AccessPlan");
    let [
        stored,
        issuance,
        slots,
        field_count,
        readers,
        reader_count,
        writers,
        writer_count,
        reach,
    ] = plan
    else {
        panic!("nine plan fields: {plan:?}")
    };
    // The plan views the image preparation froze, not a copy of its bytes.
    assert_eq!(stored, &schema);
    let [Value::Bytes(view)] = fields(stored, &unit, r"language\core\reflect\TypeSchema") else {
        panic!("a schema view")
    };
    assert!(Arc::ptr_eq(view, &inputs.schema.storage));
    // Every omitted field is the zero its declaration states.
    let empty = |path: &str| Value::Case {
        data: data(&unit, path),
        case: 0,
        fields: vec![],
    };
    let zero = Value::Integer(0.into());
    assert_eq!(slots, &Value::Array(vec![empty(r"language\core\AccessFieldSlot"); 32]));
    assert_eq!(readers, &Value::Array(vec![empty(r"language\core\AccessRuleSlot"); 32]));
    assert_eq!(writers, readers);
    assert_eq!([field_count, reader_count, writer_count], [&zero, &zero, &zero]);
    assert_eq!(
        fields(reach, &unit, r"language\core\BoundaryReach"),
        [Value::Array(vec![zero.clone(); 32]), zero]
    );
    Ok((issuance.clone(), completed.usage, certified))
}

#[test]
fn each_captured_policy_executes_and_returns_the_plan_its_issuance_selects() {
    let (unrestricted, open, _) = run(UNRESTRICTED, Ceilings::default()).unwrap();
    let (nobody, closed, _) = run(EMPTY_ISSUERS, Ceilings::default()).unwrap();
    let (unit, ..) = selected(UNRESTRICTED);
    let issuance = data(&unit, r"language\core\Issuance");
    // `Unrestricted` is the second case and carries nothing.
    assert_eq!(
        unrestricted,
        Value::Case {
            data: issuance,
            case: 1,
            fields: vec![]
        }
    );
    // `Only` is the first case; its route list is present and empty, so nobody may issue.
    assert_eq!(
        nobody,
        Value::Case {
            data: issuance,
            case: 0,
            fields: vec![Value::Array(vec![])]
        }
    );
    assert_eq!((open.work_schedule, open.cell_schedule), (1, 1));
    // The constructor copies the schema and moves the issuance (2), forms
    // three arrays of 32 empty slots (99), three zero counts (3), the zeroed
    // reach (1) and the plan (1), and returns (1): 107 units. The policy
    // forms its issuance case, calls and returns (3). Forming the empty route
    // list is the one operation the two cases differ by.
    assert_eq!((open.work_used, closed.work_used), (110, 111));
    // That list holds its stored length in the caller's frame and again in
    // the case that copies it.
    assert_eq!((open.peak_live_cells, closed.peak_live_cells), (142, 144));
    // Each of the 110 or 111 sites ran exactly once.
    for usage in [&open, &closed] {
        assert_eq!(u64::try_from(usage.sites.len()), Ok(usage.work_used));
        assert!(usage.sites.iter().all(|row| row.executions == 1));
    }
}

#[test]
fn the_roots_ceilings_admit_the_exact_usage_and_refuse_one_unit_or_cell_less() {
    for text in [UNRESTRICTED, EMPTY_ISSUERS] {
        let (issuance, usage, certified) = run(text, Ceilings::default()).unwrap();
        let exact = Ceilings {
            work: Some(certified),
            live_cells: Some(usage.peak_live_cells),
        };
        // A ceiling changes whether the plan is obtained, never which plan.
        let (same, metered, _) = run(text, exact).unwrap();
        assert_eq!((same, &metered), (issuance, &usage));
        let Err(Stop::Resource { stop, at }) = run(
            text,
            Ceilings {
                work: Some(certified - 1),
                ..exact
            },
        ) else {
            panic!("the last edge cannot be paid")
        };
        assert_eq!(
            stop,
            MeterStop::WorkExhausted {
                used: certified - 1,
                remaining: 0,
                unpaid: 1
            }
        );
        assert_eq!(at.chain.iter().map(|id| id.path.as_str()).collect::<Vec<_>>(), ["Rule::plan"]);
        let Err(Stop::Resource { stop, .. }) = run(
            text,
            Ceilings {
                live_cells: Some(usage.peak_live_cells - 1),
                ..exact
            },
        ) else {
            panic!("the peak does not fit")
        };
        assert!(
            matches!(stop, MeterStop::LiveCells { ceiling, .. } if ceiling == usage.peak_live_cells - 1),
            "{stop:?}"
        );
    }
}

#[test]
fn evidence_for_the_defining_package_does_not_admit_the_selecting_packages_bodies() {
    let (unit, _, inputs) = selected(UNRESTRICTED);
    // The toolchain's own unit proves the same constructor declaration.
    let library = omega_toolchain::SourcePackage::toolchain_definition();
    let prepared = omega_preparation::prepare_source_package(&library).unwrap();
    let defining = omega_lowering::lower_package(&prepared).unwrap();
    let foreign = omega_checker::check_record_constructor(&defining, &inputs.constructor).unwrap();
    let own = omega_checker::check_record_constructor(&unit, &inputs.constructor).unwrap();
    assert_eq!(foreign.machine().declaration, own.machine().declaration);
    let plan = unit
        .machines
        .iter()
        .find(|machine| machine.declaration.path == "Rule::plan")
        .unwrap();
    let refusal = admit(
        &unit,
        &plan.declaration,
        &[
            CheckedBody::new(&unit, plan),
            CheckedBody::new(foreign.unit(), foreign.machine()),
        ],
    )
    .unwrap_err();
    assert_eq!(refusal.to_string(), r"`language\core\access\new_plan` has no checked body evidence");
}
