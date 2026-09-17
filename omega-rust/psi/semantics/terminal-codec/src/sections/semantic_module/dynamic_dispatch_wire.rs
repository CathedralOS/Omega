//! Canonical wire rows for direct local dynamic dispatch custody.

use terminal_psi::{
    ClosedConformanceCallableResult, TerminalDirectDynamicDispatch,
    TerminalDynamicConformanceSelection, TerminalDynamicDescriptorArgument,
    TerminalDynamicDescriptorParameter, TerminalDynamicDescriptorSource,
    TerminalDynamicRequirement, TerminalIndirectDynamicDispatch, TerminalParameterDynamicDispatch,
    TerminalReboundDynamicDescriptor, TerminalStoredDynamicDescriptor,
    TerminalStoredDynamicDispatch,
};

use super::CodecError;
use super::wire::{Reader, Writer};
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_arguments, encode_structural_arguments,
};
use crate::sections::semantic_module::structural_signature_wire::{
    decode_structural_access, encode_structural_access,
};
use crate::sections::semantic_module::wire::decode_counted;

pub(crate) fn encode_dynamic_descriptor_parameters(
    writer: &mut Writer,
    parameters: &[TerminalDynamicDescriptorParameter],
) -> Result<(), CodecError> {
    writer.len("dynamic descriptor parameters", parameters.len())?;
    for parameter in parameters {
        writer.id(parameter.owner);
        writer.u32(parameter.ordinal);
        writer.u32(parameter.source_position);
        writer.string(
            "dynamic descriptor parameter trait identity",
            &parameter.trait_identity,
        )?;
        encode_structural_access(writer, parameter.access);
        writer.len(
            "dynamic descriptor requirements",
            parameter.requirements.len(),
        )?;
        for requirement in &parameter.requirements {
            writer.u32(requirement.slot);
            writer.string(
                "dynamic descriptor requirement declaring trait identity",
                &requirement.declaring_trait_identity,
            )?;
            writer.string(
                "dynamic descriptor public requirement identity",
                &requirement.public_requirement_identity,
            )?;
            writer.strings(
                "dynamic descriptor requirement family tuple",
                &requirement.family_tuple,
            )?;
            writer.u8(match requirement.result {
                ClosedConformanceCallableResult::Unit => 1,
                ClosedConformanceCallableResult::I32 => 2,
                ClosedConformanceCallableResult::Bool => 3,
            });
        }
    }
    Ok(())
}

pub(crate) fn decode_dynamic_descriptor_parameters(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalDynamicDescriptorParameter>, CodecError> {
    decode_counted(reader, |reader| {
        Ok(TerminalDynamicDescriptorParameter {
            owner: reader.id("MachineId")?,
            ordinal: reader.u32()?,
            source_position: reader.u32()?,
            trait_identity: reader.string("dynamic descriptor parameter trait identity")?,
            access: decode_structural_access(reader)?,
            requirements: decode_counted(reader, |reader| {
                Ok(TerminalDynamicRequirement {
                    slot: reader.u32()?,
                    declaring_trait_identity: reader
                        .string("dynamic descriptor requirement declaring trait identity")?,
                    public_requirement_identity: reader
                        .string("dynamic descriptor public requirement identity")?,
                    family_tuple: reader.strings("dynamic descriptor requirement family tuple")?,
                    result: match reader.u8()? {
                        1 => ClosedConformanceCallableResult::Unit,
                        2 => ClosedConformanceCallableResult::I32,
                        3 => ClosedConformanceCallableResult::Bool,
                        tag => {
                            return Err(CodecError::InvalidTag(
                                "ClosedConformanceCallableResult",
                                tag,
                            ));
                        }
                    },
                })
            })?,
        })
    })
}

pub(crate) fn encode_dynamic_descriptor_arguments(
    writer: &mut Writer,
    arguments: &[TerminalDynamicDescriptorArgument],
) -> Result<(), CodecError> {
    writer.len("dynamic descriptor arguments", arguments.len())?;
    for argument in arguments {
        writer.id(argument.owner);
        writer.id(argument.operation);
        writer.u32(argument.parameter_ordinal);
        match argument.source {
            TerminalDynamicDescriptorSource::Selection { ordinal } => {
                writer.u8(3);
                writer.u32(ordinal);
            }
            TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal } => {
                writer.u8(1);
                writer.u32(ordinal);
            }
            TerminalDynamicDescriptorSource::Parameter { ordinal } => {
                writer.u8(2);
                writer.u32(ordinal);
            }
        }
    }
    Ok(())
}

pub(crate) fn decode_dynamic_descriptor_arguments(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalDynamicDescriptorArgument>, CodecError> {
    decode_counted(reader, |reader| {
        let owner = reader.id("MachineId")?;
        let operation = reader.id("OperationId")?;
        let parameter_ordinal = reader.u32()?;
        let source = match reader.u8()? {
            1 => TerminalDynamicDescriptorSource::ReboundDescriptor {
                ordinal: reader.u32()?,
            },
            2 => TerminalDynamicDescriptorSource::Parameter {
                ordinal: reader.u32()?,
            },
            3 => TerminalDynamicDescriptorSource::Selection {
                ordinal: reader.u32()?,
            },
            tag => {
                return Err(CodecError::InvalidTag(
                    "TerminalDynamicDescriptorSource",
                    tag,
                ));
            }
        };
        Ok(TerminalDynamicDescriptorArgument {
            owner,
            operation,
            parameter_ordinal,
            source,
        })
    })
}

pub(crate) fn encode_dynamic_conformance_selections(
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

pub(crate) fn decode_dynamic_conformance_selections(
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
                terminal_psi::ClosedConformanceApplicationCommitment::from_digest(reader.array()?),
        })
    })
}

pub(crate) fn encode_direct_dynamic_dispatches(
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
        writer.strings(
            "direct dynamic dispatch family tuple",
            &dispatch.family_tuple,
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

pub(crate) fn decode_direct_dynamic_dispatches(
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
            family_tuple: reader.strings("direct dynamic dispatch family tuple")?,
            requirement_identity: reader.string("direct dynamic dispatch requirement identity")?,
            realization_identity: reader.string("direct dynamic dispatch realization identity")?,
            realization_callable_identity: reader
                .string("direct dynamic dispatch realization callable identity")?,
            realization: reader.id("MachineId")?,
        })
    })
}

pub(crate) fn encode_rebound_dynamic_descriptors(
    writer: &mut Writer,
    descriptors: &[TerminalReboundDynamicDescriptor],
) -> Result<(), CodecError> {
    writer.len("rebound dynamic descriptors", descriptors.len())?;
    for descriptor in descriptors {
        writer.id(descriptor.owner);
        writer.u32(descriptor.ordinal);
        writer.u32(descriptor.initial_selection_ordinal);
        writer.u32(descriptor.rebound_selection_ordinal);
    }
    Ok(())
}

pub(crate) fn decode_rebound_dynamic_descriptors(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalReboundDynamicDescriptor>, CodecError> {
    decode_counted(reader, |reader| {
        Ok(TerminalReboundDynamicDescriptor {
            owner: reader.id("MachineId")?,
            ordinal: reader.u32()?,
            initial_selection_ordinal: reader.u32()?,
            rebound_selection_ordinal: reader.u32()?,
        })
    })
}

pub(crate) fn encode_stored_dynamic_descriptors(
    writer: &mut Writer,
    descriptors: &[TerminalStoredDynamicDescriptor],
) -> Result<(), CodecError> {
    writer.len("stored dynamic descriptors", descriptors.len())?;
    for descriptor in descriptors {
        writer.id(descriptor.owner);
        writer.u32(descriptor.ordinal);
        writer.id(descriptor.establishment_operation);
        writer.u32(descriptor.selection_ordinal);
        writer.string(
            "stored dynamic descriptor aggregate type identity",
            &descriptor.aggregate_type_identity,
        )?;
        writer.string(
            "stored dynamic descriptor field identity",
            &descriptor.field_identity,
        )?;
    }
    Ok(())
}

pub(crate) fn decode_stored_dynamic_descriptors(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalStoredDynamicDescriptor>, CodecError> {
    decode_counted(reader, |reader| {
        Ok(TerminalStoredDynamicDescriptor {
            owner: reader.id("MachineId")?,
            ordinal: reader.u32()?,
            establishment_operation: reader.id("OperationId")?,
            selection_ordinal: reader.u32()?,
            aggregate_type_identity: reader
                .string("stored dynamic descriptor aggregate type identity")?,
            field_identity: reader.string("stored dynamic descriptor field identity")?,
        })
    })
}

pub(crate) fn encode_indirect_dynamic_dispatches(
    writer: &mut Writer,
    dispatches: &[TerminalIndirectDynamicDispatch],
) -> Result<(), CodecError> {
    writer.len("indirect dynamic dispatches", dispatches.len())?;
    for dispatch in dispatches {
        writer.id(dispatch.owner);
        writer.id(dispatch.operation);
        writer.u32(dispatch.descriptor_ordinal);
        writer.string(
            "indirect dynamic dispatch declaring trait identity",
            &dispatch.declaring_trait_identity,
        )?;
        writer.string(
            "indirect dynamic dispatch public requirement identity",
            &dispatch.public_requirement_identity,
        )?;
        writer.strings(
            "indirect dynamic dispatch family tuple",
            &dispatch.family_tuple,
        )?;
        writer.string(
            "indirect dynamic dispatch requirement identity",
            &dispatch.requirement_identity,
        )?;
        writer.string(
            "indirect dynamic dispatch realization identity",
            &dispatch.realization_identity,
        )?;
        writer.string(
            "indirect dynamic dispatch realization callable identity",
            &dispatch.realization_callable_identity,
        )?;
        writer.id(dispatch.realization);
    }
    Ok(())
}

pub(crate) fn decode_indirect_dynamic_dispatches(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalIndirectDynamicDispatch>, CodecError> {
    decode_counted(reader, |reader| {
        Ok(TerminalIndirectDynamicDispatch {
            owner: reader.id("MachineId")?,
            operation: reader.id("OperationId")?,
            descriptor_ordinal: reader.u32()?,
            declaring_trait_identity: reader
                .string("indirect dynamic dispatch declaring trait identity")?,
            public_requirement_identity: reader
                .string("indirect dynamic dispatch public requirement identity")?,
            family_tuple: reader.strings("indirect dynamic dispatch family tuple")?,
            requirement_identity: reader
                .string("indirect dynamic dispatch requirement identity")?,
            realization_identity: reader
                .string("indirect dynamic dispatch realization identity")?,
            realization_callable_identity: reader
                .string("indirect dynamic dispatch realization callable identity")?,
            realization: reader.id("MachineId")?,
        })
    })
}

pub(crate) fn encode_stored_dynamic_dispatches(
    writer: &mut Writer,
    dispatches: &[TerminalStoredDynamicDispatch],
) -> Result<(), CodecError> {
    writer.len("stored dynamic dispatches", dispatches.len())?;
    for dispatch in dispatches {
        writer.id(dispatch.owner);
        writer.id(dispatch.operation);
        writer.u32(dispatch.descriptor_ordinal);
        writer.string(
            "stored dynamic dispatch declaring trait identity",
            &dispatch.declaring_trait_identity,
        )?;
        writer.string(
            "stored dynamic dispatch public requirement identity",
            &dispatch.public_requirement_identity,
        )?;
        writer.strings(
            "stored dynamic dispatch family tuple",
            &dispatch.family_tuple,
        )?;
        writer.string(
            "stored dynamic dispatch requirement identity",
            &dispatch.requirement_identity,
        )?;
        writer.string(
            "stored dynamic dispatch realization identity",
            &dispatch.realization_identity,
        )?;
        writer.string(
            "stored dynamic dispatch realization callable identity",
            &dispatch.realization_callable_identity,
        )?;
        writer.id(dispatch.realization);
    }
    Ok(())
}

pub(crate) fn decode_stored_dynamic_dispatches(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalStoredDynamicDispatch>, CodecError> {
    decode_counted(reader, |reader| {
        Ok(TerminalStoredDynamicDispatch {
            owner: reader.id("MachineId")?,
            operation: reader.id("OperationId")?,
            descriptor_ordinal: reader.u32()?,
            declaring_trait_identity: reader
                .string("stored dynamic dispatch declaring trait identity")?,
            public_requirement_identity: reader
                .string("stored dynamic dispatch public requirement identity")?,
            family_tuple: reader.strings("stored dynamic dispatch family tuple")?,
            requirement_identity: reader.string("stored dynamic dispatch requirement identity")?,
            realization_identity: reader.string("stored dynamic dispatch realization identity")?,
            realization_callable_identity: reader
                .string("stored dynamic dispatch realization callable identity")?,
            realization: reader.id("MachineId")?,
        })
    })
}

pub(crate) fn encode_parameter_dynamic_dispatches(
    writer: &mut Writer,
    dispatches: &[TerminalParameterDynamicDispatch],
) -> Result<(), CodecError> {
    writer.len("parameter dynamic dispatches", dispatches.len())?;
    for dispatch in dispatches {
        writer.id(dispatch.owner);
        writer.id(dispatch.operation);
        writer.u32(dispatch.parameter_ordinal);
        writer.u32(dispatch.requirement_slot);
    }
    Ok(())
}

pub(crate) fn decode_parameter_dynamic_dispatches(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalParameterDynamicDispatch>, CodecError> {
    decode_counted(reader, |reader| {
        Ok(TerminalParameterDynamicDispatch {
            owner: reader.id("MachineId")?,
            operation: reader.id("OperationId")?,
            parameter_ordinal: reader.u32()?,
            requirement_slot: reader.u32()?,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Reader, Writer, decode_direct_dynamic_dispatches, decode_dynamic_conformance_selections,
        decode_dynamic_descriptor_parameters, decode_indirect_dynamic_dispatches,
        decode_rebound_dynamic_descriptors, decode_stored_dynamic_dispatches,
        encode_direct_dynamic_dispatches, encode_dynamic_conformance_selections,
        encode_dynamic_descriptor_parameters, encode_indirect_dynamic_dispatches,
        encode_rebound_dynamic_descriptors, encode_stored_dynamic_dispatches,
    };
    use semantic_vocabulary::{MachineId, OperationId, PlaceId, PsiSemanticId};
    use terminal_psi::{
        ClosedConformanceApplicationCommitment, ClosedConformanceCallableResult, StructuralAccess,
        StructuralArgument, TerminalDirectDynamicDispatch, TerminalDynamicConformanceSelection,
        TerminalDynamicDescriptorParameter, TerminalDynamicRequirement,
        TerminalIndirectDynamicDispatch, TerminalReboundDynamicDescriptor,
        TerminalStoredDynamicDispatch,
    };

    /// The canonical tuple a `<Width: u32, Lanes: u32>` family row retains:
    /// const identities in binder declaration order, never display spellings.
    fn width_lanes_tuple() -> Vec<String> {
        vec![
            "named(integer-const(16))".to_owned(),
            "named(integer-const(4))".to_owned(),
        ]
    }

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
            family_tuple: width_lanes_tuple(),
            requirement_identity: "package::Measure::measure".into(),
            realization_identity: "package::Carrier::measure".into(),
            realization_callable_identity: "package::Carrier::measure#callable".into(),
            realization: id::<MachineId>(2),
        }];
        let descriptors = vec![TerminalReboundDynamicDescriptor {
            owner: id::<MachineId>(1),
            ordinal: 0,
            initial_selection_ordinal: 0,
            rebound_selection_ordinal: 1,
        }];
        let indirect_dispatches = vec![TerminalIndirectDynamicDispatch {
            owner: id::<MachineId>(1),
            operation: id::<OperationId>(2),
            descriptor_ordinal: 0,
            declaring_trait_identity: "package::Measure".into(),
            public_requirement_identity: "package::Measure::measure()".into(),
            family_tuple: Vec::new(),
            requirement_identity: "package::Measure::measure".into(),
            realization_identity: "package::Carrier::measure".into(),
            realization_callable_identity: "package::Carrier::measure#callable".into(),
            realization: id::<MachineId>(2),
        }];
        let stored_dispatches = vec![TerminalStoredDynamicDispatch {
            owner: id::<MachineId>(1),
            operation: id::<OperationId>(3),
            descriptor_ordinal: 0,
            declaring_trait_identity: "package::Measure".into(),
            public_requirement_identity: "package::Measure::measure()".into(),
            family_tuple: width_lanes_tuple(),
            requirement_identity: "package::Measure::measure".into(),
            realization_identity: "package::Carrier::measure".into(),
            realization_callable_identity: "package::Carrier::measure#callable".into(),
            realization: id::<MachineId>(2),
        }];
        let parameters = vec![TerminalDynamicDescriptorParameter {
            owner: id::<MachineId>(3),
            ordinal: 0,
            source_position: 0,
            trait_identity: "package::Measure".into(),
            access: StructuralAccess::SharedBorrow,
            requirements: vec![
                TerminalDynamicRequirement {
                    slot: 0,
                    declaring_trait_identity: "package::Measure".into(),
                    public_requirement_identity: "package::Measure::measure()".into(),
                    family_tuple: Vec::new(),
                    result: ClosedConformanceCallableResult::I32,
                },
                TerminalDynamicRequirement {
                    slot: 1,
                    declaring_trait_identity: "package::Measure".into(),
                    public_requirement_identity: "package::Measure::scan()".into(),
                    family_tuple: width_lanes_tuple(),
                    result: ClosedConformanceCallableResult::Bool,
                },
            ],
        }];
        let mut writer = Writer::default();
        encode_dynamic_conformance_selections(&mut writer, &selections).unwrap();
        encode_rebound_dynamic_descriptors(&mut writer, &descriptors).unwrap();
        encode_direct_dynamic_dispatches(&mut writer, &dispatches).unwrap();
        encode_indirect_dynamic_dispatches(&mut writer, &indirect_dispatches).unwrap();
        encode_stored_dynamic_dispatches(&mut writer, &stored_dispatches).unwrap();
        encode_dynamic_descriptor_parameters(&mut writer, &parameters).unwrap();
        let bytes = writer.finish();

        let mut reader = Reader::new(&bytes);
        assert_eq!(
            decode_dynamic_conformance_selections(&mut reader),
            Ok(selections)
        );
        assert_eq!(
            decode_rebound_dynamic_descriptors(&mut reader),
            Ok(descriptors)
        );
        assert_eq!(
            decode_direct_dynamic_dispatches(&mut reader),
            Ok(dispatches)
        );
        assert_eq!(
            decode_indirect_dynamic_dispatches(&mut reader),
            Ok(indirect_dispatches)
        );
        assert_eq!(
            decode_stored_dynamic_dispatches(&mut reader),
            Ok(stored_dispatches)
        );
        assert_eq!(
            decode_dynamic_descriptor_parameters(&mut reader),
            Ok(parameters)
        );
        assert_eq!(reader.remaining(), 0);
    }

    fn direct_dispatch(family_tuple: Vec<String>) -> TerminalDirectDynamicDispatch {
        TerminalDirectDynamicDispatch {
            owner: id::<MachineId>(1),
            operation: id::<OperationId>(1),
            selection_ordinal: 0,
            declaring_trait_identity: "package::Measure".into(),
            public_requirement_identity: "package::Measure::measure()".into(),
            family_tuple,
            requirement_identity: "package::Measure::measure".into(),
            realization_identity: "package::Carrier::measure".into(),
            realization_callable_identity: "package::Carrier::measure#callable".into(),
            realization: id::<MachineId>(2),
        }
    }

    /// The tuple is a row coordinate: two rows equal in every identity but
    /// the tuple encode differently, and tuple order is not normalized away.
    #[test]
    fn family_tuple_participates_in_dispatch_row_bytes() {
        let encode = |dispatch: TerminalDirectDynamicDispatch| {
            let mut writer = Writer::default();
            encode_direct_dynamic_dispatches(&mut writer, &[dispatch]).unwrap();
            writer.finish()
        };
        let nongeneric = encode(direct_dispatch(Vec::new()));
        let width_lanes = encode(direct_dispatch(width_lanes_tuple()));
        let mut reversed = width_lanes_tuple();
        reversed.reverse();
        let lanes_width = encode(direct_dispatch(reversed));
        assert_ne!(nongeneric, width_lanes);
        assert_ne!(width_lanes, lanes_width);
        assert_eq!(
            decode_direct_dynamic_dispatches(&mut Reader::new(&width_lanes)),
            Ok(vec![direct_dispatch(width_lanes_tuple())])
        );
    }

    /// Rows encoded before the tuple coordinate existed must reject rather
    /// than decode as nongeneric rows: the decoder never supplies a missing
    /// tuple. These bytes are the previous row layout written field by field.
    #[test]
    fn dispatch_rows_without_family_tuple_reject() {
        let dispatch = direct_dispatch(Vec::new());
        let mut writer = Writer::default();
        writer.len("legacy direct dynamic dispatches", 1).unwrap();
        writer.id(dispatch.owner);
        writer.id(dispatch.operation);
        writer.u32(dispatch.selection_ordinal);
        writer
            .string("legacy", &dispatch.declaring_trait_identity)
            .unwrap();
        writer
            .string("legacy", &dispatch.public_requirement_identity)
            .unwrap();
        writer
            .string("legacy", &dispatch.requirement_identity)
            .unwrap();
        writer
            .string("legacy", &dispatch.realization_identity)
            .unwrap();
        writer
            .string("legacy", &dispatch.realization_callable_identity)
            .unwrap();
        writer.id(dispatch.realization);
        let legacy = writer.finish();

        let mut reader = Reader::new(&legacy);
        let decoded = decode_direct_dynamic_dispatches(&mut reader);
        assert!(
            decoded.is_err() || reader.remaining() != 0 || decoded != Ok(vec![dispatch]),
            "previous-layout row decoded cleanly as a current row: {decoded:?}"
        );

        let mut current = Writer::default();
        encode_direct_dynamic_dispatches(&mut current, &[direct_dispatch(Vec::new())]).unwrap();
        assert_ne!(current.finish(), legacy);
    }
}
