//! `reflect\callable(pick)` in a closed body. The description is one
//! `DescribeMachine` operation that names the selected machine and its static
//! operands, and an array literal around it takes that operation's place as
//! an element.
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    CallTarget, DeclarationKind, MachineBody, OperationKind, OperationResult, PlaceOrigin, ScalarType, StaticArgument, TemplateCallTarget, TemplateSelection,
    TemplateStep, TypeExpression, TypeExpressionStep, TypeShape, ValueOperand,
};
use psi_handoff_consumer::BodyValue;

/// The plan-evaluation fixture whose plan issues through one described route.
const NONEMPTY: &str = include_str!("fixtures/policy_templates/nonempty.omg");
const FILE: &str = "unused.omg";

/// Without a domain selecting `Rule`, `plan<'t>` is one closed body. Its
/// `Issuance::Only{routes:[reflect\callable(pick)]}` describes `pick` at the
/// query's own site, as the exact machine typing selected with no arguments.
/// The description's place has the toolchain's `CallableInfo` row and is the
/// one element of the route sequence the case copies into `routes`.
///
/// A description is a value. `pick` stays a closed body of the package and no
/// body calls it, and a reader of Psi alone finds the same selection an open
/// template publishes in its `Describe` step.
#[test]
fn a_closed_body_describes_the_selected_machine_as_one_sequence_element() {
    let source = NONEMPTY.replace(" access Rule;", ";");
    assert_ne!(source, NONEMPTY, "the fixture selects Rule");
    let package = sources::lower(&[(FILE, &source)]).unwrap_or_else(|error| panic!("{error:?}"));
    assert!(package.machine_templates.is_empty());
    let machine = |path: &str| {
        let found: Vec<_> = package
            .machines
            .iter()
            .filter(|m| m.declaration.package == "app" && m.declaration.path == path)
            .collect();
        let [machine] = found.as_slice() else {
            panic!("expected one closed body of {path}, found {}", found.len())
        };
        *machine
    };
    let (plan, pick) = (machine("Rule::plan"), machine("pick"));
    let body = &plan.body;

    let described: Vec<_> = body
        .operations
        .iter()
        .filter(|op| matches!(op.kind, OperationKind::DescribeMachine(_)))
        .collect();
    let [describe] = described.as_slice() else {
        panic!("expected one description, found {described:?}")
    };
    let OperationKind::DescribeMachine(selection) = &describe.kind else {
        unreachable!()
    };
    let site = &body.diagnostics.operations[describe.id.index()];
    assert_eq!((site.package.as_str(), site.file.as_str()), ("app", FILE));
    assert_eq!(site.span.slice(source.as_bytes()), Some("reflect\\callable(pick)".as_bytes()));
    assert_eq!(selection.declaration, pick.declaration);
    assert_eq!(
        (
            selection.declaration.package.as_str(),
            selection.declaration.path.as_str(),
            selection.declaration.kind
        ),
        ("app", "pick", DeclarationKind::Machine)
    );
    assert!(selection.arguments.is_empty() && selection.enclosing_arguments.is_empty());

    let callable_info = {
        use omega_parser::NodeKind;
        omega_toolchain::core_interface()
            .id("reflect\\CallableInfo")
            .try_map_kind(&|kind| (*kind == NodeKind::Data).then_some(DeclarationKind::Data))
            .unwrap()
    };
    let element = package
        .data
        .iter()
        .find(|data| data.declaration == callable_info)
        .expect("the package carries the toolchain's CallableInfo")
        .carrier;
    let OperationResult::Structural(description) = describe.result else {
        panic!("a description is a structural value, found {:?}", describe.result)
    };
    let place = &body.places[description.index()];
    assert_eq!((place.ty, place.origin), (element, PlaceOrigin::Operation(describe.id)));

    let sequences: Vec<_> = body
        .operations
        .iter()
        .filter_map(|op| match (&op.kind, op.result) {
            (OperationKind::EstablishStructuralArray { elements }, OperationResult::Structural(sequence)) => Some((elements, sequence)),
            _ => None,
        })
        .collect();
    let [(elements, sequence)] = sequences.as_slice() else {
        panic!("expected one route sequence, found {sequences:?}")
    };
    assert_eq!(elements.as_slice(), [description]);
    assert_eq!(
        package.types.get(body.places[sequence.index()].ty),
        Some(&TypeShape::ElementSequence { element })
    );
    let cases: Vec<_> = body
        .operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::EstablishCase { case, fields } => Some((case.path.as_str(), fields)),
            _ => None,
        })
        .collect();
    let [("language\\core\\Issuance::Only", fields)] = cases.as_slice() else {
        panic!("expected the one Issuance::Only establishment, found {cases:?}")
    };
    let [routes] = fields.as_slice() else {
        panic!("expected the one payload field, found {fields:?}")
    };
    assert_eq!(routes.operand, ValueOperand::CopyWhole(*sequence));

    for caller in &package.machines {
        assert!(
            caller
                .body
                .operations
                .iter()
                .all(|op| !matches!(&op.kind, OperationKind::Call { callee: CallTarget::Machine(id), .. } if *id == pick.id)),
            "{} calls the described machine",
            caller.declaration.path
        );
    }

    let rule = psi_handoff_consumer::member_machine(&package, "Rule", "language\\core\\AccessPolicy::plan").unwrap();
    let BodyValue::Provided { mut operands, .. } = psi_handoff_consumer::body_result(&package, rule, &[BodyValue::Argument(0)]).unwrap() else {
        panic!("Rule::plan does not return the toolchain constructor")
    };
    let Some((_, BodyValue::Construct { fields, .. })) = operands.pop() else {
        panic!("the constructor takes no issuance")
    };
    assert_eq!(
        fields,
        [(
            "language\\core\\Issuance::Only::routes",
            BodyValue::Array(vec![BodyValue::Descriptor(selection)])
        )]
    );
}

const U64: ScalarType = ScalarType::Integer { signed: false, width: 64 };

/// What Psi publishes for the operand `value` of a `const N: u64` binder.
fn operand(value: u64) -> TypeExpression {
    TypeExpression(vec![TypeExpressionStep::StaticInteger {
        carrier: U64,
        value: value.into(),
    }])
}

/// The descriptions of `body` in operation order, each with its result.
fn descriptions(body: &MachineBody) -> Vec<(&TemplateSelection, OperationResult)> {
    body.operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::DescribeMachine(selection) => Some((&**selection, op.result)),
            _ => None,
        })
        .collect()
}

/// The machines each direct call of `body` names, in operation order.
fn callees(body: &MachineBody) -> Vec<omega_lowering::MachineId> {
    body.operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::Call {
                callee: CallTarget::Machine(id),
                ..
            } => Some(*id),
            OperationKind::Call { callee, .. } => panic!("unexpected call target {callee:?}"),
            _ => None,
        })
        .collect()
}

/// `pick<const N: u64>` described at two applications in one route list:
/// `[reflect\callable(pick::<3>), reflect\callable(pick::<4>)]`. The closed
/// plan publishes two descriptions of the same declaration that differ only
/// in the static operand, each at its own site, and the route sequence holds
/// their places in source order.
///
/// Describing an application neither calls nor plans it. `pick` stays an
/// open template that nothing invokes, the package carries no body of
/// `pick`, and the plan's only call is the toolchain constructor.
///
/// The twin whose plan takes a static integer is an open template. Its two
/// `Describe` steps carry exactly the selections of the closed body.
#[test]
fn a_closed_body_describes_each_application_at_its_own_static_operand() {
    let source = NONEMPTY
        .replace(" access Rule;", ";")
        .replace("machine pick() -> bool { true }", "machine pick<const N: u64>() -> bool { true }")
        .replace("[reflect\\callable(pick)]", "[reflect\\callable(pick::<3>), reflect\\callable(pick::<4>)]");
    assert!(source.contains("pick::<4>") && source.contains("pick<const N: u64>") && !source.contains("access Rule"));
    let package = sources::lower(&[(FILE, &source)]).unwrap_or_else(|error| panic!("{error:?}"));

    let bodies: Vec<_> = package
        .machines
        .iter()
        .map(|m| (m.declaration.package.as_str(), m.declaration.path.as_str()))
        .collect();
    assert_eq!(
        bodies,
        [
            ("app", "Rule::plan"),
            (omega_toolchain::TOOLCHAIN_PACKAGE, "language\\core\\access\\new_plan")
        ]
    );
    let [pick] = package.machine_templates.as_slice() else {
        panic!("expected pick as the one open template, found {}", package.machine_templates.len())
    };
    assert_eq!((pick.declaration.package.as_str(), pick.declaration.path.as_str()), ("app", "pick"));
    assert!(pick.invokes.is_empty() && pick.invokes_machines.is_empty());

    let (plan, new_plan) = (&package.machines[0], &package.machines[1]);
    let body = &plan.body;
    let described = descriptions(body);
    let [
        (three, OperationResult::Structural(first)),
        (four, OperationResult::Structural(second)),
    ] = described.as_slice()
    else {
        panic!("expected two descriptions, found {described:?}")
    };
    for selection in [three, four] {
        assert_eq!(selection.declaration, pick.declaration);
        assert!(selection.enclosing_arguments.is_empty());
    }
    assert_eq!((&three.arguments, &four.arguments), (&vec![operand(3)], &vec![operand(4)]));
    let sites: Vec<_> = body
        .operations
        .iter()
        .filter(|op| matches!(op.kind, OperationKind::DescribeMachine(_)))
        .map(|op| {
            body.diagnostics.operations[op.id.index()]
                .span
                .slice(source.as_bytes())
        })
        .collect();
    assert_eq!(
        sites,
        [
            Some("reflect\\callable(pick::<3>)".as_bytes()),
            Some("reflect\\callable(pick::<4>)".as_bytes())
        ]
    );
    let sequences: Vec<_> = body
        .operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::EstablishStructuralArray { elements } => Some(elements.as_slice()),
            _ => None,
        })
        .collect();
    assert_eq!(sequences, [[*first, *second]]);
    assert_eq!(callees(body), [new_plan.id]);

    let rule = psi_handoff_consumer::member_machine(&package, "Rule", "language\\core\\AccessPolicy::plan").unwrap();
    let BodyValue::Provided { mut operands, .. } = psi_handoff_consumer::body_result(&package, rule, &[BodyValue::Argument(0)]).unwrap() else {
        panic!("Rule::plan does not return the toolchain constructor")
    };
    let Some((_, BodyValue::Construct { fields, .. })) = operands.pop() else {
        panic!("the constructor takes no issuance")
    };
    assert_eq!(
        fields,
        [(
            "language\\core\\Issuance::Only::routes",
            BodyValue::Array(vec![BodyValue::Descriptor(three), BodyValue::Descriptor(four)])
        )]
    );

    let planner = "trait Planner { machine plan<'t, const N: u64>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates; }\n";
    let open = source
        .replace("Rule satisfies AccessPolicy {", &format!("{planner}Rule satisfies Planner {{"))
        .replace("machine plan<'t>(", "machine plan<'t, const N: u64>(");
    assert!(open.contains("Rule satisfies Planner {") && open.contains("machine plan<'t, const N: u64>("));
    let opened = sources::lower(&[(FILE, &open)]).unwrap_or_else(|error| panic!("{error:?}"));
    assert!(opened.machines.is_empty());
    let mut steps = Vec::new();
    for template in &opened.machine_templates {
        assert!(template.invokes.is_empty() && template.invokes_machines.is_empty());
        for step in &template.body {
            match step {
                TemplateStep::Describe(describe) => steps.push(&describe.selection),
                TemplateStep::Call(call) => assert!(
                    !matches!(&call.target, TemplateCallTarget::Declaration(id) if id.path == "pick"),
                    "a description is not a call"
                ),
                _ => {}
            }
        }
    }
    assert_eq!(steps, [*three, *four]);
}

/// A description inside a machine with a static binder names that binder:
/// `outer<const N: u64>` holds `reflect\callable(pick::<N>)`. Each closed
/// instance of `outer` describes `pick` at the integer that instance was
/// applied to, so `outer::<3>` describes `pick::<3>` and `outer::<4>`
/// describes `pick::<4>`. No instance publishes the formal `N`, calls `pick`
/// or causes a body of `pick` to be planned.
#[test]
fn an_instance_describes_the_application_its_own_static_argument_fixes() {
    let source = "use omega\\language\\core;\nuse omega\\language\\core\\reflect;\nmachine pick<const N: u64>() -> bool { true }\nmachine outer<const N: u64>() -> bool { let described = reflect\\callable(pick::<N>); true }\nmachine root() -> bool { let first = outer::<3>(); outer::<4>() }\n";
    let package = sources::lower(&[(FILE, source)]).unwrap_or_else(|error| panic!("{error:?}"));
    assert!(package.machines.iter().all(|m| m.declaration.path != "pick"));
    let mut applied = Vec::new();
    for outer in package
        .machines
        .iter()
        .filter(|m| m.declaration.path == "outer")
    {
        let [StaticArgument::Integer { carrier, value }] = outer.arguments.as_slice() else {
            panic!("expected one static integer argument, found {:?}", outer.arguments)
        };
        assert_eq!(*carrier, U64);
        let described = descriptions(&outer.body);
        let [(selection, OperationResult::Structural(_))] = described.as_slice() else {
            panic!("expected one description, found {described:?}")
        };
        assert_eq!(
            (
                selection.declaration.package.as_str(),
                selection.declaration.path.as_str(),
                selection.declaration.kind
            ),
            ("app", "pick", DeclarationKind::Machine)
        );
        assert_eq!(
            selection.arguments,
            [TypeExpression(vec![TypeExpressionStep::StaticInteger {
                carrier: U64,
                value: value.clone()
            }])]
        );
        assert!(selection.enclosing_arguments.is_empty());
        assert!(callees(&outer.body).is_empty());
        applied.push(value.clone());
    }
    applied.sort();
    assert_eq!(applied, [3.into(), 4.into()]);
    for template in &package.machine_templates {
        assert!(template.invokes.is_empty() && template.invokes_machines.is_empty());
    }
}
