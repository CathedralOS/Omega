use super::{
    BoundaryApplicationSettlement, retain_terminal_boundary_application_coverage,
    validate_terminal_boundary_application_coverage,
};
use boundary_applications::{
    BoundaryApplication, BoundaryApplicationRealization, BoundaryApplicationRealizationCompanion,
    BoundaryNominalIdentity, BoundaryOperatorRequirement, TerminalBoundaryApplicationCoverage,
    TerminalBoundaryApplicationDemand, TerminalBoundaryApplicationDemands,
    TerminalBoundaryApplicationRealizations,
};
use checked_trees::CheckedTrees;
use lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope;
use terminal_codec::CanonicalTerminalArtifact;

fn fixture(
    value: u64,
) -> (
    CheckedTrees,
    CanonicalTerminalArtifact,
    CheckedBoundaryOperatorApplicationScope,
) {
    let source = format!("machine main() -> u64 {{ {value} }}");
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .unwrap();
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        checked_trees_to_lowered_psi::TerminalMachineSelection::Name("main"),
    )
    .unwrap();
    let semantic = terminal_codec::terminal_psi_identity(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::proof_bundle_fingerprint(&lowered.proof_bundle).unwrap();
    let execution = terminal_codec::PsiOptimizationExecutionRecord::new(
        Default::default(),
        semantic,
        proof,
        semantic,
        proof,
    )
    .unwrap();
    let artifact = CanonicalTerminalArtifact::from_parts(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &execution,
        None,
    )
    .unwrap();
    let scope =
        lowered_psi_to_terminal_psi::checked_boundary_operator_scope(&checked, &artifact, &lowered)
            .unwrap();
    (checked, artifact, scope)
}

#[test]
fn settlement_and_retained_replay_require_the_exact_scope_artifact() {
    let (checked, artifact, scope) = fixture(1);
    let (_, foreign_artifact, _) = fixture(2);
    let selected = effects::SelectedProviderPlanFacts::default();
    let settlement = BoundaryApplicationSettlement {
        checked: &checked,
        selected_provider_plans: &selected,
        selected_provider_provenance: &[],
        exact_toolchain_sources: &[],
        target_profile: None,
        console_exit_binding: None,
        process_exit_binding: None,
        calling_plan_realizations: &[],
    };
    let coverage = settlement
        .settle(&artifact, &scope)
        .expect("empty demand scope settles");
    assert!(coverage.demands().is_empty());
    assert_eq!(
        validate_terminal_boundary_application_coverage(&artifact, &scope, &coverage),
        Ok(())
    );
    assert_eq!(
        retain_terminal_boundary_application_coverage(&artifact, None, None),
        Ok(None)
    );
    for supplied in [None, Some(&coverage)] {
        assert_eq!(
            retain_terminal_boundary_application_coverage(&artifact, Some(&scope), supplied),
            Ok(Some(coverage.clone()))
        );
    }
    assert_eq!(
        retain_terminal_boundary_application_coverage(&artifact, None, Some(&coverage)),
        Err("D29 coverage requires the exact checked source-to-Terminal scope")
    );
    let expected = "checked boundary-operator scope belongs to a different Terminal artifact";
    let diagnostics = settlement.settle(&foreign_artifact, &scope).unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].message, expected);
    assert_eq!(
        validate_terminal_boundary_application_coverage(&foreign_artifact, &scope, &coverage),
        Err(expected),
    );
    for supplied in [None, Some(&coverage)] {
        assert_eq!(
            retain_terminal_boundary_application_coverage(
                &foreign_artifact,
                Some(&scope),
                supplied
            ),
            Err(expected)
        );
    }
}

#[test]
fn public_companion_cannot_pad_the_sealed_occurrence_roster() {
    let (_, artifact, scope) = fixture(1);
    let operation = semantic_vocabulary::OperationId::new(1).unwrap();
    let demands = TerminalBoundaryApplicationDemands::new(
        artifact.manifest().semantic(),
        vec![TerminalBoundaryApplicationDemand::new(
            operation,
            BoundaryOperatorRequirement::new(
                BoundaryNominalIdentity::new("foreign".to_owned()).unwrap(),
                "foreign::equal".to_owned(),
            )
            .unwrap(),
            BoundaryApplication::Empty,
        )],
    )
    .unwrap();
    let realizations =
        TerminalBoundaryApplicationRealizations::new(
            &demands,
            vec![BoundaryApplicationRealizationCompanion::new(
            operation,
            [1; 32],
            BoundaryApplicationRealization::ExactCompilerIntrinsic {
                execution: effects::CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
                    operation: effects::CompilerPrimitiveIntegerComparisonOperation::Equal,
                    integer_type: effects::CompilerNumericType::I64,
                },
            },
        ).unwrap()],
        )
        .unwrap();
    let padded = TerminalBoundaryApplicationCoverage::new(demands, realizations).unwrap();
    assert_eq!(
        validate_terminal_boundary_application_coverage(&artifact, &scope, &padded),
        Err("source-free D29 coverage differs from exact checked occurrence custody"),
    );
    assert_eq!(
        retain_terminal_boundary_application_coverage(&artifact, Some(&scope), Some(&padded)),
        Err("source-free D29 coverage differs from exact checked occurrence custody"),
    );
}
