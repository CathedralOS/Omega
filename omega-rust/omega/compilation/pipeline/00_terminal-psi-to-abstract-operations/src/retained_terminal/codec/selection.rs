use super::records::record_fields;
use super::value::{RecordError, RecordReader, RecordValue, RecordWriter};

record_fields!(effects::SelectedProviderPlanCandidate {
    plans,
    assumed_bindings,
    report_fingerprint,
    installation_reach_resolutions,
    specialized_requirement_rows,
    external_requirement_applications,
    toolchain_settled_identities,
    identities,
});

impl RecordValue for effects::SelectedProviderPlanFacts {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.recovery_candidate().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        effects::SelectedProviderPlanCandidate::read(input)?
            .validate()
            .map_err(|reason| input.error(reason))
    }
}
