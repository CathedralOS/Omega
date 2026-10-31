//! Invocation ranks retain the denotation formed at their entry observation.
//! Its atom roster contains exact immutable normal-value terms. Later queries
//! rebind those terms simultaneously; they never replay entry storage or need
//! an entry predicate to remain a live fact after a write. The destination
//! application still forms independently at the completed operand frontier.

use super::{BigInt, IntegerObservations, Polynomial, StructuralJudge, StructuralTerm, atom_name};
use std::collections::BTreeMap;
use symbols::SymbolHandle;
use typed_trees::{expression::BinaryOperator, types::PrimitiveType};

pub(in crate::proof_contracts::contract_entailment) struct CapturedIntegerRank {
    pub input: StructuralTerm,
    pub primitive: PrimitiveType,
    pub measure: SymbolHandle,
}

/// Only `form_captured_integer_rank` constructs this record. The polynomial
/// and its exact observed atoms travel together with independently proved
/// natural/carrier bounds; a raw desired interval cannot construct one.
pub(in crate::proof_contracts::contract_entailment) struct FormedIntegerRank {
    value: Polynomial,
    atoms: Vec<(String, StructuralTerm, PrimitiveType)>,
    maximum: BigInt,
}

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn form_captured_integer_rank(
        &self,
        rank: &CapturedIntegerRank,
    ) -> Result<FormedIntegerRank, &'static str> {
        let mut observations = IntegerObservations::empty(self.program);
        let input = self
            .integer_value(&rank.input, rank.primitive, &mut observations, 0)
            .ok_or("the entry rank input has no exact integer observation")?;
        let required = 0..observations.operations.len();
        let mut premises = self.observed_integer_premises(&mut observations);
        if !observations.install_premises(&mut premises) || !observations.admitted(required) {
            return Err("the entry rank input has unformed arithmetic");
        }
        let engine = &mut observations.engine;
        let (value, primitive) = if rank.measure.is_valid() {
            let declaration = self
                .program
                .measures()
                .iter()
                .find(|definition| definition.symbol == rank.measure)
                .ok_or("the entry rank lost its selected measure declaration")?;
            let primitive = self
                .program
                .primitive_type_reference(declaration.return_type)
                .ok_or("the entry rank has no declared integer result carrier")?;
            let value = engine
                .form_declared_integer_application(rank.measure, &input)
                .ok_or_else(|| {
                    if crate::proof_contracts::contract_entailment::struct_trace() {
                        eprintln!("STRUCT rank-entry formation-declined measure={:?} input={:?} value={:?} bounds={:?}", rank.measure, rank.input, input, engine.bounds);
                    }
                    "the entry rank application has no independent formation"
                })?;
            (value, primitive)
        } else {
            (input, rank.primitive)
        };
        let value = engine.substituted(&value);
        if !engine.integer_value_lands(primitive, &value) {
            return Err("the entry rank has no independently proved result carrier bound");
        }
        if !engine.prove_at_least(&value, &BigInt::zero()) {
            return Err("the entry rank is not independently nonnegative");
        }
        let (_, maximum) = primitive
            .integer_bounds()
            .ok_or("the entry rank has no fixed integer carrier bounds")?;
        let mut atoms = Vec::new();
        for (position, term) in observations.atoms.into_iter().enumerate() {
            let name = atom_name(position);
            if !value
                .terms
                .keys()
                .any(|monomial| monomial.contains_key(&name))
            {
                continue;
            }
            let atom = Polynomial::atom(name.clone());
            let primitive = observations
                .carrier_bounds
                .iter()
                .find(|bounds| bounds.value == atom)
                .map(|bounds| bounds.primitive)
                .ok_or("the formed rank lost an observed atom's declared carrier")?;
            atoms.push((name, term, primitive));
        }
        if value
            .terms
            .keys()
            .flat_map(|monomial| monomial.keys())
            .any(|name| !atoms.iter().any(|(captured, _, _)| captured == name))
        {
            return Err("the formed rank denotation has an uncaptured integer atom");
        }
        Ok(FormedIntegerRank {
            value,
            atoms,
            maximum: BigInt::from_i128(maximum),
        })
    }

    pub(in crate::proof_contracts::contract_entailment) fn compare_captured_integer_ranks(
        &self,
        source: &FormedIntegerRank,
        destination: &CapturedIntegerRank,
    ) -> Result<bool, &'static str> {
        let mut observations = IntegerObservations::empty(self.program);
        let mut substitutions = BTreeMap::new();
        for (name, value, primitive) in &source.atoms {
            let value = self
                .integer_value(value, *primitive, &mut observations, 0)
                .ok_or("the saved rank lost an immutable atom observation")?;
            substitutions.insert(name.clone(), value);
        }
        let source_value =
            crate::proof_contracts::contract_entailment::inductive_judgment::apply_argument_map(
                &source.value,
                &substitutions,
            )
            .ok_or("the saved rank lost its complete observed atom roster")?;
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
        // These facts were proved at the source observation, before effects.
        // They constrain its saved produced value, never the destination input
        // or that application's still-pending intermediate operations.
        if !engine.install_hypotheses(vec![
            (
                BinaryOperator::GreaterOrEqual,
                source_value.clone(),
                Polynomial::constant(BigInt::zero()),
            ),
            (
                BinaryOperator::LessOrEqual,
                source_value.clone(),
                Polynomial::constant(source.maximum.clone()),
            ),
        ]) {
            return Err("the saved rank bounds could not enter the current observation scope");
        }
        let destination_value = if destination.measure.is_valid() {
            engine
                .form_declared_integer_application(destination.measure, &destination_input)
                .ok_or("the arriving rank application has no independent formation")?
        } else {
            destination_input
        };
        let source_value = engine.substituted(&source_value);
        let destination_value = engine.substituted(&destination_value);
        if !engine.prove_at_least(&destination_value, &BigInt::zero())
            || !engine.prove_at_least(&source_value.sub(&destination_value), &BigInt::zero())
        {
            return Err("captured ranks do not prove natural nonincrease");
        }
        Ok(engine.prove_at_least(&source_value.sub(&destination_value), &BigInt::from_u64(1)))
    }
}
