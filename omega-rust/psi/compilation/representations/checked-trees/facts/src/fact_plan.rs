//! Current fact storage and its place, evidence and program-point vocabulary.

pub mod contexts;
pub mod evidence;
pub mod places;

#[cfg(test)]
mod tests;

use arena::{Handle, HandleSpan};
type Arena<T> = arena::PagedArenaSet<T>;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

use crate::{
    BooleanFact, DataDefinitionFactRecord, DomainDefinitionFactRecord, DomainMembershipFact, Fact,
    FactContext, FactContextView, FactOrigin, FactPayload, FactPlace, FactRef,
    InstantiatedExpression, Place, PlaceRoot, PlaceSegment, ProgramPoint,
    QualificationCorrespondence, QualificationEvidence, SymbolFactSet, effective_member_symbol,
    resolve_place_member_symbol,
};

pub type FactHandle = Handle<Fact>;
pub type FactContextHandle = Handle<FactContext>;
pub type PlaceHandle = Handle<Place>;
pub type QualificationCorrespondenceHandle = Handle<QualificationCorrespondence>;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct FactPlan {
    pub places: Arena<Place>,
    pub place_segments: Arena<PlaceSegment>,
    pub facts: Arena<Fact>,
    pub scalar_values: Arena<crate::ScalarValue>,
    pub boolean_predicates: Arena<crate::BooleanPredicate>,
    pub boolean_predicate_bindings: Arena<crate::BooleanPredicateBinding>,
    pub integer_ranges: Arena<crate::IntegerRange>,
    pub qualification_correspondences: Arena<QualificationCorrespondence>,
    pub domain_definition_facts: Arena<DomainDefinitionFactRecord>,
    pub data_definition_facts: Arena<DataDefinitionFactRecord>,
    pub instantiated_expressions: Arena<InstantiatedExpression>,
    pub refs: Arena<FactRef>,
    pub contexts: contexts::FactContexts,
    pub symbol_sets: Arena<SymbolFactSet>,
}

/// Whole-plan operations over the complete root set: copying, storage-reusing
/// replacement, page extension, page handoff, and page adoption.
///
/// The roots are classified once, here. Every generated method names every
/// field of `FactPlan` in a destructuring pattern, so a new arena must be
/// classified in this list before the plan compiles, and `clone_from` — the
/// one fan-out a struct literal cannot check — cannot keep an old root while
/// replacing the others.
macro_rules! fact_plan_roots {
    ($($root:ident),* $(,)?) => {
        impl Clone for FactPlan {
            fn clone(&self) -> Self {
                Self {
                    $($root: self.$root.clone(),)*
                }
            }

            /// Replace the complete plan while reusing storage; this is not an
            /// in-place mutation preserving handles from the replaced plan.
            /// Only handles shared with the source before divergence retain
            /// their meaning after replacement.
            fn clone_from(&mut self, source: &Self) {
                $(self.$root.clone_from(&source.$root);)*
            }
        }

        /// Page partitioning for producers that extend one baseline plan from
        /// several workers. A worker copies the baseline, writes each
        /// partition on its own page, and hands back only those pages;
        /// adopting them keeps every handle the worker minted.
        impl FactPlan {
            /// Share the established baseline before independent producers fork it.
            /// Every arena and the derived context index freeze together; later
            /// pages belong to their producer and do not rewrite this baseline.
            pub fn share_pages(&mut self) {
                let Self { $($root,)* } = self;
                $($root.share_pages();)*
            }

            /// Append later rows of every arena on page `generation`.
            pub fn begin_page(&mut self, generation: u32) {
                let Self { $($root,)* } = self;
                $($root.begin_page(generation);)*
            }

            /// The pages written after the first, leaving this plan at its first page.
            pub fn take_additional_pages(&mut self) -> Self {
                let Self { $($root,)* } = self;
                Self {
                    $($root: $root.take_additional_pages(),)*
                }
            }

            /// Adopt pages taken from a copy of this plan's baseline.
            pub fn adopt_additional_pages(&mut self, other: Self) {
                let Self { $($root,)* } = other;
                $(self.$root.adopt_additional_pages($root);)*
            }
        }
    };
}

fact_plan_roots!(
    places,
    place_segments,
    facts,
    scalar_values,
    boolean_predicates,
    boolean_predicate_bindings,
    integer_ranges,
    qualification_correspondences,
    domain_definition_facts,
    data_definition_facts,
    instantiated_expressions,
    refs,
    contexts,
    symbol_sets,
);

impl FactPlan {
    pub fn with_capacity(fact_capacity: usize, context_capacity: usize) -> Self {
        Self {
            places: Arena::with_capacity(fact_capacity),
            place_segments: Arena::with_capacity(fact_capacity),
            facts: Arena::with_capacity(fact_capacity),
            scalar_values: Arena::with_capacity(fact_capacity),
            boolean_predicates: Arena::with_capacity(fact_capacity),
            boolean_predicate_bindings: Arena::with_capacity(fact_capacity),
            integer_ranges: Arena::with_capacity(fact_capacity),
            qualification_correspondences: Arena::with_capacity(fact_capacity),
            domain_definition_facts: Arena::with_capacity(fact_capacity),
            data_definition_facts: Arena::with_capacity(fact_capacity),
            instantiated_expressions: Arena::with_capacity(fact_capacity),
            refs: Arena::with_capacity(fact_capacity),
            contexts: contexts::FactContexts::with_capacity(context_capacity),
            symbol_sets: Arena::with_capacity(fact_capacity),
        }
    }

    pub fn append_fact(&mut self, fact: Fact) -> FactHandle {
        self.facts.append(fact)
    }

    /// A statement-transfer fact pinned to the statement point with default
    /// evidence — the shared emission shape for assignment and movement facts.
    pub fn append_statement_transfer(
        &mut self,
        place: FactPlace,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        payload: FactPayload,
    ) -> FactHandle {
        self.append_fact(Fact {
            place,
            point: ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::default(),
            payload,
        })
    }

    /// The same statement-transfer emission at a caller-supplied point.
    pub fn append_transfer_at_point(
        &mut self,
        place: FactPlace,
        point: ProgramPoint,
        payload: FactPayload,
    ) -> FactHandle {
        self.append_fact(Fact {
            place,
            point,
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::default(),
            payload,
        })
    }

    pub fn append_qualification_correspondence(
        &mut self,
        correspondence: QualificationCorrespondence,
    ) -> QualificationCorrespondenceHandle {
        if let Some((handle, _)) = self
            .qualification_correspondences
            .iter()
            .find(|(_, retained)| **retained == correspondence)
        {
            return handle;
        }
        self.qualification_correspondences.append(correspondence)
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
        let application = program.proof_facts.get(proof_fact).as_proposition()?;
        program
            .normalize_proposition_application(application, None)
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

    pub fn push_place_segment(&mut self, place: PlaceHandle, segment: PlaceSegment) {
        let segment = self.place_segments.append(segment);
        self.places.get_mut(place).segments.push_contiguous(segment);
    }

    fn push_field_place_segment(
        &mut self,
        program: &typed_trees::TypedTrees,
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

    pub fn context_view(&self, context: &FactContext) -> FactContextView<'_> {
        FactContextView {
            plan: self,
            point: context.point,
            facts: context.facts,
        }
    }

    /// Collect each complete fact identity once, retaining first appearance.
    /// Context membership selects the input; this does not compare payloads,
    /// combine issuers or establish availability outside the supplied contexts.
    pub fn unique_context_facts(
        &self,
        contexts: impl IntoIterator<Item = FactContextHandle>,
    ) -> Vec<FactHandle> {
        arena::unique_handles_in_order(contexts.into_iter().flat_map(|context| {
            self.refs
                .span_or_empty(self.contexts.get(context).facts)
                .iter()
                .map(|reference| reference.fact)
        }))
    }

    pub fn contexts_at_point(
        &self,
        point: ProgramPoint,
    ) -> impl Iterator<Item = FactContextView<'_>> {
        self.context_handles_at_point(point)
            .map(move |handle| self.context_view(self.contexts.get(handle)))
    }

    pub fn context_handles_at_point(
        &self,
        point: ProgramPoint,
    ) -> impl Iterator<Item = FactContextHandle> + '_ {
        self.contexts.handles_at_point(point)
    }

    /// Selects the existing chain once for repeated use in this plan or copies
    /// preserving that chain. An absent selection does not bind future appends.
    pub fn context_group_at_point(&self, point: ProgramPoint) -> contexts::FactContextGroup {
        self.contexts.group_at_point(point)
    }

    /// Reads a previously selected chain without rediscovering its program point.
    pub fn context_handles_in_group(
        &self,
        group: contexts::FactContextGroup,
    ) -> impl Iterator<Item = FactContextHandle> + '_ {
        self.contexts.handles_in_group(group)
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
        self.facts_for_symbol(symbol).filter_map(|fact| {
            fact.payload
                .boolean_expression()
                .map(|expression| BooleanFact { expression })
        })
    }

    pub fn domain_memberships_for_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> impl Iterator<Item = DomainMembershipFact> + '_ {
        self.facts_for_symbol(symbol)
            .filter_map(|fact| as_domain_membership(fact.payload))
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
        if self.places_equal(left, right) {
            return true;
        }
        // Invocation storage is an exact typed identity, never a source name
        // to reconcile with executable places or another machine's formal.
        if matches!(
            self.places.get(left).root,
            PlaceRoot::InvocationReferent { .. } | PlaceRoot::InvocationOutput(_)
        ) || matches!(
            self.places.get(right).root,
            PlaceRoot::InvocationReferent { .. } | PlaceRoot::InvocationOutput(_)
        ) {
            return false;
        }
        // Labels are only compared, so both are written into this thread's
        // reusable buffers instead of two fresh strings per comparison.
        thread_local! {
            static LABELS: std::cell::RefCell<(String, String)> =
                const { std::cell::RefCell::new((String::new(), String::new())) };
        }
        let write_label = |place: PlaceHandle, label: &mut String| {
            let place = self.places.get(place);
            label.clear();
            crate::fact_plan::places::resolution::write_canonical_place_label(
                program,
                place.root,
                self.place_segments.span_or_empty(place.segments),
                label,
            );
        };
        LABELS.with(|labels| match labels.try_borrow_mut() {
            Ok(mut labels) => {
                let (left_label, right_label) = &mut *labels;
                write_label(left, left_label);
                write_label(right, right_label);
                left_label == right_label
            }
            Err(_) => self.place_label(program, left) == self.place_label(program, right),
        })
    }

    /// The canonical label of one stored place; see
    /// [`canonical_place_label_from_parts`](crate::canonical_place_label_from_parts).
    pub fn place_label(&self, program: &TypedTrees, place: PlaceHandle) -> String {
        let place = self.places.get(place);
        crate::canonical_place_label_from_parts(
            program,
            place.root,
            self.place_segments.span_or_empty(place.segments),
        )
    }
}

/// Adapt the two domain-membership payloads to a [`DomainMembershipFact`].
pub(crate) fn as_domain_membership(payload: FactPayload) -> Option<DomainMembershipFact> {
    payload
        .domain_membership()
        .map(
            |(value, domain, domain_symbol, semantic_domain)| DomainMembershipFact {
                value,
                domain,
                domain_symbol,
                semantic_domain,
            },
        )
}
