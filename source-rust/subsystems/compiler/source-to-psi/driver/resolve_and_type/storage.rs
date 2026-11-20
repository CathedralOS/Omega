use super::SourceContext;
use record_storage::{Record, RecordError, RecordReader, RecordWriter};

impl Record for SourceContext {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.record(|output| {
            self.declaration_syntax.write(output)?;
            self.bindings.write(output)?;
            self.build_helper_identity.write(output)?;
            let captured;
            let recorded = match &self.recorded_authority {
                Some(recorded) => recorded,
                None => {
                    captured = crate::source_state::declaration_authority::RecordedDeclarationAuthority::capture(
                        self.authority.as_deref(),
                    )
                    .map_err(|reason| output.error(reason))?;
                    &captured
                }
            };
            recorded.write(output)
        })
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        input.record(|input| {
            Ok(Self {
                declaration_syntax: Record::read(input)?,
                bindings: Record::read(input)?,
                build_helper_identity: Record::read(input)?,
                authority: None,
                recorded_authority: Some(Record::read(input)?),
            })
        })
    }
}
