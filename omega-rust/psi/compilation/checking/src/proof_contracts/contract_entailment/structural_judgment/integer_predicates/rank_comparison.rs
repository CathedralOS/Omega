//! Compare invocation ranks from the ordinary normal-return observation ledger.
//! Operand snapshots and public result equations retain the same identities as
//! every other runtime predicate. Declaration application formation belongs to
//! the shared arithmetic engine; no rank interval justifies its own formation.

use super::{BigInt, IntegerObservations, StructuralJudge, StructuralTerm};
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

pub(in crate::proof_contracts::contract_entailment) struct CapturedIntegerRank {
    pub input: StructuralTerm,
    pub primitive: PrimitiveType,
    pub measure: SymbolHandle,
}

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn compare_captured_integer_ranks(
        &self,
        source: &CapturedIntegerRank,
        destination: &CapturedIntegerRank,
    ) -> Result<bool, &'static str> {
        let mut observations = IntegerObservations::empty(self.program);
        let source_input = self
            .integer_value(&source.input, source.primitive, &mut observations, 0)
            .ok_or("the saved rank input has no exact integer observation")?;
        let destination_input = self
            .integer_value(
                &destination.input,
                destination.primitive,
                &mut observations,
                0,
            )
            .ok_or("the arriving rank input has no exact integer observation")?;
        let required = 0..observations.operations.len();
        let mut premises = self.observed_integer_premises(&mut observations);
        if !observations.install_premises(&mut premises) || !observations.admitted(required) {
            return Err("a captured rank operand has unformed arithmetic");
        }
        let engine = &mut observations.engine;
        let mut form = |rank: &CapturedIntegerRank, input| {
            if rank.measure.is_valid() {
                engine.form_declared_integer_application(rank.measure, &input)
            } else {
                Some(input)
            }
        };
        let source = form(source, source_input)
            .ok_or("the saved rank application has no independent formation")?;
        let destination = form(destination, destination_input)
            .ok_or("the arriving rank application has no independent formation")?;
        let source = engine.substituted(&source);
        let destination = engine.substituted(&destination);
        if !engine.prove_at_least(&source, &BigInt::zero())
            || !engine.prove_at_least(&destination, &BigInt::zero())
            || !engine.prove_at_least(&source.sub(&destination), &BigInt::zero())
        {
            return Err("captured ranks do not prove natural nonincrease");
        }
        Ok(engine.prove_at_least(&source.sub(&destination), &BigInt::from_u64(1)))
    }
}
