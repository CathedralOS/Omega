use crate::execution::terminal_unit::types::ShapeCollector;
use typed_trees::TypedTrees;

/// Two modules that each declare `Token` and `Token::consume(self)`; only
/// the multiplicity of the two records differs.
fn same_name_tokens() -> TypedTrees {
    let module_source = |module: &str, multiplicity: &str| {
        format!(
            "module {module};
             pub data Token [{multiplicity}] {{ value: i32; }}
             pub machine Token::consume(self) -> i32 {{ self.value }}"
        )
    };
    crate::tests::front_end::typed_modules_with_core_service(&[
        ("left.omg", &module_source("left", "copy")),
        ("right.omg", &module_source("right", "linear")),
    ])
}

#[test]
fn an_owned_receiver_carries_its_own_records_multiplicity() {
    let program = same_name_tokens();
    for module in ["left", "right"] {
        let machine = program
            .machines()
            .iter()
            .find(|machine| {
                program
                    .symbols
                    .display_path(machine.attached_data_symbol, "::")
                    == format!("{module}::Token")
            })
            .unwrap_or_else(|| panic!("missing `{module}::Token::consume`"));
        let declared = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == machine.attached_data_symbol)
            .expect("the attached record")
            .properties
            .multiplicity;
        let state = &program.machine_states(machine)[0];
        let mut shapes = ShapeCollector::new(&program);
        let (_, structural, _) =
            super::structural_scalar_signature(&program, &mut shapes, machine, state, &[], false)
                .expect("the owned receiver's signature");
        let receiver = structural
            .iter()
            .find(|parameter| parameter.is_self)
            .expect("the owned receiver parameter");
        assert_eq!(
            receiver.multiplicity, declared,
            "{module}::Token::consume keeps its own record's multiplicity"
        );
    }
}

#[test]
fn namespace_qualification_requires_no_storage_but_boundary_ownership_does() {
    let program = crate::tests::front_end::typed_program(
        "data Score { value: u64; }
         machine Space::write(score: &mut Score) { score.value = 70; }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| program.symbols.display_path(machine.symbol, "::") == "Space::write")
        .expect("namespace-qualified machine");
    let state = &program.machine_states(machine)[0];
    let mut shapes = ShapeCollector::new(&program);
    let (attachment, structural, scalar) =
        super::structural_scalar_signature(&program, &mut shapes, machine, state, &[], false)
            .expect("explicit parameter owns the storage");
    assert!(attachment.is_none());
    assert!(scalar.is_empty());
    assert_eq!(structural.len(), 1);
    assert!(!structural[0].is_self);
    assert_eq!(
        structural[0].access,
        checked_trees::CheckedStructuralAccess::MutableBorrow
    );
    assert!(
        super::structural_scalar_signature_with_storage(
            &program,
            &mut shapes,
            machine,
            state,
            &[],
            false,
            super::AttachmentStorage::DataOwner,
            &super::LocalConstructionTrace::default(),
        )
        .is_none(),
        "a namespace cannot manufacture a boundary data owner"
    );
}
