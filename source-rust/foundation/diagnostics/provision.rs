#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExceededProvision {
    pub resource: &'static str,
    pub limit: u64,
    pub requested: u64,
    pub coordinate: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, record_storage::Record)]
pub enum CoverageForm {
    MachineHeader,
    EntrySignature,
    StateSignature,
    StatementForm,
    CallForm,
    ExpressionForm,
    TransitionForm,
}

impl CoverageForm {
    pub fn from_resource(resource: &str) -> Option<Self> {
        match resource {
            "coverage_machine_headers" => Some(Self::MachineHeader),
            "coverage_entry_signatures" => Some(Self::EntrySignature),
            "coverage_state_signatures" => Some(Self::StateSignature),
            "coverage_statement_forms" => Some(Self::StatementForm),
            "coverage_call_forms" => Some(Self::CallForm),
            "coverage_expression_forms" => Some(Self::ExpressionForm),
            "coverage_transition_forms" => Some(Self::TransitionForm),
            _ => None,
        }
    }

    pub const fn resource(self) -> &'static str {
        match self {
            Self::MachineHeader => "coverage_machine_headers",
            Self::EntrySignature => "coverage_entry_signatures",
            Self::StateSignature => "coverage_state_signatures",
            Self::StatementForm => "coverage_statement_forms",
            Self::CallForm => "coverage_call_forms",
            Self::ExpressionForm => "coverage_expression_forms",
            Self::TransitionForm => "coverage_transition_forms",
        }
    }

    pub const fn exceeded(self, first_byte: u64) -> ExceededProvision {
        ExceededProvision {
            resource: self.resource(),
            limit: 0,
            requested: 1,
            coordinate: first_byte,
        }
    }
}

impl record_storage::Record for ExceededProvision {
    fn write(
        &self,
        output: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        output.record(|output| {
            self.resource.to_owned().write(output)?;
            self.limit.write(output)?;
            self.requested.write(output)?;
            self.coordinate.write(output)
        })
    }
    fn read<Context>(
        input: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
        input.record(|input| {
            let resource = String::read(input)?;
            let limit = u64::read(input)?;
            let requested = u64::read(input)?;
            let coordinate = u64::read(input)?;
            let form = CoverageForm::from_resource(&resource).ok_or_else(|| {
                input.error(format!("unknown retained provision resource `{resource}`"))
            })?;
            let recovered = form.exceeded(coordinate);
            if recovered.limit != limit || recovered.requested != requested {
                return Err(
                    input.error("retained coverage provision has inconsistent limit or demand")
                );
            }
            Ok(recovered)
        })
    }
}
