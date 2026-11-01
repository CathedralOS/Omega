//! Invocation ranks retain the denotation formed at their entry observation.
//! Its atom roster contains exact immutable normal-value terms. Later queries
//! rebind those terms simultaneously; they never replay entry storage or need
//! an entry predicate to remain a live fact after a write. The destination
//! application still forms independently at the completed operand frontier.

use super::{BigInt, IntegerObservations, Polynomial, StructuralJudge, StructuralTerm, atom_name};
use crate::proof_contracts::arithmetic_domains::BigIntInterval;
use crate::proof_contracts::contract_entailment::arithmetic_judgment::IntegerDenotation;
use std::collections::BTreeMap;
use symbols::SymbolHandle;
use typed_trees::{expression::BinaryOperator, types::PrimitiveType};

pub(in crate::proof_contracts::contract_entailment) struct CapturedIntegerRank {
    pub input: StructuralTerm,
    pub primitive: PrimitiveType,
    /// The selected relation's floor is a goal, not an input assumption.
    /// The independently formed word still supplies its own carrier bounds.
    pub minimum: BigInt,
    pub measure: SymbolHandle,
}

/// Only `form_captured_integer_rank` constructs this record. The polynomial
/// and its exact observed atoms travel together with independently proved
/// natural/carrier bounds; a raw desired interval cannot construct one.
pub(in crate::proof_contracts::contract_entailment) struct FormedIntegerRank {
    pub(in crate::proof_contracts::contract_entailment) application:
        Option<crate::IntegerApplicationFormation>,
    value: IntegerDenotation,
    atoms: Vec<(String, StructuralTerm, PrimitiveType)>,
    interval: BigIntInterval,
}

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn form_captured_integer_rank(
        &self,
        rank: &CapturedIntegerRank,
    ) -> Result<FormedIntegerRank, &'static str> {
        let mut observations = IntegerObservations::empty(self.program);
        observations.engine =
            super::Engine::strict_for_declaration(self.program, self.machine_symbol);
        let input = self
            .integer_value(&rank.input, rank.primitive, &mut observations, 0)
            .ok_or("the entry rank input has no exact integer observation")?;
        let required = 0..observations.operations.len();
        let mut premises = self.observed_integer_premises(&mut observations);
        if !observations.install_premises(&mut premises) || !observations.admitted(required) {
            return Err("the entry rank input has unformed arithmetic");
        }
        let engine = &mut observations.engine;
        let (value, primitive, application) = if rank.measure.is_valid() {
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
            (value.value, primitive, value.formation)
        } else {
            (input, rank.primitive, None)
        };
        let value = engine.substituted(&value);
        if !engine.integer_value_lands(primitive, &value) {
            return Err("the entry rank has no independently proved result carrier bound");
        }
        let (_, maximum) = primitive
            .integer_bounds()
            .ok_or("the entry rank has no fixed integer carrier bounds")?;
        let minimum = rank.minimum.clone();
        if !engine.prove_at_least(
            &value.sub(&Polynomial::constant(minimum.clone())),
            &BigInt::zero(),
        ) {
            return Err("the entry rank has no independently established lower bound");
        }
        let value = engine
            .capture_integer_denotation(&value)
            .ok_or("the formed rank has an incomplete opaque dependency graph")?;
        let mut atoms = Vec::new();
        for (position, term) in observations.atoms.into_iter().enumerate() {
            let name = atom_name(position);
            if !value.leaf_atoms().contains(&name) {
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
            .leaf_atoms()
            .iter()
            .any(|name| !atoms.iter().any(|(captured, _, _)| captured == name))
        {
            return Err("the formed rank denotation has an uncaptured integer leaf");
        }
        Ok(FormedIntegerRank {
            application,
            value,
            atoms,
            interval: BigIntInterval {
                low: Some(minimum),
                high: Some(BigInt::from_i128(maximum)),
            },
        })
    }

    /// One observation cut owns the entire ordered rank. Restore only bounds
    /// proved on the saved source values, then form every destination component
    /// before comparing. An earlier strict component may ignore a suffix's
    /// ordering, never its formation, ownership or carrier obligations.
    pub(in crate::proof_contracts::contract_entailment) fn compare_captured_integer_rank_sequences(
        &self,
        sources: &[FormedIntegerRank],
        destinations: &[CapturedIntegerRank],
    ) -> Result<bool, &'static str> {
        if sources.is_empty() || sources.len() != destinations.len() {
            return Err("captured rank tuples have incompatible component arity");
        }
        let mut observations = IntegerObservations::empty(self.program);
        observations.engine =
            super::Engine::strict_for_declaration(self.program, self.machine_symbol);
        let mut substitutions = Vec::with_capacity(sources.len());
        for source in sources {
            let mut bindings = BTreeMap::new();
            for (name, value, primitive) in &source.atoms {
                let value = self
                    .integer_value(value, *primitive, &mut observations, 0)
                    .ok_or("the saved rank lost an immutable atom observation")?;
                bindings.insert(name.clone(), value);
            }
            substitutions.push(bindings);
        }
        let mut inputs = Vec::with_capacity(destinations.len());
        for destination in destinations {
            inputs.push(
                self.integer_value(
                    &destination.input,
                    destination.primitive,
                    &mut observations,
                    0,
                )
                .ok_or("the arriving rank input has no exact integer observation")?,
            );
        }
        let required = 0..observations.operations.len();
        let mut premises = self.observed_integer_premises(&mut observations);
        if !observations.install_premises(&mut premises) || !observations.admitted(required) {
            return Err("a captured rank operand has unformed arithmetic");
        }
        let engine = &mut observations.engine;
        let mut source_values = Vec::with_capacity(sources.len());
        for (source, bindings) in sources.iter().zip(&substitutions) {
            let value = engine
                .instantiate_integer_denotation(&source.value, bindings)
                .ok_or("the saved rank lost its complete observed dependency roster")?;
            let (Some(minimum), Some(maximum)) = (&source.interval.low, &source.interval.high)
            else {
                return Err("the saved rank lost its independently formed finite bounds");
            };
            // These facts belong to the earlier produced value, not the next
            // input or its pending intermediate operations. Distinct captures
            // use their own local-to-shared atom map; equal atom ordinals in
            // separate denotations do not identify their underlying values.
            if !engine.install_hypotheses(vec![
                (
                    BinaryOperator::GreaterOrEqual,
                    value.clone(),
                    Polynomial::constant(minimum.clone()),
                ),
                (
                    BinaryOperator::LessOrEqual,
                    value.clone(),
                    Polynomial::constant(maximum.clone()),
                ),
            ]) {
                return Err("the saved rank bounds could not enter the current observation scope");
            }
            source_values.push(value);
        }
        let mut destination_values = Vec::with_capacity(destinations.len());
        for (destination, input) in destinations.iter().zip(&inputs) {
            let (value, minimum) = self.form_arriving_integer_rank(engine, destination, input)?;
            let value = engine.substituted(&value);
            if !engine.prove_at_least(&value.sub(&Polynomial::constant(minimum)), &BigInt::zero()) {
                return Err(if sources.len() == 1 {
                    "captured ranks do not prove natural nonincrease"
                } else {
                    "the arriving rank has no independently established lower bound"
                });
            }
            destination_values.push(value);
        }
        for (source, destination) in source_values.iter().zip(&destination_values) {
            let difference = engine.substituted(source).sub(destination);
            if !engine.prove_at_least(&difference, &BigInt::zero()) {
                return Err(if sources.len() == 1 {
                    "captured ranks do not prove natural nonincrease"
                } else {
                    "captured ranks do not prove lexicographic nonincrease"
                });
            }
            if engine.prove_at_least(&difference, &BigInt::from_u64(1)) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn form_arriving_integer_rank(
        &self,
        engine: &mut super::Engine<'_>,
        destination: &CapturedIntegerRank,
        input: &Polynomial,
    ) -> Result<(Polynomial, BigInt), &'static str> {
        if !destination.measure.is_valid() {
            return Ok((input.clone(), destination.minimum.clone()));
        }
        let primitive = self
            .program
            .measures()
            .iter()
            .find(|declaration| declaration.symbol == destination.measure)
            .and_then(|declaration| {
                self.program
                    .primitive_type_reference(declaration.return_type)
            })
            .ok_or("the arriving rank lost its declared integer carrier")?;
        primitive
            .integer_bounds()
            .ok_or("the arriving rank has no fixed integer bounds")?;
        let value = engine
            .form_declared_integer_application(destination.measure, input)
            .ok_or("the arriving rank application has no independent formation")?
            .value;
        if !engine.integer_value_lands(primitive, &value) {
            return Err("the arriving rank does not fit its declared carrier");
        }
        Ok((value, destination.minimum.clone()))
    }
}
