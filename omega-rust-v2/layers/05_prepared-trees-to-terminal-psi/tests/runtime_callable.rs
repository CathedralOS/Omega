#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    CallArgument, CallTarget, CallableContractId, LoweredMachine, LoweredPackage, OperationKind, OperationResult, Parameter, ScalarType, TypeExpression,
    TypeExpressionStep, TypeId, TypeShape, ValueId,
};

const CALLBACK: &str = "machine(x:u8)->u8 reaches none crashes none";

fn lower(source: &str) -> LoweredPackage {
    sources::lower(&[("runtime_callable.omg", source)]).unwrap_or_else(|error| panic!("{source}: {error:?}"))
}

fn machine<'a>(
    package: &'a LoweredPackage,
    path: &str,
) -> &'a LoweredMachine {
    package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == path)
        .unwrap_or_else(|| panic!("missing machine {path}"))
}

fn contract_of(
    package: &LoweredPackage,
    ty: TypeId,
) -> CallableContractId {
    match package.types.get(ty) {
        Some(TypeShape::Callable(contract)) => *contract,
        other => panic!("expected a runtime callable carrier, found {other:?}"),
    }
}

fn scalar_parameter(
    machine: &LoweredMachine,
    index: usize,
) -> ValueId {
    match machine.body.parameters[index] {
        Parameter::Scalar(value) => value,
        other => panic!("expected a scalar descriptor parameter, found {other:?}"),
    }
}

fn spelling<'s>(
    package: &LoweredPackage,
    contract: CallableContractId,
    source: &'s str,
) -> &'s [u8] {
    package.callable_contracts[contract.index()]
        .site
        .span
        .slice(source.as_bytes())
        .expect("contract site lies in its source")
}

fn byte() -> TypeExpression {
    TypeExpression(vec![TypeExpressionStep::Scalar(ScalarType::Integer {
        signed: false,
        width: 8,
    })])
}

fn calls(machine: &LoweredMachine) -> Vec<(&CallTarget, &[CallArgument])> {
    machine
        .body
        .operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::Call { callee, arguments, .. } => Some((callee, arguments.as_slice())),
            _ => None,
        })
        .collect()
}

/// (source descriptor, destination contract, refined descriptor) in emission order.
fn refinements(machine: &LoweredMachine) -> Vec<(ValueId, CallableContractId, ValueId)> {
    machine
        .body
        .operations
        .iter()
        .filter_map(|op| match (&op.kind, op.result) {
            (OperationKind::RefineCallableDescriptor { source, contract }, OperationResult::Scalar(result)) => Some((*source, *contract, result)),
            (OperationKind::RefineCallableDescriptor { .. }, other) => panic!("refinement must produce a scalar descriptor, found {other:?}"),
            _ => None,
        })
        .collect()
}

#[test]
fn a_runtime_call_names_the_current_descriptor_of_the_selected_formal_and_its_contract() {
    for (selected, index) in [("left", 0), ("right", 1)] {
        let source = format!("machine pick(left:{CALLBACK}, right:{CALLBACK}, value:u8)->u8 {{{selected}(value)}}");
        let package = lower(&source);
        let pick = machine(&package, "pick");
        let formals = [
            contract_of(&package, pick.parameter_types[0]),
            contract_of(&package, pick.parameter_types[1]),
        ];
        assert_ne!(formals[0], formals[1], "each formal declares an independent contract");
        assert_eq!(package.callable_contracts.len(), 2, "{source}");
        let [(callee, arguments)] = calls(pick)
            .try_into()
            .unwrap_or_else(|c: Vec<_>| panic!("expected one call, found {}", c.len()));
        assert_eq!(
            *callee,
            CallTarget::Runtime {
                descriptor: scalar_parameter(pick, index),
                contract: formals[index],
            },
            "{source}"
        );
        assert_eq!(arguments, [CallArgument::Scalar(scalar_parameter(pick, 2))]);
        assert!(refinements(pick).is_empty(), "the invoked formal already has the call contract");

        let contract = &package.callable_contracts[formals[index].index()];
        assert_eq!(contract.parameters, [byte()]);
        assert_eq!(contract.result, Some(byte()));
        assert_eq!(contract.reach, Some(Vec::new()));
        assert_eq!(contract.crash_ceiling, Some(Vec::new()));
        assert!(contract.requires.is_empty() && contract.ensures.is_empty());
        let declared = source.match_indices(CALLBACK).nth(index).unwrap().0;
        assert_eq!(
            contract.site.span.get_start_offset(),
            declared,
            "the contract is the selected formal's own declaration"
        );
        assert_eq!(spelling(&package, formals[index], &source), CALLBACK.as_bytes());
    }
}

#[test]
fn forwarding_passes_the_actual_descriptor_refined_into_the_callee_contract() {
    let source = format!(
        "machine run(f:{CALLBACK}, value:u8)->u8 {{f(value)}}
         machine wrap(g:{CALLBACK}, value:u8)->u8 {{run((g), value)}}"
    );
    let package = lower(&source);
    let (run, wrap) = (machine(&package, "run"), machine(&package, "wrap"));
    let run_formal = contract_of(&package, run.parameter_types[0]);
    let wrap_formal = contract_of(&package, wrap.parameter_types[0]);
    assert_ne!(run_formal, wrap_formal);

    let [(inner, _)] = calls(run)
        .try_into()
        .unwrap_or_else(|_| panic!("run invokes its formal once"));
    assert_eq!(
        *inner,
        CallTarget::Runtime {
            descriptor: scalar_parameter(run, 0),
            contract: run_formal,
        }
    );

    // The caller keeps its own descriptor; only the contract view changes.
    let [(source_descriptor, destination, refined)] = refinements(wrap)
        .try_into()
        .unwrap_or_else(|_| panic!("expected one refinement"));
    assert_eq!(source_descriptor, scalar_parameter(wrap, 0));
    assert_eq!(destination, run_formal);
    assert_eq!(contract_of(&package, wrap.body.values[refined.index()].ty), run_formal);
    let [(outer, arguments)] = calls(wrap)
        .try_into()
        .unwrap_or_else(|_| panic!("wrap forwards once"));
    assert_eq!(*outer, CallTarget::Machine(run.id), "forwarding never substitutes a runtime or static target");
    assert_eq!(
        arguments,
        [
            CallArgument::Scalar(refined),
            CallArgument::Scalar(scalar_parameter(wrap, 1))
        ]
    );
}

#[test]
fn a_local_landing_records_the_exact_refinement_between_contracts() {
    // The L3 refinement program: the actual and destination contracts differ only in their declarations.
    let source = "machine adapt(g:machine(actual:u8)->u8 reaches none crashes none) {let f:machine(formal:u8)->u8 reaches none crashes none=g;}";
    let package = lower(source);
    let adapt = machine(&package, "adapt");
    let actual = contract_of(&package, adapt.parameter_types[0]);
    assert_eq!(spelling(&package, actual, source), b"machine(actual:u8)->u8 reaches none crashes none");
    let [(descriptor, destination, refined)] = refinements(adapt)
        .try_into()
        .unwrap_or_else(|_| panic!("expected one refinement"));
    assert_eq!(descriptor, scalar_parameter(adapt, 0));
    assert_eq!(spelling(&package, destination, source), b"machine(formal:u8)->u8 reaches none crashes none");
    assert_eq!(contract_of(&package, adapt.body.values[refined.index()].ty), destination);
    assert!(calls(adapt).is_empty(), "a landing invokes nothing");
}

#[test]
fn a_join_retains_each_actual_descriptor_and_returns_the_joined_value() {
    let source = format!(
        "machine choose(flag:bool, left:{CALLBACK}, right:{CALLBACK})->{CALLBACK} {{
             let chosen:{CALLBACK} = match flag {{false -> left, true -> right,}};
             chosen
         }}"
    );
    let package = lower(&source);
    let choose = machine(&package, "choose");
    let declaration = |n: usize| source.match_indices(CALLBACK).nth(n).unwrap().0;
    let contract_at = |offset: usize| {
        (0..package.callable_contracts.len())
            .map(|i| CallableContractId::from_index(i).unwrap())
            .find(|c| {
                package.callable_contracts[c.index()]
                    .site
                    .span
                    .get_start_offset()
                    == offset
            })
            .unwrap()
    };
    let (result, local) = (contract_at(declaration(2)), contract_at(declaration(3)));
    assert_eq!(contract_of(&package, choose.result_type.unwrap()), result);
    let (left, right) = (scalar_parameter(choose, 1), scalar_parameter(choose, 2));
    let [first, second, returned] = refinements(choose)
        .try_into()
        .unwrap_or_else(|r: Vec<_>| panic!("expected three refinements, found {r:?}"));
    // Each arm contributes its own actual descriptor; no static callee replaces either.
    assert_eq!((first.0, first.1), (left, local));
    assert_eq!((second.0, second.1), (right, local));
    assert!(
        ![left, right, first.2, second.2].contains(&returned.0),
        "the return refines the joined descriptor"
    );
    assert_eq!(contract_of(&package, choose.body.values[returned.0.index()].ty), local);
    assert_eq!(returned.1, result);
    assert!(calls(choose).is_empty());
}

#[test]
fn the_invocation_after_a_join_uses_the_arriving_descriptor() {
    let source = format!(
        "machine storage(a:[u8;2])->[u8;2]{{a}} machine choose(flag:bool, left:{CALLBACK}, right:{CALLBACK}, n:u8)->u8 {{ let chosen:{CALLBACK} = match flag {{false -> left, true -> right,}}; chosen(n) }}"
    );
    let package = lower(&source);
    let choose = machine(&package, "choose");
    let [(target, arguments)] = calls(choose)
        .try_into()
        .unwrap_or_else(|_| panic!("one runtime invocation"));
    let CallTarget::Runtime { descriptor, contract } = *target else {
        panic!("lost runtime selection")
    };
    assert_eq!(arguments, [CallArgument::Scalar(scalar_parameter(choose, 3))]);
    let joined = &choose.body.values[descriptor.index()];
    let omega_lowering::ValueOrigin::BlockParameter(block) = joined.origin else {
        panic!("invocation selected a branch representative")
    };
    assert!(
        choose.body.blocks[block.get() - 1]
            .scalar_parameters
            .contains(&descriptor)
    );
    assert_eq!(contract_of(&package, joined.ty), contract);
    let incoming: Vec<_> = refinements(choose)
        .into_iter()
        .filter(|(_, id, _)| *id == contract)
        .map(|(source, _, _)| source)
        .collect();
    assert_eq!(incoming, [scalar_parameter(choose, 1), scalar_parameter(choose, 2)]);
    assert!(matches!(
        package
            .types
            .get(machine(&package, "storage").parameter_types[0]),
        Some(TypeShape::FixedArray { length: 2, .. })
    ));
}

#[test]
fn unsupported_runtime_contracts_and_descriptor_construction_refuse() {
    // A loan formal needs custody lowering the contract cannot yet state.
    let loan = "machine keep(f:machine(x:@u8)->u8 reaches none crashes none) {}";
    let error = sources::lower(&[("runtime_callable.omg", loan)]).unwrap_err();
    assert!(error.message.contains("runtime callable"), "{error:?}");

    // Constructing a descriptor from a named machine is not forwarding.
    let construction = format!(
        "machine action(x:u8)->u8 reaches none crashes none {{x}}
         machine run(f:{CALLBACK}, value:u8)->u8 {{f(value)}}
         machine start(value:u8)->u8 {{run((action), value)}}"
    );
    assert!(sources::lower(&[("runtime_callable.omg", &construction)]).is_err());
}
