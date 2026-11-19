use symbols::BuiltinTypeAtom;

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
