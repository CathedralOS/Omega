// Realize one exact callable while retaining its original Terminal namespace.
// Callback and component export entries use the same source-free admission and
// native pipeline (`pipeline::lower_to_target`, physical planning and encoding)
// while leaving their identity, installation, and publication contracts with
// the caller.

use crate::pipeline::{self, BodyEmissionError};
use diagnostics::Diagnostic;
use native_program_planning::callable_planning::CallableTargetProduct;
use provider_admission::provider_admission::realization_error;

pub(super) struct CallableObjectProduct {
    pub physical_original: native_artifact::PrivatePhysicalOriginal,
    pub projection: native_artifact::FragmentObjectProjection,
    pub runtime: native_artifact::TerminalCallableRuntimeProvisions<
        super::auxiliary_native::EncodedForeignAdapter,
    >,
    pub authority: provider_admission::provider_admission::AdmittedNativeCallableAuthority,
}

pub(super) struct CallableProduct<Body, Adapter> {
    pub body: Body,
    custody: CallableCustody,
    runtime: native_artifact::TerminalCallableRuntimeProvisions<Adapter>,
}

struct CallableCustody {
    code: terminal_codec::RetainedTerminalCode,
    coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
    authority: provider_admission::provider_admission::AdmittedNativeCallableAuthority,
    observation: crate::observations::NativePipelineObservation,
}

pub(super) type CallablePhysicalProduct =
    CallableProduct<pipeline::PreparedPhysicalBody, super::auxiliary_native::PlannedForeignAdapter>;
pub(super) type BoundCallable =
    CallableProduct<pipeline::BoundPhysicalBody, super::auxiliary_native::PlannedForeignAdapter>;
pub(super) type EncodedCallable =
    CallableProduct<pipeline::EncodedBody, super::auxiliary_native::EncodedForeignAdapter>;
pub(super) type LaidOutCallable =
    CallableProduct<pipeline::LaidOutBody, super::auxiliary_native::EncodedForeignAdapter>;

fn callable_error(error: BodyEmissionError) -> Vec<Diagnostic> {
    match error {
        BodyEmissionError::Physical(error) => {
            realization_error("callable physical pipeline", error)
        }
        BodyEmissionError::Object(diagnostics) => diagnostics,
    }
}

pub(super) fn prepare_callable_physical(
    prepared: CallableTargetProduct,
    execution: Option<provider_admission::AdmittedInstalledEntryExecution>,
    requirements: Vec<installation_evidence::InstalledInterruptStackRequirement>,
) -> Result<CallablePhysicalProduct, Vec<Diagnostic>> {
    let CallableTargetProduct {
        retain_native_stage_inputs,
        code,
        coverage,
        target: optimized_target,
        runtime,
        authority,
        observation,
    } = prepared;
    let installations = pipeline::plan_installations(&optimized_target, execution, requirements)
        .map_err(|reason| realization_error("callable installation binding", reason))?;
    let body = pipeline::prepare_physical_body(
        optimized_target,
        installations,
        &observation,
        retain_native_stage_inputs,
    )
    .map_err(callable_error)?;
    let runtime = runtime.map_adapters(super::auxiliary_native::plan);
    Ok(CallableProduct {
        body,
        runtime,
        custody: CallableCustody {
            code,
            coverage,
            authority,
            observation,
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
            runtime: self.runtime,
        })
    }
}
impl BoundCallable {
    pub(super) fn encode(self) -> Result<EncodedCallable, Vec<Diagnostic>> {
        let body = pipeline::encode_physical_body(self.body, &self.custody.observation)
            .map_err(callable_error)?;
        Ok(CallableProduct {
            body,
            runtime: self.runtime.map_adapters(super::auxiliary_native::encode),
            custody: self.custody,
        })
    }
}

impl EncodedCallable {
    pub(super) fn layout(self) -> Result<LaidOutCallable, Vec<Diagnostic>> {
        let body = pipeline::layout_encoded_body(self.body, &self.custody.observation)
            .map_err(callable_error)?;
        Ok(CallableProduct {
            body,
            runtime: self.runtime,
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
            authority,
        } = self.custody;
        let projection =
            pipeline::construct_body_object(self.body).map_err(callable_error)?;
        let physical_original = native_artifact::PrivatePhysicalOriginal::new(
            projection.object().psi(),
            code,
            coverage,
        )
        .map_err(|reason| realization_error("callable physical evidence", reason))?;
        observation.complete();
        Ok(CallableObjectProduct {
            physical_original,
            projection,
            runtime: self.runtime,
            authority,
        })
    }
}
