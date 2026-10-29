//! Case formation for coordinates under an already admitted declaration roster.
//!
//! Entry Requires and completed public Ensures supply their own independently
//! checked premises. This query proves only each selected case at its exact
//! prefix. Geometry, integer bounds, readable access and captured reference loads
//! remain with their existing owners; no source expression or payload is made up.

use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;

pub fn declaration_projection_cases_hold(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    parameters: &[StateParameter],
    premises: &[ExpressionHandle],
    coordinate: &crate::CanonicalPlace,
) -> bool {
    let Some(last_case) = coordinate
        .segments
        .iter()
        .rposition(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
    else {
        return true;
    };
    let selected = (|| {
        let (reference, mut subject) = match coordinate.root {
            facts::PlaceRoot::Symbol(symbol) => {
                let mut inputs = parameters
                    .iter()
                    .filter(|parameter| parameter.symbol == symbol);
                let parameter = inputs.next()?;
                if inputs.next().is_some() || parameter.is_const || parameter.relevance.is_erased()
                {
                    return None;
                }
                (
                    parameter.type_reference,
                    StructuralTerm::Variable(parameter.name.to_string()),
                )
            }
            facts::PlaceRoot::Expression(expression) => {
                let result = crate::reserved_result_place(program, expression)?;
                if result.machine_symbol != owner
                    || result.root != expression
                    || !result.segments.is_empty()
                {
                    return None;
                }
                (
                    result.type_reference,
                    StructuralTerm::Variable(super::RESULT_BINDER.to_owned()),
                )
            }
            _ => return None,
        };
        let judge =
            StructuralJudge::from_declaration_requires(program, owner, parameters, premises, true);
        for (position, segment) in coordinate.segments[..=last_case].iter().enumerate() {
            // Resolve the complete prefix so case selections and generic field
            // substitutions stay in the original declaration's namespace.
            let current = crate::project_type_reference_from_segments(
                program,
                reference,
                &coordinate.segments[..position],
            )?;
            match segment {
                facts::PlaceSegment::Case { variant } => {
                    let (data, selected) = program.variant_owner(*variant)?;
                    let carrier = crate::unwrapped_type_reference(program, current)?;
                    let declared = program.closed_data_definition(carrier)?;
                    if declared.symbol != data.symbol || selected.symbol != *variant {
                        return None;
                    }
                    let decision = judge.judge_case(subject.clone(), data.symbol, *variant);
                    if !matches!(decision, StructuralJudgment::Proven) {
                        if super::struct_trace() {
                            eprintln!(
                                "STRUCT coordinate-case owner={owner:?} coordinate={coordinate:?} prefix={position} subject={subject:?} decision={decision:?}"
                            );
                        }
                        return None;
                    }
                    crate::project_type_reference_from_segments(
                        program,
                        reference,
                        &coordinate.segments[..=position],
                    )?;
                }
                facts::PlaceSegment::Field { .. } => {
                    subject = super::observed_projections::project_observed_value(
                        program,
                        current,
                        subject,
                        std::slice::from_ref(segment),
                    )?;
                }
                facts::PlaceSegment::FixedIndex { index } => {
                    subject = super::observed_projections::project_index(
                        program,
                        current,
                        subject,
                        StructuralTerm::Integer(numerics::bignum::BigInt::from_u64(
                            u64::try_from(*index).ok()?,
                        )),
                    )?;
                }
                facts::PlaceSegment::Index { expression } => {
                    let selector = super::structural_terms::structural_term(
                        program,
                        None,
                        Some((owner, parameters)),
                        *expression,
                        None,
                    )?;
                    subject = super::observed_projections::project_index(
                        program, current, subject, selector,
                    )?;
                }
                _ => return None,
            }
        }
        Some(())
    })();
    selected.is_some()
}
