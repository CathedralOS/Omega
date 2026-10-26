//! Exact immutable evidence/declaration custody in optimizer content identity.
//! Numeric table coordinates remain bound to the original Terminal identity.
use super::CanonicalBytes;
use super::carrier_encoding::{encode_optional, encode_scalar_type};
use super::structural_encoding::{encode_evidence_contract_lane, encode_evidence_interface};
use crate::RuntimeEvidenceCatalog;

pub(super) fn encode_catalog(bytes: &mut CanonicalBytes, catalog: &RuntimeEvidenceCatalog) {
    bytes.u16(catalog.psi().vocabulary_marker.get());
    bytes.bytes(catalog.psi().program_fingerprint.as_bytes());
    bytes.slice(catalog.declarations(), |bytes, (machine, declaration)| {
        bytes.id(*machine);
        encode_optional(bytes, declaration.as_ref(), |bytes, declaration| {
            bytes.u32(declaration.machine.identity().0);
            bytes.u32(declaration.state.identity().0);
            bytes.u32(declaration.semantic_declaration.0);
            bytes.slice(&declaration.parameter_types, |bytes, identity| {
                bytes.u32(identity.0)
            });
            bytes.u32(declaration.callable.0);
            bytes.u32(declaration.result_type.0);
            bytes.bytes(&declaration.contract);
            encode_optional(
                bytes,
                declaration.specialization.as_ref(),
                |bytes, specialization| {
                    bytes.u32(specialization.template.identity().0);
                    bytes.u32(specialization.template_callable.0);
                    bytes.u32(specialization.template_semantic_declaration.0);
                    bytes.bytes(&specialization.commitment);
                },
            );
        });
    });
    bytes.slice(catalog.applications(), |bytes, application| {
        bytes.id(application.id);
        bytes.id(application.declaration);
        bytes.slice(&application.binder_arguments, |bytes, argument| {
            use terminal_psi::PropositionBinderArgumentKind as K;
            bytes.u8(match argument.kind {
                K::Type => 1,
                K::Const => 2,
                K::Machine => 3,
            });
            bytes.u32(argument.identity.0);
            encode_optional(
                bytes,
                argument.evidence_projection.as_ref(),
                |bytes, projection| {
                    bytes.id(projection.term);
                    bytes.u32(projection.declaring_trait_identity.0);
                    bytes.slice(&projection.declaring_trait_arguments, |bytes, argument| {
                        bytes.string(argument)
                    });
                    bytes.u32(projection.requirement_identity.0);
                },
            );
        });
        bytes.slice(&application.arguments, |bytes, argument| {
            bytes.string(argument)
        });
        encode_optional(
            bytes,
            application.evidence_interface.as_ref(),
            encode_evidence_interface,
        );
    });
    bytes.slice(catalog.terms(), |bytes, term| {
        bytes.id(term.id);
        bytes.id(term.proposition);
        encode_evidence_interface(bytes, &term.interface);
    });
    bytes.slice(catalog.lanes(), encode_evidence_contract_lane);
    bytes.slice(catalog.calls(), |bytes, call| {
        bytes.id(call.caller);
        bytes.u32(call.ordinal);
        bytes.u32(call.target_machine_identity.0);
        encode_optional(
            bytes,
            call.static_requirement_dispatch.as_ref(),
            |bytes, dispatch| {
                bytes.u64(dispatch.conformance_application_report_fingerprint);
                bytes.bytes(&dispatch.conformance_application_commitment.as_bytes());
                bytes.u32(dispatch.public_requirement_identity.0);
                bytes.u32(dispatch.declaring_trait_identity.0);
                bytes.u32(dispatch.requirement_identity.0);
                bytes.u32(dispatch.realization_identity.0);
                bytes.u32(dispatch.realization_callable_identity.0);
                bytes.id(dispatch.realization);
            },
        );
        encode_optional(
            bytes,
            call.runtime_result.as_ref(),
            |bytes, result| match result {
                terminal_psi::ProofOutputRuntimeResult::Unit => bytes.u8(1),
                terminal_psi::ProofOutputRuntimeResult::NoNormal => bytes.u8(3),
                terminal_psi::ProofOutputRuntimeResult::Scalar(scalar_type) => {
                    bytes.u8(2);
                    encode_scalar_type(bytes, *scalar_type);
                }
            },
        );
        encode_optional(bytes, call.runtime_call.as_ref(), |bytes, runtime| {
            let mut site = Vec::new();
            runtime.site.encode_identity(&mut site);
            bytes.slice(&site, |bytes, value| bytes.u8(*value));
            bytes.id(runtime.callee);
        });
        bytes.slice(&call.evidence_arguments, |bytes, argument| {
            bytes.u32(argument.input_position);
            bytes.id(argument.callee_proposition);
            bytes.id(argument.source);
            bytes.id(argument.instantiated_proposition);
        });
        bytes.slice(&call.outputs, |bytes, output| {
            bytes.u32(output.output_position);
            bytes.string(&output.output_field);
            bytes.id(output.callee_proposition);
            encode_optional(bytes, output.callee_output.as_ref(), |bytes, term| {
                bytes.id(*term)
            });
            bytes.id(output.instantiated_proposition);
            encode_optional(
                bytes,
                output.forwarded_input_position.as_ref(),
                |bytes, position| bytes.u32(*position),
            );
            encode_optional(bytes, output.output.as_ref(), |bytes, term| bytes.id(*term));
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{EvidenceTermId, MachineId, OperationId, PropositionId};
    use terminal_psi::{
        ProofOutputCall, ProofOutputEvidenceArgument, ProofOutputRuntimeCall,
        ProofOutputRuntimeResult, SemanticFingerprint, TerminalIdentityIndex, TerminalModule,
        TerminalPsiIdentity, VocabularyMarker,
    };

    #[test]
    fn invocation_subject_provenance_is_part_of_optimizer_content() {
        let machine = MachineId::new(1).unwrap();
        let mut module = TerminalModule::for_entry(machine);
        module.proof_output_calls.push(ProofOutputCall {
            caller: machine,
            ordinal: 0,
            target_machine_identity: TerminalIdentityIndex(1),
            static_requirement_dispatch: None,
            runtime_result: Some(ProofOutputRuntimeResult::Unit),
            runtime_call: Some(ProofOutputRuntimeCall {
                site: OperationId::new(1).unwrap().into(),
                callee: machine,
            }),
            evidence_arguments: vec![ProofOutputEvidenceArgument {
                input_position: 0,
                callee_proposition: PropositionId::new(1).unwrap(),
                source: EvidenceTermId::new(1).unwrap(),
                instantiated_proposition: PropositionId::new(2).unwrap(),
            }],
            outputs: Vec::new(),
        });
        let psi = TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        };
        let encoded = |module: &TerminalModule| {
            let mut bytes = CanonicalBytes::collect();
            encode_catalog(
                &mut bytes,
                &RuntimeEvidenceCatalog::from_verified_module(psi, module),
            );
            bytes.finish()
        };
        let before = encoded(&module);
        module.proof_output_calls[0].evidence_arguments[0].source = EvidenceTermId::new(2).unwrap();
        assert_ne!(before, encoded(&module));
        module.proof_output_calls[0]
            .runtime_call
            .as_mut()
            .unwrap()
            .site = OperationId::new(2).unwrap().into();
        assert_ne!(before, encoded(&module));
    }
}
