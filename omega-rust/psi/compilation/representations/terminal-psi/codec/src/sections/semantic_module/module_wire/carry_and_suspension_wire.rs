//! Carry policies and suspension call plans, sites and targets on the
//! wire. Structural frontiers use the ordinary storage operand vocabulary:
//! neither serialization nor the frontier commitment may erase loan access.

use crate::sections::semantic_module::CodecError;
use crate::sections::semantic_module::scalar_wire::{decode_scalar_type, encode_scalar_type};
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_path, encode_structural_path,
};
use crate::sections::semantic_module::structural_signature_wire::{
    decode_structural_access, encode_structural_access,
};
use crate::sections::semantic_module::wire::{Reader, Writer, decode_counted, decode_ids};
use arena::{Handle, HandleSpan};
use language_semantics::{CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
use terminal_psi::{
    TerminalSuspensionCallPlan, TerminalSuspensionCallSite, TerminalSuspensionCallTarget,
    TerminalSuspensionLiveValue, TerminalSuspensionLoan, TerminalSuspensionLoanBacking,
    TerminalSuspensionLoanCapture, TerminalSuspensionLoanResource, TerminalSuspensionLoans,
    TerminalSuspensionPlace, TerminalSuspensionStorage, TerminalSuspensionValueLoans,
    TerminalSuspensionValueType,
};

pub(super) fn encode_carry_policy(writer: &mut Writer, policy: CarryPolicy) {
    writer.u8(match policy.suspension {
        CarrySuspension::Forbidden => 1,
        CarrySuspension::Allowed => 2,
    });
    writer.u8(match policy.cpu {
        CarryCpu::Origin => 1,
        CarryCpu::Any => 2,
    });
    writer.u8(match policy.host_thread {
        CarryHostThread::Origin => 1,
        CarryHostThread::Any => 2,
    });
    writer.u8(match policy.address {
        CarryAddress::Stable => 1,
        CarryAddress::Movable => 2,
    });
}

pub(super) fn decode_carry_policy(reader: &mut Reader<'_>) -> Result<CarryPolicy, CodecError> {
    Ok(CarryPolicy {
        suspension: match reader.u8()? {
            1 => CarrySuspension::Forbidden,
            2 => CarrySuspension::Allowed,
            tag => return Err(CodecError::InvalidTag("CarrySuspension", tag)),
        },
        cpu: match reader.u8()? {
            1 => CarryCpu::Origin,
            2 => CarryCpu::Any,
            tag => return Err(CodecError::InvalidTag("CarryCpu", tag)),
        },
        host_thread: match reader.u8()? {
            1 => CarryHostThread::Origin,
            2 => CarryHostThread::Any,
            tag => return Err(CodecError::InvalidTag("CarryHostThread", tag)),
        },
        address: match reader.u8()? {
            1 => CarryAddress::Stable,
            2 => CarryAddress::Movable,
            tag => return Err(CodecError::InvalidTag("CarryAddress", tag)),
        },
    })
}

pub(crate) fn encode_suspension_call_plan(
    writer: &mut Writer,
    plan: &TerminalSuspensionCallPlan,
) -> Result<(), CodecError> {
    writer.id(plan.operation);
    writer.id(plan.crossing);
    encode_suspension_call_target(writer, plan.target);
    encode_carry_policy(writer, plan.effective);
    writer.u32(plan.live_value_count);
    writer.len("suspension live values", plan.live_values.len())?;
    for live in &plan.live_values {
        match &live.place {
            TerminalSuspensionPlace::Scalar(value) => {
                writer.u8(1);
                writer.id(*value);
            }
            TerminalSuspensionPlace::Structural(argument) => {
                writer.u8(2);
                writer.id(argument.place);
                encode_structural_access(writer, argument.access);
                encode_structural_path(writer, "suspension structural path", &argument.path)?;
            }
        }
        match live.value_type {
            TerminalSuspensionValueType::Scalar(scalar_type) => {
                writer.u8(1);
                encode_scalar_type(writer, scalar_type);
            }
            TerminalSuspensionValueType::Structural(structural_type) => {
                writer.u8(2);
                writer.id(structural_type);
            }
        }
        writer.u8(match live.storage {
            TerminalSuspensionStorage::Parameter => 2,
            TerminalSuspensionStorage::Local => 3,
            TerminalSuspensionStorage::CallArgument => 4,
        });
        writer.u32(live.claim_count);
        writer.len("suspension live claims", live.claims.len())?;
        for claim in &live.claims {
            writer.id(*claim);
        }
        encode_carry_policy(writer, live.effective);
        match live.loans {
            TerminalSuspensionValueLoans::Unresolved => writer.u8(0),
            TerminalSuspensionValueLoans::ViewFree => writer.u8(1),
            TerminalSuspensionValueLoans::Retained { links } => {
                writer.u8(2);
                encode_span(writer, links);
            }
        }
    }
    encode_loans(writer, &plan.loans)?;
    Ok(())
}

pub(crate) fn encode_suspension_call_site(writer: &mut Writer, site: &TerminalSuspensionCallSite) {
    writer.id(site.operation);
    writer.id(site.crossing);
    encode_suspension_call_target(writer, site.target);
    writer.bytes(&site.frontier_commitment);
}

pub(crate) fn decode_suspension_call_site(
    reader: &mut Reader<'_>,
) -> Result<TerminalSuspensionCallSite, CodecError> {
    Ok(TerminalSuspensionCallSite {
        operation: reader.id("OperationId")?,
        crossing: reader.id("SuspensionCrossingId")?,
        target: decode_suspension_call_target(reader)?,
        frontier_commitment: reader.array()?,
    })
}

fn encode_suspension_call_target(writer: &mut Writer, target: TerminalSuspensionCallTarget) {
    match target {
        TerminalSuspensionCallTarget::Machine(machine) => {
            writer.u8(1);
            writer.id(machine);
        }
        TerminalSuspensionCallTarget::Boundary(boundary) => {
            writer.u8(2);
            writer.id(boundary);
        }
    }
}

fn decode_suspension_call_target(
    reader: &mut Reader<'_>,
) -> Result<TerminalSuspensionCallTarget, CodecError> {
    match reader.u8()? {
        1 => Ok(TerminalSuspensionCallTarget::Machine(
            reader.id("MachineId")?,
        )),
        2 => Ok(TerminalSuspensionCallTarget::Boundary(
            reader.id("BoundaryMachineId")?,
        )),
        tag => Err(CodecError::InvalidTag("TerminalSuspensionCallTarget", tag)),
    }
}

pub(crate) fn decode_suspension_call_plan(
    reader: &mut Reader<'_>,
) -> Result<TerminalSuspensionCallPlan, CodecError> {
    let operation = reader.id("OperationId")?;
    let crossing = reader.id("SuspensionCrossingId")?;
    let target = decode_suspension_call_target(reader)?;
    let effective = decode_carry_policy(reader)?;
    let live_value_count = reader.u32()?;
    let live_values = decode_counted(reader, |reader| {
        let place = match reader.u8()? {
            1 => TerminalSuspensionPlace::Scalar(reader.id("ValueId")?),
            2 => TerminalSuspensionPlace::Structural(terminal_psi::StructuralArgument {
                place: reader.id("PlaceId")?,
                access: decode_structural_access(reader)?,
                path: decode_structural_path(reader)?,
            }),
            tag => return Err(CodecError::InvalidTag("TerminalSuspensionPlace", tag)),
        };
        let value_type = match reader.u8()? {
            1 => TerminalSuspensionValueType::Scalar(decode_scalar_type(reader)?),
            2 => TerminalSuspensionValueType::Structural(reader.id("StructuralTypeId")?),
            tag => return Err(CodecError::InvalidTag("TerminalSuspensionValueType", tag)),
        };
        let storage = match reader.u8()? {
            2 => TerminalSuspensionStorage::Parameter,
            3 => TerminalSuspensionStorage::Local,
            4 => TerminalSuspensionStorage::CallArgument,
            tag => return Err(CodecError::InvalidTag("TerminalSuspensionStorage", tag)),
        };
        Ok(TerminalSuspensionLiveValue {
            place,
            value_type,
            storage,
            claim_count: reader.u32()?,
            claims: decode_ids(reader, "ClaimId")?,
            effective: decode_carry_policy(reader)?,
            loans: match reader.u8()? {
                0 => TerminalSuspensionValueLoans::Unresolved,
                1 => TerminalSuspensionValueLoans::ViewFree,
                2 => TerminalSuspensionValueLoans::Retained {
                    links: decode_span(reader)?,
                },
                tag => return Err(CodecError::InvalidTag("TerminalSuspensionValueLoans", tag)),
            },
        })
    })?;
    let loans = decode_loans(reader)?;
    validate_loan_coordinates(&loans, &live_values)?;
    Ok(TerminalSuspensionCallPlan {
        operation,
        crossing,
        target,
        effective,
        live_value_count,
        live_values,
        loans,
    })
}

fn encode_handle<T>(writer: &mut Writer, handle: Handle<T>) {
    writer.u32(handle.arena_index());
    writer.u32(handle.generation());
}

fn decode_handle<T>(reader: &mut Reader<'_>) -> Result<Handle<T>, CodecError> {
    let slot = reader.u32()?;
    let generation = reader.u32()?;
    if (slot == 0 && generation != 0) || (slot != 0 && generation != 1) {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(Handle::from_parts(slot, generation))
}

fn encode_span<T>(writer: &mut Writer, span: HandleSpan<T>) {
    encode_handle(writer, span.start());
    writer.u32(span.count());
}

fn decode_span<T>(reader: &mut Reader<'_>) -> Result<HandleSpan<T>, CodecError> {
    let start = decode_handle(reader)?;
    let count = reader.count()?;
    if (count == 0) == start.is_valid() {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(HandleSpan::from_parts_or_empty(start, count))
}

fn encode_loans(writer: &mut Writer, loans: &TerminalSuspensionLoans) -> Result<(), CodecError> {
    let paths: Vec<_> = loans
        .paths
        .iter()
        .map(|(_, segment)| segment.clone())
        .collect();
    encode_structural_path(writer, "suspension loan paths", &paths)?;
    writer.len("suspension loan links", loans.value_links.len())?;
    for (_, link) in loans.value_links.iter() {
        encode_handle(writer, *link);
    }
    writer.len("suspension loan suppliers", loans.supplier_links.len())?;
    for (_, supplier) in loans.supplier_links.iter() {
        encode_handle(writer, *supplier);
    }
    writer.len("suspension loans", loans.occurrences.len())?;
    for (_, loan) in loans.occurrences.iter() {
        match &loan.capture {
            TerminalSuspensionLoanCapture::Unresolved => writer.u8(0),
            TerminalSuspensionLoanCapture::Reference {
                formation,
                carrier,
                source,
                source_access,
                source_path,
            } => {
                writer.u8(1);
                writer.id(*formation);
                writer.id(*carrier);
                writer.id(*source);
                encode_structural_access(writer, *source_access);
                encode_span(writer, *source_path);
            }
            TerminalSuspensionLoanCapture::CallArgument { live_value, path } => {
                writer.u8(2);
                writer.u32(*live_value);
                encode_span(writer, *path);
            }
        }
        match &loan.backing {
            TerminalSuspensionLoanBacking::Unresolved => writer.u8(0),
            TerminalSuspensionLoanBacking::RetainedOwner { live_value } => {
                writer.u8(1);
                writer.u32(*live_value);
            }
            TerminalSuspensionLoanBacking::IncomingReferent { argument } => {
                writer.u8(2);
                writer.id(argument.place);
                encode_structural_access(writer, argument.access);
                encode_structural_path(writer, "suspension incoming referent", &argument.path)?;
            }
        }
        writer.u8(match loan.resource {
            TerminalSuspensionLoanResource::Unresolved => 0,
            TerminalSuspensionLoanResource::DirectRoot => 1,
            TerminalSuspensionLoanResource::Reborrow => 2,
        });
        encode_handle(writer, loan.parent);
        encode_span(writer, loan.suppliers);
        encode_structural_access(writer, loan.access);
        writer.boolean(loan.active_at_call);
    }
    Ok(())
}

fn decode_loans(reader: &mut Reader<'_>) -> Result<TerminalSuspensionLoans, CodecError> {
    let mut loans = TerminalSuspensionLoans::default();
    for segment in decode_structural_path(reader)? {
        loans.paths.insert(segment);
    }
    for link in decode_counted(reader, decode_handle)? {
        loans.value_links.insert(link);
    }
    for supplier in decode_counted(reader, decode_handle)? {
        loans.supplier_links.insert(supplier);
    }
    for loan in decode_counted(reader, |reader| {
        let capture = match reader.u8()? {
            0 => TerminalSuspensionLoanCapture::Unresolved,
            1 => TerminalSuspensionLoanCapture::Reference {
                formation: reader.id("OperationId")?,
                carrier: reader.id("PlaceId")?,
                source: reader.id("PlaceId")?,
                source_access: decode_structural_access(reader)?,
                source_path: decode_span(reader)?,
            },
            2 => TerminalSuspensionLoanCapture::CallArgument {
                live_value: reader.u32()?,
                path: decode_span(reader)?,
            },
            tag => return Err(CodecError::InvalidTag("TerminalSuspensionLoanCapture", tag)),
        };
        let backing = match reader.u8()? {
            0 => TerminalSuspensionLoanBacking::Unresolved,
            1 => TerminalSuspensionLoanBacking::RetainedOwner {
                live_value: reader.u32()?,
            },
            2 => TerminalSuspensionLoanBacking::IncomingReferent {
                argument: terminal_psi::StructuralArgument {
                    place: reader.id("PlaceId")?,
                    access: decode_structural_access(reader)?,
                    path: decode_structural_path(reader)?,
                },
            },
            tag => return Err(CodecError::InvalidTag("TerminalSuspensionLoanBacking", tag)),
        };
        let resource = match reader.u8()? {
            0 => TerminalSuspensionLoanResource::Unresolved,
            1 => TerminalSuspensionLoanResource::DirectRoot,
            2 => TerminalSuspensionLoanResource::Reborrow,
            tag => {
                return Err(CodecError::InvalidTag(
                    "TerminalSuspensionLoanResource",
                    tag,
                ));
            }
        };
        Ok(TerminalSuspensionLoan {
            capture,
            backing,
            resource,
            parent: decode_handle(reader)?,
            suppliers: decode_span(reader)?,
            access: decode_structural_access(reader)?,
            active_at_call: reader.boolean()?,
        })
    })? {
        loans.occurrences.insert(loan);
    }
    Ok(loans)
}

// This checks foreign coordinates, not source borrow judgments. Decoding a
// well-formed graph grants no lifetime, access or runtime preservation.
fn validate_loan_coordinates(
    loans: &TerminalSuspensionLoans,
    live_values: &[TerminalSuspensionLiveValue],
) -> Result<(), CodecError> {
    for (_, link) in loans.value_links.iter() {
        if !loans.occurrences.is_valid(*link) {
            return Err(CodecError::NonCanonicalEncoding);
        }
    }
    for (_, supplier) in loans.supplier_links.iter() {
        if !loans.occurrences.is_valid(*supplier) {
            return Err(CodecError::NonCanonicalEncoding);
        }
    }
    for (_, loan) in loans.occurrences.iter() {
        if (loan.parent.is_valid() && !loans.occurrences.is_valid(loan.parent))
            || loans.supplier_links.span(loan.suppliers).is_none()
        {
            return Err(CodecError::NonCanonicalEncoding);
        }
        match loan.capture {
            TerminalSuspensionLoanCapture::Reference { source_path, .. }
                if loans.paths.span(source_path).is_none() =>
            {
                return Err(CodecError::NonCanonicalEncoding);
            }
            TerminalSuspensionLoanCapture::CallArgument { live_value, path }
                if loans.paths.span(path).is_none()
                    || !live_values.get(live_value as usize).is_some_and(|live| {
                        live.storage == TerminalSuspensionStorage::CallArgument
                            && matches!(live.place, TerminalSuspensionPlace::Structural(_))
                    }) =>
            {
                return Err(CodecError::NonCanonicalEncoding);
            }
            _ => {}
        }
        if let TerminalSuspensionLoanBacking::RetainedOwner { live_value } = loan.backing
            && live_value as usize >= live_values.len()
        {
            return Err(CodecError::NonCanonicalEncoding);
        }
    }
    for live in live_values {
        if let TerminalSuspensionValueLoans::Retained { links } = live.loans
            && (links.is_empty() || loans.value_links.span(links).is_none())
        {
            return Err(CodecError::NonCanonicalEncoding);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{decode_suspension_call_plan, encode_suspension_call_plan};
    use crate::codec_error::CodecError;
    use crate::sections::semantic_module::wire::{Reader, Writer};
    use language_semantics::CarryPolicy;
    use semantic_vocabulary::{
        MachineId, ObligationId, OperationId, PlaceId, StructuralCaseId, StructuralTypeId,
        SuspensionCrossingId, ValueId,
    };
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralPathSegment, TerminalSuspensionCallPlan,
        TerminalSuspensionCallTarget, TerminalSuspensionLiveValue, TerminalSuspensionPlace,
        TerminalSuspensionStorage, TerminalSuspensionValueType, suspension_frontier_commitment,
    };

    #[test]
    fn suspension_storage_access_survives_wire_and_changes_identity() {
        let table = terminal_psi::TerminalIdentityTable::from(
            (0..64).map(|i| format!("identity-{i}")).collect::<Vec<_>>(),
        );
        let mut encodings = Vec::new();
        let mut commitments = Vec::new();
        for access in [
            StructuralAccess::Owned,
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            let selector = ValueId::new(7).expect("nonzero selector");
            let plan = TerminalSuspensionCallPlan {
                operation: OperationId::new(1).expect("nonzero operation"),
                crossing: SuspensionCrossingId::new(2).expect("nonzero crossing"),
                target: TerminalSuspensionCallTarget::Machine(
                    MachineId::new(3).expect("nonzero machine"),
                ),
                effective: CarryPolicy::STRICT,
                live_value_count: 1,
                live_values: vec![TerminalSuspensionLiveValue {
                    place: TerminalSuspensionPlace::Structural(StructuralArgument {
                        place: PlaceId::new(4).expect("nonzero place"),
                        access,
                        path: vec![
                            StructuralPathSegment::Case(
                                StructuralCaseId::new(5).expect("nonzero case"),
                            ),
                            StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(13)),
                            StructuralPathSegment::Referent,
                            StructuralPathSegment::FixedIndex(2),
                            StructuralPathSegment::FixedByteRange { start: 1, end: 4 },
                            StructuralPathSegment::RuntimeIndex {
                                index: selector,
                                obligation: ObligationId::new(8).expect("nonzero obligation"),
                            },
                        ],
                    }),
                    value_type: TerminalSuspensionValueType::Structural(
                        StructuralTypeId::new(6).expect("nonzero structural type"),
                    ),
                    storage: TerminalSuspensionStorage::Local,
                    claim_count: 0,
                    claims: Vec::new(),
                    effective: CarryPolicy::STRICT,
                    loans: terminal_psi::TerminalSuspensionValueLoans::Unresolved,
                }],
                loans: terminal_psi::TerminalSuspensionLoans::default(),
            };
            let mut writer = Writer::default();
            encode_suspension_call_plan(&mut writer, &plan).expect("encode frontier");
            let encoded = writer.finish();
            let mut reader = Reader::new(&encoded);
            let decoded = decode_suspension_call_plan(&mut reader).expect("decode frontier");
            assert_eq!(decoded, plan);
            assert_eq!(reader.remaining(), 0);

            let mut retained_values = Vec::new();
            decoded.live_values[0]
                .place
                .visit_value_ids(|value| retained_values.push(value));
            assert_eq!(retained_values, vec![selector]);
            // Equal physical coordinates can occur in two formal positions.
            // The wire must keep both occurrence positions, not coalesce
            // equal storage or erase their ordering from the commitment.
            let mut repeated = plan.clone();
            repeated.live_values.push(repeated.live_values[0].clone());
            repeated.live_value_count = 2;
            let mut writer = Writer::default();
            encode_suspension_call_plan(&mut writer, &repeated).expect("encode repeated storage");
            let bytes = writer.finish();
            assert_eq!(
                decode_suspension_call_plan(&mut Reader::new(&bytes))
                    .expect("decode repeated storage"),
                repeated
            );
            assert_ne!(
                suspension_frontier_commitment(&table, &repeated),
                suspension_frontier_commitment(&table, &plan)
            );
            let commitment = suspension_frontier_commitment(&table, &decoded);
            assert!(!encodings.contains(&encoded));
            assert!(!commitments.contains(&commitment));
            encodings.push(encoded);
            commitments.push(commitment);
        }
    }

    #[test]
    fn suspension_access_wire_rejects_accessless_module_format() {
        let mut encoded = super::super::super::MAGIC.to_vec();
        encoded.extend_from_slice(&130_u16.to_le_bytes());
        assert!(matches!(
            super::super::super::decode_module(&encoded),
            Err(CodecError::UnsupportedFormatMarker(130))
        ));
    }

    #[test]
    fn module_format_rejects_previous_layout() {
        let mut encoded = super::super::super::MAGIC.to_vec();
        let previous = super::super::super::FORMAT_MARKER - 1;
        encoded.extend_from_slice(&previous.to_le_bytes());
        assert!(matches!(
            super::super::super::decode_module(&encoded),
            Err(CodecError::UnsupportedFormatMarker(marker)) if marker == previous
        ));
    }

    #[test]
    fn suspension_loan_graph_round_trips_capture_and_dependency_identity() {
        let table = terminal_psi::TerminalIdentityTable::from(
            (0..64).map(|i| format!("identity-{i}")).collect::<Vec<_>>(),
        );
        use arena::HandleSpan;
        use terminal_psi::{
            TerminalSuspensionLoan, TerminalSuspensionLoanBacking, TerminalSuspensionLoanCapture,
            TerminalSuspensionLoanResource, TerminalSuspensionLoans, TerminalSuspensionValueLoans,
        };
        let selector = ValueId::new(17).expect("selector");
        let mut loans = TerminalSuspensionLoans::default();
        let path = loans
            .paths
            .insert_many([StructuralPathSegment::RuntimeIndex {
                index: selector,
                obligation: ObligationId::new(18).expect("bounds obligation"),
            }]);
        let parent = loans.occurrences.insert(TerminalSuspensionLoan {
            capture: TerminalSuspensionLoanCapture::Reference {
                formation: OperationId::new(5).expect("formation"),
                carrier: PlaceId::new(6).expect("carrier"),
                source: PlaceId::new(7).expect("backing"),
                source_access: StructuralAccess::Owned,
                source_path: path,
            },
            backing: TerminalSuspensionLoanBacking::RetainedOwner { live_value: 0 },
            resource: TerminalSuspensionLoanResource::DirectRoot,
            ..Default::default()
        });
        let alternative = loans.occurrences.insert(TerminalSuspensionLoan {
            resource: TerminalSuspensionLoanResource::DirectRoot,
            ..Default::default()
        });
        let suppliers = loans.supplier_links.insert_many([parent, alternative]);
        let child = loans.occurrences.insert(TerminalSuspensionLoan {
            parent,
            suppliers,
            resource: TerminalSuspensionLoanResource::Reborrow,
            access: StructuralAccess::WriteOnlyBorrow,
            active_at_call: true,
            ..Default::default()
        });
        let links = loans.value_links.insert_many([child]);
        let plan = TerminalSuspensionCallPlan {
            operation: OperationId::new(1).expect("call"),
            crossing: SuspensionCrossingId::new(2).expect("crossing"),
            target: TerminalSuspensionCallTarget::Machine(MachineId::new(3).expect("machine")),
            effective: CarryPolicy::STRICT,
            live_value_count: 1,
            live_values: vec![TerminalSuspensionLiveValue {
                place: TerminalSuspensionPlace::Structural(StructuralArgument {
                    place: PlaceId::new(7).expect("backing"),
                    access: StructuralAccess::Owned,
                    path: vec![],
                }),
                value_type: TerminalSuspensionValueType::Structural(
                    StructuralTypeId::new(8).expect("type"),
                ),
                storage: TerminalSuspensionStorage::Local,
                claim_count: 0,
                claims: vec![],
                effective: CarryPolicy::STRICT,
                loans: TerminalSuspensionValueLoans::Retained { links },
            }],
            loans,
        };
        let mut writer = Writer::default();
        encode_suspension_call_plan(&mut writer, &plan).expect("encode graph");
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        let decoded = decode_suspension_call_plan(&mut reader).expect("decode graph");
        assert_eq!(decoded, plan);
        assert_eq!(reader.remaining(), 0);
        let mut selectors = Vec::new();
        decoded.visit_value_ids(|value| selectors.push(value));
        assert_eq!(selectors, vec![selector]);
        let commitment = suspension_frontier_commitment(&table, &plan);
        let mut changed = plan.clone();
        changed.loans.occurrences.get_mut(child).suppliers = Default::default();
        assert_ne!(suspension_frontier_commitment(&table, &changed), commitment);
        changed = plan.clone();
        *changed.loans.supplier_links.get_mut(suppliers.start()) = alternative;
        assert_ne!(suspension_frontier_commitment(&table, &changed), commitment);
        changed = plan.clone();
        changed.loans.occurrences.get_mut(child).suppliers =
            HandleSpan::from_parts(suppliers.start(), suppliers.count() + 1);
        let mut writer = Writer::default();
        encode_suspension_call_plan(&mut writer, &changed).expect("encode invalid supplier span");
        assert!(decode_suspension_call_plan(&mut Reader::new(&writer.finish())).is_err());
        changed = plan.clone();
        *changed.loans.supplier_links.get_mut(suppliers.start()) = Default::default();
        let mut writer = Writer::default();
        encode_suspension_call_plan(&mut writer, &changed).expect("encode absent supplier");
        assert!(decode_suspension_call_plan(&mut Reader::new(&writer.finish())).is_err());
        changed = plan.clone();
        changed.loans.occurrences.get_mut(parent).capture =
            TerminalSuspensionLoanCapture::Reference {
                formation: OperationId::new(9).expect("different formation"),
                carrier: PlaceId::new(6).expect("carrier"),
                source: PlaceId::new(7).expect("backing"),
                source_access: StructuralAccess::Owned,
                source_path: HandleSpan::empty(),
            };
        assert_ne!(suspension_frontier_commitment(&table, &changed), commitment);
        changed = plan;
        changed.live_values[0].storage = TerminalSuspensionStorage::CallArgument;
        changed.loans.occurrences.get_mut(child).capture =
            TerminalSuspensionLoanCapture::CallArgument {
                live_value: 0,
                path: HandleSpan::empty(),
            };
        let mut writer = Writer::default();
        encode_suspension_call_plan(&mut writer, &changed).expect("encode actual capture");
        let decoded = decode_suspension_call_plan(&mut Reader::new(&writer.finish()))
            .expect("decode actual capture");
        assert_eq!(decoded, changed);
        let actual_commitment = suspension_frontier_commitment(&table, &changed);
        changed.loans.occurrences.get_mut(child).capture =
            TerminalSuspensionLoanCapture::CallArgument {
                live_value: 1,
                path: HandleSpan::empty(),
            };
        assert_ne!(
            suspension_frontier_commitment(&table, &changed),
            actual_commitment
        );
        let mut writer = Writer::default();
        encode_suspension_call_plan(&mut writer, &changed).expect("encode absent actual");
        assert!(decode_suspension_call_plan(&mut Reader::new(&writer.finish())).is_err());
        changed.loans.occurrences.get_mut(child).capture =
            TerminalSuspensionLoanCapture::CallArgument {
                live_value: 0,
                path,
            };
        assert_ne!(
            suspension_frontier_commitment(&table, &changed),
            actual_commitment
        );
        changed.live_values[0].storage = TerminalSuspensionStorage::Local;
        let mut writer = Writer::default();
        encode_suspension_call_plan(&mut writer, &changed).expect("encode wrong storage role");
        assert!(decode_suspension_call_plan(&mut Reader::new(&writer.finish())).is_err());
        changed.live_values[0].loans = TerminalSuspensionValueLoans::Retained {
            links: HandleSpan::from_parts(links.start(), links.count() + 1),
        };
        let mut writer = Writer::default();
        encode_suspension_call_plan(&mut writer, &changed)
            .expect("encode invalid foreign coordinates");
        assert!(decode_suspension_call_plan(&mut Reader::new(&writer.finish())).is_err());
    }
}
