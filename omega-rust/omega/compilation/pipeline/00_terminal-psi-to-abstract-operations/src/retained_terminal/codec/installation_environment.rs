//! Reviewable caller assumptions; decoding creates inputs, never admission.
use super::value::{RecordError, RecordLimits, RecordReader, RecordValue, RecordWriter};
use installation_evidence::{
    BootstrapAssumptionInterval, BootstrapEnvironmentAssumption, InstallationEnvironmentAssumption,
    InterruptDeliveryInterval, X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
};

super::records::record_fields!(BootstrapEnvironmentAssumption {
    psi,
    profile,
    entry,
    installation,
    interval,
    environment_description_sha256,
    source_provenance,
    caller_provenance
});

impl RecordValue for BootstrapAssumptionInterval {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        match self {
            Self::EntryUntilInstallationPublished => 0u8.write(output),
        }
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        match u8::read(input)? {
            0 => Ok(Self::EntryUntilInstallationPublished),
            _ => Err(input.error("unknown bootstrap assumption interval")),
        }
    }
}

super::records::record_fields!(X86InterruptDeliveryAssumption {
    psi,
    profile,
    entry,
    installation,
    interval,
    delivery,
    environment_description_sha256,
    source_provenance,
    caller_provenance
});

impl RecordValue for InterruptDeliveryInterval {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        match self {
            Self::InstalledScopeLifetime => 0u8.write(output),
        }
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        match u8::read(input)? {
            0 => Ok(Self::InstalledScopeLifetime),
            _ => Err(input.error("unknown interrupt delivery interval")),
        }
    }
}
impl RecordValue for X86InterruptDeliveryProfile {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        match self {
            Self::ArchitecturalExceptionsAndExternalIrqs => 0u8.write(output),
        }
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        match u8::read(input)? {
            0 => Ok(Self::ArchitecturalExceptionsAndExternalIrqs),
            _ => Err(input.error("unknown interrupt delivery profile")),
        }
    }
}
impl RecordValue for InstallationEnvironmentAssumption {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        match self {
            Self::Bootstrap(row) => {
                0u8.write(output)?;
                row.write(output)
            }
            Self::InterruptDelivery(row) => {
                1u8.write(output)?;
                row.write(output)
            }
        }
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        match u8::read(input)? {
            0 => Ok(Self::Bootstrap(BootstrapEnvironmentAssumption::read(
                input,
            )?)),
            1 => Ok(Self::InterruptDelivery(
                X86InterruptDeliveryAssumption::read(input)?,
            )),
            _ => Err(input.error("unknown installation environment condition")),
        }
    }
}

const REVIEW_SCHEMA: &[u8; 8] = b"OMGINST1";

/// Serialize explicit assumptions for normal compile requests and review.
/// This record has no constructor for a proof or an observed CPU fact.
pub fn encode_installation_assumptions(
    rows: &[InstallationEnvironmentAssumption],
) -> Result<Vec<u8>, String> {
    for row in rows {
        row.validate_description()?;
    }
    let mut output = RecordWriter::new(RecordLimits::default());
    output
        .extend(REVIEW_SCHEMA)
        .map_err(|error| error.to_string())?;
    super::value::write_values(rows, &mut output).map_err(|error| error.to_string())?;
    Ok(output.finish())
}

/// Read the same bounded records accepted by the public native product API.
/// Exact program, target, entry and installation joins happen at admission.
pub fn decode_installation_assumptions(
    bytes: &[u8],
) -> Result<Vec<InstallationEnvironmentAssumption>, String> {
    let mut input =
        RecordReader::new(bytes, RecordLimits::default()).map_err(|error| error.to_string())?;
    let schema = input
        .take(REVIEW_SCHEMA.len())
        .map_err(|error| error.to_string())?;
    if schema != REVIEW_SCHEMA {
        return Err("foreign installation assumption review schema".into());
    }
    let rows = Vec::<InstallationEnvironmentAssumption>::read(&mut input)
        .map_err(|error| error.to_string())?;
    input.finish().map_err(|error| error.to_string())?;
    for row in &rows {
        row.validate_description()?;
    }
    Ok(rows)
}
