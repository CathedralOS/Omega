//! Relative progress along an authenticated additive endpoint chain.
//!
//! Reflexive root order supplies the reference point. Every wrapping step
//! retains its own independent no-wrap predicate, including intermediate and
//! inverse steps; the net displacement cannot hide a crossed boundary. This
//! maps the existing affine witness, not a new arithmetic policy or proof axiom.

use super::{
    CheckedIntegerAffineForm, CheckedIntegerEndpointStep, IntegerAffineBoundConversionError,
    Proposition, ScalarTerm, push_wrapping_evidence,
};

pub(super) fn applies(form: &CheckedIntegerAffineForm, strict: bool, bound: &ScalarTerm) -> bool {
    !strict && bound == form.root() && bound.integer_value().is_none()
}

fn translation(
    form: &CheckedIntegerAffineForm,
) -> Result<(i128, Vec<Proposition>), IntegerAffineBoundConversionError> {
    let mut displacement = 0_i128;
    let mut evidence = Vec::new();
    for step in &form.endpoint_steps {
        displacement = match step {
            CheckedIntegerEndpointStep::Add(literal) => displacement.checked_add(*literal),
            CheckedIntegerEndpointStep::Subtract(literal) => displacement.checked_sub(*literal),
            CheckedIntegerEndpointStep::WrappingAdd { operand, literal } => {
                push_wrapping_evidence(&mut evidence, form.integer_type(), operand, *literal)?;
                displacement.checked_add(*literal)
            }
            CheckedIntegerEndpointStep::WrappingAddBackward { operand, literal } => {
                push_wrapping_evidence(&mut evidence, form.integer_type(), operand, *literal)?;
                displacement.checked_sub(*literal)
            }
            _ => return Err(IntegerAffineBoundConversionError::StrictBoundNotTranslation),
        }
        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
    }
    if displacement == 0 {
        return Err(IntegerAffineBoundConversionError::NonProgressingTranslation);
    }
    Ok((displacement, evidence))
}

pub(super) fn evidence(
    form: &CheckedIntegerAffineForm,
) -> Result<Vec<Proposition>, IntegerAffineBoundConversionError> {
    translation(form).map(|(_, evidence)| evidence)
}

pub(super) fn map(
    form: &CheckedIntegerAffineForm,
    provided: &[&Proposition],
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let (displacement, mut required) = translation(form)?;
    for member in provided {
        let Some(position) = required.iter().position(|required| required == *member) else {
            return Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected);
        };
        required.remove(position);
    }
    if !required.is_empty() {
        return Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing);
    }
    Ok(if displacement < 0 {
        Proposition::LessThan(form.target().clone(), form.root().clone())
    } else {
        Proposition::LessThan(form.root().clone(), form.target().clone())
    })
}

#[cfg(test)]
mod tests;
