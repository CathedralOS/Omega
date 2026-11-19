use omega_lowering::{
    LoweredMachine, LoweredPackage, LoweringError, Parameter, PlaceOrigin, ReturnEdge, ScalarLiteral, ScalarType, TypeId, TypeShape, ValueOrigin, lower_package,
};
use omega_preparation::prepare_source_package;
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{SourcePackage, ValueType, resolve_source_package};

const DEFINITIONS: &str = "machine width()->u64 reaches none crashes none terminates {3u64}\nconst SIZE:u64=width();";
const SELECTOR: &str = "machine select(a:[u8; width()], b:[u8; SIZE], literal:[u8; 3], wide:[u16; 3], flag:bool)->[u8; width()]{a}
machine nested(computed:[[i8; width()]; 2], named:[[i8; SIZE]; 2], literal:[[i8; 3]; 2])->[[i8; width()]; 2]{named}";

fn resolve<'text>(files: &[(&'text str, &'text str)]) -> SourcePackage<'text> {
    let syntax: Vec<_> = files
        .iter()
        .map(|(_, source)| {
            let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
            let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
            (tokens, tree)
        })
        .collect();
    let sources: Vec<_> = files
        .iter()
        .zip(&syntax)
        .map(|(&(logical_path, text), (tokens, tree))| StageSource {
            package: "app",
            source: ParsedSource {
                logical_path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    resolve_source_package(&sources).unwrap()
}

fn lower(files: &[(&str, &str)]) -> Result<LoweredPackage, LoweringError> {
    let package = resolve(files);
    let prepared = prepare_source_package(&package).expect("the source reaches L5 after preparation");
    lower_package(&prepared)
}

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
        let ReturnEdge::Structural { source, result } = nested.body.return_edge else {
            panic!("expected a nested array return, found {:?}", nested.body.return_edge);
        };
        assert_eq!(nested.body.parameters[1], Parameter::Structural(source));
        assert_ne!(source, result);
        assert_eq!(nested.body.places[result.index()].ty, computed);
        assert_eq!(nested.body.places[result.index()].origin, PlaceOrigin::Result);

        let width = machine(package, "width");
        let ReturnEdge::Scalar(returned) = width.body.return_edge else {
            panic!("expected a scalar width, found {:?}", width.body.return_edge);
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
        assert_eq!(operation.literal, ScalarLiteral::Integer(length.into()));
        assert_eq!(operation.result, returned);
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
            let ReturnEdge::Structural { source, result } = select.body.return_edge else {
                panic!("expected a structural return, found {:?}", select.body.return_edge);
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
fn an_unsupported_runtime_expression_is_refused_at_the_expression() {
    let source = "machine increment(x:u64)->u64{x +% 1u64}";
    let error = lower(&[("increment.omg", source)]).unwrap_err();
    assert_eq!(error.site.package, "app");
    assert_eq!(error.site.file, "increment.omg");
    assert_eq!(error.message, "unsupported return expression: Binary");
    assert_eq!(error.site.span.slice(source.as_bytes()), Some(b"x +% 1u64".as_slice()));
}

#[test]
fn public_contract_defaults_are_lowered_as_effective_bounds() {
    let source = "machine private_empty() {}
        public machine implicit() {}
        public machine explicit() crashes none blocks false suspends false {}";
    let lowered = lower(&[("contracts.omg", source)]).unwrap();
    let private = &machine(&lowered, "private_empty").contract;
    let implicit = &machine(&lowered, "implicit").contract;
    let explicit = &machine(&lowered, "explicit").contract;
    assert!(!private.public && !private.has_empty_crash_ceiling);
    assert_eq!(private.pause_bounds.block, None);
    assert_eq!(private.pause_bounds.suspend, None);
    assert!(implicit.public && implicit.has_empty_crash_ceiling);
    assert_eq!(implicit.pause_bounds.block, Some(false));
    assert_eq!(implicit.pause_bounds.suspend, Some(false));
    assert_eq!(implicit, explicit);
}

#[test]
fn refusal_for_target_dependent_extent_points_to_the_observed_input() {
    use omega_preparation::{LayoutExtent, LayoutGeometry, SelectedLayout, TargetIdentity, TargetSemantics, prepare_source_package_for_target};
    let source = "use omega::language::core; const UNRELATED:u64=9u64;
        const EXTENT:u64=core::size_of::<u64>(); machine read(arr:[u8;EXTENT]) {}";
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
    let error = lower_package(&prepared).unwrap_err();
    assert_eq!(error.message, "target-dependent values require a portable target closure");
    assert_eq!(error.site.file, "extent.omg");
    assert_eq!(error.site.span.slice(source.as_bytes()), Some(b"core::size_of::<u64>()".as_slice()));
}
