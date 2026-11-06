//! Exercise the published entry namespace, not a fabricated completed-call proof.
use super::*;
use arena::HandleSpan;
use checked_trees::{BorrowFacts, IncomingReferenceIdentity, IncomingReferenceOrigin};
use facts::{PlaceRoot, PlaceSegment};
use language_core::{BindingRelevance, ReferenceAccess};
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::{
    TypedTrees,
    machine::Machine,
    name::Identifier,
    signature::StateParameter,
    state::State,
    types::{TypeReferenceHandle, TypeReferenceNode},
};

struct Fixture {
    program: TypedTrees,
    parameter: SymbolHandle,
    other: SymbolHandle,
    unit: TypeReferenceHandle,
}

impl Fixture {
    fn new(is_const: bool, relevance: BindingRelevance, duplicate: bool) -> Self {
        let mut symbols = SymbolTableBuilder::default();
        let machine = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("owner"));
        let state = symbols
            .insert_children(
                machine,
                [(SymbolKind::State, SymbolNameRef::Static("entry"))],
            )
            .start();
        let parameters = symbols.insert_children(
            state,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("outer")),
                (SymbolKind::Parameter, SymbolNameRef::Static("other")),
            ],
        );
        let parameter = parameters.handle_at(0);
        let other = parameters.handle_at(1);
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let inner = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: unit,
                access: ReferenceAccess::Mutable,
                lifetime: None,
            });
        let outer = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: inner,
                access: ReferenceAccess::Mutable,
                lifetime: None,
            });
        let mut entry = State {
            symbol: state,
            name: Identifier::generated("entry"),
            return_type: unit,
            ..Default::default()
        };
        // Deliberately use declaration order different from the symbol order.
        program.push_state_parameter(
            &mut entry,
            StateParameter {
                symbol: other,
                name: Identifier::generated("other"),
                type_reference: inner,
                ..Default::default()
            },
        );
        let primary = StateParameter {
            symbol: parameter,
            name: Identifier::generated("outer"),
            type_reference: outer,
            is_const,
            relevance,
            ..Default::default()
        };
        program.push_state_parameter(&mut entry, primary.clone());
        if duplicate {
            program.push_state_parameter(&mut entry, primary);
        }
        let mut owner = Machine {
            symbol: machine,
            name: Identifier::generated("owner"),
            ..Default::default()
        };
        program.push_machine_state(&mut owner, entry);
        program.push_machine(owner);
        Self {
            program,
            parameter,
            other,
            unit,
        }
    }

    fn machine(&self) -> &Machine {
        &self.program.machines()[0]
    }

    fn state(&self) -> &State {
        &self.program.machine_states(self.machine())[0]
    }

    fn publish(&self) -> (BorrowFacts, HandleSpan<IncomingReferenceIdentity>) {
        let borrow = super::super::build_borrow_facts(&self.program, None)
            .expect("the real entry publisher must accept this empty body");
        let states = borrow.states.iter().collect::<Vec<_>>();
        assert_eq!(states.len(), 1);
        assert!(
            states[0].1.loans.is_empty(),
            "entry grants are not statement loans"
        );
        let members = states[0].1.incoming_references;
        (borrow, members)
    }

    fn candidate(&self) -> RebasedBorrowPlace {
        let mut sources = call_views::rebase_borrow_places_through_local_loans(
            &self.program,
            checked_trees::CapturedPlace {
                root: PlaceRoot::Symbol(self.parameter),
                ..Default::default()
            },
            &[],
        );
        assert_eq!(sources.len(), 1);
        sources.pop().unwrap()
    }

    fn bind(
        &self,
        borrow: &BorrowFacts,
        members: HandleSpan<IncomingReferenceIdentity>,
        candidate: &mut RebasedBorrowPlace,
    ) {
        ParameterReferentGrants::new(
            &self.program,
            self.machine(),
            self.state(),
            &borrow.incoming_references,
            members,
        )
        .bind(self.state().symbol, candidate);
    }
}

#[test]
fn forwarded_formal_reuses_the_published_root_grant_not_the_stored_pointer() {
    let fixture = Fixture::new(false, BindingRelevance::Relevant, false);
    let (borrow, members) = fixture.publish();
    let published = borrow.incoming_references.span(members).unwrap();
    let root = members
        .handles()
        .zip(published)
        .find_map(|(handle, row)| {
            (row.origin
                == IncomingReferenceOrigin::ParameterReferent {
                    symbol: fixture.parameter,
                })
            .then_some(handle)
        })
        .expect("the publisher supplies the formal's outer grant");
    let stored = members
        .handles()
        .zip(published)
        .find_map(|(handle, row)| {
            (row.origin
                == IncomingReferenceOrigin::Parameter {
                    symbol: fixture.parameter,
                }
                && row.owner_path.is_empty())
            .then_some(handle)
        })
        .expect("the nested reference has a separate stored permission");
    assert_ne!(root, stored);
    let before = borrow.incoming_references.len();
    let mut source = fixture.candidate();
    assert!(!source.place.incoming_reference.is_valid());
    fixture.bind(&borrow, members, &mut source);
    assert_eq!(source.place.incoming_reference, root);
    assert!(!source.parent_loan.is_valid());
    assert!(!source.source_loan.is_valid());
    assert_eq!(
        borrow.incoming_references.len(),
        before,
        "binding allocates no new identity"
    );

    let mut loaded = fixture.candidate();
    loaded.place.incoming_reference = stored;
    fixture.bind(&borrow, members, &mut loaded);
    assert_eq!(
        loaded.place.incoming_reference, stored,
        "an existing inner occurrence is preserved"
    );
    let mut wrong_root = fixture.candidate();
    wrong_root.place.root = PlaceRoot::Symbol(fixture.other);
    let mut descriptor_family = fixture.candidate();
    descriptor_family
        .place
        .incoming_owner_path
        .push(PlaceSegment::FixedIndex { index: 0 });
    for mut unrelated in [wrong_root, descriptor_family] {
        fixture.bind(&borrow, members, &mut unrelated);
        assert!(!unrelated.place.incoming_reference.is_valid());
    }
    let mut interior = fixture.candidate();
    interior
        .place
        .segments
        .push(PlaceSegment::FixedIndex { index: 0 });
    fixture.bind(&borrow, members, &mut interior);
    assert!(
        !interior.place.incoming_reference.is_valid(),
        "a stored child cannot acquire the outer grant"
    );
}

#[test]
fn root_grant_binding_refuses_missing_ambiguous_or_foreign_entry_evidence() {
    let fixture = Fixture::new(false, BindingRelevance::Relevant, false);
    let (borrow, members) = fixture.publish();
    let root = members
        .handles()
        .find(|handle| {
            borrow.incoming_references.get(*handle).origin
                == IncomingReferenceOrigin::ParameterReferent {
                    symbol: fixture.parameter,
                }
        })
        .unwrap();
    let wrong_generation = HandleSpan::from_parts(
        Handle::from_parts(
            members.start().arena_index(),
            members.start().generation() + 1,
        ),
        members.count(),
    );
    for unavailable in [HandleSpan::empty(), wrong_generation] {
        let mut source = fixture.candidate();
        fixture.bind(&borrow, unavailable, &mut source);
        assert!(!source.place.incoming_reference.is_valid());
    }
    for change in [0, 1, 2, 3] {
        let mut changed = borrow.clone();
        let row = changed.incoming_references.get_mut(root);
        match change {
            0 => row.machine_symbol = SymbolHandle::invalid(),
            1 => row.state_symbol = SymbolHandle::invalid(),
            2 => row.carrier_type = fixture.unit,
            3 => {
                row.origin = IncomingReferenceOrigin::ParameterReferent {
                    symbol: SymbolHandle::from_parts(
                        fixture.parameter.arena_index(),
                        fixture.parameter.generation() + 1,
                    ),
                }
            }
            _ => unreachable!(),
        }
        let mut source = fixture.candidate();
        fixture.bind(&changed, members, &mut source);
        assert!(
            !source.place.incoming_reference.is_valid(),
            "invalid entry variant {change}"
        );
    }
    let mut duplicated = borrow.clone();
    let duplicate = duplicated.incoming_references.get(root).clone();
    let mut duplicate_members = members;
    duplicated
        .incoming_references
        .append_to_span(&mut duplicate_members, duplicate);
    let mut source = fixture.candidate();
    fixture.bind(&duplicated, duplicate_members, &mut source);
    assert!(
        !source.place.incoming_reference.is_valid(),
        "duplicate member grants stay ambiguous"
    );

    for invalid in [
        Fixture::new(true, BindingRelevance::Relevant, false),
        Fixture::new(false, BindingRelevance::Erased, false),
        Fixture::new(false, BindingRelevance::Relevant, true),
    ] {
        let (published, members) = invalid.publish();
        let mut source = invalid.candidate();
        invalid.bind(&published, members, &mut source);
        assert!(
            !source.place.incoming_reference.is_valid(),
            "const, erased and duplicate formals cannot lend runtime permission"
        );
    }
}
