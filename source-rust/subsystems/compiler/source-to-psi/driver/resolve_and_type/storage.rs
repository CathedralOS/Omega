use super::SourceContext;
use declaration_scope::PackageDeclarationScope;
use record_storage::{Record, RecordError, RecordReader, RecordWriter};

impl Record for SourceContext {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.record(|output| {
            self.bindings.write(output)?;
            self.build_helper_identity.write(output)?;
            self.purpose.write(output)?;
            PackageDeclarationScope::write_checkpoint(self.declaration_scope.as_deref(), output)
        })
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        input.record(|input| {
            Ok(Self {
                bindings: Record::read(input)?,
                build_helper_identity: Record::read(input)?,
                purpose: Record::read(input)?,
                declaration_scope: PackageDeclarationScope::read_checkpoint(input)?,
            })
        })
    }
}
