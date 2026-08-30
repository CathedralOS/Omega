#![forbid(unsafe_code)]

//! Target-neutral admission and execution of compile-time Omega machines.

mod access_plans;
mod admission;
mod build_machines;
mod const_domain_facts;
mod const_generic_calls;
mod const_lengths;
mod layout_plans;
mod placed_views;
mod plan_laid;
mod wire_plans;

use std::sync::Arc;

pub use access_plans::{compute_access_plan, compute_placement_plan};
pub use admission::{
    BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeSelectionAuthority,
};
pub use build_machines::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, BuildMachineEvaluationError,
    BuildMachineExecutionMode, BuildMachineFilesystemAccess, BuildMachineFilesystemGrantRoot,
    BuildMachineFilesystemGrantRootIdentity, BuildMachineFilesystemGrants,
    BuildMachineFilesystemMetadataLayout, BuildMachineFilesystemSponsor,
    PreparedBuildMachineProgram, evaluate_build_machine_arguments_measured,
    evaluate_build_machine_arguments_measured_with_sponsor,
};
pub use const_domain_facts::{
    evaluate_const_domain_facts, evaluate_const_domain_facts_with_authority,
};
pub use const_generic_calls::evaluate_const_generic_calls;
pub use const_lengths::{
    evaluate_const_array_lengths, evaluate_const_array_lengths_with_authority,
    evaluate_zero_argument_machine, evaluate_zero_argument_machine_for_invocation,
};
pub use layout_plans::{
    BuildTimeValue, ValidatedConstDepthTwoNestedSumOccurrenceMaterialization,
    ValidatedConstMaterialization, ValidatedConstNestedSumRecordOccurrenceMaterialization,
    ValidatedConstRecordSumArrayElementMaterialization,
    ValidatedConstRecordSumArrayElementSelection, ValidatedConstRecordSumArrayFieldMaterialization,
    ValidatedConstRecordSumFieldMaterialization,
    ValidatedConstRecordWithDepthTwoNestedSumMaterialization,
    ValidatedConstRecordWithDepthTwoNestedSumsMaterialization,
    ValidatedConstRecordWithNestedSumRecordMaterialization,
    ValidatedConstRecordWithNestedSumRecordsMaterialization,
    ValidatedConstRecordWithSumArrayMaterialization,
    ValidatedConstRecordWithSumArraysMaterialization, ValidatedConstRecordWithSumMaterialization,
    ValidatedConstSumMaterialization, compute_layout_plan, compute_layout_plan_with_authority,
    compute_native_layout_plan, compute_native_layout_plan_with_authority,
    evaluate_and_materialize_typed_owned_layout_into, materialize_typed_owned_layout_into,
    normalized_schema_report_fingerprint, validate_const_materializable_conventional_sum,
    validate_const_materializable_record_with_conventional_sum,
    validate_const_materializable_record_with_conventional_sum_array,
    validate_const_materializable_record_with_conventional_sum_arrays,
    validate_const_materializable_record_with_conventional_sums,
    validate_const_materializable_record_with_depth_two_nested_sum,
    validate_const_materializable_record_with_depth_two_nested_sums,
    validate_const_materializable_record_with_nested_sum_record,
    validate_const_materializable_record_with_nested_sum_records,
    validate_const_materializable_typed_owned_layout,
};
pub use placed_views::{
    PlacedViewRecord, desugar_placed_views, validate_placed_view_plans,
    validate_placed_view_plans_with_authority,
};
pub use plan_laid::{
    PlanLaidRecord, compute_plan_laid_layouts, compute_plan_laid_layouts_with_authority,
    desugar_plan_laid_value_types,
};
pub use wire_plans::{compute_wire_plans, compute_wire_plans_with_authority};

/// Target-neutral syntax elaboration that must finish before name resolution.
///
/// Target selection remains an Omega orchestration concern and may run on the
/// returned syntax after this service has finished owning language-level
/// elaboration.
#[must_use = "pre-resolution syntax and its matching pre-check continuation must stay paired"]
pub struct PreResolutionEvaluation {
    syntax_trees: psi_syntax_trees::SyntaxTrees,
    pre_check: PreCheckEvaluation,
}

impl PreResolutionEvaluation {
    /// Separate the syntax consumed by target filtering and name resolution
    /// from the opaque continuation that owns the matching typed-tree work.
    pub fn into_syntax_and_pre_check(self) -> (psi_syntax_trees::SyntaxTrees, PreCheckEvaluation) {
        (self.syntax_trees, self.pre_check)
    }
}

/// One-shot continuation for target-neutral typed-tree evaluation.
///
/// The records and optional package selection authority are private so a
/// caller cannot accidentally rejoin records from one pre-resolution run to
/// another run or choose a different authority after name resolution.
#[must_use = "the matching typed tree must consume this pre-check continuation"]
pub struct PreCheckEvaluation {
    placed_view_records: Vec<PlacedViewRecord>,
    plan_laid_records: Vec<PlanLaidRecord>,
    selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
}

impl PreCheckEvaluation {
    /// Consume the exact continuation produced before name resolution.
    ///
    /// Omega may target-filter and type the returned syntax before this call,
    /// but the language-level evaluation order and selection authority remain
    /// owned by this continuation.
    pub fn evaluate(
        self,
        typed: &mut psi_typed_trees::TypedTrees,
    ) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
        evaluate_pre_check_with_optional_authority(
            typed,
            &self.plan_laid_records,
            &self.placed_view_records,
            self.selection_authority,
        )
    }
}

pub fn evaluate_pre_resolution(
    syntax_trees: psi_syntax_trees::SyntaxTrees,
) -> Result<PreResolutionEvaluation, Vec<psi_diagnostics::Diagnostic>> {
    evaluate_pre_resolution_with_optional_sources(syntax_trees, None, None)
}

/// Package-aware pre-resolution evaluation.
///
/// Probe compilations must retain the same source/package custody as the
/// authoritative compilation. Otherwise a compile-time machine selected from
/// dependency source loses its owner before the execution-admission gate can
/// inspect it.
pub fn evaluate_pre_resolution_with_sources(
    syntax_trees: psi_syntax_trees::SyntaxTrees,
    sources: Arc<psi_source::SourceMap>,
) -> Result<PreResolutionEvaluation, Vec<psi_diagnostics::Diagnostic>> {
    evaluate_pre_resolution_with_optional_sources(syntax_trees, Some(sources), None)
}

pub fn evaluate_pre_resolution_with_sources_and_authority(
    syntax_trees: psi_syntax_trees::SyntaxTrees,
    sources: Arc<psi_source::SourceMap>,
    selection_authority: Arc<dyn BuildTimeSelectionAuthority>,
) -> Result<PreResolutionEvaluation, Vec<psi_diagnostics::Diagnostic>> {
    evaluate_pre_resolution_with_optional_sources(
        syntax_trees,
        Some(sources),
        Some(selection_authority),
    )
}

fn evaluate_pre_resolution_with_optional_sources(
    syntax_trees: psi_syntax_trees::SyntaxTrees,
    sources: Option<Arc<psi_source::SourceMap>>,
    selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
) -> Result<PreResolutionEvaluation, Vec<psi_diagnostics::Diagnostic>> {
    let mut syntax_trees = const_generic_calls::evaluate_const_generic_calls_with_optional_sources(
        syntax_trees,
        sources.clone(),
        selection_authority.clone(),
    )?;
    psi_syntax_trees_to_symbol_resolved_trees::synthesize_trait_defaults(&mut syntax_trees)?;
    let placed_view_records = placed_views::desugar_placed_views_with_optional_sources(
        &mut syntax_trees,
        sources,
        selection_authority.clone(),
    )?;
    let mut syntax_trees = psi_generic_instances::normalize_pre_resolution(syntax_trees)?;
    let plan_laid_records = desugar_plan_laid_value_types(&mut syntax_trees)?;
    Ok(PreResolutionEvaluation {
        syntax_trees,
        pre_check: PreCheckEvaluation {
            placed_view_records,
            plan_laid_records,
            selection_authority,
        },
    })
}

fn lower_probe_with_optional_sources(
    syntax_trees: &psi_syntax_trees::SyntaxTrees,
    sources: Option<Arc<psi_source::SourceMap>>,
) -> Result<psi_symbol_resolved_trees::SymbolResolvedTrees, Vec<psi_diagnostics::Diagnostic>> {
    match sources {
        Some(sources) => {
            psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
                syntax_trees,
                sources,
            )
        }
        None => psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(syntax_trees),
    }
}

fn evaluate_pre_check_with_optional_authority(
    typed: &mut psi_typed_trees::TypedTrees,
    plan_laid_records: &[PlanLaidRecord],
    placed_view_records: &[PlacedViewRecord],
    selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    evaluate_const_array_lengths_with_authority(typed, selection_authority.clone())?;
    evaluate_const_domain_facts_with_authority(typed, selection_authority.clone())?;
    compute_plan_laid_layouts_with_authority(
        typed,
        plan_laid_records,
        selection_authority.clone(),
    )?;
    validate_placed_view_plans_with_authority(
        typed,
        placed_view_records,
        selection_authority.clone(),
    )?;
    compute_wire_plans_with_authority(typed, selection_authority)
}

#[cfg(test)]
mod tests {
    use super::{
        BuildTimeSelectionAuthority, evaluate_pre_resolution_with_sources,
        evaluate_pre_resolution_with_sources_and_authority, lower_probe_with_optional_sources,
    };
    use psi_core::PackageKeyIdentity;
    use psi_source::{SourceMap, SourceOrigin};
    use psi_source_files_to_tokens::Lexer;
    use psi_tokens_to_syntax_trees::parse_syntax_trees_with_id;
    use psi_typed_trees::types::FixedArrayLength;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const CONST_ARRAY_SOURCE: &str = r#"
        data Main { slots: [i64; table_size()]; }

        machine table_size() -> u64 {
            transition { _ -> (12 + 4) }
        }
    "#;

    fn parsed_source(
        source: &str,
        package: PackageKeyIdentity,
    ) -> (psi_syntax_trees::SyntaxTrees, Arc<SourceMap>) {
        let mut sources = SourceMap::default();
        let source_id = sources
            .add_with_metadata(
                PathBuf::from("cache/selected/main.omg"),
                source.to_owned(),
                PathBuf::from("cache/selected"),
                Some(package),
                SourceOrigin::User,
            )
            .source_id;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees_with_id(source_id, &tokens).expect("parse");
        (syntax, Arc::new(sources))
    }

    fn typed_after_pre_resolution(
        syntax: &psi_syntax_trees::SyntaxTrees,
        sources: Arc<SourceMap>,
    ) -> psi_typed_trees::TypedTrees {
        let resolved = lower_probe_with_optional_sources(syntax, Some(sources))
            .expect("resolve pre-evaluated syntax");
        psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type pre-evaluated syntax")
    }

    #[test]
    fn package_aware_probe_retains_authored_symbol_ownership() {
        let source = "machine selected() {}";
        let package =
            PackageKeyIdentity::from_digest([0x6a; 32]).expect("nonzero package identity");
        let (syntax, sources) = parsed_source(source, package);

        let resolved = lower_probe_with_optional_sources(&syntax, Some(sources))
            .expect("package-aware probe resolution");
        let machine = resolved.machines.first().expect("selected machine");

        assert_eq!(
            resolved.symbols.symbol_package_identity(machine.symbol),
            Some(package)
        );
    }

    #[test]
    fn plain_pre_resolution_owns_one_coherent_pre_check_continuation() {
        let package =
            PackageKeyIdentity::from_digest([0x71; 32]).expect("nonzero package identity");
        let (syntax, sources) = parsed_source(CONST_ARRAY_SOURCE, package);

        let evaluated = evaluate_pre_resolution_with_sources(syntax, sources.clone())
            .expect("plain pre-resolution evaluation");
        let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
        assert!(pre_check.selection_authority.is_none());

        let mut typed = typed_after_pre_resolution(&syntax, sources);
        assert!(
            typed
                .type_reference_table
                .fixed_array_lengths()
                .any(|(_, length)| matches!(length, FixedArrayLength::ConstCall { .. }))
        );
        pre_check
            .evaluate(&mut typed)
            .expect("the matching plain continuation should evaluate exactly once");
        assert!(
            typed
                .type_reference_table
                .fixed_array_lengths()
                .any(|(_, length)| *length == FixedArrayLength::Literal(16))
        );
        assert!(
            !typed
                .type_reference_table
                .fixed_array_lengths()
                .any(|(_, length)| matches!(length, FixedArrayLength::ConstCall { .. }))
        );
    }

    #[derive(Default)]
    struct AllowAllSelections {
        consultations: AtomicUsize,
    }

    impl BuildTimeSelectionAuthority for AllowAllSelections {
        fn allows_declaration_selection(
            &self,
            _requester: PackageKeyIdentity,
            _owner: PackageKeyIdentity,
        ) -> bool {
            self.consultations.fetch_add(1, Ordering::SeqCst);
            true
        }

        fn package_label(&self, identity: PackageKeyIdentity) -> String {
            format!("package-{identity:?}")
        }
    }

    #[test]
    fn authority_pre_resolution_retains_the_exact_authority_for_pre_check() {
        let package =
            PackageKeyIdentity::from_digest([0x72; 32]).expect("nonzero package identity");
        let source = r#"
            machine table_size() -> u64 {
                transition { _ -> 4 }
            }

            data FixedBuffer<const N: u64> {
                items: [i32 in Wrapping; N];
            }

            data Main {
                generic: FixedBuffer<table_size()>;
                slots: [i64; table_size()];
            }
        "#;
        let (syntax, sources) = parsed_source(source, package);
        let counter = Arc::new(AllowAllSelections::default());
        let authority: Arc<dyn BuildTimeSelectionAuthority> = counter.clone();
        let expected_authority = authority.clone();

        let evaluated =
            evaluate_pre_resolution_with_sources_and_authority(syntax, sources.clone(), authority)
                .expect("authority-bearing pre-resolution evaluation");
        let pre_resolution_consultations = counter.consultations.load(Ordering::SeqCst);
        assert!(
            pre_resolution_consultations > 0,
            "const-generic evaluation must consult the selected authority before resolution"
        );
        let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
        assert!(Arc::ptr_eq(
            pre_check
                .selection_authority
                .as_ref()
                .expect("retained selection authority"),
            &expected_authority,
        ));

        let mut typed = typed_after_pre_resolution(&syntax, sources);
        pre_check
            .evaluate(&mut typed)
            .expect("the authority-bearing continuation should evaluate exactly once");
        assert!(
            counter.consultations.load(Ordering::SeqCst) > pre_resolution_consultations,
            "the retained authority must be consulted again by fixed-array pre-check evaluation"
        );
        assert!(
            typed
                .type_reference_table
                .fixed_array_lengths()
                .all(|(_, length)| !matches!(length, FixedArrayLength::ConstCall { .. }))
        );
    }
}
