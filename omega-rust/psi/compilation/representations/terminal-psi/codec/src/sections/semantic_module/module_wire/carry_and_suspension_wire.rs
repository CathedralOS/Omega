//! Carry policies and suspension call plans, sites and targets on the
//! wire. Structural frontiers use the ordinary storage operand vocabulary:
//! neither serialization nor the frontier commitment may erase loan access.

use crate::sections::semantic_module::CodecError;
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_path, encode_structural_path,
};
use crate::sections::semantic_module::wire::{Reader, Writer};
use crate::sections::semantic_module::wire_fields::{
    Access, Argument, Digest, Field, Flag, Id, Ids, List, Scalar, U32, wire_enum, wire_record,
};
use arena::{Handle, HandleSpan};
use language_semantics::{CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
use terminal_psi::{
    TerminalSuspensionCallPlan, TerminalSuspensionCallSite, TerminalSuspensionCallTarget,
    TerminalSuspensionLiveValue, TerminalSuspensionLoan, TerminalSuspensionLoanBacking,
    TerminalSuspensionLoanCapture, TerminalSuspensionLoanResource, TerminalSuspensionLoans,
    TerminalSuspensionPlace, TerminalSuspensionStorage, TerminalSuspensionValueLoans,
    TerminalSuspensionValueType,
};

wire_record! {
    Carry: CarryPolicy {
        suspension: Suspension,
        cpu: Cpu,
        host_thread: HostThread,
        address: Address,
    }
}

wire_enum! {
    Suspension: CarrySuspension, "CarrySuspension" { 1 => Forbidden, 2 => Allowed }
    Cpu: CarryCpu, "CarryCpu" { 1 => Origin, 2 => Any }
    HostThread: CarryHostThread, "CarryHostThread" { 1 => Origin, 2 => Any }
    Address: CarryAddress, "CarryAddress" { 1 => Stable, 2 => Movable }
}

wire_record! {
    LiveValue: TerminalSuspensionLiveValue {
        place: Place,
        value_type: ValueType,
        storage: Storage,
        claim_count: U32,
        claims: Ids,
        effective: Carry,
        loans: ValueLoans,
    }
    CallSite: TerminalSuspensionCallSite {
        operation: Id,
        crossing: Id,
        target: CallTarget,
        frontier_commitment: Digest,
    }
    Loan: TerminalSuspensionLoan {
        capture: Capture,
        backing: Backing,
        resource: Resource,
        parent: HandleKind,
        suppliers: SpanKind,
        access: Access,
        active_at_call: Flag,
    }
}

wire_enum! {
    Place: TerminalSuspensionPlace, "TerminalSuspensionPlace" {
        1 => Scalar(value: Id),
        2 => Structural(argument: Argument),
    }
    ValueType: TerminalSuspensionValueType, "TerminalSuspensionValueType" {
        1 => Scalar(scalar: Scalar),
        2 => Structural(identity: Id),
    }
    Storage: TerminalSuspensionStorage, "TerminalSuspensionStorage" {
        2 => Parameter,
        3 => Local,
        4 => CallArgument,
    }
    ValueLoans: TerminalSuspensionValueLoans, "TerminalSuspensionValueLoans" {
        0 => Unresolved,
        1 => ViewFree,
        2 => Retained { links: SpanKind },
    }
    CallTarget: TerminalSuspensionCallTarget, "TerminalSuspensionCallTarget" {
        1 => Machine(machine: Id),
        2 => Boundary(boundary: Id),
        5 => Dynamic { interface: U32, slot: U32 },
    }
    Capture: TerminalSuspensionLoanCapture, "TerminalSuspensionLoanCapture" {
        0 => Unresolved,
        1 => Reference {
            formation: Id,
            carrier: Id,
            source: Id,
            source_access: Access,
            source_path: SpanKind,
        },
        2 => RetainedValue { live_value: U32, path: SpanKind },
    }
    Backing: TerminalSuspensionLoanBacking, "TerminalSuspensionLoanBacking" {
        0 => Unresolved,
        1 => RetainedOwner { live_value: U32 },
        2 => IncomingReferent { argument: Argument },
    }
    Resource: TerminalSuspensionLoanResource, "TerminalSuspensionLoanResource" {
        0 => Unresolved,
        1 => DirectRoot,
        2 => Reborrow,
        3 => IncomingReference,
    }
}

/// An arena handle in canonical slot form: zero is absence, and a present
/// handle is generation one, so the wire never carries reuse history.
struct HandleKind;
impl<T> Field<Handle<T>> for HandleKind {
    fn write(writer: &mut Writer, value: &Handle<T>) -> Result<(), CodecError> {
        writer.u32(value.arena_index());
        writer.u32(value.generation());
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Handle<T>, CodecError> {
        let slot = reader.u32()?;
        let generation = reader.u32()?;
        if (slot == 0 && generation != 0) || (slot != 0 && generation != 1) {
            return Err(CodecError::NonCanonicalEncoding);
        }
        Ok(Handle::from_parts(slot, generation))
    }
}

/// A handle span; only the empty span starts at the absent handle.
struct SpanKind;
impl<T> Field<HandleSpan<T>> for SpanKind {
    fn write(writer: &mut Writer, value: &HandleSpan<T>) -> Result<(), CodecError> {
        HandleKind::write(writer, &value.start())?;
        writer.u32(value.count());
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<HandleSpan<T>, CodecError> {
        let start: Handle<T> = HandleKind::read(reader)?;
        let count = reader.count()?;
        if (count == 0) == start.is_valid() {
            return Err(CodecError::NonCanonicalEncoding);
        }
        Ok(HandleSpan::from_parts_or_empty(start, count))
    }
}

pub(crate) fn encode_suspension_call_plan(
    writer: &mut Writer,
    plan: &TerminalSuspensionCallPlan,
) -> Result<(), CodecError> {
    writer.id(plan.operation);
    writer.id(plan.crossing);
    CallTarget::write(writer, &plan.target)?;
    Carry::write(writer, &plan.effective)?;
    writer.u32(plan.live_value_count);
    List::<LiveValue>::write(writer, &plan.live_values)?;
    encode_loans(writer, &plan.loans)
}

pub(crate) fn decode_suspension_call_plan(
    reader: &mut Reader<'_>,
) -> Result<TerminalSuspensionCallPlan, CodecError> {
    let operation = reader.id("OperationId")?;
    let crossing = reader.id("SuspensionCrossingId")?;
    let target = CallTarget::read(reader)?;
    let effective = Carry::read(reader)?;
    let live_value_count = reader.u32()?;
    let live_values = List::<LiveValue>::read(reader)?;
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

/// Loan arenas cross as their rows in arena order; decoding reinserts them.
fn encode_loans(writer: &mut Writer, loans: &TerminalSuspensionLoans) -> Result<(), CodecError> {
    let paths: Vec<_> = loans
        .paths
        .iter()
        .map(|(_, segment)| segment.clone())
        .collect();
    encode_structural_path(writer, "suspension loan paths", &paths)?;
    writer.len("suspension loan links", loans.value_links.len())?;
    for (_, link) in loans.value_links.iter() {
        HandleKind::write(writer, link)?;
    }
    writer.len("suspension loan suppliers", loans.supplier_links.len())?;
    for (_, supplier) in loans.supplier_links.iter() {
        HandleKind::write(writer, supplier)?;
    }
    writer.len("suspension loans", loans.occurrences.len())?;
    for (_, loan) in loans.occurrences.iter() {
        Loan::write(writer, loan)?;
    }
    Ok(())
}

fn decode_loans(reader: &mut Reader<'_>) -> Result<TerminalSuspensionLoans, CodecError> {
    let mut loans = TerminalSuspensionLoans::default();
    for segment in decode_structural_path(reader)? {
        loans.paths.insert(segment);
    }
    for link in List::<HandleKind>::read(reader)? {
        loans.value_links.insert(link);
    }
    for supplier in List::<HandleKind>::read(reader)? {
        loans.supplier_links.insert(supplier);
    }
    for loan in List::<Loan>::read(reader)? {
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
            TerminalSuspensionLoanCapture::RetainedValue { live_value, path }
                if loans.paths.span(path).is_none()
                    || !live_values.get(live_value as usize).is_some_and(|live| {
                        live.storage
                            == if loan.resource == TerminalSuspensionLoanResource::IncomingReference
                            {
                                TerminalSuspensionStorage::Parameter
                            } else {
                                TerminalSuspensionStorage::CallArgument
                            }
                            && matches!(live.place, TerminalSuspensionPlace::Structural(_))
                    }) =>
            {
                return Err(CodecError::NonCanonicalEncoding);
            }
            _ => {}
        }
        if loan.resource == TerminalSuspensionLoanResource::IncomingReference
            && (!matches!(
                loan.capture,
                TerminalSuspensionLoanCapture::RetainedValue { .. }
            ) || !matches!(
                loan.backing,
                TerminalSuspensionLoanBacking::IncomingReferent { .. }
            ) || loan.parent.is_valid()
                || !loan.suppliers.is_empty()
                || !loan.active_at_call)
        {
            return Err(CodecError::NonCanonicalEncoding);
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
    use super::{CallTarget, Field, decode_suspension_call_plan, encode_suspension_call_plan};
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
    fn dynamic_suspension_target_keeps_its_slot_and_rejects_retired_tags() {
        for target in [
            TerminalSuspensionCallTarget::Dynamic {
                interface: 0,
                slot: 0,
            },
            TerminalSuspensionCallTarget::Dynamic {
                interface: 7,
                slot: 13,
            },
        ] {
            let mut writer = Writer::default();
            CallTarget::write(&mut writer, &target).unwrap();
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            assert_eq!(
                CallTarget::read(&mut reader).expect("selector target"),
                target
            );
            assert_eq!(reader.remaining(), 0);
            assert_eq!(bytes[0], 5);
        }
        for retired in [3, 4] {
            let bytes = [retired];
            assert!(matches!(
                CallTarget::read(&mut Reader::new(&bytes)),
                Err(CodecError::InvalidTag("TerminalSuspensionCallTarget", tag)) if tag == retired
            ));
        }
    }

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
                            StructuralPathSegment::FixedRange { start: 1, end: 4 },
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
        let mut ingress = plan.clone();
        ingress.live_values[0].storage = TerminalSuspensionStorage::Parameter;
        let argument = StructuralArgument {
            place: PlaceId::new(7).expect("ingress place"),
            path: Vec::new(),
            access: StructuralAccess::MutableBorrow,
        };
        ingress.live_values[0].place = TerminalSuspensionPlace::Structural(argument.clone());
        *ingress.loans.occurrences.get_mut(child) = TerminalSuspensionLoan {
            capture: TerminalSuspensionLoanCapture::RetainedValue {
                live_value: 0,
                path: HandleSpan::empty(),
            },
            backing: TerminalSuspensionLoanBacking::IncomingReferent { argument },
            resource: TerminalSuspensionLoanResource::IncomingReference,
            access: StructuralAccess::MutableBorrow,
            active_at_call: true,
            ..Default::default()
        };
        let mut writer = Writer::default();
        encode_suspension_call_plan(&mut writer, &ingress).expect("encode ingress capture");
        assert_eq!(
            decode_suspension_call_plan(&mut Reader::new(&writer.finish()))
                .expect("decode ingress capture"),
            ingress
        );
        assert_ne!(suspension_frontier_commitment(&table, &ingress), commitment);
        ingress.live_values[0].storage = TerminalSuspensionStorage::CallArgument;
        let mut writer = Writer::default();
        encode_suspension_call_plan(&mut writer, &ingress).expect("encode wrong ingress role");
        assert!(decode_suspension_call_plan(&mut Reader::new(&writer.finish())).is_err());
        ingress.live_values[0].storage = TerminalSuspensionStorage::Parameter;
        ingress.loans.occurrences.get_mut(child).backing =
            TerminalSuspensionLoanBacking::RetainedOwner { live_value: 0 };
        let mut writer = Writer::default();
        encode_suspension_call_plan(&mut writer, &ingress).expect("encode descriptor as owner");
        assert!(decode_suspension_call_plan(&mut Reader::new(&writer.finish())).is_err());

        changed = plan;
        changed.live_values[0].storage = TerminalSuspensionStorage::CallArgument;
        changed.loans.occurrences.get_mut(child).capture =
            TerminalSuspensionLoanCapture::RetainedValue {
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
            TerminalSuspensionLoanCapture::RetainedValue {
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
            TerminalSuspensionLoanCapture::RetainedValue {
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
