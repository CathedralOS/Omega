//! Statement transfer propagation tests.

use super::{
    ExpressionHandle, FactPayload, FactPlan, PlaceHandle, ProgramPoint, QualificationEvidence,
    QualificationPayloadIdentity, SymbolHandle,
};
use crate::flow::transfers::retain_qualification_correspondence;
use arena::HandleSpan;
use facts::{Fact, FactOrigin, FactPlace, PlaceSegment};
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

struct CorrespondenceFixture {
    program: typed_trees::TypedTrees,
    semantic: FactPlan,
    source_fact: facts::FactHandle,
    destination_fact: facts::FactHandle,
    source_place: PlaceHandle,
    source_occurrence_place: PlaceHandle,
    destination_place: PlaceHandle,
    formation: ProgramPoint,
    payload: FactPayload,
    evidence: QualificationEvidence,
    local: SymbolHandle,
    exact_local: SymbolHandle,
    data_symbol: SymbolHandle,
    wrong_data_symbol: SymbolHandle,
    foreign_parameter: SymbolHandle,
    sibling_state_parameter: SymbolHandle,
    foreign_local: SymbolHandle,
    sibling_state_local: SymbolHandle,
}

fn correspondence_fixture() -> CorrespondenceFixture {
    let mut symbols = SymbolTableBuilder::new();
    let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = symbols.insert_children(
        root,
        [
            (SymbolKind::Machine, SymbolNameRef::Static("transform")),
            (SymbolKind::Domain, SymbolNameRef::Static("Ready")),
            (SymbolKind::Data, SymbolNameRef::Static("Pair")),
            (SymbolKind::Local, SymbolNameRef::Static("excluded_local")),
            (
                SymbolKind::Machine,
                SymbolNameRef::Static("foreign_transform"),
            ),
        ],
    );
    let declarations = SymbolTableBuilder::child_handles(declarations).collect::<Vec<_>>();
    let machine = declarations[0];
    let domain = declarations[1];
    let data_symbol = declarations[2];
    let local = declarations[3];
    let foreign_machine = declarations[4];
    let fields = SymbolTableBuilder::child_handles(symbols.insert_children(
        data_symbol,
        [
            (SymbolKind::Field, SymbolNameRef::Static("source")),
            (SymbolKind::Field, SymbolNameRef::Static("destination")),
        ],
    ))
    .collect::<Vec<_>>();
    let source_field = fields[0];
    let destination_field = fields[1];
    let machine_members = symbols.insert_children(
        machine,
        [
            (SymbolKind::State, SymbolNameRef::Static("entry")),
            (SymbolKind::Parameter, SymbolNameRef::Static("self")),
            (SymbolKind::State, SymbolNameRef::Static("sibling")),
        ],
    );
    let machine_members = SymbolTableBuilder::child_handles(machine_members).collect::<Vec<_>>();
    let state = machine_members[0];
    let parameter = machine_members[1];
    let sibling_state = machine_members[2];
    let sibling_members = SymbolTableBuilder::child_handles(symbols.insert_children(
        sibling_state,
        [
            (SymbolKind::Parameter, SymbolNameRef::Static("sibling_self")),
            (SymbolKind::Local, SymbolNameRef::Static("sibling_local")),
        ],
    ))
    .collect::<Vec<_>>();
    let sibling_state_parameter = sibling_members[0];
    let sibling_state_local = sibling_members[1];
    let foreign_members = SymbolTableBuilder::child_handles(symbols.insert_children(
        foreign_machine,
        [
            (SymbolKind::Parameter, SymbolNameRef::Static("foreign_self")),
            (SymbolKind::State, SymbolNameRef::Static("foreign_entry")),
        ],
    ))
    .collect::<Vec<_>>();
    let foreign_parameter = foreign_members[0];
    let foreign_state = foreign_members[1];
    let exact_local = SymbolTableBuilder::child_handles(
        symbols.insert_children(state, [(SymbolKind::Local, SymbolNameRef::Static("local"))]),
    )
    .next()
    .expect("exact state local");
    let foreign_local = SymbolTableBuilder::child_handles(symbols.insert_children(
        foreign_state,
        [(SymbolKind::Local, SymbolNameRef::Static("foreign_local"))],
    ))
    .next()
    .expect("foreign state local");
    let mut program = typed_trees::TypedTrees {
        symbols: symbols.finish(),
        ..typed_trees::TypedTrees::default()
    };
    let unit = program
        .type_reference_table
        .insert(typed_trees::types::TypeReferenceNode::Unit);
    let pair_type =
        program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Named {
                symbol: data_symbol,
                name: typed_trees::name::Identifier::generated("Pair"),
            });
    let mut data = typed_trees::data::DataDefinition {
        symbol: data_symbol,
        name: typed_trees::name::Identifier::generated("Pair"),
        ..Default::default()
    };
    for (symbol, name) in [(source_field, "source"), (destination_field, "destination")] {
        program.push_data_member(
            &mut data,
            typed_trees::data::DataMember::Field(typed_trees::data::DataField {
                symbol,
                name: typed_trees::name::Identifier::generated(name),
                type_reference: unit,
                ..Default::default()
            }),
        );
    }
    program.push_data_definition(data);
    let mut state_node = typed_trees::state::State {
        symbol: state,
        name: typed_trees::name::Identifier::generated("entry"),
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state_node,
        typed_trees::signature::StateParameter {
            symbol: parameter,
            name: typed_trees::name::Identifier::generated("self"),
            type_reference: pair_type,
            is_self: true,
            ..Default::default()
        },
    );
    program.statement_table.push_statement(
        &mut state_node.statement_nodes,
        typed_trees::statement::StatementNode::LocalData(typed_trees::statement::TableLocalData {
            symbol: exact_local,
            name: typed_trees::name::Identifier::generated("local"),
            type_reference: pair_type,
            initial_value: ExpressionHandle::invalid(),
            is_mutable: true,
            type_is_inferred: false,
            relevance: language_core::BindingRelevance::Relevant,
        }),
    );
    program.statement_table.push_statement(
        &mut state_node.statement_nodes,
        typed_trees::statement::StatementNode::Expression(ExpressionHandle::invalid()),
    );
    program.statement_table.push_statement(
        &mut state_node.statement_nodes,
        typed_trees::statement::StatementNode::Expression(ExpressionHandle::invalid()),
    );
    let mut machine_node = typed_trees::machine::Machine {
        symbol: machine,
        name: typed_trees::name::Identifier::generated("transform"),
        ..Default::default()
    };
    program.push_machine_state(&mut machine_node, state_node);
    program.push_machine(machine_node);

    let mut semantic = FactPlan::default();
    let source_place = semantic.append_symbol_place(parameter);
    semantic.push_place_segment(
        source_place,
        PlaceSegment::Field {
            symbol: source_field,
        },
    );
    let destination_place = semantic.append_symbol_place(parameter);
    semantic.push_place_segment(
        destination_place,
        PlaceSegment::Field {
            symbol: destination_field,
        },
    );
    let payload = FactPayload::DomainMembership {
        value: ExpressionHandle::invalid(),
        domain: HandleSpan::empty(),
        domain_symbol: domain,
        semantic_domain: language_semantics::SemanticDomainId::NULL,
    };
    let evidence = QualificationEvidence::from_origin(
        language_semantics::QualificationEvidenceOrigin::CheckedTransformation,
        machine,
    );
    let source_fact = semantic.append_fact(Fact {
        place: FactPlace::Place(source_place),
        point: ProgramPoint::CallEnsures {
            machine_symbol: machine,
            state_symbol: state,
            statement_index: 0,
            call_ordinal: 0,
        },
        origin: FactOrigin::CallEnsures,
        evidence,
        payload,
    });
    let formation = ProgramPoint::Statement {
        machine_symbol: machine,
        state_symbol: state,
        statement_index: 1,
    };
    let destination_fact = semantic.append_fact(Fact {
        place: FactPlace::Place(destination_place),
        point: formation,
        origin: FactOrigin::StatementTransfer,
        evidence,
        payload,
    });
    CorrespondenceFixture {
        program,
        semantic,
        source_fact,
        destination_fact,
        source_place,
        source_occurrence_place: source_place,
        destination_place,
        formation,
        payload,
        evidence,
        local,
        exact_local,
        data_symbol,
        wrong_data_symbol: foreign_machine,
        foreign_parameter,
        sibling_state_parameter,
        foreign_local,
        sibling_state_local,
    }
}

fn set_formation_parameter_type(
    fixture: &mut CorrespondenceFixture,
    type_reference: typed_trees::types::TypeReferenceHandle,
) {
    let ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        ..
    } = fixture.formation
    else {
        unreachable!("correspondence fixture formation")
    };
    let machine = fixture
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("formation machine");
    let state = fixture
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .expect("formation state");
    let parameter = state.parameters.start();
    fixture
        .program
        .state_parameters
        .get_mut(parameter)
        .type_reference = type_reference;
}

fn install_correspondence_paths(
    fixture: &mut CorrespondenceFixture,
    source: &[PlaceSegment],
    destination: &[PlaceSegment],
) {
    let root = fixture.semantic.places.get(fixture.source_place).root;
    let source_place = fixture.semantic.append_place(facts::Place {
        root,
        segments: HandleSpan::empty(),
    });
    for segment in source {
        fixture.semantic.push_place_segment(source_place, *segment);
    }
    let source_occurrence_place = fixture.semantic.append_place(facts::Place {
        root,
        segments: HandleSpan::empty(),
    });
    for segment in source {
        fixture
            .semantic
            .push_place_segment(source_occurrence_place, *segment);
    }
    let destination_place = fixture.semantic.append_place(facts::Place {
        root,
        segments: HandleSpan::empty(),
    });
    for segment in destination {
        fixture
            .semantic
            .push_place_segment(destination_place, *segment);
    }
    fixture.source_place = source_place;
    fixture.source_occurrence_place = source_occurrence_place;
    fixture.destination_place = destination_place;
    fixture.semantic.facts.get_mut(fixture.source_fact).place = FactPlace::Place(source_place);
    fixture
        .semantic
        .facts
        .get_mut(fixture.destination_fact)
        .place = FactPlace::Place(destination_place);
}

fn set_correspondence_roots(
    fixture: &mut CorrespondenceFixture,
    source_root: SymbolHandle,
    destination_root: SymbolHandle,
) {
    fixture.semantic.places.get_mut(fixture.source_place).root =
        facts::PlaceRoot::Symbol(source_root);
    fixture
        .semantic
        .places
        .get_mut(fixture.source_occurrence_place)
        .root = facts::PlaceRoot::Symbol(source_root);
    fixture
        .semantic
        .places
        .get_mut(fixture.destination_place)
        .root = facts::PlaceRoot::Symbol(destination_root);
}

fn set_pair_field_type(
    fixture: &mut CorrespondenceFixture,
    type_reference: typed_trees::types::TypeReferenceHandle,
) {
    let members = fixture
        .program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == fixture.data_symbol)
        .map(|data| data.members)
        .expect("Pair members");
    for member in fixture.program.data_members.span_mut_or_empty(members) {
        let typed_trees::data::DataMember::Field(field) = member else {
            unreachable!("Pair record field")
        };
        field.type_reference = type_reference;
    }
}

fn nested_fixed_array_fixture() -> CorrespondenceFixture {
    let mut fixture = correspondence_fixture();
    let source_field = fixture
        .semantic
        .place_segments
        .span(fixture.semantic.places.get(fixture.source_place).segments)
        .and_then(|segments| segments.first())
        .copied()
        .expect("source field");
    let destination_field = fixture
        .semantic
        .place_segments
        .span(
            fixture
                .semantic
                .places
                .get(fixture.destination_place)
                .segments,
        )
        .and_then(|segments| segments.first())
        .copied()
        .expect("destination field");
    let unit = fixture
        .program
        .type_reference_table
        .insert(typed_trees::types::TypeReferenceNode::Unit);
    let inner = fixture.program.type_reference_table.insert(
        typed_trees::types::TypeReferenceNode::FixedArray {
            element_type: unit,
            length: typed_trees::types::FixedArrayLength::Literal(2),
        },
    );
    let outer = fixture.program.type_reference_table.insert(
        typed_trees::types::TypeReferenceNode::FixedArray {
            element_type: inner,
            length: typed_trees::types::FixedArrayLength::Literal(2),
        },
    );
    set_pair_field_type(&mut fixture, outer);
    install_correspondence_paths(
        &mut fixture,
        &[
            source_field,
            PlaceSegment::FixedIndex { index: 0 },
            PlaceSegment::FixedIndex { index: 1 },
        ],
        &[
            destination_field,
            PlaceSegment::FixedIndex { index: 1 },
            PlaceSegment::FixedIndex { index: 0 },
        ],
    );
    fixture
}

fn retain(fixture: &mut CorrespondenceFixture) {
    let scalar_expressions = Default::default();
    let operators = Default::default();
    let state_mutation_summary_cache = crate::flow::StateMutationSummaryCache::default();
    let mut ctx = crate::flow::FlowBuildContext::new(
        &Default::default(),
        &Default::default(),
        &fixture.semantic,
        &scalar_expressions,
        &operators,
        &[],
        None,
        &state_mutation_summary_cache,
    );
    retain_qualification_correspondence(
        &fixture.program,
        &mut ctx,
        &mut fixture.semantic,
        fixture.source_fact,
        fixture.destination_fact,
        fixture.source_place,
        fixture.source_occurrence_place,
        fixture.destination_place,
        fixture.formation,
        fixture.payload,
        fixture.evidence,
    );
}

#[test]
fn qualification_correspondence_retains_exact_semantic_domain_identity() {
    let mut fixture = correspondence_fixture();
    let identity = fixture.program.semantic_domains.intern("Ready<7>");
    let FactPayload::DomainMembership {
        semantic_domain, ..
    } = &mut fixture.payload
    else {
        panic!("membership fixture");
    };
    *semantic_domain = identity;
    fixture.semantic.facts.get_mut(fixture.source_fact).payload = fixture.payload;
    fixture
        .semantic
        .facts
        .get_mut(fixture.destination_fact)
        .payload = fixture.payload;
    retain(&mut fixture);
    let (_, correspondence) = fixture
        .semantic
        .qualification_correspondences
        .iter()
        .next()
        .unwrap();
    assert!(matches!(correspondence.payload,
        QualificationPayloadIdentity::DomainMembership { semantic_domain, .. } if semantic_domain == identity));

    let mut mismatched = correspondence_fixture();
    let identity = mismatched.program.semantic_domains.intern("Ready<8>");
    let FactPayload::DomainMembership {
        semantic_domain, ..
    } = &mut mismatched
        .semantic
        .facts
        .get_mut(mismatched.source_fact)
        .payload
    else {
        panic!("membership fixture");
    };
    *semantic_domain = identity;
    retain(&mut mismatched);
    assert!(mismatched.semantic.qualification_correspondences.is_empty());
}

#[test]
fn exact_parameter_field_correspondence_is_retained_once() {
    let mut fixture = correspondence_fixture();
    retain(&mut fixture);
    retain(&mut fixture);
    assert_eq!(fixture.semantic.qualification_correspondences.len(), 1);
    let (_, retained) = fixture
        .semantic
        .qualification_correspondences
        .iter()
        .next()
        .expect("exact retained correspondence");
    assert_eq!(retained.source_fact, fixture.source_fact);
    assert_eq!(retained.destination_fact, fixture.destination_fact);
    assert_eq!(retained.source_place, fixture.source_place);
    assert_eq!(
        retained.source_occurrence_place,
        fixture.source_occurrence_place
    );
    assert_eq!(retained.destination_place, fixture.destination_place);
    assert_eq!(retained.formation, fixture.formation);
    assert_eq!(retained.evidence, fixture.evidence);
}

#[test]
fn exact_prior_state_local_is_retained_as_either_endpoint() {
    let mut local_source = correspondence_fixture();
    let local = local_source.exact_local;
    let parameter = match local_source
        .semantic
        .places
        .get(local_source.destination_place)
        .root
    {
        facts::PlaceRoot::Symbol(root) => root,
        _ => unreachable!("symbol root"),
    };
    set_correspondence_roots(&mut local_source, local, parameter);
    retain(&mut local_source);
    assert_eq!(local_source.semantic.qualification_correspondences.len(), 1);

    let mut local_destination = correspondence_fixture();
    let local = local_destination.exact_local;
    let parameter = match local_destination
        .semantic
        .places
        .get(local_destination.source_place)
        .root
    {
        facts::PlaceRoot::Symbol(root) => root,
        _ => unreachable!("symbol root"),
    };
    set_correspondence_roots(&mut local_destination, parameter, local);
    retain(&mut local_destination);
    assert_eq!(
        local_destination
            .semantic
            .qualification_correspondences
            .len(),
        1
    );
}

#[test]
fn state_local_at_or_after_formation_is_not_retained() {
    let mut fixture = correspondence_fixture();
    let local = fixture.exact_local;
    let parameter = match fixture.semantic.places.get(fixture.destination_place).root {
        facts::PlaceRoot::Symbol(root) => root,
        _ => unreachable!("symbol root"),
    };
    set_correspondence_roots(&mut fixture, local, parameter);
    fixture.formation = ProgramPoint::Statement {
        machine_symbol: match fixture.formation {
            ProgramPoint::Statement { machine_symbol, .. } => machine_symbol,
            _ => unreachable!("statement formation"),
        },
        state_symbol: match fixture.formation {
            ProgramPoint::Statement { state_symbol, .. } => state_symbol,
            _ => unreachable!("statement formation"),
        },
        statement_index: 0,
    };
    fixture
        .semantic
        .facts
        .get_mut(fixture.destination_fact)
        .point = fixture.formation;
    retain(&mut fixture);
    assert!(fixture.semantic.qualification_correspondences.is_empty());
}

#[test]
fn nested_literal_fixed_indexes_retain_exact_correspondence() {
    let mut fixture = nested_fixed_array_fixture();
    retain(&mut fixture);
    let retained = fixture
        .semantic
        .qualification_correspondences
        .iter()
        .next()
        .map(|(_, correspondence)| correspondence)
        .expect("nested in-bounds correspondence");
    assert_eq!(retained.source_place, fixture.source_place);
    assert_eq!(
        retained.source_occurrence_place,
        fixture.source_occurrence_place
    );
    assert_eq!(retained.destination_place, fixture.destination_place);
    assert_eq!(retained.formation, fixture.formation);
    assert_eq!(
        retained.payload,
        QualificationPayloadIdentity::from_fact_payload(fixture.payload).expect("payload")
    );
    assert_eq!(retained.evidence, fixture.evidence);
}

#[test]
fn nonliteral_out_of_bounds_runtime_and_wrong_type_indexes_are_not_retained() {
    let mut out_of_bounds = nested_fixed_array_fixture();
    let source_field = out_of_bounds
        .semantic
        .place_segments
        .span(
            out_of_bounds
                .semantic
                .places
                .get(out_of_bounds.source_place)
                .segments,
        )
        .and_then(|segments| segments.first())
        .copied()
        .expect("source field");
    let destination_field = out_of_bounds
        .semantic
        .place_segments
        .span(
            out_of_bounds
                .semantic
                .places
                .get(out_of_bounds.destination_place)
                .segments,
        )
        .and_then(|segments| segments.first())
        .copied()
        .expect("destination field");
    install_correspondence_paths(
        &mut out_of_bounds,
        &[source_field, PlaceSegment::FixedIndex { index: 2 }],
        &[destination_field, PlaceSegment::FixedIndex { index: 0 }],
    );
    retain(&mut out_of_bounds);
    assert!(
        out_of_bounds
            .semantic
            .qualification_correspondences
            .is_empty()
    );

    let mut runtime = nested_fixed_array_fixture();
    install_correspondence_paths(
        &mut runtime,
        &[
            source_field,
            PlaceSegment::Index {
                expression: ExpressionHandle::invalid(),
            },
        ],
        &[destination_field, PlaceSegment::FixedIndex { index: 0 }],
    );
    retain(&mut runtime);
    assert!(runtime.semantic.qualification_correspondences.is_empty());

    let mut range = nested_fixed_array_fixture();
    install_correspondence_paths(
        &mut range,
        &[source_field, PlaceSegment::FixedRange { start: 0, end: 1 }],
        &[destination_field, PlaceSegment::FixedIndex { index: 0 }],
    );
    retain(&mut range);
    assert!(range.semantic.qualification_correspondences.is_empty());

    let mut nonliteral = nested_fixed_array_fixture();
    let unit = nonliteral
        .program
        .type_reference_table
        .insert(typed_trees::types::TypeReferenceNode::Unit);
    let array = nonliteral.program.type_reference_table.insert(
        typed_trees::types::TypeReferenceNode::FixedArray {
            element_type: unit,
            length: typed_trees::types::FixedArrayLength::ConstParameter {
                symbol: nonliteral.local,
                name: typed_trees::name::Identifier::generated("N"),
            },
        },
    );
    set_pair_field_type(&mut nonliteral, array);
    retain(&mut nonliteral);
    assert!(nonliteral.semantic.qualification_correspondences.is_empty());

    let mut wrong_type = nested_fixed_array_fixture();
    let unit = wrong_type
        .program
        .type_reference_table
        .insert(typed_trees::types::TypeReferenceNode::Unit);
    set_pair_field_type(&mut wrong_type, unit);
    retain(&mut wrong_type);
    assert!(wrong_type.semantic.qualification_correspondences.is_empty());
}

#[test]
fn generic_or_label_only_data_traversal_is_not_retained() {
    let mut generic = correspondence_fixture();
    let arguments = generic
        .program
        .type_reference_table
        .insert_type_reference_handles([]);
    let generic_type = generic.program.type_reference_table.insert(
        typed_trees::types::TypeReferenceNode::Generic {
            base_symbol: generic.data_symbol,
            base_name: typed_trees::name::Identifier::generated("Pair"),
            lifetime_arguments: Vec::new(),
            arguments,
        },
    );
    set_formation_parameter_type(&mut generic, generic_type);
    retain(&mut generic);
    assert!(generic.semantic.qualification_correspondences.is_empty());

    let mut label_only = correspondence_fixture();
    let wrong = label_only.program.type_reference_table.insert(
        typed_trees::types::TypeReferenceNode::Named {
            symbol: label_only.wrong_data_symbol,
            name: typed_trees::name::Identifier::generated("Pair"),
        },
    );
    set_formation_parameter_type(&mut label_only, wrong);
    retain(&mut label_only);
    assert!(label_only.semantic.qualification_correspondences.is_empty());
}

#[test]
fn local_or_indexed_correspondence_is_not_retained() {
    let mut local = correspondence_fixture();
    local.semantic.places.get_mut(local.source_place).root = facts::PlaceRoot::Symbol(local.local);
    retain(&mut local);
    assert!(local.semantic.qualification_correspondences.is_empty());

    let mut indexed = correspondence_fixture();
    let root = indexed.semantic.places.get(indexed.source_place).root;
    let indexed_place = indexed.semantic.append_place(facts::Place {
        root,
        segments: HandleSpan::empty(),
    });
    indexed
        .semantic
        .push_place_segment(indexed_place, PlaceSegment::FixedIndex { index: 0 });
    indexed.source_place = indexed_place;
    indexed.semantic.facts.get_mut(indexed.source_fact).place = FactPlace::Place(indexed_place);
    retain(&mut indexed);
    assert!(indexed.semantic.qualification_correspondences.is_empty());

    let mut mismatched_occurrence = correspondence_fixture();
    mismatched_occurrence.source_occurrence_place = mismatched_occurrence.destination_place;
    assert!(!mismatched_occurrence.semantic.places_equal(
        mismatched_occurrence.source_place,
        mismatched_occurrence.source_occurrence_place
    ));
    retain(&mut mismatched_occurrence);
    assert!(
        mismatched_occurrence
            .semantic
            .qualification_correspondences
            .is_empty()
    );
}

#[test]
fn foreign_machine_or_sibling_state_parameter_correspondence_is_not_retained() {
    for sibling_state in [false, true] {
        for endpoint in 0..3 {
            let mut fixture = correspondence_fixture();
            let foreign_root = if sibling_state {
                fixture.sibling_state_parameter
            } else {
                fixture.foreign_parameter
            };
            match endpoint {
                0 => {
                    fixture.semantic.places.get_mut(fixture.source_place).root =
                        facts::PlaceRoot::Symbol(foreign_root);
                }
                1 => {
                    let occurrence_place = fixture.semantic.append_symbol_place(foreign_root);
                    fixture.source_occurrence_place = occurrence_place;
                }
                2 => {
                    fixture
                        .semantic
                        .places
                        .get_mut(fixture.destination_place)
                        .root = facts::PlaceRoot::Symbol(foreign_root);
                }
                _ => unreachable!(),
            }
            retain(&mut fixture);
            assert!(fixture.semantic.qualification_correspondences.is_empty());
        }
    }
}

#[test]
fn foreign_machine_or_sibling_state_local_correspondence_is_not_retained() {
    for sibling_state in [false, true] {
        for endpoint in 0..2 {
            let mut fixture = correspondence_fixture();
            let excluded_root = if sibling_state {
                fixture.sibling_state_local
            } else {
                fixture.foreign_local
            };
            let parameter = match fixture.semantic.places.get(fixture.destination_place).root {
                facts::PlaceRoot::Symbol(root) => root,
                _ => unreachable!("symbol root"),
            };
            if endpoint == 0 {
                set_correspondence_roots(&mut fixture, excluded_root, parameter);
            } else {
                set_correspondence_roots(&mut fixture, parameter, excluded_root);
            }
            retain(&mut fixture);
            assert!(fixture.semantic.qualification_correspondences.is_empty());
        }
    }
}
