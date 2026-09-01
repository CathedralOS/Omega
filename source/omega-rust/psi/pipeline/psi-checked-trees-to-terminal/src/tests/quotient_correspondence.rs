use std::path::PathBuf;
use std::sync::Arc;

use psi_core::PackageKeyIdentity;
use psi_source::{SourceMap, SourceOrigin};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources;
use psi_tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};
use psi_typed_trees::TypedTrees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

use super::super::{LoweringError, install_non_executable_quotient_correspondences, lower_machine};

const TOTAL_DIRECT_DEFINE: &str = r#"
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

machine admitted(value: EquivalenceClass) -> EquivalenceClass {
    Quotient::define<representative, representative_respects>(value)
}
"#;

const TRANSPORT_BACKED_LIFT: &str = r#"
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
machine equivalent_transitive(a: Representative, b: Representative, c: Representative)
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
where equivalent satisfies Equivalence<Representative, equivalent>
as RepresentativeEquivalence;

machine representative(value: Representative) -> Representative
requires value == value
{
    value
}

machine representative_respects(left: Representative, right: Representative)
requires
    equivalent(left, right)
    left == left
    right == right
ensures equivalent(representative(left), representative(right))
{
}

machine representative_transports(left: Representative, right: Representative)
requires
    left == left
    right == right
ensures
    left == left
    right == right
{
}

machine admitted(value: EquivalenceClass) -> EquivalenceClass
requires value == value
{
    Quotient::lift<
        representative,
        representative_respects,
        representative_transports
    >(value)
}
"#;

const CORE_RELATION: &str = include_str!("../../../../../../../source/library/core/relation.omg");

fn quotient_program(source: &str) -> TypedTrees {
    let package = PackageKeyIdentity::from_digest([0x72; 32]).expect("nonzero package identity");
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
    let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("package-aware resolution");
    let mut program = lower_symbol_resolved_trees(&resolved).expect("type lowering");
    let eligible = program
        .machines()
        .iter()
        .enumerate()
        .filter_map(|(position, machine)| {
            matches!(
                program.symbols.name(machine.symbol),
                "representative" | "representative_respects" | "representative_transports"
            )
            .then_some(position)
            .or_else(|| {
                program
                    .symbols
                    .name(machine.symbol)
                    .starts_with("admitted")
                    .then_some(position)
            })
        })
        .collect::<Vec<_>>();
    assert!(eligible.len() >= 3);
    for position in eligible {
        program.machines_mut()[position]
            .termination_plan
            .checked_summary = psi_language_semantics::TerminationGuarantee::Terminates {
            premises: Vec::new(),
        };
    }
    program
}

#[test]
fn retains_and_replays_source_checked_transport_backed_lift_without_execution() {
    let program = quotient_program(TRANSPORT_BACKED_LIFT);
    let batch = psi_validation::extract_non_executable_quotient_correspondences(&program)
        .expect("extract transport-backed lift");
    let mut module = baseline_module();
    install_non_executable_quotient_correspondences(batch, &mut module)
        .expect("install transport-backed proof row");
    let [retained] = module.quotient_correspondences.as_slice() else {
        panic!("one retained transport row")
    };
    assert_eq!(
        retained.certificate.operation_kind,
        psi_language_semantics::quotient_correspondence::QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport
    );
    let psi_language_semantics::quotient_correspondence::QuotientTheoremCorrespondence::ForwardPreconditionTransport(transport) =
        &retained.certificate.theorem_evidence[1].correspondence
    else {
        panic!("transport payload")
    };
    use psi_language_semantics::quotient_correspondence::{
        QuotientContractFactCoordinate, QuotientContractOwner,
        QuotientForwardPreconditionTransportFact, QuotientTheoremApplicationSide,
        QuotientTheoremCorrespondence,
    };
    let coordinate = |contract_position, fact_position| QuotientContractFactCoordinate {
        owner: QuotientContractOwner::Machine,
        contract_position,
        fact_position,
    };
    let pair = |source, left_actual, right_actual| {
        vec![
            QuotientForwardPreconditionTransportFact {
                application: QuotientTheoremApplicationSide::Left,
                source,
                actual: left_actual,
            },
            QuotientForwardPreconditionTransportFact {
                application: QuotientTheoremApplicationSide::Right,
                source,
                actual: right_actual,
            },
        ]
    };
    let public_source = coordinate(0, 0);
    let representative_source = coordinate(0, 0);
    assert_eq!(
        transport.public_premises,
        pair(public_source, coordinate(0, 0), coordinate(0, 1))
    );
    assert_eq!(
        transport.representative_conclusions,
        pair(representative_source, coordinate(1, 0), coordinate(1, 1))
    );
    let QuotientTheoremCorrespondence::Congruence(congruence) =
        &retained.certificate.theorem_evidence[0].correspondence
    else {
        panic!("congruence payload")
    };
    assert_eq!(
        congruence.legality_premises,
        pair(representative_source, coordinate(0, 1), coordinate(0, 2))
    );
    assert_eq!(
        psi_terminal_verifier::validate_module_representation(&module),
        Ok(())
    );
    assert_eq!(
        psi_terminal_verifier::validate_module(&module).unwrap_err(),
        psi_terminal_verifier::ModuleError::NonExecutableQuotientCorrespondence
    );
    let bytes = psi_terminal_codec::encode_module(&module).expect("encode transport row");
    assert_eq!(psi_terminal_codec::decode_module(&bytes), Ok(module));
}

fn baseline_module() -> psi_terminal::TerminalModule {
    let source = r#"
        machine baseline(value: i32) -> i32
        requires 0i32 == 0i32
        ensures 0i32 == 0i32
        {
            value
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize baseline");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse baseline");
    let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve baseline");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type baseline");
    let checked = lower_typed_trees(typed).expect("check baseline");
    lower_machine(&checked, "baseline")
        .expect("lower baseline")
        .semantic_module
}

#[test]
fn installs_complete_direct_define_batch_without_executable_authority() {
    let two_defines = format!(
        "{TOTAL_DIRECT_DEFINE}\n\nmachine admitted_second(value: EquivalenceClass) -> EquivalenceClass {{\n    Quotient::define<representative, representative_respects>(value)\n}}\n"
    );
    let program = quotient_program(&two_defines);
    assert!(psi_validation::validate_program(&program).is_err());
    let batch = psi_validation::extract_non_executable_quotient_correspondences(&program)
        .expect("extract complete direct define batch");
    let mut expected = batch
        .clone()
        .into_correspondences()
        .into_iter()
        .map(psi_terminal::retain_non_executable_quotient_correspondence)
        .collect::<Vec<_>>();
    expected.sort_by(|left, right| left.identity.cmp(&right.identity));
    assert_eq!(expected.len(), 2);
    assert_ne!(expected[0].identity, expected[1].identity);

    let baseline = baseline_module();
    let mut module = baseline.clone();
    module.quotient_correspondences = vec![expected[0].clone()];

    install_non_executable_quotient_correspondences(batch, &mut module)
        .expect("install proof-only direct define batch");

    assert_eq!(module.quotient_correspondences, expected);
    let mut without_rows = module.clone();
    without_rows.quotient_correspondences.clear();
    assert_eq!(without_rows, baseline);
    psi_terminal_verifier::validate_module_representation(&module)
        .expect("representation replay accepts the source-derived row");
    assert_eq!(
        psi_terminal_verifier::validate_module(&module).unwrap_err(),
        psi_terminal_verifier::ModuleError::NonExecutableQuotientCorrespondence
    );
    let bytes = psi_terminal_codec::encode_module(&module).expect("encode retained row");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes),
        Ok(module.clone())
    );
}

#[test]
fn unsupported_request_leaves_the_installed_batch_unchanged() {
    let valid = quotient_program(TOTAL_DIRECT_DEFINE);
    let mut module = baseline_module();
    let valid_batch = psi_validation::extract_non_executable_quotient_correspondences(&valid)
        .expect("extract initial proof-only batch");
    install_non_executable_quotient_correspondences(valid_batch, &mut module)
        .expect("install initial proof-only batch");
    let before = module.clone();
    let mixed = format!(
        "{TOTAL_DIRECT_DEFINE}\n\nmachine unsupported(value: EquivalenceClass) -> EquivalenceClass {{\n    Quotient::lift<representative, representative_respects>(value)\n}}\n"
    );

    let diagnostics =
        psi_validation::extract_non_executable_quotient_correspondences(&quotient_program(&mixed))
            .expect_err("one unsupported request rejects the whole replacement batch");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("direct transport-backed `lift` only")
    }));
    assert_eq!(module, before);
}

#[test]
fn replay_failure_leaves_the_module_unchanged() {
    let program = quotient_program(TOTAL_DIRECT_DEFINE);
    let batch = psi_validation::extract_non_executable_quotient_correspondences(&program)
        .expect("extract proof-only batch");
    let mut module = baseline_module();
    module.entry = psi_core::MachineId::new(99).expect("nonzero invalid entry");
    let before = module.clone();

    let error = install_non_executable_quotient_correspondences(batch, &mut module)
        .expect_err("the producer must replay the candidate module before committing rows");

    assert!(matches!(
        error,
        LoweringError::InvalidTerminalModule(
            psi_terminal_verifier::ModuleError::UnknownEntryMachine(_)
        )
    ));
    assert_eq!(module, before);
}
