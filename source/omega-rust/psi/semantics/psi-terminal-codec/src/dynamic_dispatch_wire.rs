//! Canonical wire rows for direct local dynamic dispatch custody.

use psi_terminal::{TerminalDirectDynamicDispatch, TerminalDynamicConformanceSelection};

use super::wire::{Reader, Writer};
use super::{CodecError, decode_counted, decode_structural_arguments, encode_structural_arguments};

pub(super) fn encode_dynamic_conformance_selections(
    writer: &mut Writer,
    selections: &[TerminalDynamicConformanceSelection],
) -> Result<(), CodecError> {
    writer.len("dynamic conformance selections", selections.len())?;
    for selection in selections {
        writer.id(selection.owner);
        writer.u32(selection.ordinal);
        encode_structural_arguments(writer, std::slice::from_ref(&selection.source))?;
        writer.u64(selection.conformance_application_report_fingerprint);
        writer.bytes(&selection.conformance_application_commitment.as_bytes());
    }
    Ok(())
}

pub(super) fn decode_dynamic_conformance_selections(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalDynamicConformanceSelection>, CodecError> {
    decode_counted(reader, |reader| {
        let owner = reader.id("MachineId")?;
        let ordinal = reader.u32()?;
        let mut sources = decode_structural_arguments(reader)?;
        if sources.len() != 1 {
            return Err(CodecError::MalformedStructuralFoundation(
                "dynamic conformance selection must encode exactly one source",
            ));
        }
        Ok(TerminalDynamicConformanceSelection {
            owner,
            ordinal,
            source: sources.remove(0),
            conformance_application_report_fingerprint: reader.u64()?,
            conformance_application_commitment:
                psi_terminal::ClosedConformanceApplicationCommitment::from_digest(reader.array()?),
        })
    })
}

pub(super) fn encode_direct_dynamic_dispatches(
    writer: &mut Writer,
    dispatches: &[TerminalDirectDynamicDispatch],
) -> Result<(), CodecError> {
    writer.len("direct dynamic dispatches", dispatches.len())?;
    for dispatch in dispatches {
        writer.id(dispatch.owner);
        writer.id(dispatch.operation);
        writer.u32(dispatch.selection_ordinal);
        writer.string(
            "direct dynamic dispatch declaring trait identity",
            &dispatch.declaring_trait_identity,
        )?;
        writer.string(
            "direct dynamic dispatch public requirement identity",
            &dispatch.public_requirement_identity,
        )?;
        writer.string(
            "direct dynamic dispatch requirement identity",
            &dispatch.requirement_identity,
        )?;
        writer.string(
            "direct dynamic dispatch realization identity",
            &dispatch.realization_identity,
        )?;
        writer.string(
            "direct dynamic dispatch realization callable identity",
            &dispatch.realization_callable_identity,
        )?;
        writer.id(dispatch.realization);
    }
    Ok(())
}

pub(super) fn decode_direct_dynamic_dispatches(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalDirectDynamicDispatch>, CodecError> {
    decode_counted(reader, |reader| {
        Ok(TerminalDirectDynamicDispatch {
            owner: reader.id("MachineId")?,
            operation: reader.id("OperationId")?,
            selection_ordinal: reader.u32()?,
            declaring_trait_identity: reader
                .string("direct dynamic dispatch declaring trait identity")?,
            public_requirement_identity: reader
                .string("direct dynamic dispatch public requirement identity")?,
            requirement_identity: reader.string("direct dynamic dispatch requirement identity")?,
            realization_identity: reader.string("direct dynamic dispatch realization identity")?,
            realization_callable_identity: reader
                .string("direct dynamic dispatch realization callable identity")?,
            realization: reader.id("MachineId")?,
        })
    })
}

#[cfg(test)]
mod tests {
    use psi_core::{MachineId, OperationId, PlaceId, PsiSemanticId};
    use psi_terminal::{
        ClosedConformanceApplicationCommitment, StructuralAccess, StructuralArgument,
        TerminalDirectDynamicDispatch, TerminalDynamicConformanceSelection,
    };

    use super::*;

    fn id<Identity: PsiSemanticId>(raw: u64) -> Identity {
        Identity::new(raw).expect("test identity is nonzero")
    }

    #[test]
    fn direct_dynamic_catalog_rows_round_trip_exactly() {
        let selections = vec![TerminalDynamicConformanceSelection {
            owner: id::<MachineId>(1),
            ordinal: 0,
            source: StructuralArgument {
                place: id::<PlaceId>(1),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            },
            conformance_application_report_fingerprint: 41,
            conformance_application_commitment: ClosedConformanceApplicationCommitment::from_digest(
                [0x5a; 32],
            ),
        }];
        let dispatches = vec![TerminalDirectDynamicDispatch {
            owner: id::<MachineId>(1),
            operation: id::<OperationId>(1),
            selection_ordinal: 0,
            declaring_trait_identity: "package::Measure".into(),
            public_requirement_identity: "package::Measure::measure()".into(),
            requirement_identity: "package::Measure::measure".into(),
            realization_identity: "package::Carrier::measure".into(),
            realization_callable_identity: "package::Carrier::measure#callable".into(),
            realization: id::<MachineId>(2),
        }];
        let mut writer = Writer::default();
        encode_dynamic_conformance_selections(&mut writer, &selections).unwrap();
        encode_direct_dynamic_dispatches(&mut writer, &dispatches).unwrap();
        let bytes = writer.finish();

        let mut reader = Reader::new(&bytes);
        assert_eq!(
            decode_dynamic_conformance_selections(&mut reader),
            Ok(selections)
        );
        assert_eq!(
            decode_direct_dynamic_dispatches(&mut reader),
            Ok(dispatches)
        );
        assert_eq!(reader.remaining(), 0);
    }
}
