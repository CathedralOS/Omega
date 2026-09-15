//! Physical-child custody replay for the selected-lowering catalog.
//!
//! Each covered boundary-operator application in a real compilation must bind
//! exactly one physical child — its operator-application or
//! boundary-settlement parent, the validated projection identity, and nonempty
//! machine/object spans — through allocation, layout, native emission, and
//! independent replay. The sibling coverage lives in the compiler crate's
//! `optimizer_opt_in/selected_lowering_replays.rs`; this file carries the same
//! contract for `SelectedIncomingWrappingRemainderOneZeroMaterialization`: the
//! provider body's `value % 1` on a signed wrapping carrier gives the
//! catalog's `WRAPPING_REMAINDER_ONE_MATERIALIZE` pair rule a real
//! source-reachable candidate — `x % 1` is always zero, signed or unsigned, so
//! the literal producer and the pinned remainder consumer rewrite to a
//! `MaterializeI64` of zero.

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct};
use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use semantic_vocabulary::PackageKeyIdentity;
use std::sync::atomic::{AtomicU64, Ordering};

static PROJECT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero package identity")
}

fn replay_native_artifact_parts(
    parts: &native_realization::NativeArtifactParts,
) -> native_realization::NativeArtifactParts {
    let module = terminal_codec::decode_module(parts.psi_artifact.semantic_bytes())
        .expect("replay Terminal semantics");
    let proof = terminal_codec::decode_proof_bundle(parts.psi_artifact.proof_bytes())
        .expect("replay Terminal proof");
    let debug = parts
        .psi_artifact
        .debug_bytes()
        .map(|bytes| terminal_codec::decode_debug_map(&module, bytes).expect("debug map"));
    native_realization::NativeArtifactParts {
        target: parts.target,
        psi_artifact: terminal_codec::CanonicalTerminalArtifact::from_parts(
            &module,
            &proof,
            parts.psi_artifact.optimization(),
            debug.as_ref(),
        )
        .expect("reconstruct canonical Terminal artifact"),
        object: parts.object.clone(),
        image: parts.image.clone(),
        selected_provider_closure_report_identity: parts.selected_provider_closure_report_identity,
        selected_provider_closure_digest: parts.selected_provider_closure_digest,
        selected_provider_plans: parts.selected_provider_plans.clone(),
        provider_executions: parts.provider_executions.clone(),
        terminal_authority_policy_identity: parts.terminal_authority_policy_identity,
        terminal_authority_permission_policy_identity: parts
            .terminal_authority_permission_policy_identity,
        terminal_authority_closure_review: parts.terminal_authority_closure_review.clone(),
        boundary_application_coverage: parts.boundary_application_coverage.clone(),
        physical_evidence_scope: parts.physical_evidence_scope.clone(),
        physical_evidence: parts.physical_evidence.clone(),
    }
}

#[test]
fn selected_lowering_wrapping_remainder_occurrence_replays_one_exact_physical_child() {
    // A fifth selected-lowering family carries the physical-child contract:
    // the package-bound boundary operator program compiles under
    // `SelectedIncomingWrappingRemainderOneZeroMaterialization`, and the
    // provider body's `value % 1` on a signed wrapping carrier gives the
    // catalog's `WRAPPING_REMAINDER_ONE_MATERIALIZE` pair rule a real
    // source-reachable candidate. The surviving operator occurrence still
    // binds exactly one OperatorApplicationCoverage child through allocation,
    // layout, and native emission; independent replay rejects every mutation
    // class — missing, duplicate, stale, substituted, padded, and
    // role-swapped children. linux_arm64 is the declared target here because
    // the x86-64 remainder row's early-clobber RDX scratch next to the
    // late RAX result is not yet admitted by live-range replay
    // (`UnsupportedEarlyClobber`); the AArch64 SDIV/MSUB row carries the
    // admitted single-definition early-clobber shape.
    let root = std::env::temp_dir().join(format!(
        "omega-physical-child-wrapping-remainder-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create wrapping-remainder physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::mod_one(value: i64 in Wrapping) -> i64 in Wrapping;

data CheckedMathProvider {}

machine CheckedMathProvider::mod_one_impl(value: i64 in Wrapping) -> i64 in Wrapping
satisfies CheckedMath::mod_one
{
    transition { _ -> (value % 1) }
}

data Main {}

machine Main::main(&mut self) {
    let picked: i64 in Wrapping = CheckedMath::mod_one(7 as i64 in Wrapping);
}
"#,
    )
    .expect("write wrapping-remainder physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-wrapping-remainder-physical-child");
    builder.roots.bind(linux_arm64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingWrappingRemainderOneZeroMaterialization);
}
"#,
    )
    .expect("write wrapping-remainder physical-child build");
    let root_identity = package_identity(47);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("wrapping-remainder physical-child package graph should validate");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(root.join("build")),
            target_name: Some("linux_arm64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_package_inputs(inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the wrapping-remainder selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("wrapping-remainder compilation retains its native artifact");
    artifact
        .validate()
        .expect("wrapping-remainder native artifact should replay independently");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact
        .physical_evidence()
        .expect("the surviving boundary occurrence retains nonempty physical evidence");
    let [occurrence] = physical.projection().operator_occurrences() else {
        panic!("the checked boundary operator must survive as exactly one operator occurrence")
    };
    assert!(physical.projection().boundary_occurrences().is_empty());
    let [child] = physical.children() else {
        panic!("the surviving occurrence must bind exactly one physical child")
    };
    assert_eq!(
        child.occurrence(),
        native_realization::NativePhysicalOccurrence::Operator(occurrence.identity())
    );
    assert_eq!(child.projection(), physical.projection().identity());
    assert!(matches!(
        child.parent(),
        native_realization::PhysicalChildParent::OperatorApplicationCoverage(_)
    ));
    assert!(child.machine_span().byte_count() > 0);
    assert!(child.object_span().byte_count() > 0);
    assert_eq!(
        child.relocation(),
        native_realization::PhysicalRelocationDisposition::ResolvedInternalCall
    );

    // Independent replay from the published parts alone: every mutation
    // class must fail closed.
    let parts = report
        .into_retained_native_artifact()
        .expect("owned native artifact")
        .into_parts();

    let mut missing = replay_native_artifact_parts(&parts);
    let evidence = missing
        .physical_evidence
        .take()
        .expect("replay physical evidence")
        .into_parts();
    missing.physical_evidence = Some(
        native_realization::NativePhysicalEvidence::from_replayed_parts(
            native_realization::NativePhysicalEvidenceParts {
                projection: evidence.projection,
                children: Vec::new(),
                identity: evidence.identity,
            },
        ),
    );
    assert!(
        native_realization::NativeArtifact::from_replayed_parts(missing).is_err(),
        "a missing physical child must not replay"
    );

    let mut duplicate = replay_native_artifact_parts(&parts);
    let evidence = duplicate
        .physical_evidence
        .take()
        .expect("replay physical evidence")
        .into_parts();
    let [only_child] = evidence.children.as_slice() else {
        panic!("one physical child before duplication")
    };
    duplicate.physical_evidence = Some(
        native_realization::NativePhysicalEvidence::from_replayed_parts(
            native_realization::NativePhysicalEvidenceParts {
                projection: evidence.projection,
                children: vec![only_child.clone(), only_child.clone()],
                identity: evidence.identity,
            },
        ),
    );
    assert!(
        native_realization::NativeArtifact::from_replayed_parts(duplicate).is_err(),
        "a duplicate physical child must not replay"
    );

    let assert_mutated_child_rejected =
        |mutate: &dyn Fn(&mut native_realization::NativePhysicalChildParts)| {
            let mut replay = replay_native_artifact_parts(&parts);
            let evidence = replay
                .physical_evidence
                .take()
                .expect("replay physical evidence")
                .into_parts();
            let [child] = evidence.children.as_slice() else {
                panic!("one physical child before mutation")
            };
            let mut child = child.clone().into_parts();
            mutate(&mut child);
            replay.physical_evidence = Some(
                native_realization::NativePhysicalEvidence::from_replayed_parts(
                    native_realization::NativePhysicalEvidenceParts {
                        projection: evidence.projection,
                        children: vec![
                            native_realization::NativePhysicalChild::from_replayed_parts(child),
                        ],
                        identity: evidence.identity,
                    },
                ),
            );
            assert!(
                native_realization::NativeArtifact::from_replayed_parts(replay).is_err(),
                "a mutated physical child must not replay"
            );
        };
    // Role-swapped: the operator occurrence cannot be re-presented as a
    // boundary occurrence.
    assert_mutated_child_rejected(&|child| {
        assert!(matches!(
            child.parent,
            native_realization::PhysicalChildParent::OperatorApplicationCoverage(_)
        ));
        child.occurrence = native_realization::NativePhysicalOccurrence::Boundary(
            optimization_core::OptimizedBoundaryOccurrenceIdentity::from_bytes(
                child.occurrence.identity(),
            ),
        );
    });
    // Padded: the machine span must name exactly the emitted call interval.
    assert_mutated_child_rejected(&|child| {
        child.machine_span = native_realization::NativeByteSpan::from_replayed_parts(
            child.machine_span.offset(),
            child.machine_span.byte_count() + 1,
        );
    });
    // Substituted: the child must bind the validated projection identity.
    assert_mutated_child_rejected(&|child| {
        child.projection =
            optimization_core::NativeOptimizationProjectionIdentity::from_bytes([0x5A; 32]);
    });
    // Stale: an occurrence identity no surviving projection names cannot carry
    // a child.
    assert_mutated_child_rejected(&|child| {
        child.occurrence = native_realization::NativePhysicalOccurrence::Operator(
            optimization_core::OptimizedOperatorOccurrenceIdentity::from_bytes([0xA7; 32]),
        );
    });
}
