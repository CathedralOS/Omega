// The symbols crate owns these catalog APIs. This integration target uses
// its ordinary library, so these controls need no source-compiler unit binary.
use symbols::{BuiltinFunction, BuiltinTypeAtom};

/*
    An integer atom is its carrier. The root roster installs a slot for
    each conventional spelling, so those carriers have an ordinal and a
    symbol name; a carrier outside the roster is still an integer atom
    but has no slot to index.
*/
#[test]
fn integer_atoms_are_carriers_and_only_roster_spellings_have_slots() {
    use numerics::integer_carrier::FixedInteger;
    let mut installed = 0;
    for (ordinal, atom) in BuiltinTypeAtom::ALL.into_iter().enumerate() {
        assert_eq!(atom.roster_ordinal(), Some(ordinal));
        assert_eq!(BuiltinTypeAtom::from_ordinal(ordinal), Some(atom));
        let Some(carrier) = atom.fixed_integer() else {
            assert!(!atom.is_fixed_integer());
            continue;
        };
        installed += 1;
        assert!(atom.is_fixed_integer() && atom.is_canonical_const_scalar());
        assert_eq!(atom.symbol_name(), carrier.to_string());
        assert_eq!(atom.identity(), carrier.to_string());
        assert_eq!(
            FixedInteger::from_decimal_spelling(&atom.symbol_name()),
            Some(carrier)
        );
    }
    assert_eq!(installed, 8);
    assert_eq!(BuiltinTypeAtom::U64.symbol_name(), "u64");
    assert_eq!(BuiltinTypeAtom::U8.symbol_name(), "u8");

    let odd = BuiltinTypeAtom::Integer(FixedInteger::unsigned(13));
    assert!(odd.is_fixed_integer());
    assert_eq!(odd.roster_ordinal(), None);
    assert_eq!(odd.symbol_name(), "u13");
    assert_eq!(odd.identity(), "u13");
    assert_ne!(
        BuiltinTypeAtom::Integer(FixedInteger::signed(32)).roster_ordinal(),
        BuiltinTypeAtom::Integer(FixedInteger::unsigned(32)).roster_ordinal()
    );
    for atom in [
        BuiltinTypeAtom::Address,
        BuiltinTypeAtom::UInt,
        BuiltinTypeAtom::Int,
    ] {
        assert_eq!(atom.fixed_integer(), None);
    }
}

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
