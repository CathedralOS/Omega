//! Canonical transport of callable query results, without their analysis graph.
//!
//! The result keeps the evaluated code, argument paths, unresolved ceilings and
//! formal-input associations. Encoding it does not prove the analysis correct;
//! an independent source validator still compares it with the admitted program.

use crate::CodecError;
use crate::wire::{Reader, Writer, decode_canonical};
use crate::wire_fields::{
    Access, Field, Id, InvocationSite, List, Optional, Scalar, U32, U64, wire_enum, wire_record,
};
use semantic_vocabulary::CallableInvocationSite;
use terminal_psi::ResolvedStructuralPathSegment;
use terminal_semantics::callable_values::{
    CallableFormalInput, CallableInvocation, CallableInvocationTarget, CallableParameter,
    CallableValuePossibilities, ReachableCallableInvocation, ReachableCallableInvocations,
    ReachedCallableTarget,
};
use terminal_semantics::descriptor_lifetimes::DescriptorLocation;

// Positions are counts in the portable program, not host-sized wire values.
struct Position;
impl Field<usize> for Position {
    fn write(writer: &mut Writer, value: &usize) -> Result<(), CodecError> {
        writer.len("callable position", *value)
    }

    fn read(reader: &mut Reader<'_>) -> Result<usize, CodecError> {
        usize::try_from(reader.u32()?)
            .map_err(|_| CodecError::CollectionTooLong("callable position"))
    }
}

wire_record! {
    Summary: ReachableCallableInvocations {
        roots: List<Id, 8>,
        reached_machines: List<Id, 8>,
        invocations: List<ReachedInvocation, 51>,
        inputs: List<(FormalInput, Possibilities), 33>,
    }
    ReachedInvocation: ReachableCallableInvocation {
        caller: Id,
        invocation: Invocation,
        target: ReachedTarget,
        selected_machines: List<Id, 8>,
    }
    Invocation: CallableInvocation {
        site: Site,
        operation: Optional<Id>,
        boundary: Optional<Id>,
        position: (Id, U32),
        successor: Optional<Position>,
        target: InvocationTarget,
        scalar_arguments: List<Id, 8>,
        structural_arguments: List<Location, 12>,
        structural_actual_types: List<(Id, Access), 9>,
    }
    FormalInput: CallableFormalInput { machine: Id, parameter: Parameter, contract: Id }
    Possibilities: CallableValuePossibilities {
        descriptors: List<Id, 8>,
        inputs: List<FormalInput, 21>,
        unresolved_ceilings: List<Id, 8>,
    }
    Location: DescriptorLocation { root: Id, path: List<PathSegment> }
}

wire_enum! {
    Site: CallableInvocationSite, "callable invocation site" {
        0 => Invocation(site: InvocationSite),
        1 => CleanupAction { edge: Id, action_ordinal: U32 },
    }
    InvocationTarget: CallableInvocationTarget, "callable invocation target" {
        0 => Machine(machine: Id),
        1 => Value(value: Id, contract: Id),
        2 => Opaque,
    }
    ReachedTarget: ReachedCallableTarget, "reached callable target" {
        0 => Declaration(machine: Id),
        1 => Callable(values: Possibilities),
        2 => Opaque,
    }
    Parameter: CallableParameter, "callable parameter" {
        0 => Scalar(position: Position),
        1 => Structural { position: Position, path: List<PathSegment> },
    }
    PathSegment: ResolvedStructuralPathSegment, "resolved callable storage path" {
        0 => Field(field: Id),
        1 => Case(case: Id),
        2 => FixedIndex(index: U64),
        3 => RuntimeIndex { selector: Id },
        4 => Extent,
        5 => RepresentationView { structural_type: Id, stored_scalar: Optional<Scalar> },
        6 => Referent,
    }
}

pub fn encode_callable_invocations(
    summary: &ReachableCallableInvocations,
) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    Summary::write(&mut writer, summary)?;
    Ok(writer.finish())
}

pub fn decode_callable_invocations(
    bytes: &[u8],
) -> Result<ReachableCallableInvocations, CodecError> {
    decode_canonical(bytes, Summary::read, encode_callable_invocations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        BlockId, CallableContractId, InvocationSite as SemanticInvocation, MachineDescriptorId,
        MachineId, OperationId, PlaceId, StructuralTypeId, ValueId,
    };

    #[test]
    fn nonempty_callable_result_keeps_occurrences_paths_inputs_and_unresolved_code() {
        let machine = MachineId::new(1).unwrap();
        let contract = CallableContractId::new(2).unwrap();
        let input = CallableFormalInput {
            machine,
            parameter: CallableParameter::Structural {
                position: 3,
                path: vec![
                    ResolvedStructuralPathSegment::Referent,
                    ResolvedStructuralPathSegment::RuntimeIndex {
                        selector: ValueId::new(4).unwrap(),
                    },
                ],
            },
            contract,
        };
        let possibilities = CallableValuePossibilities {
            descriptors: vec![MachineDescriptorId::new(5).unwrap()],
            inputs: vec![input.clone()],
            unresolved_ceilings: vec![contract],
        };
        let operation = OperationId::new(6).unwrap();
        let summary = ReachableCallableInvocations {
            roots: vec![machine],
            reached_machines: vec![machine],
            invocations: vec![ReachableCallableInvocation {
                caller: machine,
                invocation: CallableInvocation {
                    site: CallableInvocationSite::Invocation(SemanticInvocation::Operation(
                        operation,
                    )),
                    operation: Some(operation),
                    boundary: None,
                    position: (BlockId::new(7).unwrap(), 8),
                    successor: Some(9),
                    target: CallableInvocationTarget::Value(ValueId::new(10).unwrap(), contract),
                    scalar_arguments: vec![ValueId::new(11).unwrap()],
                    structural_arguments: vec![DescriptorLocation {
                        root: PlaceId::new(12).unwrap(),
                        path: vec![ResolvedStructuralPathSegment::RepresentationView {
                            structural_type: StructuralTypeId::new(13).unwrap(),
                            stored_scalar: Some(semantic_vocabulary::ScalarType::Boolean),
                        }],
                    }],
                    structural_actual_types: vec![(
                        StructuralTypeId::new(13).unwrap(),
                        terminal_psi::StructuralAccess::SharedBorrow,
                    )],
                },
                target: ReachedCallableTarget::Callable(possibilities.clone()),
                selected_machines: vec![machine],
            }],
            inputs: vec![(input, possibilities)],
        };
        let bytes = encode_callable_invocations(&summary).unwrap();
        assert_eq!(decode_callable_invocations(&bytes).unwrap(), summary);
        assert!(!summary.invocations[0].targets_are_resolved());
        for prefix in 0..bytes.len() {
            assert!(decode_callable_invocations(&bytes[..prefix]).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            decode_callable_invocations(&trailing),
            Err(CodecError::TrailingBytes(1))
        ));
        let mut cleanup = summary.clone();
        cleanup.invocations[0].invocation.site = CallableInvocationSite::CleanupAction {
            edge: semantic_vocabulary::EdgeId::new(14).unwrap(),
            action_ordinal: 2,
        };
        cleanup.invocations[0].invocation.operation = None;
        cleanup.invocations[0].invocation.target = CallableInvocationTarget::Machine(machine);
        cleanup.invocations[0].target = ReachedCallableTarget::Declaration(machine);
        cleanup.inputs[0].0.parameter = CallableParameter::Scalar(0);
        let encoded = encode_callable_invocations(&cleanup).unwrap();
        assert_eq!(decode_callable_invocations(&encoded).unwrap(), cleanup);
        cleanup.invocations[0].invocation.target = CallableInvocationTarget::Opaque;
        cleanup.invocations[0].target = ReachedCallableTarget::Opaque;
        let encoded = encode_callable_invocations(&cleanup).unwrap();
        assert_eq!(decode_callable_invocations(&encoded).unwrap(), cleanup);
        let mut changed = summary;
        changed.invocations[0].invocation.successor = None;
        assert_ne!(encode_callable_invocations(&changed).unwrap(), bytes);
    }
}
