//! Retain scalar semantics at the original declaration/native-telescope join.
//! The admitted physical geometry cannot recover signedness from a byte width,
//! and a same-sized record is not a scalar. This produces one immutable frame
//! adapter for the same selected foreign row, before runtime installation.

use calling_conventions::{BoundaryCallingApplication, BoundaryNativeParameterOrigin};
use semantic_vocabulary::ScalarType;
use std::sync::Arc;
use terminal_psi::{BoundaryMachineDeclaration, BoundaryParameterKind};

fn scalar_parameters(
    declaration: &BoundaryMachineDeclaration,
    application: &BoundaryCallingApplication,
) -> Result<Vec<Option<ScalarType>>, String> {
    if !declaration.has_valid_parameter_order() {
        return Err("foreign frame scalar transport has no complete semantic formal order".into());
    }
    let formals = declaration
        .parameter_positions(BoundaryParameterKind::Scalar)
        .zip(&declaration.scalar_parameters)
        .collect::<Vec<_>>();
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
        if formal >= declaration.parameter_order.len() {
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
            result[ordinal] = Some(**scalar);
        }
    }
    if scalar_seen.contains(&false) {
        return Err("foreign frame lost a scalar formal's native transport".into());
    }
    Ok(result)
}

pub(super) fn emit(
    declaration: &BoundaryMachineDeclaration,
    application: &BoundaryCallingApplication,
    locator: &target::NormalizedForeignLocator,
) -> Result<Arc<image_emission::NativeForeignAdapterImage>, String> {
    let scalar_parameters = scalar_parameters(declaration, application)?;
    let bridge = machine_emission::native_foreign_bridge::emit_native_foreign_bridge(
        machine_emission::native_foreign_bridge::NativeForeignBridgeRequest {
            target: application.target,
            foreign: application.plan.clone(),
            scalar_parameters,
        },
    )
    .map_err(|reason| format!("foreign runtime adapter physical transport: {reason:?}"))?;
    image_emission::emit_native_foreign_adapter(bridge, locator.clone(), application.commitment)
        .map(Arc::new)
        .map_err(|error| error.diagnostic.to_string())
}
