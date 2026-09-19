use super::{
    PROJECT_SEQUENCE, compile_check, compile_native_and_publish, native_evidence_standard_library,
    package_identity, project, replay_native_artifact_parts,
};
use crate::{console_acceptance, linux_entry_acceptance};
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, OptimizationRollback,
    RequestedCompileProduct, compile_to_checked,
};
use optimization_core::Optimization;
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use std::sync::atomic::Ordering;

#[test]
fn selected_lowering_replays_one_physical_child_per_surviving_occurrence_role() {
    // One program retains both surviving boundary-occurrence roles under an
    // admitted selected-lowering optimization: the closed operator application
    // settles through its provider call interval, and the hosted console exit
    // settles through a BoundaryTraitSettlement. Replay must derive exactly one
    // physical child per surviving occurrence with the matching parent role.
    let root = std::env::temp_dir().join(format!(
        "omega-optimizer-selected-lowering-boundary-settlement-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create selected-lowering package root");
    let standard_library = native_evidence_standard_library();
    std::fs::write(
        root.join("build.omg"),
        format!(
            "machine build(builder: &mut Build) {{\n\
             \x20   builder.application(\"optimizer-selected-lowering-boundary-settlement\");\n\
             \x20   builder.depend(Source::Path {{ location: \"{}\" }});\n\
             \x20   builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);\n\
             \x20   builder.optimizations.enable(Optimization::SelectedIncomingU12CompareImmediate);\n\
             }}\n",
            standard_library.display()
        ),
    )
    .expect("write selected-lowering package build");
    std::fs::write(
        root.join("main.omg"),
        "use omega_language_std::console;\n\
         \n\
         data CheckedMath {}\n\
         \n\
         boundary operator CheckedMath::select_left(left: u64, right: u64) -> u64;\n\
         \n\
         data CheckedMathProvider {}\n\
         \n\
         machine CheckedMathProvider::select_left_impl(left: u64, right: u64) -> u64\n\
         satisfies CheckedMath::select_left\n\
         {\n\
             transition { _ -> left }\n\
         }\n\
         \n\
         data Main { console: Console; }\n\
         \n\
         machine Main::main(&mut self)\n\
         reaches\n\
             Console\n\
         {\n\
             let result: u64 = CheckedMath::select_left(7u64, 9u64);\n\
             self.console.exit_process(70);\n\
         }\n",
    )
    .expect("write selected-lowering boundary-settlement program");

    let application = package_identity(1);
    let standard = package_identity(2);
    let inputs = PackageCompilationInputs::new(
        application,
        package_compilation::BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(
                application,
                "optimizer-selected-lowering-boundary-settlement",
                root.clone(),
            ),
            PackageSourceBinding::new(standard, "omega-language-std", standard_library.clone()),
        ],
        vec![PackageDependencyBinding::new(
            application,
            "omega_language_std",
            standard,
        )],
    )
    .expect("selected-lowering package inputs should validate");
    let entry_binding =
        linux_entry_acceptance::candidate_linux_x86_64_entry_binding(&standard_library, standard)
            .expect("the fixture explicitly accepts the checked Linux entry schema");
    let inputs = inputs
        .with_accepted_semantic_bindings(vec![entry_binding.clone()])
        .expect("entry acceptance binds to the std package");
    let preliminary = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs.clone()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("boundary-settlement program checks");
    let console_binding =
        console_acceptance::candidate_console_exit_binding(&preliminary, standard, false, false)
            .expect("hosted console exit acceptance should derive from the checked program");
    let inputs = inputs
        .with_accepted_semantic_bindings(vec![entry_binding, console_binding])
        .expect("entry and console exit acceptance bind to the std package");
    let permission_policy = native_realization::terminal_authority_permission_policy_with_rows(
        inputs
            .accepted_semantic_bindings()
            .flat_map(|binding| binding.terminal_authority_permissions().iter().cloned())
            .collect(),
    )
    .expect("console exit permission policy should validate");

    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(root.join("build")),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_package_inputs(inputs)
        .with_terminal_authority_permission_policy(permission_policy),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("selected-lowering boundary-settlement program emits an artifact");
    let artifact = report
        .retained_native_artifact()
        .expect("selected-lowering compilation retains its native artifact");
    artifact
        .validate()
        .expect("selected-lowering native artifact should replay independently");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact
        .physical_evidence()
        .expect("the surviving occurrences retain nonempty physical evidence");
    let [operator_occurrence] = physical.projection().operator_occurrences() else {
        panic!("the checked boundary operator must survive as exactly one operator occurrence")
    };
    let [boundary_occurrence] = physical.projection().boundary_occurrences() else {
        panic!("the hosted console exit must survive as exactly one boundary occurrence")
    };
    let [operator_child, boundary_child] = physical.children() else {
        panic!("each surviving occurrence must bind exactly one physical child")
    };
    assert_eq!(
        operator_child.occurrence(),
        native_realization::NativePhysicalOccurrence::Operator(operator_occurrence.identity())
    );
    assert_eq!(
        operator_child.projection(),
        physical.projection().identity()
    );
    assert!(matches!(
        operator_child.parent(),
        native_realization::PhysicalChildParent::OperatorApplicationCoverage(_)
    ));
    assert!(operator_child.machine_span().byte_count() > 0);
    assert!(operator_child.object_span().byte_count() > 0);
    assert_eq!(
        operator_child.relocation(),
        native_realization::PhysicalRelocationDisposition::ResolvedInternalCall
    );
    assert_eq!(
        boundary_child.occurrence(),
        native_realization::NativePhysicalOccurrence::Boundary(boundary_occurrence.identity())
    );
    assert_eq!(
        boundary_child.projection(),
        physical.projection().identity()
    );
    let native_realization::PhysicalChildParent::BoundaryTraitSettlement(settlement) =
        boundary_child.parent()
    else {
        panic!("the boundary occurrence must retain its boundary-settlement parent")
    };
    assert_eq!(settlement.occurrence(), boundary_occurrence);
    assert_eq!(
        settlement.execution(),
        target_operations::BoundaryExecutionBinding::CompilerBuiltin(
            target_operations::CompilerBuiltinExecution::HostedExitProcessI32
        )
    );
    assert!(matches!(
        settlement.role(),
        native_realization::BoundaryTraitSettlementRole::CompilerBuiltin {
            execution: target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
            ..
        }
    ));
    assert!(boundary_child.machine_span().byte_count() > 0);
    assert!(boundary_child.object_span().byte_count() > 0);
    assert_eq!(
        boundary_child.relocation(),
        native_realization::PhysicalRelocationDisposition::DirectInstructionBytes
    );

    // Independent replay derives the same two-child custody from the published
    // parts alone; each mutation class must fail closed.
    let parts = report
        .into_retained_native_artifact()
        .expect("owned native artifact")
        .into_parts();
    let assert_mutated_children_rejected =
        |mutate: &dyn Fn(&mut Vec<native_realization::NativePhysicalChild>)| {
            let mut replay = replay_native_artifact_parts(&parts);
            let evidence = replay
                .physical_evidence
                .take()
                .expect("replay physical evidence")
                .into_parts();
            let mut children = evidence.children;
            mutate(&mut children);
            replay.physical_evidence = Some(
                native_realization::NativePhysicalEvidence::from_replayed_parts(
                    native_realization::NativePhysicalEvidenceParts {
                        projection: evidence.projection,
                        children,
                        identity: evidence.identity,
                    },
                ),
            );
            assert!(
                native_realization::NativeArtifact::from_replayed_parts(replay).is_err(),
                "mutated physical-child custody must fail independent replay"
            );
        };
    assert_mutated_children_rejected(&|children| {
        children.retain(|child| {
            matches!(
                child.parent(),
                native_realization::PhysicalChildParent::OperatorApplicationCoverage(_)
            )
        });
    });
    assert_mutated_children_rejected(&|children| {
        children.retain(|child| {
            matches!(
                child.parent(),
                native_realization::PhysicalChildParent::BoundaryTraitSettlement(_)
            )
        });
    });
    assert_mutated_children_rejected(&|children| {
        children.push(children[0].clone());
    });
    assert_mutated_children_rejected(&|children| {
        let settlement = children[1].parent().clone();
        let mut swapped = children[0].clone().into_parts();
        swapped.parent = settlement;
        children[0] = native_realization::NativePhysicalChild::from_replayed_parts(swapped);
    });
}

#[test]
fn selected_lowering_exact_add_occurrence_replays_one_exact_physical_child() {
    // A second selected-lowering family carries the same physical-child
    // contract: the package-bound boundary operator program compiles under
    // `SelectedIncomingU12ExactAddImmediate`, and the provider body's bounded
    // `value + 5` gives the catalog's `EXACT_ADD_IMMEDIATE_U12` pair rule a
    // real source-reachable candidate. The surviving operator occurrence still
    // binds exactly one OperatorApplicationCoverage child through allocation,
    // layout, and native emission; independent replay rejects every mutation
    // class — missing, duplicate, stale, substituted, padded, and
    // role-swapped children.
    let root = std::env::temp_dir().join(format!(
        "omega-optimizer-opt-in-exact-add-physical-child-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create exact-add physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::add_five(value: u64 [0..=100]) -> u64 [0..=105];

data CheckedMathProvider {}

machine CheckedMathProvider::add_five_impl(value: u64 [0..=100]) -> u64 [0..=105]
satisfies CheckedMath::add_five
{
    transition { _ -> (value + 5) }
}

data Main {}

machine Main::main(&mut self) {
    let picked: u64 = CheckedMath::add_five(7u64);
}
"#,
    )
    .expect("write exact-add physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-exact-add-physical-child");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12ExactAddImmediate);
}
"#,
    )
    .expect("write exact-add physical-child build");
    let root_identity = package_identity(44);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("exact-add physical-child package graph should validate");
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
    .expect("the exact-add selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("exact-add compilation retains its native artifact");
    artifact
        .validate()
        .expect("exact-add native artifact should replay independently");
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
fn selected_lowering_exact_subtract_occurrence_replays_one_exact_physical_child() {
    // A third selected-lowering family carries the same physical-child
    // contract: the package-bound boundary operator program compiles under
    // `SelectedIncomingU12ExactSubtractImmediate`, and the provider body's
    // bounded `value - 5` gives the catalog's `EXACT_SUBTRACT_IMMEDIATE_U12`
    // pair rule a real source-reachable candidate. The input range starts at
    // five so the exact subtraction cannot underflow; the surviving operator
    // occurrence still binds exactly one OperatorApplicationCoverage child
    // through allocation, layout, and native emission; independent replay
    // rejects every mutation class — missing, duplicate, stale, substituted,
    // padded, and role-swapped children.
    let root = std::env::temp_dir().join(format!(
        "omega-optimizer-opt-in-exact-subtract-physical-child-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create exact-subtract physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::subtract_five(value: u64 [5..=100]) -> u64 [0..=95];

data CheckedMathProvider {}

machine CheckedMathProvider::subtract_five_impl(value: u64 [5..=100]) -> u64 [0..=95]
satisfies CheckedMath::subtract_five
{
    transition { _ -> (value - 5) }
}

data Main {}

machine Main::main(&mut self) {
    let picked: u64 = CheckedMath::subtract_five(7u64);
}
"#,
    )
    .expect("write exact-subtract physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-exact-subtract-physical-child");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12ExactSubtractImmediate);
}
"#,
    )
    .expect("write exact-subtract physical-child build");
    let root_identity = package_identity(45);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("exact-subtract physical-child package graph should validate");
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
    .expect("the exact-subtract selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("exact-subtract compilation retains its native artifact");
    artifact
        .validate()
        .expect("exact-subtract native artifact should replay independently");
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
fn selected_lowering_exact_divide_occurrence_replays_one_exact_physical_child() {
    // A fourth selected-lowering family carries the same physical-child
    // contract: the package-bound boundary operator program compiles under
    // `SelectedIncomingExactDivideIdentityCopy`, and the provider body's
    // `value / 1` gives the catalog's `EXACT_DIVIDE_ONE_COPY` pair rule a
    // real source-reachable candidate — an unsigned divide by one returns
    // the dividend, so the literal producer and the pinned divide consumer
    // rewrite to a `CopyI64`. The surviving operator occurrence still binds
    // exactly one OperatorApplicationCoverage child through allocation,
    // layout, and native emission; independent replay rejects every
    // mutation class — missing, duplicate, stale, substituted, padded, and
    // role-swapped children.
    let root = std::env::temp_dir().join(format!(
        "omega-optimizer-opt-in-exact-divide-physical-child-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create exact-divide physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::divide_one(value: u64) -> u64;

data CheckedMathProvider {}

machine CheckedMathProvider::divide_one_impl(value: u64) -> u64
satisfies CheckedMath::divide_one
{
    transition { _ -> (value / 1) }
}

data Main {}

machine Main::main(&mut self) {
    let picked: u64 = CheckedMath::divide_one(7u64);
}
"#,
    )
    .expect("write exact-divide physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-exact-divide-physical-child");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingExactDivideIdentityCopy);
}
"#,
    )
    .expect("write exact-divide physical-child build");
    let root_identity = package_identity(46);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("exact-divide physical-child package graph should validate");
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
    .expect("the exact-divide selection must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("exact-divide compilation retains its native artifact");
    artifact
        .validate()
        .expect("exact-divide native artifact should replay independently");
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
fn verified_eliminated_occurrence_needs_no_physical_child() {
    // The child-exemption half of the physical-child contract: a D29 covered
    // operation that an independently validated optimization proves
    // unreachable needs no physical child. Two private machines each apply
    // the boundary operator, and the entry machine reaches one of them only
    // through a constant-false transition arm, so checked D29 coverage names
    // both Terminal operations and the ordinary build binds each surviving
    // occurrence to its own child. ControlFlowCleanup's constant-conditional
    // rule folds the dead arm, the unreachable-private-machine rule then
    // proves the dead callee unreachable — pruned custody plus
    // ProvenUnreachableAt provenance for its call node in the validated
    // ledger — and the validated optimized projection keeps only the
    // surviving occurrence.
    let root = std::env::temp_dir().join(format!(
        "omega-optimizer-opt-in-eliminated-occurrence-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create eliminated-occurrence project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::select_left(left: u64, right: u64) -> u64;

data CheckedMathProvider {}

machine CheckedMathProvider::select_left_impl(left: u64, right: u64) -> u64
satisfies CheckedMath::select_left
{
    transition { _ -> left }
}

machine dead_pick() {
    let dead: u64 = CheckedMath::select_left(1u64, 2u64);
}

machine live_pick() {
    let kept: u64 = CheckedMath::select_left(7u64, 9u64);
}

data Main {}

machine Main::main() {
    transition false {
        true -> run_dead()
        false -> run_live()
    }

    state run_dead() {
        dead_pick();
        transition { _ -> run_live() }
    }

    state run_live() {
        live_pick();
        transition { _ -> done() }
    }

    state done() { }
}
"#,
    )
    .expect("write eliminated-occurrence main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-eliminated-occurrence");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12CompareImmediate);
}
"#,
    )
    .expect("write eliminated-occurrence build");
    let root_identity = package_identity(43);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("eliminated-occurrence package graph should validate");
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
    .expect("the constant-dead boundary application must reach native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("the ordinary build retains its native artifact");
    artifact
        .validate()
        .expect("the ordinary native artifact replays independently");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact
        .physical_evidence()
        .expect("both covered applications retain nonempty physical evidence");
    // Both covered applications survive the ordinary build: two operator
    // occurrences, each bound to exactly one physical child.
    let [first_covered, second_covered] = physical.projection().operator_occurrences() else {
        panic!("the ordinary build must keep both covered applications")
    };
    assert!(physical.projection().boundary_occurrences().is_empty());
    assert_eq!(physical.children().len(), 2);
    let covered_operations = [first_covered.operation(), second_covered.operation()];

    // Independently replay the published artifact sections into the canonical
    // optimizer and rerun the eliminative Psi schedule. `NativeRealizationRequest`
    // structurally cannot carry a Psi-phase selection, so this test drives the
    // identical `optimize_verified_abstract_input` admission that the
    // production optimization stage uses.
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: artifact.semantic_bytes(),
            proof_bytes: artifact.proof_bytes(),
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .expect("the published artifact replays into verified optimizer input");
    let selections =
        optimization_core::OptimizationSelections::new([Optimization::ControlFlowCleanup])
            .expect("one exact optimization selection");
    let optimized = native_realization::optimize_verified_abstract_input(
        input.clone(),
        native_realization::compiler_baseline_request_v1(&selections),
    )
    .expect("the validating run accepts the proven elimination");

    // Both applications remain D29-covered in the published Terminal module;
    // the optimized projection keeps exactly one of them.
    let coverage = artifact
        .boundary_application_coverage()
        .expect("the boundary applications retain D29 coverage");
    assert_eq!(coverage.references().len(), 2);
    let eliminated_scope =
        native_realization::NativePhysicalEvidenceScope::from_validated_optimization(
            optimized.plan(),
            optimized.validation().psi(),
            optimized.validation().identity(),
            optimized.validation().final_unit(),
            coverage,
        )
        .expect("the validated eliminated plan still derives its physical scope");
    let native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(
        eliminated_projection,
    ) = &eliminated_scope
    else {
        panic!("the eliminated run derives a validated optimized projection")
    };
    let [survivor] = eliminated_projection.projection().operator_occurrences() else {
        panic!("exactly one operator occurrence survives the verified elimination")
    };
    assert!(
        eliminated_projection
            .projection()
            .boundary_occurrences()
            .is_empty()
    );
    let eliminated_operation = covered_operations
        .into_iter()
        .find(|operation| *operation != survivor.operation())
        .expect("the projection names one covered survivor and one covered elimination");
    assert!(
        coverage
            .references()
            .iter()
            .any(|reference| reference.terminal_operation() == eliminated_operation),
        "the eliminated operation retains its checked D29 coverage reference"
    );

    // The elimination is independently verified, not merely absent from the
    // final plan: the validated transformation ledger carries a
    // ProvenUnreachableAt rewrite row for the exact input node that owned the
    // covered call.
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("replay Terminal semantics");
    let eliminated_site = module
        .machines
        .iter()
        .flat_map(|machine| machine.blocks.iter().map(move |block| (machine.id, block)))
        .flat_map(|(machine, block)| {
            block
                .operations
                .iter()
                .enumerate()
                .map(move |(node, operation)| (machine, block.id, node, operation.id))
        })
        .find(|(.., operation)| *operation == eliminated_operation)
        .map(|(machine, block, node, _)| {
            (
                machine,
                block,
                u32::try_from(node).expect("operation node index fits u32"),
            )
        })
        .expect("the eliminated covered operation exists in the source Terminal module");
    // The covered call lived in a private machine the validated run proved
    // unreachable: the replay unit retains its pruned custody while the final
    // plan drops the whole function.
    let [pruned] = optimized.unit().pruned_machines.as_slice() else {
        panic!("ControlFlowCleanup must prove exactly one private machine unreachable")
    };
    let dead_machine = pruned.machine;
    assert_eq!(
        dead_machine, eliminated_site.0,
        "the eliminated covered operation lives in the proven-unreachable machine"
    );
    assert!(
        optimized
            .verified_input()
            .plan()
            .functions
            .iter()
            .any(|function| function.machine == dead_machine),
        "the eliminated machine is present in the verified input plan"
    );
    assert!(
        !optimized
            .plan()
            .functions
            .iter()
            .any(|function| function.machine == dead_machine),
        "the eliminated machine is absent from the validated final plan"
    );
    assert!(
        optimized
            .transformation_ledger()
            .records()
            .iter()
            .any(|record| {
                record.provenance.iter().any(|rewrite| {
                    !rewrite.disposition.is_realized()
                        && rewrite.input.node().is_some_and(|location| {
                            (location.machine, location.block, location.node) == eliminated_site
                        })
                })
            }),
        "the validated ledger proves the covered call site unreachable"
    );

    // The same coverage over the identity plan still requires the child: the
    // exemption attaches to the verified elimination, not to the coverage row.
    let identity = native_realization::optimize_verified_abstract_input(
        input,
        native_realization::compiler_baseline_request_v1(
            &optimization_core::OptimizationSelections::default(),
        ),
    )
    .expect("the identity run revalidates the unchanged plan");
    let retained_scope =
        native_realization::NativePhysicalEvidenceScope::from_validated_optimization(
            identity.plan(),
            identity.validation().psi(),
            identity.validation().identity(),
            identity.validation().final_unit(),
            coverage,
        )
        .expect("the identity run still derives its physical scope");
    let native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(
        retained_projection,
    ) = &retained_scope
    else {
        panic!("the identity run derives a validated optimized projection")
    };
    assert_eq!(
        retained_projection
            .projection()
            .operator_occurrences()
            .len(),
        2,
        "a covered operation that survives still requires its physical child"
    );

    // A replayed child bound to the occurrence identity the eliminated
    // operation would have carried had it survived the eliminating run is
    // stale: no validated survivor set answers for it. The canonical identity
    // encoding is the same terminal + validation + final-unit authority walk
    // the projection derivation applies.
    let (eliminated_machine, eliminated_ordinal) = [first_covered, second_covered]
        .into_iter()
        .find(|occurrence| occurrence.operation() == eliminated_operation)
        .map(|occurrence| (occurrence.machine(), occurrence.operation_ordinal()))
        .expect("the ordinary projection names the eliminated covered operation");
    let parts = report
        .into_retained_native_artifact()
        .expect("owned native artifact")
        .into_parts();
    let mut stale = replay_native_artifact_parts(&parts);
    let evidence = stale
        .physical_evidence
        .take()
        .expect("replay physical evidence")
        .into_parts();
    let terminal = optimized.validation().psi();
    let mut canonical = Vec::new();
    canonical.extend_from_slice(&terminal.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(terminal.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&optimized.validation().identity().bytes());
    canonical.extend_from_slice(&optimized.validation().final_unit().bytes());
    canonical.extend_from_slice(&eliminated_machine.get().to_le_bytes());
    canonical.extend_from_slice(&eliminated_operation.get().to_le_bytes());
    canonical.extend_from_slice(
        &u64::try_from(eliminated_ordinal)
            .expect("occurrence ordinal")
            .to_le_bytes(),
    );
    let mut stale_child = evidence.children[0].clone().into_parts();
    stale_child.projection = eliminated_projection.projection().identity();
    stale_child.occurrence = native_realization::NativePhysicalOccurrence::Operator(
        optimization_core::OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(&canonical),
    );
    stale.physical_evidence_scope = eliminated_scope.clone();
    stale.physical_evidence = Some(
        native_realization::NativePhysicalEvidence::from_replayed_parts(
            native_realization::NativePhysicalEvidenceParts {
                projection: eliminated_projection.projection().clone(),
                children: vec![
                    native_realization::NativePhysicalChild::from_replayed_parts(stale_child),
                ],
                identity: evidence.identity,
            },
        ),
    );
    assert!(
        native_realization::NativeArtifact::from_replayed_parts(stale).is_err(),
        "a physical child bound to a verified-eliminated occurrence must not replay"
    );

    // An unverified omission is equally rejected: under the published plan's
    // own scope both covered occurrences still demand their children, so
    // dropping one cannot replay.
    let mut missing = replay_native_artifact_parts(&parts);
    let mut retained = missing
        .physical_evidence
        .take()
        .expect("replay physical evidence")
        .into_parts();
    retained.children.pop();
    missing.physical_evidence = Some(
        native_realization::NativePhysicalEvidence::from_replayed_parts(
            native_realization::NativePhysicalEvidenceParts {
                projection: retained.projection,
                children: retained.children,
                identity: retained.identity,
            },
        ),
    );
    assert!(
        native_realization::NativeArtifact::from_replayed_parts(missing).is_err(),
        "omitting a required physical child must not replay"
    );
}

#[test]
fn x86_rel8_relaxation_selection_round_trips_but_remains_default_off() {
    let absent = project("x86-rel8-default-off", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("an absent build must leave branch relaxation disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::X86RelaxConditionalBranchesToRel8V1)
    );

    let selected = project(
        "x86-rel8-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-x86-rel8-selected");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::X86RelaxConditionalBranchesToRel8V1);
}
"#,
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &selected.join("main.omg"),
        Some("windows_x86_64"),
    ))
    .expect("the named function-relative-layout selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::X86RelaxConditionalBranchesToRel8V1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );

    let build_dir = selected.join("build");
    let report = compile_native_and_publish(CompileOptions {
        root_path: selected.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x86_64".into()),
    })
    .expect("the selected layout rule executes through native publication on x86");
    let executable = report
        .checked_native_executable_path()
        .expect("publication returns the executable receipt");
    assert_eq!(executable, build_dir.join("omega-program.exe").as_path());
    assert!(executable.is_file());
    assert!(!build_dir.join("omega-program").exists());
}

#[test]
fn aarch64_cbnz_fusion_selection_round_trips_but_remains_default_off() {
    let absent = project("aarch64-cbnz-default-off", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("an absent build must leave AArch64 CBNZ fusion disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1)
    );

    let selected = project(
        "aarch64-cbnz-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-aarch64-cbnz-selected");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &selected.join("main.omg"),
        Some("windows_x86_64"),
    ))
    .expect("the named post-allocation machine selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );

    let build_dir = selected.join("build");
    let diagnostics = compile_native_and_publish(CompileOptions {
        root_path: selected.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x86_64".into()),
    })
    .expect_err("the build-visible machine selection must remain publication-gated");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("`Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1`")
    );
    assert!(diagnostics[0].message.contains("no output was installed"));
    assert!(!build_dir.join("omega-program").exists());
    assert!(!build_dir.join("omega-program.exe").exists());
}

#[test]
fn aarch64_movn_materialization_selection_round_trips_but_remains_default_off() {
    let absent = project("aarch64-movn-default-off", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("an absent build must leave AArch64 MOVN materialization disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1)
    );

    let selected = project(
        "aarch64-movn-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-aarch64-movn-selected");
    builder.optimizations.enable(Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&selected.join("main.omg"), None))
        .expect("the named MOVN materialization selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );
}

#[test]
fn x86_xor_zero_materialization_selection_round_trips_but_remains_default_off() {
    let absent = project("x86-xor-zero-default-off", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("an absent build must leave x86 XOR-zero materialization disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::X86SelectXorZeroI64MaterializationV1)
    );

    let selected = project(
        "x86-xor-zero-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-x86-xor-zero-selected");
    builder.optimizations.enable(Optimization::X86SelectXorZeroI64MaterializationV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&selected.join("main.omg"), None))
        .expect("the named x86 XOR-zero materialization selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::X86SelectXorZeroI64MaterializationV1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );
}

#[test]
fn x86_mov_r32_imm32_materialization_selection_round_trips_but_remains_default_off() {
    let absent = project("x86-mov-r32-imm32-default-off", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("an absent build must leave x86 MOV-r32-imm32 materialization disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1)
    );

    let selected = project(
        "x86-mov-r32-imm32-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-x86-mov-r32-imm32-selected");
    builder.optimizations.enable(Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&selected.join("main.omg"), None))
        .expect("the named x86 MOV-r32-imm32 materialization selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );
}

#[test]
fn x86_mov_r64_imm32_materialization_selection_round_trips_but_remains_default_off() {
    let absent = project("x86-mov-r64-imm32-default-off", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("an absent build must leave x86 MOV-r64-imm32 materialization disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1)
    );

    let selected = project(
        "x86-mov-r64-imm32-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-x86-mov-r64-imm32-selected");
    builder.optimizations.enable(Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&selected.join("main.omg"), None))
        .expect("the named x86 MOV-r64-imm32 materialization selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );
}

#[test]
fn shared_entry_fixed_view_copy_selection_round_trips_but_remains_default_off() {
    let absent = project("shared-entry-copy-default-off", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("an absent build must leave shared-entry copy insertion disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1)
    );

    let selected = project(
        "shared-entry-copy-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-shared-entry-copy-selected");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &selected.join("main.omg"),
        Some("windows_x86_64"),
    ))
    .expect("the named allocation-recovery selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );

    let build_dir = selected.join("build");
    let report = compile_native_and_publish(CompileOptions {
        root_path: selected.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x86_64".into()),
    })
    .expect("the selected allocation-recovery rule executes through native publication");
    let executable = report
        .checked_native_executable_path()
        .expect("publication returns the executable receipt");
    assert_eq!(executable, build_dir.join("omega-program.exe").as_path());
    assert!(executable.is_file());
    assert!(!build_dir.join("omega-program").exists());
}

#[test]
fn active_resident_multi_use_rematerialization_selection_round_trips_but_remains_default_off() {
    let absent = project("active-resident-rematerialization-default-off", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("an absent build must leave active-resident rematerialization disabled");
    assert!(
        !checked
            .optimization_selections()
            .contains(Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1)
    );

    let selected = project(
        "active-resident-rematerialization-selected",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-active-resident-rematerialization-selected");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1);
}
"#,
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &selected.join("main.omg"),
        Some("windows_x86_64"),
    ))
    .expect("the named allocation-recovery selection should evaluate");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1]
    );
    assert_eq!(
        checked.optimization_selection_identity(),
        checked.optimization_selections().identity()
    );

    let build_dir = selected.join("build");
    let diagnostics = compile_native_and_publish(CompileOptions {
        root_path: selected.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x86_64".into()),
    })
    .expect_err("the build-visible rematerialization must remain publication-gated");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("`ActiveResidentImmediateU64MultiUseRematerializationV1`")
    );
    assert!(
        diagnostics[0]
            .message
            .contains("complete verified optimizer pipeline"),
        "unexpected diagnostic: {}",
        diagnostics[0].message
    );
    assert!(diagnostics[0].message.contains("no output was installed"));
    assert!(!build_dir.join("omega-program").exists());
    assert!(!build_dir.join("omega-program.exe").exists());
}

#[test]
fn selected_check_only_validates_without_entering_an_optimizer_backend() {
    let root = project(
        "selected-check-only",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-selected-check-only");
    builder.optimizations.enable(Optimization::ControlFlowCleanup);
}
"#,
        ),
    );
    compile_check(CompileOptions {
        root_path: root.join("main.omg"),
        build_dir: None,
        target_name: None,
    })
    .expect("check-only compilation validates selection without running optimization");
}

#[test]
fn terminal_product_routes_selected_psi_pass_to_preterminal_stage() {
    let root = project(
        "selected-terminal-product",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-selected-terminal-product");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::ControlFlowCleanup);
}
"#,
        ),
    );
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: None,
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("a selected Psi pass executes at its pre-Terminal owner before publication");
    let artifact = report
        .artifact()
        .expect("the Terminal product retains its canonical artifact");
    let executed = artifact.optimization().selections();
    assert_eq!(executed.as_slice().len(), 1);
    assert!(
        executed.contains(
            Optimization::ControlFlowCleanup
                .optimization()
                .expect("ControlFlowCleanup is a Psi selection")
        )
    );
    assert_eq!(
        artifact.manifest().optimization(),
        artifact.optimization().identity(),
        "the published manifest binds the execution that produced it"
    );
    artifact
        .validate()
        .expect("the optimized Terminal artifact replays");
}

#[test]
fn terminal_product_executes_and_can_disable_the_selected_psi_pass() {
    let root = project(
        "terminal-dead-scalars",
        Some(
            r#"
machine build(builder: &mut Build) {
    builder.application("terminal-dead-scalars");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::DeadPureScalarElimination);
}
"#,
        ),
    );
    let request = || {
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: None,
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
    };
    let identity = compiler::compile(request().with_optimization_rollback(
        OptimizationRollback::new([Optimization::DeadPureScalarElimination]).unwrap(),
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("rollback executes the identity stage");
    let selected = compiler::compile(request())
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("selected pre-Terminal pass executes");
    assert_eq!(
        identity.artifact().unwrap().semantic_bytes(),
        selected.artifact().unwrap().semantic_bytes(),
        "an empty Unit body is already minimal"
    );
    assert_ne!(
        identity.artifact().unwrap().optimization().selection(),
        selected.artifact().unwrap().optimization().selection()
    );
    let receipt = identity.optimization_rollback_receipt().unwrap();
    assert!(receipt.effective().is_empty());
    assert!(
        receipt
            .actually_disabled()
            .contains(Optimization::DeadPureScalarElimination)
    );
    assert!(identity.has_consistent_executable_publication_custody());
    assert!(selected.optimization_rollback_receipt().is_none());
    selected.artifact().unwrap().validate().unwrap();
    assert!(
        selected
            .with_terminal_optimization_rollback(Some(receipt.clone()))
            .is_err(),
        "a rollback receipt cannot substitute for the selection that actually ran"
    );
}
