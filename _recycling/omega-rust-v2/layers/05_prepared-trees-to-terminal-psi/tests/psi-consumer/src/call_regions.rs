//! Reads which regions a call applies to its callee, from published Psi alone.
//! A machine with region binders has one body, so the call's `MachineId` does not say which
//! regions an invocation relates. The body's `call_regions` row for that call does.
//!
//! A row argument is either a region binder of the calling machine or `CallRegion::Operands`:
//! a region the caller has no binder for, which the call's operands carry. For the second the
//! reader follows each operand back to the descriptor its record was built from, so a test can
//! name the exact backing place without consulting typing.
use omega_psi::{
    AggregateRegionSlot, BlockTerminator, CallArgument, CallRegion, CallSite, CallTarget, DeclarationId, LoweredMachine, LoweredPackage, MachineBody,
    MachineRegion, OperationKind, PlaceId, PlaceOrigin, PlaceProjection, ProjectedPlace, TypeShape, ValueOperand,
};

/// One region binder of a callee at one call: the callee's slots for that binder, and the
/// calling machine's region the call applies to it.
#[derive(Debug, Eq, PartialEq)]
pub struct AppliedRegion<'a> {
    pub callee: &'a MachineRegion,
    /// Ordinal of `caller` in the calling machine's `regions`.
    pub ordinal: usize,
    pub caller: &'a MachineRegion,
}

/// What one call applies to one region binder of its callee.
#[derive(Debug, Eq, PartialEq)]
pub enum RegionAtCall<'a> {
    /// A region binder of the calling machine.
    Binder(AppliedRegion<'a>),
    /// No binder of the calling machine. `carried` lists every loan the call's operands hold
    /// in the callee's parameter slots for this binder, in slot order. It is never empty.
    Operands { callee: &'a MachineRegion, carried: Vec<CarriedLoan<'a>> },
}

/// One loan an operand carries into a callee's region slot, traced to the place it came from.
#[derive(Debug, Eq, PartialEq)]
pub struct CarriedLoan<'a> {
    /// The callee parameter the operand fills.
    pub parameter: usize,
    /// For a record operand, the field stored at the slot's lifetime binder. `None` for an
    /// operand that is itself the reference.
    pub field: Option<&'a DeclarationId>,
    /// The descriptor the loan was copied or lent from in the calling body: the place the
    /// record's establishment copied into `field`, or the place a reference operand lends.
    pub backing: ProjectedPlace,
}

/// The callee and the operands of the direct call at `site`, whether the call is an operation
/// or a terminal edge.
fn call(
    caller: &LoweredMachine,
    site: CallSite,
) -> Option<(CallTarget, &[CallArgument])> {
    match site {
        CallSite::Operation(id) => caller
            .body
            .operations
            .iter()
            .find_map(|operation| match &operation.kind {
                OperationKind::Call { callee, arguments, .. } if operation.id == id => Some((*callee, arguments.as_slice())),
                _ => None,
            }),
        CallSite::Edge(edge) => caller
            .body
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                BlockTerminator::TerminalCall { id, callee, arguments, .. } if *id == edge => Some((*callee, arguments.as_slice())),
                _ => None,
            }),
        CallSite::Cleanup(_) => None,
    }
}

/// The place a whole-value operand reads, through any whole copies the body made of it.
fn established(
    body: &MachineBody,
    mut place: PlaceId,
) -> PlaceId {
    while let PlaceOrigin::Operation(id) = body.places[place.index()].origin
        && let OperationKind::CopyStructural { source } = &body.operations[id.index()].kind
        && source.path.is_empty()
    {
        place = source.root;
    }
    place
}

/// The loans the record operand at `slot` holds at the slot's lifetime binder. The operand
/// must be a record this body established, so each such field's initializer names the
/// descriptor it stores, or the result of a call whose own row leaves that region to its
/// operands, which are then read the same way.
fn record_loans<'a>(
    package: &'a LoweredPackage,
    caller: &'a LoweredMachine,
    slot: AggregateRegionSlot,
    operand: &CallArgument,
    carried: &mut Vec<CarriedLoan<'a>>,
) -> Result<(), String> {
    let body = &caller.body;
    let (ValueOperand::CopyWhole(place) | ValueOperand::MoveWhole(place)) = operand else {
        return Err(format!("operand {} is not a whole record", slot.parameter));
    };
    let place = established(body, *place);
    let Some(TypeShape::Nominal(id)) = package.types.get(body.places[place.index()].ty) else {
        return Err(format!("operand {} is not a record", slot.parameter));
    };
    let record = package
        .data
        .iter()
        .find(|data| data.id == *id)
        .ok_or_else(|| format!("operand {} names no packaged record", slot.parameter))?;
    if slot.binder >= record.lifetime_count {
        return Err(format!(
            "slot binder {} is outside its record's {} lifetimes",
            slot.binder, record.lifetime_count
        ));
    }
    let PlaceOrigin::Operation(operation) = body.places[place.index()].origin else {
        return Err(format!("operand {} was not established in this body", slot.parameter));
    };
    let before = carried.len();
    let fields = match &body.operations[operation.index()].kind {
        OperationKind::EstablishRecord { fields } => fields,
        // The callee's slots say which of its regions the returned record's binder is in.
        // This reader accepts that call only when its row leaves the region to operands too.
        OperationKind::Call { .. } => {
            let (_, regions) = regions_at_call(package, caller, CallSite::Operation(operation))?;
            for region in regions {
                match region {
                    RegionAtCall::Operands { callee, carried: loans } if callee.aggregate_result.contains(&slot.binder) => {
                        carried.extend(loans.into_iter().map(|loan| CarriedLoan {
                            parameter: slot.parameter,
                            ..loan
                        }));
                    }
                    RegionAtCall::Binder(applied) if applied.callee.aggregate_result.contains(&slot.binder) => {
                        return Err(format!("operand {} is a call result at a region binder of the caller", slot.parameter));
                    }
                    _ => {}
                }
            }
            return if carried.len() == before {
                Err(format!("operand {} is a call result that no region of its callee reaches", slot.parameter))
            } else {
                Ok(())
            };
        }
        _ => return Err(format!("operand {} was not established as a record", slot.parameter)),
    };
    if fields.len() != record.fields.len() {
        return Err(format!("operand {} was established with another field roster", slot.parameter));
    }
    for (row, initializer) in record.fields.iter().zip(fields) {
        if row.declaration != initializer.field {
            return Err(format!("operand {} was established with another field roster", slot.parameter));
        }
        if row.aggregate_lifetimes.contains(&slot.binder) {
            return Err(format!(
                "field `{}` passes the binder to a nested record this reader does not trace",
                row.declaration.path
            ));
        }
        if row.lifetime != Some(slot.binder) {
            continue;
        }
        let ValueOperand::CopyWhole(source) = initializer.operand else {
            return Err(format!("field `{}` does not copy a shared descriptor", row.declaration.path));
        };
        if body.places[source.index()].ty != row.carrier {
            return Err(format!("field `{}` copies a descriptor of another type", row.declaration.path));
        }
        carried.push(CarriedLoan {
            parameter: slot.parameter,
            field: Some(&row.declaration),
            backing: ProjectedPlace {
                root: source,
                path: Vec::new(),
            },
        });
    }
    if carried.len() == before {
        return Err(format!("operand {} stores no reference at binder {}", slot.parameter, slot.binder));
    }
    Ok(())
}

/// The callee of the direct call at `site` and, per region binder of that callee, what the
/// call applies. A callee without region binders must have no row. A callee with region
/// binders must have exactly one, with one argument per binder: an in-range binder of the
/// caller, or operands that carry a loan in at least one of the callee's slots for it.
pub fn regions_at_call<'a>(
    package: &'a LoweredPackage,
    caller: &'a LoweredMachine,
    site: CallSite,
) -> Result<(&'a LoweredMachine, Vec<RegionAtCall<'a>>), String> {
    let (target, operands) = call(caller, site).ok_or_else(|| format!("{site:?} is not a direct machine call"))?;
    let CallTarget::Machine(id) = target else {
        return Err(format!("{site:?} is not a direct machine call"));
    };
    let callee = package
        .machines
        .iter()
        .find(|machine| machine.id == id)
        .ok_or_else(|| format!("call target {id:?} names no packaged machine"))?;
    let rows: Vec<_> = caller
        .body
        .call_regions
        .iter()
        .filter(|row| row.call == site)
        .collect();
    if callee.regions.is_empty() {
        return if rows.is_empty() {
            Ok((callee, Vec::new()))
        } else {
            Err(format!("{site:?} applies regions to a machine that has none"))
        };
    }
    let [row] = rows.as_slice() else {
        return Err(format!("{site:?} has {} region rows for a machine with region binders", rows.len()));
    };
    if row.arguments.len() != callee.regions.len() {
        return Err(format!("{site:?} applies {} regions to {} binders", row.arguments.len(), callee.regions.len()));
    }
    let applied = callee
        .regions
        .iter()
        .zip(&row.arguments)
        .map(|(binder, argument)| match *argument {
            CallRegion::Binder(ordinal) => {
                let region = caller
                    .regions
                    .get(ordinal)
                    .ok_or_else(|| format!("{site:?} names region {ordinal} of a caller with {}", caller.regions.len()))?;
                Ok(RegionAtCall::Binder(AppliedRegion {
                    callee: binder,
                    ordinal,
                    caller: region,
                }))
            }
            CallRegion::Operands => {
                let mut carried = Vec::new();
                for &parameter in &binder.reference_parameters {
                    let Some(ValueOperand::Borrow { place, .. }) = operands.get(parameter) else {
                        return Err(format!("{site:?} operand {parameter} lends no place to a reference formal"));
                    };
                    let Some((PlaceProjection::Referent, descriptor)) = place.path.split_last() else {
                        return Err(format!("{site:?} operand {parameter} does not forward a descriptor"));
                    };
                    carried.push(CarriedLoan {
                        parameter,
                        field: None,
                        backing: ProjectedPlace {
                            root: place.root,
                            path: descriptor.to_vec(),
                        },
                    });
                }
                for &slot in &binder.aggregate_parameters {
                    let operand = operands
                        .get(slot.parameter)
                        .ok_or_else(|| format!("{site:?} has no operand {}", slot.parameter))?;
                    record_loans(package, caller, slot, operand, &mut carried).map_err(|reason| format!("{site:?}: {reason}"))?;
                }
                if carried.is_empty() {
                    return Err(format!("{site:?} leaves a region to operands, and none carries it"));
                }
                Ok(RegionAtCall::Operands { callee: binder, carried })
            }
        })
        .collect::<Result<_, String>>()?;
    Ok((callee, applied))
}

/// `regions_at_call` for a call that applies only the calling machine's own region binders.
pub fn applied_regions<'a>(
    package: &'a LoweredPackage,
    caller: &'a LoweredMachine,
    site: CallSite,
) -> Result<(&'a LoweredMachine, Vec<AppliedRegion<'a>>), String> {
    let (callee, regions) = regions_at_call(package, caller, site)?;
    let applied = regions
        .into_iter()
        .map(|region| match region {
            RegionAtCall::Binder(applied) => Ok(applied),
            RegionAtCall::Operands { .. } => Err(format!("{site:?} applies a region its operands carry, not a binder of the caller")),
        })
        .collect::<Result<_, String>>()?;
    Ok((callee, applied))
}
