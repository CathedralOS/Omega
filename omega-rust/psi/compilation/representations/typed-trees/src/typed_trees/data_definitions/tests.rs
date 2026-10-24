use super::*;
use crate::name::Identifier;
use crate::{FrozenTypedTrees, TypedTrees};
use arena::Handle;

fn definition(symbol: SymbolHandle, name: &str) -> DataDefinition {
    DataDefinition {
        symbol,
        name: Identifier::generated(name),
        ..DataDefinition::default()
    }
}

// The independent roster scan remains the oracle for every indexed operation.
fn assert_fresh_rows(program: &TypedTrees, symbols: &[SymbolHandle], names: &[&str]) {
    let definitions = program.data_definitions();
    for &symbol in symbols {
        let positions: Vec<_> = definitions
            .iter()
            .enumerate()
            .filter(|(_, definition)| symbol.is_valid() && definition.symbol == symbol)
            .map(|(position, _)| position as u32)
            .collect();
        let first = positions.first().map(|position| *position as usize);
        let unique = match positions.as_slice() {
            [position] => Some(&definitions[*position as usize]),
            _ => None,
        };
        assert_eq!(
            program.data_definition_positions_for_symbol(symbol),
            positions
        );
        program.with_data_definition_index(|index| {
            assert_eq!(index.positions_for_symbol(symbol), positions.as_slice());
        });
        assert_eq!(program.data_definition_position_by_symbol(symbol), first);
        assert_eq!(
            program.data_definition_by_symbol(symbol),
            first.map(|position| &definitions[position])
        );
        assert_eq!(program.unique_data_definition_by_symbol(symbol), unique);
    }
    for &name in names {
        let positions: Vec<_> = definitions
            .iter()
            .enumerate()
            .filter(|(_, definition)| definition.name.as_str() == name)
            .map(|(position, _)| position as u32)
            .collect();
        assert_eq!(program.data_definition_positions_for_name(name), positions);
    }
}

#[test]
fn data_definition_rows_preserve_unique_and_first_match_queries() {
    let original = SymbolHandle::from_parts(1, 1);
    let next_generation = SymbolHandle::from_parts(1, 2);
    let other = SymbolHandle::from_parts(2, 1);
    let missing = SymbolHandle::from_parts(1, 3);
    let invalid = SymbolHandle::invalid();
    let symbols = [original, next_generation, other, missing, invalid];
    let names = ["Shared", "Replacement", "Duplicate", "Distinct", "Missing"];
    let mut program = TypedTrees::default();
    assert_fresh_rows(&program, &symbols, &names);
    for (symbol, name) in [
        (original, "Shared"),
        (next_generation, "Replacement"),
        (original, "Duplicate"),
        (invalid, "Shared"),
        (other, "Distinct"),
    ] {
        program.push_data_definition(definition(symbol, name));
        assert_fresh_rows(&program, &symbols, &names);
    }
    assert_eq!(
        program.data_definition_positions_for_symbol(original),
        [0, 2]
    );
    assert_eq!(program.data_definition_positions_for_name("Shared"), [0, 3]);
    assert_eq!(
        program
            .data_definition_by_symbol(original)
            .unwrap()
            .name
            .as_str(),
        "Shared"
    );
    assert!(program.unique_data_definition_by_symbol(original).is_none());
    assert!(
        program
            .unique_data_definition_by_symbol(next_generation)
            .is_some()
    );
}

#[test]
fn data_definition_rows_follow_storage_and_roster_changes() {
    let original = SymbolHandle::from_parts(1, 1);
    let replacement = SymbolHandle::from_parts(2, 1);
    let other = SymbolHandle::from_parts(3, 1);
    let symbols = [original, replacement, other, SymbolHandle::invalid()];
    let names = ["First", "Second", "Replacement"];
    let mut program = TypedTrees::default();
    program.push_data_definition(definition(original, "First"));
    program.push_data_definition(definition(other, "Second"));
    let roster = program.roots.data_definitions;
    assert_fresh_rows(&program, &symbols, &names);

    // A +1 arena revision and -1 roster count used to cancel in the summed key.
    let revision = program.tables.data_definitions.revision();
    program
        .tables
        .data_definitions
        .get_mut(roster.start())
        .symbol = replacement;
    assert_eq!(program.tables.data_definitions.revision(), revision + 1);
    program.roots.data_definitions = HandleSpan::from_parts(roster.start(), 1);
    assert_fresh_rows(&program, &symbols, &names);
    program.roots.data_definitions = roster;
    assert_fresh_rows(&program, &symbols, &names);

    // A root with the same index/count but a different generation is a different
    // visible roster, including when it is invalid and exposes no declarations.
    program.roots.data_definitions = HandleSpan::from_parts(
        Handle::from_parts(
            roster.start().arena_index(),
            roster.start().generation() + 1,
        ),
        roster.count(),
    );
    assert!(program.data_definitions().is_empty());
    assert_fresh_rows(&program, &symbols, &names);
    program.roots.data_definitions = roster;
    assert_fresh_rows(&program, &symbols, &names);

    let mut snapshot = program.clone();
    snapshot
        .tables
        .data_definitions
        .get_mut(roster.start())
        .symbol = original;
    assert_fresh_rows(&snapshot, &symbols, &names);
    assert_fresh_rows(&program, &symbols, &names);
    assert!(program.unique_data_definition_by_symbol(original).is_none());
    assert!(
        snapshot
            .unique_data_definition_by_symbol(original)
            .is_some()
    );

    let frozen = FrozenTypedTrees::from(program);
    assert_fresh_rows(&frozen, &symbols, &names);
    let mut program = frozen.into_inner();
    program
        .tables
        .data_definitions
        .get_mut(roster.start())
        .symbol = original;
    assert_fresh_rows(&program, &symbols, &names);
    assert_fresh_rows(&FrozenTypedTrees::from(program), &symbols, &names);

    // An exclusive raw-arena replacement must invalidate a warm index even if
    // the incoming arena has exactly the same revision and handle coordinates.
    let mut program = TypedTrees::default();
    program.push_data_definition(definition(original, "First"));
    assert_fresh_rows(&program, &symbols, &names);
    let mut incoming = Arena::default();
    let mut incoming_roster = HandleSpan::empty();
    let incoming_handle =
        incoming.append_to_span(&mut incoming_roster, definition(replacement, "Replacement"));
    assert_eq!(
        incoming.revision(),
        program.tables.data_definitions.revision()
    );
    assert_eq!(incoming_handle, program.roots.data_definitions.start());
    *program.tables.data_definitions = incoming;
    assert_fresh_rows(&program, &symbols, &names);

    // Public field and whole-table replacement move the matching warm owner,
    // rather than leaving a separate program-level index from the old arena.
    let mut source = TypedTrees::default();
    source.push_data_definition(definition(other, "Second"));
    assert_fresh_rows(&source, &symbols, &names);
    program.tables.data_definitions = source.tables.data_definitions.clone();
    program.roots.data_definitions = source.roots.data_definitions;
    assert_fresh_rows(&program, &symbols, &names);
    program.tables = snapshot.tables.clone();
    program.roots = snapshot.roots.clone();
    assert_fresh_rows(&program, &symbols, &names);
    assert_fresh_rows(&source, &symbols, &names);
}
