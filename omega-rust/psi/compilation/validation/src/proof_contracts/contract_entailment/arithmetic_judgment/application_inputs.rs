//! A record argument retains its complete value separately from its typed
//! scalar read roster. Each read keeps the supplying telescope discovered from
//! the public contracts: projection syntax alone cannot distinguish two calls
//! of the same helper with different actual selectors. Neither value identity
//! nor the read roster authorizes copying, storage access or call formation.

use super::Polynomial;
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::proof_contracts::contract_entailment) enum IntegerApplicationInput {
    Scalar(Polynomial),
    Record {
        type_reference: TypeReferenceHandle,
        snapshot: Option<crate::CapturedOwnedSnapshot>,
        reads: Vec<(crate::DeclaredInputRead, Polynomial)>,
    },
}

impl From<Polynomial> for IntegerApplicationInput {
    fn from(value: Polynomial) -> Self {
        Self::Scalar(value)
    }
}

impl From<&Polynomial> for IntegerApplicationInput {
    fn from(value: &Polynomial) -> Self {
        Self::Scalar(value.clone())
    }
}

impl From<&IntegerApplicationInput> for IntegerApplicationInput {
    fn from(value: &IntegerApplicationInput) -> Self {
        value.clone()
    }
}

impl IntegerApplicationInput {
    pub(super) fn scoped_bindings(
        &self,
        formal: symbols::SymbolHandle,
        mut scalar: impl FnMut(&Polynomial) -> Option<crate::ScopedArithmeticValue>,
    ) -> Option<Vec<crate::ScopedArithmeticBinding>> {
        use crate::{
            ScopedArithmeticBinder as Binder, ScopedArithmeticBinding as Binding,
            ScopedArithmeticValue as Value,
        };
        match self {
            Self::Scalar(value) => Some(vec![Binding {
                binder: Binder::Symbol(formal),
                value: scalar(value)?,
            }]),
            Self::Record {
                snapshot, reads, ..
            } => {
                let mut bindings = vec![Binding {
                    binder: Binder::Symbol(formal),
                    value: Value::OwnedSnapshot(snapshot.clone()?),
                }];
                for (read, value) in reads {
                    let mut read = read.clone();
                    read.coordinate.parameter = formal;
                    bindings.push(Binding {
                        binder: Binder::DeclaredRead(read),
                        value: scalar(value)?,
                    });
                }
                Some(bindings)
            }
        }
    }
    /// The flow owner has already authenticated each read and its observation
    /// frontier. Consume the complete coordinate roster without normalizing a
    /// record as an integer or reopening its source in the caller namespace.
    pub(in crate::proof_contracts::contract_entailment) fn captured(
        engine: &super::Engine<'_>,
        measure: symbols::SymbolHandle,
        input_type: TypeReferenceHandle,
    ) -> Option<Self> {
        let declaration = engine
            .program
            .measures()
            .iter()
            .find(|declaration| declaration.symbol == measure)?;
        let shape = super::super::ranking_range::declared_input_shape(engine.program, declaration)?;
        if !super::super::ranking_range::declared_input_type_matches(
            engine.program,
            input_type,
            shape.type_reference,
        ) {
            return None;
        }
        if engine
            .program
            .primitive_type_reference(shape.type_reference)
            .is_some()
        {
            return Some(Self::Scalar(engine.strict_symbol_value(shape.parameter)?));
        }
        let reads = shape
            .reads
            .iter()
            .map(|read| Some((read.clone(), engine.declared_read_value(read)?)))
            .collect::<Option<Vec<_>>>()?;
        Some(Self::Record {
            type_reference: input_type,
            snapshot: engine
                .owned_arguments
                .iter()
                .find_map(|(binder, snapshot)| {
                    (*binder == crate::ScopedArithmeticBinder::Symbol(shape.parameter))
                        .then(|| snapshot.clone())
                }),
            reads,
        })
    }

    pub(super) fn visit_values(&self, mut visit: impl FnMut(&Polynomial)) {
        match self {
            Self::Scalar(value) => visit(value),
            Self::Record { reads, .. } => reads.iter().for_each(|(_, value)| visit(value)),
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn mapped(
        &self,
        mut map: impl FnMut(&Polynomial) -> Option<Polynomial>,
    ) -> Option<Self> {
        Some(match self {
            Self::Scalar(value) => Self::Scalar(map(value)?),
            Self::Record {
                type_reference,
                snapshot,
                reads,
            } => Self::Record {
                type_reference: *type_reference,
                snapshot: snapshot.clone(),
                reads: reads
                    .iter()
                    .map(|(source, value)| Some((source.clone(), map(value)?)))
                    .collect::<Option<Vec<_>>>()?,
            },
        })
    }
}

impl super::Engine<'_> {
    /// A borrow argument denotes an observation of the supplied referent, while
    /// the selected formal still owns its access contract. The referent's type
    /// is available before an executable reference value is materialized; that
    /// observation does not permit a borrow to satisfy an owned formal.
    /// Executable argument planning must independently retain the actual loan.
    pub(super) fn call_input_type(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
        required: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        use language_semantics::ReferenceAccess;
        use typed_trees::expression::ExpressionNode;

        let borrowed = match self.program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => Some(borrow.access),
            _ => None,
        };
        match crate::reference_shell(self.program, required) {
            Some(shell) => {
                // Denotational call formation admits shared observations, not
                // exclusive/write-only authority. A freshly lent observation
                // also supplies no qualification on the reference descriptor
                // itself; its complete referent type is checked separately.
                if shell.access != ReferenceAccess::Shared
                    || !matches!(
                        self.program.type_reference_table.type_reference(required),
                        typed_trees::types::TypeReferenceNode::Reference { .. }
                    )
                {
                    return None;
                }
                let supplied = borrowed.or_else(|| {
                    crate::reference_shell(self.program, self.source_result_type(expression)?)
                        .map(|shell| shell.access)
                })?;
                (supplied == ReferenceAccess::Shared).then_some(shell.referee)
            }
            None => borrowed.is_none().then_some(required),
        }
    }

    /// Select a whole captured argument first, then retain only its scalar
    /// coordinates. A leaf roster without this root cannot form a record call.
    pub(super) fn captured_record_call_input(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
        parameters: &[typed_trees::signature::StateParameter],
        arguments: &[typed_trees::expression::ExpressionHandle],
    ) -> Option<IntegerApplicationInput> {
        // An explicit shared operand observes this already-captured value.
        // Do not ask for the borrow expression's constructed reference type:
        // that executable value is formed later by the actual custody owner.
        let observed = match self.program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Borrow(borrow)
                if borrow.access == language_semantics::ReferenceAccess::Shared =>
            {
                borrow.target
            }
            typed_trees::expression::ExpressionNode::Borrow(_) => return None,
            _ => expression,
        };
        let place = crate::canonical_place_from_expression(self.program, observed)?;
        let root = place.root.as_symbol()?;
        let (_, snapshot) = self
            .owned_arguments
            .iter()
            .find(|(binder, _)| *binder == crate::ScopedArithmeticBinder::Symbol(root))?;
        let snapshot = snapshot.project(self.program, &place.segments)?;
        let actual_type =
            crate::value_custody::expression_types::readable_observation_type_reference(
                self.program,
                self.source_result_type(observed)?,
            )?;
        if !super::super::ranking_range::declared_input_type_matches(
            self.program,
            snapshot.actual_type(),
            actual_type,
        ) {
            return None;
        }
        let reads = self
            .formal_read_arguments
            .iter()
            .filter_map(|(read, value)| {
                (read.coordinate.parameter == root).then_some(())?;
                let path = read
                    .coordinate
                    .path
                    .strip_prefix(place.segments.as_slice())?;
                // Fixed fields name the same captured value independently of
                // unrelated arguments. Only a dynamic selector requires its
                // supplying telescope to distinguish observations of the same syntax.
                (!read
                    .coordinate
                    .path
                    .iter()
                    .any(|step| matches!(step, facts::PlaceSegment::Index { .. }))
                    || parameters.iter().zip(arguments).all(|(formal, actual)| {
                        read.selector_arguments
                            .iter()
                            .any(|(bound, supplied)| *bound == formal.symbol && supplied == actual)
                    }))
                .then_some(())?;
                let mut read = read.clone();
                read.coordinate.path = path.to_vec();
                Some((read, value.clone()))
            })
            .collect();
        Some(IntegerApplicationInput::Record {
            type_reference: actual_type,
            snapshot: Some(snapshot),
            reads,
        })
    }
}
