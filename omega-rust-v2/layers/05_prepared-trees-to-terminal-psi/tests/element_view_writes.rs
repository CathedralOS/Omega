//! `v[i] = 1u32` through `v: @mut [u32]`
//! (`wiki/spec/terminal-psi/byte_views.md#element-typed-slice-views`).
//!
//! A scalar element write replaces one element in place. It retains the view,
//! the evaluated index, the value, the same view's length observed just before
//! the write, and the bounds obligation `index < length` that the checker
//! judges. The read control lowers the same selection to an `ElementViewRead`.
#[path = "support/source.rs"]
mod source;

use omega_lowering::{OperationKind, ScalarType};
use psi_handoff_consumer::{ElementWrite, element_writes};

const CASE: &str = include_str!("fixtures/package_cases/scalar-element-view-write/case/case.omg");
const CONTROL: &str = include_str!("fixtures/package_cases/scalar-element-view-write/control/case.omg");
const U32: ScalarType = ScalarType::Integer { signed: false, width: 32 };

fn body(package: &omega_lowering::LoweredPackage) -> &omega_lowering::MachineBody {
    &package.machines.iter().find(|m| m.declaration.path == "f").unwrap().body
}

#[test]
fn a_scalar_element_write_keeps_its_view_index_length_and_bound() {
    let pkg = source::lower(&[("case.omg", CASE)]).unwrap();
    let body = body(&pkg);
    let [omega_lowering::Parameter::Structural(view), omega_lowering::Parameter::Scalar(i)] = body.parameters[..] else {
        panic!("f(v, i)")
    };
    let kinds: Vec<_> = body.operations.iter().map(|op| &op.kind).collect();
    let [
        OperationKind::Constant(_),
        OperationKind::ElementViewLength { source },
        OperationKind::ElementViewWrite {
            destination,
            path,
            index,
            value,
            length,
            obligation,
        },
    ] = kinds.as_slice()
    else {
        panic!("the value, the length, then the write: {kinds:?}")
    };
    let omega_lowering::ValueOrigin::Operation(observed) = body.values[length.index()].origin else {
        panic!("a length observation")
    };
    spec_corpus::observe_handoff!(psi "psi-output/scalar-element-view-write", assert_eq!(
        (
            (*source, *destination, path.is_empty(), *index, observed, obligation.get()),
            body.diagnostics.bounds.len(),
            element_writes(&pkg, "f"),
        ),
        (
            (view, view, true, i, body.operations[1].id, 1),
            1,
            Ok(vec![ElementWrite { formal: 0, element: U32, index: i, value: *value }]),
        )
    ));

    // Reading the same element is an observation with the same bound.
    let control = source::lower(&[("case.omg", CONTROL)]).unwrap();
    let body = self::body(&control);
    assert!(body.operations.iter().any(|op| matches!(op.kind, OperationKind::ElementViewRead { .. })));
    assert_eq!(element_writes(&control, "f"), Ok(vec![]));
}

#[test]
fn the_reader_rejects_a_write_without_its_view_length_or_carrier() {
    let original = source::lower(&[("case.omg", CASE)]).unwrap();
    fn write(pkg: &mut omega_lowering::LoweredPackage) -> &mut OperationKind {
        let f = pkg.machines.iter_mut().find(|m| m.declaration.path == "f").unwrap();
        &mut f.body.operations[2].kind
    }
    // The length must be the observation just before the write.
    let mut pkg = source::lower(&[("case.omg", CASE)]).unwrap();
    let OperationKind::ElementViewWrite { length, index, .. } = write(&mut pkg) else { unreachable!() };
    *length = *index;
    assert_eq!(element_writes(&pkg, "f"), Err("an element write needs the same view's length observed immediately before"));
    // The value must have the element's carrier: the u64 index does not.
    let mut pkg = source::lower(&[("case.omg", CASE)]).unwrap();
    let OperationKind::ElementViewWrite { value, index, .. } = write(&mut pkg) else { unreachable!() };
    *value = *index;
    assert_eq!(element_writes(&pkg, "f"), Err("the written value disagrees with the element carrier"));
    assert!(element_writes(&original, "f").is_ok());
}
