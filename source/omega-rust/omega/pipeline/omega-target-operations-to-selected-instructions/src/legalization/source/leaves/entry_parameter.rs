use super::super::shared::*;
use super::{DerivedValue, LeafContext};

pub(super) fn derive<'a>(
    context: &LeafContext<'a>,
    expression: &TargetIntegerExpression,
    parameter_index: usize,
    location: &ScalarParameterLocation,
) -> Result<DerivedValue<'a>, LegalizationError> {
    let TargetIntegerExpression::Parameter { source_value, .. } = expression else {
        unreachable!("parameter catalog arm supplied the parameter derivation")
    };
    let ScalarParameterLocation::Register(register) = location else {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    };
    if context.nodes.len() != 1 || context.source_value != *source_value {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    let Some(parameter) = context.optimized.parameters.get(parameter_index) else {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    };
    let Some(abstract_parameter) = context.abstracted.parameters.get(parameter_index) else {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    };
    if parameter.value != context.source_value
        || parameter.scalar_type != context.u64_type
        || abstract_parameter.value != context.source_value
        || abstract_parameter.scalar_type != context.u64_type
    {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    Ok((
        &context.nodes[0],
        SourceLeafValue::EntryParameter {
            parameter_index,
            register: *register,
            definition_site: parameter.site,
        },
    ))
}
