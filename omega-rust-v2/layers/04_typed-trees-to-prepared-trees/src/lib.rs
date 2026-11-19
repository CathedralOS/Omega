//! Preparation evaluates local integer constants and the computations needed for closed type identity.
#![forbid(unsafe_code)]

mod integer_arithmetic;
mod integer_constants;
mod integer_invocation;

use std::collections::HashMap;

use integer_constants::IntegerEvaluation;
use num_bigint::BigInt;
use omega_resolver::DeclarationId;
use omega_typer::{
    SourcePackage, SpecializedIndexValue, StaticIntegerDemand, StaticIntegerRequest, TypeError, TypedTrees, TypingEnvironment, TypingProgress,
    type_source_package_for_preparation,
};

/// Typed trees whose demanded closed indices have been evaluated and materialized.
#[derive(Debug, Eq, PartialEq)]
pub struct PreparedTrees<'text> {
    typed: TypedTrees<'text>,
    integer_constants: HashMap<DeclarationId, BigInt>,
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
        self.integer_constants.get(declaration)
    }

    /// Extract the typed stage, dropping the prepared constant values.
    pub fn into_typed(self) -> TypedTrees<'text> {
        self.typed
    }
}

/// Prepare one admitted source package. Only the supported integer invocation
/// fragment may execute before a general proof checker is available.
pub fn prepare_source_package<'text>(source: &SourcePackage<'text>) -> Result<PreparedTrees<'text>, TypeError> {
    let mut evaluated_indices = Vec::new();
    let mut specialized_indices: Vec<SpecializedIndexValue> = Vec::new();
    loop {
        let environment = TypingEnvironment {
            evaluated_indices: &evaluated_indices,
            specialized_indices: &specialized_indices,
            ..Default::default()
        };
        let pending = match type_source_package_for_preparation(source, &environment)? {
            TypingProgress::Complete(typed) => {
                let integer_constants = IntegerEvaluation::new(&typed).finish()?;
                return Ok(PreparedTrees { typed, integer_constants });
            }
            TypingProgress::Pending(pending) => pending,
        };
        let previous_count = evaluated_indices.len() + specialized_indices.len();
        let mut evaluation = IntegerEvaluation::new(pending.trees());
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
            return Err(refusal(request, "typing did not advance after receiving the evaluation result"));
        }
    }
}

fn refusal(
    request: &StaticIntegerRequest,
    reason: &str,
) -> TypeError {
    TypeError {
        package: request.expression.package.clone(),
        file: request.expression.file.clone(),
        span: request.expression.span,
        message: format!("static evaluation cannot admit this invocation: {reason}"),
    }
}
