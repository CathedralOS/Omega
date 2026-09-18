//! Field-type projection commits ownership evidence to one exact data
//! declaration. A retained type symbol is exact identity; a retained name is
//! only a fallback spelling. When either selects more than one declaration —
//! a duplicated symbol row or two same-named definitions — the projection can
//! no longer be re-derived for the exact subject and must refuse the premise
//! rather than letting the first same-shaped row mint a field type.
use super::canonical_place_type_reference;
use crate::flow::CanonicalPlace;
use crate::flow::ownership::discover_state_move_events;
use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

fn typed_source(source: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize place-type fixture");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse place-type fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve place-type fixture");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type place-type fixture")
}

/// `exercise` holds an `Outer` parameter and returns its `inner` field: the
/// read's ownership disposition depends entirely on the field-type projection
/// proving `inner: u64` (a copy) through the parameter's declared type.
struct Fixture {
    program: typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    value_symbol: SymbolHandle,
    value_type: TypeReferenceHandle,
    field_symbol: SymbolHandle,
    field_type: TypeReferenceHandle,
    statement_count: usize,
}

impl Fixture {
    fn place(&self) -> CanonicalPlace {
        CanonicalPlace {
            root: facts::PlaceRoot::Symbol(self.value_symbol),
            segments: vec![facts::PlaceSegment::Field {
                symbol: self.field_symbol,
            }],
        }
    }

    fn project(&self) -> Option<TypeReferenceHandle> {
        // A parameter-rooted place does not depend on statement position; the
        // whole statement prefix stays in scope.
        canonical_place_type_reference(
            &self.program,
            self.state_symbol,
            self.statement_count,
            &self.place(),
        )
    }

    fn move_events(&self) -> Vec<(facts::PlaceRoot, Vec<facts::PlaceSegment>)> {
        let machine = self
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == self.machine_symbol)
            .expect("fixture machine");
        let state = self
            .program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == self.state_symbol)
            .expect("fixture state");
        let borrow = checked_trees::BorrowFacts::default();
        let operators = checked_trees::CheckedOperatorFacts::default();
        let mut segments = arena::Arena::default();
        discover_state_move_events(
            &self.program,
            &borrow,
            &operators,
            machine,
            state,
            &mut segments,
        )
        .into_iter()
        .map(|event| (event.root, segments.span_or_empty(event.segments).to_vec()))
        .collect()
    }

    /// Strip the declared parameter type's resolved symbol so only the
    /// authored `Outer` spelling remains to identify the declaration.
    fn detach_value_type_symbol(&mut self) {
        let TypeReferenceNode::Named { name, .. } = self
            .program
            .type_reference_table
            .type_reference(self.value_type)
            .clone()
        else {
            panic!("fixture parameter must retain a nominal type")
        };
        self.program.type_reference_table.substitute_node(
            self.value_type,
            TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name,
            },
        );
    }

    /// A second `Outer` declaration — same name and, while cloned, the same
    /// symbol — enters the program without the retained type identity
    /// changing to distinguish it.
    fn push_duplicate_outer(&mut self) {
        let duplicate = self
            .program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Outer")
            .expect("fixture Outer declaration")
            .clone();
        self.program.push_data_definition(duplicate);
    }
}

fn fixture() -> Fixture {
    let program = typed_source(
        "data Outer { inner: u64; }
         machine exercise(value: Outer) -> u64 { value.inner }",
    );
    let machine_symbol = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .expect("fixture machine")
        .symbol;
    fixture_from_machine(program, machine_symbol)
}

/// `Outer::read` attaches `Outer` non-generically, so `self.inner` projects
/// through the machine's retained `attached_data_symbol` — the symbol-only
/// declaration lookup, not the parameter's type reference.
fn attached_fixture() -> Fixture {
    let program = typed_source(
        "data Outer { inner: u64; }
         machine Outer::read(&self) -> u64 { self.inner }",
    );
    let machine_symbol = program
        .machines()
        .iter()
        .find(|machine| machine.attached_data.is_some())
        .expect("fixture attached machine")
        .symbol;
    fixture_from_machine(program, machine_symbol)
}

fn fixture_from_machine(program: typed_trees::TypedTrees, machine_symbol: SymbolHandle) -> Fixture {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("fixture machine");
    let state = &program.machine_states(machine)[0];
    // The projected place roots at the first state parameter: `value` for the
    // free machine, `&self` for the attached machine. `self` may also
    // normalize to the machine symbol; the projection accepts either form.
    let value = program.state_parameters(state)[0].clone();
    let outer = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .expect("fixture Outer declaration");
    let (field_symbol, field_type) = program
        .data_members(outer)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "inner" => {
                Some((field.symbol, field.type_reference))
            }
            _ => None,
        })
        .expect("fixture inner field");
    let state_symbol = state.symbol;
    let value_symbol = value.symbol;
    let value_type = value.type_reference;
    let statement_count = program
        .statement_table
        .statements(state.statement_nodes)
        .len();
    Fixture {
        program,
        machine_symbol,
        state_symbol,
        value_symbol,
        value_type,
        field_symbol,
        field_type,
        statement_count,
    }
}

#[test]
fn exact_symbol_and_unique_name_still_project_the_declared_field_type() {
    let mut fixture = fixture();
    assert_eq!(fixture.project(), Some(fixture.field_type));

    // A retained name alone may still identify the declaration — but only
    // while exactly one declaration carries it.
    fixture.detach_value_type_symbol();
    assert_eq!(fixture.project(), Some(fixture.field_type));
}

#[test]
fn same_named_declarations_cannot_mint_field_type_evidence() {
    // Two same-named declarations make the name-only fallback ambiguous.
    let mut ambiguous_name = fixture();
    ambiguous_name.detach_value_type_symbol();
    ambiguous_name.push_duplicate_outer();
    assert_eq!(ambiguous_name.project(), None);
}

#[test]
fn duplicated_symbol_rows_cannot_mint_field_type_evidence() {
    // A duplicated symbol row is equally ambiguous for exact identity.
    let mut ambiguous_symbol = fixture();
    ambiguous_symbol.push_duplicate_outer();
    assert_eq!(ambiguous_symbol.project(), None);
}

#[test]
fn type_reference_rooted_place_projects_through_its_segments() {
    // A place rooted at a stored type reference names that type directly:
    // its declared type is the root replayed through the retained segments,
    // so `Outer.inner` lands on the field's own `u64`.
    let fixture = fixture();
    let place = CanonicalPlace {
        root: facts::PlaceRoot::TypeReference(fixture.value_type),
        segments: vec![facts::PlaceSegment::Field {
            symbol: fixture.field_symbol,
        }],
    };
    assert_eq!(
        canonical_place_type_reference(
            &fixture.program,
            fixture.state_symbol,
            fixture.statement_count,
            &place,
        ),
        Some(fixture.field_type)
    );
}

#[test]
fn attached_self_place_projects_through_the_retained_declaration_identity() {
    // A non-generic attachment carries no application handle, so `self.inner`
    // resolves its declaration through `attached_data_symbol` alone.
    let fixture = attached_fixture();
    assert_eq!(fixture.project(), Some(fixture.field_type));
}

#[test]
fn attached_declaration_duplicates_cannot_mint_field_type_evidence() {
    // A second row bearing `attached_data_symbol` makes the retained identity
    // ambiguous; the projection must refuse rather than pick a winner.
    let mut fixture = attached_fixture();
    fixture.push_duplicate_outer();
    assert_eq!(fixture.project(), None);

    // The read keeps its ownership obligation instead of being accepted as a
    // copy on the strength of the first same-shaped row.
    assert!(
        fixture.move_events().iter().any(|(_, segments)| {
            *segments
                == vec![facts::PlaceSegment::Field {
                    symbol: fixture.field_symbol,
                }]
        }),
        "ambiguous attached provenance must retain the move, not mint a copy"
    );
}

#[test]
fn unprovable_field_provenance_keeps_the_ownership_obligation() {
    // Exact provenance proves `inner: u64`, so the result-position read is a
    // copy and records no ownership event.
    let proven = fixture();
    assert!(
        proven
            .move_events()
            .iter()
            .all(|(root, _)| *root != facts::PlaceRoot::Symbol(proven.value_symbol)),
        "a proven copy read must not record a move"
    );

    // Once the declared type's subject is ambiguous, the projection cannot
    // reconstruct which declaration supplied the field type. The read keeps
    // its ownership obligation instead of being accepted as a copy on the
    // strength of a same-name row.
    let mut ambiguous = fixture();
    ambiguous.detach_value_type_symbol();
    ambiguous.push_duplicate_outer();
    assert!(
        ambiguous.move_events().iter().any(|(root, segments)| {
            *root == facts::PlaceRoot::Symbol(ambiguous.value_symbol)
                && *segments
                    == vec![facts::PlaceSegment::Field {
                        symbol: ambiguous.field_symbol,
                    }]
        }),
        "ambiguous field provenance must retain the move, not mint a copy"
    );
}
