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
//! `SelectedIncomingBitwiseAndOnesIdentityCopy` joins them below: the
//! provider body's `value & u64::MAX` materializes the all-ones literal
//! into the right `Use` operand of the `BitwiseAndI64` consumer —
//! `MaterializeI64` feeding `BitwiseAndI64`, the catalog's
//! `BITWISE_AND_ONES_COPIES` pair rules, which rewrite the consumer to a
//! `CopyI64` of the surviving operand register at the result register —
//! all-ones is the bitwise-and identity element, so `x & MAX` is `x` —
//! again dropping the flag clobber the subtract-shared constraint row
//! carries on x86-64. The family shares its consumer kind and operand
//! grammar with the and-zero annihilator rules; the two families stay
//! disjoint on the literal's value.
//! `SelectedIncomingWrappingRemainderZeroDividendZeroMaterialization`
//! joins them below: the provider body's guarded `0 % value` on a signed
//! wrapping carrier materializes the zero literal into the dividend `Use`
//! operand of the `WrappingRemainderI64` consumer — `MaterializeI64`
//! feeding `WrappingRemainderI64`, the catalog's
//! `WRAPPING_REMAINDER_ZERO_DIVIDEND_MATERIALIZE` pair rule, which rewrites
//! the pair to a `MaterializeI64` of zero at the result register — `0 % x`
//! is `0` for every `x`. Unlike the divisor-one fold, the folded literal
//! is not the value that discharges the consumer's encoded architectural
//! fault: the rule declares `FaultDischargedByObligation`, so the constant
//! quotient is fixed by the literal while the consumer's carried nonzero
//! divisor obligation already excludes the only reachable fault.
//! `SelectedIncomingU12Load8IndexedOffset` joins them below: the free
//! `Main::walk` body's guarded `data[3u64]` on a borrowed byte-view
//! parameter lowers through `StructuralParameterIndexedRead` to
//! `ByteSequenceRead`, whose selection emits a `MaterializeI64` of the
//! literal index feeding operand 1 of `Load8Indexed` — the catalog's
//! `LOAD8_INDEXED_U12` pair rule's source-reachable candidate, which
//! rewrites the pair to a direct-offset `Load8` when the literal's
//! register is the incoming spill victim. A scalar-returning boundary
//! operator cannot carry the structural argument this family needs —
//! ordinary scalar-result calls with structural arguments require a
//! registered scalar target — so the candidate rides a unit-returning
//! free machine invoked behind a nested `Main::invoke`: routing the
//! established view through that frame keeps `Main::main`'s register
//! pressure from evicting the view's `FrameAddress` resident, an
//! `ActiveResident` victim no fold grammar admits.
//! `SelectedIncomingExactDivideZeroDividendZeroMaterialization` joins them
//! below: the provider body's guarded `0 / value` on an unsigned carrier
//! materializes the zero literal into the dividend `Use` operand of the
//! `ExactDivideU64` consumer — `MaterializeI64` feeding `ExactDivideU64`,
//! the catalog's `EXACT_DIVIDE_ZERO_DIVIDEND_MATERIALIZE` pair rule, which
//! rewrites the pair to a `MaterializeI64` of zero at the result register
//! under the same `FaultDischargedByObligation` surface: `0 / x` is `0`
//! for every nonzero `x`, a quotient of zero cannot overflow, and the
//! divide-by-zero case the encoding could still name is unreachable under
//! the consumer's accepted nonzero-divisor obligation.

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

#[test]
fn selected_lowering_bitwise_and_ones_occurrence_replays_one_exact_physical_child() {
    // An eleventh selected-lowering family carries the physical-child
    // contract: the package-bound boundary operator program compiles under
    // `SelectedIncomingBitwiseAndOnesIdentityCopy`, and the provider body's
    // `value & u64::MAX` gives the catalog's `BITWISE_AND_ONES_COPIES` pair
    // rules a real source-reachable candidate — the all-ones literal
    // materializes and feeds the right `Use` operand of the
    // `BitwiseAndI64` consumer, a `MaterializeI64` feeding `BitwiseAndI64`
    // that the rule rewrites to a `CopyI64` of the surviving operand
    // register at the consumer's result register (all-ones is the
    // bitwise-and identity element, so `x & MAX` is `x`), again dropping
    // the flag clobber the subtract-shared constraint row carries on
    // x86-64. The family shares its consumer kind and operand grammar with
    // the and-zero annihilator rules; the two families stay disjoint on
    // the literal's value. The surviving operator occurrence still binds
    // exactly one OperatorApplicationCoverage child through allocation,
    // layout, and native emission; independent replay rejects every
    // mutation class — missing, duplicate, stale, substituted, padded,
    // and role-swapped children.
    let root = std::env::temp_dir().join(format!(
        "omega-physical-child-bitwise-and-ones-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create bitwise-and-ones physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::mask_ones(value: u64) -> u64;

data CheckedMathProvider {}

machine CheckedMathProvider::mask_ones_impl(value: u64) -> u64
satisfies CheckedMath::mask_ones
{
    transition { _ -> (value & 18446744073709551615u64) }
}

data Main {}

machine Main::main(&mut self) {
    let picked: u64 = CheckedMath::mask_ones(7u64);
}
"#,
    )
    .expect("write bitwise-and-ones physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-bitwise-and-ones-physical-child");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingBitwiseAndOnesIdentityCopy);
}
"#,
    )
    .expect("write bitwise-and-ones physical-child build");
    let root_identity = package_identity(53);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("bitwise-and-ones physical-child package graph should validate");
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
    .expect("the bitwise-and-ones selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("bitwise-and-ones compilation retains its native artifact");
    artifact
        .validate()
        .expect("bitwise-and-ones native artifact should replay independently");
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
fn selected_lowering_wrapping_remainder_zero_dividend_occurrence_replays_one_exact_physical_child()
{
    // A twelfth selected-lowering family carries the physical-child
    // contract: the package-bound boundary operator program compiles under
    // `SelectedIncomingWrappingRemainderZeroDividendZeroMaterialization`,
    // and the provider body's guarded `0 % value` on a signed wrapping
    // carrier gives the catalog's
    // `WRAPPING_REMAINDER_ZERO_DIVIDEND_MATERIALIZE` pair rule a real
    // source-reachable candidate — the zero literal materializes and feeds
    // the dividend `Use` operand of the `WrappingRemainderI64` consumer, a
    // `MaterializeI64` feeding `WrappingRemainderI64` that the rule
    // rewrites to a `MaterializeI64` of zero at the consumer's result
    // register (`0 % x` is `0` for every `x`). Unlike the divisor-one
    // fold, the folded literal is not the value that discharges the
    // consumer's encoded architectural fault — the rule declares
    // `FaultDischargedByObligation` because the constant quotient is fixed
    // by the literal while the consumer's carried nonzero-divisor
    // obligation already excludes the only reachable fault, so the guard
    // on the provider keeps the divisor's definedness proven. The
    // surviving operator occurrence still binds exactly one
    // OperatorApplicationCoverage child through allocation, layout, and
    // native emission; independent replay rejects every mutation class —
    // missing, duplicate, stale, substituted, padded, and role-swapped
    // children. linux_arm64 is the declared target for the same reason as
    // the divisor-one family: the x86-64 remainder row's early-clobber
    // RDX scratch next to the late RAX result is not yet admitted by
    // live-range replay (`UnsupportedEarlyClobber`), while the AArch64
    // SDIV/MSUB row carries the admitted single-definition early-clobber
    // shape.
    let root = std::env::temp_dir().join(format!(
        "omega-physical-child-wrapping-remainder-zero-dividend-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root)
        .expect("create wrapping-remainder-zero-dividend physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::zero_mod(value: i64 in Wrapping) -> i64 in Wrapping;

data CheckedMathProvider {}

machine CheckedMathProvider::zero_mod_impl(value: i64 in Wrapping) -> i64 in Wrapping
satisfies CheckedMath::zero_mod
{
    transition value != 0 {
        true -> (0 % value)
        false -> 0
    }
}

data Main {}

machine Main::main(&mut self) {
    let picked: i64 in Wrapping = CheckedMath::zero_mod(7 as i64 in Wrapping);
}
"#,
    )
    .expect("write wrapping-remainder-zero-dividend physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-wrapping-remainder-zero-dividend-physical-child");
    builder.roots.bind(linux_arm64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingWrappingRemainderZeroDividendZeroMaterialization);
}
"#,
    )
    .expect("write wrapping-remainder-zero-dividend physical-child build");
    let root_identity = package_identity(54);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("wrapping-remainder-zero-dividend physical-child package graph should validate");
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
    .expect("the wrapping-remainder-zero-dividend selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("wrapping-remainder-zero-dividend compilation retains its native artifact");
    artifact
        .validate()
        .expect("wrapping-remainder-zero-dividend native artifact should replay independently");
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
fn selected_lowering_exact_divide_zero_dividend_occurrence_replays_one_exact_physical_child() {
    // A thirteenth selected-lowering family carries the physical-child
    // contract: the package-bound boundary operator program compiles under
    // `SelectedIncomingExactDivideZeroDividendZeroMaterialization`, and
    // the provider body's guarded `0 / value` on an unsigned carrier gives
    // the catalog's `EXACT_DIVIDE_ZERO_DIVIDEND_MATERIALIZE` pair rule a
    // real source-reachable candidate — the zero literal materializes and
    // feeds the dividend `Use` operand of the `ExactDivideU64` consumer, a
    // `MaterializeI64` feeding `ExactDivideU64` that the rule rewrites to
    // a `MaterializeI64` of zero at the consumer's result register (`0 / x`
    // is `0` for every nonzero `x`, and a quotient of zero cannot
    // overflow). Like the wrapping-remainder sibling, the folded literal
    // is not the value that discharges the consumer's encoded
    // architectural fault — the rule declares `FaultDischargedByObligation`
    // because the divide-by-zero case the encoding could still name is
    // unreachable only under the `ExactDivideU64` kind's proven nonzero
    // divisor obligation, which the provider's own guard keeps proven.
    // The family shares its consumer kind with the divisor-one fold; the
    // grammars stay disjoint on the folded literal's operand position.
    // The surviving operator occurrence still binds exactly one
    // OperatorApplicationCoverage child through allocation, layout, and
    // native emission; independent replay rejects every mutation class —
    // missing, duplicate, stale, substituted, padded, and role-swapped
    // children. linux_arm64 is the declared target here even though the
    // divisor-one family runs x86-64: under the dividend guard the x86-64
    // divide row's pinned-operand pressure shape is not yet admitted by
    // spill-choice replay (`UnsupportedPressureShape`), while the AArch64
    // `udiv` row carries no pinned scratch tail at all.
    let root = std::env::temp_dir().join(format!(
        "omega-physical-child-exact-divide-zero-dividend-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root)
        .expect("create exact-divide-zero-dividend physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::zero_over(value: u64) -> u64;

data CheckedMathProvider {}

machine CheckedMathProvider::zero_over_impl(value: u64) -> u64
satisfies CheckedMath::zero_over
{
    transition value != 0 {
        true -> (0 / value)
        false -> 0
    }
}

data Main {}

machine Main::main(&mut self) {
    let picked: u64 = CheckedMath::zero_over(7u64);
}
"#,
    )
    .expect("write exact-divide-zero-dividend physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-exact-divide-zero-dividend-physical-child");
    builder.roots.bind(linux_arm64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingExactDivideZeroDividendZeroMaterialization);
}
"#,
    )
    .expect("write exact-divide-zero-dividend physical-child build");
    let root_identity = package_identity(55);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("exact-divide-zero-dividend physical-child package graph should validate");
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
    .expect("the exact-divide-zero-dividend selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("exact-divide-zero-dividend compilation retains its native artifact");
    artifact
        .validate()
        .expect("exact-divide-zero-dividend native artifact should replay independently");
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
fn selected_lowering_load8_indexed_occurrence_replays_one_exact_physical_child() {
    // `SelectedIncomingU12Load8IndexedOffset` joins the catalog: the free
    // `Main::walk` body's guarded `data[3u64]` on a borrowed byte-view
    // parameter lowers through `StructuralParameterIndexedRead` to
    // `ByteSequenceRead`, whose selection emits a `MaterializeI64` of the
    // literal index feeding operand 1 of `Load8Indexed` — the catalog's
    // `LOAD8_INDEXED_U12` pair rule's source-reachable candidate, folding
    // to a direct-offset `Load8` under pressure. The application routes
    // the literal view through a nested `Main::invoke` so `Main::main`
    // carries only the boundary call and a unit call: a direct
    // `Main::walk("hello")` beside the operator local leaves the
    // established view's `FrameAddress` resident across the scalar call,
    // and the spill choice evicts that resident — an `ActiveResident`
    // victim the fold pass cannot recover. The surviving operator
    // occurrence still binds exactly one OperatorApplicationCoverage child
    // through allocation, layout, and native emission; independent replay
    // rejects every mutation class — missing, duplicate, stale,
    // substituted, padded, and role-swapped children.
    let root = std::env::temp_dir().join(format!(
        "omega-physical-child-load8-indexed-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create load8-indexed physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::select_left(left: u64, right: u64) -> u64;

data CheckedMathProvider {}

machine CheckedMathProvider::select_left_impl(left: u64, right: u64) -> u64
satisfies CheckedMath::select_left
{
    transition { _ -> (left) }
}

data Main {}

machine Main::walk(data: &[u8]) {
    transition 3u64 < data.len {
        true -> done(data[3u64])
        _ -> done(0u8)
    }

    state done(b: u8) { }
}

machine Main::invoke() {
    Main::walk("hello");
}

machine Main::main(&mut self) {
    let picked: u64 = CheckedMath::select_left(7u64, 9u64);
    Main::invoke();
}
"#,
    )
    .expect("write load8-indexed physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-load8-indexed-physical-child");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12Load8IndexedOffset);
}
"#,
    )
    .expect("write load8-indexed physical-child build");
    let root_identity = package_identity(56);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("load8-indexed physical-child package graph should validate");
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
    .expect("the load8-indexed selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("load8-indexed compilation retains its native artifact");
    artifact
        .validate()
        .expect("load8-indexed native artifact should replay independently");
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
fn structural_result_operator_occurrence_replays_one_exact_physical_child() {
    // A checked boundary operator application whose realization returns a
    // structural result lowers to `CallStructuralWithScalarArguments`, which
    // emitted the same resolved internal machine call as the scalar call
    // kinds. The surviving operator occurrence still binds exactly one
    // OperatorApplicationCoverage child through allocation, layout, and
    // native emission; independent replay rejects every mutation class —
    // missing, duplicate, stale, substituted, padded, and role-swapped
    // children.
    let root = std::env::temp_dir().join(format!(
        "omega-physical-child-structural-operator-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create structural-operator physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data Cell {
    value: i64;
}

data GenericMath {}

boundary operator GenericMath::pick(items: Cell, at: i64) -> Cell;

data GenericProvider {}

machine GenericProvider::pick(items: Cell, at: i64) -> Cell
satisfies GenericMath::pick
{
    items
}

machine consume(items: Cell) {
    let selected: Cell = GenericMath::pick(items, 2);
}

data Main {}

machine Main::main(&mut self) {
    let items: Cell = Cell { value: 7 };
    consume(items);
}
"#,
    )
    .expect("write structural-operator physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("structural-operator-physical-child");
    builder.roots.bind(linux_arm64::ProgramEntry, Main::main);
}
"#,
    )
    .expect("write structural-operator physical-child build");
    let root_identity = package_identity(53);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("structural-operator physical-child package graph should validate");
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
    .expect("the structural-operator selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("structural-operator compilation retains its native artifact");
    artifact
        .validate()
        .expect("structural-operator native artifact should replay independently");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact.physical_evidence().unwrap_or_else(|| {
        panic!(
            "the surviving boundary occurrence retains nonempty physical evidence; gap: {:?}",
            artifact.physical_evidence_gap()
        )
    });
    let [occurrence] = physical.projection().operator_occurrences() else {
        panic!("the structural boundary operator must survive as exactly one operator occurrence")
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
fn integer_comparison_occurrence_replays_one_exact_physical_child() {
    // A checked `==` application bound to a compiler-intrinsic provider lowers
    // to the intrinsic `IntegerEqual` operation — an instruction-only
    // operation with no dedicated emitted roster. The surviving operator
    // occurrence still binds exactly one OperatorApplicationCoverage child
    // whose span is the attributed instruction bytes; independent replay
    // rejects every mutation class — missing, duplicate, stale, substituted,
    // padded, and role-swapped children.
    let root = std::env::temp_dir().join(format!(
        "omega-physical-child-integer-comparison-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create integer-comparison physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"boundary machine == Comparison::equal(left: i32, right: i32) -> bool;

data ComparisonProvider {}

machine ComparisonProvider::equal(left: i32, right: i32) -> bool
    satisfies Comparison::equal
    via Binding::CompilerIntrinsic;

data Main {}

machine Main::main(&mut self) {
    let _hit: bool = probe(3, 7);
}

machine probe(left: i32, right: i32) -> bool {
    left == right
}
"#,
    )
    .expect("write integer-comparison physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("integer-comparison-physical-child");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
    )
    .expect("write integer-comparison physical-child build");
    let root_identity = package_identity(57);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("integer-comparison physical-child package graph should validate");
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
    .expect("the integer-comparison selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("integer-comparison compilation retains its native artifact");
    artifact
        .validate()
        .expect("integer-comparison native artifact should replay independently");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact.physical_evidence().unwrap_or_else(|| {
        panic!(
            "the surviving comparison occurrence retains nonempty physical evidence; gap: {:?}",
            artifact.physical_evidence_gap()
        )
    });
    let [occurrence] = physical.projection().operator_occurrences() else {
        panic!("the integer comparison must survive as exactly one operator occurrence")
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
        native_realization::PhysicalRelocationDisposition::DirectInstructionBytes
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
    // Padded: the machine span must name exactly the attributed instruction
    // interval.
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
