use language_core::BuiltinFunction;

#[test]
fn builtin_function_names_round_trip_through_from_name() {
    for (function, spelling) in BuiltinFunction::ALL {
        assert_eq!(
            BuiltinFunction::from_name(spelling),
            Some(function),
            "{:?} must be recoverable from its own spelling",
            function,
        );
        assert_eq!(
            function.name(),
            spelling,
            "{:?} must read the spelling paired with it",
            function,
        );
    }
    assert_eq!(BuiltinFunction::from_name(""), None);
    assert_eq!(BuiltinFunction::from_name("Min"), None);
    for name in ["min", "max", "abs", "clamp", "sqrt"] {
        assert_eq!(BuiltinFunction::from_name(name), None);
    }
}

#[test]
fn value_position_asm_intrinsics_are_exactly_port_in_pushfq_rdmsr() {
    let value_position: Vec<BuiltinFunction> = BuiltinFunction::ALL
        .into_iter()
        .map(|(function, _)| function)
        .filter(|function| function.is_value_position_asm_intrinsic())
        .collect();
    assert_eq!(
        value_position,
        [
            BuiltinFunction::AsmPortIn,
            BuiltinFunction::AsmSnapshotFlags,
            BuiltinFunction::AsmReadMsr
        ]
    );
    assert_eq!(
        value_position
            .iter()
            .map(|function| function.name())
            .collect::<Vec<_>>(),
        ["asm#port_in", "asm#pushfq", "asm#rdmsr"]
    );
    assert!(
        value_position
            .iter()
            .all(|function| function.is_asm_intrinsic())
    );
}

#[test]
fn asm_intrinsics_are_unnameable_and_contract_bearing() {
    let roster: Vec<BuiltinFunction> = BuiltinFunction::ALL
        .into_iter()
        .filter(|(_, spelling)| spelling.starts_with("asm#"))
        .map(|(function, _)| function)
        .collect();
    assert_eq!(
        roster.len(),
        52,
        "the unnameable instruction roster is closed"
    );
    assert!(roster.contains(&BuiltinFunction::AsmStoreGdtr));
    assert!(roster.contains(&BuiltinFunction::AsmLoadGdtr));
    for function in roster {
        assert!(
            function.name().contains('#'),
            "{:?} must be unnameable from source",
            function
        );
        assert!(
            function.is_asm_intrinsic(),
            "{:?} must carry an instruction contract",
            function
        );
    }
}
