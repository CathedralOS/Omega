//! Relational data predicates after a scalar-loan invocation.
//!
//! Checked return contracts supply changed fields. The caller's current value
//! environment supplies only fields outside the complete may-write frame.
//! Simultaneous substitution retains the declaration's selected arithmetic;
//! no pre-call invariant is assumed to prove its own handback.
use super::{Interval, ValueEnvironment};
use crate::proof_contracts::arithmetic_domains::BigIntInterval;
use crate::value_custody::places::CanonicalPlace;
use crate::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticValue, StrictArithmeticImplicationJudgment, scoped_arithmetic_implication,
};
use numerics::bignum::BigInt;
use typed_trees::{data::DataDefinition, machine::Machine, state::State, types::PrimitiveType};

pub(crate) fn referent_predicates_preserved(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
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
    // A complete noninterfering invocation preserves the current predicate,
    // including relations between fields. Reconstructing each field as an
    // unrelated carrier interval loses those relations; a formal's carrier
    // also cannot replace an unchanged source value across a representation view.
    let mut dependencies = Vec::new();
    for fact in program
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .chain(variant.into_iter().flat_map(|variant| {
            program
                .proof_facts
                .span_or_empty(variant.where_facts)
                .iter()
        }))
    {
        for field in crate::proof_contracts::default_domains::data_predicate_fields(
            program, definition, fact,
        ) {
            if !dependencies.contains(&field) {
                dependencies.push(field);
            }
        }
    }
    // An empty field list can still carry a case-classifier requirement.
    // Without a classifier footprint, require the complete parent unchanged.
    let classifier_unwritten = !dependencies.is_empty()
        || !written
            .iter()
            .any(|path| super::place_paths::place_paths_overlap(path, &parent.label(program)));
    let predicate_unwritten = classifier_unwritten
        && dependencies.iter().all(|symbol| {
            let Some((_, field)) = program.any_field_owner(*symbol) else {
                return false;
            };
            // An inline scalar has a complete owned read footprint. A descriptor
            // or aggregate requires its referent/recursive read inventory; its
            // field slot alone cannot prove that those observations survived.
            if crate::reference_shell(program, field.type_reference).is_some()
                || program
                    .primitive_type_reference(field.type_reference)
                    .is_none()
            {
                return false;
            }
            let mut place = parent.clone();
            crate::value_custody::places::push_field_place_segments(
                program,
                &mut place.segments,
                field.symbol,
            );
            let label = place.label(program);
            !written
                .iter()
                .any(|path| super::place_paths::place_paths_overlap(path, &label))
        });
    if predicate_unwritten {
        let Some(receiver) = crate::value_custody::places::canonical_place_type_reference(
            program,
            state.symbol,
            statement_index,
            parent,
        ) else {
            return false;
        };
        return program
            .proof_facts
            .span_or_empty(definition.where_facts)
            .iter()
            .chain(variant.into_iter().flat_map(|variant| {
                program
                    .proof_facts
                    .span_or_empty(variant.where_facts)
                    .iter()
            }))
            .all(|fact| {
                fact.as_expression().is_some_and(|predicate| {
                    crate::proof_contracts::contract_entailment::captured_data_predicate_holds(
                        program,
                        machine,
                        state,
                        statement_index,
                        parent,
                        receiver,
                        definition,
                        *predicate,
                    )
                })
            });
    }
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
        crate::value_custody::places::push_field_place_segments(
            program,
            &mut place.segments,
            field.symbol,
        );
        let label = place.label(program);
        let unwritten = !written
            .iter()
            .any(|path| super::place_paths::place_paths_overlap(path, &label));
        let interval =
            unwritten
                .then(|| environment.get(&label))
                .flatten()
                .or_else(|| {
                    guarantees
                        .iter()
                        .find(|(target, _)| *target == place)
                        .map(|(_, interval)| *interval)
                })
                .or_else(|| {
                    let carrier = super::integer_ranges::primitive_range(primitive)?;
                    let interval =
                        super::enforced_declared_range_interval(program, field.type_reference)
                            .map_or(carrier, |declared| carrier.intersect(declared));
                    Some(
                    unwritten
                        .then(|| {
                            crate::proof_contracts::default_domains::field_literal_where_interval(
                                program,
                                definition,
                                field.name.as_str(),
                            )
                        })
                        .flatten()
                        .map_or(interval, |where_interval| interval.intersect(where_interval)),
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
                interval: BigIntInterval {
                    low: interval
                        .low
                        .map(|value| BigInt::from_i128(i128::from(value))),
                    high: interval
                        .high
                        .map(|value| BigInt::from_i128(i128::from(value))),
                },
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
