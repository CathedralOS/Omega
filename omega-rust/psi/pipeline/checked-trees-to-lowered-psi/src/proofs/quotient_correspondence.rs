//! Proof-only quotient correspondence retention at the checked-to-Terminal boundary.
//!
//! This bridge carries the all-or-nothing, source-handle-free direct-`define`
//! and bounded transport-backed direct-`lift` batch into semantic-module
//! identity. It does not emit or authorize an executable quotient operation.
//!
//! [`retain_checked_quotient_correspondences`] is the production entrance:
//! machine lowering calls it once the selected module is complete and before
//! module validation. A program without a `Quotient::define`/`Quotient::lift`
//! request leaves the module untouched. A program with requests either
//! installs the complete batch the semantic extractor admits, or fails closed
//! with [`LoweringError::UnadmittedQuotientRequest`] naming every extraction
//! diagnostic — no partial table and no silently dropped request. The
//! installed table still owns no executable operation: module validation
//! afterwards rejects a nonempty table with
//! `ModuleError::NonExecutableQuotientCorrespondence` until executable
//! quotient lowering exists
//! ([published quotient correspondence](../../../../../../wiki/spec/proofs/quotients.md#published-quotient-correspondence)).
//!
//! Boundary: ordinary checked validation (`validation::validate_program`,
//! `reject_quotient_operation_requests`) still rejects every request before
//! checked trees exist, and the checked stage exits every value path whose
//! call carries `quotient_operation`, so on the current route the production
//! entrance sees no request. Its tests substitute a request-bearing typed
//! program into a checked baseline, the same input the extractor reads.

use checked_trees::CheckedTrees;
use checked_trees::expression::ExpressionNode;
use terminal_psi::{TerminalModule, retain_non_executable_quotient_correspondence};
use validation::NonExecutableQuotientCorrespondenceBatch;

use crate::lowering_error::LoweringError;

/// Install a complete proof-only quotient-correspondence batch derived by
/// semantic validation.
///
/// This entry point is separate from executable quotient admission:
/// [`retain_checked_quotient_correspondences`] is its production caller and
/// producer tests exercise the source-free carrier directly. The opaque batch
/// can only be constructed by the all-or-nothing semantic extractor; raw
/// typed-tree vocabulary does not cross into this Terminal producer.
pub(crate) fn install_non_executable_quotient_correspondences(
    batch: NonExecutableQuotientCorrespondenceBatch,
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    let mut retained = batch
        .into_correspondences()
        .into_iter()
        .map(retain_non_executable_quotient_correspondence)
        .collect::<Vec<_>>();
    retained.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut candidate = module.clone();
    candidate.quotient_correspondences = retained;
    terminal_verifier::validate_module_representation(&candidate)
        .map_err(LoweringError::InvalidTerminalModule)?;
    module.quotient_correspondences = candidate.quotient_correspondences;
    Ok(())
}

/// Retain the checked program's quotient requests on the selected module.
///
/// The semantic extractor rederives every request from the checked stage's
/// retained program input (`CheckedTrees::typed`) rather than trusting any
/// checked-stage summary. With no request present the module is returned
/// untouched and the extractor never runs.
pub(crate) fn retain_checked_quotient_correspondences(
    checked: &CheckedTrees,
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    if !program_carries_quotient_request(checked) {
        return Ok(());
    }
    let batch = validation::extract_non_executable_quotient_correspondences(&checked.typed)
        .map_err(|diagnostics| LoweringError::UnadmittedQuotientRequest {
            diagnostics: diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.message)
                .collect(),
        })?;
    install_non_executable_quotient_correspondences(batch, module)
}

fn program_carries_quotient_request(checked: &CheckedTrees) -> bool {
    checked
        .typed
        .expression_table
        .iter_expressions()
        .any(|(_, expression)| {
            matches!(expression, ExpressionNode::Call(call) if call.quotient_operation.is_some())
        })
}

#[cfg(test)]
mod tests {
    //! The production entrance is driven through a checked baseline whose
    //! retained typed program is swapped for a request-bearing one, exactly
    //! the input the extractor reads: ordinary checked validation rejects
    //! every request today, so no `CheckedTrees` can carry one through
    //! `lower_typed_trees`, and the swapped program cannot drive the rest of
    //! `lower_machine` (its checked facts join the baseline's symbols). The
    //! baseline module comes from the ordinary lowering route so the
    //! installed rows land beside a real selected machine, and the execution
    //! gate that route applies afterwards is asserted directly.

    use std::path::PathBuf;
    use std::sync::Arc;

    use semantic_vocabulary::PackageKeyIdentity;
    use source::{SourceMap, SourceOrigin};
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};
    use typed_trees::TypedTrees;
    use typed_trees_to_checked_trees::lower_typed_trees;

    use super::retain_checked_quotient_correspondences;
    use crate::lower_machine;
    use crate::lowering_error::LoweringError;

    const CORE_RELATION: &str = include_str!("../../../../../../source/library/core/relation.omg");

    const EQUIVALENCE_PRELUDE: &str = r#"
use omega::language::core::relation;

data Representative {
    case Zero;
    case Next(previous: Representative);
}

proposition equivalent(a: Representative, b: Representative) = a == b;

machine equivalent_reflexive(a: Representative)
ensures a == a
{
}

machine equivalent_symmetric(a: Representative, b: Representative)
requires a == b
ensures b == a
{
}

machine equivalent_transitive(
    a: Representative,
    b: Representative,
    c: Representative
)
requires
    a == b
    b == c
ensures a == c
{
}

RepresentativeEquivalence: satisfies Equivalence<Representative, equivalent> {
    Reflexive::reflexive = equivalent_reflexive;
    Symmetric::symmetric = equivalent_symmetric;
    Transitive::transitive = equivalent_transitive;
}

data EquivalenceClass = Representative % equivalent
where equivalent satisfies
    Equivalence<Representative, equivalent>
    as RepresentativeEquivalence;

machine representative(value: Representative) -> Representative {
    value
}

machine representative_respects(left: Representative, right: Representative)
requires equivalent(left, right)
ensures equivalent(representative(left), representative(right))
{
}
"#;

    const DIRECT_DEFINE_REQUEST: &str = r#"
machine admitted(value: EquivalenceClass) -> EquivalenceClass {
    Quotient::define<representative, representative_respects>(value)
}
"#;

    /// A congruence-only `lift` has no canonical wire payload yet, so the
    /// extractor refuses the whole batch that contains it.
    const CONGRUENCE_ONLY_LIFT_REQUEST: &str = r#"
machine unsupported(value: EquivalenceClass) -> EquivalenceClass {
    Quotient::lift<representative, representative_respects>(value)
}
"#;

    fn quotient_program(source: &str) -> TypedTrees {
        let package =
            PackageKeyIdentity::from_digest([0x72; 32]).expect("nonzero package identity");
        let mut sources = SourceMap::default();
        let core_source_id = sources
            .add_with_metadata(
                PathBuf::from("source/library/core/relation.omg"),
                CORE_RELATION.to_owned(),
                PathBuf::from("source/library/core"),
                None,
                SourceOrigin::Toolchain,
            )
            .source_id;
        let source_id = sources
            .add_with_metadata(
                PathBuf::from("managed/quotient/main.omg"),
                source.to_owned(),
                PathBuf::from("managed/quotient"),
                Some(package),
                SourceOrigin::User,
            )
            .source_id;
        let core_tokens = Lexer::new(CORE_RELATION).tokenize().expect("tokenize core");
        let mut syntax =
            parse_syntax_trees_with_id(core_source_id, &core_tokens).expect("parse core relation");
        let tokens = Lexer::new(source).tokenize().expect("tokenize fixture");
        parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens).expect("parse fixture");
        let resolved = resolve(ResolutionRequest {
            syntax: &syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
        })
        .expect("package-aware resolution");
        let mut program = lower_symbol_resolved_trees(&resolved).expect("type lowering");
        // The extractor requires checked termination on the representative,
        // theorem and requesting machines; the fixture pins it directly, as
        // the extractor's own tests do.
        let eligible = program
            .machines()
            .iter()
            .enumerate()
            .filter_map(|(position, machine)| {
                let name = program.symbols.name(machine.symbol);
                (matches!(name, "representative" | "representative_respects")
                    || name.starts_with("admitted")
                    || name.starts_with("unsupported"))
                .then_some(position)
            })
            .collect::<Vec<_>>();
        assert!(eligible.len() >= 3);
        for position in eligible {
            program.machines_mut()[position]
                .termination_plan
                .checked_summary = language_semantics::TerminationGuarantee::Terminates {
                premises: Vec::new(),
            };
        }
        program
    }

    fn baseline_checked() -> checked_trees::CheckedTrees {
        let source = r#"
            machine baseline(value: i32) -> i32
            requires 0i32 == 0i32
            ensures 0i32 == 0i32
            {
                value
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize baseline");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse baseline");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve baseline");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type baseline");
        lower_typed_trees(typed).expect("check baseline")
    }

    fn baseline_module() -> terminal_psi::TerminalModule {
        lower_machine(&baseline_checked(), "baseline")
            .expect("lower baseline")
            .semantic_module
    }

    /// The checked baseline with a request-bearing typed program substituted
    /// as its retained input: the only field the production entrance reads.
    fn checked_with_requests(source: &str) -> checked_trees::CheckedTrees {
        let mut checked = baseline_checked();
        checked.typed = quotient_program(source);
        checked
    }

    #[test]
    fn a_program_without_requests_leaves_the_module_untouched() {
        let checked = baseline_checked();
        let mut module = baseline_module();
        let before = module.clone();
        retain_checked_quotient_correspondences(&checked, &mut module)
            .expect("no request retains nothing");
        assert_eq!(module, before);
        assert!(module.quotient_correspondences.is_empty());
    }

    #[test]
    fn an_admitted_direct_define_installs_its_row_and_stays_non_executable() {
        let checked =
            checked_with_requests(&format!("{EQUIVALENCE_PRELUDE}{DIRECT_DEFINE_REQUEST}"));
        // Ordinary validation still refuses the request; the production
        // entrance is what carries the admitted batch once it does not.
        assert!(validation::validate_program(&checked.typed).is_err());
        let mut module = baseline_module();
        retain_checked_quotient_correspondences(&checked, &mut module)
            .expect("the admitted batch installs");
        let [row] = module.quotient_correspondences.as_slice() else {
            panic!("one retained define row");
        };
        assert_eq!(
            row.certificate.operation_kind,
            language_semantics::quotient_correspondence::QuotientCorrespondenceOperationKind::Define
        );
        let mut rederived =
            validation::extract_non_executable_quotient_correspondences(&checked.typed)
                .expect("rederive")
                .into_correspondences()
                .into_iter()
                .map(terminal_psi::retain_non_executable_quotient_correspondence)
                .collect::<Vec<_>>();
        rederived.sort_by(|left, right| left.identity.cmp(&right.identity));
        assert_eq!(module.quotient_correspondences, rederived);
        // The table is proof-only: representation replay accepts it and the
        // execution gate machine lowering applies afterwards refuses it.
        terminal_verifier::validate_module_representation(&module)
            .expect("representation replay accepts the retained row");
        assert_eq!(
            terminal_verifier::validate_module(&module).unwrap_err(),
            terminal_verifier::ModuleError::NonExecutableQuotientCorrespondence
        );
        let bytes = terminal_codec::encode_module(&module).expect("encode retained row");
        assert_eq!(terminal_codec::decode_module(&bytes), Ok(module));
    }

    #[test]
    fn an_unadmitted_request_shape_fails_closed_and_installs_nothing() {
        let checked = checked_with_requests(&format!(
            "{EQUIVALENCE_PRELUDE}{DIRECT_DEFINE_REQUEST}{CONGRUENCE_ONLY_LIFT_REQUEST}"
        ));
        let mut module = baseline_module();
        let before = module.clone();
        let error = retain_checked_quotient_correspondences(&checked, &mut module)
            .expect_err("one unadmitted request refuses the whole batch");
        let LoweringError::UnadmittedQuotientRequest { diagnostics } = error else {
            panic!("expected the unadmitted-request refusal, got {error:?}");
        };
        assert!(
            diagnostics
                .iter()
                .any(|message| message.contains("direct transport-backed `lift` only")),
            "diagnostics name the unadmitted shape: {diagnostics:?}"
        );
        assert_eq!(module, before);
    }

    #[test]
    fn a_replay_failure_installs_nothing() {
        let checked =
            checked_with_requests(&format!("{EQUIVALENCE_PRELUDE}{DIRECT_DEFINE_REQUEST}"));
        let mut module = baseline_module();
        module.entry = semantic_vocabulary::MachineId::new(99).expect("nonzero invalid entry");
        let before = module.clone();
        let error = retain_checked_quotient_correspondences(&checked, &mut module)
            .expect_err("the candidate module replays before rows commit");
        assert!(matches!(
            error,
            LoweringError::InvalidTerminalModule(
                terminal_verifier::ModuleError::UnknownEntryMachine(_)
            )
        ));
        assert_eq!(module, before);
    }
}
