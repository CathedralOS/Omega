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
//! program into a checked baseline, the same input the extractor reads, and
//! answer termination from `facts.termination` as the compiler route does.

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
/// checked-stage summary, except termination eligibility: the typed machine
/// carries no guarantee on the compiler route (the checked stage proves
/// termination after validation into `facts.termination`), so that one
/// judgment reads the checked termination facts. With no request present the
/// module is returned untouched and the extractor never runs.
pub(crate) fn retain_checked_quotient_correspondences(
    checked: &CheckedTrees,
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    if !program_carries_quotient_request(checked) {
        return Ok(());
    }
    let checked_termination = |machine: symbols::SymbolHandle| {
        checked
            .facts
            .termination
            .for_machine(machine)
            .map(|plan| plan.checked_summary.clone())
    };
    let batch = validation::extract_non_executable_quotient_correspondences_with_termination(
        &checked.typed,
        &checked_termination,
    )
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

    use crate::TerminalMachineSelection;
    use std::path::PathBuf;
    use std::sync::Arc;

    use semantic_vocabulary::PackageKeyIdentity;
    use source::{SourceMap, SourceOrigin};
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};
    use typed_trees::TypedTrees;
    use typed_trees_to_checked_trees::CheckingRequest;
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
        // As on the compiler route, the typed machines carry no checked
        // termination guarantee; the checked termination facts supply it.
        lower_symbol_resolved_trees(&resolved).expect("type lowering")
    }

    /// The machines the extractor requires an unconditional checked
    /// termination guarantee for: the representative, the selected theorem
    /// and the requesting machine.
    fn eligibility_machines(program: &TypedTrees) -> Vec<symbols::SymbolHandle> {
        let symbols = program
            .machines()
            .iter()
            .filter(|machine| {
                let name = program.symbols.name(machine.symbol);
                matches!(name, "representative" | "representative_respects")
                    || name.starts_with("admitted")
                    || name.starts_with("unsupported")
            })
            .map(|machine| machine.symbol)
            .collect::<Vec<_>>();
        assert!(symbols.len() >= 3);
        symbols
    }

    /// Record `guarantee` as the checked termination fact of every
    /// eligibility machine, the way `build_check_facts` records what the
    /// checked stage proved.
    fn record_checked_termination(
        checked: &mut checked_trees::CheckedTrees,
        guarantee: language_semantics::TerminationGuarantee,
    ) {
        for symbol in eligibility_machines(&checked.typed) {
            let machine = checked
                .typed
                .machines()
                .iter()
                .find(|machine| machine.symbol == symbol)
                .expect("eligibility machine");
            let mut plan = machine.termination_plan.clone();
            plan.checked_summary = guarantee.clone();
            checked
                .facts
                .termination
                .machines
                .retain(|fact| fact.machine != symbol);
            checked
                .facts
                .termination
                .machines
                .push(checked_trees::MachineTerminationFact {
                    machine: symbol,
                    plan,
                });
        }
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
        lower_typed_trees(typed, &CheckingRequest::settled()).expect("check baseline")
    }

    fn baseline_module() -> terminal_psi::TerminalModule {
        lower_machine(
            &baseline_checked(),
            TerminalMachineSelection::Name("baseline"),
        )
        .expect("lower baseline")
        .semantic_module
    }

    /// The checked baseline with a request-bearing typed program substituted
    /// as its retained input and the eligibility machines' checked termination
    /// facts recorded as proved: the two inputs the production entrance reads.
    fn checked_with_requests(source: &str) -> checked_trees::CheckedTrees {
        let mut checked = baseline_checked();
        checked.typed = quotient_program(source);
        record_checked_termination(
            &mut checked,
            language_semantics::TerminationGuarantee::Terminates {
                premises: Vec::new(),
            },
        );
        checked
    }

    fn assert_unadmitted_naming(error: LoweringError, fragment: &str) {
        let LoweringError::UnadmittedQuotientRequest { diagnostics } = error else {
            panic!("expected the unadmitted-request refusal, got {error:?}");
        };
        assert!(
            diagnostics.iter().any(|message| message.contains(fragment)),
            "diagnostics name `{fragment}`: {diagnostics:?}"
        );
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
        // The typed machines carry no guarantee, so the typed-summary
        // extractor (what ordinary validation consults) refuses the batch;
        // the production entrance admits it from the checked facts.
        assert!(checked.typed.machines().iter().all(|machine| matches!(
            machine.termination_plan.checked_summary,
            language_semantics::TerminationGuarantee::NoGuarantee
        )));
        assert!(
            validation::extract_non_executable_quotient_correspondences(&checked.typed).is_err()
        );
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
            validation::extract_non_executable_quotient_correspondences_with_termination(
                &checked.typed,
                &|machine: symbols::SymbolHandle| {
                    checked
                        .facts
                        .termination
                        .for_machine(machine)
                        .map(|plan| plan.checked_summary.clone())
                },
            )
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
        assert_unadmitted_naming(error, "direct transport-backed `lift` only");
        assert_eq!(module, before);
    }

    #[test]
    fn termination_is_read_from_the_checked_facts_not_the_typed_summary() {
        let source = format!("{EQUIVALENCE_PRELUDE}{DIRECT_DEFINE_REQUEST}");
        // No checked termination fact at all: the checked stage proved
        // nothing, so the batch is refused at the termination fence.
        let mut unproved = baseline_checked();
        unproved.typed = quotient_program(&source);
        let mut module = baseline_module();
        let before = module.clone();
        let error = retain_checked_quotient_correspondences(&unproved, &mut module)
            .expect_err("no checked guarantee refuses the batch");
        assert_unadmitted_naming(
            error,
            "purity, termination, or theorem crash eligibility is incomplete",
        );
        assert_eq!(module, before);

        // A typed summary alone does not admit: the checked facts record no
        // guarantee for the same machines, and the facts are authoritative.
        let mut typed_only = baseline_checked();
        typed_only.typed = quotient_program(&source);
        for symbol in eligibility_machines(&typed_only.typed) {
            let position = typed_only
                .typed
                .machines()
                .iter()
                .position(|machine| machine.symbol == symbol)
                .expect("eligibility machine");
            typed_only.typed.machines_mut()[position]
                .termination_plan
                .checked_summary = language_semantics::TerminationGuarantee::Terminates {
                premises: Vec::new(),
            };
        }
        record_checked_termination(
            &mut typed_only,
            language_semantics::TerminationGuarantee::NoGuarantee,
        );
        assert!(
            validation::extract_non_executable_quotient_correspondences(&typed_only.typed).is_ok(),
            "the typed summaries alone would admit"
        );
        let error = retain_checked_quotient_correspondences(&typed_only, &mut module)
            .expect_err("checked facts without a guarantee refuse the batch");
        assert_unadmitted_naming(
            error,
            "purity, termination, or theorem crash eligibility is incomplete",
        );
        assert_eq!(module, before);

        // A guarantee carrying progress premises is not unconditional.
        let mut conditional = baseline_checked();
        conditional.typed = quotient_program(&source);
        record_checked_termination(
            &mut conditional,
            language_semantics::TerminationGuarantee::Terminates {
                premises: vec![language_semantics::ProgressPremise {
                    profile: language_semantics::SemanticDomainId::default(),
                    subject: language_semantics::ProgressSubject {
                        root: symbols::SymbolHandle::invalid(),
                        projections: Vec::new(),
                    },
                }],
            },
        );
        assert!(retain_checked_quotient_correspondences(&conditional, &mut module).is_err());
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

    /// The managed compiler-route program checked through the real route:
    /// `lower_typed_trees` admits the direct define after proving
    /// termination, so the checked trees carry the request as the compiler
    /// sees it.
    fn managed_checked_program() -> checked_trees::CheckedTrees {
        let typed = quotient_program(&format!(
            "{EQUIVALENCE_PRELUDE}{DIRECT_DEFINE_REQUEST}\ndata Main {{\n}}\n\nmachine Main::main(&mut self) {{\n}}\n"
        ));
        lower_typed_trees(typed, &CheckingRequest::settled())
            .expect("the checked route admits the managed direct define")
    }

    #[test]
    fn lowering_any_machine_of_an_admitted_program_stops_at_the_published_correspondence_gate() {
        let checked = managed_checked_program();
        assert!(checked.facts.termination.for_machine(
            checked
                .machines()
                .iter()
                .find(|machine| checked.symbols.name(machine.symbol) == "representative")
                .expect("representative")
                .symbol
        )
        .is_some_and(|plan| matches!(
            plan.checked_summary,
            language_semantics::TerminationGuarantee::Terminates { ref premises } if premises.is_empty()
        )));
        // The entry carries no request of its own; the correspondence rows
        // are program facts, so they join its module and the execution gate
        // refuses the nonempty table exactly as the published-correspondence
        // contract requires.
        let error = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
            .expect_err("a nonempty proof-only table cannot publish an executable module");
        assert_eq!(
            error,
            LoweringError::InvalidTerminalModule(
                terminal_verifier::ModuleError::NonExecutableQuotientCorrespondence
            )
        );
    }

    #[test]
    fn the_requesting_machine_itself_stays_unlowerable() {
        // The request owns no executable value plan: its owner machine has no
        // checked plan family, so lowering it fails closed before any
        // Terminal module exists, while the rows still reach every other
        // lowered machine of the program (see the test above).
        let checked = managed_checked_program();
        assert_eq!(
            lower_machine(&checked, TerminalMachineSelection::Name("admitted")).unwrap_err(),
            LoweringError::Unsupported(
                "machine has no source-independent checked scalar control plan"
            )
        );
    }
}
