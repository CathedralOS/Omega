//! Token-bearing machines reach operand-directed selection through their
//! operator-signature view, under the machine's own symbol.

use language_core::operator_spelling::OperatorSpelling;
use typed_trees::TypedTrees;
use typed_trees::operator::resolve_spelling_for_operands;
use typed_trees::types::TypeReferenceHandle;

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
