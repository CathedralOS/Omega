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

fn shared_byte_reference() -> TypeExpression {
    let mut ty = byte();
    ty.0.push(TypeExpressionStep::Reference(omega_lowering::TemplateAccess::Shared));
    ty
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

    // Sharing a contract does not make the two descriptors interchangeable.
    use omega_lowering::{BlockTerminator, ReturnEdge};
    let blocks = &choose.body.blocks;
    let BlockTerminator::Conditional { condition, when_false, when_true } = &blocks[0].terminator else {
        panic!("expected flag to select the descriptor")
    };
    assert_eq!(*condition, scalar_parameter(choose, 0));
    let [from_left, from_right] = [(when_false, 1), (when_true, 2)].map(|(edge, formal)| {
        let BlockTerminator::Jump(arrival) = &blocks[edge.target.get() - 1].terminator else {
            panic!("expected arm to jump to the join")
        };
        assert_eq!(arrival.arguments, [scalar_parameter(choose, formal)], "arm must forward its selected descriptor");
        arrival.target
    });
    assert_eq!(from_left, from_right);
    let join = &blocks[from_left.get() - 1];
    let BlockTerminator::Return { result: ReturnEdge::Scalar(returned), .. } = join.terminator else {
        panic!("expected the join to return a descriptor")
    };
    assert_eq!(join.scalar_parameters, [returned], "return must use the joined descriptor");
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
    let [(target, _)] = calls(choose)
        .try_into()
        .unwrap_or_else(|_| panic!("one runtime invocation"));
    let CallTarget::Runtime { descriptor, contract } = *target else {
        panic!("lost runtime selection")
    };
    assert_eq!(contract, contract_at(&package, declared(&source, 2)));
    let joined = &choose.body.values[descriptor.index()];
    assert!(
        matches!(joined.origin, omega_lowering::ValueOrigin::BlockParameter(_)),
        "invocation selected a branch representative"
    );
    assert_eq!(contract_of(&package, joined.ty), contract);
    // `flag` false reaches the arm refining `left`, true the arm refining `right`; the call reads `n`.
    // `negative_controls.rs` swaps the branch targets and requires this witness to reject them.
    spec_corpus::observe_handoff!(psi "psi-output/runtime-dispatch-arriving-descriptor", assert_eq!(
        psi_handoff_consumer::product_witness::runtime_call_after_branch(&choose.body, 0, 2, 1, &[3]),
        Ok(())
    ));
    assert!(matches!(
        package
            .types
            .get(machine(&package, "storage").parameter_types[0]),
        Some(TypeShape::FixedArray { length: 2, .. })
    ));
}

/// The published reader alias of machine_values.md, followed by a caller that
/// invokes the reader under its own bound on the offset.
const READER: &str = "data ByteReader = machine(offset: u32) -> u8
    requires offset < 256
    reaches none crashes none;

machine open_reader(ready: bool, reader: ByteReader) -> ByteReader
requires ready
{
    reader
}

machine read(reader: ByteReader, offset: u32) -> u8
requires offset < 256
{
    reader(offset)
}";

#[test]
fn a_runtime_contract_publishes_its_precondition_for_every_call_through_it() {
    use omega_lowering::{PredicateOperator, PredicateStep};
    let package = lower(READER);
    let [contract] = &package.callable_contracts[..] else {
        panic!("one alias declares one contract, found {:?}", package.callable_contracts)
    };
    let id = CallableContractId::from_index(0).unwrap();
    // The precondition names the contract's own formal, not a caller's.
    let [bound] = contract.requires.as_slice() else {
        panic!("expected the authored precondition, found {:?}", contract.requires)
    };
    assert_eq!(bound.predicate, [PredicateStep::Formal(0), PredicateStep::Integer(256.into()), PredicateStep::Apply(PredicateOperator::LessThan)]);
    assert_eq!(bound.site.span.slice(READER.as_bytes()), Some(b"requires offset < 256".as_slice()));
    assert!(contract.ensures.is_empty() && contract.crash_routes.is_empty());
    assert_eq!((contract.parameters.as_slice(), &contract.result), ([integer(32)].as_slice(), &Some(byte())));
    assert_eq!((&contract.reach, &contract.crash_ceiling), (&Some(Vec::new()), &Some(Vec::new())));
    assert!(spelling(&package, id, READER).starts_with(b"machine(offset: u32) -> u8"));

    // The alias, the factory's formal and result, and the invoked formal are one contract.
    let [reader] = &package.aliases[..] else { panic!("expected only ByteReader") };
    assert_eq!(reader.declaration.path, "ByteReader");
    let AliasTarget::Structural(ty) = reader.target else {
        panic!("a closed runtime signature is a structural alias, found {:?}", reader.target)
    };
    assert_eq!(contract_of(&package, ty), id);
    let open = machine(&package, "open_reader");
    assert_eq!(contract_of(&package, open.parameter_types[1]), id);
    assert_eq!(contract_of(&package, open.result_type.unwrap()), id);
    let [ready] = open.contract.requires.as_slice() else { panic!("expected the factory's own precondition") };
    assert_eq!(ready.predicate, [PredicateStep::Formal(0)]);
    assert!(refinements(open).is_empty() && calls(open).is_empty(), "returning the formal forwards its descriptor");
    let omega_lowering::BlockTerminator::Return {
        result: omega_lowering::ReturnEdge::Scalar(returned),
        ..
    } = open.body.blocks[0].terminator
    else {
        panic!("expected the factory to return a descriptor")
    };
    assert_eq!(returned, scalar_parameter(open, 1));

    // The invocation names the contract, whose precondition governs its argument.
    let read = machine(&package, "read");
    let [(callee, arguments)] = calls(read)
        .try_into()
        .unwrap_or_else(|c: Vec<_>| panic!("expected one call, found {}", c.len()));
    assert_eq!(*callee, CallTarget::Runtime { descriptor: scalar_parameter(read, 0), contract: id });
    assert_eq!(arguments, [CallArgument::Scalar(scalar_parameter(read, 1))]);

    // Without the clause the same contract publishes no precondition and nothing else changes.
    let open_contract = READER.replacen("\n    requires offset < 256", "", 1);
    let twin = lower(&open_contract);
    let [unbounded] = &twin.callable_contracts[..] else { panic!("expected one contract") };
    assert!(unbounded.requires.is_empty());
    assert_eq!(
        (&unbounded.parameters, &unbounded.result, &unbounded.ensures, &unbounded.reach, &unbounded.crash_ceiling, &unbounded.crash_routes),
        (&contract.parameters, &contract.result, &contract.ensures, &contract.reach, &contract.crash_ceiling, &contract.crash_routes)
    );
    assert_eq!(calls(machine(&twin, "read")), calls(read));
}

#[test]
fn loan_formal_is_part_of_the_descriptor_contract_not_a_loan_of_the_descriptor() {
    let loan = "data Lend = machine(x:@u8)->u8 reaches none crashes none; machine keep(f:Lend) {}";
    let scalar = loan.replace("x:@u8", "x:u8");
    let parameter = |source: &str| {
        let package = lower(source);
        let keep = machine(&package, "keep");
        let descriptor = scalar_parameter(keep, 0);
        assert_eq!(keep.body.values[descriptor.index()].ty, keep.parameter_types[0]);
        let contract = contract_of(&package, keep.parameter_types[0]);
        let [alias] = package.aliases.as_slice() else { panic!("one callable alias") };
        assert_eq!(alias.declaration.path, "Lend");
        assert_eq!(alias.target, AliasTarget::Structural(keep.parameter_types[0]));
        let signature = &source["data Lend = ".len()..source.find(';').unwrap()];
        assert_eq!(spelling(&package, contract, source), signature.as_bytes());
        let row = &package.callable_contracts[contract.index()];
        assert_eq!(row.result, Some(byte()));
        assert!(calls(keep).is_empty());
        row.parameters.clone()
    };
    spec_corpus::observe_handoff!(psi "psi-output/loan-callable-contract",
        assert_eq!((parameter(loan), parameter(&scalar)), (vec![shared_byte_reference()], vec![byte()])));
}

#[test]
fn runtime_calls_reborrow_the_selected_referent_in_its_declared_slot() {
    use omega_lowering::{BlockTerminator, PlaceProjection, ProjectedPlace, ReferenceAccess};
    let source = "data Use = machine(before:u8, x:@u8, after:u8)->u8 reaches none crashes none; machine apply(f:Use, g:Use, left:@u8, right:@u8, a:u8, b:u8, flag:bool)->u8 {let n:u8=f(a,right,b); transition flag {true -> g(b,left,n), false -> n,}}";
    let package = lower(source);
    let apply = machine(&package, "apply");
    let contract = contract_at(&package, source.find("machine(").unwrap());
    assert_eq!(package.callable_contracts[contract.index()].parameters, [byte(), shared_byte_reference(), byte()]);
    let target = |slot| CallTarget::Runtime { descriptor: scalar_parameter(apply, slot), contract };
    let scalar = |slot| CallArgument::Scalar(scalar_parameter(apply, slot));
    let reborrow = |slot: usize| {
        let Parameter::Structural(root) = apply.body.parameters[slot] else { panic!("reference formal") };
        CallArgument::Borrow { place: ProjectedPlace { root, path: vec![PlaceProjection::Referent] }, access: ReferenceAccess::Shared }
    };
    let [(callee, arguments)]: [_; 1] = calls(apply).try_into().expect("one ordinary invocation");
    spec_corpus::observe_handoff!(psi "psi-output/runtime-loan-actual",
        assert_eq!((*callee, arguments), (target(0), [scalar(4), reborrow(3), scalar(5)].as_slice())));
    let n = apply.body.operations.iter().find_map(|op| match (&op.kind, op.result) {
        (OperationKind::Call { .. }, OperationResult::Scalar(result)) => Some(result),
        _ => None,
    }).unwrap();
    let edge = apply.body.blocks.iter().find_map(|block| match &block.terminator {
        BlockTerminator::TerminalCall { callee, arguments, .. } => Some((*callee, arguments.as_slice())),
        _ => None,
    }).expect("terminal invocation");
    spec_corpus::observe_handoff!(psi "psi-output/runtime-loan-edge",
        assert_eq!(edge, (target(1), [scalar(5), reborrow(2), CallArgument::Scalar(n)].as_slice())));
}

#[test]
fn a_named_machine_constructs_the_descriptor_delivered_to_the_callee() {
    // The original grouped operand, then the same selection with a competing
    // machine present, then change only the selected name.
    let construction = format!(
        "{} machine action(x:u8)->u8 reaches none crashes none {{x}}
         machine run(f:Step, value:u8)->u8 {{f(value)}}
         machine start(value:u8)->u8 {{run((action), value)}}",
        alias("Step")
    );
    let competing = format!("{} {construction} machine second(x:u8)->u8 reaches none crashes none {{0u8}}", alias("Decoy"));
    let changed = competing.replace("(action)", "(second)");
    for (source, selected, step) in [(&construction, "action", 0), (&competing, "action", 1), (&changed, "second", 1)] {
        let files = [("runtime_callable.omg", source.as_str())];
        let (package, refinement_site) = {
            let resolved = sources::resolve(&files);
            let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
            let typed = prepared.typed();
            let call = typed.calls.iter().find(|c| c.machine.path == "run").unwrap();
            let Some(omega_typed::InvocationTarget::Declaration(slot)) = &call.operands[0].callable else {
                panic!("expected a selected declaration for the first argument")
            };
            let mut constructions = typed.obligations.iter().filter_map(|o| match &o.kind {
                omega_typed::ObligationKind::RefineCallable { formal: None, selection, contract } => Some((selection, contract, &o.site)),
                _ => None,
            });
            let (selection, bound, site) = constructions.next().unwrap();
            assert!(constructions.next().is_none());
            assert_eq!((&slot.declaration, slot.declaration.path.as_str()), (selection.machine.as_ref(), selected));
            assert!(selection.arguments.is_empty() && selection.enclosing_arguments.is_empty() && slot.trait_arguments.is_empty());
            assert_eq!(bound.contract.span.get_start_offset(), declared(source, step));
            assert_eq!(bound.contract.span.slice(source.as_bytes()), Some(SIGNATURE.as_bytes()));
            (sources::lower_prepared(&prepared, &files).unwrap(), site.clone())
        };
        let (start, run) = (machine(&package, "start"), machine(&package, "run"));
        let [make, call] = start.body.operations.as_slice() else { panic!("expected construction then direct call") };
        let OperationResult::Scalar(descriptor) = make.result else { panic!("not a scalar descriptor") };
        let contract = contract_at(&package, declared(source, step));
        assert_eq!(contract_of(&package, start.body.values[descriptor.index()].ty), contract);
        assert_eq!(start.body.diagnostics.operations[make.id.index()], refinement_site);
        let occurrence = refinement_site.span;
        assert_eq!(occurrence.get_start_offset(), source.find(&format!("({selected})")).unwrap() + 1);
        assert_eq!(occurrence.slice(source.as_bytes()), Some(selected.as_bytes()));
        spec_corpus::observe_handoff!(psi "psi-output/named-descriptor-selection", assert_eq!(
            make.kind, OperationKind::ConstructCallableDescriptor { machine: machine(&package, selected).id, contract }
        ));
        let OperationKind::Call { callee, arguments, .. } = &call.kind else { panic!("expected direct call") };
        spec_corpus::observe_handoff!(psi "psi-output/named-descriptor-delivery", assert_eq!(
            (callee, arguments.as_slice()),
            (&CallTarget::Machine(run.id), [CallArgument::Scalar(descriptor), CallArgument::Scalar(scalar_parameter(start, 0))].as_slice())
        ));
        let runtime = CallTarget::Runtime { descriptor: scalar_parameter(run, 0), contract };
        assert_eq!(calls(run), [(&runtime, [CallArgument::Scalar(scalar_parameter(run, 1))].as_slice())]);
    }
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

/// The probe `runtime callable contract with a guarantee`, byte for byte: the
/// reader alias also promises `result < 200`.
const GUARANTEED: &str = include_str!("fixtures/package_cases/runtime-callable-contract-guarantee/case/case.omg");

/// The single runtime call operation of `machine`, with its index.
fn runtime_call(machine: &LoweredMachine) -> (usize, &omega_lowering::PsiOperation) {
    let [found] = machine
        .body
        .operations
        .iter()
        .enumerate()
        .filter(|(_, op)| {
            matches!(
                op.kind,
                OperationKind::Call {
                    callee: CallTarget::Runtime { .. },
                    ..
                }
            )
        })
        .collect::<Vec<_>>()[..]
    else {
        panic!("expected one runtime call in {}", machine.declaration.path)
    };
    found
}

#[test]
fn a_runtime_contract_publishes_its_guarantee_and_each_call_owes_its_precondition() {
    use omega_lowering::{CallRequirement, CallSite, PredicateOperator, PredicateStep};
    use psi_handoff_consumer::product_witness::{CallTerm, runtime_call_contract};
    let package = lower(GUARANTEED);
    let [contract] = &package.callable_contracts[..] else {
        panic!("one alias declares one contract, found {:?}", package.callable_contracts)
    };
    let id = CallableContractId::from_index(0).unwrap();
    let less = |left: PredicateStep, bound: u32| {
        vec![
            left,
            PredicateStep::Integer(bound.into()),
            PredicateStep::Apply(PredicateOperator::LessThan),
        ]
    };
    let clauses = |clauses: &[omega_lowering::ContractClause]| {
        clauses
            .iter()
            .map(|c| (c.predicate.clone(), c.site.span.slice(GUARANTEED.as_bytes()).map(<[u8]>::to_vec)))
            .collect::<Vec<_>>()
    };
    // The guarantee names the contract's own result, beside the precondition over its formal.
    spec_corpus::observe_handoff!(psi "psi-output/runtime-contract-guarantee", assert_eq!(
        (clauses(&contract.requires), clauses(&contract.ensures)),
        (vec![(less(PredicateStep::Formal(0), 256), Some(b"requires offset < 256".to_vec()))],
         vec![(less(PredicateStep::Result, 200), Some(b"ensures result < 200".to_vec()))])
    ));
    assert_eq!((contract.parameters.as_slice(), &contract.result), ([integer(32)].as_slice(), &Some(byte())));
    assert_eq!(
        (&contract.reach, &contract.crash_ceiling, contract.crash_routes.len()),
        (&Some(Vec::new()), &Some(Vec::new()), 0)
    );
    let [alias] = &package.aliases[..] else { panic!("expected only ByteReader") };
    let AliasTarget::Structural(ty) = alias.target else {
        panic!(
            "a closed runtime signature with a guarantee is still a structural alias, found {:?}",
            alias.target
        )
    };
    assert_eq!(contract_of(&package, ty), id);

    // The call names the formal's descriptor and that contract, and owes the
    // contract's one precondition at its own position.
    let read = machine(&package, "read");
    assert_eq!(contract_of(&package, read.parameter_types[0]), id);
    let (index, call) = runtime_call(read);
    let descriptor = scalar_parameter(read, 0);
    let offset = scalar_parameter(read, 1);
    spec_corpus::observe_handoff!(psi "psi-output/runtime-call-debt", assert_eq!(
        call.kind,
        OperationKind::Call {
            callee: CallTarget::Runtime { descriptor, contract: id },
            arguments: vec![CallArgument::Scalar(offset)],
            requirements: vec![CallRequirement { site: CallSite::Operation(call.id), clause: 0 }],
        }
    ));
    let OperationResult::Scalar(result) = call.result else {
        panic!("expected the u8 result")
    };
    let omega_lowering::BlockTerminator::Return {
        result: omega_lowering::ReturnEdge::Scalar(returned),
        ..
    } = read.body.blocks[0].terminator
    else {
        panic!("expected read to return the call result")
    };
    assert_eq!(returned, result);

    // A Psi-only reader instantiates the debt on the caller's offset and the
    // guarantee on this call's result.
    let instantiated = runtime_call_contract(&package, &read.body, index).expect("the published call and contract agree");
    let term = |n: u32| CallTerm::Step(PredicateStep::Integer(n.into()));
    let lt = CallTerm::Step(PredicateStep::Apply(PredicateOperator::LessThan));
    spec_corpus::observe_handoff!(psi "psi-output/runtime-call-guarantee", assert_eq!(
        (instantiated.contract, instantiated.debts, instantiated.guarantees),
        (id, vec![vec![CallTerm::Argument(offset), term(256), lt.clone()]], vec![vec![CallTerm::Result(result), term(200), lt]])
    ));

    // Changing only the bound changes only the guarantee; removing the clause
    // leaves the twin's contract and call otherwise identical.
    for (variant, ensures) in [
        (GUARANTEED.replacen("result < 200", "result < 100", 1), vec![less(PredicateStep::Result, 100)]),
        (GUARANTEED.replacen("\n    ensures result < 200", "", 1), Vec::new()),
    ] {
        let twin = lower(&variant);
        let [other] = &twin.callable_contracts[..] else {
            panic!("expected one contract")
        };
        assert_eq!(
            other
                .ensures
                .iter()
                .map(|c| c.predicate.clone())
                .collect::<Vec<_>>(),
            ensures,
            "{variant}"
        );
        assert_eq!(
            (
                &other.parameters,
                &other.result,
                other.requires.iter().map(|c| &c.predicate).collect::<Vec<_>>(),
                &other.reach,
                &other.crash_ceiling,
                &other.crash_routes
            ),
            (
                &contract.parameters,
                &contract.result,
                contract
                    .requires
                    .iter()
                    .map(|c| &c.predicate)
                    .collect::<Vec<_>>(),
                &contract.reach,
                &contract.crash_ceiling,
                &contract.crash_routes
            ),
            "{variant}"
        );
        let twin_read = machine(&twin, "read");
        assert_eq!(
            (&twin_read.body.operations, &twin_read.body.blocks),
            (&read.body.operations, &read.body.blocks),
            "{variant}"
        );
    }
}

/// Construction, refinement and a join all pass the same contract gate. Each
/// keeps its existing operation; whether the machine or source contract
/// supplies `result < 200` is the checker's refinement question, so lowering
/// publishes `narrow` as well as `widen`.
#[test]
fn guarantee_contracts_keep_construction_refinement_and_join_for_the_checker() {
    let reader = GUARANTEED.split("\n\nmachine read").next().unwrap();
    let construct = format!(
        "{GUARANTEED}\n\nmachine low(offset: u32) -> u8\nrequires offset < 256\nensures result < 200\n{{\n    7\n}}\n\nmachine use_low(offset: u32) -> u8\nrequires offset < 256\n{{\n    read(low, offset)\n}}\n"
    );
    let package = lower(&construct);
    let id = CallableContractId::from_index(0).unwrap();
    let use_low = machine(&package, "use_low");
    let (made, descriptor) = use_low
        .body
        .operations
        .iter()
        .find_map(|op| match (&op.kind, op.result) {
            (OperationKind::ConstructCallableDescriptor { .. }, OperationResult::Scalar(value)) => Some((op.kind.clone(), value)),
            _ => None,
        })
        .expect("a constructed descriptor");
    spec_corpus::observe_handoff!(psi "psi-output/runtime-guarantee-construction", assert_eq!(
        made,
        OperationKind::ConstructCallableDescriptor { machine: machine(&package, "low").id, contract: id }
    ));
    let read = machine(&package, "read").id;
    assert_eq!(
        calls(use_low),
        [(
            &CallTarget::Machine(read),
            [
                CallArgument::Scalar(descriptor),
                CallArgument::Scalar(scalar_parameter(use_low, 0))
            ]
            .as_slice()
        )]
    );

    let plain = "data PlainReader = machine(offset: u32) -> u8\n    requires offset < 256\n    reaches none crashes none;";
    let refine = format!(
        "{reader}\n\n{plain}\n\nmachine widen(reader: ByteReader) -> PlainReader\n{{\n    reader\n}}\n\nmachine narrow(reader: PlainReader) -> ByteReader\n{{\n    reader\n}}\n"
    );
    let package = lower(&refine);
    let contract = |name: &str| {
        let Some(AliasTarget::Structural(ty)) = package
            .aliases
            .iter()
            .find(|a| a.declaration.path == name)
            .map(|a| &a.target)
        else {
            panic!("expected structural {name}")
        };
        contract_of(&package, *ty)
    };
    let (guaranteed, unguaranteed) = (contract("ByteReader"), contract("PlainReader"));
    assert!(
        !package.callable_contracts[guaranteed.index()]
            .ensures
            .is_empty()
            && package.callable_contracts[unguaranteed.index()]
                .ensures
                .is_empty()
    );
    let observed = ["widen", "narrow"].map(|name| {
        let m = machine(&package, name);
        refinements(m)
            .into_iter()
            .map(|(source, to, _)| (source == scalar_parameter(m, 0), to))
            .collect::<Vec<_>>()
    });
    spec_corpus::observe_handoff!(psi "psi-output/runtime-guarantee-refinement", assert_eq!(
        observed,
        [vec![(true, unguaranteed)], vec![(true, guaranteed)]]
    ));

    let joined = format!(
        "{reader}\n\nmachine choose(flag: bool, left: ByteReader, right: ByteReader, offset: u32) -> u8\nrequires offset < 256\n{{\n    let chosen = match flag {{true -> left, false -> right,}};\n    chosen(offset)\n}}\n"
    );
    let package = lower(&joined);
    let choose = machine(&package, "choose");
    let (_, call) = runtime_call(choose);
    let OperationKind::Call {
        callee: CallTarget::Runtime { descriptor, contract },
        requirements,
        ..
    } = &call.kind
    else {
        unreachable!()
    };
    assert_eq!((*contract, requirements.len()), (id, 1));
    assert!(
        matches!(choose.body.values[descriptor.index()].origin, omega_lowering::ValueOrigin::BlockParameter(_)),
        "the joined descriptor arrives at the call"
    );
}
