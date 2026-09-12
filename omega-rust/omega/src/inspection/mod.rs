use crate::arguments::InspectTerminalArguments;
use std::fmt::Write;

use compiler::compile_to_checked;
use semantic_vocabulary::{ServiceId, StructuralTypeId};
use terminal_psi::{OperationKind, TerminalMachineResult, TerminalModule, Terminator};

mod evidence;

pub(crate) fn run(arguments: InspectTerminalArguments) {
    let checked = match compile_to_checked(&arguments.root_path, arguments.target_name.as_deref()) {
        Ok(checked) => checked,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{diagnostic}");
            }
            std::process::exit(1);
        }
    };
    let lowered = match checked_trees_to_lowered_psi::lower_machine(&checked, &arguments.machine) {
        Ok(lowered) => lowered,
        Err(error) => {
            eprintln!(
                "cannot lower terminal machine `{}`: {error}",
                arguments.machine
            );
            std::process::exit(1);
        }
    };
    if let Err(diagnostics) =
        compiler::validate_lowered_ieee_float_comparison_custody(&checked, &lowered)
    {
        for diagnostic in diagnostics {
            eprintln!("{diagnostic}");
        }
        std::process::exit(1);
    }
    let fixed_fuel = match evidence::inspect(&lowered.semantic_module, &lowered.proof_bundle) {
        Ok(fixed_fuel) => fixed_fuel,
        Err(error) => {
            eprintln!(
                "cannot inspect terminal machine `{}`: {error}",
                arguments.machine
            );
            std::process::exit(1);
        }
    };
    print!(
        "{}",
        terminal_summary(&arguments.machine, &lowered.semantic_module, &fixed_fuel,)
    );
}

fn terminal_summary(
    selected_machine: &str,
    module: &TerminalModule,
    fixed_fuel: &evidence::FixedFuel,
) -> String {
    let mut output = String::new();
    writeln!(
        output,
        "terminal selected_machine={} entry=machine:{} verified=true",
        selected_machine,
        module.entry.get()
    )
    .expect("writing to a String cannot fail");
    for declaration in &module.structural_types {
        writeln!(
            output,
            "type id=type:{} identity={} shape={}",
            declaration.id.get(),
            declaration.identity,
            match &declaration.shape {
                terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                    format!("primitive_scalar({scalar_type:?})")
                }
                terminal_psi::StructuralTypeShape::ByteSequence(carrier) => match carrier {
                    terminal_psi::ByteSequenceCarrier::BorrowedView => {
                        "byte_sequence(borrowed_view)".to_owned()
                    }
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => {
                        format!("byte_sequence(bounded_owned,capacity={capacity})")
                    }
                },
                terminal_psi::StructuralTypeShape::Record { fields } => {
                    format!("record(fields={})", fields.len())
                }
                terminal_psi::StructuralTypeShape::FixedArray { element, length } => {
                    format!(
                        "fixed_array(element=type:{},length={length})",
                        element.get()
                    )
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
            declaration.identity,
            declaration.carrier.get()
        )
        .expect("writing to a String cannot fail");
    }
    for declaration in &module.services {
        writeln!(
            output,
            "service id=service:{} identity={} parents={}",
            declaration.id.get(),
            declaration.identity,
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
            boundary.identity,
            boundary
                .attachment
                .and_then(|id| structural_type_identity(module, id))
                .unwrap_or("none"),
            format_ids(boundary.published_service_ceiling.iter().map(|service| {
                format!(
                    "service:{}:{}",
                    service.get(),
                    service_identity(module, *service).unwrap_or("unknown")
                )
            })),
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
            machine
                .attachment
                .and_then(|id| structural_type_identity(module, id))
                .unwrap_or("none"),
            match machine.result {
                TerminalMachineResult::Unit => "unit",
                TerminalMachineResult::Scalar(_) => "scalar",
                TerminalMachineResult::Structural(_) => "structural",
            },
            format_ids(machine.published_service_ceiling.iter().map(|service| {
                format!(
                    "service:{}:{}",
                    service.get(),
                    service_identity(module, *service).unwrap_or("unknown")
                )
            }))
        )
        .expect("writing to a String cannot fail");
        if let Some(terminal_psi::TerminalRankedScc::Natural(components)) = &machine.ranked_scc {
            for component in components {
                writeln!(
                    output,
                    "control_cycle machine=machine:{} component={} ranking=natural",
                    machine.id.get(),
                    terminal_verifier::control_cycle_identity(machine, component).get(),
                )
                .expect("writing to a String cannot fail");
            }
        }
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
                Terminator::ReturnUnit {
                    edge,
                    trivial_affine_discards,
                } => writeln!(
                    output,
                    "terminator machine=machine:{} block=block:{} kind=ReturnUnit edge=edge:{} trivial_affine_discards={:?}",
                    machine.id.get(),
                    block.id.get(),
                    edge.get(),
                    trivial_affine_discards
                        .iter()
                        .map(|place| place.get())
                        .collect::<Vec<_>>()
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
    let fixed_fuel = match fixed_fuel {
        evidence::FixedFuel::Available(certificate) => certificate,
        evidence::FixedFuel::Unavailable(reason) => {
            writeln!(output, "fixed_fuel status=unknown reason={reason}")
                .expect("writing to a String cannot fail");
            return output;
        }
    };
    let identity = fixed_fuel.terminal_psi();
    writeln!(
        output,
        "fixed_fuel terminal_vocabulary={} terminal_fingerprint={} schedule={} entry=machine:{} ceiling_units={} relevant_preconditions={}",
        identity.vocabulary_marker.get(),
        identity.program_fingerprint,
        fixed_fuel.schedule().marker(),
        fixed_fuel.entry().get(),
        fixed_fuel.ceiling_units(),
        fixed_fuel.relevant_preconditions().len(),
    )
    .expect("writing to a String cannot fail");
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
            structural_arguments,
            claim_transfers,
            ..
        } => {
            let callee_attachment = module
                .machines
                .iter()
                .find(|machine| machine.id == *callee)
                .and_then(|machine| machine.attachment)
                .and_then(|id| structural_type_identity(module, id))
                .unwrap_or("none");
            writeln!(
                output,
                "operation machine=machine:{machine} block=block:{block} id=operation:{} kind=CallUnit callee=machine:{} callee_attachment={} arguments={} transfers={}",
                operation.id.get(),
                callee.get(),
                callee_attachment,
                format_ids(
                    structural_arguments
                        .iter()
                        .map(|argument| format!("place:{}", argument.place.get()))
                ),
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
            let identity = module
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == *boundary)
                .map(|boundary| boundary.identity.as_str())
                .unwrap_or("unknown");
            writeln!(
                output,
                "operation machine=machine:{machine} block=block:{block} id=operation:{} kind=BoundaryCall boundary=boundary:{} boundary_identity={} arguments={} completion_receipts={}",
                operation.id.get(),
                boundary.get(),
                identity,
                format_ids(
                    structural_arguments
                        .iter()
                        .map(|argument| format!("place:{}", argument.place.get()))
                ),
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

fn structural_type_identity(module: &TerminalModule, id: StructuralTypeId) -> Option<&str> {
    module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == id)
        .map(|declaration| declaration.identity.as_str())
}

fn service_identity(module: &TerminalModule, id: ServiceId) -> Option<&str> {
    module
        .services
        .iter()
        .find(|declaration| declaration.id == id)
        .map(|declaration| declaration.identity.as_str())
}
