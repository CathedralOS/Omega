//! Proof-output invocations on the wire: the target machine, static
//! requirement dispatch, runtime result and call, evidence arguments and
//! outputs of one invocation.

use super::super::wire_fields::{
    Commitment, Id, Index, InvocationSite, List, Optional, Scalar, Text, U32, U64, wire_enum,
    wire_record,
};
use terminal_psi::{
    ProofOutput, ProofOutputCall, ProofOutputEvidenceArgument, ProofOutputRuntimeCall,
    ProofOutputRuntimeResult, StaticRequirementDispatch,
};

wire_record! {
    Call: ProofOutputCall {
        caller: Id,
        ordinal: U32,
        target_machine_identity: Index,
        static_requirement_dispatch: Optional<Dispatch>,
        runtime_result: Optional<RuntimeResult>,
        runtime_call: Optional<RuntimeCall>,
        evidence_arguments: List<EvidenceArgument>,
        outputs: List<Output>,
    }
    Dispatch: StaticRequirementDispatch {
        conformance_application_report_fingerprint: U64,
        conformance_application_commitment: Commitment,
        public_requirement_identity: Index,
        declaring_trait_identity: Index,
        requirement_identity: Index,
        realization_identity: Index,
        realization_callable_identity: Index,
        realization: Id,
    }
    RuntimeCall: ProofOutputRuntimeCall { site: InvocationSite, callee: Id }
    EvidenceArgument: ProofOutputEvidenceArgument {
        input_position: U32,
        callee_proposition: Id,
        source: Id,
        instantiated_proposition: Id,
    }
    Output: ProofOutput {
        output_position: U32,
        output_field: Text,
        callee_proposition: Id,
        callee_output: Optional<Id>,
        instantiated_proposition: Id,
        forwarded_input_position: Optional<U32>,
        output: Optional<Id>,
    }
}

wire_enum! {
    RuntimeResult: ProofOutputRuntimeResult, "ProofOutputRuntimeResult" {
        0 => Unit,
        1 => Scalar(scalar: Scalar),
        2 => NoNormal,
    }
}
