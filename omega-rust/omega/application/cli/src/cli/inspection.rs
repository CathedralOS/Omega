//! Text presentation of a checked terminal inspection.

use super::arguments::InspectTerminalArguments;
use omega::inspection::{InspectTerminalRequest, inspect_terminal, inspect_terminal_artifact};
use semantic_vocabulary::{ServiceId, StructuralTypeId};
use std::fmt::Write;
use terminal_psi::{
    OperationKind, StructuralArgument, TerminalMachineResult, TerminalModule, Terminator,
};

pub(crate) fn run(arguments: InspectTerminalArguments) {
    let (selected, result, output) = match arguments {
        InspectTerminalArguments::Source {
            machine,
            root_path,
            target_name,
            output,
        } => {
            let request = InspectTerminalRequest {
                root_path,
                machine,
                target_name,
            };
            (request.machine.clone(), inspect_terminal(&request), output)
        }
        InspectTerminalArguments::Artifact { path } => {
            use std::io::Read;
            const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
            let bytes = (|| -> std::io::Result<Vec<u8>> {
                let file = std::fs::File::open(path)?;
                let mut bytes = Vec::new();
                file.take(MAX_ARTIFACT_BYTES + 1).read_to_end(&mut bytes)?;
                if bytes.len() as u64 > MAX_ARTIFACT_BYTES {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Terminal inspection artifact exceeds 64 MiB",
                    ));
                }
                Ok(bytes)
            })();
            let bytes = match bytes {
                Ok(bytes) => bytes,
                Err(error) => {
                    eprintln!("cannot read Terminal artifact: {error}");
                    std::process::exit(1);
                }
            };
            (
                "artifact".to_owned(),
                inspect_terminal_artifact(&bytes),
                None,
            )
        }
    };
    match result {
        Ok(inspection) => {
            if let Some(path) = output {
                if let Err(error) = std::fs::write(path, inspection.artifact.to_bytes()) {
                    eprintln!("cannot write Terminal artifact: {error}");
                    std::process::exit(1);
                }
            }
            print!(
                "{}",
                terminal_summary(&selected, inspection.artifact.semantic_module())
            );
            print!("terminal program_fingerprint=");
            for byte in inspection
                .artifact
                .view()
                .identity()
                .program_fingerprint
                .as_bytes()
            {
                print!("{byte:02x}");
            }
            println!();
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn terminal_summary(selected_machine: &str, module: &TerminalModule) -> String {
    let mut output = String::new();
    writeln!(
        output,
        "terminal selected_machine={} entry={:?}",
        selected_machine, module.entry
    )
    .expect("writing to a String cannot fail");
    for family in &module.target_machine_families {
        writeln!(output, "target_family canonical={:?}", family.canonical)
            .expect("writing to a String cannot fail");
        for member in &family.members {
            writeln!(
                output,
                "  target={} callable={:?} supply={:?}",
                member.target, member.callable, member.supply,
            )
            .expect("writing to a String cannot fail");
        }
    }
    for candidate in &module.provider_candidates {
        writeln!(
            output,
            "provider_candidate boundary=boundary:{} requirement={} provider={} candidate={} callable=machine:{}",
            candidate.boundary.get(),
            module.identities.text(candidate.requirement_identity).unwrap_or_default(),
            module.identities.text(candidate.provider_identity).unwrap_or_default(),
            module.identities.text(candidate.candidate_identity).unwrap_or_default(),
            candidate.candidate.get(),
        )
        .expect("writing to a String cannot fail");
    }
    for input in &module.placed_view_inputs {
        writeln!(output, "placed_input {input:?}").expect("writing to a String cannot fail");
    }
    for declaration in &module.structural_types {
        writeln!(
            output,
            "type id=type:{} identity={} shape={}",
            declaration.id.get(),
            module
                .identities
                .text(declaration.identity)
                .unwrap_or_default(),
            match &declaration.shape {
                terminal_psi::StructuralTypeShape::BindingCarrier => {
                    "binding_carrier".to_owned()
                }
                terminal_psi::StructuralTypeShape::Reference { referent, access } => {
                    format!(
                        "reference(referent=type:{},access={access:?})",
                        referent.get()
                    )
                }
                terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                    format!("primitive_scalar({scalar_type:?})")
                }
                terminal_psi::StructuralTypeShape::ByteSequence(carrier) => match carrier {
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. } => {
                        "byte_sequence(borrowed_view)".to_owned()
                    }
                    terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. } => {
                        format!("byte_sequence(bounded_view,capacity={capacity})")
                    }
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => {
                        format!("byte_sequence(bounded_owned,capacity={capacity})")
                    }
                },
                terminal_psi::StructuralTypeShape::Record { fields, .. } => {
                    format!("record(fields={})", fields.len())
                }
                terminal_psi::StructuralTypeShape::FixedArray { element, length } => {
                    format!(
                        "fixed_array(element=type:{},length={length})",
                        element.get()
                    )
                }
                terminal_psi::StructuralTypeShape::ElementView { element } => {
                    format!("element_view(element=type:{})", element.get())
                }
                terminal_psi::StructuralTypeShape::Sum { cases } => {
                    format!("sum(cases={})", cases.len())
                }
                terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                    format!("mixed(fields={},cases={})", fields.len(), cases.len())
                }
            }
        )
        .expect("writing to a String cannot fail");
    }
    for declaration in &module.structural_domains {
        writeln!(
            output,
            "domain id=domain:{} identity={} carrier=type:{}",
            declaration.id.get(),
            module
                .identities
                .text(declaration.identity)
                .unwrap_or_default(),
            declaration.carrier.get()
        )
        .expect("writing to a String cannot fail");
    }
    for declaration in &module.services {
        writeln!(
            output,
            "service id=service:{} identity={} parents={}",
            declaration.id.get(),
            module
                .identities
                .text(declaration.identity)
                .unwrap_or_default(),
            format_ids(
                declaration
                    .parents
                    .iter()
                    .map(|parent| format!("service:{}", parent.get()))
            )
        )
        .expect("writing to a String cannot fail");
    }
    for boundary in &module.boundary_machines {
        writeln!(
            output,
            "boundary id=boundary:{} identity={} attachment={} services={} requirements={}",
            boundary.id.get(),
            module
                .identities
                .text(boundary.identity)
                .unwrap_or_default(),
            attachment_identity(module, boundary.attachment),
            service_ceiling(module, &boundary.published_service_ceiling),
            format_ids(boundary.requires.iter().map(|requirement| format!(
                "argument:{}:domain:{}",
                requirement.argument_index,
                requirement.domain.get()
            )))
        )
        .expect("writing to a String cannot fail");
    }
    for machine in &module.machines {
        writeln!(
            output,
            "machine id=machine:{} attachment={} result={} services={}",
            machine.id.get(),
            attachment_identity(module, machine.attachment),
            match machine.result {
                TerminalMachineResult::Unit => "unit",
                TerminalMachineResult::Scalar(_) => "scalar",
                TerminalMachineResult::Structural(_) => "structural",
            },
            service_ceiling(module, &machine.published_service_ceiling)
        )
        .expect("writing to a String cannot fail");
        if let Some(terminal_psi::TerminalRankedScc::Natural(components)) = &machine.ranked_scc {
            for component in components {
                writeln!(
                    output,
                    "control_cycle machine=machine:{} component={} ranking=natural",
                    machine.id.get(),
                    terminal_semantics::control_graph::control_cycle_identity(machine, component)
                        .get(),
                )
                .expect("writing to a String cannot fail");
            }
        }
        writeln!(
            output,
            "signature machine=machine:{} runtime_scalars={:?} erased_scalars={:?} erased_proofs={}",
            machine.id.get(), machine.parameters, machine.contract.erased_scalar_formals,
            proof_formals(module, &machine.contract.erased_proof_formals),
        ).expect("writing to a String cannot fail");
        for (index, parameter) in machine.structural_parameters.iter().enumerate() {
            writeln!(
                output,
                "parameter machine=machine:{} index={} place=place:{} type={} multiplicity={:?} qualifications={}",
                machine.id.get(),
                index,
                parameter.place.get(),
                structural_type_identity(module, parameter.structural_type).unwrap_or("unknown"),
                parameter.multiplicity,
                format_ids(
                    parameter
                        .qualifications
                        .iter()
                        .map(|domain| format!("domain:{}", domain.get()))
                )
            )
            .expect("writing to a String cannot fail");
        }
        for claim in &machine.entry_claims {
            writeln!(
                output,
                "claim machine=machine:{} id=claim:{} input=place:{}",
                machine.id.get(),
                claim.claim.get(),
                claim.input.get()
            )
            .expect("writing to a String cannot fail");
        }
        for block in &machine.blocks {
            writeln!(output,
                "block_signature machine=machine:{} block=block:{} runtime_scalars={:?} erased_scalars={:?} erased_proofs={}",
                machine.id.get(), block.id.get(), block.parameters, block.erased_scalar_formals,
                proof_formals(module, &block.erased_proof_formals),
            ).expect("writing to a String cannot fail");
            for operation in &block.operations {
                write_operation_summary(
                    &mut output,
                    module,
                    machine.id.get(),
                    block.id.get(),
                    operation,
                );
            }
            match &block.terminator {
                Terminator::ReturnUnit { edge, cleanup_actions } => writeln!(
                    output,
                    "terminator machine=machine:{} block=block:{} kind=ReturnUnit edge=edge:{} cleanup_actions={cleanup_actions:?}",
                    machine.id.get(),
                    block.id.get(),
                    edge.get(),
                ),
                other => writeln!(
                    output,
                    "terminator machine=machine:{} block=block:{} kind={other:?}",
                    machine.id.get(),
                    block.id.get()
                ),
            }
            .expect("writing to a String cannot fail");
        }
    }
    output
}

fn write_operation_summary(
    output: &mut String,
    module: &TerminalModule,
    machine: u64,
    block: u64,
    operation: &terminal_psi::Operation,
) {
    match &operation.kind {
        OperationKind::CallUnit {
            callee,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            ..
        } => {
            writeln!(output,
                "call_lanes machine=machine:{machine} block=block:{block} operation=operation:{} runtime_scalars={arguments:?} erased_scalars={erased_arguments:?} erased_proofs={erased_proof_arguments:?}",
                operation.id.get(),
            ).expect("writing to a String cannot fail");
            let callee_attachment = module
                .machine(*callee)
                .and_then(|machine| machine.attachment)
                .and_then(|id| structural_type_identity(module, id))
                .unwrap_or("none");
            writeln!(
                output,
                "operation machine=machine:{machine} block=block:{block} id=operation:{} kind=CallUnit callee=machine:{} callee_attachment={} arguments={} transfers={}",
                operation.id.get(),
                callee.get(),
                callee_attachment,
                argument_places(structural_arguments),
                format_ids(claim_transfers.iter().map(|transfer| format!(
                    "claim:{}->argument:{}",
                    transfer.claim.get(),
                    transfer.argument_index
                )))
            )
            .expect("writing to a String cannot fail");
        }
        OperationKind::BoundaryCall {
            boundary,
            structural_arguments,
            completion_receipts,
            ..
        } => {
            let identity =
                terminal_psi::find_boundary_machine(&module.boundary_machines, *boundary)
                    .map(|boundary| {
                        module
                            .identities
                            .text(boundary.identity)
                            .unwrap_or_default()
                    })
                    .unwrap_or("unknown");
            writeln!(
                output,
                "operation machine=machine:{machine} block=block:{block} id=operation:{} kind=BoundaryCall boundary=boundary:{} boundary_identity={} arguments={} completion_receipts={}",
                operation.id.get(),
                boundary.get(),
                identity,
                argument_places(structural_arguments),
                format_ids(completion_receipts.iter().map(|receipt| format!(
                    "claim:{}->argument:{}",
                    receipt.claim.get(),
                    receipt.argument_index
                )))
            )
            .expect("writing to a String cannot fail");
        }
        OperationKind::PortWrite {
            service,
            port,
            value,
        } => {
            writeln!(
                output,
                "operation machine=machine:{machine} block=block:{block} id=operation:{} kind=PortWrite service=service:{} service_identity={} port=0x{port:04x} value=0x{value:02x}",
                operation.id.get(),
                service.get(),
                service_identity(module, *service).unwrap_or("unknown")
            )
            .expect("writing to a String cannot fail");
        }
        other => {
            writeln!(
                output,
                "operation machine=machine:{machine} block=block:{block} id=operation:{} kind={other:?}",
                operation.id.get()
            )
            .expect("writing to a String cannot fail");
        }
    }
}

fn format_ids(values: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", values.into_iter().collect::<Vec<_>>().join(","))
}

fn attachment_identity(module: &TerminalModule, attachment: Option<StructuralTypeId>) -> &str {
    attachment
        .and_then(|id| structural_type_identity(module, id))
        .unwrap_or("none")
}

fn service_ceiling(module: &TerminalModule, ceiling: &[ServiceId]) -> String {
    format_ids(ceiling.iter().map(|service| {
        format!(
            "service:{}:{}",
            service.get(),
            service_identity(module, *service).unwrap_or("unknown")
        )
    }))
}

fn argument_places(arguments: &[StructuralArgument]) -> String {
    format_ids(
        arguments
            .iter()
            .map(|argument| format!("place:{}", argument.place.get())),
    )
}

fn structural_type_identity(module: &TerminalModule, id: StructuralTypeId) -> Option<&str> {
    module
        .structural_type(id)
        .and_then(|declaration| module.identities.text(declaration.identity))
}

fn service_identity(module: &TerminalModule, id: ServiceId) -> Option<&str> {
    module
        .service(id)
        .and_then(|declaration| module.identities.text(declaration.identity))
}

/// Expose checked evidence separately from runtime signature positions. A type
/// identity in this roster must never be mistaken for an executable ABI slot.
fn proof_formals(module: &TerminalModule, formals: &[terminal_psi::ErasedProofFormal]) -> String {
    format_ids(formals.iter().map(|formal| {
        format!(
            "source:{}:{}",
            formal.source_position,
            module
                .identities
                .text(formal.type_identity)
                .unwrap_or("unknown"),
        )
    }))
}
