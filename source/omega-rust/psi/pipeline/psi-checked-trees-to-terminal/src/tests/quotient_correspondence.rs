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
                "representative" | "representative_respects"
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
    let mut expected = psi_validation::extract_non_executable_quotient_correspondences(&program)
        .expect("extract complete direct define batch")
        .into_iter()
        .map(psi_terminal::retain_non_executable_quotient_correspondence)
        .collect::<Vec<_>>();
    expected.sort_by(|left, right| left.identity.cmp(&right.identity));
    assert_eq!(expected.len(), 2);
    assert_ne!(expected[0].identity, expected[1].identity);

    let baseline = baseline_module();
    let mut module = baseline.clone();
    module.quotient_correspondences = vec![expected[0].clone()];

    install_non_executable_quotient_correspondences(&program, &mut module)
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
    install_non_executable_quotient_correspondences(&valid, &mut module)
        .expect("install initial proof-only batch");
    let before = module.clone();
    let mixed = format!(
        "{TOTAL_DIRECT_DEFINE}\n\nmachine unsupported(value: EquivalenceClass) -> EquivalenceClass {{\n    Quotient::lift<representative, representative_respects>(value)\n}}\n"
    );

    let error =
        install_non_executable_quotient_correspondences(&quotient_program(&mixed), &mut module)
            .expect_err("one unsupported request rejects the whole replacement batch");

    let LoweringError::InvalidQuotientCorrespondence(diagnostics) = error else {
        panic!("unexpected installer error: {error:?}")
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("faithful `define` only"))
    );
    assert_eq!(module, before);
}

#[test]
fn replay_failure_leaves_the_module_unchanged() {
    let program = quotient_program(TOTAL_DIRECT_DEFINE);
    let mut module = baseline_module();
    module.entry = psi_core::MachineId::new(99).expect("nonzero invalid entry");
    let before = module.clone();

    let error = install_non_executable_quotient_correspondences(&program, &mut module)
        .expect_err("the producer must replay the candidate module before committing rows");

    assert!(matches!(
        error,
        LoweringError::InvalidTerminalModule(
            psi_terminal_verifier::ModuleError::UnknownEntryMachine(_)
        )
    ));
    assert_eq!(module, before);
}
