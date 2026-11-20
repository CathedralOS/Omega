/*
We close semantic formals and native-only direct callback arguments in authored
order (`NativeParameterTelescope`), enforcing fresh compact parameter
identities, contiguous zero-based native ordinals, and admitted 8/8 target
function-pointer shapes while deriving direct callback demands directly from the
emitted rows.
*/

use crate::{
    BoundaryNativeParameterGeometry, BoundaryNativeParameterOrigin, BoundaryNativeParameterShape,
    NativeCallbackDemand, NativeParameterId, NativePlace,
};
use semantic_vocabulary::ScalarType;
use target::NativeTarget;
use terminal_psi::CallingDirectCallback;

pub struct NativeParameterTelescope {
    pub parameters: Vec<BoundaryNativeParameterGeometry>,
}

impl BoundaryNativeParameterShape {
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

impl crate::BoundaryCallingApplication {
    /// Project the foreign frame's scalar formals onto their checked native ordinals.
    pub fn foreign_scalar_parameters(
        &self,
        formal_count: usize,
        formals: &[(usize, ScalarType)],
    ) -> Result<Vec<Option<ScalarType>>, String> {
        let rows = self
            .signature
            .native_parameters
            .span(self.signature.native_parameter_rows)
            .ok_or("foreign frame scalar transport has stale native telescope storage")?;
        let count = self.plan.plan().call.parameters.len();
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
}
