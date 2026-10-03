//! Context provisioning follows an explicit source parameter, never a qualifier.

use super::{SelectedProgramEntry, validate_selected_program_entry_shape};
use program_entry_plan::ProgramEntrySourceReceiverSignature;
use typed_trees::TypedTrees;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
    )
    .unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn selected(program: &TypedTrees) -> SelectedProgramEntry<'_> {
    let [machine] = program.machines() else {
        panic!("fixture must declare exactly one machine");
    };
    SelectedProgramEntry {
        machine_name: machine.name.as_str(),
        machine_symbol: machine.symbol,
        slot: target::TargetProfile::WindowsX64.program_entry_slot(),
    }
}

#[test]
fn hosted_entry_selects_explicit_context_independently_of_machine_namespace() {
    for declaration in [
        "machine run",
        "machine Context::run",
        "machine Namespace::run",
    ] {
        let program = typed(&format!(
            "data Context {{ value: i32; }}
             data Namespace where unrelated > 0 {{ unrelated: i32; }}
             {declaration}(context: &mut Context) {{ context.value = 7; }}"
        ));
        let selected = selected(&program);
        let source = validate_selected_program_entry_shape(&program, selected).unwrap();
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == selected.machine_symbol)
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let parameter = &program.state_parameters(state)[0];
        assert_eq!(
            source.receiver(),
            &ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                source_position: 0,
                source_symbol: parameter.symbol,
                normalized_type_identity: program
                    .normalized_type_identity(parameter.type_reference)
                    .into_string(),
            }
        );
        assert!(source.visible_parameters().is_empty());
    }
}

#[test]
fn qualification_without_context_requests_no_provisioning() {
    let program = typed("data Namespace { value: i32; } machine Namespace::run() {}");
    let source = validate_selected_program_entry_shape(&program, selected(&program)).unwrap();
    assert_eq!(
        source.receiver(),
        &ProgramEntrySourceReceiverSignature::Free
    );
}

#[test]
fn hosted_context_rejects_shared_ambiguous_and_nonzero_storage() {
    for (source, diagnostic) in [
        (
            "data Context { value: i32; } machine run(context: &Context) {}",
            "exclusive mutable borrow",
        ),
        (
            "data Context { value: i32; } machine run(left: &mut Context, right: &mut Context) {}",
            "multiple context candidates",
        ),
        (
            "data Context where value > 0 { value: i32; } machine run(context: &mut Context) {}",
            "all-zero image is not a valid value",
        ),
        (
            "data Context { value: i32; } machine run(context: &mut Context, unexpected: u32) {}",
            "exposes no arrival parameters",
        ),
    ] {
        let program = typed(source);
        let errors =
            validate_selected_program_entry_shape(&program, selected(&program)).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.to_string().contains(diagnostic)),
            "{errors:?}"
        );
    }
}

#[test]
fn hosted_context_judges_float_zero_in_the_declared_format_before_checking() {
    for (field_type, predicate, admitted) in [
        ("f32", "value >= 0.0 && value <= 1.0", true),
        ("f32", "value >= 1.0e-50", true),
        ("f64", "value >= 1.0e-50", false),
        ("f32", "value > 0.0", false),
    ] {
        // Product entry selection precedes checked-tree construction. Its
        // typed input still has conservative gates and unlanded literals;
        // source compatibility must use the same exact Psi zero judgment.
        let program = typed(&format!(
            "data Context where {predicate} {{ value: {field_type}; }}
             machine run(context: &mut Context) {{}}"
        ));
        let result = validate_selected_program_entry_shape(&program, selected(&program));
        assert_eq!(
            result.is_ok(),
            admitted,
            "{field_type}: {predicate}: {result:?}"
        );
    }
}

#[test]
fn context_source_position_is_not_a_dense_structural_ordinal() {
    let program = typed(
        "data Context { value: i32; }
        machine run(arrival: u32, context: &mut Context) {}",
    );
    let selected = selected(&program);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == selected.machine_symbol)
        .unwrap();
    let entry = &program.machine_states(machine)[0];
    let context = super::provisioned_entry_context(&program, machine, entry)
        .unwrap()
        .unwrap();
    assert_eq!(context.source_position, 1);
    assert_eq!(
        context.parameter.symbol,
        program.state_parameters(entry)[1].symbol
    );
}
