//! Private recovery of a complete source continuation. Decoding retains only
//! authority comparison data; the caller rejoins its current authority before
//! Build can mutate or evaluate the recovered input. Completed source charges
//! and the full resolver/typed correspondence travel with the graph.
use super::{SourceCompletionInput, SourceContext, SourceContinuation};
use build_time_evaluation::BuildTimeSelectionAuthority;
use compiler_storage::{Record, RecordError, RecordReader, RecordWriter};
use std::sync::Arc;

const SOURCE_CONTEXT_RECORD: [u8; 8] = *b"PSICTX03";

impl Record for SourceContext {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.record(|output| {
            SOURCE_CONTEXT_RECORD.write(output)?;
            self.declaration_syntax.write(output)?;
            self.normalization_context.write(output)?;
            self.bindings.write(output)?;
            self.build_helper_identity.write(output)?;
            let captured;
            let recorded = match &self.recorded_authority {
                Some(recorded) => recorded,
                None => {
                    captured = validation::RecordedDeclarationAuthority::capture(
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
            if <[u8; 8]>::read(input)? != SOURCE_CONTEXT_RECORD {
                return Err(input.error("unsupported source-context record schema"));
            }
            Ok(Self {
                declaration_syntax: Record::read(input)?,
                normalization_context: Record::read(input)?,
                bindings: Record::read(input)?,
                build_helper_identity: Record::read(input)?,
                authority: None,
                recorded_authority: Some(Record::read(input)?),
            })
        })
    }
}
impl SourceCompletionInput {
    pub(super) fn rejoin_current_authority(
        self: &Arc<Self>,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Result<Arc<Self>, String> {
        let Some(recorded) = &self.context.recorded_authority else {
            return Ok(self.clone());
        };
        let sources = self
            .typing
            .typed()
            .symbols
            .source_map()
            .ok_or("restored source continuation has no source context")?;
        recorded.require_current(authority.as_deref(), &sources)?;
        let mut input = self.as_ref().clone();
        for pending in &mut input.pending_pre_checks {
            pending.rejoin_source_authority(input.typing.typed(), authority.clone())?;
        }
        input.context.authority = authority;
        input.context.recorded_authority = None;
        Ok(Arc::new(input))
    }
}
impl SourceContinuation {
    /// Recover the original source input under the receiving invocation's
    /// declaration authority. Changed scope is a frontend cache miss, not a
    /// weakened selector predicate. Build effects and grants remain fresh.
    pub fn rejoin_current_authority(
        self,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Result<Self, String> {
        if self.input.context.recorded_authority.is_none() {
            return Ok(self);
        }
        Ok(Self {
            input: self.input.rejoin_current_authority(authority.clone())?,
            completions: self.completions.rejoin_current_authority(authority)?,
        })
    }
}
