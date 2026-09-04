use std::fmt::Write;
use std::path::PathBuf;

use omega_compiler::compile_to_checked;
use psi_core::{ServiceId, StructuralTypeId};
use psi_proof_admission::AdmissionProfile;
use psi_terminal::{OperationKind, TerminalMachineResult, TerminalModule, Terminator};
use psi_terminal_fixed_fuel::{
    derive_fixed_entry_fuel, derive_ranked_countdown_entry_fuel, validate_fixed_entry_fuel,
    validate_ranked_countdown_entry_fuel,
};
use psi_terminal_verifier::{verify_module, verify_module_for_fixed_fuel};

pub(super) fn run(arguments: impl Iterator<Item = std::ffi::OsString>) {
    let Some(arguments) = parse_inspect_terminal_arguments(arguments) else {
        eprintln!(
            "usage: omega inspect-terminal --machine <qualified> [--target <name>] <root.omg>"
        );
        std::process::exit(2);
    };
    let checked = match compile_to_checked(&arguments.root_path, arguments.target_name.as_deref()) {
        Ok(checked) => checked,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{diagnostic}");
            }
            std::process::exit(1);
        }
    };
    let lowered = match psi_checked_trees_to_terminal::lower_machine(&checked, &arguments.machine) {
        Ok(lowered) => lowered,
        Err(error) => {
            eprintln!(
                "cannot lower terminal machine `{}`: {error}",
                arguments.machine
            );
            std::process::exit(1);
        }
    };
    let fixed_fuel = if lowered
        .semantic_module
        .machines
        .iter()
        .any(|machine| machine.ranked_scc.is_some())
    {
        let verified = match verify_module_for_fixed_fuel(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        ) {
            Ok(verified) => verified,
            Err(error) => {
                eprintln!(
                    "cannot verify terminal machine `{}` for fixed fuel: {error}",
                    arguments.machine
                );
                std::process::exit(1);
            }
        };
        let fixed_fuel =
            match derive_ranked_countdown_entry_fuel(&verified, lowered.semantic_module.entry) {
                Ok(fixed_fuel) => fixed_fuel,
                Err(error) => {
                    eprintln!(
                        "cannot derive ranked fixed fuel for terminal machine `{}`: {error}",
                        arguments.machine
                    );
                    std::process::exit(1);
                }
            };
        if let Err(error) = validate_ranked_countdown_entry_fuel(&verified, &fixed_fuel) {
            eprintln!(
                "cannot validate ranked fixed fuel for terminal machine `{}`: {error}",
                arguments.machine
            );
            std::process::exit(1);
        }
        fixed_fuel
    } else {
        let verified = match verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        ) {
            Ok(verified) => verified,
            Err(error) => {
                eprintln!(
                    "cannot verify terminal machine `{}`: {error}",
                    arguments.machine
                );
                std::process::exit(1);
            }
        };
        let fixed_fuel = match derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry) {
            Ok(fixed_fuel) => fixed_fuel,
            Err(error) => {
                eprintln!(
                    "cannot derive fixed fuel for terminal machine `{}`: {error}",
                    arguments.machine
                );
                std::process::exit(1);
            }
        };
        if let Err(error) = validate_fixed_entry_fuel(&verified, &fixed_fuel) {
            eprintln!(
                "cannot validate fixed fuel for terminal machine `{}`: {error}",
                arguments.machine
            );
            std::process::exit(1);
        }
        fixed_fuel
    };
    print!(
        "{}",
        terminal_summary(&arguments.machine, &lowered.semantic_module, &fixed_fuel,)
    );
}

struct InspectTerminalArguments {
    machine: String,
    root_path: PathBuf,
    target_name: Option<String>,
}

fn parse_inspect_terminal_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Option<InspectTerminalArguments> {
    let mut machine = None;
    let mut root_path = None;
    let mut target_name = None;
    while let Some(argument) = arguments.next() {
        if argument == "--machine" {
            if machine.is_some() {
                return None;
            }
            machine = arguments.next().and_then(|value| value.into_string().ok());
            machine.as_ref()?;
            continue;
        }
        if argument == "--target" {
            if target_name.is_some() {
                return None;
            }
            target_name = arguments.next().and_then(|value| value.into_string().ok());
            target_name.as_ref()?;
            continue;
        }
        if root_path.is_some() || argument.to_string_lossy().starts_with('-') {
            return None;
        }
        root_path = Some(PathBuf::from(argument));
    }
    Some(InspectTerminalArguments {
        machine: machine?,
        root_path: root_path?,
        target_name,
    })
}

fn terminal_summary(
    selected_machine: &str,
    module: &TerminalModule,
    fixed_fuel: &psi_terminal_fixed_fuel::FixedEntryFuelCertificate,
) -> String {
    let mut output = String::new();
    writeln!(
        output,
        "terminal selected_machine={} entry=machine:{}",
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
                psi_terminal::StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                    format!("primitive_scalar({scalar_type:?})")
                }
                psi_terminal::StructuralTypeShape::ByteSequence(carrier) => match carrier {
                    psi_terminal::ByteSequenceCarrier::BorrowedView => {
                        "byte_sequence(borrowed_view)".to_owned()
                    }
                    psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity } => {
                        format!("byte_sequence(bounded_owned,capacity={capacity})")
                    }
                },
                psi_terminal::StructuralTypeShape::Record { fields } => {
                    format!("record(fields={})", fields.len())
                }
                psi_terminal::StructuralTypeShape::FixedArray { element, length } => {
                    format!(
                        "fixed_array(element=type:{},length={length})",
                        element.get()
                    )
                }
                psi_terminal::StructuralTypeShape::Sum { cases } => {
                    format!("sum(cases={})", cases.len())
                }
                psi_terminal::StructuralTypeShape::Mixed { fields, cases } => {
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
    operation: &psi_terminal::Operation,
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
