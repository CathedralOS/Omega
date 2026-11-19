use symbols::{
    BUILTIN_TYPE_COUNT, BuiltinFunction, BuiltinTypeAtom, BuiltinTypeMember, FixedCoreDeclaration,
    SymbolKind, SymbolNameRef,
};
pub fn is_builtin_type_spelling(name: &str) -> bool {
    BuiltinTypeAtom::is_builtin_type_spelling(name)
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
    use super::{
        BuiltinFunction, BuiltinTypeAtom, FixedCoreDeclaration, SymbolKind, SymbolNameRef,
        builtin_function_symbols, builtin_type_symbols, fixed_core_declaration_symbols,
    };
    use symbols::BuiltinType;

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
