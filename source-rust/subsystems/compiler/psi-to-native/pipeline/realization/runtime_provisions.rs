//! Plan each admitted foreign ABI adapter as a separate native root.
#![forbid(unsafe_code)]

use calling_conventions::{BoundaryCallingApplication, BoundaryNativeParameterOrigin};
use semantic_vocabulary::ScalarType;
use verified_optimization_input::runtime_provisions::AdmittedRuntimeForeignCall;
pub use verified_optimization_input::runtime_provisions::{
    NativeBindingRuntimeInputs, NativeRuntimePlan, TerminalCallableRuntimeProvisions,
    TerminalRuntimeHostProvision, TerminalRuntimeHostProvisions,
};
use verified_optimization_input::{AdmittedRuntimeInput, NativeProgramEntrySettlement};

fn scalar_parameters(
    formal_count: usize,
    formals: &[(usize, ScalarType)],
    application: &BoundaryCallingApplication,
) -> Result<Vec<Option<ScalarType>>, String> {
    let rows = application
        .signature
        .native_parameters
        .span(application.signature.native_parameter_rows)
        .ok_or("foreign frame scalar transport has stale native telescope storage")?;
    let count = application.plan.plan().call.parameters.len();
    if rows.len() != count {
        return Err("foreign frame scalar transport has an incomplete native telescope".into());
    }
    let mut result = vec![None; count];
    let mut native_seen = vec![false; count];
    let mut scalar_seen = vec![false; formals.len()];
    for row in rows {
        let ordinal = usize::try_from(row.native_ordinal)
            .map_err(|_| "foreign frame native ordinal overflows")?;
        let seen = native_seen
            .get_mut(ordinal)
            .ok_or("foreign frame native ordinal is outside its calling plan")?;
        if std::mem::replace(seen, true) {
            return Err("foreign frame native ordinal is repeated".into());
        }
        let BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } = row.origin else {
            continue;
        };
        let formal = usize::try_from(formal_ordinal)
            .map_err(|_| "foreign frame semantic ordinal overflows")?;
        if formal >= formal_count {
            return Err("foreign frame semantic ordinal is outside its declaration".into());
        }
        if let Some((index, (_, scalar))) = formals
            .iter()
            .enumerate()
            .find(|(_, (position, _))| *position == formal)
        {
            if std::mem::replace(&mut scalar_seen[index], true) {
                return Err("foreign frame scalar formal has repeated native transport".into());
            }
            result[ordinal] = Some(*scalar);
        }
    }
    if scalar_seen.contains(&false) {
        return Err("foreign frame lost a scalar formal's native transport".into());
    }
    Ok(result)
}

/// An admitted auxiliary native root. Capturing runtime metadata records this
/// request; the native compiler plans and encodes it before publishing provisions.
#[derive(Debug, Clone)]
pub struct NativeForeignAdapterRequest {
    bridge: calling_conventions::NativeForeignBridgeRequest,
    locator: target::NormalizedForeignLocator,
    application: [u8; 32],
}

impl NativeForeignAdapterRequest {
    #[cfg(feature = "test-support")]
    pub(crate) fn for_test(
        bridge: calling_conventions::NativeForeignBridgeRequest,
        locator: target::NormalizedForeignLocator,
        application: [u8; 32],
    ) -> Self {
        Self {
            bridge,
            locator,
            application,
        }
    }

    pub fn into_parts(
        self,
    ) -> (
        calling_conventions::NativeForeignBridgeRequest,
        target::NormalizedForeignLocator,
        [u8; 32],
    ) {
        (self.bridge, self.locator, self.application)
    }
}

fn plan_adapter(call: AdmittedRuntimeForeignCall) -> Result<NativeForeignAdapterRequest, String> {
    let (formal_count, formals, application, locator) = call.into_parts();
    let scalar_parameters = scalar_parameters(formal_count, &formals, &application)?;
    Ok(NativeForeignAdapterRequest {
        bridge: calling_conventions::NativeForeignBridgeRequest {
            target: application.target,
            foreign: application.plan,
            scalar_parameters,
        },
        locator,
        application: application.commitment,
    })
}

pub fn plan_host_provisions(
    installation: Option<&terminal_installation::AdmittedProviderSelections>,
    host: Vec<TerminalRuntimeHostProvision>,
    input: AdmittedRuntimeInput<'_>,
    selected: &effects::SelectedProviderPlanFacts,
    external_bindings: &[calling_conventions::ExternalBindingRow],
    native: NativeBindingRuntimeInputs<'_>,
) -> Result<TerminalRuntimeHostProvisions<NativeForeignAdapterRequest>, String> {
    let admitted = TerminalRuntimeHostProvisions::admit(
        installation,
        host,
        input,
        selected,
        external_bindings,
        native,
    )?;
    Ok(admitted.map_adapters(plan_adapter))
}

pub fn plan_root_provisions(
    program_entry: NativeProgramEntrySettlement<'_>,
    input: AdmittedRuntimeInput<'_>,
    installation: Option<&terminal_installation::AdmittedProviderSelections>,
    host: Vec<TerminalRuntimeHostProvision>,
    execution: &verified_optimization_input::PreparedNativeExecution,
    external_bindings: &[calling_conventions::ExternalBindingRow],
    native: NativeBindingRuntimeInputs<'_>,
) -> Result<NativeRuntimePlan<NativeForeignAdapterRequest>, String> {
    let admitted = NativeRuntimePlan::admit(
        program_entry,
        input,
        installation,
        host,
        execution,
        external_bindings,
        native,
    )?;
    Ok(admitted.map_adapters(plan_adapter))
}
