#[path = "support/known_gap.rs"]
mod known_gap;
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{BlockTerminator, LoweredMachine, LoweredPackage, OperationKind, OperationResult, Parameter, ReturnEdge, ScalarType, TypeShape};

const U64: ScalarType = ScalarType::Integer { signed: false, width: 64 };

fn machine<'a>(
    package: &'a LoweredPackage,
    name: &str,
) -> &'a LoweredMachine {
    package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == name)
        .unwrap_or_else(|| panic!("expected machine {name}"))
}

#[test]
fn a_read_of_one_same_typed_element_view_measures_only_that_view() {
    let u32_element = ScalarType::Integer { signed: false, width: 32 };
    for (read_view, slot) in [("left", 0), ("right", 1)] {
        let text = format!("machine get(left:@[u32],right:@[u32],i:u64)->u32 {{ {read_view}[i] }}");
        let package = sources::lower(&[("views.omg", &text)]).unwrap();
        let machine = &package.machines[0];
        let body = &machine.body;
        let [
            Parameter::Structural(left),
            Parameter::Structural(right),
            Parameter::Scalar(selector),
        ] = body.parameters[..]
        else {
            panic!("expected two element views and an index")
        };
        // Same-typed twins share one descriptor type but keep separate places.
        assert_ne!(left, right);
        assert_eq!(body.places[left.index()].ty, body.places[right.index()].ty);
        assert_eq!(machine.parameter_types[0], machine.parameter_types[1]);
        assert_eq!(
            package.types.get(body.places[left.index()].ty),
            Some(&TypeShape::SharedElementView { element: u32_element })
        );
        let selected = [left, right][slot];
        let [measure, read] = body.operations.as_slice() else {
            panic!("expected one length observation immediately before the read")
        };
        assert_eq!(measure.kind, OperationKind::ElementViewLength { source: selected });
        let OperationResult::Scalar(measured) = measure.result else {
            panic!("expected a scalar element count")
        };
        assert_eq!(package.types.get(body.values[measured.index()].ty), Some(&TypeShape::PrimitiveScalar(U64)));
        let OperationKind::ElementViewRead {
            source,
            index,
            length,
            obligation,
            path,
        } = &read.kind
        else {
            panic!("expected an element view read")
        };
        assert_eq!((*source, *index, *length), (selected, selector, measured));
        assert!(path.is_empty(), "a scalar element is selected by the empty leaf path");
        assert_eq!(
            body.diagnostics.bounds[obligation.get() - 1]
                .span
                .slice(text.as_bytes()),
            Some(&b"i"[..])
        );
        let OperationResult::Scalar(element) = read.result else {
            panic!("expected a scalar element")
        };
        assert_eq!(
            package.types.get(body.values[element.index()].ty),
            Some(&TypeShape::PrimitiveScalar(u32_element))
        );
        assert!(matches!(
            body.blocks[0].terminator,
            BlockTerminator::Return { result: ReturnEdge::Scalar(value), .. } if value == element
        ));
    }
}

#[test]
fn an_explicit_element_length_is_an_element_count_of_the_exact_view() {
    let text = "machine count(other:@[u16],items:@[u16])->u64 { items.len }";
    let package = sources::lower(&[("count.omg", text)]).unwrap();
    let body = &machine(&package, "count").body;
    let [Parameter::Structural(other), Parameter::Structural(items)] = body.parameters[..] else {
        panic!("expected two element views")
    };
    assert_ne!(other, items);
    let [length] = body.operations.as_slice() else {
        panic!("expected one length observation")
    };
    assert_eq!(length.kind, OperationKind::ElementViewLength { source: items });
    assert_eq!(
        package.types.get(body.places[items.index()].ty),
        Some(&TypeShape::SharedElementView {
            element: ScalarType::Integer { signed: false, width: 16 }
        })
    );
}

#[test]
fn byte_and_element_views_keep_distinct_descriptors_and_operations() {
    let text = "machine bytes(b:@[u8],i:u64)->u8 { b[i] }
                machine signed(s:@[i8],i:u64)->i8 { s[i] }
                machine flags(f:@[bool],i:u64)->bool { f[i] }";
    let package = sources::lower(&[("kinds.omg", text)]).unwrap();
    let bytes = machine(&package, "bytes");
    let signed = machine(&package, "signed");
    let flags = machine(&package, "flags");
    assert_eq!(package.types.get(bytes.parameter_types[0]), Some(&TypeShape::SharedByteView));
    assert_eq!(
        package.types.get(signed.parameter_types[0]),
        Some(&TypeShape::SharedElementView {
            element: ScalarType::Integer { signed: true, width: 8 }
        })
    );
    assert_eq!(
        package.types.get(flags.parameter_types[0]),
        Some(&TypeShape::SharedElementView { element: ScalarType::Boolean })
    );
    let [byte_length, byte_read] = bytes.body.operations.as_slice() else {
        panic!("expected a byte length and read")
    };
    assert!(matches!(byte_length.kind, OperationKind::ByteSequenceLength { .. }));
    assert!(matches!(byte_read.kind, OperationKind::ByteSequenceRead { .. }));
    for typed in [signed, flags] {
        let [length, read] = typed.body.operations.as_slice() else {
            panic!("expected an element length and read")
        };
        assert!(matches!(length.kind, OperationKind::ElementViewLength { .. }));
        assert!(matches!(read.kind, OperationKind::ElementViewRead { .. }));
    }
}

#[test]
fn an_element_length_contract_names_its_exact_view_and_supplies_no_twin_bound() {
    use omega_lowering::{PredicateOperator, PredicateStep};
    for (read_view, slot) in [("items", 1), ("other", 0)] {
        let text = format!("machine get(other:@[u32],items:@[u32],i:u64)->u32 requires i < items.len {{ {read_view}[i] }}");
        let package = sources::lower(&[("contract.omg", &text)]).unwrap();
        let machine = &package.machines[0];
        // The clause measures `items` (formal 1), never its same-typed twin.
        assert_eq!(
            machine.contract.requires[0].predicate,
            [
                PredicateStep::Formal(2),
                PredicateStep::Formal(1),
                PredicateStep::ElementViewLength,
                PredicateStep::Apply(PredicateOperator::LessThan)
            ]
        );
        assert_ne!(
            machine.contract.requires[0].predicate[1],
            PredicateStep::Formal(0),
            "the twin view is not the measured subject"
        );
        let body = &machine.body;
        let [
            Parameter::Structural(other),
            Parameter::Structural(items),
            Parameter::Scalar(selector),
        ] = body.parameters[..]
        else {
            panic!("expected two element views and an index")
        };
        assert_ne!(other, items);
        let selected = [other, items][slot];
        // The requirement is not a body bound: each read still measures its own view.
        let [measure, read] = body.operations.as_slice() else {
            panic!("expected one length observation immediately before the read")
        };
        assert_eq!(measure.kind, OperationKind::ElementViewLength { source: selected });
        let OperationKind::ElementViewRead {
            source,
            index,
            length,
            obligation,
            ..
        } = &read.kind
        else {
            panic!("expected an element view read")
        };
        assert_eq!((*source, *index), (selected, selector));
        assert_eq!(OperationResult::Scalar(*length), measure.result);
        assert_eq!(
            body.diagnostics.bounds[obligation.get() - 1]
                .span
                .slice(text.as_bytes()),
            Some(&b"i"[..])
        );
    }
}

#[test]
fn an_element_length_guarantee_names_the_exact_view() {
    use omega_lowering::{PredicateOperator, PredicateStep};
    let text = "machine count(other:@[u16],items:@[u16])->u64 ensures result == items.len { items.len }";
    let package = sources::lower(&[("count.omg", text)]).unwrap();
    let count = machine(&package, "count");
    assert_eq!(
        count.contract.ensures[0].predicate,
        [
            PredicateStep::Result,
            PredicateStep::Formal(1),
            PredicateStep::ElementViewLength,
            PredicateStep::Apply(PredicateOperator::Equal)
        ]
    );
}

#[test]
fn an_element_length_contract_is_not_a_byte_or_fixed_length() {
    use omega_lowering::PredicateStep;
    let text = "machine bytes(b:@[u8],i:u64)->u8 requires i < b.len { b[i] }
                machine fixed(a:[u8;4],i:u64)->bool requires i < a.len { true }
                machine elements(e:@[i8],i:u64)->i8 requires i < e.len { e[i] }";
    let package = sources::lower(&[("kinds.omg", text)]).unwrap();
    for (name, step) in [
        ("bytes", PredicateStep::ByteSequenceLength),
        ("fixed", PredicateStep::FixedArrayLength),
        ("elements", PredicateStep::ElementViewLength),
    ] {
        assert_eq!(machine(&package, name).contract.requires[0].predicate[2], step, "{name}");
    }
}

#[test]
fn an_element_length_contract_on_a_mutable_view_still_rejects() {
    let text = "machine get(items:@mut [u32],i:u64)->u32 requires i < items.len { items[i] }";
    let error = sources::refuse(
        &[("contract.omg", text)],
        "contract.omg",
        sources::unique_span(text, "items.len"),
        "unsupported contract predicate expression",
    );
    known_gap::report("mutable element-view length contract", "Psi needs the element count of the mutable formal", text, &error);
    assert_eq!(error.site.span.slice(text.as_bytes()), Some(b"items.len".as_slice()));
}

#[test]
fn a_mutable_element_view_is_not_an_immutable_parameter() {
    let text = "machine get(items:@mut [u32],i:u64)->u32 { items[i] }";
    let error = sources::refuse(
        &[("mutable.omg", text)],
        "mutable.omg",
        sources::span_within(text, "items:@mut [u32]", "items"),
        "no Psi lowering for carrier @mut [u32]",
    );
    known_gap::report("mutable element-view read", "Psi needs the mutable descriptor, selector and bounds obligation", text, &error);
    assert_eq!(error.site.span.slice(text.as_bytes()), Some(b"items".as_slice()));
}
