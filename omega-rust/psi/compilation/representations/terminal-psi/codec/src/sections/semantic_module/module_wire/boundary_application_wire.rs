//! Closed source-free requirements; selected providers never enter this section.

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::OperationId;

    #[test]
    fn boundary_demand_wire_round_trip_and_truncation() {
        let applications = [
            BoundaryApplication::Empty,
            BoundaryApplication::exact(vec![
                BoundaryApplicationArgument::type_argument(
                    0,
                    BoundaryTypeIdentity::new("owner::Element".into()).unwrap(),
                ),
                BoundaryApplicationArgument::const_argument(
                    1,
                    BoundaryTypeIdentity::new("u64".into()).unwrap(),
                    "u64".into(),
                    "integer(u64,4)".into(),
                )
                .unwrap(),
            ])
            .unwrap(),
        ];
        for application in applications {
            let demand = TerminalBoundaryApplicationDemand::new(
                OperationId::new(1).unwrap(),
                BoundaryOperatorRequirement::new(
                    BoundaryNominalIdentity::new("package::requirement".into()).unwrap(),
                    "operator::compare(Element,Element)->bool".into(),
                )
                .unwrap(),
                application,
            );
            let mut writer = Writer::default();
            encode_demand(&mut writer, &demand).unwrap();
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            assert_eq!(decode_demand(&mut reader).unwrap(), demand);
            assert_eq!(reader.remaining(), 0);
            for end in 0..bytes.len() {
                assert!(decode_demand(&mut Reader::new(&bytes[..end])).is_err());
            }
        }
    }

    #[test]
    fn boundary_demand_wire_rejects_noncanonical_binder_order() {
        let demand = TerminalBoundaryApplicationDemand::new(
            OperationId::new(1).unwrap(),
            BoundaryOperatorRequirement::new(
                BoundaryNominalIdentity::new("package::requirement".into()).unwrap(),
                "operator::compare".into(),
            )
            .unwrap(),
            BoundaryApplication::Exact(vec![BoundaryApplicationArgument::type_argument(
                1,
                BoundaryTypeIdentity::new("u64".into()).unwrap(),
            )]),
        );
        let mut writer = Writer::default();
        encode_demand(&mut writer, &demand).unwrap();
        assert!(decode_demand(&mut Reader::new(&writer.finish())).is_err());
    }
}

use super::super::CodecError;
use super::super::wire::{Reader, Writer, decode_counted};
use terminal_psi::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryNominalIdentity,
    BoundaryOperatorRequirement, BoundaryTypeIdentity, TerminalBoundaryApplicationDemand,
};

pub(super) fn encode_demand(
    writer: &mut Writer,
    demand: &TerminalBoundaryApplicationDemand,
) -> Result<(), CodecError> {
    writer.id(demand.terminal_operation());
    writer.string(
        "boundary requirement declaration",
        demand.requirement().declaration().canonical(),
    )?;
    writer.string(
        "boundary requirement overload",
        demand.requirement().overload(),
    )?;
    writer.len(
        "boundary application arguments",
        demand.application().arguments().len(),
    )?;
    for argument in demand.application().arguments() {
        match argument {
            BoundaryApplicationArgument::Type {
                binder_ordinal,
                type_identity,
            } => {
                writer.u8(1);
                writer.u32(*binder_ordinal);
                writer.string("boundary type argument", type_identity.canonical())?;
            }
            BoundaryApplicationArgument::Const {
                binder_ordinal,
                declared_carrier,
                value_type,
                value_encoding,
            } => {
                writer.u8(2);
                writer.u32(*binder_ordinal);
                writer.string("boundary const carrier", declared_carrier.canonical())?;
                writer.string("boundary const value type", value_type)?;
                writer.string("boundary const value", value_encoding)?;
            }
        }
    }
    Ok(())
}

pub(super) fn decode_demand(
    reader: &mut Reader<'_>,
) -> Result<TerminalBoundaryApplicationDemand, CodecError> {
    let operation = reader.id("OperationId")?;
    let declaration =
        BoundaryNominalIdentity::new(reader.string("boundary requirement declaration")?)
            .map_err(CodecError::MalformedStructuralFoundation)?;
    let requirement = BoundaryOperatorRequirement::new(
        declaration,
        reader.string("boundary requirement overload")?,
    )
    .map_err(CodecError::MalformedStructuralFoundation)?;
    let arguments = decode_counted(reader, |reader| {
        let tag = reader.u8()?;
        let ordinal = reader.u32()?;
        match tag {
            1 => Ok(BoundaryApplicationArgument::type_argument(
                ordinal,
                BoundaryTypeIdentity::new(reader.string("boundary type argument")?)
                    .map_err(CodecError::MalformedStructuralFoundation)?,
            )),
            2 => BoundaryApplicationArgument::const_argument(
                ordinal,
                BoundaryTypeIdentity::new(reader.string("boundary const carrier")?)
                    .map_err(CodecError::MalformedStructuralFoundation)?,
                reader.string("boundary const value type")?,
                reader.string("boundary const value")?,
            )
            .map_err(CodecError::MalformedStructuralFoundation),
            tag => Err(CodecError::InvalidTag("BoundaryApplicationArgument", tag)),
        }
    })?;
    let application = if arguments.is_empty() {
        BoundaryApplication::Empty
    } else {
        BoundaryApplication::exact(arguments).map_err(CodecError::MalformedStructuralFoundation)?
    };
    Ok(TerminalBoundaryApplicationDemand::new(
        operation,
        requirement,
        application,
    ))
}
