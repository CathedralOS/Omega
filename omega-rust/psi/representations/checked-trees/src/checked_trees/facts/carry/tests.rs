use super::{SuspensionCrossingCarryFact, canonical_suspension_crossing_id};
use language_semantics::CarryPolicy;
use semantic_vocabulary::SuspensionCrossingId;
use source::{SourceMap, SourceSpan, Span};
use std::{path::PathBuf, sync::Arc};
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;

fn crossing_identity(module: Option<&str>) -> SuspensionCrossingId {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(
            PathBuf::from("crossing.omg"),
            format!("{} value entry", module.unwrap_or("left")),
        )
        .source_id;
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let machine_symbol = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [(
            SymbolKind::Machine,
            SymbolNameRef::Source(SourceSpan::new(source_id, Span::new(5, 10))),
        )],
    ))
    .next()
    .expect("machine symbol");
    let state_symbol = SymbolTableBuilder::child_handles(builder.insert_children(
        machine_symbol,
        [(
            SymbolKind::State,
            SymbolNameRef::Source(SourceSpan::new(source_id, Span::new(11, 16))),
        )],
    ))
    .next()
    .expect("state symbol");
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..TypedTrees::default()
    };
    if let Some(module) = module {
        program
            .symbols
            .register_source_module(
                source_id,
                [(module, SourceSpan::new(source_id, Span::new(0, 4)))],
            )
            .expect("register module");
    }
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("value"),
        ..Machine::default()
    };
    program.push_machine_state(
        &mut machine,
        State {
            symbol: state_symbol,
            name: Identifier::generated("entry"),
            ..State::default()
        },
    );
    program.push_machine(machine);
    canonical_suspension_crossing_id(
        &program,
        &SuspensionCrossingCarryFact {
            machine: machine_symbol,
            state: state_symbol,
            statement_index: 3,
            call_ordinal: 2,
            target: machine_symbol,
            receiver: None,
            effective: CarryPolicy::PERMISSIVE,
            live_values: Vec::new(),
        },
    )
    .expect("crossing identity")
}

#[test]
fn namespaced_crossings_retain_distinct_declaration_identities() {
    assert_ne!(
        crossing_identity(Some("left")),
        crossing_identity(Some("next"))
    );
}

#[test]
fn unmoduled_crossing_preserves_its_published_identity() {
    assert_eq!(
        crossing_identity(None),
        SuspensionCrossingId::new(13_975_233_874_791_475_505).expect("legacy identity"),
    );
}
