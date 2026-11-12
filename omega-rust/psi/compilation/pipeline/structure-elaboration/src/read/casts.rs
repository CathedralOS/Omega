/*
The cast family of the reader: `operand as T` between two integer carriers
with no domain on either side.

`wiki/spec/language/domains.md`, "Carrier casts": "The operand of a carrier
cast must carry no domain." A cast that names a domain, lands in a
policy-qualified type, re-views storage (`recast`) or converts an anonymous
expression is refused here by what the typed trees record, never by its
spelling: the domain symbol, the arithmetic domain, the cast form and the
target's type node.
*/

use semantic_vocabulary::{MachineId, ScalarType};
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::TableCastExpression;

use super::{Program, Slot};
use crate::body::{Expression, ExpressionKind, ValueType};
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
        if cast.domain != numerics::arithmetic::ArithmeticDomain::Exact {
            return unserved("cast into a non-exact arithmetic domain");
        }
        // A constrained target (`T in D`) is refused by the type route.
        let target = self.scalar_type(id, cast.target_type)?;
        if cast.result_type.is_valid() && self.scalar_type(id, cast.result_type)? != target {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("cast result type"),
            ));
        }
        // An anonymous operand has no carrier of its own; landing it is the
        // numeric landing rules' work, outside this family.
        let operand = self.expression(id, slots, cast.value, None)?;
        let (ScalarType::Integer(from), ScalarType::Integer(to)) = (
            match operand.result {
                Some(ValueType::Scalar(scalar_type)) => scalar_type,
                Some(ValueType::Record(_) | ValueType::Rigid(_)) | None => {
                    return unserved("cast of a non-scalar");
                }
            },
            target,
        ) else {
            return unserved("cast between carriers that are not integers");
        };
        // The same carrier converts nothing.
        if from == to {
            return Ok(operand);
        }
        if !from.can_widen_to(to) && !from.can_exact_cast_to(to) {
            return unserved("cast with no exact conversion");
        }
        Ok(Expression {
            kind: ExpressionKind::Cast(Box::new(operand)),
            result: Some(ValueType::Scalar(target)),
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
        for domain in [
            ArithmeticDomain::Wrapping,
            ArithmeticDomain::Saturating,
            ArithmeticDomain::Trapping,
        ] {
            assert_eq!(
                refused(TableCastExpression { domain, ..plain() }),
                "expression cast into a non-exact arithmetic domain"
            );
        }
        // A plain cast with no target type is refused by the type route.
        assert_eq!(refused(plain()), "type missing type");
    }
}
