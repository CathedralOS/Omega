//! A public result may carry a borrow of storage containing an atomic word.
//! Its boundary still owes the input lifetime of the returned field.

#[path = "support/project.rs"]
mod project;

use terminal_psi::{BoundaryLifetimeDependency, StructuralAccess, StructuralPathSegment};

const BUILD: &str = r#"machine build(build: @mutable BuildPlan) {
    build.product = BuildProduct::Package { name: "cells" };
}"#;

const SOURCE: &str = r#"public data Cell { word: AtomicU32; }
public data BorrowedCell<'storage> { cell: @'storage Cell; }

public boundary trait Loan {
    machine lend<'storage>(cell: @'storage Cell) -> BorrowedCell<'storage> reaches none crashes none;
}

public CellLoans satisfies Loan {
    machine lend<'storage>(cell: @'storage Cell) -> BorrowedCell<'storage> {
        BorrowedCell { cell: cell }
    }
}"#;

#[test]
fn returned_borrow_keeps_its_input_lifetime_across_atomic_storage() {
    let scratch = project::stage(
        "borrow-of-atomic-storage",
        &[("build.omg", BUILD), ("main.omg", SOURCE)],
    );
    let publish = ["--emit-psi", "cells.psi", "build.omg"];
    let (built, report) = project::status(&project::omega(scratch.path(), &publish));
    assert!(built, "{report}");
    let bytes = std::fs::read(scratch.path().join("cells.psi")).unwrap();
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
    let module = artifact.semantic_module();
    let [requirement] = module.interfaces.boundary_machines.as_slice() else {
        panic!("one published loan requirement");
    };
    let [group] = requirement.result_lifetimes.as_deref().unwrap() else {
        panic!("the returned field has one declared lifetime");
    };
    assert_eq!(
        group.inputs,
        [BoundaryLifetimeDependency::Referent {
            argument_index: 0,
            access: StructuralAccess::SharedBorrow,
        }]
    );
    let [path] = group.results.as_slice() else {
        panic!("one borrowed result field");
    };
    assert!(matches!(path.as_slice(), [StructuralPathSegment::Field(_)]));

    let bad = SOURCE.replace(
        "BorrowedCell { cell: cell }",
        "let mutable local: Cell; BorrowedCell { cell: @local }",
    );
    let (built, report) = project::check(
        "invocation-local-atomic-storage",
        &[("build.omg", BUILD), ("main.omg", &bad)],
        &publish,
    );
    assert!(
        !built && report.contains("local storage cannot establish that promise"),
        "{report}"
    );
}

#[test]
fn returned_guards_publish_their_destructor_and_its_called_body() {
    let source = SOURCE.replace(
        "BorrowedCell<'storage> { cell:",
        "BorrowedCell<'storage> dropped by unlock { cell:",
    );
    let source = format!(
        "{source}\nmachine unlock<'storage>(guard: BorrowedCell<'storage>) {{
            let {{ cell }} = move guard;
            release(cell);
        }}
        machine release(cell: @Cell) {{ cell.word.store(0, Publish); }}"
    );
    let scratch = project::stage(
        "guard-returning-provider",
        &[("build.omg", BUILD), ("main.omg", &source)],
    );
    let (built, report) = project::status(&project::omega(
        scratch.path(),
        &["--emit-psi", "provider.psi", "build.omg"],
    ));
    assert!(built, "{report}");
    let bytes = std::fs::read(scratch.path().join("provider.psi")).unwrap();
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
    let module = artifact.semantic_module();
    let destructor = module
        .types
        .structural_types
        .iter()
        .find_map(|ty| ty.nominal_cleanup.as_ref())
        .unwrap();
    let unlock = module.machine(destructor.cleanup_machine).unwrap();
    assert!(unlock.module_invocations().any(|call| {
        module.machine(call.inputs.callee).is_some_and(|helper| {
            helper.operations().any(|op| {
                matches!(
                    op.kind,
                    terminal_psi::OperationKind::AtomicAccess {
                        event: terminal_psi::AtomicAccessEvent::Store { .. },
                        ..
                    }
                )
            })
        })
    }));
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn an_independent_native_call_accepts_its_declared_borrow_lifetime() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synchronization/run/independent_wait");
    let build = std::fs::read_to_string(fixtures.join("application/build.omg"))
        .unwrap()
        .replace("::Wait,", "::Loan,")
        .replace("::ObservedWait>", "::CellLoans>");
    let package = std::fs::read_to_string(fixtures.join("supplier/build.omg")).unwrap();
    let supplier = r#"public data Cell { word: u32; }
public boundary trait Loan {
    machine read<'storage>(cell: @'storage Cell) -> u32 reaches none crashes none;
}
public CellLoans satisfies Loan {
    machine read<'storage>(cell: @'storage Cell) -> u32 { cell.word }
}"#;
    let main = r#"use {signals}/main.omg as cells;
machine main() reaches |cells|::Loan crashes none {
    let cell: |cells|::Cell = |cells|::Cell { word: 31 };
    let value: u32 = |cells|::Loan::read(@cell);
}"#;
    let scratch = project::stage(
        "named-independent-abi",
        &[
            ("supplier/build.omg", &package),
            ("supplier/main.omg", supplier),
            ("application/build.omg", &build),
            ("application/main.omg", main),
        ],
    );
    let root = scratch.path().join("application");
    let (built, report) = project::status(&project::omega(
        &root,
        &[
            "--target",
            "linux_x86_64",
            "--build-dir",
            "out",
            "build.omg",
        ],
    ));
    assert!(built, "{report}");
    let components = std::fs::read_to_string(root.join("out/omega-components.tsv")).unwrap();
    assert!(components.contains(".so"), "{components}");
    let ran = std::process::Command::new(root.join("out/omega-program"))
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(ran.status.success(), "{ran:?}");
}
