/*
The cast family of the reader: `operand as T` between two integer carriers
with no domain on either side, and the cast that adds or removes the
`Wrapping` or `Saturating` policy of an integer while keeping its carrier
(`x as u32 in Wrapping`, `w as u32`). A cast from a plain integer of another
carrier into a policy is the exact carrier conversion, then the policy.

`wiki/spec/language/domains.md`, "Carrier casts": "The operand of a carrier
cast must carry no domain." A cast that names a declared domain, lands in
`Trapping`, changes the carrier of an integer that already carries a policy,
re-views
storage (`recast`) or converts an anonymous expression is refused here by
what the typed trees record, never by its spelling: the domain symbol, the
arithmetic domain, the cast form and the target's type node.
*/

use semantic_vocabulary::{MachineId, ScalarType};
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::TableCastExpression;

use super::{Program, Slot};
use crate::body::{Expression, ExpressionKind, Policy, Qualification, ValueType};
use crate::{SourceOrigin, Unserved, refuse};

impl Program<'_> {
    pub(super) fn cast(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        cast: &TableCastExpression,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        if let Some(entry) = cast.form.entry() {
            return self.domain_entry(id, slots, cast, entry, source);
        }
        if cast.form != language_core::CastForm::Value {
            return unserved("recast");
        }
        if cast.semantic_domain_symbol.is_valid() || !cast.semantic_domain.is_empty() {
            return unserved("cast naming a domain");
        }
        // The typed trees hold `value as T in Wrapping` as the carrier `T`
        // with the policy beside it (`cast.domain`); a target written as a
        // constrained type goes through the type route like any other.
        use numerics::arithmetic::ArithmeticDomain;
        let named = match cast.domain {
            ArithmeticDomain::Exact => None,
            ArithmeticDomain::Wrapping => Some(Policy::Wrapping),
            ArithmeticDomain::Saturating => Some(Policy::Saturating),
            ArithmeticDomain::Trapping => return unserved("cast into Trapping"),
        };
        let target = match (self.value_type(id, cast.target_type)?, named) {
            (
                ValueType::Scalar(carrier @ ScalarType::Integer(_), Qualification::NONE),
                Some(policy),
            ) => ValueType::Scalar(carrier, Qualification::of_policy(policy)),
            (target @ ValueType::Scalar(..), None) => target,
            (ValueType::Scalar(..), Some(_)) => {
                return unserved("cast that names a policy for a type that cannot carry one");
            }
            (ValueType::Record(_, _) | ValueType::Rigid(_), _) => {
                return unserved("cast to a type that is not a scalar");
            }
        };
        if cast.result_type.is_valid() && self.value_type(id, cast.result_type)? != target {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("cast result type"),
            ));
        }
        // An anonymous operand has no carrier of its own; landing it is the
        // numeric landing rules' work, outside this family. The one such
        // operand read here is a literal cast into a policy: `5 as u32 in
        // Wrapping` is `(5 as u32)` with the policy added (domains.md,
        // "Carrier casts": "The cast may add domains to the result"), so
        // the literal lands in the carrier the cast names, in range or not
        // at all (numeric_values.md: no policy destination can "repair an
        // out-of-range initial value").
        let landing = if named.is_some() && self.wholly_anonymous(cast.value) {
            target.scalar()
        } else {
            None
        };
        let operand = self.expression(id, slots, cast.value, landing.map(ValueType::plain))?;
        let source_type = match operand.result {
            Some(source_type @ ValueType::Scalar(..)) => source_type,
            Some(ValueType::Record(_, _) | ValueType::Rigid(_)) | None => {
                return unserved("cast of a non-scalar");
            }
        };
        // domains.md, "Carrier casts": "The operand of a carrier cast must
        // carry no domain", and of a qualified value moved to another
        // carrier, "Rejected: each of these changes the carrier of a
        // qualified value". A declared domain is added and removed by an
        // entry (`domain_entry`), never by this cast.
        if !source_type.qualification().domains.is_empty()
            || !target.qualification().domains.is_empty()
        {
            return unserved("carrier cast of or into a type that carries a declared domain");
        }
        if source_type.policy().is_some() && source_type.scalar() != target.scalar() {
            return unserved("cast that changes the carrier of a policy-qualified integer");
        }
        let (Some(ScalarType::Integer(from)), Some(carrier @ ScalarType::Integer(to))) =
            (source_type.scalar(), target.scalar())
        else {
            return unserved("cast between carriers that are not integers");
        };
        // The carrier first: the same carrier converts nothing, and any
        // other is the one exact conversion, whatever policy the cast then
        // adds ("`plain as i64 +Km` abbreviates `(plain as i64) as +Km`").
        // No policy makes it a truncation or a clamp: "`as` never calls
        // user code, rounds, saturates, traps".
        let converted = if from == to {
            operand
        } else {
            if !from.can_widen_to(to) && !from.can_exact_cast_to(to) {
                return unserved("cast with no exact conversion");
            }
            Expression {
                kind: ExpressionKind::Cast(Box::new(operand)),
                result: Some(ValueType::Scalar(carrier, Qualification::NONE)),
                source,
            }
        };
        // Then the policy. Adding or removing one keeps the value:
        // numeric_values.md, "Adding and removing a policy domain ...
        // change future operations, not existing payloads".
        if converted.result == Some(target) {
            return Ok(converted);
        }
        Ok(Expression {
            kind: ExpressionKind::Requalify(Box::new(converted)),
            result: Some(target),
            source,
        })
    }
}

impl Program<'_> {
    /// One entry of a domain cast, `value as =Keep -Remove +Add`
    /// (`wiki/spec/language/domains.md`, "Domain-only casts"). Typing
    /// accounted for the whole cast against the operand's declared type and
    /// left this entry with the type after it. The reader types the operand
    /// itself, so it does not take that accounting on trust: the entry must
    /// do to the operand's type exactly what its sign says.
    ///
    /// | Entry | The operand's type | The result's type |
    /// | `=D` | carries `D` | carries `D` |
    /// | `-D` | carries `D` | does not carry `D` |
    /// | `+D` | does not carry `D` | carries `D` |
    ///
    /// The value is never changed: "`as` never calls user code, rounds,
    /// saturates, traps".
    fn domain_entry(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        cast: &TableCastExpression,
        entry: language_core::cast_form::CastEntry,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        use language_core::cast_form::CastEntryKind;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        let mistyped = |what| Err(refuse(id, Construct::Machine, Unserved::Typing(what)));
        // A cast typing deferred is a row on its machine, raised before any
        // body is read; an entry without its type here was never accounted.
        if !cast.result_type.is_valid() {
            return unserved("domain cast typing did not account for");
        }
        let result = self.value_type(id, cast.result_type)?;
        // A literal lands in the carrier of the type the cast gives it
        // (domains.md, "Literals and expected types": "An expected type
        // selects the carrier in which an anonymous literal lands").
        let landing = if self.wholly_anonymous(cast.value) {
            result.scalar().map(ValueType::plain)
        } else {
            None
        };
        let operand = self.expression(id, slots, cast.value, landing)?;
        let Some(operand_type) = operand.result else {
            return unserved("domain cast of a Unit value");
        };
        let (before, after) = (operand_type.qualification(), result.qualification());
        if operand_type.qualified(Qualification::NONE) != result.qualified(Qualification::NONE)
            || operand_type.qualified(Qualification::NONE).is_none()
        {
            return mistyped("domain cast entry that changes the carrier");
        }
        // The one member the entry names: what the two sets differ by.
        let policy_differs = before.policy != after.policy;
        let (wider, narrower) = match entry.kind {
            CastEntryKind::Keep => {
                if before != after {
                    return mistyped("kept domain that changes the type");
                }
                return Ok(operand);
            }
            CastEntryKind::Remove => (before, after),
            CastEntryKind::Add => (after, before),
        };
        let mut named = wider
            .domains
            .members()
            .filter(|domain| !narrower.domains.contains(*domain));
        let domain = named.next();
        let one_member = named.next().is_none()
            && narrower.domains.is_subset_of(&wider.domains)
            && match (policy_differs, domain) {
                (true, None) => wider.policy.is_some() && narrower.policy.is_none(),
                (false, Some(_)) => true,
                _ => false,
            };
        if !one_member {
            return mistyped("domain cast entry that does not change exactly the domain it names");
        }
        if let Some(domain) = domain {
            match entry.kind {
                CastEntryKind::Add => {
                    if cast.semantic_domain_symbol != domain {
                        return mistyped("added domain that is not the one the entry selected");
                    }
                    self.issue(id, domain)?;
                }
                // "Removal also never increases authority. Owned claims
                // must be consumed or transferred, not cast away ... Access
                // policies survive whenever they are needed to prevent
                // authority amplification, and a view that cannot retain
                // them rejects." What a restricted domain's removal must
                // leave behind is not kept on the plain value here, so only
                // a domain anyone may add again is removed.
                _ => {
                    let free = self
                        .typed
                        .domain_definition(domain)
                        .is_some_and(|definition| {
                            definition.issuance()
                                == typed_trees::domain::DomainIssuance::Unrestricted
                        });
                    if !free {
                        return unserved("removal of a domain with restricted issuance");
                    }
                }
            }
        }
        Ok(Expression {
            kind: ExpressionKind::Requalify(Box::new(operand)),
            result: Some(result),
            source,
        })
    }

    /// Whether the machine being read may write `as +D` for the declared
    /// domain `domain`, by who the declaration says may issue it
    /// (`DomainDefinition::issuance`). This is the one place that judgment
    /// is made. domains.md, "Domain-only casts":
    ///
    /// | Domain | Obligation of `+D` |
    /// | Has predicates | Prove them for this value. |
    /// | Has neither predicates nor establishment routes | None beyond carrier validity |
    /// | Has establishment routes | Retain exact existing or authorized evidence; a cast cannot mint it. |
    ///
    /// A route names "an exact trait requirement or concrete machine as an
    /// alternative authorized origin", and "An exact machine is authorized
    /// during its invocation": the machine whose body holds the cast is one
    /// of the routes, or the cast is refused. A requirement route
    /// authorizes "the invocation through that requirement and its selected
    /// implementation", which this reader does not follow, so it admits no
    /// machine here. A policy states its issuers in its plan, which no pass
    /// evaluates (`DomainIssuance::Planned`): nobody is shown to be one.
    ///
    /// Authorization proves no predicate ("Policy authorization cannot
    /// prove those predicates"). Nothing here states a domain's predicates
    /// as an obligation on the cast's operand yet, so a domain with
    /// predicates is refused whoever asks.
    fn issue(&mut self, id: MachineId, domain: symbols::SymbolHandle) -> Checked<()> {
        use language_semantics::DomainEstablishmentRoute;
        use typed_trees::domain::DomainIssuance;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        let Some(definition) = self.typed.domain_definition(domain) else {
            return unserved("cast that adds a domain with no declaration");
        };
        match definition.issuance() {
            DomainIssuance::Unrestricted => {}
            DomainIssuance::Only(routes) => {
                let machine =
                    *terminal_check_tables::row(&self.symbols, id, id, Construct::Machine)?;
                let routed = routes.iter().any(|route| {
                    matches!(route, DomainEstablishmentRoute::ExactMachine { machine: origin }
                        if *origin == machine)
                });
                if !routed {
                    return unserved("cast that adds a domain outside its establishment routes");
                }
            }
            DomainIssuance::Planned(_) => {
                return unserved("cast that adds a domain whose issuers are in an access plan");
            }
        }
        if super::authority::has_predicates(definition) {
            return unserved("cast that adds a domain with predicates");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use language_core::{CastForm, ReferenceAccess};
    use numerics::arithmetic::ArithmeticDomain;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::types::TypeReferenceHandle;

    fn plain() -> TableCastExpression {
        TableCastExpression {
            value: ExpressionHandle::invalid(),
            target_type: TypeReferenceHandle::invalid(),
            result_type: TypeReferenceHandle::invalid(),
            target_label: arena::HandleSpan::empty(),
            domain: ArithmeticDomain::Exact,
            semantic_domain: arena::HandleSpan::empty(),
            semantic_domain_arguments: arena::HandleSpan::empty(),
            semantic_domain_symbol: SymbolHandle::invalid(),
            semantic_domain_id: language_semantics::SemanticDomainId::NULL,
            form: CastForm::Value,
        }
    }

    fn refused(cast: TableCastExpression) -> String {
        let mut typed = TypedTrees::default();
        let handle = typed.expression_table.insert(ExpressionNode::Cast(cast));
        let mut program = Program::new(&typed, false);
        let refusal = program
            .expression(MachineId::new(1).unwrap(), &[], handle, None)
            .unwrap_err();
        assert_eq!(
            refusal.reason,
            terminal_check_tables::RefusalReason::Unsupported
        );
        refusal.detail
    }

    /// Each cast outside the carrier family is refused by what the typed
    /// trees record about it, before its operand or target is read.
    #[test]
    fn a_cast_that_is_not_a_plain_carrier_cast_is_refused_by_name() {
        assert_eq!(
            refused(TableCastExpression {
                form: CastForm::Recast(ReferenceAccess::Shared),
                ..plain()
            }),
            "expression recast"
        );
        assert_eq!(
            refused(TableCastExpression {
                semantic_domain_symbol: SymbolHandle::from_arena_index(3),
                ..plain()
            }),
            "expression cast naming a domain"
        );
        // A cast into Trapping is refused before its target is read.
        assert_eq!(
            refused(TableCastExpression {
                domain: ArithmeticDomain::Trapping,
                ..plain()
            }),
            "expression cast into Trapping"
        );
        // A plain cast with no target type is refused by the type route,
        // and so is one into Wrapping or Saturating: the policy is read,
        // then the carrier it qualifies.
        for domain in [
            ArithmeticDomain::Exact,
            ArithmeticDomain::Wrapping,
            ArithmeticDomain::Saturating,
        ] {
            assert_eq!(
                refused(TableCastExpression { domain, ..plain() }),
                "type missing type"
            );
        }
    }
}
