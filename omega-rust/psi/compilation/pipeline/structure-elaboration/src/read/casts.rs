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
use crate::body::{Expression, ExpressionKind, Policy, ValueType};
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
            (ValueType::Scalar(carrier @ ScalarType::Integer(_)), Some(policy)) => {
                ValueType::Qualified(carrier, policy)
            }
            (target @ (ValueType::Scalar(_) | ValueType::Qualified(..)), None) => target,
            (ValueType::Scalar(_) | ValueType::Qualified(..), Some(_)) => {
                return unserved("cast that names a policy for a type that cannot carry one");
            }
            (ValueType::Record(_) | ValueType::Rigid(_), _) => {
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
        let operand = self.expression(id, slots, cast.value, landing.map(ValueType::Scalar))?;
        let source_type = match operand.result {
            Some(source_type @ (ValueType::Scalar(_) | ValueType::Qualified(..))) => source_type,
            Some(ValueType::Record(_) | ValueType::Rigid(_)) | None => {
                return unserved("cast of a non-scalar");
            }
        };
        // domains.md, "Carrier casts": "The operand of a carrier cast must
        // carry no domain", and of a qualified value moved to another
        // carrier, "Rejected: each of these changes the carrier of a
        // qualified value".
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
                result: Some(ValueType::Scalar(carrier)),
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
