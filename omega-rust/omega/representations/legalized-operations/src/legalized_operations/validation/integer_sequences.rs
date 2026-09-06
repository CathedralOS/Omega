use crate::{LegalizedExactIntegerSequence, LegalizedIntegerSequenceError, LegalizedIntegerStep};
use optimization_unit::ValueDefinitionSite;
use semantic_vocabulary::{IntegerValue, ValueId};

impl LegalizedExactIntegerSequence {
    /// Checks data shape only. Source correspondence and proof authority require
    /// independent legalization admission; they are not established here.
    pub fn validate_shape(
        &self,
        entry_values: &[ValueId],
        result: ValueId,
    ) -> Result<(), LegalizedIntegerSequenceError> {
        use LegalizedIntegerSequenceError as Error;
        let mut available = Vec::new();
        for &value in entry_values {
            if available.contains(&value) {
                return Err(Error::DuplicateValue(value));
            }
            available.push(value);
        }
        let mut operations = Vec::new();
        let mut sites = Vec::new();
        for step in &self.steps {
            let (value, operation, site) = match step {
                LegalizedIntegerStep::Immediate(immediate) => {
                    if !matches!(immediate.value, IntegerValue::Unsigned(value) if value <= u128::from(u64::MAX))
                    {
                        return Err(Error::NonU64Immediate(immediate.source_value));
                    }
                    (
                        immediate.source_value,
                        immediate.constant_operation,
                        immediate.definition_site,
                    )
                }
                LegalizedIntegerStep::ExactBinary(binary) => {
                    for operand in [binary.left, binary.right] {
                        if !available.contains(&operand) {
                            return Err(Error::UnavailableValue(operand));
                        }
                    }
                    (
                        binary.source_value,
                        binary.operation,
                        binary.definition_site,
                    )
                }
            };
            if available.contains(&value) {
                return Err(Error::DuplicateValue(value));
            }
            if operations.contains(&operation) {
                return Err(Error::DuplicateOperation(operation));
            }
            if !matches!(site, ValueDefinitionSite::Node { .. }) {
                return Err(Error::NonNodeDefinition(site));
            }
            if sites.contains(&site) {
                return Err(Error::DuplicateDefinitionSite(site));
            }
            available.push(value);
            operations.push(operation);
            sites.push(site);
        }
        if !available.contains(&result) {
            return Err(Error::UnavailableValue(result));
        }
        Ok(())
    }
}
