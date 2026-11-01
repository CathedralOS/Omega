//! Canonical native-layout provenance; compiler-private record encoding is not
//! a portable wire format. Arena coordinates are reconstructed on ingress.

use super::super::wire_fields::{
    Field, Id, Index, List, Optional, Text, U64, wire_enum, wire_record,
};
use super::super::{
    CodecError,
    wire::{Reader, Writer, decode_bytes},
};
use language_semantics::callback_declarations::PrivateCallbackSlotSelection;
use layout_plans::{
    IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
    NativeLayoutPlanReport, PrivateCallbackLayoutDemandReport,
};
use terminal_psi::{NativeLayoutCatalog, NativeLayoutFieldIdentity, NativeLayoutRealization};
type NativeReport = NativeLayoutPlanReport<PrivateCallbackSlotSelection>;
type PrivateDemand = PrivateCallbackLayoutDemandReport<PrivateCallbackSlotSelection>;

pub(super) struct Catalog;
impl Field<NativeLayoutCatalog> for Catalog {
    fn write(writer: &mut Writer, value: &NativeLayoutCatalog) -> Result<(), CodecError> {
        writer.len("native layout records", value.records().count())?;
        for record in value.records() {
            Id::write(writer, &record.record)?;
            Index::write(writer, &record.data_identity)?;
            Index::write(writer, &record.policy_identity)?;
            Index::write(writer, &record.semantic_identity)?;
            let fields = value.fields(record).map_err(CodecError::NativeLayout)?;
            writer.len("native nominal fields", fields.len())?;
            for field in fields {
                NominalField::write(writer, field)?;
            }
            let realizations = value
                .realizations(record)
                .map_err(CodecError::NativeLayout)?;
            writer.len("native layout realizations", realizations.len())?;
            for realization in realizations {
                Realization::write(writer, realization)?;
            }
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<NativeLayoutCatalog, CodecError> {
        let mut catalog = NativeLayoutCatalog::default();
        for _ in 0..reader.count()? {
            let record = Id::read(reader)?;
            let data = Index::read(reader)?;
            let policy = Index::read(reader)?;
            let semantic = Index::read(reader)?;
            let fields = List::<NominalField>::read(reader)?;
            let realizations = List::<Realization>::read(reader)?;
            catalog.insert(record, data, policy, semantic, fields, realizations);
        }
        Ok(catalog)
    }
}

wire_record! {
    NominalField: NativeLayoutFieldIdentity { field: Id, declaration_identity: Index }
    Realization: NativeLayoutRealization {
        target: Optional<super::target_layout_wire::Input>,
        report: Report,
    }
    Report: NativeReport {
        layout: Layout,
        private_callback_demands: List<Demand>,
    }
    Layout: LayoutPlanReport {
        schema_report_fingerprint: U64,
        entries: List<Entry>,
        offsets: Optional<List<U64>>,
        size: Optional<U64>,
        align: U64,
    }
    Entry: LayoutFieldEntryReport {
        field: Text,
        member_identity: Optional<U64>,
        placement: Placement,
    }
    Demand: PrivateDemand {
        slot_application: Selection,
        slot_identity: Text,
        layout_subject_identity: Text,
        callback_requirement_identity: Text,
        offset: U64,
    }
    Selection: PrivateCallbackSlotSelection {
        declaration_identity: Text,
        trait_identity: Text,
        semantic_identity: Text,
        layout_identity: Text,
        requirement_identity: Text,
        application_report_fingerprint: U64,
        application_commitment: Digest,
        application_encoding: Preimage,
    }
}

wire_enum! {
    Interpretation: IntegerInterpretation, "native integer interpretation" {
        0 => Signed,
        1 => Unsigned,
    }
    Placement: LayoutPlacementReport, "native layout placement" {
        0 => At { offset: U64 },
        1 => IntegerAt { offset: U64, stored_width: U64, interpretation: Interpretation },
        2 => Bits { container: U64, container_width: U64, destination_lsb: U64, source_lsb: U64, width: U64 },
    }
}

struct Digest;
impl Field<[u8; 32]> for Digest {
    fn write(writer: &mut Writer, value: &[u8; 32]) -> Result<(), CodecError> {
        writer.bytes(value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<[u8; 32], CodecError> {
        reader.array()
    }
}
struct Preimage;
impl Field<std::sync::Arc<[u8]>> for Preimage {
    fn write(writer: &mut Writer, value: &std::sync::Arc<[u8]>) -> Result<(), CodecError> {
        writer.len("Source conformance identity preimage", value.len())?;
        writer.bytes(value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<std::sync::Arc<[u8]>, CodecError> {
        Ok(decode_bytes(reader)?.into())
    }
}
