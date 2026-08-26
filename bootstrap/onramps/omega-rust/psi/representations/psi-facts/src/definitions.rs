use psi_arena::{Handle, HandleSpan};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;

use crate::{
    DomainDefinitionFactDependency, DomainDefinitionFactRecord, Fact, FactOrigin, FactPayload,
    FactPlace, FactPlan, PlaceHandle, ProgramPoint,
};

pub fn build_definition_fact_plan(program: &TypedTrees) -> FactPlan {
    let mut facts = FactPlan::with_capacity(
        estimated_definition_fact_capacity(program),
        estimated_definition_context_capacity(program),
    );

    append_domain_definition_facts(program, &mut facts);

    facts
}

fn estimated_definition_fact_capacity(program: &TypedTrees) -> usize {
    let domain_facts = program
        .domain_definitions()
        .iter()
        .map(|domain| program.proof_facts(domain).len())
        .sum::<usize>();
    domain_facts
}

fn estimated_definition_context_capacity(program: &TypedTrees) -> usize {
    program.domain_definitions().len()
}

fn append_domain_definition_facts(program: &TypedTrees, facts: &mut FactPlan) {
    for domain in program.domain_definitions() {
        let mut refs = HandleSpan::empty();
        for fact_handle in proof_fact_handles(domain.facts) {
            let proof_fact = program.proof_facts.get(fact_handle);
            let dependencies = domain_fact_dependency_places(program, facts, domain, proof_fact);
            let place = append_proof_fact_place(program, facts, proof_fact);
            let payload = match proof_fact {
                ProofFact::Expression(expression) => FactPayload::BooleanExpression(*expression),
                ProofFact::Membership(membership) => FactPayload::DomainMembership {
                    value: membership.value,
                    domain: membership.domain,
                    domain_symbol: membership.domain_symbol,
                },
                ProofFact::Proposition(application) => FactPayload::PropositionApplication {
                    fact: fact_handle,
                    proposition: application.proposition,
                },
            };
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

fn domain_fact_dependency_places(
    program: &TypedTrees,
    facts: &mut FactPlan,
    domain: &psi_typed_trees::domain::DomainDefinition,
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

fn append_domain_expression_dependency_places(
    program: &TypedTrees,
    facts: &mut FactPlan,
    domain_target: psi_typed_trees::types::TypeReferenceHandle,
    expression: psi_typed_trees::expression::ExpressionHandle,
    dependencies: &mut Vec<DomainDefinitionFactDependency>,
) {
    use psi_typed_trees::expression::ExpressionNode;

    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member)
            if matches!(
                program.expression_table.expression(member.receiver),
                ExpressionNode::Call(_)
            ) =>
        {
            // A fact-call projection has no materialized result place. Its
            // validity/revision scope is exactly the transitive union of the
            // call's input occurrences.
            append_domain_expression_dependency_places(
                program,
                facts,
                domain_target,
                member.receiver,
                dependencies,
            );
        }
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            let place = append_domain_expression_place(program, facts, domain_target, expression);
            dependencies.push(DomainDefinitionFactDependency { expression, place });
        }
        ExpressionNode::Borrow(inner) => append_domain_expression_dependency_places(
            program,
            facts,
            domain_target,
            inner.target,
            dependencies,
        ),
        ExpressionNode::Atomic(atomic) => {
            append_domain_expression_dependency_places(
                program,
                facts,
                domain_target,
                atomic.value,
                dependencies,
            );
            append_domain_expression_dependency_places(
                program,
                facts,
                domain_target,
                atomic.result,
                dependencies,
            );
        }
        ExpressionNode::Binary(binary) => {
            append_domain_expression_dependency_places(
                program,
                facts,
                domain_target,
                binary.left,
                dependencies,
            );
            append_domain_expression_dependency_places(
                program,
                facts,
                domain_target,
                binary.right,
                dependencies,
            );
        }
        ExpressionNode::Cast(cast) => append_domain_expression_dependency_places(
            program,
            facts,
            domain_target,
            cast.value,
            dependencies,
        ),
        ExpressionNode::Call(call) => {
            append_domain_expression_dependency_places(
                program,
                facts,
                domain_target,
                call.receiver,
                dependencies,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                append_domain_expression_dependency_places(
                    program,
                    facts,
                    domain_target,
                    *argument,
                    dependencies,
                );
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                append_domain_expression_dependency_places(
                    program,
                    facts,
                    domain_target,
                    *value,
                    dependencies,
                );
            }
        }
        ExpressionNode::Range(range) => {
            append_domain_expression_dependency_places(
                program,
                facts,
                domain_target,
                range.start,
                dependencies,
            );
            append_domain_expression_dependency_places(
                program,
                facts,
                domain_target,
                range.end,
                dependencies,
            );
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                append_domain_expression_dependency_places(
                    program,
                    facts,
                    domain_target,
                    field.value,
                    dependencies,
                );
            }
        }
        ExpressionNode::Unary(unary) => append_domain_expression_dependency_places(
            program,
            facts,
            domain_target,
            unary.operand,
            dependencies,
        ),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn append_domain_expression_place(
    program: &TypedTrees,
    facts: &mut FactPlan,
    domain_target: psi_typed_trees::types::TypeReferenceHandle,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> PlaceHandle {
    use psi_typed_trees::expression::ExpressionNode;

    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            append_domain_expression_place(program, facts, domain_target, inner.target)
        }
        ExpressionNode::Name(path) => {
            let [name] = program.expression_table.name_path_members(path.members) else {
                return facts.append_place_from_expression(program, expression);
            };
            if !path.head_symbol.is_valid() && !path.symbol.is_valid() && name.as_str() == "self" {
                facts.append_expression_place(expression)
            } else {
                facts.append_place_from_expression(program, expression)
            }
        }
        ExpressionNode::Member(member) => {
            let place =
                append_domain_expression_place(program, facts, domain_target, member.receiver);
            let symbol = crate::effective_member_symbol(program, member.receiver, member);
            let symbol = if symbol.is_valid() {
                symbol
            } else {
                crate::resolve_place_member_symbol(program, facts, place, member.member.as_str())
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
                    .unwrap_or_else(psi_symbols::SymbolHandle::invalid)
            };
            if let Some(variant) = crate::payload_variant_for_field(program, symbol) {
                facts.push_place_segment(place, crate::PlaceSegment::Case { variant });
            }
            facts.push_place_segment(place, crate::PlaceSegment::Field { symbol });
            place
        }
        ExpressionNode::Indexed(indexed) => {
            let place =
                append_domain_expression_place(program, facts, domain_target, indexed.collection);
            let segment = program
                .expression_table
                .constant_integer_value(indexed.index)
                .and_then(|value| usize::try_from(value).ok())
                .map(|index| crate::PlaceSegment::FixedIndex { index })
                .unwrap_or(crate::PlaceSegment::Index {
                    expression: indexed.index,
                });
            facts.push_place_segment(place, segment);
            place
        }
        _ => facts.append_place_from_expression(program, expression),
    }
}

fn domain_target_member_symbol(
    program: &TypedTrees,
    target: psi_typed_trees::types::TypeReferenceHandle,
    member_name: &str,
) -> Option<psi_symbols::SymbolHandle> {
    use psi_typed_trees::data::DataMember;
    use psi_typed_trees::types::TypeReferenceNode;

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
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == target_symbol)?;
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
    program: &TypedTrees,
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

fn proof_fact_handles(facts: HandleSpan<ProofFact>) -> impl Iterator<Item = Handle<ProofFact>> {
    (0..facts.count()).map(move |offset| {
        Handle::from_parts(
            facts
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("proof fact handle index overflow"),
            facts.start().generation(),
        )
    })
}
