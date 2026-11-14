// Format a `TerminalModule` into the line-oriented text dump printed by `omega psi inspect`.
//
// In machine and block signatures (`proof_formals`), erased proof formals are
// printed as `source:<position>:<type_identity>` in their own `erased_proofs=`
// column so checked proof evidence is never confused with a runtime ABI slot.

use semantic_vocabulary::{ServiceId, StructuralTypeId};
use std::fmt::Write;
use terminal_psi::{
    ByteSequenceCarrier, OperationKind, StructuralArgument, StructuralTypeShape,
    TerminalMachineResult, TerminalModule, Terminator,
};

pub fn terminal_summary(selected_machine: &str, module: &TerminalModule) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "terminal selected_machine={} entry={:?}",
        selected_machine, module.entry
    );
    for family in &module.target_machine_families {
        let _ = writeln!(output, "target_family canonical={:?}", family.canonical);
        for member in &family.members {
            let _ = writeln!(
                output,
                "  target={} callable={:?} supply={:?}",
                member.target, member.callable, member.supply,
            );
        }
    }
    for candidate in &module.provider_candidates {
        let _ = writeln!(
            output,
            "provider_candidate boundary=boundary:{} requirement={} provider={} candidate={} callable=machine:{}",
            candidate.boundary.get(),
            module
                .identities
                .text(candidate.requirement_identity)
                .unwrap_or_default(),
            module
                .identities
                .text(candidate.provider_identity)
                .unwrap_or_default(),
            module
                .identities
                .text(candidate.candidate_identity)
                .unwrap_or_default(),
            candidate.candidate.get(),
        );
    }
    for input in &module.access_view_inputs {
        let _ = writeln!(output, "access_input {input:?}");
    }
    for declaration in &module.structural_types {
        let _ = writeln!(
            output,
            "type id=type:{} identity={} shape={} copyable={}",
            declaration.id.get(),
            module
                .identities
                .text(declaration.identity)
                .unwrap_or_default(),
            format_type_shape(&declaration.shape),
            declaration.copyable
        );
    }
    for declaration in &module.structural_domains {
        let _ = writeln!(
            output,
            "domain id=domain:{} identity={} carrier=type:{} content_projection={:?}",
            declaration.id.get(),
            module
                .identities
                .text(declaration.identity)
                .unwrap_or_default(),
            declaration.carrier.get(),
            declaration.content_projection
        );
    }
    for declaration in &module.services {
        let _ = writeln!(
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
        );
    }
    for boundary in &module.boundary_machines {
        let _ = writeln!(
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
                "argument:{}:domain:{}:path:{:?}",
                requirement.argument_index,
                requirement.domain.get(),
                requirement.path
            )))
        );
    }
    for machine in &module.machines {
        write_machine_summary(&mut output, module, machine);
    }
    output
}

fn format_type_shape(shape: &StructuralTypeShape) -> String {
    match shape {
        StructuralTypeShape::Reference { referent, access } => {
            format!(
                "reference(referent=type:{},access={access:?})",
                referent.get()
            )
        }
        StructuralTypeShape::PrimitiveScalar(scalar_type) => {
            format!("primitive_scalar({scalar_type:?})")
        }
        StructuralTypeShape::AtomicCell(cell) => format!("atomic_cell({cell:?})"),
        StructuralTypeShape::ByteSequence(carrier) => match carrier {
            ByteSequenceCarrier::BorrowedView { .. } => "byte_sequence(borrowed_view)".to_owned(),
            ByteSequenceCarrier::BoundedView { capacity, .. } => {
                format!("byte_sequence(bounded_view,capacity={capacity})")
            }
            ByteSequenceCarrier::BoundedOwned { capacity } => {
                format!("byte_sequence(bounded_owned,capacity={capacity})")
            }
        },
        StructuralTypeShape::Record { fields, .. } => format!("record(fields={})", fields.len()),
        StructuralTypeShape::FixedArray { element, length } => {
            format!(
                "fixed_array(element=type:{},length={length})",
                element.get()
            )
        }
        StructuralTypeShape::ElementView { element, .. } => {
            format!("element_view(element=type:{})", element.get())
        }
        StructuralTypeShape::Sum { cases } => format!("sum(cases={})", cases.len()),
        StructuralTypeShape::Mixed { fields, cases } => {
            format!("mixed(fields={},cases={})", fields.len(), cases.len())
        }
    }
}

fn write_machine_summary(
    output: &mut String,
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
) {
    let _ = writeln!(
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
    );
    if let Some(terminal_psi::TerminalRankedScc::Natural(components)) = &machine.ranked_scc {
        for component in components {
            let _ = writeln!(
                output,
                "control_cycle machine=machine:{} component={} ranking=natural",
                machine.id.get(),
                terminal_semantics::control_graph::control_cycle_identity(machine, component).get(),
            );
        }
    }
    let _ = writeln!(
        output,
        "signature machine=machine:{} runtime_scalars={:?} erased_scalars={:?} erased_proofs={}",
        machine.id.get(),
        machine.parameters,
        machine.contract.erased_scalar_formals,
        proof_formals(module, &machine.contract.erased_proof_formals),
    );
    for (index, parameter) in machine.structural_parameters.iter().enumerate() {
        let _ = writeln!(
            output,
            "parameter machine=machine:{} index={} place=place:{} type={} multiplicity={:?} qualifications={} projected_qualifications={:?}",
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
            ),
            parameter.projected_qualifications,
        );
    }
    for claim in &machine.entry_claims {
        let _ = writeln!(
            output,
            "claim machine=machine:{} id=claim:{} input=place:{}",
            machine.id.get(),
            claim.claim.get(),
            claim.input.get()
        );
    }
    for block in &machine.blocks {
        let _ = writeln!(
            output,
            "block_signature machine=machine:{} block=block:{} runtime_scalars={:?} erased_scalars={:?} erased_proofs={}",
            machine.id.get(),
            block.id.get(),
            block.parameters,
            block.erased_scalar_formals,
            proof_formals(module, &block.erased_proof_formals),
        );
        for (index, parameter) in block.structural_parameters.iter().enumerate() {
            let _ = writeln!(
                output,
                "block_parameter machine=machine:{} block=block:{} index={} declaration={parameter:?}",
                machine.id.get(),
                block.id.get(),
                index,
            );
        }
        for operation in &block.operations {
            if let Some(result) = operation.result.structural() {
                let _ = writeln!(
                    output,
                    "structural_result machine=machine:{} operation=operation:{} result={result:?}",
                    machine.id.get(),
                    operation.id.get(),
                );
            }
            write_operation_summary(output, module, machine.id.get(), block.id.get(), operation);
        }
        match &block.terminator {
            Terminator::ReturnUnit {
                edge,
                cleanup_actions,
            } => {
                let _ = writeln!(
                    output,
                    "terminator machine=machine:{} block=block:{} kind=ReturnUnit edge=edge:{} cleanup_actions={cleanup_actions:?}",
                    machine.id.get(),
                    block.id.get(),
                    edge.get(),
                );
            }
            other => {
                let _ = writeln!(
                    output,
                    "terminator machine=machine:{} block=block:{} kind={other:?}",
                    machine.id.get(),
                    block.id.get()
                );
            }
        }
    }
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
            let _ = writeln!(
                output,
                "call_lanes machine=machine:{machine} block=block:{block} operation=operation:{} runtime_scalars={arguments:?} erased_scalars={erased_arguments:?} erased_proofs={erased_proof_arguments:?}",
                operation.id.get(),
            );
            let callee_attachment = module
                .machine(*callee)
                .and_then(|machine| machine.attachment)
                .and_then(|id| structural_type_identity(module, id))
                .unwrap_or("none");
            let _ = writeln!(
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
            );
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
            let _ = writeln!(
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
            );
        }
        OperationKind::PortWrite {
            service,
            port,
            value,
        } => {
            let _ = writeln!(
                output,
                "operation machine=machine:{machine} block=block:{block} id=operation:{} kind=PortWrite service=service:{} service_identity={} port=0x{port:04x} value=0x{value:02x}",
                operation.id.get(),
                service.get(),
                service_identity(module, *service).unwrap_or("unknown")
            );
        }
        other => {
            let _ = writeln!(
                output,
                "operation machine=machine:{machine} block=block:{block} id=operation:{} kind={other:?}",
                operation.id.get()
            );
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
