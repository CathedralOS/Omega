//! The closed `plan` body a selecting package publishes is a different product
//! from the open template the caller proof reads. These tests lower both from
//! the two original access policies and pair the closed body with the checked
//! constructor only when it performs the template's steps and nothing else.
use omega_checker::{check_closed_policy_call, check_record_constructor, check_selected_policy_call};
use omega_psi::{
    BlockTerminator, CallRequirement, CallSite, CallTarget, CleanupAction, ContractClause, LoweredMachine, LoweredPackage, OperationKind, PlaceId,
    PlaceProjection, PredicateStep, ReferenceAccess, ReturnEdge, SelectedPolicyDefinition, ValueOperand,
};
use omega_resolver::{ParsedSource, StageSource};
use omega_toolchain::TOOLCHAIN_PACKAGE;
use std::num::NonZeroUsize;

const UNRESTRICTED: &str = include_str!("../../05_prepared-trees-to-terminal-psi/tests/fixtures/domain_access/unrestricted/access_plan.omg");
const EMPTY_ISSUERS: &str = include_str!("../../05_prepared-trees-to-terminal-psi/tests/fixtures/domain_access/empty-issuers/policy.omg");
const ZEROED: &str = include_str!("../../05_prepared-trees-to-terminal-psi/tests/fixtures/policy_templates/dotdot.omg");
const NEW_PLAN: &str = r"language\core\access\new_plan";

/// The open template and the closed unit of the one policy `text` selects.
fn lowered(text: &str) -> (SelectedPolicyDefinition, LoweredPackage) {
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
    let [policy] = prepared.typed().access_policies.as_slice() else {
        panic!("one selected domain policy")
    };
    (
        omega_lowering::lower_selected_policy_definition(&prepared, policy).unwrap(),
        omega_lowering::lower_selected_policy_unit(&prepared, policy)
            .unwrap()
            .expect("the selected plan and its constructor close")
            .into_package(),
    )
}

fn machine<'a>(
    unit: &'a LoweredPackage,
    package: &str,
    path: &str,
) -> &'a LoweredMachine {
    let found: Vec<_> = unit
        .machines
        .iter()
        .filter(|machine| machine.declaration.package == package && machine.declaration.path == path)
        .collect();
    let [machine] = found.as_slice() else {
        panic!("expected one body of {package} {path}, found {}", found.len())
    };
    machine
}

fn plan(unit: &mut LoweredPackage) -> &mut LoweredMachine {
    unit.machines
        .iter_mut()
        .find(|machine| machine.declaration.package == "app" && machine.declaration.path == "Rule::plan")
        .expect("the closed plan body")
}

/// Run the three judgments in order. The first two must hold, so a refusal
/// comes from matching the closed body.
fn closed(
    definition: &SelectedPolicyDefinition,
    unit: &LoweredPackage,
) -> Result<(), &'static str> {
    let id = &machine(unit, TOOLCHAIN_PACKAGE, NEW_PLAN).declaration;
    let constructor = check_record_constructor(unit, id).expect("the constructor body is unchanged");
    let call = check_selected_policy_call(definition, &constructor).expect("the open template is unchanged");
    check_closed_policy_call(&call).map(|_| ())
}

fn place(number: usize) -> PlaceId {
    PlaceId(NonZeroUsize::new(number).unwrap())
}

#[test]
fn both_original_policies_pair_their_closed_plan_with_the_checked_constructor() {
    for (text, case, routes, operations) in [
        (UNRESTRICTED, "Unrestricted", None, 2),
        (EMPTY_ISSUERS, "Only", Some("Only::routes"), 3),
        // An omitted route field closes to the same body as an authored `[]`.
        (ZEROED, "Only", Some("Only::routes"), 3),
    ] {
        let (definition, unit) = lowered(text);
        let (expected_plan, expected_constructor) = (machine(&unit, "app", "Rule::plan"), machine(&unit, TOOLCHAIN_PACKAGE, NEW_PLAN));
        let constructor = check_record_constructor(&unit, &expected_constructor.declaration).unwrap();
        let call = check_selected_policy_call(&definition, &constructor).unwrap();
        let checked = check_closed_policy_call(&call).unwrap();

        // The pair is the unit's own two bodies, and the constructor is the
        // body the constructor proof read.
        assert!(std::ptr::eq(checked.unit(), &unit));
        assert!(std::ptr::eq(checked.plan(), expected_plan));
        assert!(std::ptr::eq(checked.constructor(), expected_constructor));
        assert!(std::ptr::eq(checked.constructor(), constructor.machine()));
        assert_eq!(checked.plan().declaration, definition.plan.declaration);
        assert_eq!(checked.issuance_case().path, format!(r"language\core\Issuance::{case}"));
        assert_eq!(
            checked.empty_list_field().map(|field| field.path.to_owned()),
            routes.map(|name| format!(r"language\core\Issuance::{name}"))
        );

        // What a driver executes: the plan's only call is an ordinary
        // operation naming that constructor body, followed by a return.
        let body = &checked.plan().body;
        assert_eq!(body.operations.len(), operations, "{case}");
        let OperationKind::Call { callee, .. } = &body.operations.last().unwrap().kind else {
            panic!("the plan ends in its constructor call")
        };
        assert_eq!(*callee, CallTarget::Machine(checked.constructor().id));
        assert!(matches!(body.blocks[0].terminator, BlockTerminator::Return { .. }));
    }
}

#[test]
fn a_constructor_checked_in_another_unit_has_no_closed_caller_beside_it() {
    // The toolchain's own package publishes the constructor and no plan.
    let library = omega_toolchain::SourcePackage::toolchain_definition();
    let prepared = omega_preparation::prepare_source_package(&library).unwrap();
    let defining = omega_lowering::lower_package(&prepared).unwrap();
    let (definition, unit) = lowered(UNRESTRICTED);
    assert_eq!(
        closed(&definition, &defining),
        Err("selected policy has no closed body in its constructor's unit")
    );
    assert_eq!(closed(&definition, &unit), Ok(()));

    // Another policy's unit holds a `Rule::plan` of the same declaration and
    // the same constructor. Its conformance is written at another occurrence,
    // so it is not the implementation the template proof read.
    let (other_definition, other_unit) = lowered(EMPTY_ISSUERS);
    assert_eq!(definition.plan.declaration, other_definition.plan.declaration);
    let another_implementation = "unit publishes a different implementation of the selected policy";
    assert_eq!(closed(&definition, &other_unit), Err(another_implementation));
    assert_eq!(closed(&other_definition, &unit), Err(another_implementation));

    // The same policy text with only the issuance replaced keeps every
    // declaration, the conformance and the call at the same occurrences. The
    // issuance step is then the first difference, in either direction.
    let only = UNRESTRICTED.replace("Issuance::Unrestricted{}", "Issuance::Only{..}      ");
    assert_eq!(only.len(), UNRESTRICTED.len());
    let (only_definition, only_unit) = lowered(&only);
    assert_eq!(only_unit.conformances, unit.conformances);
    let another_step = "closed policy operation is not at its template step's occurrence";
    assert_eq!(closed(&definition, &only_unit), Err(another_step));
    assert_eq!(closed(&only_definition, &unit), Err(another_step));
    assert_eq!(closed(&only_definition, &only_unit), Ok(()));
}

#[test]
fn a_closed_body_that_departs_from_its_template_is_refused() {
    type Mutation = fn(&mut LoweredPackage);
    fn call(unit: &mut LoweredPackage) -> (&mut CallTarget, &mut Vec<ValueOperand>, &mut Vec<CallRequirement>) {
        let OperationKind::Call {
            callee,
            arguments,
            requirements,
        } = &mut plan(unit).body.operations.last_mut().unwrap().kind
        else {
            panic!("the plan ends in its constructor call")
        };
        (callee, arguments, requirements)
    }
    fn issuance(unit: &mut LoweredPackage) -> (&mut omega_psi::DeclarationId, &mut Vec<omega_psi::FieldInitializer>) {
        let OperationKind::EstablishCase { case, fields } = &mut plan(unit).body.operations[1].kind else {
            panic!("the plan establishes its issuance second")
        };
        (case, fields)
    }
    fn terminator(unit: &mut LoweredPackage) -> &mut BlockTerminator {
        &mut plan(unit).body.blocks[0].terminator
    }
    fn clause(unit: &LoweredPackage) -> ContractClause {
        ContractClause {
            predicate: vec![PredicateStep::Boolean(false)],
            site: machine(unit, "app", "Rule::plan").body.diagnostics.body.clone(),
        }
    }

    const CALLEE: &str = "closed policy call names another callee, region or requirement";
    const ADMISSION: &str = "unsupported closed policy regions, effects or contract";
    const LOAN: &str = "closed policy call does not lend its own schema formal";
    const ARGUMENTS: &str = "closed policy call needs a schema loan and a transferred issuance";
    const RETURN: &str = "closed policy body does not return its constructor's result";
    const CASE: &str = "closed policy body transfers a different issuance than its checked template";
    const ROSTER: &str = "closed policy issuance does not hold exactly the empty route sequence";
    const FLOW: &str = "unsupported control flow in closed policy body";
    // In the empty-issuers body: place 1 is the schema formal, 2 the empty
    // route sequence, 3 the issuance, 4 the call's result and 5 the returned place.
    let mutants: &[(&str, Mutation, &str)] = &[
        // Callee.
        (
            "the plan calls itself",
            |unit| {
                let own = plan(unit).id;
                *call(unit).0 = CallTarget::Machine(own);
            },
            CALLEE,
        ),
        ("the call dispatches through a member", |unit| *call(unit).0 = CallTarget::Member(0), CALLEE),
        (
            "the call owes a requirement",
            |unit| {
                let site = CallSite::Operation(plan(unit).body.operations.last().unwrap().id);
                call(unit).2.push(CallRequirement { site, clause: 0 });
            },
            CALLEE,
        ),
        // Region mapping.
        (
            "the call applies another region",
            |unit| plan(unit).body.call_regions[0].arguments = vec![omega_psi::CallRegion::Binder(1)],
            CALLEE,
        ),
        ("the call applies no region", |unit| plan(unit).body.call_regions.clear(), CALLEE),
        (
            "the call's region row names another site",
            |unit| plan(unit).body.call_regions[0].call = CallSite::Operation(omega_psi::OperationId(NonZeroUsize::new(1).unwrap())),
            CALLEE,
        ),
        (
            "the result is outside the plan's region",
            |unit| plan(unit).regions[0].aggregate_result.clear(),
            ADMISSION,
        ),
        (
            "the schema fills another binder",
            |unit| plan(unit).regions[0].aggregate_parameters[0].binder = 1,
            ADMISSION,
        ),
        ("the plan has a second region", |unit| plan(unit).regions.push(Default::default()), ADMISSION),
        (
            "the result is a reference in the region",
            |unit| plan(unit).regions[0].reference_result = true,
            ADMISSION,
        ),
        // Borrowed input.
        (
            "the schema is lent mutably",
            |unit| {
                let ValueOperand::Borrow { access, .. } = &mut call(unit).1[0] else {
                    unreachable!()
                };
                *access = ReferenceAccess::Mutable;
            },
            LOAN,
        ),
        (
            "the loan is of a referent below the formal",
            |unit| {
                let ValueOperand::Borrow { place, .. } = &mut call(unit).1[0] else {
                    unreachable!()
                };
                place.path.push(PlaceProjection::Referent);
            },
            LOAN,
        ),
        (
            "the loan is of the issuance",
            |unit| {
                let ValueOperand::Borrow { place: lent, .. } = &mut call(unit).1[0] else {
                    unreachable!()
                };
                lent.root = place(3);
            },
            LOAN,
        ),
        ("the schema is copied in", |unit| call(unit).1[0] = ValueOperand::CopyWhole(place(1)), LOAN),
        (
            "the issuance is copied in",
            |unit| call(unit).1[1] = ValueOperand::CopyWhole(place(3)),
            ARGUMENTS,
        ),
        ("a third argument", |unit| call(unit).1.push(ValueOperand::MoveWhole(place(2))), ARGUMENTS),
        // Transferred case and its roster.
        (
            "the route sequence is transferred instead",
            |unit| call(unit).1[1] = ValueOperand::MoveWhole(place(2)),
            CASE,
        ),
        (
            "the other case with the same payload",
            |unit| std::sync::Arc::make_mut(&mut issuance(unit).0.name).path = r"language\core\Issuance::Unrestricted".into(),
            CASE,
        ),
        (
            "the checked case without its route field",
            |unit| issuance(unit).1.clear(),
            "closed policy issuance payload differs from its checked template",
        ),
        (
            "the route sequence is moved, not copied",
            |unit| issuance(unit).1[0].operand = ValueOperand::MoveWhole(place(2)),
            ROSTER,
        ),
        (
            "the route field is another field",
            |unit| std::sync::Arc::make_mut(&mut issuance(unit).1[0].field.name).path = r"language\core\Issuance::Only::other".into(),
            ROSTER,
        ),
        (
            "the route sequence has an element",
            |unit| {
                let OperationKind::EstablishStructuralArray { elements } = &mut plan(unit).body.operations[0].kind else {
                    unreachable!()
                };
                elements.push(place(1));
            },
            ROSTER,
        ),
        // Return.
        (
            "the issuance is returned",
            |unit| {
                let BlockTerminator::Return {
                    result: ReturnEdge::Structural { source, .. },
                    ..
                } = terminator(unit)
                else {
                    unreachable!()
                };
                *source = place(3);
            },
            RETURN,
        ),
        (
            "the result is delivered into the call's own place",
            |unit| {
                let BlockTerminator::Return {
                    result: ReturnEdge::Structural { result, .. },
                    ..
                } = terminator(unit)
                else {
                    unreachable!()
                };
                *result = place(4);
            },
            RETURN,
        ),
        (
            "nothing is returned",
            |unit| {
                let BlockTerminator::Return { result, .. } = terminator(unit) else {
                    unreachable!()
                };
                *result = ReturnEdge::Unit;
            },
            RETURN,
        ),
        (
            "the return discards the schema",
            |unit| {
                let BlockTerminator::Return { cleanup, .. } = terminator(unit) else {
                    unreachable!()
                };
                cleanup.push(CleanupAction::DiscardRoot(place(1)));
            },
            RETURN,
        ),
        (
            "a terminal call replaces the return",
            |unit| {
                let (callee, arguments) = {
                    let (callee, arguments, _) = call(unit);
                    (*callee, arguments.clone())
                };
                let BlockTerminator::Return { id, .. } = *terminator(unit) else {
                    unreachable!()
                };
                *terminator(unit) = BlockTerminator::TerminalCall {
                    id,
                    callee,
                    arguments,
                    requirements: vec![],
                    cleanup: vec![],
                };
            },
            RETURN,
        ),
        // Effects and contract.
        ("the body blocks", |unit| plan(unit).contract.realized_pause.block = true, ADMISSION),
        (
            "the declared bound permits suspension",
            |unit| plan(unit).contract.pause_bounds.suspend = Some(true),
            ADMISSION,
        ),
        ("no termination promise", |unit| plan(unit).contract.promises_termination = false, ADMISSION),
        ("an inherited reach", |unit| plan(unit).contract.reach = None, ADMISSION),
        ("a crash ceiling", |unit| plan(unit).contract.has_empty_crash_ceiling = false, ADMISSION),
        (
            "an unproved guarantee",
            |unit| {
                let clause = clause(unit);
                plan(unit).contract.ensures.push(clause);
            },
            ADMISSION,
        ),
        (
            "a requirement",
            |unit| {
                let clause = clause(unit);
                plan(unit).contract.requires.push(clause);
            },
            ADMISSION,
        ),
        ("a mutable home", |unit| plan(unit).body.mutable_homes.push(place(1)), ADMISSION),
        ("a region binding", |unit| plan(unit).body.lifetime_bindings.push((place(4), 0)), ADMISSION),
        // Unrelated operations and places.
        (
            "an operation after the call",
            |unit| {
                let body = &mut plan(unit).body;
                let extra = body.operations[0].clone();
                body.operations.push(extra);
                body.diagnostics.operations.push(body.diagnostics.body.clone());
                body.blocks[0].operations = 0..4;
            },
            FLOW,
        ),
        ("an operation outside the block", |unit| plan(unit).body.blocks[0].operations = 1..3, FLOW),
        (
            "a place no operation defines",
            |unit| {
                let body = &mut plan(unit).body;
                let extra = body.places[0];
                body.places.push(extra);
            },
            FLOW,
        ),
        (
            "the call is at another occurrence",
            |unit| {
                let sites = &mut plan(unit).body.diagnostics;
                sites.operations[2] = sites.body.clone();
            },
            "closed policy operation is not at its template step's occurrence",
        ),
        (
            "an operation of another package",
            |unit| plan(unit).body.diagnostics.operations[0].package = TOOLCHAIN_PACKAGE.into(),
            "closed policy body does not belong to its declaration",
        ),
        // The declaration's other subjects in the unit.
        (
            "an open template of the same declaration",
            |unit| unit.machine_templates.push(lowered(EMPTY_ISSUERS).0.plan),
            "ambiguous closed policy declaration",
        ),
        (
            "the unit drops the policy's conformance",
            |unit| unit.conformances.clear(),
            "unit publishes a different implementation of the selected policy",
        ),
        (
            "the conformance supplies another member",
            |unit| std::sync::Arc::make_mut(&mut unit.conformances[0].members[0].implementation.name).path = "Rule::other".into(),
            "unit publishes a different implementation of the selected policy",
        ),
    ];
    let (definition, original) = lowered(EMPTY_ISSUERS);
    assert_eq!(closed(&definition, &original), Ok(()));
    for &(label, mutate, expected) in mutants {
        let (_, mut unit) = lowered(EMPTY_ISSUERS);
        mutate(&mut unit);
        assert_ne!(unit, original, "{label}");
        assert_eq!(closed(&definition, &unit), Err(expected), "{label}");
    }
}

#[test]
fn the_unrestricted_body_cannot_gain_a_case_payload_or_another_case() {
    // Place 1 is the schema formal, 2 the issuance and 3 the call's result.
    let (definition, original) = lowered(UNRESTRICTED);
    let (_, donor) = lowered(EMPTY_ISSUERS);
    let OperationKind::EstablishCase { case: only, fields: routes } = &machine(&donor, "app", "Rule::plan").body.operations[1].kind else {
        panic!("the empty-issuers plan establishes `Only`")
    };

    // `Only` in place of `Unrestricted`, without the roster it stores.
    let (_, mut unit) = lowered(UNRESTRICTED);
    let OperationKind::EstablishCase { case, .. } = &mut plan(&mut unit).body.operations[0].kind else {
        panic!("the unrestricted plan establishes its issuance first")
    };
    *case = only.clone();
    assert_ne!(unit, original);
    assert_eq!(
        closed(&definition, &unit),
        Err("closed policy body transfers a different issuance than its checked template")
    );

    // `Unrestricted` that names a route field it does not declare.
    let (_, mut unit) = lowered(UNRESTRICTED);
    let OperationKind::EstablishCase { fields, .. } = &mut plan(&mut unit).body.operations[0].kind else {
        unreachable!()
    };
    fields.push(omega_psi::FieldInitializer {
        field: routes[0].field.clone(),
        operand: ValueOperand::CopyWhole(place(1)),
    });
    assert_eq!(
        closed(&definition, &unit),
        Err("closed policy issuance payload differs from its checked template")
    );
}
