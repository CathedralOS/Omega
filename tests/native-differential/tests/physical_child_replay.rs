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
//! `MaterializeI64` of zero. `SelectedIncomingLiteralCopyMaterialization`
//! joins it below: the provider's literal return `transition { _ -> 5u64 }`
//! materializes the constant and then copies it into the return-channel
//! register — `MaterializeI64` feeding `CopyI64`, the catalog's
//! `COPY_LITERAL_FOLD` unary rule — while the call site's `7u64` argument
//! materializes and copies into the pinned argument register.
//! `SelectedIncomingBitwiseAndZeroMaterialization` joins them below: the
//! provider body's `value & 0` materializes the zero literal into the right
//! `Use` operand of the `BitwiseAndI64` consumer — `MaterializeI64` feeding
//! `BitwiseAndI64`, the catalog's `BITWISE_AND_ZERO_MATERIALIZE` pair rule,
//! which rewrites the consumer to a `MaterializeI64` of zero at the result
//! register, dropping the surviving-side `Use` and the flag clobber the
//! subtract-shared constraint row carries on x86-64.
//! `SelectedIncomingBitwiseXorZeroIdentityCopy` joins them below: the
//! provider body's `value ^ 0` materializes the zero literal into the right
//! `Use` operand of the `BitwiseXorI64` consumer — `MaterializeI64` feeding
//! `BitwiseXorI64`, the catalog's `BITWISE_XOR_ZERO_COPIES` pair rules, which
//! rewrite the consumer to a `CopyI64` of the surviving operand register at
//! the result register — zero is the bitwise-xor identity element, so `x ^ 0`
//! is `x` — again dropping the flag clobber the subtract-shared constraint
//! row carries on x86-64.
//! `SelectedIncomingLiteralExtensionElimination` joins them below: the
//! provider body's `let bounded: u64 [0..=100] = 7u64` materializes the
//! literal into a scalar local, and `bounded as u8` on that declared range
//! lowers to `IntegerExactCast`, whose selection is `ZeroExtendU8` consuming
//! the local's register — `MaterializeI64` feeding `ZeroExtendU8`, the
//! catalog's `ZERO_EXTEND_U8_LITERAL_FOLD` rule, which rewrites the
//! extension to a `MaterializeI64` of the literal's low eight bits. A
//! direct literal cast would instead retag the checked literal and never
//! reach selection; the `let` keeps the operand a `Local` checked
//! expression so the runtime extension survives to be folded.
//! `SelectedIncomingWrappingAddZeroIdentityCopy` joins them below: the
//! provider body's `value + 0` on a signed wrapping carrier materializes
//! the zero literal into the right `Use` operand of the `WrappingAddI64`
//! consumer — `MaterializeI64` feeding `WrappingAddI64`, the catalog's
//! `WRAPPING_ADD_ZERO_COPIES` pair rules, which rewrite the consumer to a
//! `CopyI64` of the surviving operand register at the result register —
//! zero is the additive identity under modulo-2^64 wrap, so `x + 0` is
//! `x`. The wrapping-add row is flag-transparent, so unlike the bitwise
//! forms there is no flag clobber to retire.

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

#[test]
fn selected_lowering_literal_copy_occurrence_replays_one_exact_physical_child() {
    // A sixth selected-lowering family carries the physical-child contract:
    // the package-bound boundary operator program compiles under
    // `SelectedIncomingLiteralCopyMaterialization`, and the provider body's
    // `transition { _ -> 5u64 }` gives the catalog's `COPY_LITERAL_FOLD` pair
    // rule a real source-reachable candidate — the constant materializes and
    // the return-channel copy moves it into the ABI result register, a
    // `MaterializeI64` feeding `CopyI64` that the rule rewrites to a
    // `MaterializeI64` of the literal at the copy's destination register; the
    // call site's `7u64` argument materializes and copies into the pinned
    // argument register the same way. The surviving operator occurrence still
    // binds exactly one OperatorApplicationCoverage child through allocation,
    // layout, and native emission; independent replay rejects every mutation
    // class — missing, duplicate, stale, substituted, padded, and
    // role-swapped children.
    let root = std::env::temp_dir().join(format!(
        "omega-physical-child-literal-copy-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create literal-copy physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::echo(value: u64) -> u64;

data CheckedMathProvider {}

machine CheckedMathProvider::echo_impl(value: u64) -> u64
satisfies CheckedMath::echo
{
    transition { _ -> 5u64 }
}

data Main {}

machine Main::main(&mut self) {
    let picked: u64 = CheckedMath::echo(7u64);
}
"#,
    )
    .expect("write literal-copy physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-literal-copy-physical-child");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingLiteralCopyMaterialization);
}
"#,
    )
    .expect("write literal-copy physical-child build");
    let root_identity = package_identity(48);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("literal-copy physical-child package graph should validate");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(root.join("build")),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_package_inputs(inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the literal-copy selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("literal-copy compilation retains its native artifact");
    artifact
        .validate()
        .expect("literal-copy native artifact should replay independently");
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

#[test]
fn selected_lowering_bitwise_and_zero_occurrence_replays_one_exact_physical_child() {
    // A seventh selected-lowering family carries the physical-child contract:
    // the package-bound boundary operator program compiles under
    // `SelectedIncomingBitwiseAndZeroMaterialization`, and the provider body's
    // `value & 0` gives the catalog's `BITWISE_AND_ZERO_MATERIALIZE` pair rule
    // a real source-reachable candidate — the zero literal materializes and
    // feeds the right `Use` operand of the `BitwiseAndI64` consumer, a
    // `MaterializeI64` feeding `BitwiseAndI64` that the rule rewrites to a
    // `MaterializeI64` of zero at the consumer's result register. The
    // surviving operator occurrence still binds exactly one
    // OperatorApplicationCoverage child through allocation, layout, and
    // native emission; independent replay rejects every mutation class —
    // missing, duplicate, stale, substituted, padded, and role-swapped
    // children.
    let root = std::env::temp_dir().join(format!(
        "omega-physical-child-bitwise-and-zero-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create bitwise-and-zero physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::mask_zero(value: u64) -> u64;

data CheckedMathProvider {}

machine CheckedMathProvider::mask_zero_impl(value: u64) -> u64
satisfies CheckedMath::mask_zero
{
    transition { _ -> (value & 0) }
}

data Main {}

machine Main::main(&mut self) {
    let picked: u64 = CheckedMath::mask_zero(7u64);
}
"#,
    )
    .expect("write bitwise-and-zero physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-bitwise-and-zero-physical-child");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingBitwiseAndZeroMaterialization);
}
"#,
    )
    .expect("write bitwise-and-zero physical-child build");
    let root_identity = package_identity(49);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("bitwise-and-zero physical-child package graph should validate");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(root.join("build")),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_package_inputs(inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the bitwise-and-zero selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("bitwise-and-zero compilation retains its native artifact");
    artifact
        .validate()
        .expect("bitwise-and-zero native artifact should replay independently");
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

#[test]
fn selected_lowering_bitwise_xor_zero_occurrence_replays_one_exact_physical_child() {
    // An eighth selected-lowering family carries the physical-child contract:
    // the package-bound boundary operator program compiles under
    // `SelectedIncomingBitwiseXorZeroIdentityCopy`, and the provider body's
    // `value ^ 0` gives the catalog's `BITWISE_XOR_ZERO_COPIES` pair rules a
    // real source-reachable candidate — the zero literal materializes and
    // feeds the right `Use` operand of the `BitwiseXorI64` consumer, a
    // `MaterializeI64` feeding `BitwiseXorI64` that the rule rewrites to a
    // `CopyI64` of the surviving operand register at the consumer's result
    // register (zero is the bitwise-xor identity element, so `x ^ 0` is `x`),
    // dropping the flag clobber the subtract-shared constraint row carries on
    // x86-64. The surviving operator occurrence still binds exactly one
    // OperatorApplicationCoverage child through allocation, layout, and
    // native emission; independent replay rejects every mutation class —
    // missing, duplicate, stale, substituted, padded, and role-swapped
    // children.
    let root = std::env::temp_dir().join(format!(
        "omega-physical-child-bitwise-xor-zero-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create bitwise-xor-zero physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::xor_zero(value: u64) -> u64;

data CheckedMathProvider {}

machine CheckedMathProvider::xor_zero_impl(value: u64) -> u64
satisfies CheckedMath::xor_zero
{
    transition { _ -> (value ^ 0) }
}

data Main {}

machine Main::main(&mut self) {
    let picked: u64 = CheckedMath::xor_zero(7u64);
}
"#,
    )
    .expect("write bitwise-xor-zero physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-bitwise-xor-zero-physical-child");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingBitwiseXorZeroIdentityCopy);
}
"#,
    )
    .expect("write bitwise-xor-zero physical-child build");
    let root_identity = package_identity(50);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("bitwise-xor-zero physical-child package graph should validate");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(root.join("build")),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_package_inputs(inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the bitwise-xor-zero selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("bitwise-xor-zero compilation retains its native artifact");
    artifact
        .validate()
        .expect("bitwise-xor-zero native artifact should replay independently");
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

#[test]
fn selected_lowering_literal_extension_occurrence_replays_one_exact_physical_child() {
    // A ninth selected-lowering family carries the physical-child contract:
    // the package-bound boundary operator program compiles under
    // `SelectedIncomingLiteralExtensionElimination`, and the provider body's
    // `let bounded: u64 [0..=100] = 7u64` followed by `bounded as u8` gives
    // the catalog's `ZERO_EXTEND_U8_LITERAL_FOLD` pair rule a real
    // source-reachable candidate. The immutable `let` binds the literal to a
    // scalar local, so `bounded` reads back as a `Local` checked expression
    // rather than a literal the checker could retag; its materialization
    // (`MaterializeI64` of 7) feeds the `ZeroExtendU8` consumer the exact
    // cast selects, a `MaterializeI64` feeding `ZeroExtendU8` that the rule
    // rewrites to a `MaterializeI64` of the literal's low eight bits. The
    // surviving operator occurrence still binds exactly one
    // OperatorApplicationCoverage child through allocation, layout, and
    // native emission; independent replay rejects every mutation class —
    // missing, duplicate, stale, substituted, padded, and role-swapped
    // children.
    let root = std::env::temp_dir().join(format!(
        "omega-physical-child-literal-extension-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create literal-extension physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::narrow(value: u64) -> u8;

data CheckedMathProvider {}

machine CheckedMathProvider::narrow_impl(value: u64) -> u8
satisfies CheckedMath::narrow
{
    let bounded: u64 [0..=100] = 7u64;
    transition { _ -> (bounded as u8) }
}

data Main {}

machine Main::main(&mut self) {
    let picked: u8 = CheckedMath::narrow(7u64);
}
"#,
    )
    .expect("write literal-extension physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-literal-extension-physical-child");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingLiteralExtensionElimination);
}
"#,
    )
    .expect("write literal-extension physical-child build");
    let root_identity = package_identity(51);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("literal-extension physical-child package graph should validate");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(root.join("build")),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_package_inputs(inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the literal-extension selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("literal-extension compilation retains its native artifact");
    artifact
        .validate()
        .expect("literal-extension native artifact should replay independently");
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

#[test]
fn selected_lowering_wrapping_add_zero_occurrence_replays_one_exact_physical_child() {
    // A tenth selected-lowering family carries the physical-child contract:
    // the package-bound boundary operator program compiles under
    // `SelectedIncomingWrappingAddZeroIdentityCopy`, and the provider body's
    // `value + 0` on a signed wrapping carrier gives the catalog's
    // `WRAPPING_ADD_ZERO_COPIES` pair rules a real source-reachable
    // candidate — the zero literal materializes and feeds the right `Use`
    // operand of the `WrappingAddI64` consumer, a `MaterializeI64` feeding
    // `WrappingAddI64` that the rule rewrites to a `CopyI64` of the
    // surviving operand register at the consumer's result register (zero is
    // the additive identity under modulo-2^64 wrap, so `x + 0` is `x`). The
    // wrapping-add row is flag-transparent, so unlike the bitwise forms
    // there is no flag clobber to retire. The surviving operator occurrence
    // still binds exactly one OperatorApplicationCoverage child through
    // allocation, layout, and native emission; independent replay rejects
    // every mutation class — missing, duplicate, stale, substituted,
    // padded, and role-swapped children.
    let root = std::env::temp_dir().join(format!(
        "omega-physical-child-wrapping-add-zero-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create wrapping-add-zero physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::add_zero(value: i64 in Wrapping) -> i64 in Wrapping;

data CheckedMathProvider {}

machine CheckedMathProvider::add_zero_impl(value: i64 in Wrapping) -> i64 in Wrapping
satisfies CheckedMath::add_zero
{
    transition { _ -> (value + 0) }
}

data Main {}

machine Main::main(&mut self) {
    let picked: i64 in Wrapping = CheckedMath::add_zero(7 as i64 in Wrapping);
}
"#,
    )
    .expect("write wrapping-add-zero physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-wrapping-add-zero-physical-child");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingWrappingAddZeroIdentityCopy);
}
"#,
    )
    .expect("write wrapping-add-zero physical-child build");
    let root_identity = package_identity(52);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("wrapping-add-zero physical-child package graph should validate");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(root.join("build")),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_package_inputs(inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the wrapping-add-zero selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("wrapping-add-zero compilation retains its native artifact");
    artifact
        .validate()
        .expect("wrapping-add-zero native artifact should replay independently");
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
