/*
A named domain's `requires` predicates, stated as ordinary contracts on the
place whose declared type carries the domain. A domain predicate is not a
kind of fact of its own: each place a type can be written on already has a
contract the checker proves and assumes, and the predicate joins it.

| Where `T in D` is written | What the predicates of `D` become |
| A formal | Entry `requires` clauses of the callable, over the formal. |
| A result | `ensures` clauses of the callable, over `result`. |
| A field of a data type | `requires` clauses of that data type, over `self.field`. |
| The result of `value as +D` | The invariant of the block the cast continues in. |

`wiki/spec/language/domains.md`: a parameter `value: T in D` "gives the body
the fact `value in D`"; "`+D` obeys the establishment rules of `D`: Has
predicates | Prove them for this value."; and of mutation, "Declared
default-domain obligations are enforced at consumption points".
`wiki/spec/language/dependent_values.md`: "A data declaration's field
qualifications and `requires` clauses define its default domain", so a
field's domain is owed by every construction and store of its owner and
kept by the owner's invariant windows, with nothing added here.

`self` in a domain's clause "is the value in the underlying carrier without
the domain being defined" (domains.md, "Operands"), so it is read at the
plain carrier.

Served: a scalar carrier. A domain with predicates on a data carrier is
refused where the type is read (`declared_domain`): a predicate over the
fields of a value that is itself a local or a formal has no owning data
type to keep its window.
*/

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, MachineId, Proposition, ScalarTerm, ScalarType, ValueId,
};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::ExpressionHandle;

use super::authority::has_predicates;
use super::contracts::{ClauseScope, DataSubject, MathematicalTerms, ResultTerm};
use super::{Program, Slot};
use crate::body::{Qualification, ValueType};
use crate::{ClauseText, Unserved, refuse};

/// One predicate of a declared domain: the symbol its `self` resolved to
/// and the clause's expression.
struct Predicate {
    subject: SymbolHandle,
    expression: ExpressionHandle,
    /// The declaring domain's name.
    domain: String,
}

impl Program<'_> {
    /// The predicates of every declared domain in `qualification`, in the
    /// domains' order and each domain's authored order. A clause that says
    /// nothing (`requires true`) is left out.
    fn predicates(&self, id: MachineId, qualification: Qualification) -> Checked<Vec<Predicate>> {
        let mut predicates = Vec::new();
        for domain in qualification.domains.members() {
            let Some(definition) = self.typed.domain_definition(domain) else {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Type("constrained by a domain with no declaration"),
                ));
            };
            if !has_predicates(definition) {
                continue;
            }
            let facts = super::span(
                &self.typed.proof_facts,
                definition.facts,
                id,
                "domain requires span",
            )?;
            for fact in facts {
                let expression = self.fact(id, fact)?;
                if !self.states_nothing(id, expression)? {
                    predicates.push(Predicate {
                        subject: definition.subject_symbol,
                        expression,
                        domain: definition.name.as_str().to_owned(),
                    });
                }
            }
        }
        Ok(predicates)
    }

    /// The predicates of `qualification` about one scalar value, `subject`,
    /// read in `scope` with each clause's `self` bound to that value.
    fn of_value(
        &self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        qualification: Qualification,
        subject: &ScalarTerm,
    ) -> Checked<Vec<(Proposition, ExpressionHandle)>> {
        let mut clauses = Vec::new();
        for predicate in self.predicates(id, qualification)? {
            if !predicate.subject.is_valid() {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Spelling("domain predicate without a subject symbol"),
                ));
            }
            let mut bound = scope.bound.to_vec();
            bound.push((predicate.subject, subject.clone()));
            let scope = ClauseScope {
                bound: &bound,
                ..*scope
            };
            clauses.push((
                self.clause(id, &scope, predicate.expression)?,
                predicate.expression,
            ));
        }
        Ok(clauses)
    }

    /// What the domains of a callable's formals and result add to its
    /// contract: entry clauses over each formal in a domain with
    /// predicates, and guarantees over a result in one. `entry` and `exit`
    /// are the scopes the callable's authored `requires` and `ensures` are
    /// read in.
    pub(super) fn signature_predicates(
        &self,
        id: MachineId,
        entry: &ClauseScope<'_>,
        exit: &ClauseScope<'_>,
        result: Option<(ValueType, ValueId)>,
    ) -> Checked<SignaturePredicates> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        let mut added = SignaturePredicates::default();
        for (position, slot) in entry.slots.iter().enumerate() {
            let qualification = slot.value_type.qualification();
            if self.predicates(id, qualification)?.is_empty() {
                continue;
            }
            let ValueType::Scalar(scalar_type, _) = slot.value_type else {
                return unserved("formal of a data type in a domain with predicates");
            };
            // A borrowed formal's clause is about what its referent holds,
            // at entry and again at every handback; only the first has a
            // contract to join yet.
            if slot.borrowed.is_some() {
                return unserved("borrowed formal in a domain with predicates");
            }
            let Some(value) = entry.values.get(position).copied().flatten() else {
                return unserved("erased formal in a domain with predicates");
            };
            let subject = ScalarTerm::Value {
                id: value,
                scalar_type,
            };
            for (clause, expression) in self.of_value(id, entry, qualification, &subject)? {
                // The text names the formal where the domain wrote `self`.
                let text = self.subject_text(entry.slots, position, expression);
                added.requires.push((clause, text));
            }
        }
        if let Some((ValueType::Scalar(scalar_type, qualification), value)) = result {
            let subject = ScalarTerm::Value {
                id: value,
                scalar_type,
            };
            // The result's own name is refused in an authored clause of a
            // qualified result (`ResultTerm::Absent`); the domain's clause
            // names the value through its `self`.
            let exit = ClauseScope {
                result: ResultTerm::Absent("result in a domain predicate"),
                ..*exit
            };
            for (clause, expression) in self.of_value(id, &exit, qualification, &subject)? {
                added
                    .ensures
                    .push((clause, self.clause_text(&[], expression)));
            }
        }
        Ok(added)
    }

    /// A domain clause's text with the formal at `position` where the
    /// domain wrote `self`.
    fn subject_text(
        &self,
        slots: &[Slot],
        position: usize,
        expression: ExpressionHandle,
    ) -> ClauseText {
        let subject = self.clause_subject(expression);
        let renamed: Vec<Slot> = slots
            .iter()
            .enumerate()
            .map(|(at, slot)| Slot {
                symbol: if at == position {
                    subject
                } else {
                    SymbolHandle::invalid()
                },
                value_type: slot.value_type,
                assignable: slot.assignable,
                homed: slot.homed,
                erased: slot.erased,
                borrowed: slot.borrowed,
            })
            .collect();
        self.clause_text(&renamed, expression)
    }

    /// The symbol of the first name in a clause: a domain clause names only
    /// its `self` and literals, so that is the subject.
    fn clause_subject(&self, expression: ExpressionHandle) -> SymbolHandle {
        use typed_trees::expression::ExpressionNode;
        let table = &self.typed.expression_table;
        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            if !expression.is_valid() {
                continue;
            }
            match table.expression(expression) {
                ExpressionNode::Name(path) if path.symbol.is_valid() => return path.symbol,
                ExpressionNode::Binary(binary) => {
                    pending.push(binary.right);
                    pending.push(binary.left);
                }
                ExpressionNode::Unary(unary) => pending.push(unary.operand),
                _ => {}
            }
        }
        SymbolHandle::invalid()
    }

    /// The predicates `value as +D` proves, for the domains `added` to a
    /// scalar of `carrier`: each clause over the one value `ValueId(1)`,
    /// which lowering replaces by the parameter of the block the cast
    /// continues in (`canonical::substitute`). Beside them, the domain and
    /// the predicates as written.
    pub(super) fn cast_predicates(
        &self,
        id: MachineId,
        added: Qualification,
        carrier: ScalarType,
    ) -> Checked<(Vec<Proposition>, String)> {
        let Some(subject) = ValueId::new(1) else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("cast subject identity"),
            ));
        };
        let scope = ClauseScope {
            slots: &[],
            values: &[],
            bound: &[],
            owner: SymbolHandle::invalid(),
            result: ResultTerm::Absent("result in a domain predicate"),
            subject: None,
            formal_fields: false,
            mathematical: MathematicalTerms::Refused(
                "mathematical comparison in a domain predicate",
            ),
        };
        let subject = ScalarTerm::Value {
            id: subject,
            scalar_type: carrier,
        };
        let stated = self
            .predicates(id, added)?
            .iter()
            .map(|predicate| {
                format!(
                    "domain `{}` requires `{}`",
                    predicate.domain,
                    self.typed
                        .expression_table
                        .display_name(predicate.expression)
                )
            })
            .collect::<Vec<_>>()
            .join(" and ");
        let clauses = self
            .of_value(id, &scope, added, &subject)?
            .into_iter()
            .map(|(clause, _)| clause)
            .collect();
        Ok((clauses, stated))
    }

    /// Reads the predicates of a subject's domain declaration as a cast
    /// would state them, so a declaration no type names is still refused
    /// when the route cannot read it.
    pub(super) fn declared_predicates(
        &mut self,
        id: MachineId,
        definition: &typed_trees::domain::DomainDefinition,
    ) -> Checked<()> {
        if !has_predicates(definition) {
            return Ok(());
        }
        // `domain [u8]::Utf8 requires valid_utf8(self)`: a primitive byte
        // predicate of a byte slice is decided by a view's bytes, where a
        // field of the domain is stored (`read::bytes`), not stated as a
        // clause.
        if super::bytes::primitive_byte_domain(self.typed, definition, id) {
            return Ok(());
        }
        let carrier = self.value_type(id, definition.target_type)?;
        self.declared_domain(id, definition.symbol, carrier)?;
        let (ValueType::Scalar(scalar_type, _), Some(domains)) = (
            carrier,
            crate::body::DomainSet::EMPTY.with(definition.symbol),
        ) else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Declaration("domain with predicates on a carrier that is not a scalar"),
            ));
        };
        self.cast_predicates(
            id,
            Qualification {
                policy: None,
                domains,
            },
            scalar_type,
        )
        .map(|_| ())
    }

    /// What a field's domains add to the `requires` of the data type that
    /// declares it: each predicate over the field at `path` beneath the
    /// owner's `self`.
    pub(super) fn field_predicates(
        &self,
        id: MachineId,
        path: &[CanonicalStructuralPathSegment],
        value_type: ValueType,
    ) -> Checked<Vec<Proposition>> {
        let qualification = value_type.qualification();
        // A byte view's domains are its primitive byte predicates, which
        // the lowered form carries on the field's own type
        // (`StructuralFieldType::PredicateByteSequence`) and the checker
        // judges at every store from the bytes supplied, not as a clause of
        // the owner (`byte_view_domains`).
        if let ValueType::Record(row, _, _) = value_type
            && matches!(
                self.data_type(id, row)?.shape,
                crate::StorageShape::ByteSequence(_)
            )
        {
            return Ok(Vec::new());
        }
        let predicates = self.predicates(id, qualification)?;
        if predicates.is_empty() {
            return Ok(Vec::new());
        }
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Declaration(what)));
        let Some(plain @ ValueType::Scalar(..)) = value_type.qualified(Qualification::NONE) else {
            return unserved("field of a data type in a domain with predicates");
        };
        let Some(root) = crate::body::self_place() else {
            return unserved("self place identity");
        };
        let mut clauses = Vec::with_capacity(predicates.len());
        for predicate in predicates {
            let scope = ClauseScope {
                slots: &[],
                values: &[],
                bound: &[],
                owner: SymbolHandle::invalid(),
                result: ResultTerm::Absent("result in a domain predicate"),
                formal_fields: false,
                mathematical: MathematicalTerms::Refused(
                    "mathematical comparison in a domain predicate",
                ),
                subject: Some(DataSubject {
                    value_type: plain,
                    symbol: predicate.subject,
                    root,
                    path,
                    bare_fields: false,
                }),
            };
            clauses.push(self.clause(id, &scope, predicate.expression)?);
        }
        Ok(clauses)
    }
}

/// The clauses the domains of a callable's formals and result add to its
/// contract, each with its text.
#[derive(Default)]
pub(super) struct SignaturePredicates {
    pub(super) requires: Vec<(Proposition, ClauseText)>,
    pub(super) ensures: Vec<(Proposition, ClauseText)>,
}
