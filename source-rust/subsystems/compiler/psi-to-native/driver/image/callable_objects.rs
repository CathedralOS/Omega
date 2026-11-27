//! Lower a reachable private callable and retain its original Psi identity and evidence.

use super::plan_callables::CallableTargetProduct;
use crate::driver::compile_body::{self, BodyEmissionError};
use diagnostics::Diagnostic;
use provider_admission::realization_error;

pub(super) struct CallableObjectProduct {
    pub stage_originals: Option<Box<published_object_verification::RetainedPhysicalInputs>>,
    pub physical_original: native_artifact::PrivatePhysicalOriginal,
    pub projection: native_artifact::FragmentObjectProjection,
}

pub(super) struct CallableProduct<Body> {
    pub body: Body,
    custody: CallableCustody,
}

struct CallableCustody {
    code: terminal_codec::RetainedTerminalCode,
    coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
    observation: crate::driver::observations::NativePipelineObservation,
    architecture: target::Architecture,
}

pub(super) type CallablePhysicalProduct = CallableProduct<compile_body::PreparedPhysicalBody>;
pub(super) type BoundCallable = CallableProduct<compile_body::BoundPhysicalBody>;
pub(super) type EncodedCallable = CallableProduct<compile_body::EncodedBody>;
pub(super) type LaidOutCallable = CallableProduct<compile_body::LaidOutBody>;

fn callable_error(architecture: target::Architecture, error: BodyEmissionError) -> Vec<Diagnostic> {
    match error {
        BodyEmissionError::Physical(error) => match error.unimplemented_optimization() {
            Some(optimization) => {
                compile_body::unimplemented_optimization(optimization, architecture)
            }
            None => realization_error("callable physical pipeline", error),
        },
        BodyEmissionError::Installation(reason) => {
            realization_error("callable installation binding", reason)
        }
        BodyEmissionError::Object(diagnostics) => diagnostics,
    }
}

pub(super) fn prepare_callable_physical(
    prepared: CallableTargetProduct,
    execution: Option<provider_admission::AdmittedInstalledEntryExecution>,
    requirements: Vec<installation_conditions::InstalledInterruptStackRequirement>,
) -> Result<CallablePhysicalProduct, Vec<Diagnostic>> {
    let CallableTargetProduct {
        code,
        coverage,
        target: optimized_target,
        observation,
    } = prepared;
    let architecture = optimized_target.execution_plan().plan().target.architecture;
    let body = compile_body::prepare_physical_body(
        optimized_target,
        execution,
        requirements,
        &observation,
    )
    .map_err(|error| callable_error(architecture, error))?;
    Ok(CallableProduct {
        body,
        custody: CallableCustody {
            code,
            coverage,
            observation,
            architecture,
        },
    })
}

impl CallablePhysicalProduct {
    pub(super) fn bind(
        self,
        handlers: &installation_binding::RetainedHandlerMeasurements,
    ) -> Result<BoundCallable, Vec<Diagnostic>> {
        Ok(CallableProduct {
            body: self.body.bind(handlers, None)?,
            custody: self.custody,
        })
    }
}
impl BoundCallable {
    pub(super) fn encode(self) -> Result<EncodedCallable, Vec<Diagnostic>> {
        let body = compile_body::encode_physical_body(self.body, &self.custody.observation)
            .map_err(|error| callable_error(self.custody.architecture, error))?;
        Ok(CallableProduct {
            body,
            custody: self.custody,
        })
    }
}

impl EncodedCallable {
    pub(super) fn layout(self) -> Result<LaidOutCallable, Vec<Diagnostic>> {
        let body = compile_body::layout_encoded_body(self.body, &self.custody.observation)
            .map_err(|error| callable_error(self.custody.architecture, error))?;
        Ok(CallableProduct {
            body,
            custody: self.custody,
        })
    }
}

impl LaidOutCallable {
    pub(super) fn construct_object(self) -> Result<CallableObjectProduct, Vec<Diagnostic>> {
        let CallableCustody {
            code,
            coverage,
            observation,
            architecture,
        } = self.custody;
        let compile_body::BodyObject {
            projection,
            originals,
        } = compile_body::construct_body_object(self.body)
            .map_err(|error| callable_error(architecture, error))?;
        let physical_original = native_artifact::PrivatePhysicalOriginal::new(
            projection.object().psi(),
            code,
            coverage,
        )
        .map_err(|reason| realization_error("callable physical evidence", reason))?;
        observation.complete();
        Ok(CallableObjectProduct {
            stage_originals: originals,
            physical_original,
            projection,
        })
    }
}
