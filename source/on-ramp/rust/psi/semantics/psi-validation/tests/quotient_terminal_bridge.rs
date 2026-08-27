use std::path::PathBuf;
use std::sync::Arc;

use psi_core::PackageKeyIdentity;
use psi_source::{SourceMap, SourceOrigin};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources;
use psi_tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};
use psi_typed_trees::TypedTrees;
use psi_validation::{extract_non_executable_quotient_correspondences, validate_program};

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

fn lower(source: &str) -> TypedTrees {
    let package = PackageKeyIdentity::from_digest([0x71; 32]).expect("nonzero package identity");
    lower_with_package(source, Some(package))
}

fn lower_with_package(source: &str, package: Option<PackageKeyIdentity>) -> TypedTrees {
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
            package,
            SourceOrigin::User,
        )
        .source_id;
    let core_tokens = Lexer::new(CORE_RELATION).tokenize().expect("tokenize core");
    let mut syntax =
        parse_syntax_trees_with_id(core_source_id, &core_tokens).expect("parse core relation");
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens).expect("parse fixture");
    let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("package-aware resolution");
    let mut program = lower_symbol_resolved_trees(&resolved).expect("type lowering");
    let checked_total = program
        .machines()
        .iter()
        .enumerate()
        .filter_map(|(position, machine)| {
            matches!(
                program.symbols.name(machine.symbol),
                "representative" | "representative_respects" | "admitted"
            )
            .then_some(position)
        })
        .collect::<Vec<_>>();
    assert_eq!(checked_total.len(), 3);
    for position in checked_total {
        program.machines_mut()[position]
            .termination_plan
            .checked_summary = psi_language_semantics::TerminationGuarantee::Terminates {
            premises: Vec::new(),
        };
    }
    program
}

#[test]
fn extracts_one_source_free_total_direct_define_without_weakening_normal_validation() {
    let program = lower(TOTAL_DIRECT_DEFINE);

    let rows = extract_non_executable_quotient_correspondences(&program)
        .expect("the narrow total direct define should extract");
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert!(row.public_operation.declaration.starts_with("package:"));
    assert!(
        row.representative
            .callable
            .declaration
            .starts_with("package:")
    );
    assert!(
        row.selected_theorem
            .callable
            .declaration
            .starts_with("package:")
    );
    assert_eq!(row.runtime_positions.len(), 1);
    assert_eq!(row.input_relations.len(), 1);
    assert_eq!(row.theorem.parameters.len(), 2);
    assert_eq!(row.theorem.relation_premises.len(), 1);
    assert!(row.theorem.legality_premises.is_empty());

    let diagnostics = validate_program(&program)
        .expect_err("ordinary validation must retain the executable quotient-operation fence");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("executable quotient operations are not admitted")
    }));
}

#[test]
fn rejects_unchecked_eligibility_and_result_aliases() {
    let mut program = lower(TOTAL_DIRECT_DEFINE);
    let representative = program
        .machines()
        .iter()
        .position(|machine| program.symbols.name(machine.symbol) == "representative")
        .expect("representative machine");
    program.machines_mut()[representative]
        .termination_plan
        .checked_summary = psi_language_semantics::TerminationGuarantee::NoGuarantee;
    assert!(extract_non_executable_quotient_correspondences(&program).is_err());

    let aliased = TOTAL_DIRECT_DEFINE.replace(
        "    Quotient::define<representative, representative_respects>(value)\n",
        "    let result: EquivalenceClass = Quotient::define<representative, representative_respects>(value);\n    result\n",
    );
    assert!(extract_non_executable_quotient_correspondences(&lower(&aliased)).is_err());
}

#[test]
fn rejects_nonhermetic_authored_identity() {
    let program = lower_with_package(TOTAL_DIRECT_DEFINE, None);
    assert!(extract_non_executable_quotient_correspondences(&program).is_err());
}

#[test]
fn fails_the_whole_batch_when_one_request_is_unsupported() {
    let mixed = format!(
        "{TOTAL_DIRECT_DEFINE}\n\nmachine unsupported(value: EquivalenceClass) -> EquivalenceClass {{\n    Quotient::lift<representative, representative_respects>(value)\n}}\n"
    );
    let program = lower(&mixed);
    assert!(
        extract_non_executable_quotient_correspondences(&program).is_err(),
        "one unsupported request must prevent returning the otherwise valid define row"
    );
}
