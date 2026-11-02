//! Declaration values do not borrow another state's storage frontier.

use super::*;
use symbols::{SymbolNameRef, SymbolTableBuilder};
use typed_trees::{TypedTrees, constant::ConstDeclaration, name::Identifier};

#[test]
fn declaration_constants_keep_exact_scope_without_admitting_foreign_storage() {
    for copies in [0, 1, 2] {
        let mut builder = SymbolTableBuilder::default();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let roots = builder.insert_children(
            root,
            symbols::builtin_type_symbols().into_iter().chain([
                (SymbolKind::Const, SymbolNameRef::Static("ZERO")),
                (SymbolKind::Machine, SymbolNameRef::Static("main")),
                (SymbolKind::Machine, SymbolNameRef::Static("other")),
            ]),
        );
        let constant = roots.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
        let machine_symbol = roots.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 1);
        let other = roots.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 2);
        let state_symbol = builder
            .insert_children(
                machine_symbol,
                [(SymbolKind::State, SymbolNameRef::Static("entry"))],
            )
            .start();
        let foreign_state = builder
            .insert_children(other, [(SymbolKind::State, SymbolNameRef::Static("entry"))])
            .start();
        let foreign = builder.insert_children(
            foreign_state,
            [
                (SymbolKind::Local, SymbolNameRef::Static("ZERO")),
                (SymbolKind::Parameter, SymbolNameRef::Static("ZERO")),
            ],
        );
        let mut program = TypedTrees::default();
        program.symbols = builder.finish();
        for _ in 0..copies {
            program.push_const_declaration(ConstDeclaration {
                symbol: constant,
                pending_target_observation: true,
                ..Default::default()
            });
        }
        let machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated("main"),
            ..Default::default()
        };
        let state = State {
            symbol: state_symbol,
            name: Identifier::generated("entry"),
            ..Default::default()
        };
        let symbols = TopLevelSymbols::build(&program, &mut Vec::new());
        let known = |root| {
            state_value_root_is_known(&program, &machine, &state, &[], &symbols, root, "ZERO")
        };
        assert_eq!(known(constant), copies == 1);
        assert!(!known(foreign.handle_at(0)));
        assert!(!known(foreign.handle_at(1)));
    }
}
