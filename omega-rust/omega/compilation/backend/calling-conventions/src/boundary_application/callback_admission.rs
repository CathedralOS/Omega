//! Admission joins a complete untrusted payload to an independently selected
//! method commitment. Neither a matching copied offset nor the payload's own
//! claimed digest is an authority root.
use super::{BoundaryCallingApplication, BoundaryNativeParameterShape};
use crate::{
    BoundaryNativeParameterOrigin, CallbackMaterializationContext, NativeParameterApplication,
    NativePlace, StaticMachineBinderId,
};

impl BoundaryCallingApplication {
    pub fn authenticated_callback_offset(
        &self,
        expected_commitment: [u8; 32],
        binder: StaticMachineBinderId,
        destination: &NativePlace,
        parameter: &NativeParameterApplication,
        context: &CallbackMaterializationContext,
    ) -> Result<Option<usize>, String> {
        if expected_commitment == [0; 32]
            || self.commitment != expected_commitment
            || self.recomputed_commitment()? != expected_commitment
            || !self.callback_context_closed
        {
            return Err(
                "callback application does not match the admitted method commitment".to_owned(),
            );
        }
        let native = self
            .signature
            .native_parameters
            .span(self.signature.native_parameter_rows)
            .ok_or("callback application has stale native telescope")?;
        let catalog = self
            .signature
            .callback_layouts
            .span(self.signature.callback_layout_rows)
            .ok_or("callback application has stale layout catalog")?;
        let rows = &self.plan.plan().call.callback_materializations;
        if context.binders.len() != rows.len() || context.demands.len() != rows.len() {
            return Err(
                "callback context changed its authenticated application coverage".to_owned(),
            );
        }
        // Every context item must rejoin the complete authenticated application,
        // including callbacks other than the occurrence being materialized.
        for row in rows {
            let requirement = match &row.destination {
                NativePlace::Parameter(identity) => {
                    let mut matching = native.iter().filter(|entry| entry.identity == *identity);
                    let entry = matching.next().ok_or("callback parameter is absent")?;
                    if matching.next().is_some() {
                        return Err("callback parameter is ambiguous".to_owned());
                    }
                    match entry.origin {
                        BoundaryNativeParameterOrigin::PrivateCallback {
                            binder,
                            requirement,
                        } if binder == row.binder => requirement,
                        _ => return Err("callback parameter changed its binder origin".to_owned()),
                    }
                }
                NativePlace::Field { .. } => {
                    let mut matching = catalog
                        .iter()
                        .filter(|entry| entry.destination == row.destination);
                    let entry = matching
                        .next()
                        .ok_or("callback destination has no authenticated catalog entry")?;
                    if matching.next().is_some() {
                        return Err("callback catalog destination is ambiguous".to_owned());
                    }
                    entry.terminal.requirement
                }
            };
            if context
                .binders
                .iter()
                .filter(|entry| entry.binder == row.binder && entry.requirement == requirement)
                .count()
                != 1
                || context
                    .demands
                    .iter()
                    .filter(|entry| {
                        entry.destination == row.destination && entry.requirement == requirement
                    })
                    .count()
                    != 1
            {
                return Err(
                    "callback context changed an authenticated binder or requirement".to_owned(),
                );
            }
        }
        if rows
            .iter()
            .filter(|row| row.binder == binder && row.destination == *destination)
            .count()
            != 1
        {
            return Err(
                "callback occurrence has no unique authenticated binder destination".to_owned(),
            );
        }
        let identity = match destination {
            NativePlace::Parameter(identity)
            | NativePlace::Field {
                parameter: identity,
                ..
            } => identity,
        };
        let mut matching = native.iter().filter(|entry| entry.identity == *identity);
        let entry = matching
            .next()
            .ok_or("callback root has no native parameter")?;
        if matching.next().is_some()
            || entry.identity != parameter.parameter
            || entry.native_ordinal != parameter.native_ordinal
            || self
                .plan
                .plan()
                .call
                .parameters
                .get(entry.native_ordinal as usize)
                != Some(&parameter.placement)
            || parameter.shape != parameter.placement.shape
        {
            return Err("callback root changed its authenticated parameter application".to_owned());
        }
        match destination {
            NativePlace::Parameter(_) => {
                if !matches!(entry.shape, BoundaryNativeParameterShape::TargetFunctionPointer { byte_size, alignment }
                    if usize::from(byte_size) == self.target.pointer_size && usize::from(alignment) == self.target.pointer_alignment)
                {
                    return Err("callback parameter changed its target pointer geometry".to_owned());
                }
                Ok(None)
            }
            NativePlace::Field { .. } => {
                let layout = catalog
                    .iter()
                    .find(|layout| layout.destination == *destination)
                    .ok_or("callback field has no authenticated geometry")?;
                if entry.origin
                    != (BoundaryNativeParameterOrigin::SemanticFormal {
                        formal_ordinal: layout.formal_ordinal,
                    })
                    || entry.native_ordinal != layout.native_ordinal
                    || !matches!(entry.shape, BoundaryNativeParameterShape::Semantic(_))
                {
                    return Err("callback field changed its semantic formal association".to_owned());
                }
                Ok(Some(layout.field_offset(self.target)?))
            }
        }
    }
}
