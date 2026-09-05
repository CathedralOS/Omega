use super::calling::{encode_call_plan, encode_placement, encode_shape};
use super::scalar::{
    encode_definition_site, encode_integer, encode_integer_type, encode_scalar_type,
};
use super::shared::*;
use super::structural::{encode_effect, encode_ownership_roster};

pub(super) fn encode_scalar_call_unit_function(
    bytes: &mut Vec<u8>,
    function: &LegalizedScalarCallUnitFunction,
) {
    bytes.extend_from_slice(&function.machine.get().to_le_bytes());
    bytes.extend_from_slice(&function.attachment.get().to_le_bytes());
    encode_ids(
        bytes,
        function
            .provenance
            .operations
            .iter()
            .map(|operation| operation.get()),
    );
    encode_ids(
        bytes,
        function.provenance.edges.iter().map(|edge| edge.get()),
    );
    bytes.push(match function.recipe {
        ScalarCallUnitLegalizationRecipe::U64EqualityConditionalThreeCallChainThenReturnUnitV1 => 0,
    });
    bytes.extend_from_slice(&function.entry_block.get().to_le_bytes());
    for constant in &function.constants {
        bytes.extend_from_slice(&constant.operation.get().to_le_bytes());
        bytes.extend_from_slice(&constant.result.get().to_le_bytes());
        encode_integer_type(bytes, constant.scalar_type);
        encode_integer(bytes, constant.value);
        encode_definition_site(bytes, constant.definition_site);
        encode_fuel(bytes, &constant.fuel);
        encode_effect(bytes, constant.effect);
        encode_ownership_roster(bytes, &constant.ownership);
    }
    for call in &function.calls {
        bytes.extend_from_slice(&call.operation.get().to_le_bytes());
        bytes.extend_from_slice(&call.callee.get().to_le_bytes());
        encode_call_plan(bytes, &call.call_plan);
        encode_home(bytes, call.result_home);
        encode_definition_site(bytes, call.result_definition_site);
        for argument in &call.arguments {
            bytes.extend_from_slice(&argument.parameter_index.to_le_bytes());
            encode_argument_source(bytes, argument.source);
            encode_placement(bytes, &argument.placement);
        }
        encode_ids(
            bytes,
            call.requirement_obligations
                .iter()
                .map(|obligation| obligation.get()),
        );
        let crash = terminal_codec::encode_crash_route_buckets(&call.crash_continuations)
            .expect("legalized crash routes were admitted by Terminal validation");
        encode_len(bytes, crash.len());
        bytes.extend_from_slice(&crash);
        encode_fuel(bytes, &call.fuel);
        encode_effect(bytes, call.effect);
        encode_ownership_roster(bytes, &call.ownership);
    }
    bytes.extend_from_slice(&function.return_edge.get().to_le_bytes());
    encode_fuel(bytes, &function.return_fuel);
    encode_effect(bytes, function.return_effect);
    encode_ownership_roster(bytes, &function.return_ownership);
}

fn encode_home(bytes: &mut Vec<u8>, home: target_operations::TargetUnitScalarHomeRequirement) {
    bytes.extend_from_slice(&home.defining_operation.get().to_le_bytes());
    bytes.extend_from_slice(&home.source_value.get().to_le_bytes());
    encode_scalar_type(bytes, home.scalar_type);
    encode_shape(bytes, home.shape);
}

fn encode_argument_source(
    bytes: &mut Vec<u8>,
    source: target_operations::TargetUnitScalarArgumentSource,
) {
    match source {
        target_operations::TargetUnitScalarArgumentSource::Parameter {
            parameter_index,
            source_value,
            scalar_type,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&parameter_index.to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
            encode_scalar_type(bytes, scalar_type);
        }
        target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            bytes.push(0);
            bytes.extend_from_slice(&defining_operation.get().to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
            encode_integer_type(bytes, scalar_type);
            encode_integer(bytes, value);
        }
        target_operations::TargetUnitScalarArgumentSource::BooleanImmediate {
            defining_operation,
            source_value,
            value,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&defining_operation.get().to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
            bytes.push(u8::from(value));
        }
        target_operations::TargetUnitScalarArgumentSource::Home(home) => {
            bytes.push(1);
            encode_home(bytes, home);
        }
    }
}
