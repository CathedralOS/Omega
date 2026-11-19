/*
A contract clause or domain predicate that calls a declared machine
(`domain bool::Held requires premise(self);`) states what that machine's body
computes for the arguments. The clause reader unfolds the call: it reads the
arguments as terms in the caller's scope, binds the callee's parameters to
those terms and reads the callee's body under that binding. The result is the
same proposition or term the author could have written inline.

`wiki/spec/language/domains.md`: "Eligible total, pure machine calls may
occur as denotational value terms inside propositions", and "Predicate calls
retain their ordinary formation and evaluation obligations."
`wiki/spec/terminal-psi/fact_validity.md`: "Pure call congruence requires the
same admitted operation and unchanged argument values". Unfolding meets that
requirement by construction: two calls of one machine on the same argument
terms unfold to the same proposition, and no fact about the call outlives its
argument terms.

Only a machine whose meaning is exactly one body expression the clause reader
itself accepts is unfolded, so the call adds no operation a clause could not
already state. The callee must be closed (no type parameters or static
arguments), take plain scalar values or shared structural inputs, declare no contracts or
effects, and end in a single value expression. A callee with `requires`
would owe them at the call, which this reader does not yet discharge, so it
is refused, as is any recursion through unfolding. Anything else stays
refused at the clause that calls it.
*/

use crate::source_state::expression::TableCallExpression;
use crate::source_state::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ContentPlaceVersion, IntegerType, MachineId, PlaceId,
    Proposition, ScalarTerm, ScalarType, StructuralCaseId, StructuralCaseSubject, ValueId,
};
use symbols::{SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};

use super::contracts::{ClauseScope, ResultTerm};
use super::{Program, Slot};
use crate::lowering::StorageShape;
use crate::lowering::body::{Qualification, Regions, ValueType};
use crate::lowering::{Unserved, refuse};
use terminal_semantics::substitution::clauses::{self, ClauseBindings};

/// A called machine prepared for unfolding: its parameter bindings, its body
/// expression and its declared result.
struct Unfolded {
    machine: SymbolHandle,
    slots: Vec<Slot>,
    values: Vec<Option<ValueId>>,
    bound: Vec<(SymbolHandle, ScalarTerm)>,
    arguments: Arguments,
    body: crate::source_state::expression::ExpressionHandle,
    result: ScalarType,
}

/// A scalar term forms all its operands. Track substitutions during its
/// expansion so only discarded arguments need a separate formation clause.
pub(super) struct Unfolding {
    machine: SymbolHandle,
    used: Vec<bool>,
}

impl Program<'_> {
    /// The clause a call of a declared Boolean machine states.
    pub(super) fn called_clause(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        call: &TableCallExpression,
    ) -> Checked<Proposition> {
        let mut unfolded = self.unfold(id, scope, call)?;
        if unfolded.result != ScalarType::Boolean {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("called predicate whose result is not Boolean"),
            ));
        }
        let body = unfolded.body;
        let (clause, _) =
            self.within_unfolding(id, unfolded.machine, unfolded.bound.len(), |program| {
                program.clause(id, &callee_scope(&unfolded), body)
            })?;
        let clause = clauses::substitute_clause(&clause, &mut unfolded.arguments)
            .map_err(|reason| refuse(id, Construct::Machine, Unserved::Contract(reason)))?;
        self.retain_call_arguments(
            id,
            unfolded.arguments.scalar.into_iter().map(|(_, term)| term),
        )?;
        crate::lowering::canonical::canonical_clause(clause, id)
    }

    /// The term a call of a declared scalar machine denotes.
    pub(super) fn called_term(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        call: &TableCallExpression,
    ) -> Checked<(ScalarTerm, ValueType)> {
        let mut unfolded = self.unfold(id, scope, call)?;
        let body = unfolded.body;
        let result = unfolded.result;
        // Nested calls retain terms in this callee's namespace. Transport them
        // together with the result before adding anything to the caller scope.
        let (((term, value_type), pending), used) =
            self.within_unfolding(id, unfolded.machine, unfolded.bound.len(), |program| {
                program.with_clause_arguments(id, |program| {
                    program.term(id, &callee_scope(&unfolded), body, Some(result))
                })
            })?;
        if value_type.scalar() != Some(result) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("called machine's body differs from its result type"),
            ));
        }
        let term = clauses::substitute_clause_term(&term, &mut unfolded.arguments)
            .map_err(|reason| refuse(id, Construct::Machine, Unserved::Contract(reason)))?;
        for pending in pending {
            let argument = clauses::substitute_clause_term(&pending, &mut unfolded.arguments)
                .map_err(|reason| refuse(id, Construct::Machine, Unserved::Contract(reason)))?;
            self.retain_call_arguments(id, std::iter::once(argument))?;
        }
        self.retain_call_arguments(
            id,
            unfolded
                .arguments
                .scalar
                .into_iter()
                .zip(used)
                .filter_map(|((_, term), used)| (!used).then_some(term)),
        )?;
        Ok((term, ValueType::plain(result)))
    }

    /// Read `body` with `machine` marked as being unfolded, refusing a
    /// machine that is already being unfolded: a recursive predicate has no
    /// finite unfolding.
    fn within_unfolding<T>(
        &mut self,
        id: MachineId,
        machine: SymbolHandle,
        arguments: usize,
        body: impl FnOnce(&mut Self) -> Checked<T>,
    ) -> Checked<(T, Vec<bool>)> {
        self.unfolding_calls.push(Unfolding {
            machine,
            used: vec![false; arguments],
        });
        let read = body(self);
        let unfolding = self.unfolding_calls.pop().ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Contract("called machine lost its unfolding scope"),
            )
        })?;
        if read.is_ok() {
            self.admitted_declarations.include(machine);
            self.admitted_bodies.include(machine);
        }
        read.map(|result| (result, unfolding.used))
    }

    pub(super) fn use_unfolded_argument(
        &mut self,
        id: MachineId,
        owner: SymbolHandle,
        position: usize,
    ) -> Checked<()> {
        if let Some(unfolding) = self
            .unfolding_calls
            .last_mut()
            .filter(|frame| frame.machine == owner)
        {
            let used = unfolding.used.get_mut(position).ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Contract("called machine argument is outside its unfolding scope"),
                )
            })?;
            *used = true;
        }
        Ok(())
    }

    fn unfold(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        call: &TableCallExpression,
    ) -> Checked<Unfolded> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        if call.receiver.is_valid()
            || !call.evidence_arguments.is_empty()
            || call.requirement_application.is_valid()
            || call.exact_coercion_target.is_valid()
            || call.build_selection.is_valid()
            || call.static_machine_parameter.is_valid()
            || call.static_requirement_dispatch.is_some()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.installed_interrupt_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
            || call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
        {
            return unserved("call in a clause with a receiver, evidence or another selection");
        }
        let target = call.target_symbol;
        if !target.is_valid() || !self.typed.symbols.symbols().nodes().is_valid(target) {
            return unserved("call in a clause without a resolved target");
        }
        let machine = match self.typed.symbols.get(target).kind {
            SymbolKind::Machine => target,
            SymbolKind::State => self.typed.symbols.get(target).parent,
            _ => return unserved("call in a clause of something that is not a machine"),
        };
        self.direct_selection(
            id,
            target,
            call.authored_call_selection,
            call.authored_call_binding,
        )?;
        self.applies_nothing(
            id,
            target,
            call.authored_machine_application,
            &call.machine_arguments,
        )?;
        let typed = self.typed;
        let (declaration, state) = self.declaration(id, machine)?;
        if target != machine && target != state.symbol {
            return unserved("call in a clause of a state other than the entry");
        }
        if self
            .unfolding_calls
            .iter()
            .any(|frame| frame.machine == machine)
        {
            return unserved("recursive call in a clause");
        }
        if !declaration.body_is_present
            || declaration.suspends
            || declaration.blocks
            || !declaration.type_parameters.is_empty()
            || !declaration.contracts.is_empty()
            || !state.contracts.is_empty()
        {
            return unserved("called machine in a clause is not a closed, contract-free body");
        }
        // The body's one value: a trailing expression, or the unconditional
        // value transition stage 02 writes for a body that is only a value.
        let body = match typed.statement_table.statements(state.statement_nodes) {
            [StatementNode::Expression(body)] => *body,
            [StatementNode::Transition(transition)]
                if transition.patterns.is_empty()
                    && transition.guard == TransitionGuardNode::Always
                    && transition.exit == TransitionExit::Ordinary
                    && !transition.continuation.is_valid()
                    && transition.proof_selectors.is_empty()
                    && transition.target.is_valid() =>
            {
                match typed.statement_table.transition_target(transition.target) {
                    TransitionTargetNode::Value(body) => *body,
                    _ => return unserved("called machine in a clause is not one value expression"),
                }
            }
            _ => return unserved("called machine in a clause is not one value expression"),
        };
        // The signature owner reads these types at the called machine's
        // lifetime and application context, independently of the caller.
        let called = self.machine(machine)?;
        let signature = self.signatures.get(called).ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Contract("called machine lost its signature"),
            )
        })?;
        let formals: Vec<_> = signature
            .parameters
            .iter()
            .enumerate()
            .map(|(position, value)| {
                (
                    *value,
                    signature.is_erased(position),
                    signature.borrowed(position).cloned(),
                )
            })
            .collect();
        let Some(result) = signature
            .result
            .filter(|value| value.qualification() == Qualification::NONE)
            .and_then(|value| value.scalar())
        else {
            return unserved("called machine in a clause has no plain scalar result");
        };
        let parameters = typed.state_parameters(state);
        if parameters.len() != formals.len() {
            return unserved("called signature differs from its declared parameter roster");
        }
        let mut slots = Vec::with_capacity(parameters.len());
        for (parameter, (value_type, erased, borrowed)) in parameters.iter().zip(&formals) {
            if *erased
                || parameter.is_mutable
                || borrowed.as_ref().is_some_and(|borrowed| {
                    borrowed.access != language_core::ReferenceAccess::Shared
                })
            {
                return unserved("called machine in a clause takes a non-value parameter");
            }
            if value_type.qualification() != Qualification::NONE {
                return unserved("called machine in a clause takes a qualified parameter");
            }
            if borrowed.is_none() && value_type.scalar().is_none() {
                return unserved("called machine in a clause takes a non-scalar value parameter");
            }
            slots.push(Slot {
                symbol: parameter.symbol,
                value_type: *value_type,
                assignable: false,
                homed: false,
                erased: false,
                borrowed: borrowed.as_ref().map(|borrowed| borrowed.access),
                borrowed_regions: borrowed
                    .as_ref()
                    .map_or(Regions::NONE, |borrowed| borrowed.regions),
            });
        }
        let values = crate::lowering::body::formal_values(
            slots
                .iter()
                .map(|slot| (slot.value_type, slot.erased, slot.borrowed.is_some())),
            Some(ValueType::plain(result)),
        );
        let supplied = typed.expression_table.expression_handles(call.arguments);
        if slots.len() != supplied.len() {
            return unserved("call in a clause with the wrong number of arguments");
        }
        let mut arguments = Arguments::default();
        let free = self.machine_lifetimes(machine);
        let mut regions = super::regions::Substitution::new();
        let mut supplied_types = Vec::with_capacity(slots.len());
        let mut structural_position = 0u64;
        for (position, (slot, &argument)) in slots.iter().zip(supplied).enumerate() {
            if slot.borrowed.is_some() {
                let borrowed = formals[position].2.as_ref().ok_or_else(|| {
                    refuse(
                        id,
                        Construct::Machine,
                        Unserved::Contract("logical parameter lost its borrow declaration"),
                    )
                })?;
                let actual = self.shared_call_input(id, scope, argument, borrowed)?;
                let (presented, extent) =
                    self.shared_clause_input_type(id, slot.value_type, &actual, borrowed.referent)?;
                self.unify_regions(
                    id,
                    &free,
                    borrowed.regions,
                    actual.loan_regions,
                    &mut regions,
                )?;
                self.unify(id, &free, slot.value_type, presented, &mut regions)?;
                supplied_types.push((presented, Some(actual.loan_regions)));
                let actual_root = actual.root;
                let path = actual.path;
                // Formal places skip scalar values, in declaration order,
                // exactly as Signature and contracts::formal_place do.
                structural_position += 1;
                let formal = PlaceId::new(structural_position).ok_or_else(|| {
                    refuse(
                        id,
                        Construct::Machine,
                        Unserved::Contract("logical input place overflow"),
                    )
                })?;
                arguments.structural.push(StructuralArgument {
                    formal,
                    actual: actual_root,
                    path,
                    extent,
                });
            } else {
                let Some(scalar_type) = slot.value_type.scalar() else {
                    return unserved("logical value argument has no scalar carrier");
                };
                let (term, supplied) = self.term(id, scope, argument, Some(scalar_type))?;
                if supplied.scalar() != Some(scalar_type) {
                    return unserved("argument in a clause call differs from its parameter type");
                }
                let Some(value) = values[position] else {
                    return unserved("logical scalar argument has no formal identity");
                };
                self.unify(id, &free, slot.value_type, supplied, &mut regions)?;
                supplied_types.push((supplied, None));
                arguments.scalar.push((value, term));
            }
        }
        for ((declared, _, borrowed), (actual, loan)) in formals.iter().zip(&supplied_types) {
            let expected = self.substitute(*declared, &regions);
            if !self.same_type(Some(*actual), expected) {
                return unserved(
                    "logical argument differs from its complete declared parameter type",
                );
            }
            if let (Some(borrowed), Some(loan)) = (borrowed, loan) {
                let expected = self.substituted_regions(borrowed.regions, &regions);
                if !self.same_regions(*loan, expected) {
                    return unserved(
                        "logical shared argument differs from its declared loan lifetime",
                    );
                }
            }
        }
        let bound = slots
            .iter()
            .zip(&values)
            .filter_map(|(slot, value)| {
                let value = (*value)?;
                Some((
                    slot.symbol,
                    ScalarTerm::value(value, slot.value_type.scalar()?),
                ))
            })
            .collect();
        Ok(Unfolded {
            machine,
            slots,
            values,
            bound,
            arguments,
            body,
            result,
        })
    }

    fn shared_clause_input_type(
        &mut self,
        id: MachineId,
        expected: ValueType,
        input: &super::contracts::SharedClauseInput,
        referent: crate::lowering::body::BorrowedReferent,
    ) -> Checked<(ValueType, Option<ScalarTerm>)> {
        use crate::lowering::body::BorrowedReferent;
        use crate::source_state::expression::SharedObservationForm;
        let actual = input.value_type;
        let unserved = |reason| refuse(id, Construct::Machine, Unserved::Contract(reason));
        self.authorized(id, actual, super::authority::Access::Read)?;
        if expected.qualification() != actual.qualification() {
            return Err(unserved(
                "shared clause argument differs from its declared domains",
            ));
        }
        if referent != BorrowedReferent::Slice {
            if expected.erased() != actual.erased() {
                return Err(unserved(
                    "shared clause argument differs from its declared carrier",
                ));
            }
            return Ok((actual, None));
        }
        let ValueType::Record(actual_row, qualification, _) = actual else {
            return Err(unserved("a slice parameter needs a collection view"));
        };
        if self
            .data_type(id, actual_row)?
            .shape
            .fixed_array_extent()
            .is_some()
            && input.form != SharedObservationForm::WholeRange
        {
            return Err(unserved(
                "a fixed array reference is not a view; form its whole range",
            ));
        }
        let (ValueType::Record(expected, _, _), ValueType::Record(actual_row, _, _)) =
            (expected, actual)
        else {
            return Err(unserved(
                "shared clause argument differs from its declared carrier",
            ));
        };
        let expected_shape = &self.data_type(id, expected)?.shape;
        let actual_shape = &self.data_type(id, actual_row)?.shape;
        let collection = match (expected_shape, actual_shape) {
            (StorageShape::ByteSequence(expected), StorageShape::ByteSequence(actual)) => {
                expected.is_shared_readable_view() && actual.is_shared_readable_view()
            }
            (
                StorageShape::ElementView {
                    element: a,
                    qualification: aq,
                    access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                },
                StorageShape::ElementView {
                    element: b,
                    qualification: bq,
                    access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                },
            ) => a == b && aq == bq,
            (
                StorageShape::ByteSequence(expected),
                StorageShape::FixedArray {
                    element,
                    qualification,
                    ..
                },
            ) if expected.is_shared_readable_view() && qualification.is_none() => {
                matches!(self.storage_value_type(id, *element, *qualification)?,
                        ValueType::Scalar(ScalarType::Integer(integer), _) if integer.bits() == Some(8)
                            && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && !integer.is_address())
            }
            (
                StorageShape::ElementView {
                    element: a,
                    qualification: aq,
                    access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                },
                StorageShape::FixedArray {
                    element: b,
                    qualification: bq,
                    ..
                },
            ) => a == b && aq == bq,
            _ => false,
        };
        if !collection {
            return Err(unserved(
                "shared clause argument differs from its declared carrier",
            ));
        }
        let elements = match self.data_type(id, actual_row)?.shape {
            StorageShape::ByteSequence(_) => Regions::NONE,
            _ => self.collection_element_regions(id, actual_row, actual.regions())?,
        };
        let mut tuple = self.region_tuple(input.loan_regions).to_vec();
        tuple.extend_from_slice(self.region_tuple(elements));
        let regions = self.intern_regions(tuple);
        let extent = self.clause_collection_extent(id, input.root, input.path.clone(), actual)?;
        Ok((
            ValueType::Record(expected, qualification, regions),
            Some(extent),
        ))
    }
}

/// The scope the callee's body is read in: its parameters, each bound to the
/// caller's argument term, and nothing of the caller's own names.
fn callee_scope(unfolded: &Unfolded) -> ClauseScope<'_> {
    ClauseScope {
        slots: &unfolded.slots,
        values: &unfolded.values,
        bound: &unfolded.bound,
        owner: unfolded.machine,
        result: ResultTerm::Absent("result in a called machine's body"),
        subject: None,
        formal_fields: true,
        logical: None,
    }
}

/// Replacements are returned directly by the existing substitution visitor.
/// Caller terms are never revisited as if their IDs belonged to the callee.
#[derive(Default)]
struct Arguments {
    scalar: Vec<(ValueId, ScalarTerm)>,
    structural: Vec<StructuralArgument>,
}

struct StructuralArgument {
    formal: PlaceId,
    actual: PlaceId,
    path: Vec<CanonicalStructuralPathSegment>,
    extent: Option<ScalarTerm>,
}

impl Arguments {
    fn projection(
        &self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<(PlaceId, Vec<CanonicalStructuralPathSegment>), &'static str> {
        let position = self
            .structural
            .binary_search_by_key(&root, |argument| argument.formal)
            .map_err(|_| "called clause retains an unbound structural input")?;
        let argument = &self.structural[position];
        let mut supplied = Vec::with_capacity(argument.path.len() + path.len());
        supplied.extend_from_slice(&argument.path);
        supplied.extend_from_slice(path);
        Ok((argument.actual, supplied))
    }
}

impl ClauseBindings for Arguments {
    type Error = &'static str;

    fn preserve_unformed_terms(&self) -> bool {
        true
    }

    fn value(&mut self, id: ValueId, scalar_type: ScalarType) -> Result<ScalarTerm, Self::Error> {
        let position = self
            .scalar
            .binary_search_by_key(&id, |(id, _)| *id)
            .map_err(|_| "called clause retains an unbound scalar input")?;
        let term = &self.scalar[position].1;
        if term.scalar_type() != scalar_type {
            return Err("called clause scalar input changed its carrier");
        }
        Ok(term.clone())
    }
    fn field(
        &mut self,
        version: ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        let (root, path) = self.projection(root, path)?;
        match scalar_type {
            ScalarType::Boolean => Ok(ScalarTerm::BooleanField {
                version,
                root,
                path,
            }),
            ScalarType::Integer(scalar_type) => Ok(ScalarTerm::IntegerField {
                version,
                root,
                path,
                scalar_type,
            }),
            ScalarType::IeeeFloat(format) => Ok(ScalarTerm::IeeeFloatField {
                version,
                root,
                path,
                format,
            }),
            ScalarType::Callable(_) => Err("called clause observes a callable field"),
        }
    }
    fn extent(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: IntegerType,
    ) -> Result<ScalarTerm, Self::Error> {
        if path.is_empty() {
            let position = self
                .structural
                .binary_search_by_key(&root, |argument| argument.formal)
                .map_err(|_| "called clause retains an unbound descriptor input")?;
            if let Some(extent) = &self.structural[position].extent {
                if extent.scalar_type() != ScalarType::Integer(scalar_type) {
                    return Err("called clause descriptor extent changed its carrier");
                }
                return Ok(extent.clone());
            }
        }
        let (root, path) = self.projection(root, path)?;
        Ok(ScalarTerm::ViewExtent {
            root,
            path,
            scalar_type,
        })
    }
    fn byte_predicate(
        &mut self,
        predicate: semantic_vocabulary::ByteSequencePredicate,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<ScalarTerm, Self::Error> {
        let (root, path) = self.projection(root, path)?;
        Ok(ScalarTerm::byte_sequence_predicate(predicate, root, path))
    }
    fn case(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: StructuralCaseId,
    ) -> Result<bool, Self::Error> {
        Err("called clause case membership is not a known Boolean")
    }
    fn case_predicate(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        case: StructuralCaseId,
    ) -> Result<Proposition, Self::Error> {
        let (root, path) = self.projection(root, path)?;
        Ok(Proposition::StructuralCaseMembership {
            subject: StructuralCaseSubject::new(root, path),
            case,
        })
    }
}
