#[path = "support/known_gap.rs"]
mod known_gap;
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    AliasTarget, CallArgument, CallTarget, CallableContractId, DeclarationTerm, LoweredMachine, LoweredPackage, OperationKind, OperationResult, Parameter,
    ScalarType, TypeExpression, TypeExpressionStep, TypeId, TypeShape, ValueId,
};

/// A callable signature is written once, as the whole target of a transparent
/// alias. The contract's declaration site is this signature.
const SIGNATURE: &str = "machine(x:u8)->u8 reaches none crashes none";

fn alias(name: &str) -> String {
    format!("data {name} = {SIGNATURE};")
}

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

/// The offset of the `n`th authored alias signature.
fn declared(
    source: &str,
    n: usize,
) -> usize {
    source.match_indices(SIGNATURE).nth(n).unwrap().0
}

/// The single contract whose declaration begins at `offset`.
fn contract_at(
    package: &LoweredPackage,
    offset: usize,
) -> CallableContractId {
    let [found] = (0..package.callable_contracts.len())
        .map(|i| CallableContractId::from_index(i).unwrap())
        .filter(|c| {
            package.callable_contracts[c.index()]
                .site
                .span
                .get_start_offset()
                == offset
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap_or_else(|found: Vec<_>| panic!("expected one contract at {offset}, found {found:?}"));
    found
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

fn integer(width: u64) -> TypeExpression {
    TypeExpression(vec![TypeExpressionStep::Scalar(ScalarType::Integer { signed: false, width })])
}

fn byte() -> TypeExpression {
    integer(8)
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
    // One alias for both formals, then an independent alias for each formal.
    for (headers, formal_types, shared) in [
        (alias("Step"), ["Step", "Step"], true),
        (format!("{} {}", alias("Left"), alias("Right")), ["Left", "Right"], false),
    ] {
        for (selected, index) in [("left", 0), ("right", 1)] {
            let [left, right] = formal_types;
            let source = format!("{headers} machine pick(left:{left}, right:{right}, value:u8)->u8 {{{selected}(value)}}");
            let package = lower(&source);
            let pick = machine(&package, "pick");
            let formals = [
                contract_of(&package, pick.parameter_types[0]),
                contract_of(&package, pick.parameter_types[1]),
            ];
            if shared {
                assert_eq!(formals[0], formals[1], "every use of one alias names its one contract: {source}");
                assert_eq!(package.callable_contracts.len(), 1, "{source}");
            } else {
                assert_ne!(formals[0], formals[1], "each alias declares an independent contract: {source}");
                assert_eq!(package.callable_contracts.len(), 2, "{source}");
            }
            let [(callee, arguments)] = calls(pick)
                .try_into()
                .unwrap_or_else(|c: Vec<_>| panic!("expected one call, found {}", c.len()));
            // The descriptor, never the contract, identifies the selected formal.
            assert_eq!(
                *callee,
                CallTarget::Runtime {
                    descriptor: scalar_parameter(pick, index),
                    contract: formals[index],
                },
                "{source}"
            );
            assert_ne!(scalar_parameter(pick, 0), scalar_parameter(pick, 1));
            assert_eq!(arguments, [CallArgument::Scalar(scalar_parameter(pick, 2))]);
            assert!(refinements(pick).is_empty(), "the invoked formal already has the call contract");

            let contract = &package.callable_contracts[formals[index].index()];
            assert_eq!(contract.parameters, [byte()]);
            assert_eq!(contract.result, Some(byte()));
            assert_eq!(contract.reach, Some(Vec::new()));
            assert_eq!(contract.crash_ceiling, Some(Vec::new()));
            assert!(contract.requires.is_empty() && contract.ensures.is_empty());
            assert_eq!(
                contract.site.span.get_start_offset(),
                declared(&source, if shared { 0 } else { index }),
                "the contract is the signature of the selected formal's alias"
            );
            assert_eq!(spelling(&package, formals[index], &source), SIGNATURE.as_bytes());
        }
    }
}

#[test]
fn an_alias_declaration_names_the_contract_that_its_uses_share() {
    let source = format!("{} machine run(f:Step, value:u8)->u8 {{f(value)}}", alias("Step"));
    let package = lower(&source);
    let used = contract_of(&package, machine(&package, "run").parameter_types[0]);
    let [step] = &package.aliases[..] else {
        panic!("expected only Step, found {:?}", package.aliases)
    };
    assert_eq!(step.declaration.path, "Step");
    let AliasTarget::Structural(ty) = step.target else {
        panic!("a closed runtime signature is a structural alias, found {:?}", step.target)
    };
    assert_eq!(contract_of(&package, ty), used);
    assert_eq!(package.callable_contracts.len(), 1);
    assert_eq!(spelling(&package, used, &source), SIGNATURE.as_bytes());
    assert_eq!(package.callable_contracts[used.index()].site.span.get_start_offset(), declared(&source, 0));

    // An unused alias still declares its contract at its own signature.
    let unused = alias("Step");
    let package = lower(&unused);
    let [step] = &package.aliases[..] else { panic!("expected only Step") };
    let AliasTarget::Structural(ty) = step.target else {
        panic!("expected a structural alias")
    };
    assert_eq!(spelling(&package, contract_of(&package, ty), &unused), SIGNATURE.as_bytes());
}

#[test]
fn applications_of_one_generic_alias_keep_its_declaration_and_their_own_structure() {
    let signature = "machine(x:T)->T reaches none crashes none";
    let source = format!(
        "data Unary<T> = {signature};
         machine byte(f:Unary<u8>, x:u8)->u8 {{f(x)}}
         machine again(f:Unary<u8>, x:u8)->u8 {{f(x)}}
         machine word(f:Unary<u16>, x:u16)->u16 {{f(x)}}"
    );
    let package = lower(&source);
    let formal = |path: &str| contract_of(&package, machine(&package, path).parameter_types[0]);
    let (narrow, repeated, wide) = (formal("byte"), formal("again"), formal("word"));
    assert_eq!(narrow, repeated, "equal applications are one structural contract");
    assert_ne!(narrow, wide, "distinct applications are distinct structural contracts");
    assert_eq!(package.callable_contracts.len(), 2);
    for (contract, width) in [(narrow, 8), (wide, 16)] {
        let row = &package.callable_contracts[contract.index()];
        assert_eq!(row.parameters, [integer(width)]);
        assert_eq!(row.result, Some(integer(width)));
        assert_eq!(row.site.span.get_start_offset(), source.find(signature).unwrap());
        assert_eq!(spelling(&package, contract, &source), signature.as_bytes());
    }
    for (path, contract) in [("byte", narrow), ("again", narrow), ("word", wide)] {
        let invoker = machine(&package, path);
        let [(callee, _)] = calls(invoker)
            .try_into()
            .unwrap_or_else(|_| panic!("{path} invokes its formal once"));
        assert_eq!(
            *callee,
            CallTarget::Runtime {
                descriptor: scalar_parameter(invoker, 0),
                contract,
            }
        );
    }

    // The alias hands off its own binder; uses substitute it by identity.
    let [unary] = &package.aliases[..] else { panic!("expected only Unary") };
    let AliasTarget::Expansion(expansion) = &unary.target else {
        panic!("a generic alias is an expansion, found {:?}", unary.target)
    };
    assert_eq!(expansion.parameters.len(), 1);
    let DeclarationTerm::Type(TypeExpression(steps)) = &expansion.expansion else {
        panic!("expected a callable type expansion")
    };
    let [TypeExpressionStep::Callable(interface)] = steps.as_slice() else {
        panic!("expected one callable signature, found {steps:?}")
    };
    let binder = TypeExpression(vec![TypeExpressionStep::GenericParameter {
        owner: unary.declaration.clone(),
        index: 0,
    }]);
    assert_eq!(interface.parameters.as_slice(), std::slice::from_ref(&binder));
    assert_eq!(interface.result, Some(binder));
    assert_eq!(interface.site.span.slice(source.as_bytes()), Some(signature.as_bytes()));
}

#[test]
fn forwarding_passes_the_actual_descriptor_refined_into_the_callee_contract() {
    // Both formals use one alias: the descriptor is forwarded unchanged.
    let source = format!(
        "{} machine run(f:Step, value:u8)->u8 {{f(value)}}
         machine wrap(g:Step, value:u8)->u8 {{run((g), value)}}",
        alias("Step")
    );
    let package = lower(&source);
    let (run, wrap) = (machine(&package, "run"), machine(&package, "wrap"));
    assert_eq!(contract_of(&package, run.parameter_types[0]), contract_of(&package, wrap.parameter_types[0]));
    assert!(refinements(wrap).is_empty(), "one alias needs no refinement");
    let [(outer, arguments)] = calls(wrap)
        .try_into()
        .unwrap_or_else(|_| panic!("wrap forwards once"));
    assert_eq!(*outer, CallTarget::Machine(run.id));
    assert_eq!(
        arguments,
        [
            CallArgument::Scalar(scalar_parameter(wrap, 0)),
            CallArgument::Scalar(scalar_parameter(wrap, 1))
        ]
    );

    // Distinct aliases are distinct contracts: only the contract view changes.
    let source = format!(
        "{} {} machine run(f:Step, value:u8)->u8 {{f(value)}}
         machine wrap(g:Relay, value:u8)->u8 {{run((g), value)}}",
        alias("Step"),
        alias("Relay")
    );
    let package = lower(&source);
    let (run, wrap) = (machine(&package, "run"), machine(&package, "wrap"));
    let run_formal = contract_of(&package, run.parameter_types[0]);
    let wrap_formal = contract_of(&package, wrap.parameter_types[0]);
    assert_ne!(run_formal, wrap_formal);
    assert_eq!(run_formal, contract_at(&package, declared(&source, 0)));
    assert_eq!(wrap_formal, contract_at(&package, declared(&source, 1)));

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
    let actual_signature = "machine(actual:u8)->u8 reaches none crashes none";
    let formal_signature = "machine(formal:u8)->u8 reaches none crashes none";
    let source = format!(
        "data Actual = {actual_signature}; data Formal = {formal_signature};
         machine adapt(g:Actual) {{let f:Formal=g;}}"
    );
    let package = lower(&source);
    let adapt = machine(&package, "adapt");
    let actual = contract_of(&package, adapt.parameter_types[0]);
    assert_eq!(spelling(&package, actual, &source), actual_signature.as_bytes());
    let [(descriptor, destination, refined)] = refinements(adapt)
        .try_into()
        .unwrap_or_else(|_| panic!("expected one refinement"));
    assert_eq!(descriptor, scalar_parameter(adapt, 0));
    assert_eq!(spelling(&package, destination, &source), formal_signature.as_bytes());
    assert_eq!(contract_of(&package, adapt.body.values[refined.index()].ty), destination);
    assert!(calls(adapt).is_empty(), "a landing invokes nothing");

    // A landing at the same alias keeps the contract and needs no refinement.
    let same = source.replace("let f:Formal=g;", "let f:Actual=g;");
    let package = lower(&same);
    assert!(refinements(machine(&package, "adapt")).is_empty(), "{same}");
}

#[test]
fn a_join_retains_each_actual_descriptor_and_returns_the_joined_value() {
    let source = format!(
        "{} {} {} {}
         machine choose(flag:bool, left:Left, right:Right)->Answer {{
             let chosen:Chosen = match flag {{false -> left, true -> right,}};
             chosen
         }}",
        alias("Left"),
        alias("Right"),
        alias("Answer"),
        alias("Chosen")
    );
    let package = lower(&source);
    let choose = machine(&package, "choose");
    let (result, local) = (contract_at(&package, declared(&source, 2)), contract_at(&package, declared(&source, 3)));
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

    // Through one alias every arm, the local and the result share a contract.
    let shared = format!(
        "{} machine choose(flag:bool, left:Step, right:Step)->Step {{
             let chosen:Step = match flag {{false -> left, true -> right,}};
             chosen
         }}",
        alias("Step")
    );
    let package = lower(&shared);
    let choose = machine(&package, "choose");
    assert!(refinements(choose).is_empty(), "{shared}");
    assert_eq!(package.callable_contracts.len(), 1);
    assert_eq!(
        contract_of(&package, choose.result_type.unwrap()),
        contract_of(&package, choose.parameter_types[1])
    );
    assert!(calls(choose).is_empty());
}

#[test]
fn the_invocation_after_a_join_uses_the_arriving_descriptor() {
    let source = format!(
        "{} {} {} machine storage(a:[u8;2])->[u8;2]{{a}} machine choose(flag:bool, left:Left, right:Right, n:u8)->u8 {{ let chosen:Chosen = match flag {{false -> left, true -> right,}}; chosen(n) }}",
        alias("Left"),
        alias("Right"),
        alias("Chosen")
    );
    let package = lower(&source);
    let choose = machine(&package, "choose");
    let [(target, arguments)] = calls(choose)
        .try_into()
        .unwrap_or_else(|_| panic!("one runtime invocation"));
    let CallTarget::Runtime { descriptor, contract } = *target else {
        panic!("lost runtime selection")
    };
    assert_eq!(contract, contract_at(&package, declared(&source, 2)));
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
    let omega_lowering::BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &choose.body.blocks[0].terminator
    else {
        panic!("expected flag to select the descriptor");
    };
    assert_eq!(*condition, scalar_parameter(choose, 0));
    assert_eq!(choose.body.blocks[block.get() - 1].scalar_parameters, [descriptor]);
    for (edge, expected) in [
        (when_false, scalar_parameter(choose, 1)),
        (when_true, scalar_parameter(choose, 2)),
    ] {
        let arm = &choose.body.blocks[edge.target.get() - 1];
        let [refinement] = &choose.body.operations[arm.operations.clone()] else {
            panic!("one refinement per arm")
        };
        assert!(
            matches!(&refinement.kind, OperationKind::RefineCallableDescriptor { source, contract: destination }
            if *source == expected && *destination == contract)
        );
        let omega_lowering::BlockTerminator::Jump(arrival) = &arm.terminator else {
            panic!("expected join arrival")
        };
        assert_eq!(arrival.target, block);
        assert!(matches!(arrival.arguments.as_slice(), [selected] if refinement.result == OperationResult::Scalar(*selected)));
    }
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
    let loan = "data Lend = machine(x:@u8)->u8 reaches none crashes none; machine keep(f:Lend) {}";
    let error = sources::lower(&[("runtime_callable.omg", loan)]).unwrap_err();
    assert_eq!(error.message, "no Psi lowering for carrier runtime callable machine(@u8) -> u8");
    assert_eq!(error.site.span.slice(loan.as_bytes()), Some(b"f".as_slice()));
    known_gap::report("runtime callable with a loan formal", "The callable interface needs the formal's access and custody", loan, &error);

    // Constructing a descriptor from a named machine is not forwarding.
    let construction = format!(
        "{} machine action(x:u8)->u8 reaches none crashes none {{x}}
         machine run(f:Step, value:u8)->u8 {{f(value)}}
         machine start(value:u8)->u8 {{run((action), value)}}",
        alias("Step")
    );
    let error = sources::lower(&[("runtime_callable.omg", &construction)]).unwrap_err();
    assert_eq!(
        error.message,
        "forwarded descriptor requires the actual current descriptor; construction from a named machine is unsupported"
    );
    assert_eq!(error.site.span.slice(construction.as_bytes()), Some(b"(action)".as_slice()));
    known_gap::report("construct a runtime descriptor", "The descriptor needs the selected named machine", &construction, &error);
}

#[test]
fn redundant_call_markers_keep_their_operation_or_edge_without_parking() {
    use omega_lowering::{BlockTerminator, CallAcknowledgement, CallSite, PauseEffects};

    for (marker, effects) in [
        ("", PauseEffects::default()),
        ("block ", PauseEffects { block: true, suspend: false }),
        ("suspend ", PauseEffects { block: false, suspend: true }),
    ] {
        for name in ["plain", "f"] {
            let invocation = format!("{marker}{name}()");
            for (terminal, expression) in [
                (false, format!("let x:u8 = {invocation}; x")),
                (true, format!("transition flag {{true -> {invocation}, false -> 0u8,}}")),
            ] {
                let source = format!("data Step=machine()->u8 reaches none crashes none; machine plain()->u8{{1u8}} machine apply(f:Step,flag:bool)->u8 {{{expression}}}");
                let package = lower(&source);
                let owner = machine(&package, "apply");
                let body = &owner.body;
                let target = if name == "plain" {
                    CallTarget::Machine(machine(&package, name).id)
                } else {
                    CallTarget::Runtime { descriptor: scalar_parameter(owner, 0), contract: contract_of(&package, owner.parameter_types[0]) }
                };
                let site = if terminal {
                    body.blocks.iter().find_map(|block| match block.terminator {
                        BlockTerminator::TerminalCall { id, callee, .. } if callee == target => Some(CallSite::Edge(id)),
                        _ => None,
                    })
                } else {
                    body.operations.iter().find_map(|op| match op.kind {
                        OperationKind::Call { callee, .. } if callee == target => Some(CallSite::Operation(op.id)),
                        _ => None,
                    })
                }.expect("the selected target must be called at the requested operation or edge");
                let expected = (effects != PauseEffects::default()).then_some(CallAcknowledgement { call: site, effects });
                assert_eq!(body.call_acknowledgements.as_slice(), expected.as_slice(), "{source}");
                assert_eq!(owner.contract.realized_pause, PauseEffects::default(), "{source}");
                assert!(body.suspension_sites.is_empty(), "{source}");
            }
        }
    }
}
