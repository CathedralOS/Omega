use super::*;
use build_providers::plans::ProviderPlan;
use native_intrinsics::CompilerIntrinsicExecutionIdentity;
use numerics::intrinsics::NumericIntrinsic;
use provider_correspondence::external_providers::reconcile_numeric_meaning;
use std::collections::BTreeSet;
#[test]
fn intrinsic_slots_are_lent_only_to_the_execution_that_selected_them() {
    let execution = |fingerprint| {
        terminal_codec::TerminalExecutionIdentity::portable(terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([fingerprint; 32]),
        })
    };
    let selected = AdmittedTerminalIntrinsics {
        terminal: execution(1),
        operations: Vec::new(),
    };
    assert_eq!(selected.operations_for(&execution(1)), Ok(&[][..]));
    assert_eq!(
        selected.operations_for(&execution(2)),
        Err("semantic intrinsic admission belongs to another Terminal artifact"),
    );
}

#[test]
fn portable_numeric_meaning_cannot_authorize_or_replace_provider_custody() {
    let meaning = NumericIntrinsic::Builtin {
        function: language_core::BuiltinFunction::FloatIsNan,
        format: numeric_literals::FloatFormat::F64,
    };
    let selected = CompilerIntrinsicExecutionIdentity::from(meaning);
    assert!(reconcile_numeric_meaning(Some(meaning), Some(selected)).is_ok());
    assert!(reconcile_numeric_meaning(Some(meaning), None).is_err());
    assert!(reconcile_numeric_meaning(None, Some(selected)).is_err());
    let other = NumericIntrinsic::Builtin {
        function: language_core::BuiltinFunction::FloatIsFinite,
        format: numeric_literals::FloatFormat::F64,
    };
    assert!(reconcile_numeric_meaning(Some(other), Some(selected)).is_err());
    assert!(
        reconcile_numeric_meaning(
            Some(meaning),
            Some(CompilerIntrinsicExecutionIdentity::HostedExitProcessI32)
        )
        .is_err()
    );
    assert!(
        reconcile_numeric_meaning(
            None,
            Some(CompilerIntrinsicExecutionIdentity::HostedExitProcessI32)
        )
        .is_ok()
    );
    assert!(reconcile_numeric_meaning(None, None).is_ok());
    let format = semantic_vocabulary::IeeeFloatFormat::from(meaning.builtin_format().unwrap());
    assert_eq!(format, semantic_vocabulary::IeeeFloatFormat::Binary64);
    assert!(matches!(
        catalog_realization(selected, Some(format)).map(|value| (value.operation, value.format)),
        Some((
            target_vocabulary::IeeeFloatArithmeticOperation::IsNan,
            semantic_vocabulary::IeeeFloatFormat::Binary64
        ))
    ));
}

fn intrinsic_plan(realization: &str) -> build_providers::selections::SelectedProviderPlanFacts {
    let mut plan = ProviderPlan {
        name: "selected".into(),
        provider_type: "NativeFloats".into(),
        provider_declaration: Some("NativeFloats".into()),
        row_numeric_realizations: vec![Some(realization.into())],
        target: "linux_x86_64".into(),
        ..Default::default()
    };
    let declaration = std::sync::Arc::make_mut(&mut plan.schema.declaration);
    declaration.trait_name = declaration.identities.intern("Float");
    declaration
        .methods
        .push(build_providers::plans::ServiceMethod {
            name: "add".into(),
            requirement_owner: "Float".into(),
            requirement_identity: declaration.identities.intern("Float::add"),
            ..Default::default()
        });
    let requirement = plan.identities.intern("Float::add");
    plan.rows.push(build_providers::plans::ProviderPlanRow {
        method: "add".into(),
        requirement_identity: requirement,
        declared_requirement_identity: requirement,
        calling_signature: None,
        requirement_lifetime_partition: Vec::new(),
        admitted_same_stack_bytes: 0,
        binding: build_providers::plans::ProviderBinding::CompilerIntrinsic {
            machine: "float_add".into(),
        },
    });
    build_providers::selections::SelectedProviderPlanFacts::from_selected_plans(vec![plan]).unwrap()
}

#[test]
fn external_realization_owner_is_demanded_only_for_retained_requirements() {
    let facts = intrinsic_plan("NativeFloatAdd");
    let identities = terminal_psi::TerminalIdentityTable::default();
    // A selected float row the program never retains needs no declaration:
    // a scalar-only module must not be refused for metadata it never runs.
    assert_eq!(
        selection::validate_retained_provider_owners(&[], &identities, &facts, &BTreeSet::new()),
        Ok(())
    );
    let unrelated = BTreeSet::from(["Integer::add"]);
    assert_eq!(
        selection::validate_retained_provider_owners(&[], &identities, &facts, &unrelated),
        Ok(())
    );
    // Once the module retains the requirement, its declaration is required.
    let retained = BTreeSet::from(["Float::add"]);
    let refusal = selection::validate_retained_provider_owners(&[], &identities, &facts, &retained)
        .unwrap_err();
    assert!(
        refusal.contains("has no retained external implementation for `Float::add`"),
        "{refusal}"
    );
}

#[test]
fn admission_requires_a_published_boundary_for_a_demanded_selected_row() {
    let module = terminal_psi::TerminalModule::declarations();
    let record = terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_parts(module, record, None).unwrap();
    let execution = terminal_codec::TerminalExecutionView::from(artifact.view());
    let facts = intrinsic_plan("NativeFloatAdd");
    let plan = &facts.plans()[0];
    let provenance = || {
        std::iter::once(CompilerIntrinsicPlanProvenance {
            plan,
            semantic_schema: &plan.schema,
            row_executions: &[None],
        })
    };
    let admitted =
        admit_terminal_intrinsics(execution, &facts, provenance(), &BTreeSet::new()).unwrap();
    assert!(
        admitted
            .operations_for(&execution.custody_identity())
            .unwrap()
            .is_empty()
    );

    let diagnostics = admit_terminal_intrinsics(
        execution,
        &facts,
        provenance(),
        &BTreeSet::from(["Float::add".into()]),
    )
    .unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("does not rejoin one Terminal boundary"),
        "{diagnostics:?}"
    );
}
