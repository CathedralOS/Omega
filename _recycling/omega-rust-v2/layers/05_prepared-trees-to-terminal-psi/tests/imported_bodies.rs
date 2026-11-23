//! A closed body that calls an imported machine with a body gets that body
//! in its own package, under the package that declares it. The body is the
//! one typing selected for the call and is lowered from its library's facts.
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    AggregateRegionSlot, CallArgument, CallSite, CallTarget, LoweredMachine, LoweredPackage, MachineRegion, OperationKind, PlaceProjection, PsiOperation,
    ReferenceAccess,
};
use omega_toolchain::TOOLCHAIN_PACKAGE;
use psi_handoff_consumer::applied_regions;

const NEW_PLAN: &str = "language\\core\\access\\new_plan";
const SELECTED: &str = include_str!("../../04_typed-trees-to-prepared-trees/tests/fixtures/access_policy/selected.omg");

/// The toolchain's own package, which defines `new_plan`.
fn toolchain() -> LoweredPackage {
    let library = omega_toolchain::SourcePackage::toolchain_definition();
    let prepared = omega_preparation::prepare_source_package(&library).unwrap();
    omega_lowering::lower_package(&prepared).unwrap()
}

fn machine<'a>(
    package: &'a LoweredPackage,
    owner: &str,
    path: &str,
) -> &'a LoweredMachine {
    let found: Vec<_> = package
        .machines
        .iter()
        .filter(|machine| machine.declaration.package == owner && machine.declaration.path == path)
        .collect();
    let [machine] = found.as_slice() else {
        panic!("expected one body of {owner} {path}, found {}", found.len())
    };
    machine
}

fn call(machine: &LoweredMachine) -> &PsiOperation {
    let calls: Vec<_> = machine
        .body
        .operations
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .collect();
    let [call] = calls.as_slice() else { panic!("{calls:?}") };
    call
}

/// What a body does, without the row numbers of the package that holds it:
/// each operation's kind, in order.
fn shape(machine: &LoweredMachine) -> Vec<std::mem::Discriminant<OperationKind>> {
    machine
        .body
        .operations
        .iter()
        .map(|operation| std::mem::discriminant(&operation.kind))
        .collect()
}

/// The consumer's copy of `new_plan` is the toolchain's body: the same
/// declaration, contract clauses, region slots and operations as the
/// toolchain's own package publishes.
fn assert_toolchain_body(
    package: &LoweredPackage,
    defining: &LoweredPackage,
) -> MachineRegion {
    let (ours, theirs) = (machine(package, TOOLCHAIN_PACKAGE, NEW_PLAN), machine(defining, TOOLCHAIN_PACKAGE, NEW_PLAN));
    assert_eq!(ours.declaration, theirs.declaration);
    assert_eq!(ours.regions, theirs.regions);
    assert_eq!(
        (&ours.contract.requires, &ours.contract.ensures),
        (&theirs.contract.requires, &theirs.contract.ensures)
    );
    assert_eq!(ours.contract.ensures.len(), 4, "the four count guarantees of the defining file");
    assert!(
        ours.contract
            .ensures
            .iter()
            .all(|clause| clause.site.package == TOOLCHAIN_PACKAGE)
    );
    assert_eq!(shape(ours), shape(theirs));
    assert_eq!(
        ours.body.diagnostics.operations, theirs.body.diagnostics.operations,
        "every operation is at its toolchain site"
    );
    assert!(ours.arguments.is_empty() && ours.body.call_regions.is_empty());
    let [region] = ours.regions.as_slice() else { panic!("{:?}", ours.regions) };
    region.clone()
}

/// The closed unit of the one policy `source` selects, in a package with a Build owner.
fn selected_unit(source: &str) -> Option<omega_lowering::PolicyUnit> {
    let mut resolved = sources::resolve(&[("policy.omg", source)]);
    resolved.bind_package_key("app", [0x42; 32]).unwrap();
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let [selection] = prepared.typed().access_policies.as_slice() else {
        panic!("one selected policy")
    };
    omega_lowering::lower_selected_policy_unit(&prepared, selection).unwrap()
}

#[test]
fn a_policy_unit_excludes_the_domains_that_depend_on_its_issuance_decision() {
    let source = format!("{SELECTED}
        domain i32::Locked::Child requires true;
        domain i32::Locked::Child::Grandchild;
        domain i32::Alias = i32::Locked::Child::Grandchild;
        domain i32::Independent;
        domain i32::Independent::Child;
        domain i32::IndependentAlias = i32::Independent::Child;");
    let unit = selected_unit(&source).expect("the selected plan closes without the policy-governed domains");
    let names: Vec<_> = unit.domains.iter().map(|domain| domain.declaration.path.as_str()).collect();
    assert_eq!(names, ["i32::Independent", "i32::Independent::Child"]);
    let aliases: Vec<_> = unit.domain_aliases.iter().map(|alias| alias.declaration.path.as_str()).collect();
    assert_eq!(aliases, ["i32::IndependentAlias"]);
}

/// The plan reaches the toolchain's constructor through a local wrapper. The
/// unit holds the three bodies in call order, each under its own package. Each
/// call names the next body and applies the caller's one region to it.
#[test]
fn a_local_wrapper_reaches_the_imported_body_through_its_own_region() {
    let defining = toolchain();
    let relay = "machine relay<'t>(schema:@TypeSchema<'t>, issuance:Issuance<'t>)->AccessPlan<'t> reaches none crashes none terminates {\n    new_plan::<'t>(schema, issuance)\n}\n";
    let source = format!("{}\n{relay}", SELECTED.replace("new_plan::<'t>(@schema", "relay::<'t>(@schema"));
    let unit = selected_unit(&source).expect("the plan, its wrapper and the constructor close");
    let bodies: Vec<_> = unit
        .machines
        .iter()
        .map(|machine| (machine.declaration.package.as_str(), machine.declaration.path.as_str()))
        .collect();
    assert_eq!(
        bodies,
        [
            ("app", "Rule::plan"),
            ("app", "relay"),
            (TOOLCHAIN_PACKAGE, NEW_PLAN)
        ]
    );
    let constructor_region = assert_toolchain_body(&unit, &defining);
    let [plan, relay, new_plan] = [
        ("app", "Rule::plan"),
        ("app", "relay"),
        (TOOLCHAIN_PACKAGE, NEW_PLAN),
    ]
    .map(|(owner, path)| machine(&unit, owner, path));
    let original = machine(&defining, TOOLCHAIN_PACKAGE, NEW_PLAN);
    spec_corpus::observe_handoff!(psi "psi-output/imported-operation-sites",
        assert_eq!((&new_plan.declaration, &new_plan.body.diagnostics.operations), (&original.declaration, &original.body.diagnostics.operations)),
    );
    // The wrapper and the constructor have the same slots: the region fills
    // the schema behind the reference, the issuance and the returned plan.
    let slots = MachineRegion {
        aggregate_parameters: vec![
            AggregateRegionSlot { parameter: 0, binder: 0 },
            AggregateRegionSlot { parameter: 1, binder: 0 },
        ],
        aggregate_result: vec![0],
        ..Default::default()
    };
    assert_eq!((&relay.regions[..], &constructor_region), (&[slots.clone()][..], &slots));

    for (caller, callee) in [(plan, relay), (relay, new_plan)] {
        let operation = call(caller);
        let OperationKind::Call { callee: target, .. } = &operation.kind else {
            unreachable!()
        };
        assert_eq!(*target, CallTarget::Machine(callee.id));
        let (selected, applied) = applied_regions(&unit, caller, CallSite::Operation(operation.id)).unwrap();
        assert_eq!(selected.declaration, callee.declaration);
        assert_eq!(applied.iter().map(|region| region.ordinal).collect::<Vec<_>>(), [0]);
    }
    // The wrapper forwards its own reference formal: the constructor reads the same referent.
    let OperationKind::Call { arguments, .. } = &call(relay).kind else {
        unreachable!()
    };
    let [
        CallArgument::Borrow { place, access },
        CallArgument::MoveWhole(_),
    ] = arguments.as_slice()
    else {
        panic!("{arguments:?}")
    };
    assert_eq!((place.path.as_slice(), *access), (&[PlaceProjection::Referent][..], ReferenceAccess::Shared));

    // Every schema the three bodies store is the toolchain's, with its own predicates.
    assert!(!unit.data.is_empty());
    for data in &unit.data {
        let defined = defining
            .data
            .iter()
            .find(|theirs| theirs.declaration == data.declaration)
            .unwrap_or_else(|| panic!("{:?} is not a toolchain schema", data.declaration));
        assert_eq!((data.lifetime_count, &data.requires), (defined.lifetime_count, &defined.requires));
    }
}

/// Identity is the declaring package. A local machine spelled `new_plan` is
/// the plan's callee when the source names it, and the toolchain's body is
/// then no part of the unit.
#[test]
fn a_local_namesake_is_selected_instead_of_the_imported_body() {
    let source = r"use omega\language\core;
use omega\language\core\reflect;
domain i32::Locked access Rule;
Rule satisfies AccessPolicy {
    machine plan<'t>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates {
        new_plan::<'t>(schema)
    }
}
machine new_plan<'t>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates {
    AccessPlan::<'t> { schema: schema, issuance: Issuance::Unrestricted{}, .. }
}
";
    let unit = selected_unit(source).expect("the plan and the local constructor close");
    let bodies: Vec<_> = unit
        .machines
        .iter()
        .map(|machine| (machine.declaration.package.as_str(), machine.declaration.path.as_str()))
        .collect();
    assert_eq!(bodies, [("app", "Rule::plan"), ("app", "new_plan")]);
    let (plan, namesake) = (machine(&unit, "app", "Rule::plan"), machine(&unit, "app", "new_plan"));
    let OperationKind::Call { callee, .. } = &call(plan).kind else {
        unreachable!()
    };
    assert_eq!(*callee, CallTarget::Machine(namesake.id));
}

/// The selected policy of the captured fixture as one closed unit: `Rule::plan`
/// and the constructor it calls, and nothing else of the package. The domain
/// that selects the policy is not in the unit, and the whole package still
/// refuses at that domain.
#[test]
fn a_selected_policy_lowers_as_a_closed_unit_of_its_plan_and_constructor() {
    let defining = toolchain();
    for issuance in ["Issuance::Unrestricted{}", "Issuance::Only{routes:[]}"] {
        let policy = SELECTED.replace("Issuance::Unrestricted{}", issuance);
        let source = format!("{policy}\nmachine forward(x:i32 in Locked)->i32 in Locked {{x}}\n");
        let mut resolved = sources::resolve(&[("policy.omg", &source)]);
        resolved.bind_package_key("app", [0x42; 32]).unwrap();
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let [selection] = prepared.typed().access_policies.as_slice() else {
            panic!("one selected policy")
        };
        let error = omega_lowering::lower_package(&prepared).expect_err("the domain's plan is not evaluated");
        assert_eq!(error.site, selection.site, "{issuance}");

        let unit = omega_lowering::lower_selected_policy_unit(&prepared, selection).unwrap();
        let unit = unit.expect("the plan and its constructor close");
        assert!(unit.machine_templates.is_empty() && unit.domains.is_empty(), "{issuance}");
        // `forward` is a machine of the package and no part of the policy's execution.
        let bodies: Vec<_> = unit
            .machines
            .iter()
            .map(|machine| (machine.declaration.package.as_str(), machine.declaration.path.as_str()))
            .collect();
        assert_eq!(bodies, [("app", "Rule::plan"), (TOOLCHAIN_PACKAGE, NEW_PLAN)], "{issuance}");
        assert_toolchain_body(&unit, &defining);

        let (plan, new_plan) = (machine(&unit, "app", "Rule::plan"), machine(&unit, TOOLCHAIN_PACKAGE, NEW_PLAN));
        let operation = call(plan);
        let OperationKind::Call { callee, arguments, .. } = &operation.kind else {
            unreachable!()
        };
        spec_corpus::observe_handoff!(psi "psi-output/closed-policy-constructor", assert_eq!(*callee, CallTarget::Machine(new_plan.id)));
        // The schema is lent from the plan's own formal, and the issuance the
        // source constructs is moved in.
        let [
            CallArgument::Borrow { place, access },
            CallArgument::MoveWhole(_),
        ] = arguments.as_slice()
        else {
            panic!("{arguments:?}")
        };
        assert_eq!((place.path.as_slice(), *access), (&[][..], ReferenceAccess::Shared), "{issuance}");
        let (_, applied) = applied_regions(&unit, plan, CallSite::Operation(operation.id)).unwrap();
        assert_eq!(applied.iter().map(|region| region.ordinal).collect::<Vec<_>>(), [0], "{issuance}");
    }
}
