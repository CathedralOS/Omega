#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    AliasTarget, BlockTerminator, CallArgument, LoweredMachine, LoweredPackage, OperationKind, OperationResult, Parameter, ReferenceAccess, ReturnEdge,
    ScalarType, TemplateAccess, TemplateCallable, TypeExpression, TypeExpressionStep, TypeShape,
};
use psi_handoff_consumer::callable_loans::{CallLoan, Referent, RuntimeLoans, runtime_loans};

const MUTABLE: &str = include_str!("fixtures/package_cases/runtime-callable-mutable-loan-formal/case/case.omg");
const MUTABLE_CONTROL: &str = include_str!("fixtures/package_cases/runtime-callable-mutable-loan-formal/control/case.omg");
const RECORD: &str = include_str!("fixtures/package_cases/runtime-callable-shared-record-reference-formal/case/case.omg");
const RECORD_CONTROL: &str = include_str!("fixtures/package_cases/runtime-callable-shared-record-reference-formal/control/case.omg");
const BORROWED: &str = include_str!("fixtures/package_cases/runtime-callable-borrowed-result/case/case.omg");
const BORROWED_CONTROL: &str = include_str!("fixtures/package_cases/runtime-callable-borrowed-result/control/case.omg");

fn lower(source: &str) -> LoweredPackage {
    sources::lower(&[("case.omg", source)]).unwrap_or_else(|e| panic!("{source}: {e:?}"))
}

fn machine<'a>(
    package: &'a LoweredPackage,
    name: &str,
) -> &'a LoweredMachine {
    package
        .machines
        .iter()
        .find(|m| m.declaration.path == name)
        .unwrap()
}

fn kept_contract<'a>(
    package: &'a LoweredPackage,
    alias_name: &str,
) -> &'a TemplateCallable {
    let keep = machine(package, "keep");
    let [carrier] = keep.parameter_types[..] else { panic!("one keep parameter") };
    let [Parameter::Scalar(descriptor)] = keep.body.parameters[..] else {
        panic!("one descriptor")
    };
    assert_eq!(keep.body.values[descriptor.index()].ty, carrier);
    let alias = package
        .aliases
        .iter()
        .find(|a| a.declaration.path == alias_name)
        .unwrap();
    assert_eq!(alias.target, AliasTarget::Structural(carrier));
    let Some(TypeShape::Callable(id)) = package.types.get(carrier) else {
        panic!("callable descriptor type")
    };
    &package.callable_contracts[id.index()]
}

fn integer(width: u64) -> ScalarType {
    ScalarType::Integer { signed: false, width }
}

fn scalar(width: u64) -> TypeExpression {
    TypeExpression(vec![TypeExpressionStep::Scalar(integer(width))])
}

fn reference(
    referent: TypeExpressionStep,
    access: TemplateAccess,
) -> TypeExpression {
    TypeExpression(vec![referent, TypeExpressionStep::Reference(access)])
}

#[test]
fn mutable_callable_formal_publishes_exclusive_access_and_reborrows_on_each_call() {
    let package = lower(MUTABLE);
    let control = lower(MUTABLE_CONTROL);
    let contract = kept_contract(&package, "Bump");
    let shared = kept_contract(&control, "Bump");
    let byte_ref = |access| reference(TypeExpressionStep::Scalar(integer(8)), access);
    spec_corpus::observe_handoff!(psi "psi-output/runtime-mutable-loan-formal", assert_eq!(
        (&contract.parameters, &contract.result, &shared.parameters, &shared.result),
        (&vec![byte_ref(TemplateAccess::Mut)], &Some(scalar(8)), &vec![byte_ref(TemplateAccess::Shared)], &Some(scalar(8)))
    ));

    let mut invoked = lower(&format!("{MUTABLE} machine apply(f:Bump, x:@mut u8)->u8 {{ let first:u8=f(x); f(x) }}"));
    let expected = || RuntimeLoans {
        descriptor_parameter: 0,
        loans: vec![CallLoan {
            formal: 0,
            caller_parameter: 1,
            access: ReferenceAccess::Mutable,
            referent: Referent::Scalar(integer(8)),
        }],
        result_lifetime_parameter: None,
    };
    spec_corpus::observe_handoff!(psi "psi-output/runtime-mutable-reborrows",
        assert_eq!(runtime_loans(&invoked, machine(&invoked, "apply")), Some(vec![expected(), expected()])));

    let apply = invoked
        .machines
        .iter_mut()
        .find(|m| m.declaration.path == "apply")
        .unwrap();
    let arguments = apply
        .body
        .operations
        .iter_mut()
        .find_map(|op| match &mut op.kind {
            OperationKind::Call { arguments, .. } => Some(arguments),
            _ => None,
        })
        .unwrap();
    let CallArgument::Borrow { access, .. } = &mut arguments[0] else {
        panic!("reborrow")
    };
    *access = ReferenceAccess::Shared;
    assert!(runtime_loans(&invoked, machine(&invoked, "apply")).is_none());
}

#[test]
fn shared_record_callable_names_the_schema_used_by_the_invocation() {
    let package = lower(RECORD);
    let control = lower(RECORD_CONTROL);
    let contract = kept_contract(&package, "CounterValue");
    let scalar_control = kept_contract(&control, "CounterValue");
    let counter = package
        .data
        .iter()
        .find(|d| d.declaration.path == "Counter")
        .unwrap();
    let target = TypeExpressionStep::Named {
        declaration: counter.declaration.clone(),
        argument_count: 0,
    };
    assert!(contract.terminates && scalar_control.terminates);
    spec_corpus::observe_handoff!(psi "psi-output/runtime-shared-record-formal", assert_eq!(
        (&contract.parameters, &contract.result, &scalar_control.parameters, &scalar_control.result),
        (&vec![reference(target, TemplateAccess::Shared)], &Some(scalar(64)),
         &vec![reference(TypeExpressionStep::Scalar(integer(64)), TemplateAccess::Shared)], &Some(scalar(64)))
    ));

    // A second record has identical storage. The declaration identity still
    // chooses Counter when the caller reborrows its third parameter.
    let invoked = lower(&format!(
        "{RECORD} data Decoy {{n:u64;}} machine read(f:CounterValue, y:@Decoy, x:@Counter)->u64 {{ f(x) }}"
    ));
    let counter = &invoked
        .data
        .iter()
        .find(|d| d.declaration.path == "Counter")
        .unwrap()
        .declaration;
    spec_corpus::observe_handoff!(psi "psi-output/runtime-record-reborrow", assert_eq!(
        runtime_loans(&invoked, machine(&invoked, "read")),
        Some(vec![RuntimeLoans { descriptor_parameter: 0, loans: vec![CallLoan {
            formal: 0, caller_parameter: 2, access: ReferenceAccess::Shared, referent: Referent::Nominal(counter)
        }], result_lifetime_parameter: None }])
    ));
}

#[test]
fn borrowed_callable_result_uses_the_contracts_lifetime_input_and_its_own_descriptor() {
    let package = lower(BORROWED);
    let control = lower(BORROWED_CONTROL);
    let contract = kept_contract(&package, "Get");
    let plain = kept_contract(&control, "Get");
    let byte_ref = || reference(TypeExpressionStep::Scalar(integer(8)), TemplateAccess::Shared);
    assert_eq!(contract.parameters, [byte_ref()]);
    assert_eq!(contract.parameters, plain.parameters);
    spec_corpus::observe_handoff!(psi "psi-output/runtime-borrowed-result", assert_eq!(
        (&contract.result, contract.result_lifetime_input, &plain.result, plain.result_lifetime_input),
        (&Some(byte_ref()), Some(0), &Some(scalar(8)), None)
    ));

    let source = BORROWED.replace("machine(x:@u8)", "machine(n:u8,x:@u8)");
    let mut invoked = lower(&format!("{source} machine relay(f:Get,n:u8,x:@u8)->@u8 {{ f(n,x) }}"));
    let relay = machine(&invoked, "relay");
    let [block] = relay.body.blocks.as_slice() else { panic!("one return block") };
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source, result },
        ..
    } = block.terminator
    else {
        panic!("returned reference")
    };
    let call = relay
        .body
        .operations
        .iter()
        .find(|op| matches!(op.kind, OperationKind::Call { .. }))
        .unwrap();
    assert_eq!(call.result, OperationResult::Structural(source));
    assert_ne!(source, result);
    assert_ne!(Parameter::Structural(source), relay.body.parameters[2]);
    spec_corpus::observe_handoff!(psi "psi-output/runtime-borrowed-call-lifetime", assert_eq!(
        runtime_loans(&invoked, relay),
        Some(vec![RuntimeLoans { descriptor_parameter: 0, loans: vec![CallLoan {
            formal: 1, caller_parameter: 2, access: ReferenceAccess::Shared, referent: Referent::Scalar(integer(8))
        }], result_lifetime_parameter: Some(2) }])
    ));

    // Retargeting the published lifetime to the scalar input forges a relation
    // the consumer cannot instantiate from the ordered call operands.
    let [contract] = invoked.callable_contracts.as_mut_slice() else {
        panic!("one contract")
    };
    assert_eq!(contract.result_lifetime_input, Some(1));
    contract.result_lifetime_input = Some(0);
    assert!(runtime_loans(&invoked, machine(&invoked, "relay")).is_none());
}

#[test]
fn unresolved_or_explicit_lifetimes_do_not_acquire_an_elided_input_relation() {
    // These aliases can describe their interfaces. None is an admitted
    // runtime contract with the direct, single-input lifetime relation.
    for source in [
        "data Two = machine(x:@u8,y:@u8)->@u8 reaches none crashes none;",
        "data Aggregate = machine(x:@u8,others:[@u8;2])->@u8 reaches none crashes none;",
        "data Explicit<'r> = machine(x:@u8)->@'r u8 reaches none crashes none;",
        "data NoInput = machine(x:u8)->@u8 reaches none crashes none;",
    ] {
        let package = lower(source);
        assert!(package.callable_contracts.is_empty(), "{source}");
        let [alias] = package.aliases.as_slice() else { panic!("one alias") };
        let AliasTarget::Expansion(expansion) = &alias.target else {
            panic!("declaration-only alias: {source}")
        };
        let omega_lowering::DeclarationTerm::Type(TypeExpression(steps)) = &expansion.expansion else {
            panic!("callable expansion")
        };
        let [TypeExpressionStep::Callable(interface)] = steps.as_slice() else {
            panic!("callable interface")
        };
        assert_eq!(interface.result_lifetime_input, None, "{source}");
        if alias.declaration.path == "Explicit" {
            assert_eq!(interface.result.as_ref().unwrap().0.last(), Some(&TypeExpressionStep::AtLifetime));
        }
    }
}

#[test]
fn a_suspending_callable_parks_the_selected_descriptor_until_its_own_completion() {
    const PAUSE: &str = include_str!("fixtures/package_cases/suspending-runtime-callable-contract/case/case.omg");
    const PLAIN: &str = include_str!("fixtures/package_cases/suspending-runtime-callable-contract/control/case.omg");
    let pause = lower(PAUSE);
    let plain = lower(PLAIN);
    spec_corpus::observe_handoff!(psi "psi-output/suspending-callable-contract", assert_eq!(
        (kept_contract(&pause,"Pause").pause.suspend, kept_contract(&plain,"Pause").pause.suspend), (true,false)
    ));
    let use_site = "machine run(f:Pause,g:Pause,b:bool)->u8 suspends {let n:u8=suspend f(); transition b {true -> suspend g(), false -> n,}}";
    let package = lower(&format!("{PAUSE} {use_site}"));
    let quiet = lower(&format!("{PLAIN} {use_site}"));
    let run=machine(&package,"run");
    assert!(run.contract.realized_pause.suspend);
    spec_corpus::observe_handoff!(psi "psi-output/suspending-callable-crossings", assert_eq!(
        (psi_handoff_consumer::callable_loans::parked_runtime(&package,run),
         psi_handoff_consumer::callable_loans::parked_runtime(&quiet,machine(&quiet,"run"))),
        (Some(vec![(0,false,false),(1,true,false)]),Some(vec![]))
    ));
    let (_,other) = run.body.suspension_sites.split_first().unwrap();
    assert_eq!(other.len(),1);
    let first = &run.body.suspension_sites[0];
    let Parameter::Scalar(g)=run.body.parameters[1] else {panic!("descriptor")};
    assert!(first.retained_callables.iter().any(|v| v.descriptor==g && !v.needed_by_callee && v.needed_after_completion));
}
