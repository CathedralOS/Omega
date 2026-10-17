use arena::{Handle, HandleSpan};
use typed_trees::data::DataMember;
use typed_trees::domain::ProofFact;

use facts::{
    DataDefinitionFactDependency, DataDefinitionFactRecord, DomainDefinitionFactDependency,
    DomainDefinitionFactRecord, Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceHandle,
    ProgramPoint,
};

pub fn build_definition_fact_plan(program: &dyn crate::CheckingInput) -> FactPlan {
    let mut facts = FactPlan::with_capacity(
        estimated_definition_fact_capacity(program),
        estimated_definition_context_capacity(program),
    );

    append_domain_definition_facts(program, &mut facts);
    append_data_definition_facts(program, &mut facts);

    facts
}

fn estimated_definition_fact_capacity(program: &dyn crate::CheckingInput) -> usize {
    let domain_facts = program
        .domain_definitions()
        .iter()
        .map(|domain| program.proof_facts(domain).len())
        .sum::<usize>();
    let data_facts = program
        .data_definitions()
        .iter()
        .map(|data| program.proof_facts.span_or_empty(data.where_facts).len())
        .sum::<usize>();
    domain_facts + data_facts
}

fn estimated_definition_context_capacity(program: &dyn crate::CheckingInput) -> usize {
    program.domain_definitions().len() + program.data_definitions().len()
}

fn append_domain_definition_facts(program: &dyn crate::CheckingInput, facts: &mut FactPlan) {
    for domain in program.domain_definitions() {
        let mut refs = HandleSpan::empty();
        for fact_handle in proof_fact_handles(domain.facts) {
            let proof_fact = program.proof_facts.get(fact_handle);
            let dependencies = domain_fact_dependency_places(program, facts, domain, proof_fact);
            let place = append_proof_fact_place(program, facts, proof_fact);
            let payload = fact_payload(proof_fact, fact_handle);
            let fact = facts.append_fact(Fact {
                place: FactPlace::Place(place),
                point: ProgramPoint::Definition {
                    symbol: domain.symbol,
                },
                origin: FactOrigin::DomainDefinition {
                    domain_symbol: domain.symbol,
                },
                evidence: Default::default(),
                payload,
            });
            facts
                .domain_definition_facts
                .append(DomainDefinitionFactRecord {
                    domain_symbol: domain.symbol,
                    fact: fact_handle,
                    semantic_fact: fact,
                    dependencies,
                });
            facts.append_ref(&mut refs, fact);
        }
        facts.append_context(
            ProgramPoint::Definition {
                symbol: domain.symbol,
            },
            refs,
        );
        facts.append_symbol_set(domain.symbol, refs);
    }
}

fn append_data_definition_facts(program: &dyn crate::CheckingInput, facts: &mut FactPlan) {
    for data in program.data_definitions() {
        let mut refs = HandleSpan::empty();
        for fact_handle in proof_fact_handles(data.where_facts) {
            let proof_fact = program.proof_facts.get(fact_handle);
            let dependencies = data_fact_dependency_places(program, facts, data, proof_fact);
            let place = append_data_proof_fact_place(program, facts, data, proof_fact);
            let payload = fact_payload(proof_fact, fact_handle);
            let semantic_fact = facts.append_fact(Fact {
                place: FactPlace::Place(place),
                point: ProgramPoint::Definition {
                    symbol: data.symbol,
                },
                origin: FactOrigin::DataDefinition {
                    data_symbol: data.symbol,
                },
                evidence: Default::default(),
                payload,
            });
            facts
                .data_definition_facts
                .append(DataDefinitionFactRecord {
                    data_symbol: data.symbol,
                    fact: fact_handle,
                    semantic_fact,
                    dependencies,
                });
            facts.append_ref(&mut refs, semantic_fact);
        }
        facts.append_context(
            ProgramPoint::Definition {
                symbol: data.symbol,
            },
            refs,
        );
        facts.append_symbol_set(data.symbol, refs);
    }
}

fn data_fact_dependency_places(
    program: &dyn crate::CheckingInput,
    facts: &mut FactPlan,
    data: &typed_trees::data::DataDefinition,
    proof_fact: &ProofFact,
) -> Vec<DataDefinitionFactDependency> {
    let mut dependencies = Vec::new();
    match proof_fact {
        ProofFact::Expression(expression) => append_data_expression_dependency_places(
            program,
            facts,
            data,
            *expression,
            &mut dependencies,
        ),
        ProofFact::Membership(membership) => append_data_expression_dependency_places(
            program,
            facts,
            data,
            membership.value,
            &mut dependencies,
        ),
        ProofFact::Proposition(application) => {
            for argument in program
                .expression_table
                .expression_handles(application.arguments)
            {
                append_data_expression_dependency_places(
                    program,
                    facts,
                    data,
                    *argument,
                    &mut dependencies,
                );
            }
        }
    }
    dependencies
}

/// One dependency walk over `expression`: every Name/Member/Indexed leaf
/// becomes a dependency `leaf` builds (it also owns the place append into
/// `facts`); every other node recurses into its operands.
fn append_expression_dependency_places<D>(
    program: &dyn crate::CheckingInput,
    facts: &mut FactPlan,
    expression: typed_trees::expression::ExpressionHandle,
    dependencies: &mut Vec<D>,
    leaf: impl Fn(&mut FactPlan, typed_trees::expression::ExpressionHandle) -> D + Copy,
) {
    use typed_trees::expression::ExpressionNode;

    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            for child in crate::expression_traversal::match_children(program, *dispatch) {
                append_expression_dependency_places(program, facts, child, dependencies, leaf);
            }
        }
        ExpressionNode::Member(member)
            if matches!(
                program.expression_table.expression(member.receiver),
                ExpressionNode::Call(_)
            ) =>
        {
            // A fact-call projection has no materialized result place. Its
            // validity/revision scope is exactly the transitive union of the
            // call's input occurrences.
            append_expression_dependency_places(
                program,
                facts,
                member.receiver,
                dependencies,
                leaf,
            );
        }
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            dependencies.push(leaf(facts, expression));
        }
        ExpressionNode::Borrow(inner) => {
            append_expression_dependency_places(program, facts, inner.target, dependencies, leaf)
        }
        ExpressionNode::Atomic(atomic) => {
            append_expression_dependency_places(program, facts, atomic.value, dependencies, leaf);
            append_expression_dependency_places(program, facts, atomic.result, dependencies, leaf);
        }
        ExpressionNode::Binary(binary) => {
            append_expression_dependency_places(program, facts, binary.left, dependencies, leaf);
            append_expression_dependency_places(program, facts, binary.right, dependencies, leaf);
        }
        ExpressionNode::Cast(cast) => {
            append_expression_dependency_places(program, facts, cast.value, dependencies, leaf)
        }
        ExpressionNode::Call(call) => {
            append_expression_dependency_places(program, facts, call.receiver, dependencies, leaf);
            for argument in program.expression_table.expression_handles(call.arguments) {
                append_expression_dependency_places(program, facts, *argument, dependencies, leaf);
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                append_expression_dependency_places(program, facts, *value, dependencies, leaf);
            }
        }
        ExpressionNode::Range(range) => {
            append_expression_dependency_places(program, facts, range.start, dependencies, leaf);
            append_expression_dependency_places(program, facts, range.end, dependencies, leaf);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                append_expression_dependency_places(
                    program,
                    facts,
                    field.value,
                    dependencies,
                    leaf,
                );
            }
        }
        ExpressionNode::Unary(unary) => {
            append_expression_dependency_places(program, facts, unary.operand, dependencies, leaf)
        }
        typed_trees::spelled_literal_expression_pattern!() => {}
    }
}

fn append_data_expression_dependency_places(
    program: &dyn crate::CheckingInput,
    facts: &mut FactPlan,
    data: &typed_trees::data::DataDefinition,
    expression: typed_trees::expression::ExpressionHandle,
    dependencies: &mut Vec<DataDefinitionFactDependency>,
) {
    append_expression_dependency_places(
        program,
        facts,
        expression,
        dependencies,
        |facts, expression| DataDefinitionFactDependency {
            expression,
            place: append_data_expression_place(program, facts, data, expression),
        },
    );
}

fn append_domain_expression_dependency_places(
    program: &dyn crate::CheckingInput,
    facts: &mut FactPlan,
    domain_target: typed_trees::types::TypeReferenceHandle,
    expression: typed_trees::expression::ExpressionHandle,
    dependencies: &mut Vec<DomainDefinitionFactDependency>,
) {
    append_expression_dependency_places(
        program,
        facts,
        expression,
        dependencies,
        |facts, expression| DomainDefinitionFactDependency {
            expression,
            place: append_domain_expression_place(program, facts, domain_target, expression),
        },
    );
}

fn append_data_expression_place(
    program: &dyn crate::CheckingInput,
    facts: &mut FactPlan,
    data: &typed_trees::data::DataDefinition,
    expression: typed_trees::expression::ExpressionHandle,
) -> PlaceHandle {
    use typed_trees::expression::ExpressionNode;

    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            append_data_expression_place(program, facts, data, inner.target)
        }
        ExpressionNode::Name(path) => {
            let member_names = program.expression_table.name_path_members(path.members);
            let member_symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            if member_names.is_empty() {
                return facts.append_place_from_expression(program, expression);
            }
            let resolved_root = path
                .head_symbol
                .is_valid()
                .then_some(path.head_symbol)
                .or_else(|| {
                    member_symbols
                        .first()
                        .copied()
                        .filter(|symbol| symbol.is_valid())
                })
                .or_else(|| {
                    (member_names.len() == 1 && path.symbol.is_valid()).then_some(path.symbol)
                });
            let first_field =
                resolved_root.and_then(|symbol| data_field_symbol(program, data, symbol));
            let Some(first_field) = first_field else {
                return facts.append_place_from_expression(program, expression);
            };

            let place = facts.append_symbol_place(data.symbol);
            facts.push_place_segment(
                place,
                facts::PlaceSegment::Field {
                    symbol: first_field,
                },
            );
            for (offset, member_name) in member_names.iter().enumerate().skip(1) {
                let exact_symbol = member_symbols
                    .get(offset)
                    .copied()
                    .filter(|symbol| symbol.is_valid())
                    .or_else(|| {
                        (offset + 1 == member_names.len() && path.symbol.is_valid())
                            .then_some(path.symbol)
                    });
                let symbol = exact_symbol
                    .or_else(|| {
                        facts::resolve_place_member_symbol(
                            program,
                            facts,
                            place,
                            member_name.as_str(),
                        )
                    })
                    .unwrap_or_else(symbols::SymbolHandle::invalid);
                if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
                    facts.push_place_segment(place, facts::PlaceSegment::Case { variant });
                }
                facts.push_place_segment(place, facts::PlaceSegment::Field { symbol });
            }
            place
        }
        ExpressionNode::Member(member) => {
            let place = append_data_expression_place(program, facts, data, member.receiver);
            let symbol = facts::effective_member_symbol(program, member.receiver, member);
            let symbol = if symbol.is_valid() {
                symbol
            } else {
                facts::resolve_place_member_symbol(program, facts, place, member.member.as_str())
                    .unwrap_or_else(symbols::SymbolHandle::invalid)
            };
            push_member_field_segments(program, facts, place, symbol);
            place
        }
        ExpressionNode::Indexed(indexed) => {
            let place = append_data_expression_place(program, facts, data, indexed.collection);
            push_index_place_segment(program, facts, indexed, place);
            place
        }
        _ => facts.append_place_from_expression(program, expression),
    }
}

/// An index expression's place segment. A selector fixed to one ordinal,
/// including a proved equality and not only a literal, becomes that slot.
fn push_index_place_segment(
    program: &dyn crate::CheckingInput,
    facts: &mut FactPlan,
    indexed: &typed_trees::expression::TableIndexedExpression,
    place: PlaceHandle,
) {
    facts.push_place_segment(
        place,
        crate::value_custody::places::element_place_segment(program, indexed.index),
    );
}

/// Push the resolved member's case-variant (when payload-carried) and field
/// segments onto `place`.
fn push_member_field_segments(
    program: &dyn crate::CheckingInput,
    facts: &mut FactPlan,
    place: PlaceHandle,
    symbol: symbols::SymbolHandle,
) {
    if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
        facts.push_place_segment(place, facts::PlaceSegment::Case { variant });
    }
    facts.push_place_segment(place, facts::PlaceSegment::Field { symbol });
}

fn data_field_symbol(
    program: &dyn crate::CheckingInput,
    data: &typed_trees::data::DataDefinition,
    symbol: symbols::SymbolHandle,
) -> Option<symbols::SymbolHandle> {
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.symbol == symbol => Some(field.symbol),
            DataMember::Field(_) | DataMember::Variant(_) => None,
        })
}

fn domain_fact_dependency_places(
    program: &dyn crate::CheckingInput,
    facts: &mut FactPlan,
    domain: &typed_trees::domain::DomainDefinition,
    proof_fact: &ProofFact,
) -> Vec<DomainDefinitionFactDependency> {
    let mut dependencies = Vec::new();
    match proof_fact {
        ProofFact::Expression(expression) => append_domain_expression_dependency_places(
            program,
            facts,
            domain.target_type,
            *expression,
            &mut dependencies,
        ),
        ProofFact::Membership(membership) => append_domain_expression_dependency_places(
            program,
            facts,
            domain.target_type,
            membership.value,
            &mut dependencies,
        ),
        ProofFact::Proposition(application) => {
            for argument in program
                .expression_table
                .expression_handles(application.arguments)
            {
                append_domain_expression_dependency_places(
                    program,
                    facts,
                    domain.target_type,
                    *argument,
                    &mut dependencies,
                );
            }
        }
    }
    dependencies
}

fn append_domain_expression_place(
    program: &dyn crate::CheckingInput,
    facts: &mut FactPlan,
    domain_target: typed_trees::types::TypeReferenceHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> PlaceHandle {
    use typed_trees::expression::ExpressionNode;

    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            append_domain_expression_place(program, facts, domain_target, inner.target)
        }
        ExpressionNode::Member(member) => {
            let place =
                append_domain_expression_place(program, facts, domain_target, member.receiver);
            let symbol = facts::effective_member_symbol(program, member.receiver, member);
            let symbol = if symbol.is_valid() {
                symbol
            } else {
                facts::resolve_place_member_symbol(program, facts, place, member.member.as_str())
                    .or_else(|| {
                        facts
                            .places
                            .get(place)
                            .segments
                            .is_empty()
                            .then(|| {
                                domain_target_member_symbol(
                                    program,
                                    domain_target,
                                    member.member.as_str(),
                                )
                            })
                            .flatten()
                    })
                    .unwrap_or_else(symbols::SymbolHandle::invalid)
            };
            push_member_field_segments(program, facts, place, symbol);
            place
        }
        ExpressionNode::Indexed(indexed) => {
            let place =
                append_domain_expression_place(program, facts, domain_target, indexed.collection);
            push_index_place_segment(program, facts, indexed, place);
            place
        }
        _ => facts.append_place_from_expression(program, expression),
    }
}

fn domain_target_member_symbol(
    program: &dyn crate::CheckingInput,
    target: typed_trees::types::TypeReferenceHandle,
    member_name: &str,
) -> Option<symbols::SymbolHandle> {
    use typed_trees::data::DataMember;
    use typed_trees::types::TypeReferenceNode;

    let target_symbol = match program.type_reference_table.type_reference(target) {
        TypeReferenceNode::Reference { referee, .. } => {
            return domain_target_member_symbol(program, *referee, member_name);
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            return domain_target_member_symbol(program, *base_type, member_name);
        }
        TypeReferenceNode::Generic { base_symbol, .. }
        | TypeReferenceNode::Named {
            symbol: base_symbol,
            ..
        } => *base_symbol,
        _ => return None,
    };
    let data =
        crate::declarations::data_queries::data_definition_by_symbol(program, target_symbol)?;
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == member_name => Some(field.symbol),
            DataMember::Variant(variant) if variant.name.as_str() == member_name => {
                Some(variant.symbol)
            }
            DataMember::Variant(_) | DataMember::Field(_) => None,
        })
}

fn append_proof_fact_place(
    program: &dyn crate::CheckingInput,
    facts: &mut FactPlan,
    proof_fact: &ProofFact,
) -> PlaceHandle {
    match proof_fact {
        ProofFact::Expression(expression) => {
            facts.append_place_from_expression(program, *expression)
        }
        ProofFact::Membership(membership) => {
            facts.append_place_from_expression(program, membership.value)
        }
        ProofFact::Proposition(application) => facts.append_symbol_place(application.proposition),
    }
}

fn append_data_proof_fact_place(
    program: &dyn crate::CheckingInput,
    facts: &mut FactPlan,
    data: &typed_trees::data::DataDefinition,
    proof_fact: &ProofFact,
) -> PlaceHandle {
    match proof_fact {
        ProofFact::Expression(expression) => {
            append_data_expression_place(program, facts, data, *expression)
        }
        ProofFact::Membership(membership) => {
            append_data_expression_place(program, facts, data, membership.value)
        }
        ProofFact::Proposition(application) => facts.append_symbol_place(application.proposition),
    }
}

fn proof_fact_handles(facts: HandleSpan<ProofFact>) -> impl Iterator<Item = Handle<ProofFact>> {
    facts.handles()
}

/// The payload every recorded definition fact carries: the proof term's
/// expression, membership, or proposition application verbatim.
fn fact_payload(proof_fact: &ProofFact, fact_handle: Handle<ProofFact>) -> FactPayload {
    match proof_fact {
        ProofFact::Expression(expression) => FactPayload::BooleanExpression(*expression),
        ProofFact::Membership(membership) => FactPayload::DomainMembership {
            value: membership.value,
            domain: membership.domain,
            domain_symbol: membership.domain_symbol,
            semantic_domain: membership.semantic_domain,
        },
        ProofFact::Proposition(application) => FactPayload::PropositionApplication {
            fact: fact_handle,
            proposition: application.proposition,
        },
    }
}
