//! The OMGINST1 review envelope and the installation rows embedded in retained native inputs.
use super::{
    BootstrapAssumptionInterval, BootstrapEnvironmentAssumption, InstallationEnvironmentAssumption,
    InterruptDeliveryInterval, X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
};
use psi_program_identity::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};
use record_storage::{Record, RecordError, RecordLimits, RecordReader, RecordWriter};
use semantic_vocabulary::{MachineId, OperationId};
use target::TargetProfile;

const SCHEMA: &[u8; 8] = b"OMGINST1";
// The native-record target tag is the index in this table, encoded as a little-endian u16.
const PROFILES: &[TargetProfile] = &[
    TargetProfile::LinuxArm64,
    TargetProfile::LinuxX64,
    TargetProfile::MacosArm64,
    TargetProfile::MacosX64,
    TargetProfile::WindowsX64,
    TargetProfile::UefiX64,
    TargetProfile::CrossPlatformCli,
    TargetProfile::LocalUnchecked,
    TargetProfile::AlphaBootstrap,
];

impl Record for InstallationEnvironmentAssumption {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.condition_tag().write(output)?;
        let psi = self.psi();
        psi.vocabulary_marker.get().write(output)?;
        output.extend(psi.program_fingerprint.as_bytes())?;
        let profile = PROFILES
            .iter()
            .position(|profile| *profile == self.profile())
            .ok_or_else(|| output.error("unknown installation target profile"))?;
        (profile as u16).write(output)?;
        self.entry().get().write(output)?;
        self.installation().get().write(output)?;
        match self {
            Self::Bootstrap(row) => match row.interval {
                BootstrapAssumptionInterval::EntryUntilInstallationPublished => {
                    0u8.write(output)?
                }
            },
            Self::InterruptDelivery(row) => {
                match row.interval {
                    InterruptDeliveryInterval::InstalledScopeLifetime => 0u8.write(output)?,
                }
                match row.delivery {
                    X86InterruptDeliveryProfile::ArchitecturalExceptionsAndExternalIrqs => {
                        0u8.write(output)?
                    }
                }
            }
        }
        output.extend(self.environment_description_sha256())?;
        for text in [self.source_provenance(), self.caller_provenance()] {
            output.sequence(text.len())?;
            output.extend(text.as_bytes())?;
        }
        Ok(())
    }

    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        let kind = u8::read(input)?;
        if kind > 1 {
            return Err(input.error("unknown installation environment condition"));
        }
        input.record(|input| {
            let vocabulary_marker = VocabularyMarker::new(u16::read(input)?)
                .ok_or_else(|| input.error("foreign Terminal vocabulary"))?;
            let psi = TerminalPsiIdentity {
                vocabulary_marker,
                program_fingerprint: SemanticFingerprint::from_bytes(read_digest(input)?),
            };
            let profile = *PROFILES
                .get(usize::from(u16::read(input)?))
                .ok_or_else(|| input.error("unknown installation target profile"))?;
            let entry = MachineId::new(u64::read(input)?)
                .ok_or_else(|| input.error("invalid machine identity"))?;
            let installation = OperationId::new(u64::read(input)?)
                .ok_or_else(|| input.error("invalid installation identity"))?;
            if u8::read(input)? != 0 {
                return Err(input.error(if kind == 0 {
                    "unknown bootstrap assumption interval"
                } else {
                    "unknown interrupt delivery interval"
                }));
            }
            if kind == 1 && u8::read(input)? != 0 {
                return Err(input.error("unknown x86 interrupt delivery profile"));
            }
            let environment_description_sha256 = read_digest(input)?;
            let source_provenance = String::read(input)?;
            let caller_provenance = String::read(input)?;
            Ok(match kind {
                0 => Self::Bootstrap(BootstrapEnvironmentAssumption {
                    psi,
                    profile,
                    entry,
                    installation,
                    interval: BootstrapAssumptionInterval::EntryUntilInstallationPublished,
                    environment_description_sha256,
                    source_provenance,
                    caller_provenance,
                }),
                _ => Self::InterruptDelivery(X86InterruptDeliveryAssumption {
                    psi,
                    profile,
                    entry,
                    installation,
                    interval: InterruptDeliveryInterval::InstalledScopeLifetime,
                    delivery: X86InterruptDeliveryProfile::ArchitecturalExceptionsAndExternalIrqs,
                    environment_description_sha256,
                    source_provenance,
                    caller_provenance,
                }),
            })
        })
    }
}

fn read_digest<Context>(input: &mut RecordReader<'_, Context>) -> Result<[u8; 32], RecordError> {
    // A retained digest consumes the same allocation budget as older readers.
    input.allocate(32)?;
    Ok(input.take(32)?.try_into().expect("32-byte digest"))
}

pub fn encode_installation_assumptions(
    rows: &[InstallationEnvironmentAssumption],
) -> Result<Vec<u8>, String> {
    for row in rows {
        row.validate_description()?;
    }
    let mut output = RecordWriter::new(RecordLimits::default());
    output.extend(SCHEMA).map_err(|error| error.to_string())?;
    output
        .sequence(rows.len())
        .map_err(|error| error.to_string())?;
    for row in rows {
        row.write(&mut output).map_err(|error| error.to_string())?;
    }
    Ok(output.finish())
}

pub fn decode_installation_assumptions(
    bytes: &[u8],
) -> Result<Vec<InstallationEnvironmentAssumption>, String> {
    let mut input = RecordReader::<()>::new(bytes, RecordLimits::default())
        .map_err(|error| error.to_string())?;
    if input
        .take(SCHEMA.len())
        .map_err(|error| error.to_string())?
        != SCHEMA
    {
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

#[cfg(test)]
mod tests;
