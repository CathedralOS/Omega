//! Import already-validated scalar aggregate contents through constructor
//! storage. Pure scalar array subtrees use its existing array materializer;
//! treating every indexed byte as a primitive cell would create the wrong
//! backing for subsequent array reads, copies and borrowed descriptors.

use super::ConstructorStorage;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::primitive_storage::PrimitiveStorageEndpoint;
use crate::values::{
    ScalarAggregatePart, ScalarAggregatePlan, StructuralRuntimePlace, StructuralScalarRuntimeField,
    TerminalScalarAggregateValue,
};
use semantic_vocabulary::CanonicalStructuralPathSegment as Path;

impl ConstructorStorage {
    pub(crate) fn stage_scalar_aggregate(
        &mut self,
        execution: &TerminalExecution,
        aggregate: &TerminalScalarAggregateValue,
        root: &StructuralRuntimePlace,
    ) -> Result<(), TerminalInterpretError> {
        let destination = |path: &[Path]| {
            let mut destination = root.clone();
            destination.path.extend(
                terminal_semantics::canonical_structural_storage_path(
                    execution.program.structural_types().values(),
                    aggregate.structural_type,
                    path,
                )
                .ok_or_else(malformed)?,
            );
            Ok::<_, TerminalInterpretError>(destination)
        };
        if let Some(selected) = aggregate.result_case {
            self.stage_case(root.clone(), aggregate.structural_type, selected)?;
            for (path, scalar) in &aggregate.fields {
                let [Path::Field(field)] = path.as_slice() else {
                    return Err(malformed());
                };
                self.stage_scalar_endpoint(
                    &PrimitiveStorageEndpoint::ScalarField(
                        StructuralScalarRuntimeField::case_payload(root.clone(), *field),
                    ),
                    *scalar,
                )?;
            }
            return Ok(());
        }
        let plan = ScalarAggregatePlan::new(
            execution
                .program
                .structural_types()
                .get(&aggregate.structural_type)
                .ok_or_else(malformed)?,
            |identity| execution.program.structural_types().get(&identity),
        )?;
        let mut fields = aggregate.fields.iter();
        plan.storage_parts(|path, part| {
            let endpoint = match part {
                ScalarAggregatePart::Array {
                    structural_type,
                    count,
                } => {
                    let elements = fields
                        .by_ref()
                        .take(count)
                        .map(|(_, value)| *value)
                        .collect::<Vec<_>>();
                    if elements.len() != count {
                        return Err(malformed());
                    }
                    self.stage_array(execution, structural_type, &elements, &destination(path)?)?;
                    return Ok(());
                }
                ScalarAggregatePart::Primitive(_) => {
                    PrimitiveStorageEndpoint::Scalar(destination(path)?)
                }
                ScalarAggregatePart::Field(_) => {
                    let Some((Path::Field(field), parents)) = path.split_last() else {
                        return Err(malformed());
                    };
                    PrimitiveStorageEndpoint::ScalarField(StructuralScalarRuntimeField::member(
                        destination(parents)?,
                        *field,
                    ))
                }
            };
            let (_, scalar) = fields.next().ok_or_else(malformed)?;
            self.stage_scalar_endpoint(&endpoint, *scalar)
        })?;
        if fields.next().is_some() {
            return Err(malformed());
        }
        Ok(())
    }
}
