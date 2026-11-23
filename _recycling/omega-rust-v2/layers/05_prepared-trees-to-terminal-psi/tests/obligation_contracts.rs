//! Typed obligation input to Psi output for qualification, callable selection
//! and callback guarantee obligations.
//!
//! Each test lowers one source package through the shared obligation pipeline,
//! locates every obligation of its kind by source site in the prepared trees,
//! then reads the Psi record retained at that same site after the prepared
//! trees are dropped. Twins in the same package change one authored choice, and
//! the expected Psi is written from the source. L6 judges the obligation.
#[allow(dead_code)]
#[path = "obligation_outputs/pipeline.rs"]
mod pipeline;
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    BlockTerminator, CallArgument, CallTarget, CallableContractId, LoweredMachine, LoweredPackage, MachineId, MachineTemplateBinder, OperationKind,
    OperationResult, Parameter, PredicateStep, QualificationDirection, ScalarQualificationChange, Site, StaticArgument, TypeId, TypeShape,
};
use omega_typed::{ObligationKind, TypingObligation, ValueType};
use pipeline::{Lowered, operation_at};

/// Receipt for the L5 readiness report, printed after the checks above it pass.
fn observed(label: &str) {
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        println!("OMEGA_L5_ASSERT\t{label}");
    }
}

/// The one obligation of `kind` whose site is spelled `text`.
fn one_at<'a>(
    lowered: &'a Lowered<'_>,
    kind: &str,
    text: &str,
) -> &'a TypingObligation {
    let found: Vec<_> = lowered
        .obligations(kind)
        .into_iter()
        .filter(|o| lowered.spelling(&o.site) == text)
        .collect();
    let [obligation] = found[..] else {
        panic!("expected one {kind} at `{text}`, found {found:?}")
    };
    obligation
}

fn by_id(
    package: &LoweredPackage,
    id: MachineId,
) -> &LoweredMachine {
    package
        .machines
        .iter()
        .find(|m| m.id == id)
        .expect("call target is packaged")
}

fn within(
    inner: &Site,
    outer: &Site,
) -> bool {
    inner.file == outer.file && outer.span.get_start_offset() <= inner.span.get_start_offset() && inner.span.get_end_offset() <= outer.span.get_end_offset()
}

/// A cast that adds a predicate-free tag carries one `EstablishDomain` naming
/// that domain at the cast. The Psi edge retained at that site forwards the
/// cast operand with an `Add` change, and the destination parameter carries
/// exactly that domain. Adding the other tag names the other domain. An erasing
/// cast owes no establishment and its edge is an `Erase`.
#[test]
fn establish_domain_adds_the_named_qualification_on_the_cast_edge() {
    use QualificationDirection::{Add, Erase};
    let source = "domain u8::A; domain u8::B;
                  machine first(x:u8)->u8 in A { x as +A }
                  machine second(x:u8)->u8 in B { x as +B }
                  machine erase(x:u8 in A)->u8 { x as -A }";
    let lowered = pipeline::lower(source);
    let package = &lowered.package;
    let established = lowered.obligations("EstablishDomain");
    assert_eq!(established.len(), 2, "only adding casts establish a domain: {established:?}");
    for (name, cast, domain, direction, delivered) in [
        ("first", "x as +A", Some("u8::A"), Add, &["u8::A"][..]),
        ("second", "x as +B", Some("u8::B"), Add, &["u8::B"][..]),
        ("erase", "x as -A", None, Erase, &[][..]),
    ] {
        let body = &lowered.machine(name).body;
        let [Parameter::Scalar(x)] = body.parameters[..] else {
            panic!("{name}: expected one scalar formal")
        };
        let BlockTerminator::Jump(edge) = &body.blocks[0].terminator else {
            panic!("{name}: expected the cast edge")
        };
        let site = &body.diagnostics.edges[edge.id.get() - 1];
        assert_eq!(lowered.spelling(site), cast);
        let here: Vec<_> = established.iter().filter(|o| &o.site == site).collect();
        match domain {
            Some(path) => {
                let [obligation] = here[..] else {
                    panic!("{cast}: expected one establishment at the edge, found {here:?}")
                };
                let ObligationKind::EstablishDomain(application) = &obligation.kind else {
                    unreachable!()
                };
                assert_eq!((application.declaration.path.as_str(), application.arguments.len()), (path, 0), "{cast}");
            }
            None => assert!(here.is_empty(), "{cast}: erasing establishes nothing"),
        }
        assert_eq!(edge.arguments, [x], "{cast}");
        assert_eq!(
            edge.scalar_qualification_changes,
            [ScalarQualificationChange { argument_index: 0, direction }],
            "{cast}"
        );
        let [destination] = body.blocks[edge.target.get() - 1].scalar_parameters[..] else {
            panic!("{cast}: expected one fresh parameter")
        };
        let domains: Vec<_> = match package.types.get(body.values[destination.index()].ty) {
            Some(TypeShape::QualifiedScalar { domains, .. }) => package
                .types
                .domain_set(*domains)
                .unwrap()
                .iter()
                .map(|id| id.path.as_str())
                .collect(),
            Some(TypeShape::PrimitiveScalar(_)) => Vec::new(),
            other => panic!("{cast}: unexpected destination {other:?}"),
        };
        assert_eq!(domains, delivered, "{cast}");
    }
    observed("obligation-output/establish-domain");
}

/// A closed application of a callback wrapper carries one `RefineCallable`
/// naming the wrapper's binder, the bound contract and the selected machine at
/// the call. The Psi call at that site targets an instance whose static argument
/// is that machine with that contract, and the instance invokes exactly that
/// machine. Selecting the other machine changes the argument and the instance.
#[test]
fn refine_callable_binds_the_selected_machine_as_the_instance_static_argument() {
    let source = "data ByteStep=machine(x:u8)->u8;
                  machine one(x:u8)->u8{x} machine two(x:u8)->u8{x}
                  machine apply<const F:ByteStep>(v:u8)->u8{F(v)}
                  machine first(a:u8)->u8{apply::<one>(a)}
                  machine second(a:u8)->u8{apply::<two>(a)}";
    let lowered = pipeline::lower(source);
    let package = &lowered.package;
    assert_eq!(lowered.obligations("RefineCallable").len(), 2, "one refinement per closed application");
    let mut instances = Vec::new();
    for (caller, call, selected) in [
        ("first", "apply::<one>(a)", "one"),
        ("second", "apply::<two>(a)", "two"),
    ] {
        let obligation = one_at(&lowered, "RefineCallable", call);
        let ObligationKind::RefineCallable {
            formal: Some((owner, slot)),
            contract,
            selection,
        } = &obligation.kind
        else {
            panic!("{call}: expected a static binder refinement, found {:?}", obligation.kind)
        };
        assert_eq!((owner.path.as_str(), *slot), ("apply", 0), "{call}");
        assert_eq!(lowered.spelling(&contract.contract), "machine(x:u8)->u8", "{call}");
        assert_eq!(selection.machine.path, selected, "{call}");
        assert!(selection.arguments.is_empty() && selection.enclosing_arguments.is_empty(), "{call}");

        let body = &lowered.machine(caller).body;
        let [Parameter::Scalar(a)] = body.parameters[..] else {
            panic!("{caller}: expected one scalar formal")
        };
        let OperationKind::Call {
            callee: CallTarget::Machine(id),
            arguments,
            ..
        } = &operation_at(body, &obligation.site).kind
        else {
            panic!("{call}: expected a direct call of the instance")
        };
        assert_eq!(*arguments, [CallArgument::Scalar(a)], "{call}");
        let instance = by_id(package, *id);
        let chosen = lowered.machine(selected);
        assert_eq!(instance.declaration.path, "apply");
        let [StaticArgument::Callable(binding)] = instance.arguments.as_slice() else {
            panic!("{call}: expected one static callable argument, found {:?}", instance.arguments)
        };
        assert_eq!(binding.machine, chosen.id, "{call}");
        assert_eq!(binding.site, contract.contract, "{call}");
        assert_eq!((&binding.parameters, binding.result), (&chosen.parameter_types, chosen.result_type), "{call}");
        let [Parameter::Scalar(v)] = instance.body.parameters[..] else {
            panic!("{call}: expected the instance formal")
        };
        let [inside] = instance.body.operations.as_slice() else {
            panic!("{call}: expected the direct callback invocation")
        };
        assert!(
            matches!(&inside.kind, OperationKind::Call { callee: CallTarget::Machine(target), arguments, .. }
                if *target == chosen.id && *arguments == [CallArgument::Scalar(v)]),
            "{call}: {:?}",
            inside.kind
        );
        instances.push(instance.id);
    }
    assert_ne!(instances[0], instances[1]);
    observed("obligation-output/refine-callable");
}

/// Landing a descriptor in a different structural contract carries one
/// `RefineCallableValue` from the actual to the destination contract at the
/// landed value. The Psi operation at that site refines the same incoming
/// descriptor into the destination contract, whose declaration is that site.
/// Another destination names the other contract. Landing at the actual
/// contract owes nothing and emits no refinement.
#[test]
fn refine_callable_value_views_the_same_descriptor_through_the_destination_contract() {
    let actual = "machine(actual:u8)->u8 reaches none crashes none";
    let formal = "machine(formal:u8)->u8 reaches none crashes none";
    let other = "machine(other:u8)->u8 reaches none crashes none";
    let source = format!(
        "data Actual={actual}; data Formal={formal}; data Other={other};
         machine to_formal(g:Actual) {{let f:Formal=g;}}
         machine to_other(g:Actual) {{let f:Other=g;}}
         machine keep(g:Actual) {{let f:Actual=g;}}"
    );
    let lowered = pipeline::lower(&source);
    let package = &lowered.package;
    let refinements = lowered.obligations("RefineCallableValue");
    assert_eq!(refinements.len(), 2, "only a changed contract is refined: {refinements:?}");
    let contract_of = |ty: TypeId| match package.types.get(ty) {
        Some(TypeShape::Callable(contract)) => *contract,
        other => panic!("expected a callable carrier, found {other:?}"),
    };
    let declared = |contract: CallableContractId| package.callable_contracts[contract.index()].site.clone();
    for (name, destination) in [
        ("to_formal", Some(formal)),
        ("to_other", Some(other)),
        ("keep", None),
    ] {
        let body = &lowered.machine(name).body;
        let [Parameter::Scalar(g)] = body.parameters[..] else {
            panic!("{name}: expected one descriptor formal")
        };
        let here: Vec<_> = refinements
            .iter()
            .filter(|o| within(&o.site, &body.diagnostics.body))
            .collect();
        let emitted = body
            .operations
            .iter()
            .filter(|op| matches!(op.kind, OperationKind::RefineCallableDescriptor { .. }))
            .count();
        let Some(destination) = destination else {
            assert!(here.is_empty() && emitted == 0, "{name}: an unchanged contract needs no refinement");
            continue;
        };
        let [obligation] = here[..] else {
            panic!("{name}: expected one refinement, found {here:?}")
        };
        let ObligationKind::RefineCallableValue { source: from, destination: to } = &obligation.kind else {
            unreachable!()
        };
        assert_eq!(lowered.spelling(&obligation.site), "g");
        assert_eq!((lowered.spelling(from), lowered.spelling(to)), (actual, destination), "{name}");
        let operation = operation_at(body, &obligation.site);
        let OperationKind::RefineCallableDescriptor { source: descriptor, contract } = operation.kind else {
            panic!("{name}: expected a descriptor refinement, found {:?}", operation.kind)
        };
        assert_eq!(emitted, 1, "{name}");
        assert_eq!(descriptor, g, "{name}: the incoming descriptor itself is refined");
        assert_eq!(declared(contract_of(body.values[g.index()].ty)), *from, "{name}");
        assert_eq!(declared(contract), *to, "{name}");
        let OperationResult::Scalar(refined) = operation.result else {
            panic!("{name}: a refinement yields a descriptor")
        };
        assert_eq!(contract_of(body.values[refined.index()].ty), contract, "{name}");
    }
    observed("obligation-output/refine-callable-value");
}

/// `F::ensures(...)` in a wrapper contract carries one `ApplyCallbackGuarantee`
/// naming the static binder, its contract and the ordered subject sites. The
/// open wrapper's Psi guarantee at that clause applies binder 0 to the same
/// subjects in the same order, as formals or the result. Twins select the other
/// formal or swap the subjects.
#[test]
fn apply_callback_guarantee_projects_the_binder_guarantee_onto_the_authored_subjects() {
    use PredicateStep as Step;
    let callback = "machine(x:u8)->u8 ensures result==x";
    let source = format!(
        "data Open={callback};
         machine forward<const F:Open>(x:u8,y:u8)->u8 ensures F::ensures(x,result) {{F(x)}}
         machine second<const F:Open>(x:u8,y:u8)->u8 ensures F::ensures(y,result) {{F(y)}}
         machine swap<const F:Open>(x:u8,y:u8)->u8 ensures F::ensures(result,x) {{F(x)}}"
    );
    let lowered = pipeline::lower(&source);
    let package = &lowered.package;
    assert_eq!(lowered.obligations("ApplyCallbackGuarantee").len(), 3, "one projection per wrapper");
    for (name, projection, subjects, steps) in [
        ("forward", "F::ensures(x,result)", ["x", "result"], [Step::Formal(0), Step::Result]),
        ("second", "F::ensures(y,result)", ["y", "result"], [Step::Formal(1), Step::Result]),
        ("swap", "F::ensures(result,x)", ["result", "x"], [Step::Result, Step::Formal(0)]),
    ] {
        let obligation = one_at(&lowered, "ApplyCallbackGuarantee", projection);
        let ObligationKind::ApplyCallbackGuarantee { binder, contract, operands } = &obligation.kind else {
            unreachable!()
        };
        assert!(
            matches!(binder, ValueType::Parameter { owner, index: 0 } if owner.path == name),
            "{projection}: {binder:?}"
        );
        assert_eq!(lowered.spelling(contract), callback, "{projection}");
        assert_eq!(operands.iter().map(|s| lowered.spelling(s)).collect::<Vec<_>>(), subjects, "{projection}");

        let [template] = package
            .machine_templates
            .iter()
            .filter(|t| t.declaration.path == name)
            .collect::<Vec<_>>()[..]
        else {
            panic!("{name}: expected one open wrapper")
        };
        let [MachineTemplateBinder::Callable { requirement: None, signature }] = template.parameters.as_slice() else {
            panic!("{name}: expected one structural callable binder")
        };
        assert_eq!(signature.site, *contract, "{projection}: the binder keeps the projected contract");
        let [clause] = template.signature.ensures.as_slice() else {
            panic!("{name}: expected one guarantee")
        };
        assert!(within(&obligation.site, &clause.site), "{projection}: the clause owns the projection");
        let mut expected = Vec::from(steps);
        expected.push(Step::CallbackGuarantee {
            binder: 0,
            operand_count: subjects.len(),
        });
        assert_eq!(clause.predicate, expected, "{projection}");
    }
    observed("obligation-output/apply-callback-guarantee");
}
