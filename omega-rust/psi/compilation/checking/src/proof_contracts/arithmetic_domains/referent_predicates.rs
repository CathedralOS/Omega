//! Relational data predicates after a scalar-loan invocation.
//!
//! Checked return contracts supply changed fields. The caller's current value
//! environment supplies only fields outside the complete may-write frame.
//! Simultaneous substitution retains the declaration's selected arithmetic;
//! no pre-call invariant is assumed to prove its own handback.
use super::{Interval, ValueEnvironment};
use crate::proof_contracts::contract_entailment::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticValue, StrictArithmeticImplicationJudgment, scoped_arithmetic_implication,
};
use crate::value_custody::places::CanonicalPlace;
use numerics::bignum::BigInt;
use typed_trees::{data::DataDefinition, machine::Machine, types::PrimitiveType};

pub(crate) fn referent_predicates_preserved(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    definition: &DataDefinition,
    parent: &CanonicalPlace,
    selected_field: symbols::SymbolHandle,
    guarantees: &[(CanonicalPlace, Interval)],
    environment: &ValueEnvironment,
    written: &[String],
) -> bool {
    let variant = program.data_members(definition).iter().find_map(|member| {
        let typed_trees::data::DataMember::Variant(variant) = member else {
            return None;
        };
        program
            .data_payload_fields(variant)
            .iter()
            .any(|field| field.symbol == selected_field)
            .then_some(variant)
    });
    let mut bindings = Vec::new();
    for field in program
        .data_members(definition)
        .iter()
        .filter_map(|member| member.as_field())
        .chain(
            variant
                .into_iter()
                .flat_map(|variant| program.data_payload_fields(variant)),
        )
    {
        let Some(primitive) = program
            .primitive_type_reference(field.type_reference)
            .filter(|primitive| primitive.is_fixed_integer())
        else {
            continue;
        };
        let mut place = parent.clone();
        place.segments.push(facts::PlaceSegment::Field {
            symbol: field.symbol,
        });
        let label = place.label(program);
        let interval = guarantees
            .iter()
            .find(|(target, _)| *target == place)
            .map(|(_, interval)| *interval)
            .or_else(|| {
                (!written
                    .iter()
                    .any(|path| super::place_paths::place_paths_overlap(path, &label)))
                .then(|| environment.get(&label))
                .flatten()
            })
            .or_else(|| {
                let carrier = super::integer_ranges::primitive_range(primitive)?;
                Some(
                    super::enforced_declared_range_interval(program, field.type_reference)
                        .map_or(carrier, |declared| carrier.intersect(declared)),
                )
            });
        let Some(interval) = interval else {
            continue;
        };
        bindings.push(ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Symbol(field.symbol),
            value: ScopedArithmeticValue::BoundedInteger {
                value: Box::new(ScopedArithmeticValue::Atom {
                    identity: format!(
                        "referent-handback:{}:{}",
                        field.symbol.arena_index(),
                        field.symbol.generation()
                    ),
                    unsigned: matches!(
                        primitive,
                        PrimitiveType::U8
                            | PrimitiveType::U16
                            | PrimitiveType::U32
                            | PrimitiveType::U64
                    ),
                }),
                minimum: interval
                    .low
                    .map(|value| BigInt::from_i128(i128::from(value))),
                maximum: interval
                    .high
                    .map(|value| BigInt::from_i128(i128::from(value))),
            },
        });
    }
    program
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .map(|fact| (definition.symbol, fact))
        .chain(variant.into_iter().flat_map(|variant| {
            program
                .proof_facts
                .span_or_empty(variant.where_facts)
                .iter()
                .map(move |fact| (variant.symbol, fact))
        }))
        .all(|(owner, fact)| {
            let Some(expression) = fact.as_expression() else {
                return false;
            };
            scoped_arithmetic_implication(
                program,
                machine,
                &[],
                &ScopedArithmeticExpression {
                    owner,
                    expression: *expression,
                    bindings: bindings.clone(),
                },
            ) == StrictArithmeticImplicationJudgment::Proven
        })
}
