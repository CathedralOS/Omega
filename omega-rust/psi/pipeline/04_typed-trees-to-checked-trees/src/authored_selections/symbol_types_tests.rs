use crate::authored_selections::operator_targets::type_reference_for_symbol;
use crate::tests::front_end::typed_program;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::{TypedTrees, data::DataMember, statement::StatementNode};

fn fixture() -> TypedTrees {
    let source = r#"
        data First { value: u16; }
        data Second { value: u32; }
        data Choice { case First(value: u16); case Second(value: u32); }
        trait Reader { machine read(value: u64) -> u64 terminates; }
        proposition related(left: First, right: Second);
        machine First::read(&self) { let observed: u16 = value; }
        machine second(value: u32) { let observed: u32 = value; }
    "#;
    typed_program(source)
}

#[test]
fn symbol_types_follow_exact_declaration_owners() {
    let program = fixture();
    let mut checked = 0;
    let mut assert_type = |symbol, expected| {
        assert_eq!(
            type_reference_for_symbol(&program, symbol),
            Some(expected),
            "{}: {:?}",
            program.symbols.name(symbol),
            program.symbols.get(symbol)
        );
        checked += 1;
    };
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            match member {
                DataMember::Field(field) => assert_type(field.symbol, field.type_reference),
                DataMember::Variant(variant) => {
                    for field in program.data_payload_fields(variant) {
                        assert_type(field.symbol, field.type_reference);
                    }
                }
            }
        }
    }
    for machine in program.machines() {
        if machine.attached_data_symbol.is_valid() {
            let inherited = program
                .symbols
                .find_child_by_name_and_kind(machine.symbol, "value", SymbolKind::Field)
                .expect("inherited field slot");
            let field = validation::exact_attached_field(&program, machine, inherited, "value")
                .expect("exact inherited field");
            assert_ne!(inherited, field.symbol);
            assert_type(inherited, field.type_reference);
        }
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                assert_type(parameter.symbol, parameter.type_reference);
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::LocalData(local) = statement {
                    assert_type(local.symbol, local.type_reference);
                }
            }
        }
    }
    for definition in program.traits() {
        for signature in program.trait_machine_signatures(definition) {
            for parameter in program.state_signature_parameters(signature) {
                assert_type(parameter.symbol, parameter.type_reference);
            }
        }
    }
    for proposition in program.propositions() {
        for parameter in program.proposition_parameters(proposition) {
            assert_type(parameter.symbol, parameter.type_reference);
        }
    }
    assert!(checked >= 10);
}

#[test]
fn symbol_types_do_not_recover_stale_or_provenance_only_symbols() {
    let mut program = fixture();
    let DataMember::Field(field) = &program.data_members(&program.data_definitions()[0])[0] else {
        panic!("field")
    };
    let symbol = field.symbol;
    for missing in [
        SymbolHandle::invalid(),
        SymbolHandle::from_parts(symbol.arena_index(), symbol.generation() + 1),
        SymbolHandle::from_arena_index(u32::MAX),
    ] {
        assert_eq!(type_reference_for_symbol(&program, missing), None);
    }
    let generated =
        program
            .symbols
            .insert_generated_root_from(symbol, SymbolKind::Field, "generated");
    assert_eq!(type_reference_for_symbol(&program, generated), None);
}
