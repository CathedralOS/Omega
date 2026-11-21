//! Bind a meaning template to the caller's actual scalar producers. A bodyless
//! declaration has no machine owner here. The caller prepares this index once;
//! every projection then reuses exact parameter, block, operation or call
//! membership rather than guessing a source class from a numeric ID.

use semantic_vocabulary::{ScalarTerm, ScalarType, ValueId};
use terminal_psi::{
    DirectBlockFloatParameter, DirectCallFloatResult, DirectMachineFloatParameter,
    DirectOperationFloatResult, FloatMeaningSource, TerminalMachine,
};

pub struct Sources {
    values: Vec<(ValueId, FloatMeaningSource)>,
}

impl Sources {
    /// The checker supplies its validated machine reference. Runtime value
    /// membership and unique operation IDs are checked before Storage exists;
    /// declaration result placeholders are not executable scalar producers.
    pub fn for_machine(machine: &TerminalMachine) -> Result<Self, &'static str> {
        let owner = machine.id;
        let mut sources = Self { values: Vec::new() };
        for parameter in &machine.parameters {
            if let ScalarType::IeeeFloat(format) = parameter.scalar_type {
                sources.insert(
                    parameter.id,
                    FloatMeaningSource::DirectMachineParameter(DirectMachineFloatParameter {
                        owner,
                        parameter: parameter.id,
                        format,
                    }),
                );
            }
        }
        for block in &machine.blocks {
            // Some entry forms repeat the complete direct scalar header. They
            // describe machine inputs, not a second block-parameter producer.
            let entry_header = block.id == machine.entry && block.parameters == machine.parameters;
            for parameter in &block.parameters {
                if !entry_header && let ScalarType::IeeeFloat(format) = parameter.scalar_type {
                    sources.insert(
                        parameter.id,
                        FloatMeaningSource::DirectBlockParameter(DirectBlockFloatParameter {
                            owner,
                            block: block.id,
                            parameter: parameter.id,
                            format,
                        }),
                    );
                }
            }
            for operation in &block.operations {
                let Some(result) = operation.result.scalar_ref() else {
                    continue;
                };
                let ScalarType::IeeeFloat(format) = result.scalar_type else {
                    continue;
                };
                let semantic = crate::operation_semantic_row(&operation.kind)
                    .map_err(|_| "float meaning producer has no unique operation semantic row")?;
                let source =
                    if semantic.custody() == crate::OperationSemanticCustody::CallComposition {
                        FloatMeaningSource::DirectCallResult(DirectCallFloatResult {
                            owner,
                            producer: operation.id,
                            result: result.id,
                            format,
                        })
                    } else {
                        FloatMeaningSource::DirectOperationResult(DirectOperationFloatResult {
                            owner,
                            producer: operation.id,
                            result: result.id,
                            format,
                        })
                    };
                sources.insert(result.id, source);
            }
        }
        sources.values.sort_unstable_by_key(|(value, _)| *value);
        if sources.values.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err("float meaning source has more than one actual producer");
        }
        Ok(sources)
    }

    fn insert(&mut self, value: ValueId, source: FloatMeaningSource) {
        self.values.push((value, source));
    }

    pub fn source(&self, term: &ScalarTerm) -> Result<FloatMeaningSource, &'static str> {
        match term {
            ScalarTerm::IeeeFloatLiteral(semantic_vocabulary::IeeeFloatValue::Binary32(bits)) => {
                Ok(FloatMeaningSource::ExactBinary32Literal(*bits))
            }
            ScalarTerm::IeeeFloatLiteral(semantic_vocabulary::IeeeFloatValue::Binary64(bits)) => {
                Ok(FloatMeaningSource::ExactBinary64Literal(*bits))
            }
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::IeeeFloat(format),
            } => {
                let position = self
                    .values
                    .binary_search_by_key(id, |(value, _)| *value)
                    .map_err(|_| "float meaning projection has no actual scalar producer")?;
                let source = &self.values[position].1;
                if source.format() != *format {
                    return Err("float meaning projection changes its actual source format");
                }
                Ok(source.clone())
            }
            // Resolve Entry/Current through the existing consumer-owned storage
            // binding first. A bare path does not identify its captured value.
            ScalarTerm::IeeeFloatField { .. } => {
                Err("float meaning projection needs the exact captured storage observation")
            }
            _ => Err("float meaning projection has no IEEE scalar source"),
        }
    }

    pub fn validate_source(
        &self,
        term: &ScalarTerm,
        claimed: &FloatMeaningSource,
    ) -> Result<(), &'static str> {
        if self.source(term)? != *claimed {
            return Err("float meaning source differs from its actual owner or producer class");
        }
        Ok(())
    }
}
