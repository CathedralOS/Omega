use psi_arena::{Arena, Handle, HandleSpan};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::types::TypeReferenceHandle;

use crate::{
    BooleanFact, DataDefinitionFactRecord, DomainDefinitionFactRecord, DomainMembershipFact, Fact,
    FactContext, FactContextHandle, FactContextView, FactHandle, FactPayload, FactPlace, FactRef,
    InstantiatedExpression, Place, PlaceHandle, PlaceRoot, PlaceSegment, ProgramPoint,
    SymbolFactSet, TypeConstraintFact, effective_member_symbol, resolve_place_member_symbol,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactPlan {
    pub places: Arena<Place>,
    pub place_segments: Arena<PlaceSegment>,
    pub facts: Arena<Fact>,
    pub domain_definition_facts: Arena<DomainDefinitionFactRecord>,
    pub data_definition_facts: Arena<DataDefinitionFactRecord>,
    pub instantiated_expressions: Arena<InstantiatedExpression>,
    pub refs: Arena<FactRef>,
    pub contexts: Arena<FactContext>,
    pub symbol_sets: Arena<SymbolFactSet>,
}

impl FactPlan {
    pub fn with_capacity(fact_capacity: usize, context_capacity: usize) -> Self {
        Self {
            places: Arena::with_capacity(fact_capacity),
            place_segments: Arena::with_capacity(fact_capacity),
            facts: Arena::with_capacity(fact_capacity),
            domain_definition_facts: Arena::with_capacity(fact_capacity),
            data_definition_facts: Arena::with_capacity(fact_capacity),
            instantiated_expressions: Arena::with_capacity(fact_capacity),
            refs: Arena::with_capacity(fact_capacity),
            contexts: Arena::with_capacity(context_capacity),
            symbol_sets: Arena::with_capacity(fact_capacity),
        }
    }

    pub fn append_fact(&mut self, fact: Fact) -> FactHandle {
        self.facts.append(fact)
    }

    pub fn append_instantiated_expression(
        &mut self,
        label: String,
    ) -> Handle<InstantiatedExpression> {
        self.instantiated_expressions
            .append(InstantiatedExpression { label })
    }

    pub fn boolean_fact_label(&self, program: &TypedTrees, fact: &Fact) -> Option<String> {
        match fact.payload {
            FactPayload::BooleanExpression(expression) => {
                Some(program.expression_table.display_name(expression))
            }
            FactPayload::ContractBooleanExpression {
                expression,
                instantiated,
                ..
            } => {
                if instantiated.is_valid() {
                    Some(
                        self.instantiated_expressions
                            .get(instantiated)
                            .label
                            .clone(),
                    )
                } else {
                    Some(program.expression_table.display_name(expression))
                }
            }
            _ => None,
        }
    }

    pub fn proposition_fact_label(&self, program: &TypedTrees, fact: &Fact) -> Option<String> {
        let (proof_fact, instantiated) = match fact.payload {
            FactPayload::PropositionApplication { fact, .. } => (fact, Handle::invalid()),
            FactPayload::ContractPropositionApplication {
                fact, instantiated, ..
            } => (fact, instantiated),
            _ => return None,
        };
        if instantiated.is_valid() {
            return Some(
                self.instantiated_expressions
                    .get(instantiated)
                    .label
                    .clone(),
            );
        }
        let psi_typed_trees::domain::ProofFact::Proposition(application) =
            program.proof_facts.get(proof_fact)
        else {
            return None;
        };
        program
            .normalize_proposition_application(application)
            .map(|formula| formula.identity_label())
    }

    pub fn append_place(&mut self, place: Place) -> PlaceHandle {
        self.places.append(place)
    }

    pub fn append_symbol_place(&mut self, symbol: SymbolHandle) -> PlaceHandle {
        self.append_place(Place {
            root: PlaceRoot::Symbol(symbol),
            segments: HandleSpan::empty(),
        })
    }

    pub fn append_expression_place(&mut self, expression: ExpressionHandle) -> PlaceHandle {
        self.append_place(Place {
            root: PlaceRoot::Expression(expression),
            segments: HandleSpan::empty(),
        })
    }

    pub fn append_place_from_expression(
        &mut self,
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> PlaceHandle {
        if !expression.is_valid() {
            return self.append_place(Place {
                root: PlaceRoot::Unknown,
                segments: HandleSpan::empty(),
            });
        }

        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(inner) => {
                self.append_place_from_expression(program, inner.target)
            }
            ExpressionNode::Name(path) => {
                let root = if path.head_symbol.is_valid() {
                    path.head_symbol
                } else {
                    path.symbol
                };
                if !root.is_valid() {
                    return self.append_expression_place(expression);
                }
                let place = self.append_symbol_place(root);
                let member_symbols = program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols);
                let member_names = program.expression_table.name_path_members(path.members);
                for (offset, member_name) in member_names.iter().skip(1).enumerate() {
                    let member_symbol = member_symbols
                        .get(offset + 1)
                        .copied()
                        .filter(|symbol| symbol.is_valid())
                        .or_else(|| {
                            resolve_place_member_symbol(program, self, place, member_name.as_str())
                        })
                        .unwrap_or_else(SymbolHandle::invalid);
                    self.push_field_place_segment(program, place, member_symbol);
                }
                place
            }
            ExpressionNode::Member(member) => {
                let place = self.append_place_from_expression(program, member.receiver);
                let symbol = {
                    let symbol = effective_member_symbol(program, member.receiver, member);
                    if symbol.is_valid() {
                        symbol
                    } else {
                        resolve_place_member_symbol(program, self, place, member.member.as_str())
                            .unwrap_or_else(SymbolHandle::invalid)
                    }
                };
                self.push_field_place_segment(program, place, symbol);
                place
            }
            ExpressionNode::Indexed(indexed) => {
                let place = self.append_place_from_expression(program, indexed.collection);
                let segment = program
                    .expression_table
                    .constant_integer_value(indexed.index)
                    .and_then(|value| usize::try_from(value).ok())
                    .map(|index| PlaceSegment::FixedIndex { index })
                    .unwrap_or(PlaceSegment::Index {
                        expression: indexed.index,
                    });
                self.push_place_segment(place, segment);
                place
            }
            _ => self.append_expression_place(expression),
        }
    }

    pub fn append_type_reference_place(
        &mut self,
        type_reference: TypeReferenceHandle,
    ) -> PlaceHandle {
        self.append_place(Place {
            root: PlaceRoot::TypeReference(type_reference),
            segments: HandleSpan::empty(),
        })
    }

    pub fn push_place_segment(&mut self, place: PlaceHandle, segment: PlaceSegment) {
        let segment = self.place_segments.append(segment);
        self.places.get_mut(place).segments.push_contiguous(segment);
    }

    fn push_field_place_segment(
        &mut self,
        program: &psi_typed_trees::TypedTrees,
        place: PlaceHandle,
        symbol: SymbolHandle,
    ) {
        if let Some(variant) = crate::payload_variant_for_field(program, symbol) {
            self.push_place_segment(place, PlaceSegment::Case { variant });
        }
        self.push_place_segment(place, PlaceSegment::Field { symbol });
    }

    pub fn append_fact_context(&mut self, fact: Fact) -> FactHandle {
        let point = fact.point;
        let fact = self.append_fact(fact);
        let mut refs = HandleSpan::empty();
        self.append_ref(&mut refs, fact);
        self.append_context(point, refs);
        fact
    }

    pub fn append_ref(&mut self, refs: &mut HandleSpan<FactRef>, fact: FactHandle) {
        self.refs.append_to_span(refs, FactRef { fact });
    }

    pub fn append_context(
        &mut self,
        point: ProgramPoint,
        facts: HandleSpan<FactRef>,
    ) -> FactContextHandle {
        self.contexts.append(FactContext { point, facts })
    }

    pub fn append_symbol_set(
        &mut self,
        symbol: SymbolHandle,
        facts: HandleSpan<FactRef>,
    ) -> Handle<SymbolFactSet> {
        self.symbol_sets.append(SymbolFactSet { symbol, facts })
    }

    pub fn facts_for_symbol(&self, symbol: SymbolHandle) -> impl Iterator<Item = &Fact> {
        self.symbol_sets
            .iter()
            .filter(move |(_, set)| set.symbol == symbol)
            .flat_map(move |(_, set)| {
                self.refs
                    .span_or_empty(set.facts)
                    .iter()
                    .map(move |reference| self.facts.get(reference.fact))
            })
    }

    pub fn context_facts(&self, context: &FactContext) -> impl Iterator<Item = &Fact> {
        self.refs
            .span_or_empty(context.facts)
            .iter()
            .map(move |reference| self.facts.get(reference.fact))
    }

    pub fn context_view(&self, context: &FactContext) -> FactContextView<'_> {
        FactContextView {
            plan: self,
            point: context.point,
            facts: context.facts,
        }
    }

    pub fn contexts_at_point(
        &self,
        point: ProgramPoint,
    ) -> impl Iterator<Item = FactContextView<'_>> {
        self.contexts
            .iter()
            .filter(move |(_, context)| context.point == point)
            .map(move |(_, context)| self.context_view(context))
    }

    pub fn context_handles_at_point(
        &self,
        point: ProgramPoint,
    ) -> impl Iterator<Item = FactContextHandle> + '_ {
        self.contexts
            .iter()
            .filter(move |(_, context)| context.point == point)
            .map(|(handle, _)| handle)
    }

    pub fn facts_at_point(&self, point: ProgramPoint) -> impl Iterator<Item = &Fact> {
        self.contexts_at_point(point)
            .flat_map(|context| context.facts())
    }

    pub fn proves_domain_membership_at_point(
        &self,
        point: ProgramPoint,
        value: ExpressionHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        self.contexts_at_point(point)
            .any(|context| context.proves_domain_membership(value, domain_symbol))
    }

    pub fn proves_place_domain_membership_at_point(
        &self,
        point: ProgramPoint,
        place: PlaceHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        self.contexts_at_point(point)
            .any(|context| context.proves_place_domain_membership(place, domain_symbol))
    }

    pub fn symbol_references_domain(
        &self,
        symbol: SymbolHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        self.domain_memberships_for_symbol(symbol)
            .any(|fact| self.domain_implies(fact.domain_symbol, domain_symbol))
    }

    pub fn domain_implies(
        &self,
        source_domain_symbol: SymbolHandle,
        target_domain_symbol: SymbolHandle,
    ) -> bool {
        let mut visited = Vec::new();
        self.domain_implies_inner(source_domain_symbol, target_domain_symbol, &mut visited)
    }

    fn domain_implies_inner(
        &self,
        source_domain_symbol: SymbolHandle,
        target_domain_symbol: SymbolHandle,
        visited: &mut Vec<SymbolHandle>,
    ) -> bool {
        if !source_domain_symbol.is_valid() || !target_domain_symbol.is_valid() {
            return false;
        }
        if source_domain_symbol == target_domain_symbol {
            return true;
        }
        if visited.contains(&source_domain_symbol) {
            return false;
        }
        visited.push(source_domain_symbol);

        self.domain_memberships_for_symbol(source_domain_symbol)
            .any(|fact| {
                self.domain_implies_inner(fact.domain_symbol, target_domain_symbol, visited)
            })
    }

    pub fn boolean_facts_for_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> impl Iterator<Item = BooleanFact> + '_ {
        self.facts_for_symbol(symbol)
            .filter_map(|fact| match fact.payload {
                FactPayload::BooleanExpression(expression)
                | FactPayload::ContractBooleanExpression { expression, .. } => {
                    Some(BooleanFact { expression })
                }
                _ => None,
            })
    }

    pub fn domain_memberships_for_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> impl Iterator<Item = DomainMembershipFact> + '_ {
        self.facts_for_symbol(symbol)
            .filter_map(|fact| match fact.payload {
                FactPayload::DomainMembership {
                    value,
                    domain,
                    domain_symbol,
                }
                | FactPayload::ContractDomainMembership {
                    value,
                    domain,
                    domain_symbol,
                    ..
                } => Some(DomainMembershipFact {
                    value,
                    domain,
                    domain_symbol,
                }),
                _ => None,
            })
    }

    pub fn type_constraints_for_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> impl Iterator<Item = TypeConstraintFact> + '_ {
        self.facts_for_symbol(symbol)
            .filter_map(|fact| match fact.payload {
                FactPayload::TypeConstraint { constraint } => {
                    Some(TypeConstraintFact { constraint })
                }
                _ => None,
            })
    }

    pub fn fact_place_equals(&self, fact_place: FactPlace, place: PlaceHandle) -> bool {
        match fact_place {
            FactPlace::Place(other) => self.places_equal(other, place),
            _ => false,
        }
    }

    pub fn places_equal(&self, left: PlaceHandle, right: PlaceHandle) -> bool {
        let left_place = self.places.get(left);
        let right_place = self.places.get(right);
        left_place.root == right_place.root
            && self.place_segments.span_or_empty(left_place.segments)
                == self.place_segments.span_or_empty(right_place.segments)
    }

    pub fn places_match(
        &self,
        program: &TypedTrees,
        left: PlaceHandle,
        right: PlaceHandle,
    ) -> bool {
        self.places_equal(left, right)
            || crate::canonical_place_label(program, self, self.places.get(left))
                == crate::canonical_place_label(program, self, self.places.get(right))
    }
}
