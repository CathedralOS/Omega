use symbols::{BuiltinTypeAtom, BuiltinFunction, FixedCoreDeclaration, BuiltinTypeMember, SymbolKind, SymbolNameRef, BUILTIN_TYPE_COUNT};
pub fn is_builtin_type_spelling(name: &str) -> bool {
    let integer_family = name
        .strip_prefix(['i', 'u'])
        .is_some_and(|digits| digits.bytes().all(|byte| byte.is_ascii_digit()));
    integer_family
        || BuiltinTypeAtom::SPELLINGS
            .iter()
            .any(|(_, _, symbol_name)| *symbol_name == name)
}
pub fn builtin_type_symbols() -> [(SymbolKind, SymbolNameRef<'static>); BUILTIN_TYPE_COUNT] {
    BuiltinTypeAtom::SPELLINGS
        .map(|(_, _, symbol_name)| (SymbolKind::BuiltinType, SymbolNameRef::Static(symbol_name)))
}
pub fn builtin_function_symbols() -> [(SymbolKind, SymbolNameRef<'static>); BuiltinFunction::COUNT]
{
    BuiltinFunction::ALL
        .map(|(_, spelling)| (SymbolKind::BuiltinFunction, SymbolNameRef::Static(spelling)))
}
pub fn fixed_core_declaration_symbols()
-> [(SymbolKind, SymbolNameRef<'static>); FixedCoreDeclaration::COUNT] {
    FixedCoreDeclaration::ALL.map(|declaration| {
        (
            SymbolKind::MathematicalDefinition,
            SymbolNameRef::Static(declaration.name()),
        )
    })
}
pub fn builtin_type_member_symbols(
    builtin_type: BuiltinTypeAtom,
) -> impl Iterator<Item = (SymbolKind, SymbolNameRef<'static>)> {
    (builtin_type == BuiltinTypeMember::AddressBound.owner())
        .then_some((
            SymbolKind::MathematicalDefinition,
            SymbolNameRef::Static("Bound"),
        ))
        .into_iter()
}
#[cfg(test)]
mod builtin_ordinal_tests {
    use symbols::BuiltinType;
    use super::{
        BuiltinFunction, BuiltinTypeAtom, FixedCoreDeclaration, SymbolKind,
        SymbolNameRef, builtin_function_symbols, builtin_type_symbols,
        fixed_core_declaration_symbols,
    };

    #[test]
    fn builtin_type_ordinals_track_the_symbol_table() {
        let table = builtin_type_symbols();
        for builtin_type in [BuiltinType::UInt, BuiltinType::Int] {
            assert_eq!(
                table[builtin_type.ordinal()].1.as_str(),
                builtin_type.name(),
                "ordinal for {:?} does not match the symbol table position",
                builtin_type
            );
        }
        for atom in BuiltinTypeAtom::ALL {
            assert_eq!(
                (table[atom.ordinal()].0, table[atom.ordinal()].1.as_str()),
                (SymbolKind::BuiltinType, atom.symbol_name().as_ref()),
                "closed builtin atom {:?} must retain its exact root slot",
                atom,
            );
        }
    }

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
    fn builtin_function_ordinals_track_the_symbol_table() {
        let table = builtin_function_symbols();
        for (ordinal, (function, spelling)) in BuiltinFunction::ALL.into_iter().enumerate() {
            assert_eq!(
                function.ordinal(),
                ordinal,
                "closed function inventory must retain the canonical ordinal for {:?}",
                function,
            );
            assert_eq!(
                table[function.ordinal()].1.as_str(),
                spelling,
                "ordinal for {:?} does not match the symbol table position",
                function
            );
            assert_eq!(
                BuiltinFunction::from_ordinal(function.ordinal()),
                Some(function),
                "ordinal for {:?} must recover its closed identity",
                function,
            );
        }
        for (function, ordinal) in [
            (BuiltinFunction::FloatMaximum, 0),
            (BuiltinFunction::AsmHlt, 3),
            (BuiltinFunction::ContentOld, 33),
            (BuiltinFunction::IntegerEmbed, 71),
            (BuiltinFunction::AsmWriteTpidrEl1, 99),
            (BuiltinFunction::AsmAarch64LoadFence, 100),
            (BuiltinFunction::AsmAarch64FullFence, 102),
        ] {
            assert_eq!(
                function.ordinal(),
                ordinal,
                "{function:?} must keep its published slot number"
            );
        }
        assert_eq!(BuiltinFunction::from_ordinal(BuiltinFunction::COUNT), None);
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
    fn content_old_owns_the_stable_callable_entry_builtin_slot() {
        let table = builtin_function_symbols();
        assert_eq!(BuiltinFunction::ContentOld.ordinal(), 33);
        assert_eq!(BuiltinFunction::ContentOld.name(), "old");
        assert_eq!(table[33].1.as_str(), "old");
        assert!(
            table.iter().all(
                |(kind, name)| *kind != SymbolKind::BuiltinFunction || name.as_str() != "entry"
            ),
            "the retired ContentEntry builtin must not survive under its source spelling"
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
        assert_eq!(
            BuiltinFunction::AsmHlt.asm_intrinsic_service_name(),
            Some("MachineControl")
        );
        assert_eq!(
            BuiltinFunction::AsmPortOut.asm_intrinsic_service_name(),
            Some("PortIo")
        );
        assert_eq!(
            BuiltinFunction::AsmPortIn.asm_intrinsic_service_name(),
            Some("PortIo")
        );
        assert_eq!(
            BuiltinFunction::AsmFullFence.asm_intrinsic_service_name(),
            None
        );
        assert_eq!(
            BuiltinFunction::AsmDisableInterrupts.asm_intrinsic_service_name(),
            Some("MachineControl")
        );
        for (function, _) in BuiltinFunction::ALL {
            assert!(
                function.asm_intrinsic_service_name().is_none() || function.is_asm_intrinsic(),
                "{:?} reaches a boundary service but is missing from the asm roster",
                function,
            );
        }
    }

    #[test]
    fn fixed_core_declaration_ordinals_track_the_symbol_table() {
        let table = fixed_core_declaration_symbols();
        for (ordinal, declaration) in FixedCoreDeclaration::ALL.into_iter().enumerate() {
            assert_eq!(
                declaration.ordinal(),
                ordinal,
                "closed fixed-core inventory must retain the canonical ordinal for {:?}",
                declaration,
            );
            assert_eq!(
                table[declaration.ordinal()],
                (
                    SymbolKind::MathematicalDefinition,
                    SymbolNameRef::Static(declaration.name()),
                ),
                "fixed-core declaration {:?} must retain its exact root slot",
                declaration,
            );
            assert_eq!(
                FixedCoreDeclaration::from_ordinal(declaration.ordinal()),
                Some(declaration),
                "ordinal for {:?} must recover its closed identity",
                declaration,
            );
            assert_eq!(
                FixedCoreDeclaration::from_name(declaration.name()),
                Some(declaration),
                "{:?} must be recoverable from its own spelling",
                declaration,
            );
        }
        assert_eq!(
            FixedCoreDeclaration::from_ordinal(FixedCoreDeclaration::COUNT),
            None
        );
        assert_eq!(FixedCoreDeclaration::from_name("core::Vec"), None);
    }

    /* The integer family is open-ended; a value-like spelling is not a type name. */
    #[test]
    fn builtin_type_spellings_cover_the_roster_and_the_integer_family() {
        for name in ["bool", "addr", "f32", "u32", "i", "u", "u0", "i7", "u257"] {
            assert!(super::is_builtin_type_spelling(name), "{name}");
        }
        for name in ["count", "ui", "u8x", "i_1", "uint", "U32", "Never", ""] {
            assert!(!super::is_builtin_type_spelling(name), "{name}");
        }
    }
}
