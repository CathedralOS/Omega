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
    /// A required proof goal whose form the prover has no rule for (code 36,
    /// `wiki/spec/build/compiler_request.md`, "Unproved goal form
    /// publication"). Its coordinate is the obligation's identity.
    ProofGoalForm,
    /// A required proof goal whose search ran out of its step budget
    /// (code 37). Its coordinate is the obligation's identity.
    ProofGoalSteps,
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
            "coverage_proof_goal_forms" => Some(Self::ProofGoalForm),
            "proof_search_goal_steps" => Some(Self::ProofGoalSteps),
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
            Self::ProofGoalForm => "coverage_proof_goal_forms",
            Self::ProofGoalSteps => "proof_search_goal_steps",
        }
    }

    /// The specified scalar-resource code of the form's provision.
    pub const fn code(self) -> u16 {
        match self {
            Self::MachineHeader => 17,
            Self::EntrySignature => 18,
            Self::StateSignature => 19,
            Self::StatementForm => 20,
            Self::CallForm => 21,
            Self::ExpressionForm => 22,
            Self::TransitionForm => 23,
            Self::ProofGoalForm => 36,
            Self::ProofGoalSteps => 37,
        }
    }

    /// The provision's specified limit: 0 for a coverage form, which has no
    /// capacity, else the largest amount a request may use.
    pub const fn limit(self) -> u64 {
        match self {
            Self::ProofGoalSteps => 100_000,
            _ => 0,
        }
    }

    /// The provision exceeded at its specified limit. `coordinate` is the
    /// first byte of a refused source row, or for a proof provision the
    /// identity of the obligation left unproved.
    pub const fn exceeded(self, coordinate: u64) -> ExceededProvision {
        self.exceeded_at(coordinate, self.limit())
    }

    /// The provision exceeded at `limit`, the limit the compiler selected
    /// for this use, which may be below the specified one: the request asked
    /// for one more than it.
    pub const fn exceeded_at(self, coordinate: u64, limit: u64) -> ExceededProvision {
        ExceededProvision {
            resource: self.resource(),
            limit,
            requested: limit.saturating_add(1),
            coordinate,
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
            if limit > form.limit() {
                return Err(input.error("retained provision exceeds its specified limit"));
            }
            let recovered = form.exceeded_at(coordinate, limit);
            if recovered.requested != requested {
                return Err(
                    input.error("retained coverage provision has inconsistent limit or demand")
                );
            }
            Ok(recovered)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_goal_step_budget_asks_for_one_more_step_than_its_limit() {
        let full = CoverageForm::ProofGoalSteps.exceeded(9);
        assert_eq!(
            (full.resource, full.limit, full.requested, full.coordinate),
            ("proof_search_goal_steps", 100_000, 100_001, 9)
        );
        let reduced = CoverageForm::ProofGoalSteps.exceeded_at(9, 8_192);
        assert_eq!((reduced.limit, reduced.requested), (8_192, 8_193));
        assert_eq!(CoverageForm::ProofGoalSteps.code(), 37);
    }

    #[test]
    fn a_coverage_form_has_no_capacity() {
        let goal_form = CoverageForm::ProofGoalForm.exceeded(9);
        assert_eq!((goal_form.limit, goal_form.requested), (0, 1));
        assert_eq!(CoverageForm::ProofGoalForm.code(), 36);
        assert_eq!(
            CoverageForm::from_resource("proof_search_goal_steps"),
            Some(CoverageForm::ProofGoalSteps)
        );
    }
}
