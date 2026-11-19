//! Preparation evaluates local integer constants and the computations needed for closed type identity.
#![forbid(unsafe_code)]

mod constant_evaluation;
mod integer_arithmetic;
mod owned_values;
pub use owned_values::PreparedValue;
mod integer_invocation;

use std::collections::HashMap;

use constant_evaluation::ConstantEvaluation;
use num_bigint::BigInt;
use omega_resolver::DeclarationId;
use omega_typer::{
    SourcePackage, SpecializedIndexValue, StaticIntegerDemand, TypeError, TypedTrees, TypingEnvironment, TypingProgress, type_source_package_for_preparation,
};

/// Typed trees whose demanded closed indices have been evaluated and materialized.
#[derive(Debug, Eq, PartialEq)]
pub struct PreparedTrees<'text> {
    typed: TypedTrees<'text>,
    constants: HashMap<DeclarationId, PreparedValue>,
}

impl<'text> PreparedTrees<'text> {
    pub fn typed(&self) -> &TypedTrees<'text> {
        &self.typed
    }

    /// The frozen value of a local integer declaration.
    pub fn integer_constant(
        &self,
        declaration: &DeclarationId,
    ) -> Option<&BigInt> {
        match self.constant(declaration)? {
            PreparedValue::Integer(value) => Some(value),
            _ => None,
        }
    }

    pub fn constant(
        &self,
        declaration: &DeclarationId,
    ) -> Option<&PreparedValue> {
        self.constants.get(declaration)
    }

    /// Extract the typed stage, dropping the prepared constant values.
    pub fn into_typed(self) -> TypedTrees<'text> {
        self.typed
    }
}

/// Prepare one admitted source package. Only the supported integer invocation
/// fragment may execute before a general proof checker is available.
pub fn prepare_source_package<'text>(source: &SourcePackage<'text>) -> Result<PreparedTrees<'text>, TypeError> {
    prepare_source_package_with_boundary_slots(source, &[])
}

/// Use the same Build-selected boundary slots in every typing pass, including
/// the strict pass that checks the computed identities.
pub fn prepare_source_package_with_boundary_slots<'text>(
    source: &SourcePackage<'text>,
    boundary_slots: &[omega_typer::BoundarySlot],
) -> Result<PreparedTrees<'text>, TypeError> {
    let mut evaluated_indices = Vec::new();
    let mut specialized_indices: Vec<SpecializedIndexValue> = Vec::new();
    loop {
        let environment = TypingEnvironment {
            evaluated_indices: &evaluated_indices,
            specialized_indices: &specialized_indices,
            boundary_slots,
            ..TypingEnvironment::default()
        };
        let pending = match type_source_package_for_preparation(source, &environment)? {
            TypingProgress::Complete(typed) => {
                let constants = ConstantEvaluation::new(&typed, boundary_slots).finish()?;
                return Ok(PreparedTrees { typed, constants });
            }
            TypingProgress::Pending(pending) => pending,
        };
        let previous_count = evaluated_indices.len() + specialized_indices.len();
        let mut evaluation = ConstantEvaluation::new(pending.trees(), boundary_slots);
        for demand in pending.demands() {
            let (request, arguments) = match demand {
                StaticIntegerDemand::Closed(request) => (request, None),
                StaticIntegerDemand::Specialized { request, arguments } => (request, Some(arguments.as_slice())),
            };
            let supplied = if let Some(arguments) = arguments {
                specialized_indices
                    .iter()
                    .any(|result| result.request == *request && result.arguments == arguments)
            } else {
                evaluated_indices
                    .iter()
                    .any(|(supplied, _)| supplied == request)
            };
            if supplied {
                continue;
            }
            let value = evaluation.evaluate(request, arguments)?;
            if let Some(arguments) = arguments {
                specialized_indices.push(SpecializedIndexValue {
                    request: request.clone(),
                    arguments: arguments.to_vec(),
                    value,
                });
            } else {
                evaluated_indices.push((request.clone(), value));
            }
        }
        if evaluated_indices.len() + specialized_indices.len() == previous_count {
            let request = match pending.demands().first().expect("pending typing has a demand") {
                StaticIntegerDemand::Closed(request) | StaticIntegerDemand::Specialized { request, .. } => request,
            };
            return Err(refusal(&request.expression, "typing did not advance after receiving the evaluation result"));
        }
    }
}

fn refusal(
    site: &omega_typer::Site,
    reason: &str,
) -> TypeError {
    TypeError {
        package: site.package.clone(),
        file: site.file.clone(),
        span: site.span,
        message: format!("static evaluation cannot admit this invocation: {reason}"),
    }
}
