#![forbid(unsafe_code)]

//! Target-neutral admission and execution of compile-time Omega machines.

mod access_plans;
mod admission;
mod build_machines;
mod const_domain_facts;
mod const_generic_calls;
mod const_generic_expressions;
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
    BuildMachineFilesystemMetadataLayout, BuildMachineFilesystemSponsor, PreparedBuildMachineEntry,
    PreparedBuildMachineProgram, evaluate_build_machine_arguments_measured,
    evaluate_build_machine_arguments_measured_with_sponsor,
    evaluate_build_machine_entry_arguments_measured,
    evaluate_build_machine_entry_arguments_measured_with_sponsor,
};
pub use checked_interpreter::{CURRENT_EVALUATION_SEMANTICS, EvaluationUsage, MeasuredEvaluation};
pub use const_domain_facts::{
    evaluate_const_domain_facts, evaluate_const_domain_facts_with_authority,
};
pub use const_generic_calls::evaluate_const_generic_calls;
pub use const_lengths::{
    evaluate_const_array_lengths, evaluate_const_array_lengths_with_authority,
    evaluate_zero_argument_machine, evaluate_zero_argument_machine_for_invocation,
};
pub use layout_plans::{
    BuildTimeValue, ValidatedConstDepthEightNestedSumOccurrenceMaterialization,
    ValidatedConstDepthEighteenNestedSumOccurrenceMaterialization,
    ValidatedConstDepthElevenNestedSumOccurrenceMaterialization,
    ValidatedConstDepthFifteenNestedSumOccurrenceMaterialization,
    ValidatedConstDepthFiveNestedSumOccurrenceMaterialization,
    ValidatedConstDepthFourNestedSumOccurrenceMaterialization,
    ValidatedConstDepthFourteenNestedSumOccurrenceMaterialization,
    ValidatedConstDepthNineNestedSumOccurrenceMaterialization,
    ValidatedConstDepthNineteenNestedSumOccurrenceMaterialization,
    ValidatedConstDepthSevenNestedSumOccurrenceMaterialization,
    ValidatedConstDepthSeventeenNestedSumOccurrenceMaterialization,
    ValidatedConstDepthSixNestedSumOccurrenceMaterialization,
    ValidatedConstDepthSixteenNestedSumOccurrenceMaterialization,
    ValidatedConstDepthTenNestedSumOccurrenceMaterialization,
    ValidatedConstDepthThirteenNestedSumOccurrenceMaterialization,
    ValidatedConstDepthThreeNestedSumOccurrenceMaterialization,
    ValidatedConstDepthTwelveNestedSumOccurrenceMaterialization,
    ValidatedConstDepthTwentyNestedSumOccurrenceMaterialization,
    ValidatedConstDepthTwentyOneNestedSumOccurrenceMaterialization,
    ValidatedConstDepthTwentyThreeNestedSumOccurrenceMaterialization,
    ValidatedConstDepthTwentyTwoNestedSumOccurrenceMaterialization,
    ValidatedConstDepthTwoNestedSumOccurrenceMaterialization, ValidatedConstMaterialization,
    ValidatedConstNestedSumRecordOccurrenceMaterialization,
    ValidatedConstRecordSumArrayElementMaterialization,
    ValidatedConstRecordSumArrayElementSelection, ValidatedConstRecordSumArrayFieldMaterialization,
    ValidatedConstRecordSumFieldMaterialization,
    ValidatedConstRecordWithDepthEightNestedSumsMaterialization,
    ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization,
    ValidatedConstRecordWithDepthElevenNestedSumsMaterialization,
    ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization,
    ValidatedConstRecordWithDepthFiveNestedSumsMaterialization,
    ValidatedConstRecordWithDepthFourNestedSumsMaterialization,
    ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization,
    ValidatedConstRecordWithDepthNineNestedSumsMaterialization,
    ValidatedConstRecordWithDepthNineteenNestedSumsMaterialization,
    ValidatedConstRecordWithDepthSevenNestedSumsMaterialization,
    ValidatedConstRecordWithDepthSeventeenNestedSumsMaterialization,
    ValidatedConstRecordWithDepthSixNestedSumsMaterialization,
    ValidatedConstRecordWithDepthSixteenNestedSumsMaterialization,
    ValidatedConstRecordWithDepthTenNestedSumsMaterialization,
    ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization,
    ValidatedConstRecordWithDepthThreeNestedSumMaterialization,
    ValidatedConstRecordWithDepthThreeNestedSumsMaterialization,
    ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization,
    ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization,
    ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization,
    ValidatedConstRecordWithDepthTwentyThreeNestedSumsMaterialization,
    ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization,
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
    validate_const_materializable_record_with_depth_eight_nested_sums,
    validate_const_materializable_record_with_depth_eighteen_nested_sums,
    validate_const_materializable_record_with_depth_eleven_nested_sums,
    validate_const_materializable_record_with_depth_fifteen_nested_sums,
    validate_const_materializable_record_with_depth_five_nested_sums,
    validate_const_materializable_record_with_depth_four_nested_sums,
    validate_const_materializable_record_with_depth_fourteen_nested_sums,
    validate_const_materializable_record_with_depth_nine_nested_sums,
    validate_const_materializable_record_with_depth_nineteen_nested_sums,
    validate_const_materializable_record_with_depth_seven_nested_sums,
    validate_const_materializable_record_with_depth_seventeen_nested_sums,
    validate_const_materializable_record_with_depth_six_nested_sums,
    validate_const_materializable_record_with_depth_sixteen_nested_sums,
    validate_const_materializable_record_with_depth_ten_nested_sums,
    validate_const_materializable_record_with_depth_thirteen_nested_sums,
    validate_const_materializable_record_with_depth_three_nested_sum,
    validate_const_materializable_record_with_depth_three_nested_sums,
    validate_const_materializable_record_with_depth_twelve_nested_sums,
    validate_const_materializable_record_with_depth_twenty_nested_sums,
    validate_const_materializable_record_with_depth_twenty_one_nested_sums,
    validate_const_materializable_record_with_depth_twenty_three_nested_sums,
    validate_const_materializable_record_with_depth_twenty_two_nested_sums,
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
    syntax_trees: syntax_trees::SyntaxTrees,
    pre_check: PreCheckEvaluation,
}

impl PreResolutionEvaluation {
    /// Separate the syntax consumed by target filtering and name resolution
    /// from the opaque continuation that owns the matching typed-tree work.
    pub fn into_syntax_and_pre_check(self) -> (syntax_trees::SyntaxTrees, PreCheckEvaluation) {
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
        typed: &mut typed_trees::TypedTrees,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        evaluate_pre_check_with_optional_authority(
            typed,
            &self.plan_laid_records,
            &self.placed_view_records,
            self.selection_authority,
            0,
        )
    }

    /// Consume the continuation for syntax appended to an already evaluated
    /// typed checkpoint. Global pending const work remains detectable, while
    /// wire-plan publication is restricted to extension-owned schemas.
    pub fn evaluate_extension(
        self,
        typed: &mut typed_trees::TypedTrees,
        wire_schema_frontier: usize,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        evaluate_pre_check_with_optional_authority(
            typed,
            &self.plan_laid_records,
            &self.placed_view_records,
            self.selection_authority,
            wire_schema_frontier,
        )
    }
}

pub fn evaluate_pre_resolution(
    syntax_trees: syntax_trees::SyntaxTrees,
) -> Result<PreResolutionEvaluation, Vec<diagnostics::Diagnostic>> {
    evaluate_pre_resolution_with_optional_sources(syntax_trees, None, &[], None)
}

/// Package-aware pre-resolution evaluation.
///
/// Probe compilations must retain the same source/package custody as the
/// authoritative compilation. Otherwise a compile-time machine selected from
/// dependency source loses its owner before the execution-admission gate can
/// inspect it.
pub fn evaluate_pre_resolution_with_sources(
    syntax_trees: syntax_trees::SyntaxTrees,
    sources: Arc<source::SourceMap>,
) -> Result<PreResolutionEvaluation, Vec<diagnostics::Diagnostic>> {
    evaluate_pre_resolution_with_sources_and_top_level_bindings(syntax_trees, sources, Vec::new())
}

/// Preserve the loader's exact requester-to-declaration bindings in every
/// normalization and probe compilation preceding ordinary resolution.
pub fn evaluate_pre_resolution_with_sources_and_top_level_bindings(
    syntax_trees: syntax_trees::SyntaxTrees,
    sources: Arc<source::SourceMap>,
    source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
) -> Result<PreResolutionEvaluation, Vec<diagnostics::Diagnostic>> {
    evaluate_pre_resolution_with_optional_sources(
        syntax_trees,
        Some(sources),
        &source_scoped_top_level_bindings,
        None,
    )
}

pub fn evaluate_pre_resolution_with_sources_and_authority(
    syntax_trees: syntax_trees::SyntaxTrees,
    sources: Arc<source::SourceMap>,
    selection_authority: Arc<dyn BuildTimeSelectionAuthority>,
) -> Result<PreResolutionEvaluation, Vec<diagnostics::Diagnostic>> {
    evaluate_pre_resolution_with_sources_top_level_bindings_and_authority(
        syntax_trees,
        sources,
        Vec::new(),
        selection_authority,
    )
}

/// Add execution-admission authority without changing the loader's exact
/// source-scoped name bindings used by normalization and probe resolution.
pub fn evaluate_pre_resolution_with_sources_top_level_bindings_and_authority(
    syntax_trees: syntax_trees::SyntaxTrees,
    sources: Arc<source::SourceMap>,
    source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    selection_authority: Arc<dyn BuildTimeSelectionAuthority>,
) -> Result<PreResolutionEvaluation, Vec<diagnostics::Diagnostic>> {
    evaluate_pre_resolution_with_optional_sources(
        syntax_trees,
        Some(sources),
        &source_scoped_top_level_bindings,
        Some(selection_authority),
    )
}

fn evaluate_pre_resolution_with_optional_sources(
    syntax_trees: syntax_trees::SyntaxTrees,
    sources: Option<Arc<source::SourceMap>>,
    source_scoped_top_level_bindings: &[symbols::SourceScopedTopLevelBinding],
    selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
) -> Result<PreResolutionEvaluation, Vec<diagnostics::Diagnostic>> {
    let syntax_trees = const_generic_expressions::evaluate(
        syntax_trees,
        sources.clone(),
        source_scoped_top_level_bindings,
        selection_authority.as_deref(),
    )?;
    let mut syntax_trees = const_generic_calls::evaluate_const_generic_calls_with_optional_sources(
        syntax_trees,
        sources.clone(),
        source_scoped_top_level_bindings,
        selection_authority.clone(),
    )?;
    syntax_trees_to_symbol_resolved_trees::synthesize_trait_defaults(&mut syntax_trees)?;
    let placed_view_records = placed_views::desugar_placed_views_with_optional_sources(
        &mut syntax_trees,
        sources.clone(),
        source_scoped_top_level_bindings,
        selection_authority.clone(),
    )?;
    let mut syntax_trees = normalize_generic_data_with_optional_sources(
        syntax_trees,
        sources,
        source_scoped_top_level_bindings,
    )?;
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

fn normalize_generic_data_with_optional_sources(
    syntax_trees: syntax_trees::SyntaxTrees,
    sources: Option<Arc<source::SourceMap>>,
    source_scoped_top_level_bindings: &[symbols::SourceScopedTopLevelBinding],
) -> Result<syntax_trees::SyntaxTrees, Vec<diagnostics::Diagnostic>> {
    match sources {
        Some(sources) => syntax_trees_to_symbol_resolved_trees::normalize_generic_data_with_sources_and_top_level_bindings(syntax_trees, sources, source_scoped_top_level_bindings.to_vec()),
        None => syntax_trees_to_symbol_resolved_trees::normalize_generic_data(syntax_trees),
    }
}

fn lower_probe_with_optional_sources(
    syntax_trees: &syntax_trees::SyntaxTrees,
    sources: Option<Arc<source::SourceMap>>,
    source_scoped_top_level_bindings: &[symbols::SourceScopedTopLevelBinding],
) -> Result<symbol_resolved_trees::SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    match sources {
        Some(sources) => syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources_and_top_level_bindings(
            syntax_trees,
            sources,
            source_scoped_top_level_bindings.to_vec(),
        ),
        None => syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(syntax_trees),
    }
}

fn evaluate_pre_check_with_optional_authority(
    typed: &mut typed_trees::TypedTrees,
    plan_laid_records: &[PlanLaidRecord],
    placed_view_records: &[PlacedViewRecord],
    selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    wire_schema_frontier: usize,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
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
    wire_plans::compute_wire_plans_with_authority_from(
        typed,
        selection_authority,
        wire_schema_frontier,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        BuildTimeSelectionAuthority, evaluate_pre_resolution_with_sources,
        evaluate_pre_resolution_with_sources_and_authority, lower_probe_with_optional_sources,
    };
    use semantic_vocabulary::PackageKeyIdentity;
    use source::{SourceMap, SourceOrigin};
    use source_files_to_tokens::Lexer;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokens_to_syntax_trees::parse_syntax_trees_with_id;
    use typed_trees::types::FixedArrayLength;

    const CONST_ARRAY_SOURCE: &str = r#"
        data Main { slots: [i64; table_size()]; }

        machine table_size() -> u64 {
            transition { _ -> (12 + 4) }
        }
    "#;

    #[test]
    fn probe_retains_exact_loader_module_alias_binding() {
        let mut sources = SourceMap::default();
        let requester_text = "use selected::combat::Damage; data Main { damage: Damage; }";
        let declaration_text = "module combat; pub data Damage { amount: u64; }";
        let requester = sources
            .add(PathBuf::from("root/main.omg"), requester_text.to_owned())
            .source_id;
        let declaration = sources
            .add(
                PathBuf::from("dependency/combat.omg"),
                declaration_text.to_owned(),
            )
            .source_id;
        let mut syntax = syntax_trees::SyntaxTrees::default();
        for (source_id, text) in [(requester, requester_text), (declaration, declaration_text)] {
            let tokens = Lexer::new(text)
                .tokenize()
                .expect("tokenize module alias probe");
            tokens_to_syntax_trees::parse_syntax_trees_into_with_id(
                &mut syntax,
                source_id,
                &tokens,
            )
            .expect("parse module alias probe");
        }
        let bindings = [symbols::SourceScopedTopLevelBinding::module_import(
            requester,
            declaration,
            "selected::combat::Damage",
            1,
        )];
        let sources = Arc::new(sources);
        let unbound = lower_probe_with_optional_sources(&syntax, Some(sources.clone()), &[])
            .expect("unresolved nominal names remain for typing");
        let root = unbound
            .data_definitions
            .iter()
            .find(|definition| definition.name.as_str() == "Main")
            .expect("unbound requester");
        let symbol_resolved_trees::data::DataMember::Field(field) =
            &unbound.data_members(root.members)[0]
        else {
            panic!("unbound requester field");
        };
        let symbol_resolved_trees::types::TypeReference::Named { symbol, .. } =
            &field.type_reference
        else {
            panic!("unbound nominal field");
        };
        assert!(
            !symbol.is_valid(),
            "alias cannot be reconstructed without loader bindings"
        );
        let resolved = lower_probe_with_optional_sources(&syntax, Some(sources), &bindings)
            .expect("resolve probe with exact alias binding");
        let declaration = resolved
            .data_definitions
            .iter()
            .find(|definition| definition.name.as_str() == "Damage")
            .expect("module data");
        let root = resolved
            .data_definitions
            .iter()
            .find(|definition| definition.name.as_str() == "Main")
            .expect("requester data");
        let symbol_resolved_trees::data::DataMember::Field(field) =
            &resolved.data_members(root.members)[0]
        else {
            panic!("requester field");
        };
        let symbol_resolved_trees::types::TypeReference::Named { symbol, .. } =
            &field.type_reference
        else {
            panic!("nominal module field");
        };
        assert_eq!(*symbol, declaration.symbol);
    }

    fn parsed_source(
        source: &str,
        package: PackageKeyIdentity,
    ) -> (syntax_trees::SyntaxTrees, Arc<SourceMap>) {
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
        syntax: &syntax_trees::SyntaxTrees,
        sources: Arc<SourceMap>,
    ) -> typed_trees::TypedTrees {
        let resolved = lower_probe_with_optional_sources(syntax, Some(sources), &[])
            .expect("resolve pre-evaluated syntax");
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type pre-evaluated syntax")
    }

    #[test]
    fn package_aware_probe_retains_authored_symbol_ownership() {
        let source = "machine selected() {}";
        let package =
            PackageKeyIdentity::from_digest([0x6a; 32]).expect("nonzero package identity");
        let (syntax, sources) = parsed_source(source, package);

        let resolved = lower_probe_with_optional_sources(&syntax, Some(sources), &[])
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
    fn package_permission_does_not_authorize_builtin_operator_substitution() {
        let package = PackageKeyIdentity::from_digest([0x73; 32]).expect("nonzero package");
        let source = r#"
            data Math {}
            boundary operator % Math::remainder(left: u64, right: u64) -> u64;
            data Provider {}
            machine Provider::remainder(left: u64, right: u64) -> u64 satisfies Math::remainder { 0 }
            machine count() -> u64 { 7u64 % 2 }
            data Buffer<const N: u64> { values: [u8; N]; }
            data Main { value: Buffer<count()>; }
        "#;
        let (syntax, sources) = parsed_source(source, package);
        let result = evaluate_pre_resolution_with_sources_and_authority(
            syntax,
            sources,
            Arc::new(AllowAllSelections::default()),
        );
        let errors = match result {
            Ok(_) => panic!("package permission cannot supply selected operator semantics"),
            Err(errors) => errors,
        };
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("requires exact authored selection")),
            "{errors:?}"
        );
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
