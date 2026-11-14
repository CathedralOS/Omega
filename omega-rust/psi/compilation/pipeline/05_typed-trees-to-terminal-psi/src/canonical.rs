//! Source diagnostics around the shared canonical contract operations.

use semantic_vocabulary::{CrashCause, MachineId, Proposition, ScalarTerm, ValueId};
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::{CrashRouteBucket, CrashRouteGuard};

pub(crate) use terminal_codec::contract_forms::RepeatedClauses as Repeated;

fn refusal(id: MachineId, detail: String) -> Refusal {
    Refusal {
        reason: RefusalReason::Unsupported,
        machine: id,
        concerns: Construct::Machine,
        detail: format!("contract {detail}"),
    }
}

pub(crate) fn equal(left: ScalarTerm, right: ScalarTerm, id: MachineId) -> Checked<Proposition> {
    terminal_codec::contract_forms::equal(left, right).map_err(|detail| refusal(id, detail))
}

pub(crate) fn clauses(
    propositions: Vec<Proposition>,
    repeated: Repeated,
    id: MachineId,
) -> Checked<Vec<Proposition>> {
    terminal_codec::contract_forms::clauses(propositions, repeated)
        .map_err(|detail| refusal(id, detail))
}

pub(crate) fn conjunction(conjuncts: Vec<Proposition>, id: MachineId) -> Checked<Proposition> {
    terminal_codec::contract_forms::conjunction(conjuncts).map_err(|detail| refusal(id, detail))
}

pub(crate) fn routes(
    alternatives: impl IntoIterator<Item = (CrashCause, CrashRouteGuard)>,
) -> Vec<CrashRouteBucket> {
    terminal_codec::contract_forms::routes(alternatives)
}

pub fn substituted_routes(
    routes: &[CrashRouteBucket],
    actuals: &[ValueId],
    id: MachineId,
) -> Checked<Vec<CrashRouteBucket>> {
    terminal_codec::contract_forms::substituted_routes(routes, actuals)
        .map_err(|detail| refusal(id, detail))
}

pub(crate) fn substitute(
    proposition: &Proposition,
    actuals: &[ValueId],
    id: MachineId,
) -> Checked<Proposition> {
    terminal_codec::contract_forms::substitute(proposition, actuals)
        .map_err(|detail| refusal(id, detail))
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerMathTerm, IntegerSign, IntegerType, ScalarType};

    #[test]
    fn clause_substitution_preserves_nested_operations_and_actual_identity() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let scalar = ScalarType::Integer(integer);
        let value = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar);
        let clause = Proposition::IntegerMathLessThan(
            IntegerMathTerm::embed(ScalarTerm::WrappingIntegerAdd {
                scalar_type: integer,
                left: Box::new(value(1)),
                right: Box::new(value(2)),
            })
            .unwrap(),
            IntegerMathTerm::embed(value(2)).unwrap(),
        );
        let actuals = [ValueId::new(8).unwrap(), ValueId::new(3).unwrap()];
        let mut expected = clause.clone();
        expected.remap_value_ids(|formal| actuals[(formal.get() - 1) as usize]);
        assert_eq!(
            substitute(&clause, &actuals, MachineId::new(1).unwrap()).unwrap(),
            expected
        );
        assert!(substitute(&clause, &actuals[..1], MachineId::new(1).unwrap()).is_err());
    }

    #[test]
    fn clause_substitution_reorders_equalities_without_rewriting_their_operands() {
        let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
        let value = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar);
        let mathematical = |raw| IntegerMathTerm::embed(value(raw)).unwrap();
        let clause = Proposition::Conjunction(vec![
            Proposition::Equal(value(1), value(2)),
            Proposition::IntegerMathEqual(mathematical(1), mathematical(2)),
        ]);
        let actuals = [ValueId::new(8).unwrap(), ValueId::new(3).unwrap()];
        let machine = MachineId::new(1).unwrap();
        assert_eq!(
            substitute(&clause, &actuals, machine).unwrap(),
            conjunction(
                vec![
                    equal(value(3), value(8), machine).unwrap(),
                    Proposition::IntegerMathEqual(mathematical(3), mathematical(8)),
                ],
                machine,
            )
            .unwrap()
        );
    }
}
