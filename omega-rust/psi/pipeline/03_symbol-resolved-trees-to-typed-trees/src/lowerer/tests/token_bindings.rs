//! Token-bearing machines reach operand-directed selection through their
//! operator-signature view, under the machine's own symbol.

use crate::typed_trees::TypedTrees;
use crate::typed_trees::operator::{declaration_by_symbol, resolve_spelling_for_operands};
use crate::typed_trees::types::TypeReferenceHandle;
use language_core::operator_spelling::OperatorSpelling;

const SOURCE: &str = "data Wrapped { value: u8; }
    pub machine + Wrapped::add(left: Wrapped, right: Wrapped) -> u64
    requires left.value < 128u8
    { (left.value as u64) + (right.value as u64) }
    machine Wrapped::length(left: Wrapped) -> u64 { left.value as u64 }
    machine - subtract(left: &Wrapped, right: u64) -> u64 { right }
    machine choose(left: Wrapped, right: Wrapped, scale: u64) -> u64 { left + right }";

fn entry_parameter_types(program: &TypedTrees, machine_name: &str) -> Vec<TypeReferenceHandle> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("machine by name");
    let entry = program
        .machine_states(machine)
        .first()
        .expect("entry state");
    program
        .state_parameters(entry)
        .iter()
        .map(|parameter| parameter.type_reference)
        .collect()
}

#[test]
fn token_bearing_machines_expose_one_signature_view_under_their_own_symbol() {
    let program = crate::front_end::typed_program_result(SOURCE).expect("lowering succeeds");
    let add = &program.machines()[0];
    let subtract = &program.machines()[2];
    assert_eq!(add.spelling, Some(OperatorSpelling::Add));
    assert!(
        program.operators().is_empty(),
        "no authored `operator` declaration"
    );

    let views = program.machine_token_bindings();
    assert_eq!(views.len(), 2, "one view per token-bearing machine");
    let view = &views[0];
    assert_eq!(view.symbol, add.symbol);
    assert!(view.is_public);
    assert!(!view.is_boundary);
    assert_eq!(view.spelling, Some(OperatorSpelling::Add));
    assert_eq!(
        program
            .operator_path_members(view.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["Wrapped", "add"]
    );
    let entry = program.machine_states(add).first().expect("entry state");
    assert_eq!(
        view.parameters, entry.parameters,
        "the entry telescope is shared"
    );
    assert_eq!(view.return_type, entry.return_type);
    assert_eq!(view.contracts, add.contracts, "head contracts are shared");
    assert_eq!(program.operator_contracts(view).len(), 1);
    assert_eq!(views[1].symbol, subtract.symbol);
    assert_eq!(
        program
            .operator_path_members(views[1].name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["subtract"]
    );
    assert_eq!(
        declaration_by_symbol(&program, add.symbol).map(|found| found.symbol),
        Some(add.symbol)
    );
    assert!(
        declaration_by_symbol(&program, program.machines()[1].symbol).is_none(),
        "a named machine has no operator view"
    );
}

#[test]
fn operand_directed_selection_finds_the_matching_binding_only() {
    let program = crate::front_end::typed_program_result(SOURCE).expect("lowering succeeds");
    let add = &program.machines()[0];
    let [wrapped, other_wrapped, scale] = entry_parameter_types(&program, "choose")[..] else {
        panic!("choose telescope");
    };

    let selected = resolve_spelling_for_operands(
        &program,
        OperatorSpelling::Add,
        &[Some(wrapped), Some(other_wrapped)],
        source::SourceSpan::default(),
    );
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].operator.symbol, add.symbol);
    assert!(selected[0].domain.is_none());

    assert!(
        resolve_spelling_for_operands(
            &program,
            OperatorSpelling::Add,
            &[Some(wrapped), Some(scale)],
            source::SourceSpan::default()
        )
        .is_empty(),
        "a foreign operand shape selects nothing"
    );
    assert!(
        resolve_spelling_for_operands(
            &program,
            OperatorSpelling::Subtract,
            &[Some(wrapped), Some(scale)],
            source::SourceSpan::default()
        )
        .is_empty(),
        "`&Wrapped` is not `Wrapped`: the reference shape is part of the binding"
    );
    assert!(
        resolve_spelling_for_operands(
            &program,
            OperatorSpelling::Multiply,
            &[Some(wrapped), Some(other_wrapped)],
            source::SourceSpan::default()
        )
        .is_empty(),
        "a token nobody bound selects nothing"
    );
}

#[test]
fn boundary_token_satisfaction_selects_each_exact_operand_overload() {
    let program = crate::front_end::typed_program_result(
        "boundary machine + Float::add(left: f32, right: f32) -> f32;
         boundary machine + Float::add(left: f64, right: f64) -> f64;
         boundary machine Provider::add32(left: f32, right: f32) -> f32
             satisfies Float::add { left }
         boundary machine Provider::add64(left: f64, right: f64) -> f64
             satisfies Float::add { left }",
    )
    .expect("typed overloads");
    let requirements = &program.machines()[..2];
    for (provider, requirement) in program.machines()[2..].iter().zip(requirements) {
        let [conformance] = program.declared_machine_trait_conformances(provider) else {
            panic!("one satisfaction edge");
        };
        assert_eq!(conformance.symbol, requirement.symbol);
        assert_eq!(conformance.requirement_symbol, requirement.symbol);
        assert!(matches!(
            crate::typed_trees::machine::resolve_satisfied_declaration(&program, provider, conformance),
            Some(crate::typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(selected))
                if selected.symbol == requirement.symbol
        ));
    }
}

#[test]
fn boundary_token_satisfaction_does_not_settle_a_mismatch() {
    let program = crate::front_end::typed_program_result(
        "boundary machine + Float::add(left: f32, right: f32) -> f32;
             boundary machine Provider::add64(left: f64, right: f64) -> f64
                 satisfies Float::add { left }",
    )
    .expect("formation precedes satisfaction validation");
    let provider = program.machines().last().expect("provider");
    let [conformance] = program.declared_machine_trait_conformances(provider) else {
        panic!("one satisfaction edge");
    };
    assert!(
        !conformance.symbol.is_valid(),
        "no arbitrary family representative"
    );
    assert!(!conformance.requirement_symbol.is_valid());
    assert!(
        crate::typed_trees::machine::resolve_satisfied_declaration(&program, provider, conformance)
            .is_none()
    );
}

#[test]
fn boundary_token_satisfaction_rejects_duplicate_requirement_shapes() {
    let syntax = crate::front_end::syntax_program_with_id(
        source::SourceId(0),
        "boundary machine + Float::add(left: f64, right: f64) -> f64;
         boundary machine + Float::add(left: f64, right: f64) -> f64;
         boundary machine Provider::add64(left: f64, right: f64) -> f64
             satisfies Float::add { left }",
    );
    let diagnostics = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect_err("duplicate token declarations reject before satisfaction settlement");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .to_string()
            .contains("same owner and operand shape")),
        "{diagnostics:?}"
    );
}

#[test]
fn boundary_token_satisfaction_retains_package_scope_and_foreign_ambiguity() {
    const REQUIREMENT: &str = "pub boundary machine + Float::add(left: f32, right: f32) -> f32;";
    const PROVIDER: &str = "boundary machine Provider::add32(left: f32, right: f32) -> f32
        satisfies Float::add { left }";
    for local_requirement in [true, false] {
        let provider_source = if local_requirement {
            format!("{REQUIREMENT} {PROVIDER}")
        } else {
            PROVIDER.to_owned()
        };
        let mut sources = source::SourceMap::default();
        let mut texts = Vec::new();
        for (package, text) in [
            ("first", REQUIREMENT),
            ("second", REQUIREMENT),
            ("provider", provider_source.as_str()),
        ] {
            let source_id = sources
                .add_with_metadata(
                    std::path::PathBuf::from(format!("{package}/main.omg")),
                    text.to_owned(),
                    std::path::PathBuf::from(package),
                    None,
                    source::SourceOrigin::User,
                )
                .source_id;
            texts.push((source_id, text));
        }
        let resolved = crate::front_end::resolved_program_from_merged_source_map(sources, &texts);
        let program = crate::lowerer::lower_symbol_resolved_trees(&resolved)
            .expect("type source-scoped satisfaction");
        let provider = program.machines().last().expect("provider");
        let [conformance] = program.declared_machine_trait_conformances(provider) else {
            panic!("one satisfaction edge");
        };
        assert_eq!(conformance.requirement_symbol.is_valid(), local_requirement);
        if local_requirement {
            assert_eq!(conformance.requirement_symbol, program.machines()[2].symbol);
            assert_eq!(conformance.symbol, program.machines()[2].symbol);
        }
    }
}

#[test]
fn boundary_token_satisfaction_preserves_generic_binder_and_bound_matching() {
    for (provider_parameter, accepted) in [("U", true), ("U [copy]", false)] {
        let program = crate::front_end::typed_program_result(&format!(
            "boundary machine + Sequence::append<T>(left: &[T], right: &[T]) -> &[T];
             boundary machine Provider::append<{provider_parameter}>(left: &[U], right: &[U]) -> &[U]
                 satisfies Sequence::append {{ left }}"
        )).expect("generic typed formation");
        let provider = &program.machines()[1];
        let [conformance] = program.declared_machine_trait_conformances(provider) else {
            panic!("one satisfaction edge");
        };
        assert_eq!(conformance.requirement_symbol.is_valid(), accepted);
    }
}
