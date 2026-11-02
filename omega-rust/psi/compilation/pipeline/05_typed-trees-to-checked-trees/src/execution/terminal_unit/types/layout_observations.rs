//! A layout query retains its subject even when no runtime value has that type.
//! Walk owned source references rather than the arena: removed declarations do
//! not demand geometry, and a shared expression keeps its supplying telescope.

use super::ShapeCollector;
use std::collections::{BTreeSet, HashSet};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::references::TypedReferences;
use typed_trees::type_identity::TypeIdentityId;
use typed_trees::types::TypeReferenceHandle;

pub(in crate::execution::terminal_unit) fn retain_layout_observation_types(
    program: &dyn validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    retained: &mut BTreeSet<TypeIdentityId>,
) -> Vec<checked_trees::CheckedLayoutObservation> {
    let mut queries = QuerySubjects {
        program,
        owner: SymbolHandle::default(),
        outer_owners: Vec::new(),
        signatures: Vec::new(),
        expressions: HashSet::new(),
        types: HashSet::new(),
        facts: HashSet::new(),
        mathematical_types: HashSet::new(),
        subjects: Vec::new(),
    };
    typed_trees::references::visit_program(program.typed_program(), &mut queries);
    let mut observations = Vec::new();
    for subject in queries.subjects {
        // Discovery includes unapplied generic declarations. Their missing
        // concrete shape is not admission; demanded lowering diagnoses it.
        // The query measures its type's stored carrier. Borrowed parameters
        // instead measure the referent; peeling that loan here would answer
        // size_of<&T> with T's geometry.
        if let Some(identity) = shapes.add_stored_type(subject.reference, &subject.binders, &[]) {
            retained.insert(identity);
            observations.push(checked_trees::CheckedLayoutObservation {
                owner: subject.owner,
                expression: subject.expression,
                subject: identity,
                measurement: subject.measurement,
            });
        }
    }
    observations
}

struct QuerySubject {
    owner: SymbolHandle,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    binders: Vec<(SymbolHandle, String)>,
    measurement: language_core::data_layout_queries::DataLayoutQueryKind,
}

struct QuerySubjects<'a> {
    program: &'a dyn validation::CheckingInput,
    owner: SymbolHandle,
    outer_owners: Vec<SymbolHandle>,
    signatures: Vec<(SymbolHandle, Vec<(SymbolHandle, String)>)>,
    expressions: HashSet<(SymbolHandle, ExpressionHandle)>,
    types: HashSet<(SymbolHandle, TypeReferenceHandle)>,
    facts: HashSet<(SymbolHandle, arena::Handle<typed_trees::domain::ProofFact>)>,
    mathematical_types: HashSet<(
        SymbolHandle,
        typed_trees::mathematical::MathematicalTypeHandle,
    )>,
    subjects: Vec<QuerySubject>,
}

impl Extend<SymbolHandle> for QuerySubjects<'_> {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
}

impl TypedReferences for QuerySubjects<'_> {
    fn declaration(&mut self, owner: SymbolHandle) -> bool {
        self.owner = owner;
        true
    }

    fn enter_lexical_scope(&mut self, owner: SymbolHandle) {
        self.outer_owners.push(self.owner);
        if owner.is_valid() {
            self.owner = owner;
        }
    }

    fn leave_lexical_scope(&mut self) {
        if let Some(owner) = self.outer_owners.pop() {
            self.owner = owner;
        }
    }

    fn callable_signature(&mut self, signature: &typed_trees::signature::StateSignature) {
        self.signatures.push((
            signature.symbol,
            validation::contract_structure::signature_generic_binders(self.program, signature),
        ));
    }

    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        if !self.expressions.insert((self.owner, expression)) {
            return false;
        }
        if let ExpressionNode::Call(call) = self.program.expression_table.expression(expression)
            && let Some(query) = &call.layout_query
        {
            let binders = if let Some((_, binders)) = self
                .signatures
                .iter()
                .rev()
                .find(|(owner, _)| *owner == self.owner)
            {
                binders.clone()
            } else if let Some(scope) =
                validation::StaticTypeArgumentOwner::from_lexical_owner(self.program, self.owner)
            {
                let (parameters, _, _) = scope.parameters(self.program);
                parameters
                    .iter()
                    .enumerate()
                    .map(|(position, parameter)| (parameter.symbol, format!("$T{position}")))
                    .collect()
            } else {
                // An unknown owner cannot borrow another declaration's scope.
                return true;
            };
            self.subjects.push(QuerySubject {
                owner: self.owner,
                expression,
                reference: query.operand.type_reference,
                binders,
                measurement: query.kind,
            });
        }
        true
    }

    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        self.types.insert((self.owner, reference))
    }

    fn proof_fact(&mut self, fact: arena::Handle<typed_trees::domain::ProofFact>) -> bool {
        self.facts.insert((self.owner, fact))
    }

    fn mathematical_type(
        &mut self,
        reference: typed_trees::mathematical::MathematicalTypeHandle,
    ) -> bool {
        self.mathematical_types.insert((self.owner, reference))
    }
}
