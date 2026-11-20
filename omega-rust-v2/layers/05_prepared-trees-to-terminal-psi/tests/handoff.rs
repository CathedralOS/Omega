#[path = "support/source.rs"]
mod sources;

#[path = "support/returns.rs"]
mod returns;
use returns::sole_return;

use omega_lowering::{
    LoweredMachine, LoweredPackage, Parameter, PlaceOrigin, ReturnEdge, ScalarLiteral, ScalarType, TypeId, TypeShape, ValueOrigin, lower_package,
};
use omega_preparation::prepare_source_package;
use omega_typer::ValueType;
use sources::{lower, resolve};

const DEFINITIONS: &str = "machine width()->u64 reaches none crashes none terminates {3u64}\nconst SIZE:u64=width();";
const SELECTOR: &str = "machine select(a:[u8; width()], b:[u8; SIZE], literal:[u8; 3], wide:[u16; 3], flag:bool)->[u8; width()]{a}
machine nested(computed:[[i8; width()]; 2], named:[[i8; SIZE]; 2], literal:[[i8; 3]; 2])->[[i8; width()]; 2]{named}";

fn machine<'package>(
    package: &'package LoweredPackage,
    path: &str,
) -> &'package LoweredMachine {
    package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == path)
        .unwrap()
}

fn array_element(
    package: &LoweredPackage,
    ty: TypeId,
    length: u64,
) -> TypeId {
    let Some(TypeShape::FixedArray {
        element,
        length: actual_length,
    }) = package.types.get(ty)
    else {
        panic!("expected a fixed array type, found {:?}", package.types.get(ty));
    };
    assert_eq!(*actual_length, length);
    *element
}

fn assert_parameter_rows(
    package: &LoweredPackage,
    machine: &LoweredMachine,
) {
    assert_eq!(machine.body.parameters.len(), machine.parameter_types.len());
    for (position, (&parameter, &ty)) in machine
        .body
        .parameters
        .iter()
        .zip(&machine.parameter_types)
        .enumerate()
    {
        assert!(!machine.body.parameters[..position].contains(&parameter));
        match (package.types.get(ty), parameter) {
            (Some(TypeShape::PrimitiveScalar(_)), Parameter::Scalar(id)) => {
                let value = &machine.body.values[id.index()];
                assert_eq!(value.ty, ty);
                assert_eq!(value.origin, ValueOrigin::Parameter);
            }
            (Some(TypeShape::FixedArray { .. }), Parameter::Structural(id)) => {
                let place = &machine.body.places[id.index()];
                assert_eq!(place.ty, ty);
                assert_eq!(place.origin, PlaceOrigin::Parameter);
            }
            _ => panic!("parameter {position} does not match its declared type"),
        }
    }
}

#[test]
fn prepared_array_extents_and_neighboring_types_reach_owned_psi() {
    let (baseline, changed) = {
        let definitions = DEFINITIONS.to_owned();
        let selector = SELECTOR.to_owned();
        let changed_definitions = definitions.replace("3u64", "4u64");
        (
            lower(&[("width.omg", &definitions), ("select.omg", &selector)]).unwrap(),
            lower(&[("width.omg", &changed_definitions), ("select.omg", &selector)]).unwrap(),
        )
    };

    for (package, length) in [(&baseline, 3), (&changed, 4)] {
        let select = machine(package, "select");
        let [computed, named, literal, wide, flag] = select.parameter_types[..] else {
            panic!("expected all five parameter types, found {:?}", select.parameter_types);
        };
        assert_eq!(select.result_type, Some(computed));
        assert_eq!(computed, named);
        assert_eq!(computed == literal, length == 3);
        for (array, count, width) in [(computed, length, 8), (literal, 3, 8), (wide, 3, 16)] {
            let element = array_element(package, array, count);
            assert_eq!(
                package.types.get(element),
                Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width }))
            );
        }
        assert_eq!(package.types.get(flag), Some(&TypeShape::PrimitiveScalar(ScalarType::Boolean)));
        assert_parameter_rows(package, select);

        let nested = machine(package, "nested");
        let [computed, named, literal] = nested.parameter_types[..] else {
            panic!("expected all three nested parameter types, found {:?}", nested.parameter_types);
        };
        assert_eq!(nested.result_type, Some(computed));
        assert_eq!(computed, named);
        assert_eq!(computed == literal, length == 3);
        for (outer, inner_count) in [(computed, length), (literal, 3)] {
            let inner = array_element(package, outer, 2);
            let element = array_element(package, inner, inner_count);
            assert_eq!(
                package.types.get(element),
                Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: true, width: 8 }))
            );
        }
        assert_parameter_rows(package, nested);
        let ReturnEdge::Structural { source, result } = sole_return(&nested.body) else {
            panic!("expected a nested array return, found {:?}", sole_return(&nested.body));
        };
        assert_eq!(nested.body.parameters[1], Parameter::Structural(source));
        assert_ne!(source, result);
        assert_eq!(nested.body.places[result.index()].ty, computed);
        assert_eq!(nested.body.places[result.index()].origin, PlaceOrigin::Result);

        let width = machine(package, "width");
        let ReturnEdge::Scalar(returned) = sole_return(&width.body) else {
            panic!("expected a scalar width, found {:?}", sole_return(&width.body));
        };
        let value = &width.body.values[returned.index()];
        assert_eq!(Some(value.ty), width.result_type);
        assert_eq!(
            package.types.get(value.ty),
            Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 64 }))
        );
        let ValueOrigin::Operation(id) = value.origin else {
            panic!("the returned literal has a defining operation")
        };
        let operation = &width.body.operations[id.index()];
        assert_eq!(operation.id, id);
        assert_eq!(operation.kind, omega_lowering::OperationKind::Constant(ScalarLiteral::Integer(length.into())));
        assert_eq!(operation.result, omega_lowering::OperationResult::Scalar(returned));
    }
}

#[test]
fn structural_return_names_the_selected_parameter_in_either_file_order() {
    let mut baseline = None;
    for (returned_parameter, selector) in [(0, SELECTOR.to_owned()), (1, SELECTOR.replace("{a}", "{b}"))] {
        let files = [("width.omg", DEFINITIONS), ("select.omg", selector.as_str())];
        let reversed = [files[1], files[0]];
        for files in [files, reversed] {
            let lowered = lower(&files).unwrap();
            let select = machine(&lowered, "select");
            let ReturnEdge::Structural { source, result } = sole_return(&select.body) else {
                panic!("expected a structural return, found {:?}", sole_return(&select.body));
            };
            let Parameter::Structural(selected) = select.body.parameters[returned_parameter] else {
                panic!("expected a structural parameter");
            };
            assert_ne!(select.body.parameters[0], select.body.parameters[1]);
            assert_eq!(source, selected);
            assert_ne!(source, result);
            assert_eq!(select.body.places[source.index()].origin, PlaceOrigin::Parameter);
            assert_eq!(select.body.places[result.index()].origin, PlaceOrigin::Result);
            assert_eq!(Some(select.body.places[result.index()].ty), select.result_type);
            assert_eq!(select.body.places[source.index()].ty, select.body.places[result.index()].ty);

            let unchanged = (
                lowered.types.clone(),
                machine(&lowered, "width").id,
                select.id,
                select.parameter_types.clone(),
                select.result_type,
                select.body.parameters.clone(),
                select.body.places.clone(),
                select.body.values.clone(),
                result,
            );
            let baseline = baseline.get_or_insert(unchanged.clone());
            assert_eq!(*baseline, unchanged);
        }
    }
}

#[test]
fn a_changed_extent_refuses_a_return_with_the_old_shape_during_preparation() {
    let source = "machine width()->u64 reaches none crashes none terminates {4u64}
                  machine select(a:[u8; width()], b:[u8; 3])->[u8; width()]{b}";
    let package = resolve(&[("select.omg", source)]);
    let error = prepare_source_package(&package).unwrap_err();
    assert_eq!(error.file, "select.omg");
    assert_eq!(error.message, "expected [u8; 4], found [u8; 3]");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"b".as_slice()));
}

#[test]
fn an_open_array_extent_is_refused_at_the_generic_machine_declaration() {
    let source = "machine open_extent<const N:u64>(array:[u8; N]){}";
    let package = resolve(&[("open.omg", source)]);
    let prepared = prepare_source_package(&package).unwrap();
    let signature = prepared
        .typed()
        .signatures
        .iter()
        .find(|sig| sig.machine.path == "open_extent")
        .unwrap();
    assert!(matches!(signature.parameters[0].1, ValueType::IndexedArray(..)));
    let error = lower_package(&prepared).unwrap_err();
    assert_eq!(error.site, signature.site);
    assert_eq!(error.site.package, "app");
    assert_eq!(error.site.file, "open.omg");
    assert_eq!(error.site.span.slice(source.as_bytes()), Some(source.as_bytes()));
}

#[test]
fn a_runtime_increment_returns_the_result_of_the_wrapping_operation() {
    let source = "machine increment(x:u64)->u64{x +% 1u64}";
    let package = lower(&[("increment.omg", source)]).unwrap();
    let body = &machine(&package, "increment").body;
    let ReturnEdge::Scalar(value) = sole_return(body) else {
        panic!("expected a scalar return")
    };
    let ValueOrigin::Operation(id) = body.values[value.index()].origin else {
        panic!("expected a computed value")
    };
    assert!(matches!(body.operations[id.index()].kind, omega_lowering::OperationKind::Integer { .. }));
    assert_eq!(
        body.diagnostics.operations[id.index()]
            .span
            .slice(source.as_bytes()),
        Some(b"x +% 1u64".as_slice())
    );
}

#[test]
fn public_contract_defaults_are_lowered_as_effective_bounds() {
    let source = "machine private_empty() {}
        public machine implicit() {}
        public machine explicit() crashes none blocks false suspends false {}";
    let lowered = lower(&[("contracts.omg", source)]).unwrap();
    let private = psi_handoff_consumer::machine_contract(&lowered, "private_empty").unwrap();
    let implicit = psi_handoff_consumer::machine_contract(&lowered, "implicit").unwrap();
    let explicit = psi_handoff_consumer::machine_contract(&lowered, "explicit").unwrap();
    assert!(!private.public && !private.has_empty_crash_ceiling);
    assert_eq!(private.pause_bounds.block, None);
    assert_eq!(private.pause_bounds.suspend, None);
    assert!(implicit.public && implicit.has_empty_crash_ceiling);
    assert_eq!(implicit.pause_bounds.block, Some(false));
    assert_eq!(implicit.pause_bounds.suspend, Some(false));
    assert_eq!(implicit, explicit);
}

#[test]
fn assigning_a_scalar_local_changes_only_subsequent_reads() {
    let source = "machine read(x:u8)->u8{let mut local:u8=x; let before:u8=local; local=1u8; before +% local}";
    let package = lower(&[("local.omg", source)]).unwrap();
    let body = &machine(&package, "read").body;
    let [Parameter::Scalar(original)] = body.parameters[..] else {
        panic!("expected one scalar")
    };
    let [constant, add] = body.operations.as_slice() else {
        panic!("expected the update and the trailing sum")
    };
    let omega_lowering::OperationResult::Scalar(new_value) = constant.result else {
        panic!("expected scalar initializer")
    };
    let omega_lowering::OperationKind::Integer { operands, .. } = &add.kind else {
        panic!("expected addition")
    };
    assert_eq!(operands, &[original, new_value]);
}

#[test]
fn nested_machine_has_its_own_body_and_explicit_argument() {
    let source = "machine outer(x:u8)->u8 { inner(x) machine inner(y:u8)->u8 {y} }";
    let package = lower(&[("nested.omg", source)]).unwrap();
    let outer = machine(&package, "outer");
    let inner = machine(&package, "outer::inner");
    let [Parameter::Scalar(x)] = outer.body.parameters[..] else {
        panic!("expected a caller argument")
    };
    let [call] = outer.body.operations.as_slice() else {
        panic!("expected one call")
    };
    assert!(matches!(&call.kind, omega_lowering::OperationKind::Call {callee: omega_lowering::CallTarget::Machine(callee), arguments, ..}
        if *callee == inner.id && arguments == &[omega_lowering::CallArgument::Scalar(x)]));
    let [Parameter::Scalar(y)] = inner.body.parameters[..] else {
        panic!("expected a callee argument")
    };
    assert_eq!(sole_return(&inner.body), ReturnEdge::Scalar(y));
}

use tree_handoff_consumer::{assignment_spelling, frozen_integer, resolves_reference_to, selected_nullary_call};

#[test]
fn each_product_can_be_read_without_the_code_that_built_it() {
    for (selected, other, expected) in [("three", "four", 3), ("four", "three", 4)] {
        let text = format!(
            "machine three()->u64 reaches none crashes none terminates {{3u64}}
             machine four()->u64 reaches none crashes none terminates {{4u64}}
             const SIZE:u64={selected}();"
        );
        let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
        let syntax = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
        assert_eq!(assignment_spelling(&text, &tokens, &syntax), Some("="));
        let package = resolve(&[("size.omg", &text)]);
        let prepared = prepare_source_package(&package).unwrap();
        let typed = prepared.typed();
        assert!(resolves_reference_to(typed.resolved(), selected));
        assert!(!resolves_reference_to(typed.resolved(), other));
        assert!(selected_nullary_call(typed, selected));
        assert!(!selected_nullary_call(typed, other));
        assert_eq!(frozen_integer(&prepared, "SIZE"), Some(expected));
    }
}

#[test]
fn target_dependent_extent_retains_its_observed_input() {
    use omega_preparation::{LayoutExtent, LayoutGeometry, SelectedLayout, TargetIdentity, TargetSemantics, prepare_source_package_for_target};
    let source = "use omega\\language\\core; const UNRELATED:u64=9u64;
        const EXTENT:u64=core\\size_of::<u64>(); machine read(arr:[u8;EXTENT]) {}";
    let package = resolve(&[("extent.omg", source)]);
    let identity = TargetIdentity {
        profile: "test-target".into(),
        semantics_version: 1,
        realization: vec![7].into(),
    };
    let layout = SelectedLayout {
        target: identity.clone(),
        subject: ValueType::Integer {
            signed: false,
            width: Some(64),
        },
        plan: vec![1].into(),
        schema: vec![1].into(),
        geometry: LayoutGeometry {
            extent: LayoutExtent::Fixed(8),
            alignment: Some(8),
        },
    };
    let target = TargetSemantics::new(identity, [layout], []).unwrap();
    let prepared = prepare_source_package_for_target(&package, &[], &target).unwrap();
    let lowered = lower_package(&prepared).unwrap();
    let closure = lowered.target_closure.as_ref().unwrap();
    let extent = closure
        .inputs()
        .iter()
        .find(|input| matches!(&input.origin, omega_lowering::FoldedValueOrigin::Constant(id) if id.path == "EXTENT"))
        .unwrap();
    assert_eq!(extent.site.file, "extent.omg");
    assert_eq!(extent.site.span.slice(source.as_bytes()), Some(b"core\\size_of::<u64>()".as_slice()));
    assert_eq!(extent.observations, [0]);
    assert!(!closure.inputs().iter().any(|input| matches!(
        &input.origin, omega_lowering::FoldedValueOrigin::Constant(id) if id.path == "UNRELATED"
    )));
}

#[test]
fn explicit_dereference_reads_and_writes_the_original_scalar() {
    use omega_lowering::{OperationKind, OperationResult, PlaceProjection};

    let source = "machine update(at:@mut u8)->u8 { $at = 2u8; $at }";
    let package = lower(&[("dereference.omg", source)]).unwrap();
    let body = &machine(&package, "update").body;
    let [Parameter::Structural(reference)] = body.parameters[..] else {
        panic!("expected a reference parameter");
    };
    let [constant, store, read] = body.operations.as_slice() else {
        panic!("expected the update followed by the read");
    };
    let OperationResult::Scalar(value) = constant.result else {
        panic!("expected the scalar replacement");
    };
    assert!(matches!(&store.kind, OperationKind::ProjectedScalarStore { destination, value: stored }
        if destination.root == reference && destination.path == [PlaceProjection::Referent] && *stored == value));
    assert!(matches!(&read.kind, OperationKind::ProjectedScalarRead { source }
        if source.root == reference && source.path == [PlaceProjection::Referent]));
    let OperationResult::Scalar(result) = read.result else {
        panic!("expected a scalar read");
    };
    assert_eq!(sole_return(body), ReturnEdge::Scalar(result));
}

#[test]
fn borrowing_a_field_following_either_dereference_spelling_keeps_the_same_place() {
    use omega_lowering::{CallArgument, OperationKind, PlaceProjection, ReferenceAccess};

    let source = "data Cell { mark:u8; }
        machine take(at:@u8){}
        machine implicit(cell:@Cell){take(@cell.mark);}
        machine explicit(cell:@Cell){take(@($cell).mark);}";
    let package = lower(&[("borrow_field.omg", source)]).unwrap();
    for name in ["implicit", "explicit"] {
        let body = &machine(&package, name).body;
        let [Parameter::Structural(reference)] = body.parameters[..] else {
            panic!("expected a reference parameter");
        };
        let [call] = body.operations.as_slice() else {
            panic!("expected one call");
        };
        let OperationKind::Call { arguments, .. } = &call.kind else {
            panic!("expected a call");
        };
        assert!(matches!(arguments.as_slice(), [CallArgument::Borrow { place, access: ReferenceAccess::Shared }]
            if place.root == reference
                && matches!(place.path.as_slice(), [PlaceProjection::Referent, PlaceProjection::Field(field)]
                    if field.path == "Cell::mark")));
    }
}
