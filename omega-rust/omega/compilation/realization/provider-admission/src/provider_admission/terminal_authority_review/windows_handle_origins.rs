//! Finite scalar origin equations with explicit invocation and return custody.
//!
//! Recursive calls remain graph edges. In particular, this graph does not erase
//! a call/return pair, select one incoming source, or claim that a cyclic formal
//! has a producing event. The lifetime replay must match those edges in an
//! activation and discharge the native success/argument obligations separately.

use std::collections::BTreeMap;

use semantic_vocabulary::{MachineId, ScalarReadProjection, ScalarTerm, ValueId};
use terminal_psi::TerminalModule;
use terminal_semantics::descriptors::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPosition, canonical_descriptor_location,
};

use super::windows_handle_calls::{CallTarget, Caller, HandleCallGraph};
use super::windows_handle_values::{
    HandleValueReader, ScalarDefinition, ScalarReturnValue, StoredScalarSource,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ScalarSubject {
    Value {
        machine: MachineId,
        value: ValueId,
    },
    Storage {
        machine: MachineId,
        position: DescriptorPosition,
        location: DescriptorLocation,
    },
    Expression {
        machine: MachineId,
        position: DescriptorPosition,
        value: ScalarTerm,
    },
    CallResult {
        caller: Caller,
        projection: Option<
            Vec<terminal_semantics::descriptors::descriptor_lifetimes::DescriptorPathSegment>,
        >,
    },
    NativeOutput {
        caller: Caller,
        parameter: usize,
    },
}

#[derive(Clone, Debug)]
pub(super) enum OriginEdgeRole {
    Operand,
    Stored,
    /// The edge's original callee is retained even when several targets share
    /// one dynamic invocation site. Actuals are simultaneous pre-call captures.
    Actual {
        caller: Caller,
        callee: MachineId,
    },
    Returned {
        caller: Caller,
        callee: MachineId,
        position: DescriptorPosition,
    },
    Arrival {
        predecessor: DescriptorPosition,
        successor_ordinal: usize,
    },
}

#[derive(Clone, Debug)]
pub(super) struct OriginEdge {
    pub role: OriginEdgeRole,
    pub source: ScalarSubject,
}

#[derive(Clone, Debug)]
pub(super) enum OriginKind {
    Alternatives,
    Expression(ScalarTerm),
    /// A result is not a successful acquisition merely because it came from
    /// this API. The replay must establish its status on each consuming path.
    BoundaryReturn {
        caller: Caller,
        boundary: semantic_vocabulary::BoundaryMachineId,
    },
    NativeOutput {
        caller: Caller,
        parameter: usize,
    },
    IntrinsicReturn {
        caller: Caller,
        execution: effects::CompilerIntrinsicExecutionIdentity,
    },
    /// Inputs or observations with no derived identity remain explicit unknowns.
    /// No consumer may interpret their absent edges as empty physical reach.
    ExternalInput,
    OpaqueObservation(ScalarTerm),
    InactiveCase(terminal_semantics::descriptors::descriptor_results::DescriptorCasePredicate),
}

#[derive(Clone, Debug)]
pub(super) struct OriginEquation {
    pub kind: OriginKind,
    pub sources: Vec<OriginEdge>,
}

pub(super) struct HandleOrigins {
    pub equations: BTreeMap<ScalarSubject, OriginEquation>,
}

impl HandleOrigins {
    /// Build only the demanded operand closure. Every node is a real binder,
    /// current storage cut, expression capture or native output coordinate.
    pub(super) fn derive(
        module: &TerminalModule,
        calls: &HandleCallGraph<'_>,
        reader: &mut HandleValueReader<'_>,
        roots: impl IntoIterator<Item = ScalarSubject>,
    ) -> Result<Self, String> {
        let mut equations = BTreeMap::new();
        let mut pending = roots.into_iter().collect::<Vec<_>>();
        while let Some(subject) = pending.pop() {
            if equations.contains_key(&subject) {
                continue;
            }
            if equations.len() >= 65_536 {
                return Err(
                    "native handle operand closure exceeds its finite analysis budget".into(),
                );
            }
            let equation = match &subject {
                ScalarSubject::Value { machine, value } => {
                    let definition = reader.read(*machine, *value)?;
                    let position = reader.value_position(*machine, *value)?;
                    definition_equation(calls, reader, *machine, position, definition)?
                }
                ScalarSubject::Storage {
                    machine,
                    position,
                    location,
                } => {
                    let definition = reader.read_storage(*machine, *position, location)?;
                    definition_equation(calls, reader, *machine, *position, definition)?
                }
                ScalarSubject::Expression {
                    machine,
                    position,
                    value,
                } => expression_equation(module, calls, *machine, *position, value)?,
                ScalarSubject::CallResult { caller, projection } => {
                    result_equation(calls, reader, *caller, projection.as_deref())?
                }
                ScalarSubject::NativeOutput { caller, parameter } => {
                    reader.require_native_output(*caller, *parameter)?;
                    OriginEquation {
                        kind: OriginKind::NativeOutput {
                            caller: *caller,
                            parameter: *parameter,
                        },
                        sources: Vec::new(),
                    }
                }
            };
            pending.extend(equation.sources.iter().map(|edge| edge.source.clone()));
            equations.insert(subject, equation);
        }
        Ok(Self { equations })
    }
}

fn definition_equation(
    calls: &HandleCallGraph<'_>,
    reader: &HandleValueReader<'_>,
    identity: MachineId,
    position: DescriptorPosition,
    definition: ScalarDefinition,
) -> Result<OriginEquation, String> {
    let mut sources = Vec::new();
    match definition {
        ScalarDefinition::Parameter(parameter) => {
            for call in calls.callers.get(&identity).into_iter().flatten() {
                sources.push(OriginEdge {
                    role: OriginEdgeRole::Actual {
                        caller: call.caller,
                        callee: identity,
                    },
                    source: ScalarSubject::Value {
                        machine: call.caller.machine,
                        value: reader.scalar_actual(call, identity, parameter)?,
                    },
                });
            }
            if sources.is_empty() {
                return Ok(OriginEquation {
                    kind: OriginKind::ExternalInput,
                    sources,
                });
            }
        }
        ScalarDefinition::EntryStorage(entry) => {
            for call in calls.callers.get(&identity).into_iter().flatten() {
                sources.push(OriginEdge {
                    role: OriginEdgeRole::Actual {
                        caller: call.caller,
                        callee: identity,
                    },
                    source: ScalarSubject::Storage {
                        machine: call.caller.machine,
                        position: caller_position(call.caller)?,
                        location: reader.storage_actual(call, identity, &entry)?,
                    },
                });
            }
            if sources.is_empty() {
                return Ok(OriginEquation {
                    kind: OriginKind::ExternalInput,
                    sources,
                });
            }
        }
        ScalarDefinition::BorrowedEntry(entry) => {
            for call in calls.callers.get(&identity).into_iter().flatten() {
                sources.push(OriginEdge {
                    role: OriginEdgeRole::Actual {
                        caller: call.caller,
                        callee: identity,
                    },
                    source: ScalarSubject::Storage {
                        machine: call.caller.machine,
                        position: caller_position(call.caller)?,
                        location: reader.storage_actual_location(call, identity, &entry)?,
                    },
                });
            }
            if sources.is_empty() {
                return Ok(OriginEquation {
                    kind: OriginKind::ExternalInput,
                    sources,
                });
            }
        }
        ScalarDefinition::InactiveCase(predicate) => {
            return Ok(OriginEquation {
                kind: OriginKind::InactiveCase(predicate),
                sources,
            });
        }
        ScalarDefinition::Expression(value) => sources.push(OriginEdge {
            role: OriginEdgeRole::Operand,
            source: ScalarSubject::Expression {
                machine: identity,
                position,
                value,
            },
        }),
        ScalarDefinition::CallResult(caller) => sources.push(OriginEdge {
            role: OriginEdgeRole::Operand,
            source: ScalarSubject::CallResult {
                caller,
                projection: None,
            },
        }),
        ScalarDefinition::Arrivals(arrivals) => {
            for arrival in arrivals {
                sources.push(OriginEdge {
                    role: OriginEdgeRole::Arrival {
                        predecessor: arrival.predecessor,
                        successor_ordinal: arrival.successor_ordinal,
                    },
                    source: match arrival.value {
                        terminal_semantics::descriptors::descriptor_results::CapturedScalarOperand::Value { id, scalar_type } => ScalarSubject::Expression {
                            machine: identity,
                            position: arrival.predecessor,
                            value: ScalarTerm::value(id, scalar_type),
                        },
                        terminal_semantics::descriptors::descriptor_results::CapturedScalarOperand::Storage(location) => ScalarSubject::Storage {
                            machine: identity,
                            position: arrival.predecessor,
                            location,
                        },
                    },
                });
            }
        }
        ScalarDefinition::Stored(stored) => {
            for source in stored {
                let source = match source {
                    StoredScalarSource::Scalar { position, value } => ScalarSubject::Expression {
                        machine: identity,
                        position,
                        value,
                    },
                    StoredScalarSource::Transport { position, source } => ScalarSubject::Storage {
                        machine: identity,
                        position,
                        location: source,
                    },
                    StoredScalarSource::NativeOutput { caller, parameter } => {
                        ScalarSubject::NativeOutput { caller, parameter }
                    }
                    StoredScalarSource::CallResult { caller, path } => ScalarSubject::CallResult {
                        caller,
                        projection: Some(path),
                    },
                };
                sources.push(OriginEdge {
                    role: OriginEdgeRole::Stored,
                    source,
                });
            }
        }
    }
    if sources.is_empty() {
        return Err(format!(
            "handle origin {identity:?}/{position:?} has an empty producing alternative"
        ));
    }
    Ok(OriginEquation {
        kind: OriginKind::Alternatives,
        sources,
    })
}

fn caller_position(caller: Caller) -> Result<DescriptorPosition, String> {
    Ok((
        caller.block,
        u32::try_from(caller.operation_index)
            .map_err(|_| "handle caller ordinal exceeds its canonical position")?,
    ))
}

fn result_equation(
    calls: &HandleCallGraph<'_>,
    reader: &HandleValueReader<'_>,
    caller: Caller,
    projection: Option<
        &[terminal_semantics::descriptors::descriptor_lifetimes::DescriptorPathSegment],
    >,
) -> Result<OriginEquation, String> {
    let alternatives = calls
        .calls
        .get(&caller)
        .ok_or("handle result has no original runtime invocation")?;
    if alternatives.is_empty() {
        return Err("handle result has an empty executable target roster".into());
    }
    let mut sources = Vec::new();
    for call in alternatives {
        if !call.normal_return {
            return Err("a nonnormal invocation cannot publish a handle result".into());
        }
        match call.target {
            CallTarget::Machine(callee) => {
                for returned in reader.returns(callee, projection)? {
                    sources.push(OriginEdge {
                        role: OriginEdgeRole::Returned {
                            caller,
                            callee,
                            position: returned.position,
                        },
                        source: match returned.value {
                            ScalarReturnValue::Value(value) => ScalarSubject::Value {
                                machine: callee,
                                value,
                            },
                            ScalarReturnValue::Storage(location) => ScalarSubject::Storage {
                                machine: callee,
                                position: returned.position,
                                location,
                            },
                        },
                    });
                }
            }
            CallTarget::Boundary(boundary) if alternatives.len() == 1 && projection.is_none() => {
                return Ok(OriginEquation {
                    kind: OriginKind::BoundaryReturn { caller, boundary },
                    sources,
                });
            }
            CallTarget::Intrinsic(execution) if alternatives.len() == 1 && projection.is_none() => {
                let super::windows_handle_calls::RuntimeArguments::Explicit { scalar, structural } =
                    call.arguments
                else {
                    return Err("intrinsic scalar result has no explicit invocation inputs".into());
                };
                if !structural.is_empty() {
                    return Err("intrinsic scalar result requires an unsupported structural operand interpretation".into());
                }
                sources.extend(scalar.iter().map(|value| OriginEdge {
                    role: OriginEdgeRole::Operand,
                    source: ScalarSubject::Value {
                        machine: caller.machine,
                        value: *value,
                    },
                }));
                return Ok(OriginEquation {
                    kind: OriginKind::IntrinsicReturn { caller, execution },
                    sources,
                });
            }
            _ => {
                return Err(
                    "handle result mixes a physical result with a body or structural projection"
                        .into(),
                );
            }
        }
    }
    Ok(OriginEquation {
        kind: OriginKind::Alternatives,
        sources,
    })
}

fn expression_equation(
    module: &TerminalModule,
    calls: &HandleCallGraph<'_>,
    machine: MachineId,
    position: DescriptorPosition,
    value: &ScalarTerm,
) -> Result<OriginEquation, String> {
    let source = match value {
        ScalarTerm::Value { id, .. } => Some(ScalarSubject::Value {
            machine,
            value: *id,
        }),
        ScalarTerm::BooleanField { root, path }
        | ScalarTerm::IntegerField { root, path, .. }
        | ScalarTerm::IeeeFloatField { root, path, .. } => {
            let root_type = calls.machines[&machine]
                .structural_place_type(*root)
                .ok_or("handle expression storage has no exact carrier")?;
            Some(ScalarSubject::Storage {
                machine,
                position,
                location: canonical_descriptor_location(
                    &module.structural_types,
                    *root,
                    root_type,
                    path,
                )
                .ok_or("handle expression has an unresolved canonical storage projection")?,
            })
        }
        ScalarTerm::StructuralRead { root, path, .. } => {
            let path = path
                .iter()
                .map(|segment| match segment {
                    ScalarReadProjection::Referent => {
                        Some(semantic_vocabulary::CanonicalStructuralPathSegment::Referent)
                    }
                    ScalarReadProjection::Static(segment) => Some(*segment),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>();
            let Some(path) = path else {
                return Ok(OriginEquation {
                    kind: OriginKind::OpaqueObservation(value.clone()),
                    sources: Vec::new(),
                });
            };
            let root_type = calls.machines[&machine]
                .structural_place_type(*root)
                .ok_or("handle read expression has no exact carrier")?;
            Some(ScalarSubject::Storage {
                machine,
                position,
                location: canonical_descriptor_location(
                    &module.structural_types,
                    *root,
                    root_type,
                    &path,
                )
                .ok_or("handle read expression has an unresolved canonical storage projection")?,
            })
        }
        _ => None,
    };
    if let Some(source) = source {
        return Ok(OriginEquation {
            kind: OriginKind::Alternatives,
            sources: vec![OriginEdge {
                role: OriginEdgeRole::Operand,
                source,
            }],
        });
    }
    let operands = value.operands();
    let literal = matches!(
        value,
        ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_) | ScalarTerm::IeeeFloatLiteral(_)
    );
    let kind = if operands.is_empty() && !literal {
        OriginKind::OpaqueObservation(value.clone())
    } else {
        OriginKind::Expression(value.clone())
    };
    Ok(OriginEquation {
        kind,
        sources: operands
            .into_iter()
            .map(|operand| OriginEdge {
                role: OriginEdgeRole::Operand,
                source: ScalarSubject::Expression {
                    machine,
                    position,
                    value: operand.clone(),
                },
            })
            .collect(),
    })
}
