use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::name::Identifier;
use omega_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle};

pub type FactHandle = Handle<Fact>;
pub type FactRefHandle = Handle<FactRef>;
pub type FactContextHandle = Handle<FactContext>;
pub type PlaceHandle = Handle<Place>;
pub type PlaceSegmentHandle = Handle<PlaceSegment>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlaceRoot {
    #[default]
    Unknown,
    Symbol(SymbolHandle),
    Expression(ExpressionHandle),
    TypeReference(TypeReferenceHandle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceSegment {
    Field { symbol: SymbolHandle },
    Index { expression: ExpressionHandle },
}

impl Default for PlaceSegment {
    fn default() -> Self {
        Self::Field {
            symbol: SymbolHandle::invalid(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Place {
    pub root: PlaceRoot,
    pub segments: HandleSpan<PlaceSegment>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FactPlace {
    #[default]
    Unknown,
    Place(PlaceHandle),
    Symbol(SymbolHandle),
    Expression(ExpressionHandle),
    TypeReference(TypeReferenceHandle),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProgramPoint {
    #[default]
    Global,
    Definition {
        symbol: SymbolHandle,
    },
    Machine {
        machine_symbol: SymbolHandle,
    },
    State {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    Statement {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
    },
    Call {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    },
    CallRequires {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    },
    CallEnsures {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    },
    Exit {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FactOrigin {
    #[default]
    Unknown,
    DomainDefinition {
        domain_symbol: SymbolHandle,
    },
    InvariantDefinition {
        invariant_symbol: SymbolHandle,
    },
    TypeReference,
    ProofObligation,
    MachineContract {
        machine_symbol: SymbolHandle,
    },
    StateSignatureContract {
        owner_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    CallRequires,
    CallEnsures,
    ExitEnsures,
    StatementTransfer,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContractFactKind {
    #[default]
    Requires,
    Ensures,
    Trusted,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProofObligationKind {
    #[default]
    BoundedAssignment,
    BoundedCallArgument,
    BoundedInitializer,
    BoundedStateReturn,
    BoundedValue,
    BoundedTransitionArgument,
    GuardedTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactPayload {
    BooleanExpression(ExpressionHandle),
    DomainMembership {
        value: ExpressionHandle,
        domain: HandleSpan<Identifier>,
        domain_symbol: SymbolHandle,
    },
    TypeConstraint {
        constraint: Handle<TypeConstraintNode>,
    },
    ProofObligation {
        kind: ProofObligationKind,
    },
    Contract {
        kind: ContractFactKind,
        fact: Handle<ProofFact>,
    },
    ContractBooleanExpression {
        kind: ContractFactKind,
        fact: Handle<ProofFact>,
        expression: ExpressionHandle,
    },
    ContractDomainMembership {
        kind: ContractFactKind,
        fact: Handle<ProofFact>,
        value: ExpressionHandle,
        domain: HandleSpan<Identifier>,
        domain_symbol: SymbolHandle,
    },
    InvariantDefinition {
        constraint_count: usize,
    },
}

impl Default for FactPayload {
    fn default() -> Self {
        Self::BooleanExpression(ExpressionHandle::invalid())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Fact {
    pub place: FactPlace,
    pub point: ProgramPoint,
    pub origin: FactOrigin,
    pub payload: FactPayload,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FactRef {
    pub fact: FactHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactContext {
    pub point: ProgramPoint,
    pub facts: HandleSpan<FactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolFactSet {
    pub symbol: SymbolHandle,
    pub facts: HandleSpan<FactRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BooleanFact {
    pub expression: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainMembershipFact {
    pub value: ExpressionHandle,
    pub domain: HandleSpan<Identifier>,
    pub domain_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeConstraintFact {
    pub constraint: Handle<TypeConstraintNode>,
}

#[derive(Debug, Clone, Copy)]
pub struct FactContextView<'facts> {
    plan: &'facts FactPlan,
    pub point: ProgramPoint,
    facts: HandleSpan<FactRef>,
}

impl<'facts> FactContextView<'facts> {
    pub fn facts(self) -> impl Iterator<Item = &'facts Fact> {
        self.plan
            .refs
            .span_or_empty(self.facts)
            .iter()
            .map(move |reference| self.plan.facts.get(reference.fact))
    }

    pub fn boolean_facts(self) -> impl Iterator<Item = BooleanFact> + 'facts {
        self.facts().filter_map(|fact| match fact.payload {
            FactPayload::BooleanExpression(expression)
            | FactPayload::ContractBooleanExpression { expression, .. } => {
                Some(BooleanFact { expression })
            }
            _ => None,
        })
    }

    pub fn domain_memberships(self) -> impl Iterator<Item = DomainMembershipFact> + 'facts {
        self.facts().filter_map(|fact| match fact.payload {
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

    pub fn proves_domain_membership(
        self,
        value: ExpressionHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        self.domain_memberships().any(|fact| {
            fact.value == value && self.plan.domain_implies(fact.domain_symbol, domain_symbol)
        })
    }

    pub fn proves_boolean_expression_in_program(
        self,
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> bool {
        self.proves_boolean_expression_for_place_in_program(program, expression, None)
    }

    pub fn proves_boolean_expression_for_place_in_program(
        self,
        program: &TypedTrees,
        expression: ExpressionHandle,
        place: Option<PlaceHandle>,
    ) -> bool {
        let required_label = program.expression_table.display_name(expression);
        self.facts().any(|fact| {
            let candidate_expression = match fact.payload {
                FactPayload::BooleanExpression(candidate_expression)
                | FactPayload::ContractBooleanExpression {
                    expression: candidate_expression,
                    ..
                } => candidate_expression,
                _ => return false,
            };

            candidate_expression == expression
                || program.expression_table.display_name(candidate_expression) == required_label
                || place.is_some_and(|required_place| {
                    matches!(fact.place, FactPlace::Place(candidate_place)
                        if self.plan.places_match(program, candidate_place, required_place))
                })
        })
    }

    pub fn proves_place_domain_membership(
        self,
        place: PlaceHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        self.facts().any(|fact| {
            matches!(
                fact.payload,
                FactPayload::DomainMembership { domain_symbol: fact_domain, .. }
                    | FactPayload::ContractDomainMembership { domain_symbol: fact_domain, .. }
                    if self.plan.fact_place_equals(fact.place, place)
                && self.plan.domain_implies(fact_domain, domain_symbol)
            )
        })
    }

    pub fn proves_place_domain_membership_in_program(
        self,
        program: &TypedTrees,
        place: PlaceHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        self.facts().any(|fact| {
            let (fact_domain, fact_place) = match fact.payload {
                FactPayload::DomainMembership { domain_symbol, .. }
                | FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                    let FactPlace::Place(place) = fact.place else {
                        return false;
                    };
                    (domain_symbol, place)
                }
                _ => return false,
            };

            self.plan.domain_implies(fact_domain, domain_symbol)
                && self.plan.places_match(program, fact_place, place)
        })
    }

    pub fn references_domain(self, domain_symbol: SymbolHandle) -> bool {
        self.domain_memberships()
            .any(|fact| self.plan.domain_implies(fact.domain_symbol, domain_symbol))
    }

    pub fn type_constraints(self) -> impl Iterator<Item = TypeConstraintFact> + 'facts {
        self.facts().filter_map(|fact| match fact.payload {
            FactPayload::TypeConstraint { constraint } => Some(TypeConstraintFact { constraint }),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactPlan {
    pub places: Arena<Place>,
    pub place_segments: Arena<PlaceSegment>,
    pub facts: Arena<Fact>,
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
            refs: Arena::with_capacity(fact_capacity),
            contexts: Arena::with_capacity(context_capacity),
            symbol_sets: Arena::with_capacity(fact_capacity),
        }
    }

    pub fn append_fact(&mut self, fact: Fact) -> FactHandle {
        self.facts.append(fact)
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
            ExpressionNode::Mutable(inner) => self.append_place_from_expression(program, *inner),
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
                    self.push_place_segment(
                        place,
                        PlaceSegment::Field {
                            symbol: member_symbol,
                        },
                    );
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
                self.push_place_segment(place, PlaceSegment::Field { symbol });
                place
            }
            ExpressionNode::Indexed(indexed) => {
                let place = self.append_place_from_expression(program, indexed.collection);
                self.push_place_segment(
                    place,
                    PlaceSegment::Index {
                        expression: indexed.index,
                    },
                );
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
            || canonical_place_label(program, self, self.places.get(left))
                == canonical_place_label(program, self, self.places.get(right))
    }
}

fn effective_member_symbol(
    program: &TypedTrees,
    receiver: ExpressionHandle,
    member: &omega_typed_trees::expression::TableMemberExpression,
) -> SymbolHandle {
    if let Some(symbol) =
        resolve_member_symbol_from_receiver(program, receiver, member.member.as_str())
    {
        return symbol;
    }

    if member.member_symbol.is_valid() {
        return member.member_symbol;
    }

    SymbolHandle::invalid()
}

fn resolve_member_symbol_from_receiver(
    program: &TypedTrees,
    receiver: ExpressionHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    let type_symbol = expression_type_symbol(program, receiver)?;

    if let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == type_symbol)
    {
        for member in program.data_members(data) {
            match member {
                omega_typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    return Some(field.symbol);
                }
                omega_typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == member_name =>
                {
                    return Some(variant.symbol);
                }
                _ => {}
            }
        }
    }

    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == type_symbol)
    {
        if let Some(attached_data) = machine.attached_data.as_deref()
            && let Some(data) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == attached_data)
        {
            for member in program.data_members(data) {
                match member {
                    omega_typed_trees::data::DataMember::Field(field)
                        if field.name.as_str() == member_name =>
                    {
                        return Some(field.symbol);
                    }
                    omega_typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == member_name =>
                    {
                        return Some(variant.symbol);
                    }
                    _ => {}
                }
            }
        }
        for owned in program.machine_owned_data(machine) {
            if owned.name.as_str() == member_name {
                return Some(owned.symbol);
            }
        }
        for contained in program.machine_contained_objects(machine) {
            if contained.name.as_str() == member_name {
                return Some(contained.symbol);
            }
        }
    }

    None
}

fn canonical_place_label(program: &TypedTrees, facts: &FactPlan, place: &Place) -> String {
    canonical_place_label_from_parts(
        program,
        place.root,
        facts.place_segments.span_or_empty(place.segments),
    )
}

fn canonical_place_label_from_parts(
    program: &TypedTrees,
    root: PlaceRoot,
    segments: &[PlaceSegment],
) -> String {
    let mut label = match root {
        PlaceRoot::Unknown => "unknown".to_owned(),
        PlaceRoot::Symbol(symbol) => symbol_label(program, symbol),
        PlaceRoot::Expression(expression) => program.expression_table.display_name(expression),
        PlaceRoot::TypeReference(type_reference) => program.display_type_reference(type_reference),
    };

    for segment in segments {
        match segment {
            PlaceSegment::Field { symbol } => {
                label.push('.');
                label.push_str(&symbol_label(program, *symbol));
            }
            PlaceSegment::Index { expression } => {
                label.push('[');
                label.push_str(&program.expression_table.display_name(*expression));
                label.push(']');
            }
        }
    }

    label
}

fn symbol_label(program: &TypedTrees, symbol: SymbolHandle) -> String {
    for data in program.data_definitions() {
        if data.symbol == symbol {
            return data.name.as_str().to_owned();
        }

        for member in program.data_members(data) {
            match member {
                omega_typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    return field.name.as_str().to_owned();
                }
                omega_typed_trees::data::DataMember::Variant(variant)
                    if variant.symbol == symbol =>
                {
                    return variant.name.as_str().to_owned();
                }
                omega_typed_trees::data::DataMember::Field(_)
                | omega_typed_trees::data::DataMember::Variant(_) => {}
            }
        }
    }

    for machine in program.machines() {
        if machine.symbol == symbol {
            return machine.name.as_str().to_owned();
        }
        for contained_object in program.machine_contained_objects(machine) {
            if contained_object.symbol == symbol || contained_object.type_symbol == symbol {
                return contained_object.name.as_str().to_owned();
            }
        }
        for owned_data in program.machine_owned_data(machine) {
            if owned_data.symbol == symbol {
                return owned_data.name.as_str().to_owned();
            }
        }
        for state in program.machine_states(machine) {
            if state.symbol == symbol {
                return state.name.as_str().to_owned();
            }
            for parameter in program.state_parameters(state) {
                if parameter.symbol == symbol {
                    return parameter.name.as_str().to_owned();
                }
            }
        }
    }

    for trait_definition in program.traits() {
        if trait_definition.symbol == symbol {
            return trait_definition.name.as_str().to_owned();
        }
        for requirement in program.trait_requirements(trait_definition) {
            if requirement.symbol == symbol {
                return requirement.name.as_str().to_owned();
            }
        }
        for machine_signature in program.trait_machine_signatures(trait_definition) {
            if machine_signature.symbol == symbol {
                return machine_signature.name.as_str().to_owned();
            }
            for parameter in program.state_signature_parameters(machine_signature) {
                if parameter.symbol == symbol {
                    return parameter.name.as_str().to_owned();
                }
            }
        }
    }

    for platform in program.platforms() {
        if platform.symbol == symbol {
            return platform.name.as_str().to_owned();
        }
        for state_signature in program.platform_state_signatures(platform) {
            if state_signature.symbol == symbol {
                return state_signature.name.as_str().to_owned();
            }
            for parameter in program.state_signature_parameters(state_signature) {
                if parameter.symbol == symbol {
                    return parameter.name.as_str().to_owned();
                }
            }
        }
    }

    format!("symbol#{}", symbol.arena_index())
}

fn expression_type_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => expression_type_symbol(program, *inner),
        ExpressionNode::Name(path) => {
            let symbol = if path.head_symbol.is_valid() {
                path.head_symbol
            } else {
                path.symbol
            };
            symbol_type_symbol(program, symbol)
        }
        ExpressionNode::Member(member) => {
            let symbol = effective_member_symbol(program, member.receiver, member);
            symbol_type_symbol(program, symbol)
        }
        _ => None,
    }
}

fn symbol_type_symbol(program: &TypedTrees, symbol: SymbolHandle) -> Option<SymbolHandle> {
    if !symbol.is_valid() {
        return None;
    }

    for machine in program.machines() {
        if machine.symbol == symbol {
            if let Some(attached_data) = machine.attached_data.as_deref() {
                if let Some(data) = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.name.as_str() == attached_data)
                {
                    return Some(data.symbol);
                }
            }
        }
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                if parameter.symbol == symbol {
                    return Some(type_reference_base_symbol(
                        program,
                        parameter.type_reference,
                    ));
                }
            }
        }
        for owned in program.machine_owned_data(machine) {
            if owned.symbol == symbol {
                return Some(type_reference_base_symbol(program, owned.type_reference));
            }
        }
        for contained in program.machine_contained_objects(machine) {
            if contained.symbol == symbol {
                return Some(contained.type_symbol);
            }
        }
    }

    for data in program.data_definitions() {
        for member in program.data_members(data) {
            if let omega_typed_trees::data::DataMember::Field(field) = member
                && field.symbol == symbol
            {
                return Some(type_reference_base_symbol(program, field.type_reference));
            }
        }
    }

    None
}

fn type_reference_base_symbol(
    program: &TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> SymbolHandle {
    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            type_reference_base_symbol(program, *referee)
        }
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_base_symbol(program, *base_type)
        }
        omega_typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
        | omega_typed_trees::types::TypeReferenceNode::Named {
            symbol: base_symbol,
            ..
        } => *base_symbol,
        omega_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Unit => SymbolHandle::invalid(),
    }
}

fn resolve_place_member_symbol(
    program: &TypedTrees,
    facts: &FactPlan,
    place: PlaceHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    let place = facts.places.get(place);
    let base_symbol = fact_place_type_symbol(program, facts, place)?;

    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == base_symbol)
    {
        if let Some(attached_data) = machine.attached_data.as_deref()
            && let Some(data) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == attached_data)
        {
            for member in program.data_members(data) {
                match member {
                    omega_typed_trees::data::DataMember::Field(field)
                        if field.name.as_str() == member_name =>
                    {
                        return Some(field.symbol);
                    }
                    omega_typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == member_name =>
                    {
                        return Some(variant.symbol);
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == base_symbol)
    {
        for member in program.data_members(data) {
            match member {
                omega_typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    return Some(field.symbol);
                }
                omega_typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == member_name =>
                {
                    return Some(variant.symbol);
                }
                _ => {}
            }
        }
    }

    None
}

fn fact_place_type_symbol(
    program: &TypedTrees,
    facts: &FactPlan,
    place: &Place,
) -> Option<SymbolHandle> {
    let mut current = match place.root {
        PlaceRoot::Symbol(symbol) => symbol_type_symbol(program, symbol)?,
        PlaceRoot::Expression(expression) => expression_type_symbol(program, expression)?,
        PlaceRoot::Unknown | PlaceRoot::TypeReference(_) => return None,
    };

    for segment in facts.place_segments.span_or_empty(place.segments) {
        match segment {
            PlaceSegment::Field { symbol } => {
                current = symbol_type_symbol(program, *symbol)?;
            }
            PlaceSegment::Index { .. } => return None,
        }
    }

    Some(current)
}

pub fn build_definition_fact_plan(program: &TypedTrees) -> FactPlan {
    let mut facts = FactPlan::with_capacity(
        estimated_definition_fact_capacity(program),
        estimated_definition_context_capacity(program),
    );

    append_domain_definition_facts(program, &mut facts);
    append_invariant_definition_facts(program, &mut facts);

    facts
}

fn estimated_definition_fact_capacity(program: &TypedTrees) -> usize {
    let domain_facts = program
        .domain_definitions()
        .iter()
        .map(|domain| program.proof_facts(domain).len())
        .sum::<usize>();
    let invariant_constraints = program
        .invariant_definitions()
        .iter()
        .map(|invariant| invariant.constraints.len())
        .sum::<usize>();

    domain_facts.saturating_add(invariant_constraints)
}

fn estimated_definition_context_capacity(program: &TypedTrees) -> usize {
    program
        .domain_definitions()
        .len()
        .saturating_add(program.invariant_definitions().len())
}

fn append_domain_definition_facts(program: &TypedTrees, facts: &mut FactPlan) {
    for domain in program.domain_definitions() {
        let mut refs = HandleSpan::empty();
        for fact_handle in proof_fact_handles(domain.facts) {
            let proof_fact = program.proof_facts.get(fact_handle);
            let place = append_proof_fact_place(program, facts, proof_fact);
            let payload = match proof_fact {
                ProofFact::Expression(expression) => FactPayload::BooleanExpression(*expression),
                ProofFact::Membership(membership) => FactPayload::DomainMembership {
                    value: membership.value,
                    domain: membership.domain,
                    domain_symbol: membership.domain_symbol,
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
                payload,
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
    }
}

fn append_invariant_definition_facts(program: &TypedTrees, facts: &mut FactPlan) {
    for invariant in program.invariant_definitions() {
        let mut refs = HandleSpan::empty();
        let place = facts.append_symbol_place(invariant.symbol);
        for constraint in type_constraint_handles(invariant.constraints) {
            let fact = facts.append_fact(Fact {
                place: FactPlace::Place(place),
                point: ProgramPoint::Definition {
                    symbol: invariant.symbol,
                },
                origin: FactOrigin::InvariantDefinition {
                    invariant_symbol: invariant.symbol,
                },
                payload: FactPayload::TypeConstraint { constraint },
            });
            facts.append_ref(&mut refs, fact);
        }
        facts.append_context(
            ProgramPoint::Definition {
                symbol: invariant.symbol,
            },
            refs,
        );
        facts.append_symbol_set(invariant.symbol, refs);
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

fn type_constraint_handles(
    constraints: HandleSpan<TypeConstraintNode>,
) -> impl Iterator<Item = Handle<TypeConstraintNode>> {
    (0..constraints.count()).map(move |offset| {
        Handle::from_parts(
            constraints
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("type constraint handle index overflow"),
            constraints.start().generation(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceRoot, PlaceSegment, ProgramPoint,
        build_definition_fact_plan,
    };
    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolHandle;
    use omega_typed_trees::TypedTrees;
    use omega_typed_trees::domain::{DomainDefinition, ProofFact, ProofMembershipFact};
    use omega_typed_trees::expression::{
        ExpressionNode, TableIndexedExpression, TableMemberExpression, TableNamePath,
    };
    use omega_typed_trees::invariant::InvariantDefinition;
    use omega_typed_trees::name::Identifier;
    use omega_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle};

    #[test]
    fn builds_definition_fact_plan_for_domains_and_invariants() {
        let valid_domain_symbol = SymbolHandle::from_arena_index(10);
        let alive_domain_symbol = SymbolHandle::from_arena_index(11);
        let invariant_symbol = SymbolHandle::from_arena_index(12);

        let mut program = TypedTrees::default();
        let expression = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let fact = program
            .proof_facts
            .append(ProofFact::Expression(expression));
        let membership = program
            .proof_facts
            .append(ProofFact::Membership(ProofMembershipFact {
                value: expression,
                domain: HandleSpan::empty(),
                domain_symbol: valid_domain_symbol,
            }));
        assert_eq!(membership.arena_index(), fact.arena_index() + 1);
        program.push_domain_definition(DomainDefinition {
            symbol: alive_domain_symbol,
            name: Identifier::generated("Player::Alive"),
            target_type: TypeReferenceHandle::invalid(),
            facts: HandleSpan::from_parts(fact, 2),
            body_token_count: 2,
        });
        program.push_domain_definition(DomainDefinition {
            symbol: valid_domain_symbol,
            name: Identifier::generated("Player::Valid"),
            target_type: TypeReferenceHandle::invalid(),
            facts: HandleSpan::empty(),
            body_token_count: 0,
        });

        let constraint = program
            .type_reference_table
            .insert_constraints([TypeConstraintNode::Named(Identifier::generated("finite"))]);
        program.push_invariant_definition(InvariantDefinition {
            symbol: invariant_symbol,
            name: Identifier::generated("Finite"),
            constraints: constraint,
        });

        let facts = build_definition_fact_plan(&program);

        assert_eq!(facts.places.len(), 3);
        assert_eq!(facts.facts.len(), 3);
        assert_eq!(facts.contexts.len(), 3);
        assert_eq!(facts.symbol_sets.len(), 3);
        assert_eq!(
            facts.boolean_facts_for_symbol(alive_domain_symbol).count(),
            1
        );
        assert!(facts.symbol_references_domain(alive_domain_symbol, valid_domain_symbol));
        assert_eq!(
            facts.type_constraints_for_symbol(invariant_symbol).count(),
            1
        );
        assert_eq!(
            facts
                .facts_at_point(super::ProgramPoint::Definition {
                    symbol: alive_domain_symbol,
                })
                .count(),
            2
        );
        let domain_context = facts
            .contexts_at_point(super::ProgramPoint::Definition {
                symbol: alive_domain_symbol,
            })
            .next()
            .expect("domain context");
        assert_eq!(domain_context.boolean_facts().count(), 1);
        assert!(domain_context.proves_domain_membership(expression, valid_domain_symbol));

        let invariant_context = facts
            .contexts_at_point(super::ProgramPoint::Definition {
                symbol: invariant_symbol,
            })
            .next()
            .expect("invariant context");
        assert_eq!(invariant_context.type_constraints().count(), 1);
    }

    #[test]
    fn domain_membership_queries_follow_domain_imports() {
        let valid_domain_symbol = SymbolHandle::from_arena_index(20);
        let alive_domain_symbol = SymbolHandle::from_arena_index(21);

        let mut program = TypedTrees::default();
        let expression = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let membership = program
            .proof_facts
            .append(ProofFact::Membership(ProofMembershipFact {
                value: expression,
                domain: HandleSpan::empty(),
                domain_symbol: valid_domain_symbol,
            }));
        program.push_domain_definition(DomainDefinition {
            symbol: alive_domain_symbol,
            name: Identifier::generated("Player::Alive"),
            target_type: TypeReferenceHandle::invalid(),
            facts: HandleSpan::from_parts(membership, 1),
            body_token_count: 1,
        });
        program.push_domain_definition(DomainDefinition {
            symbol: valid_domain_symbol,
            name: Identifier::generated("Player::Valid"),
            target_type: TypeReferenceHandle::invalid(),
            facts: HandleSpan::empty(),
            body_token_count: 0,
        });

        let mut facts = build_definition_fact_plan(&program);
        let place = facts.append_expression_place(expression);
        facts.append_fact_context(Fact {
            place: FactPlace::Place(place),
            point: ProgramPoint::Global,
            origin: FactOrigin::Unknown,
            payload: FactPayload::DomainMembership {
                value: expression,
                domain: HandleSpan::empty(),
                domain_symbol: alive_domain_symbol,
            },
        });

        assert!(facts.domain_implies(alive_domain_symbol, valid_domain_symbol));
        assert!(facts.proves_domain_membership_at_point(
            ProgramPoint::Global,
            expression,
            valid_domain_symbol
        ));
    }

    #[test]
    fn expression_places_preserve_roots_and_segments() {
        let root_symbol = SymbolHandle::from_arena_index(30);
        let field_symbol = SymbolHandle::from_arena_index(31);
        let tail_symbol = SymbolHandle::from_arena_index(32);

        let mut program = TypedTrees::default();
        let mut members = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member(&mut members, Identifier::generated("root"));
        program
            .expression_table
            .push_name_path_member(&mut members, Identifier::generated("field"));
        let mut member_symbols = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member_symbol(&mut member_symbols, root_symbol);
        program
            .expression_table
            .push_name_path_member_symbol(&mut member_symbols, field_symbol);
        let name = program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members,
                member_symbols,
                head_symbol: root_symbol,
                symbol: field_symbol,
            }));
        let index = program.expression_table.insert(ExpressionNode::Integer(0));
        let indexed =
            program
                .expression_table
                .insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection: name,
                    index,
                }));
        let member =
            program
                .expression_table
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver: indexed,
                    member_symbol: tail_symbol,
                    member: Identifier::generated("tail"),
                }));

        let mut facts = FactPlan::default();
        let place = facts.append_place_from_expression(&program, member);
        let place = facts.places.get(place);
        let segments = facts.place_segments.span_or_empty(place.segments);

        assert_eq!(place.root, PlaceRoot::Symbol(root_symbol));
        assert_eq!(segments.len(), 3);
        assert_eq!(
            segments[0],
            PlaceSegment::Field {
                symbol: field_symbol
            }
        );
        assert_eq!(segments[1], PlaceSegment::Index { expression: index });
        assert_eq!(
            segments[2],
            PlaceSegment::Field {
                symbol: tail_symbol
            }
        );
    }

    #[test]
    fn proves_domain_membership_for_structurally_equal_places() {
        let domain_symbol = SymbolHandle::from_arena_index(40);
        let value_symbol = SymbolHandle::from_arena_index(41);
        let field_symbol = SymbolHandle::from_arena_index(42);

        let mut facts = FactPlan::default();
        let left = facts.append_symbol_place(value_symbol);
        facts.push_place_segment(
            left,
            PlaceSegment::Field {
                symbol: field_symbol,
            },
        );

        let right = facts.append_symbol_place(value_symbol);
        facts.push_place_segment(
            right,
            PlaceSegment::Field {
                symbol: field_symbol,
            },
        );

        let fact = facts.append_fact(Fact {
            place: FactPlace::Place(left),
            point: ProgramPoint::Global,
            origin: FactOrigin::DomainDefinition { domain_symbol },
            payload: FactPayload::DomainMembership {
                value: omega_typed_trees::expression::ExpressionHandle::invalid(),
                domain: HandleSpan::empty(),
                domain_symbol,
            },
        });
        let mut refs = HandleSpan::empty();
        facts.append_ref(&mut refs, fact);
        let context = facts.append_context(ProgramPoint::Global, refs);

        assert!(facts.places_equal(left, right));
        assert!(
            facts
                .context_view(facts.contexts.get(context))
                .proves_place_domain_membership(right, domain_symbol)
        );
    }

    #[test]
    fn expression_places_resolve_attached_data_members() {
        let machine_symbol = SymbolHandle::from_arena_index(50);
        let self_symbol = SymbolHandle::from_arena_index(51);
        let player_field_symbol = SymbolHandle::from_arena_index(52);
        let player_type_symbol = SymbolHandle::from_arena_index(53);
        let main_data_symbol = SymbolHandle::from_arena_index(54);

        let mut program = TypedTrees::default();
        program.push_data_definition(omega_typed_trees::data::DataDefinition {
            symbol: player_type_symbol,
            name: Identifier::generated("Player"),
            type_parameters: HandleSpan::empty(),
            members: HandleSpan::empty(),
        });
        let mut main_data = omega_typed_trees::data::DataDefinition {
            symbol: main_data_symbol,
            name: Identifier::generated("Main"),
            type_parameters: HandleSpan::empty(),
            members: HandleSpan::empty(),
        };
        program.push_data_member(
            &mut main_data,
            omega_typed_trees::data::DataMember::Field(omega_typed_trees::data::DataField {
                symbol: player_field_symbol,
                name: Identifier::generated("player"),
                type_reference: TypeReferenceHandle::invalid(),
            }),
        );
        program.push_data_definition(main_data);

        let mut machine = omega_typed_trees::machine::Machine {
            symbol: machine_symbol,
            name: Identifier::generated("Main::main"),
            attached_data: Some(Identifier::generated("Main")),
            contains: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            effects: HandleSpan::empty(),
            contracts: HandleSpan::empty(),
            states: HandleSpan::empty(),
        };
        let mut state = omega_typed_trees::state::State {
            symbol: SymbolHandle::from_arena_index(55),
            name: Identifier::generated("main"),
            parameters: HandleSpan::empty(),
            return_type: TypeReferenceHandle::invalid(),
            statement_nodes: HandleSpan::empty(),
        };
        let self_type = program.type_reference_table.insert(
            omega_typed_trees::types::TypeReferenceNode::Named {
                symbol: machine_symbol,
                name: Identifier::generated("Self"),
            },
        );
        program.push_state_parameter(
            &mut state,
            omega_typed_trees::signature::StateParameter {
                symbol: self_symbol,
                name: Identifier::generated("self"),
                type_reference: self_type,
                is_const: false,
                is_mutable: true,
                is_self: true,
            },
        );
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);

        let self_expression =
            program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members: HandleSpan::empty(),
                    member_symbols: HandleSpan::empty(),
                    head_symbol: self_symbol,
                    symbol: self_symbol,
                }));
        let member_expression =
            program
                .expression_table
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver: self_expression,
                    member_symbol: SymbolHandle::invalid(),
                    member: Identifier::generated("player"),
                }));

        let mut facts = FactPlan::default();
        let place = facts.append_place_from_expression(&program, member_expression);
        let place = facts.places.get(place);
        let segments = facts.place_segments.span_or_empty(place.segments);

        assert_eq!(place.root, PlaceRoot::Symbol(self_symbol));
        assert_eq!(segments.len(), 1);
        assert_eq!(
            segments[0],
            PlaceSegment::Field {
                symbol: player_field_symbol
            }
        );
    }
}
