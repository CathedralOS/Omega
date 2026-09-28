//! Field-type projection commits ownership evidence to one exact data
//! declaration. A retained type symbol is exact identity; a retained name is
//! only a fallback spelling. When either selects more than one declaration —
//! a duplicated symbol row or two same-named definitions — the projection can
//! no longer be re-derived for the exact subject and must refuse the premise
//! rather than letting the first same-shaped row mint a field type.
use super::canonical_place_type_reference;
use crate::flow::CanonicalPlace;
use crate::flow::canonical_place_from_expression;
use crate::flow::ownership::discover_state_move_events;
use crate::tests::front_end::typed_program;
use checked_trees::expression::ExpressionNode;
use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

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
    let program = typed_program(
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

/// `observe` returns a member read whose receiver is a value-producing
/// expression rather than a named place: the canonical place roots at the
/// expression itself and the member type must be reconstructed from the
/// position walk's leaf evidence.
struct RootedFixture {
    program: typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    place: CanonicalPlace,
    statement_count: usize,
}

impl RootedFixture {
    fn project(&self) -> Option<TypeReferenceHandle> {
        canonical_place_type_reference(
            &self.program,
            self.state_symbol,
            self.statement_count,
            &self.place,
        )
    }
}

/// The body's single member expression names `member_name`; its canonical
/// place roots at whatever expression produces the receiver — a dispatch, a
/// literal, an indexed window, or a call.
fn rooted_fixture(source: &str, machine_name: &str, member_name: &str) -> RootedFixture {
    let program = typed_program(source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("fixture machine");
    let state = &program.machine_states(machine)[0];
    let member = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            let ExpressionNode::Member(member) = node else {
                return None;
            };
            (member.member.as_str() == member_name).then_some(handle)
        })
        .expect("fixture member expression");
    let place = canonical_place_from_expression(&program, member)
        .expect("a member read must form a canonical place");
    RootedFixture {
        state_symbol: state.symbol,
        statement_count: program
            .statement_table
            .statements(state.statement_nodes)
            .len(),
        place,
        program,
    }
}

fn declared_field_type(
    program: &typed_trees::TypedTrees,
    type_name: &str,
    field_name: &str,
) -> TypeReferenceHandle {
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == type_name)
        .and_then(|data| {
            program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    DataMember::Field(field) if field.name.as_str() == field_name => {
                        Some(field.type_reference)
                    }
                    _ => None,
                })
        })
        .expect("declared field type")
}

/// `[context, context][0].scheduler` roots at the literal, which the
/// position walk proves is a window over the element: the index hop resumes
/// at the element and the field segment projects its declared type.
#[test]
fn array_literal_rooted_place_resumes_at_the_element_through_the_index() {
    let fixture = rooted_fixture(
        "data Main {}
         machine Main::run(&mut self) {}
         pub data SchedulerHandle [copy] {}
         pub data Context [copy] { scheduler: SchedulerHandle; }
         machine observe(context: Context) -> SchedulerHandle {
             [context, context][0].scheduler
         }",
        "observe",
        "scheduler",
    );
    assert_eq!(
        fixture.project(),
        Some(declared_field_type(
            &fixture.program,
            "Context",
            "scheduler"
        )),
        "the indexed literal must project the element's declared field type"
    );
}

/// A member demanded on the literal itself names a field of the array,
/// which declares none: the window position keeps no reference rather than
/// lending the element's field type to the collection.
#[test]
fn array_literal_members_keep_no_reference_on_the_literal_itself() {
    let fixture = rooted_fixture(
        "data Main {}
         machine Main::run(&mut self) {}
         pub data SchedulerHandle [copy] {}
         pub data Context [copy] { scheduler: SchedulerHandle; }
         machine observe(context: Context) -> SchedulerHandle {
             [context, context].scheduler
         }",
        "observe",
        "scheduler",
    );
    assert_eq!(fixture.project(), None);
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
