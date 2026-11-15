//! Indexed reads and length observations share the exact stored or reborrowed
//! slice descriptor after the published program is reopened.

use std::path::{Path, PathBuf};
use terminal_codec::{CanonicalTerminalArtifact, ClosedTerminalModule, CodecError};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInstantiation, TerminalProgram,
};
use terminal_psi::{
    OperationKind, StructuralPathSegment, StructuralTypeShape, TerminalIdentityIndex,
    TerminalModule,
};
use terminal_work::WorkMeter;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .join("tests/fixtures")
}

#[test]
fn reopened_byte_views_read_the_original_backing_after_arrival() {
    for fixture in [
        "mutable_byte_view_arrival",
        "mutable_byte_view_short_circuit",
    ] {
        let root = fixtures().join("run/views").join(fixture).join("main.omg");
        let checked = orchestration::check_source(
            &mut compiler::CompilationSession::default(),
            compiler::CheckedCompileRequest::new(&root, Some("macos_arm64")),
        )
        .result
        .unwrap_or_else(|failure| panic!("{fixture}: {:?}", failure.into_diagnostics()));
        let closure = checked.terminal_publication().accepted().unwrap().closure();
        let published = closure.publish().unwrap();
        let reopened = CanonicalTerminalArtifact::from_bytes(&published.artifact.to_bytes())
            .expect("reopen the checked program through the artifact reader");
        let program = TerminalProgram::borrow(reopened.view());
        let mut execution = program
            .instantiate(TerminalInstantiation::default())
            .unwrap();
        assert_eq!(
            execution.invoke(&mut WorkMeter::unbounded()).unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit),
            "{fixture} must reach the successful byte-equality branch"
        );
    }
}

#[test]
fn arrival_index_equal_to_exact_view_length_refuses() {
    let root = fixtures().join("fail/views/mutable_byte_view_arrival_bounds/main.omg");
    let diagnostics = orchestration::check_source(
        &mut compiler::CompilationSession::default(),
        compiler::CheckedCompileRequest::new(&root, Some("macos_arm64")),
    )
    .result
    .expect_err("index two exceeds the same two-byte descriptor")
    .into_diagnostics();
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove operation safety in machine `inspect`")
            && diagnostic
                .message
                .contains("the bounds do not decide the comparison")),
        "{diagnostics:?}"
    );
}

#[test]
fn saved_field_element_read_requires_the_projected_descriptor_and_its_count() {
    let source = fixtures().join("run/views/typed_slice_field_index/main.omg");
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&source, Some("macos_arm64")),
            )
            .result
            .unwrap_or_else(|failure| panic!("{:?}", failure.into_diagnostics()));
            let accepted = checked.terminal_publication().accepted().unwrap();
            let publication = accepted.closure().publish().unwrap();
            let bytes = publication.artifact.semantic_bytes();
            let reopened = ClosedTerminalModule::from_semantic_bytes(bytes).unwrap();
            assert_eq!(bytes, reopened.semantic_bytes());

            let peer = sibling_descriptor(reopened.module());
            let mut substituted_descriptor = reopened.module().clone();
            let OperationKind::ElementViewRead { source, .. } =
                element_read(&mut substituted_descriptor)
            else {
                unreachable!("selected element read");
            };
            assert!(matches!(
                source.path.as_slice(),
                [StructuralPathSegment::Field(_)]
            ));
            source.path[0] = StructuralPathSegment::Field(peer);
            refuses_wrong_measurement(substituted_descriptor);

            let mut substituted_count = reopened.module().clone();
            let guard_count = substituted_count
                .operations()
                .find(|operation| matches!(operation.kind, OperationKind::ElementViewLength { .. }))
                .and_then(|operation| operation.result.scalar_ref())
                .expect("the guard observes the same field before the read")
                .id;
            let OperationKind::ElementViewRead { length, .. } =
                element_read(&mut substituted_count)
            else {
                unreachable!("selected element read");
            };
            assert_ne!(*length, guard_count);
            *length = guard_count;
            refuses_wrong_measurement(substituted_count);

            for fixture in [
                "typed_slice_field_index_out_of_bounds",
                "typed_slice_field_index_wrong_descriptor",
            ] {
                let source = fixtures().join("fail/views").join(fixture).join("main.omg");
                let diagnostics = orchestration::check_source(
                    &mut compiler::CompilationSession::default(),
                    compiler::CheckedCompileRequest::new(&source, Some("macos_arm64")),
                )
                .result
                .expect_err("the authored guard cannot prove this read's bound")
                .into_diagnostics();
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic
                        .message
                        .contains("cannot prove operation safety in machine `pick`")
                        && diagnostic
                            .message
                            .contains("the bounds do not decide the comparison")),
                    "{fixture}: {diagnostics:?}",
                );
            }
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    });
}

fn sibling_descriptor(module: &TerminalModule) -> TerminalIdentityIndex {
    let (root, observed) = module
        .machines
        .iter()
        .find_map(|machine| {
            let source = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|operation| match &operation.kind {
                    OperationKind::ElementViewRead { source, .. } => Some(source),
                    _ => None,
                })?;
            let [StructuralPathSegment::Field(observed)] = source.path.as_slice() else {
                panic!("the element read must use the stored field");
            };
            Some((
                machine.structural_place_type(source.place).unwrap(),
                *observed,
            ))
        })
        .expect("the guarded access must publish an element read");
    let StructuralTypeShape::Record { fields, .. } = &module.structural_type(root).unwrap().shape
    else {
        panic!("the element view must be stored in a record");
    };
    assert_eq!(fields.len(), 2, "the fixture must retain both slice fields");
    fields
        .iter()
        .find(|field| field.identity != observed)
        .expect("the fixture must retain a different descriptor with the same element type")
        .identity
}

fn element_read(module: &mut TerminalModule) -> &mut OperationKind {
    module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .map(|operation| &mut operation.kind)
        .find(|operation| matches!(operation, OperationKind::ElementViewRead { .. }))
        .expect("the guarded access must publish an element read")
}

fn refuses_wrong_measurement(module: TerminalModule) {
    let modified = ClosedTerminalModule::new(module).unwrap();
    let refusal = ClosedTerminalModule::from_semantic_bytes(modified.semantic_bytes()).unwrap_err();
    assert!(
        matches!(
            refusal,
            CodecError::MalformedOperationFoundation {
                reason: "element-view read length does not measure its exact descriptor",
                ..
            }
        ),
        "{refusal:?}",
    );
}

#[test]
fn fixed_array_element_window_reopens_with_its_exact_measurement() {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let root = fixtures().join("run/views/typed_array_partial_window/main.omg");
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&root, Some("macos_arm64")),
            )
            .result
            .unwrap_or_else(|failure| panic!("{:?}", failure.into_diagnostics()));
            let accepted = checked.terminal_publication().accepted().unwrap();
            let published = accepted.closure().publish().unwrap();
            let reopened = CanonicalTerminalArtifact::from_bytes(&published.artifact.to_bytes())
                .expect("reopen the typed array window");
            let program = TerminalProgram::borrow(reopened.view());
            assert_eq!(
                program.instantiate(TerminalInstantiation::default()).unwrap()
                    .invoke(&mut WorkMeter::unbounded()).unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit),
                "the reopened window must observe the second element of the original array",
            );

            let mut substituted_count = reopened.semantic_module().clone();
            let subslice = substituted_count
                .machines
                .iter_mut()
                .flat_map(|machine| &mut machine.blocks)
                .flat_map(|block| &mut block.operations)
                .map(|operation| &mut operation.kind)
                .find(|kind| matches!(kind, OperationKind::ElementViewSubslice { .. }))
                .expect("a partial typed array borrow retains one element-view subslice");
            let OperationKind::ElementViewSubslice { end, length, .. } = subslice else {
                unreachable!("selected element subslice");
            };
            assert_ne!(*length, *end);
            // The literal endpoint is equal to the array's extent. Its
            // scalar value cannot stand in for the measured descriptor.
            *length = *end;
            let modified = ClosedTerminalModule::new(substituted_count).unwrap();
            let refusal = ClosedTerminalModule::from_semantic_bytes(modified.semantic_bytes())
                .unwrap_err();
            assert!(matches!(
                refusal,
                CodecError::MalformedOperationFoundation {
                    reason: "element-view subslice length does not measure its exact descriptor",
                    ..
                }
            ), "{refusal:?}");

            let root = fixtures().join("fail/views/typed_array_partial_window_past_end/main.omg");
            let diagnostics = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&root, Some("macos_arm64")),
            )
            .result
            .expect_err("the end of the window exceeds the exact array extent")
            .into_diagnostics();
            assert!(diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("cannot prove operation safety in machine `main`")
                    && diagnostic.message.contains("the bounds do not decide the comparison")
            }), "{diagnostics:?}");
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    });
}
