//! Close semantic formals and native-only callback arguments in authored order.

use crate::{
    BoundaryNativeParameterGeometry, BoundaryNativeParameterOrigin, BoundaryNativeParameterShape,
    NativeCallbackDemand, NativeParameterId, NativePlace,
};
use target::NativeTarget;
use terminal_psi::CallingDirectCallback;

pub struct NativeParameterTelescope {
    pub parameters: Vec<BoundaryNativeParameterGeometry>,
}

impl BoundaryNativeParameterShape {
    /// The admitted targets use one 64-bit function pointer. A different
    /// representation needs its own ABI admission, not a truncated address.
    pub fn function_pointer(target: NativeTarget) -> Result<Self, String> {
        if target.pointer_size != 8 || target.pointer_alignment != 8 {
            return Err(format!(
                "direct callback parameters currently require an admitted 8/8 target function-pointer shape, got {}/{}",
                target.pointer_size, target.pointer_alignment
            ));
        }
        Ok(Self::TargetFunctionPointer {
            byte_size: 8,
            alignment: 8,
        })
    }
}

impl NativeParameterTelescope {
    /// Preserve semantic shape/identity correspondence while filling native-only
    /// arguments. Compact IDs and native ordinals must each name one parameter;
    /// gaps are not another ABI and cannot silently shift a callback address.
    pub fn close(
        semantic: impl IntoIterator<Item = BoundaryNativeParameterGeometry>,
        callbacks: impl IntoIterator<Item = CallingDirectCallback>,
        target: NativeTarget,
    ) -> Result<Self, String> {
        let mut parameters = Vec::<BoundaryNativeParameterGeometry>::new();
        for parameter in semantic {
            if !matches!(
                parameter.origin,
                BoundaryNativeParameterOrigin::SemanticFormal { .. }
            ) {
                return Err(
                    "native telescope semantic input contains a native-only parameter".into(),
                );
            }
            validate_fresh_native_parameter_identity(
                parameters.iter().map(|prior| prior.identity),
                parameter.identity,
                parameter.native_ordinal,
            )?;
            parameters.push(parameter);
        }
        for callback in callbacks {
            validate_fresh_native_parameter_identity(
                parameters.iter().map(|prior| prior.identity),
                callback.identity,
                callback.native_ordinal,
            )?;
            parameters.push(BoundaryNativeParameterGeometry {
                identity: callback.identity,
                native_ordinal: callback.native_ordinal,
                shape: BoundaryNativeParameterShape::function_pointer(target)?,
                origin: BoundaryNativeParameterOrigin::PrivateCallback {
                    binder: callback.binder,
                    requirement: callback.requirement,
                },
            });
        }
        parameters.sort_unstable_by_key(|parameter| parameter.native_ordinal);
        for (expected, parameter) in parameters.iter().enumerate() {
            if usize::try_from(parameter.native_ordinal).ok() != Some(expected) {
                return Err(format!(
                    "native parameter telescope has position {} where contiguous position {expected} is required",
                    parameter.native_ordinal
                ));
            }
        }
        Ok(Self { parameters })
    }

    /// The emitted rows retain every direct obligation; no parallel request
    /// roster is needed to recover the callback identity or requirement.
    pub fn direct_demands(&self) -> impl Iterator<Item = NativeCallbackDemand> + '_ {
        self.parameters
            .iter()
            .filter_map(|parameter| match parameter.origin {
                BoundaryNativeParameterOrigin::PrivateCallback { requirement, .. } => {
                    Some(NativeCallbackDemand {
                        destination: NativePlace::Parameter(parameter.identity),
                        requirement,
                    })
                }
                BoundaryNativeParameterOrigin::SemanticFormal { .. } => None,
            })
    }
}

pub fn validate_fresh_native_parameter_identity(
    prior: impl IntoIterator<Item = NativeParameterId>,
    identity: NativeParameterId,
    ordinal: u32,
) -> Result<(), String> {
    if prior.into_iter().any(|candidate| candidate == identity) {
        return Err(format!(
            "boundary runtime parameter {ordinal} collides with an earlier exact parameter on one compact native-parameter report identity"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CallbackRequirementId, StaticMachineBinderId};

    fn target() -> NativeTarget {
        target::TargetProfile::from_omega_target_name(Some("linux_x86_64"))
            .unwrap()
            .native_target()
    }

    fn semantic(
        identity: u64,
        native_ordinal: u32,
        formal_ordinal: u32,
    ) -> BoundaryNativeParameterGeometry {
        BoundaryNativeParameterGeometry {
            identity: NativeParameterId::new(identity).unwrap(),
            native_ordinal,
            shape: BoundaryNativeParameterShape::Semantic(formal_ordinal as u16),
            origin: BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal },
        }
    }

    fn callback(identity: u64, native_ordinal: u32) -> CallingDirectCallback {
        CallingDirectCallback {
            identity: NativeParameterId::new(identity).unwrap(),
            native_ordinal,
            binder: StaticMachineBinderId::new(identity + 1000).unwrap(),
            requirement: CallbackRequirementId::new(identity + 2000).unwrap(),
        }
    }

    #[test]
    fn mixed_native_order_keeps_semantic_origins_and_all_direct_demands() {
        let closed = NativeParameterTelescope::close(
            [semantic(1, 1, 0), semantic(2, 3, 1)],
            [callback(102, 4), callback(100, 0), callback(101, 2)],
            target(),
        )
        .unwrap();
        assert_eq!(
            closed
                .parameters
                .iter()
                .map(|row| row.identity.get())
                .collect::<Vec<_>>(),
            [100, 1, 101, 2, 102]
        );
        assert_eq!(closed.parameters[1], semantic(1, 1, 0));
        assert_eq!(closed.parameters[3], semantic(2, 3, 1));
        let demands = closed.direct_demands().collect::<Vec<_>>();
        assert_eq!(demands.len(), 3);
        for (demand, identity) in demands.iter().zip([100, 101, 102]) {
            assert_eq!(
                demand.destination,
                NativePlace::Parameter(NativeParameterId::new(identity).unwrap())
            );
            assert_eq!(
                demand.requirement,
                CallbackRequirementId::new(identity + 2000).unwrap()
            );
        }
    }

    #[test]
    fn duplicate_id_ordinal_gap_and_unadmitted_pointer_shape_refuse() {
        let collision =
            NativeParameterTelescope::close([semantic(1, 0, 0)], [callback(1, 1)], target())
                .err()
                .unwrap();
        assert!(collision.contains("compact native-parameter report identity"));
        let gap = NativeParameterTelescope::close([semantic(1, 1, 0)], [], target())
            .err()
            .unwrap();
        assert!(gap.contains("contiguous position 0"));
        let ordinal =
            NativeParameterTelescope::close([semantic(1, 0, 0)], [callback(2, 0)], target())
                .err()
                .unwrap();
        assert!(ordinal.contains("contiguous position 1"));
        let mut invalid = target();
        invalid.pointer_size = 4;
        let pointer = NativeParameterTelescope::close([], [callback(1, 0)], invalid)
            .err()
            .unwrap();
        assert!(pointer.contains("8/8 target function-pointer shape"));
    }
}
