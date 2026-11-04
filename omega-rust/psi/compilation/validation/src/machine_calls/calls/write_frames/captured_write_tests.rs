use super::*;
use language_core::ReferenceAccess;
use language_semantics::MachineSupplyMode;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::{
    TypedTrees,
    expression::{TableBorrowExpression, TableNamePath},
    name::Identifier,
};

#[test]
fn signature_state_keeps_captured_write_origins() {
    let mut builder = SymbolTableBuilder::default();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("test"));
    let machines = builder.insert_children(
        root,
        [
            (SymbolKind::Machine, SymbolNameRef::Static("Caller")),
            (SymbolKind::Machine, SymbolNameRef::Static("External")),
        ],
    );
    let caller_symbol = machines.handle_at(0);
    let external_symbol = machines.handle_at(1);
    let caller_state = builder
        .insert_children(
            caller_symbol,
            [(SymbolKind::State, SymbolNameRef::Static("entry"))],
        )
        .start();
    let external_state = builder
        .insert_children(
            external_symbol,
            [(SymbolKind::State, SymbolNameRef::Static("entry"))],
        )
        .start();
    let locals = builder.insert_children(
        caller_state,
        [
            (SymbolKind::Parameter, SymbolNameRef::Static("syntax")),
            (SymbolKind::Parameter, SymbolNameRef::Static("captured")),
        ],
    );
    let destination = builder
        .insert_children(
            external_state,
            [(SymbolKind::Parameter, SymbolNameRef::Static("destination"))],
        )
        .start();
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..Default::default()
    };
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let pointer = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: unit,
            access: ReferenceAccess::Mutable,
            lifetime: None,
        });
    let members = program.expression_table.reserve_name_path_members(1);
    program.expression_table.set_name_path_member_at_offset(
        members,
        0,
        Identifier::generated("syntax"),
    );
    let member_symbols = program.expression_table.reserve_name_path_member_symbols(1);
    program
        .expression_table
        .set_name_path_member_symbol_at_offset(member_symbols, 0, locals.handle_at(0));
    let name = program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath::new(
            members,
            member_symbols,
            locals.handle_at(0),
            locals.handle_at(0),
        )));
    let argument = program
        .expression_table
        .insert(ExpressionNode::Borrow(TableBorrowExpression {
            target: name,
            access: ReferenceAccess::Mutable,
        }));
    let mut entry = State {
        symbol: caller_state,
        name: Identifier::generated("entry"),
        return_type: unit,
        ..Default::default()
    };
    for (symbol, name) in [
        (locals.handle_at(0), "syntax"),
        (locals.handle_at(1), "captured"),
    ] {
        program.push_state_parameter(
            &mut entry,
            StateParameter {
                symbol,
                name: Identifier::generated(name),
                type_reference: unit,
                is_mutable: true,
                ..Default::default()
            },
        );
    }
    entry.statement_nodes = program
        .statement_table
        .insert_statements([StatementNode::Expression(argument)]);
    let mut caller = Machine {
        symbol: caller_symbol,
        name: Identifier::generated("Caller"),
        ..Default::default()
    };
    program.push_machine_state(&mut caller, entry);
    program.push_machine(caller);
    let mut entry = State {
        symbol: external_state,
        name: Identifier::generated("entry"),
        return_type: unit,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut entry,
        StateParameter {
            symbol: destination,
            name: Identifier::generated("destination"),
            type_reference: pointer,
            ..Default::default()
        },
    );
    let mut external = Machine {
        symbol: external_symbol,
        name: Identifier::generated("External"),
        supply_mode: MachineSupplyMode::Boundary,
        ..Default::default()
    };
    program.push_machine_state(&mut external, entry);
    program.push_machine(external);
    let caller = &program.machines()[0];
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(&program, &mut diagnostics);
    let machine_symbols = MachineSymbols::build(&program, caller, &mut diagnostics);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let resolve = |origins: Option<&[Option<Vec<FramePlaceOrigin>>]>| {
        known_call_written_paths_for_parts(
            &program,
            external_state,
            "entry",
            &[],
            &[],
            None,
            &[argument],
            caller,
            &machine_symbols,
            &symbols,
            &mut FrameInference::default(),
            origins,
        )
    };
    assert_eq!(resolve(None), Some(vec!["syntax".to_owned()]));
    assert_eq!(resolve(Some(&[None])), None);
    let captured = FramePlaceOrigin {
        path: "captured".to_owned(),
        precision: FramePathPrecision::Exact,
        source: FrameSourcePlace {
            root: locals.handle_at(1),
            builtin_coordinates: true,
            ..Default::default()
        },
    };
    assert_eq!(
        resolve(Some(&[Some(vec![captured])])),
        Some(vec!["captured".to_owned()])
    );
}
