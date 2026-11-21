//! Compare each L4 product with what L5 returns for that same package. The nested
//! helpers keep their enclosing binder and both frozen returns, and the specialized
//! field readers return concrete arrays.
#[path = "support/returns.rs"]
mod returns;
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_preparation::{PreparedValue, prepare_source_package};
use omega_typer::{StaticValue, ValueType};

/// L4 CONTRACT "A local declaration after a tail expression or transition": a nested
/// helper receives its enclosing static argument when called directly and as a callback.
/// The frozen constants are what a Psi reader of `FIRST` and `SECOND` would consume.
#[test]
fn nested_helpers_preserve_the_enclosing_binder_and_both_frozen_returns() {
    use omega_lowering::{MachineTemplateBinder, OperationKind, OperationResult, ReturnEdge, ScalarLiteral, ScalarType,
        TemplateCallTarget, TemplateResult, TypeExpression, TypeExpressionStep, ValueOrigin};
    for (entry, twin) in [("helper(input)", false), ("invoke::<helper>(input)", false), ("invoke::<helper>(input)", true)] {
        let outer = format!(
            "machine outer<const FLAG:bool>(input:bool)->bool {{
                 {entry}
                 machine helper(value:bool)->bool {{FLAG}}
             }}"
        );
        let source = format!(
            "data BoolStep=machine(input:bool)->bool;
             machine invoke<const K:BoolStep>(input:bool)->bool {{K(input)}}
             {outer}
             const FIRST:bool=outer::<true>(false);
             const SECOND:bool=outer::<false>(true);
             machine first()->bool{{FIRST}}
             machine second()->bool{{SECOND}}"
        );
        // Move FLAG to the second slot and exchange its values. Same carriers, paths and operands.
        let source = if twin { source.replace("<const FLAG", "<const OTHER:bool,const FLAG")
            .replace("outer::<true>", "outer::<true,false>").replace("outer::<false>", "outer::<false,true>") } else { source };
        let package = sources::resolve(&[("helper.omg", &source)]);
        let prepared = prepare_source_package(&package).unwrap_or_else(|error| panic!("{entry}: {error:?}"));
        let frozen: Vec<_> = ["FIRST", "SECOND"]
            .into_iter()
            .map(|name| {
                let constant = prepared
                    .typed()
                    .constants
                    .iter()
                    .find(|constant| constant.declaration.path == name)
                    .unwrap();
                prepared.constant(&constant.declaration).cloned()
            })
            .collect();
        // The value comes from the enclosing argument, not from the runtime operand.
        assert_eq!(
            frozen,
            [
                Some(PreparedValue::Bool(!twin)),
                Some(PreparedValue::Bool(twin))
            ],
            "{entry}"
        );

        let psi = sources::lower_prepared(&prepared, &[("helper.omg", &source)]).unwrap();
        drop(prepared);
        let values = ["first", "second"].map(|name| {
            let body = &psi.machines.iter().find(|m| m.declaration.path == name).unwrap().body;
            let ReturnEdge::Scalar(value) = returns::sole_return(body) else { panic!("{name} has no scalar return") };
            let ValueOrigin::Operation(id) = body.values[value.index()].origin else { panic!("missing literal") };
            let op = &body.operations[id.index()];
            assert_eq!(op.result, OperationResult::Scalar(value));
            let OperationKind::Constant(ScalarLiteral::Boolean(b)) = op.kind else { panic!("expected Boolean") };
            b
        });
        spec_corpus::observe_handoff!("nested-helpers-and-tail-expressions/frozen-returns", assert_eq!(values, [!twin, twin]));
        let template = |path: &str| psi.machine_templates.iter().find(|m| m.declaration.path == path).unwrap();
        let (outer, helper, invoke) = (template("outer"), template("outer::helper"), template("invoke"));
        assert_eq!(helper.declaration.enclosing_machine.as_ref(), Some(&outer.declaration));
        assert_eq!(outer.parameters.len(), usize::from(twin) + 1);
        assert!(outer.parameters.iter().all(|p| matches!(p, MachineTemplateBinder::Const(t) if t.0 == [TypeExpressionStep::Scalar(ScalarType::Boolean)])));
        let Some(TemplateResult::StaticParameter { owner, index, site }) = &helper.result else { panic!("missing static read") };
        assert_eq!(site.span.slice(source.as_bytes()), Some(&b"FLAG"[..]));
        spec_corpus::observe_handoff!(psi "psi-output/nested-return-binder", assert_eq!((owner, *index), (&outer.declaration, usize::from(twin))));
        assert!(helper.body.is_empty() && !helper.public);
        assert_eq!(outer.result, Some(TemplateResult::Operand(omega_psi::TemplateOperand::Step(0))));
        let [omega_psi::TemplateStep::Call(call)] = outer.body.as_slice() else { panic!("expected one executable call") };
        assert_eq!(call.operands, [omega_lowering::TemplateOperand::Formal(0)]);
        let captures = vec![(outer.declaration.clone(), (0..=usize::from(twin)).map(|index| TypeExpression(vec![TypeExpressionStep::GenericParameter { owner: outer.declaration.clone(), index }])).collect::<Vec<_>>())];
        if entry == "helper(input)" {
            assert!(call.arguments.is_empty() && outer.invokes_machines.is_empty());
            spec_corpus::observe_handoff!(psi "psi-output/nested-direct-capture", assert_eq!((&call.target, &call.enclosing_arguments, values), (&TemplateCallTarget::Declaration(helper.declaration.clone()), &captures, [true,false])));
        } else {
            assert_eq!(call.target, TemplateCallTarget::Declaration(invoke.declaration.clone()));
            assert!(call.enclosing_arguments.is_empty());
            let [TypeExpression(steps)] = call.arguments.as_slice() else { panic!("expected one static argument") };
            let [TypeExpressionStep::MachineSelection(selection)] = steps.as_slice() else { panic!("not a selected callback") };
            assert!(selection.arguments.is_empty());
            assert_eq!(outer.invokes_machines.as_slice(), std::slice::from_ref(selection.as_ref()));
            assert_eq!(invoke.body[0].call().unwrap().target, TemplateCallTarget::StaticBinder(0));
            spec_corpus::observe_handoff!(psi "psi-output/nested-callback-capture", assert_eq!((&selection.declaration, &selection.enclosing_arguments, values), (&helper.declaration, &captures, [!twin,twin])));
        }
    }
}

/// L4 CONTRACT "A specialized field request carries the declarations that own its
/// arguments, in order from the enclosing declaration to the field's immediate owner."
#[test]
fn nested_field_owner_arguments_select_the_returned_field_arrays() {
    let source = "machine inc(n:u64)->u64{n +% 1}
                  machine same(n:u64)->u64{n}
                  data Outer<const A:u64>{data Inner<const B:u64>{
                    outer:[u8;same(inc(A))];
                    inner:[u8;same(inc(B))];
                  }}
                  machine outer(x:Outer<2>::Inner<4>)->[u8;3]{x.outer}
                  machine inner(x:Outer<2>::Inner<4>)->[u8;5]{x.inner}";
    let swapped = source.replace("Outer<2>::Inner<4>", "Outer<4>::Inner<2>")
        .replace(";3]{x.outer}", ";5]{x.outer}").replace(";5]{x.inner}", ";3]{x.inner}");
    let original = field_extents(source, [2, 4]);
    let control = field_extents(&swapped, [4, 2]);
    spec_corpus::observe_handoff!(psi "psi-output/specialized-field-extents", assert_eq!(original, [3, 5]));
    spec_corpus::observe_handoff!("specialized-owner-layout/psi-field-reads", assert_eq!((original, control), (vec![3, 5], vec![5, 3])));
}

/// Integer arguments are identities, not just recipes for computing layout.
#[test]
fn equal_layout_does_not_merge_different_integer_applications() {
    use omega_psi::{DataArgument, ScalarType};
    let source = "data Slot<const N:u64>{value:u8;} machine a(x:Slot<3>){} machine b(x:Slot<4>){} machine c(x:Slot<3>){}";
    let psi = sources::lower(&[("slots.omg", source)]).unwrap();
    let [three, four] = psi.data.as_slice() else { panic!("two closed applications") };
    let carrier = ScalarType::Integer { signed: false, width: 64 };
    assert_eq!(three.type_arguments, [DataArgument::Integer { carrier, value: 3.into() }]);
    assert_eq!(four.type_arguments, [DataArgument::Integer { carrier, value: 4.into() }]);
    assert_eq!(three.declaration, four.declaration);
    assert_eq!(three.fields, four.fields);
    assert_ne!(three.carrier, four.carrier);
    let actual: Vec<_> = psi.machines.iter().map(|m| (m.declaration.path.as_str(), m.parameter_types.as_slice())).collect();
    assert_eq!(actual, [("a", [three.carrier].as_slice()), ("b", [four.carrier].as_slice()), ("c", [three.carrier].as_slice())]);
}

fn field_extents(source: &str, args: [u64; 2]) -> Vec<u64> {
    use omega_psi::{BlockTerminator, DataArgument, OperationKind, OperationResult, Parameter, PlaceProjection, ReturnEdge, ScalarType, TypeShape, TypeExpressionStep as Step};
    let package = sources::resolve(&[("fields.omg", source)]);
    let prepared = prepare_source_package(&package).unwrap();
    let integer = |ty: &ValueType| match ty {
        ValueType::StaticInteger { value, .. } => value.to_string(),
        ValueType::StaticValue {
            value: StaticValue::Integer(value),
            ..
        } => value.to_string(),
        other => panic!("expected a closed integer owner argument, found {other:?}"),
    };
    // Both fields are answered under Outer's argument followed by Inner's argument.
    let psi = sources::lower_prepared(&prepared, &[("fields.omg", source)]).unwrap();
    let inner = psi.data_templates.iter().find(|t| t.declaration.path == "Outer::Inner").unwrap();
    let carrier = ScalarType::Integer {signed:false, width:64};
    let schema = psi.data.iter().find(|d| d.declaration == inner.declaration).unwrap();
    assert_eq!(schema.type_arguments, args.map(|n| DataArgument::Integer {carrier, value:n.into()}));
    let mut returned = Vec::new();
    for (i, name) in ["outer", "inner"].into_iter().enumerate() {
        let extent = args[i]+1;
        let recipe = if i == 0 { "same(inc(A))" } else { "same(inc(B))" };
        let answer = prepared.specialized_indices().iter().find(|a| a.request.expression.span.slice(source.as_bytes()) == Some(recipe.as_bytes())).unwrap();
        assert_eq!(answer.arguments.iter().map(integer).collect::<Vec<_>>(), args.map(|n| n.to_string()));
        assert_eq!(answer.value, StaticValue::Integer(extent.into()));
        let binder = if i == 0 { &inner.enclosing_parameters[0] } else { &inner.declaration };
        let machine = |name| &psi.machines.iter().find(|m| m.declaration.path == name).unwrap().declaration;
        assert!(matches!(inner.fields[i].ty.0.as_slice(), [Step::Scalar(_), Step::GenericParameter { owner, index: 0 },
            Step::StaticCall { machine: inc, operands: 1, .. }, Step::StaticCall { machine: same, operands: 1, .. }, Step::IndexedArray]
            if owner == binder && inc == machine("inc") && same == machine("same")));
        let reader = psi.machines.iter().find(|m| m.declaration.path == name).unwrap();
        assert_eq!(reader.parameter_types, [schema.carrier]);
        let field = &schema.fields[i];
        assert_eq!(field.declaration.path, format!("Outer::Inner::{name}"));
        let [op] = reader.body.operations.as_slice() else { panic!("one field copy") };
        let OperationKind::CopyStructural { source: place } = &op.kind else { panic!("expected field copy: {op:?}") };
        assert_eq!(reader.body.parameters, [Parameter::Structural(place.root)]);
        assert_eq!(place.path, [PlaceProjection::Field(field.declaration.clone())]);
        let OperationResult::Structural(copy) = op.result else { panic!("array copy") };
        assert!(reader.body.blocks.iter().any(|b| matches!(b.terminator, BlockTerminator::Return {result:ReturnEdge::Structural {source,..},..} if source == copy)));
        assert_eq!(reader.body.places[copy.index()].ty, field.carrier);
        assert_eq!(reader.result_type, Some(field.carrier));
        let Some(TypeShape::FixedArray {element,length}) = psi.types.get(field.carrier) else { panic!("fixed field extent") };
        assert_eq!(psi.types.get(*element), Some(&TypeShape::PrimitiveScalar(ScalarType::Integer {signed:false,width:8})));
        returned.push(*length);
    }
    returned
}
