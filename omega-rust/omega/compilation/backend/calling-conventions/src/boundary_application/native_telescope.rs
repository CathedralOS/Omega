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
use target::NativeTarget;
use terminal_psi::CallingDirectCallback;

#[derive(Debug)]
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
        table_parameter: crate::TableParameter,
    ) -> Result<Self, String> {
        let mut parameters = Vec::<BoundaryNativeParameterGeometry>::new();
        for (ordinal, parameter) in semantic.into_iter().enumerate() {
            if !matches!(
                parameter.origin,
                BoundaryNativeParameterOrigin::SemanticFormal { .. }
            ) {
                return Err(
                    "native telescope semantic input contains a native-only parameter".into(),
                );
            }
            if !matches!(parameter.origin, BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal }
                if usize::try_from(formal_ordinal).ok() == Some(ordinal))
            {
                return Err(
                    "semantic formals are missing, repeated or out of declaration order".into(),
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
        // Roles precede the final native positions. Callback destinations are
        // authored native positions; removing a semantic formal must not shift
        // them along with the old all-passed declaration roster.
        let selected = match table_parameter {
            crate::TableParameter::None => None,
            crate::TableParameter::Passed {
                formal,
                formal_ordinal,
            }
            | crate::TableParameter::DispatchOnly {
                formal,
                formal_ordinal,
            } => Some((formal, formal_ordinal)),
        };
        if let Some((identity, ordinal)) = selected {
            let parameter = parameters
                .iter()
                .find(|row| row.identity == identity)
                .ok_or("table role names no declared semantic formal")?;
            if !matches!(parameter.origin, BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } if formal_ordinal == ordinal)
            {
                return Err("table role changes its exact semantic formal correspondence or names a private callback".into());
            }
        }
        if let crate::TableParameter::DispatchOnly { formal, .. } = table_parameter {
            parameters.retain(|row| row.identity != formal);
            let count = parameters.len();
            let mut occupied = vec![false; count];
            for row in &parameters {
                if matches!(
                    row.origin,
                    BoundaryNativeParameterOrigin::PrivateCallback { .. }
                ) {
                    let position = usize::try_from(row.native_ordinal)
                        .ok().and_then(|ordinal| occupied.get_mut(ordinal))
                        .ok_or("callback destination is outside the native roster after dispatch exclusion")?;
                    if std::mem::replace(position, true) {
                        return Err("callback destinations repeat a native position".into());
                    }
                }
            }
            parameters.sort_unstable_by_key(|row| match row.origin {
                BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } => formal_ordinal,
                BoundaryNativeParameterOrigin::PrivateCallback { .. } => u32::MAX,
            });
            let mut free = occupied
                .iter()
                .enumerate()
                .filter_map(|(ordinal, used)| (!used).then_some(ordinal));
            for row in &mut parameters {
                if matches!(
                    row.origin,
                    BoundaryNativeParameterOrigin::SemanticFormal { .. }
                ) {
                    row.native_ordinal = u32::try_from(
                        free.next()
                            .ok_or("semantic formals exceed the remaining native positions")?,
                    )
                    .map_err(|_| "native position exceeds u32")?;
                }
            }
            parameters.sort_unstable_by_key(|row| row.native_ordinal);
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
            crate::TableParameter::None,
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
    fn table_roles_close_before_native_positions_without_moving_callbacks() {
        let id = |value| NativeParameterId::new(value).unwrap();
        let close = |role| {
            NativeParameterTelescope::close(
                [semantic(1, 0, 0), semantic(2, 2, 1), semantic(4, 3, 2)],
                [callback(3, 1)],
                target(),
                role,
            )
        };
        let passed = close(crate::TableParameter::Passed {
            formal: id(2),
            formal_ordinal: 1,
        })
        .unwrap();
        assert_eq!(
            passed
                .parameters
                .iter()
                .map(|row| row.identity.get())
                .collect::<Vec<_>>(),
            [1, 3, 2, 4]
        );
        let omitted = close(crate::TableParameter::DispatchOnly {
            formal: id(2),
            formal_ordinal: 1,
        })
        .unwrap();
        assert_eq!(
            omitted
                .parameters
                .iter()
                .map(|row| (row.identity.get(), row.native_ordinal))
                .collect::<Vec<_>>(),
            [(1, 0), (3, 1), (4, 2)]
        );
        assert_eq!(
            omitted.parameters[2].origin,
            BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal: 2 }
        );
        // The callback's authored native destination is one after exclusion,
        // even when the removed table originally preceded it.
        let first = NativeParameterTelescope::close(
            [semantic(1, 0, 0), semantic(2, 2, 1)],
            [callback(3, 1)],
            target(),
            crate::TableParameter::DispatchOnly {
                formal: id(1),
                formal_ordinal: 0,
            },
        )
        .unwrap();
        assert_eq!(
            first
                .parameters
                .iter()
                .map(|row| (row.identity.get(), row.native_ordinal))
                .collect::<Vec<_>>(),
            [(2, 0), (3, 1)]
        );
        assert!(
            close(crate::TableParameter::Passed {
                formal: id(3),
                formal_ordinal: 1
            })
            .unwrap_err()
            .contains("private callback")
        );
        assert!(
            close(crate::TableParameter::DispatchOnly {
                formal: id(99),
                formal_ordinal: 1
            })
            .unwrap_err()
            .contains("no declared semantic formal")
        );
    }

    #[test]
    fn duplicate_id_ordinal_gap_and_unadmitted_pointer_shape_refuse() {
        let collision = NativeParameterTelescope::close(
            [semantic(1, 0, 0)],
            [callback(1, 1)],
            target(),
            crate::TableParameter::None,
        )
        .err()
        .unwrap();
        assert!(collision.contains("compact native-parameter report identity"));
        let gap = NativeParameterTelescope::close(
            [semantic(1, 1, 0)],
            [],
            target(),
            crate::TableParameter::None,
        )
        .err()
        .unwrap();
        assert!(gap.contains("contiguous position 0"));
        let ordinal = NativeParameterTelescope::close(
            [semantic(1, 0, 0)],
            [callback(2, 0)],
            target(),
            crate::TableParameter::None,
        )
        .err()
        .unwrap();
        assert!(ordinal.contains("contiguous position 1"));
        let mut invalid = target();
        invalid.pointer_size = 4;
        let pointer = NativeParameterTelescope::close(
            [],
            [callback(1, 0)],
            invalid,
            crate::TableParameter::None,
        )
        .err()
        .unwrap();
        assert!(pointer.contains("8/8 target function-pointer shape"));
    }
}
