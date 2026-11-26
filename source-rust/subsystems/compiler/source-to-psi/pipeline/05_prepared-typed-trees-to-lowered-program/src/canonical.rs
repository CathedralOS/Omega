//! Source diagnostics around the shared canonical contract operations.

use semantic_vocabulary::{CrashCause, MachineId, Proposition, ScalarTerm, ValueId};
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::{CrashRouteBucket, CrashRouteGuard};

pub(crate) use terminal_codec::contract_forms::FormalStorage;
pub(crate) use terminal_codec::contract_forms::RepeatedClauses as Repeated;

fn refusal(id: MachineId, detail: String) -> Refusal {
    Refusal {
        source_span: None,
        reason: RefusalReason::Unsupported,
        site: terminal_check_tables::RefusalSite::Machine(id, Construct::Machine),
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

/// Keep a clause's source description beside its canonical proposition.
/// Associated data never participates in proposition identity or ordering.
pub(crate) fn associated_clauses<T>(
    propositions: Vec<(Proposition, T)>,
    repeated: Repeated,
    id: MachineId,
) -> Checked<Vec<(Proposition, T)>> {
    let mut keyed = Vec::with_capacity(propositions.len());
    for (proposition, associated) in propositions {
        let key = terminal_codec::canonical_proposition_order_key(&proposition)
            .map_err(|_| refusal(id, "unencodable proposition".into()))?;
        keyed.push((key, proposition, associated));
    }
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    if matches!(repeated, Repeated::Refuse) && keyed.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(refusal(id, "duplicate clause".into()));
    }
    keyed.dedup_by(|later, earlier| later.0 == earlier.0);
    Ok(keyed
        .into_iter()
        .map(|(_, proposition, associated)| (proposition, associated))
        .collect())
}

pub(crate) fn conjunction(conjuncts: Vec<Proposition>, id: MachineId) -> Checked<Proposition> {
    terminal_codec::contract_forms::conjunction(conjuncts).map_err(|detail| refusal(id, detail))
}

pub(crate) fn routes(
    alternatives: impl IntoIterator<Item = (CrashCause, CrashRouteGuard)>,
) -> Vec<CrashRouteBucket> {
    terminal_codec::contract_forms::routes(alternatives)
}

/// A call's crash continuations: the callee's published routes over the
/// call's scalar actuals and the storage each structural actual stands for.
/// The wire owner states them, so the checker's retained-row comparison
/// restates them by the same rule.
pub fn substituted_routes(
    routes: &[CrashRouteBucket],
    actuals: &[ValueId],
    places: &[Option<FormalStorage>],
    id: MachineId,
) -> Checked<Vec<CrashRouteBucket>> {
    terminal_codec::contract_forms::substituted_routes(routes, actuals, places)
        .map_err(|detail| refusal(id, detail))
}

pub(crate) fn substitute(
    proposition: &Proposition,
    actuals: &[ValueId],
    values: &terminal_check_tables::MachineValues,
    id: MachineId,
) -> Checked<Proposition> {
    substitute_parameters(proposition, actuals, &[], values, id)
}

/// Restate a loop header's or record cast's contract over its actual scalar
/// and storage formals. Each incoming edge still owes the resulting predicate,
/// so the restatement keeps byte predicates and float meaning sources the
/// lowered value table traces. The wire owner's one binding renames the
/// formals, as it does for a call's crash continuation.
pub(crate) fn substitute_parameters(
    proposition: &Proposition,
    actuals: &[ValueId],
    places: &[Option<FormalStorage>],
    values: &terminal_check_tables::MachineValues,
    id: MachineId,
) -> Checked<Proposition> {
    terminal_codec::contract_forms::substitute_formals(
        proposition,
        actuals,
        places,
        &mut LoweredSources { values },
    )
    .map_err(|detail| refusal(id, detail))
}

/// Substitution can reverse equality order or make conjuncts identical. The
/// wire owner restores the artifact's canonical form for every producer.
pub(crate) fn canonical_clause(proposition: Proposition, id: MachineId) -> Checked<Proposition> {
    terminal_codec::contract_forms::canonical_clause(proposition)
        .map_err(|detail| refusal(id, detail))
}

/// The sources a lowered value table traces for a restated clause.
struct LoweredSources<'a> {
    values: &'a terminal_check_tables::MachineValues,
}

impl terminal_codec::contract_forms::RestatedSources for LoweredSources<'_> {
    fn float_meaning_source(&mut self, source: &ScalarTerm) -> Result<(), &'static str> {
        use terminal_check_tables::values::ValueOrigin;
        match source {
            ScalarTerm::IeeeFloatLiteral(_) => Ok(()),
            ScalarTerm::Value { id, scalar_type } => {
                let row = self
                    .values
                    .get(*id)
                    .ok_or("float meaning restatement names no lowered value")?;
                if row.scalar_type != *scalar_type || row.rigid.is_some() {
                    return Err("float meaning restatement changes its lowered value type");
                }
                match row.origin {
                    ValueOrigin::MachineParameter { .. }
                    | ValueOrigin::BlockParameter { .. }
                    | ValueOrigin::Operation(_) => Ok(()),
                    ValueOrigin::MachineResult
                    | ValueOrigin::StorageContent { .. }
                    | ValueOrigin::StorageCase { .. } => {
                        Err("float meaning restatement has no executable scalar producer")
                    }
                }
            }
            _ => Err("float meaning restatement needs an exact scalar producer or literal"),
        }
    }

    fn keeps_byte_predicates(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerMathTerm, IntegerSign, IntegerType, ScalarType};

    fn values(scalar_type: ScalarType) -> terminal_check_tables::MachineValues {
        let mut values = terminal_check_tables::MachineValues::default();
        for position in 0..8 {
            values.push(terminal_check_tables::values::ValueRow {
                scalar_type,
                origin: terminal_check_tables::values::ValueOrigin::MachineParameter { position },
                rigid: None,
            });
        }
        values
    }

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
            substitute(
                &clause,
                &actuals,
                &values(scalar),
                MachineId::new(1).unwrap()
            )
            .unwrap(),
            expected
        );
        assert!(
            substitute(
                &clause,
                &actuals[..1],
                &values(scalar),
                MachineId::new(1).unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn restated_indexed_contract_renames_the_storage_and_the_selector() {
        use semantic_vocabulary::{PlaceId, ScalarReadProjection};
        let ty = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let clause = |root, selector| {
            let index = ScalarTerm::value(ValueId::new(selector).unwrap(), ty);
            Proposition::Equal(
                ScalarTerm::StructuralRead {
                    root: PlaceId::new(root).unwrap(),
                    path: vec![ScalarReadProjection::Index(Box::new(index.clone()))],
                    scalar_type: ty,
                },
                index,
            )
        };
        let machine = MachineId::new(1).unwrap();
        let actuals = [ValueId::new(3).unwrap()];
        let places = [Some(FormalStorage::whole(PlaceId::new(4).unwrap()))];
        let cells = values(ty);
        assert_eq!(
            substitute_parameters(&clause(1, 1), &actuals, &places, &cells, machine).unwrap(),
            canonical_clause(clause(4, 3), machine).unwrap(),
        );
        assert!(substitute_parameters(&clause(2, 1), &actuals, &places, &cells, machine).is_err());
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
            substitute(&clause, &actuals, &values(scalar), machine).unwrap(),
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
    #[test]
    fn meaning_restatement_requires_the_actual_minted_value_and_format() {
        use semantic_vocabulary::{ClauseIdentity, IeeeFloatFormat};
        let scalar = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
        let clause = Proposition::DeclarationIdentity(
            ClauseIdentity::float_conversion_roundtrip(
                ScalarTerm::value(ValueId::new(1).unwrap(), scalar),
                IeeeFloatFormat::Binary32,
            )
            .unwrap(),
        );
        let actuals = [ValueId::new(8).unwrap()];
        let machine = MachineId::new(1).unwrap();
        let mut roster = values(scalar);
        let mapped = substitute(&clause, &actuals, &roster, machine).unwrap();
        let Proposition::DeclarationIdentity(mapped) = mapped else {
            unreachable!()
        };
        assert_eq!(
            mapped.float_sources().collect::<Vec<_>>(),
            [&ScalarTerm::value(actuals[0], scalar)]
        );
        assert!(substitute(&clause, &[ValueId::new(9).unwrap()], &roster, machine).is_err());
        roster.get_mut(actuals[0]).unwrap().scalar_type =
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
        assert!(substitute(&clause, &actuals, &roster, machine).is_err());
        let row = roster.get_mut(actuals[0]).unwrap();
        row.scalar_type = scalar;
        row.origin = terminal_check_tables::values::ValueOrigin::MachineResult;
        assert!(substitute(&clause, &actuals, &roster, machine).is_err());
    }
}
