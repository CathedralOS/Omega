//! Source-produced placed-view helpers reborrow one host-established backing.
//! Import adapters model the exact authored external operations on that backing;
//! they reject a detached carrier, so a staged copy cannot satisfy the oracle.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CheckedCompileRequest, CompilationProduct, CompilationSession};
use fixture_package_inputs::{repo_root, reviewed_repository_fixture_package_inputs};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use std::sync::{Arc, Mutex};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalImportAdapter,
    TerminalImportSignature, TerminalPlacedViewEstablishment, TerminalProgram, TerminalScalarValue,
    TerminalStructuralInputs, TerminalStructuralValue,
};

fn produce(machine: &str) -> CanonicalTerminalArtifact {
    let root = repo_root().join("tests/omega/pass/layouts/placed_view_call_custody/main.omg");
    let prepared =
        reviewed_repository_fixture_package_inputs(&root, None).expect("sealed fixture inputs");
    let mut request = CheckedCompileRequest::new(&prepared.root_path, None);
    request.package_inputs = prepared.package_inputs;
    CompilationSession::default()
        .produce(request, CompilationProduct::Inspection(machine.to_owned()))
        .and_then(compiler::CompilationOutput::into_inspection)
        .unwrap_or_else(|error| panic!("{machine}: {error:?}"))
}

#[derive(Debug)]
struct Registers {
    status: u32,
    transmit: u8,
    events: Vec<String>,
}

struct RegisterAdapter {
    signature: TerminalImportSignature,
    registers: Vec<(u64, Arc<Mutex<Registers>>)>,
    write: bool,
}

impl TerminalImportAdapter for RegisterAdapter {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}
impl TerminalEffectHandler for RegisterAdapter {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new("use the typed result handler"))
    }
    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            return Err(TerminalEffectRejection::new("unexpected effect"));
        };
        if structural_arguments.len() != 1 || structural_arguments[0].path.len() != 1 {
            return Err(TerminalEffectRejection::new(
                "accessor lost original placed backing",
            ));
        }
        let registers = self
            .registers
            .iter()
            .find(|(identity, _)| *identity == structural_arguments[0].opaque_identity)
            .ok_or_else(|| TerminalEffectRejection::new("accessor lost original placed backing"))?;
        let mut registers = registers.1.lock().unwrap();
        if self.write {
            let [
                TerminalScalarValue::Integer {
                    scalar_type,
                    value: IntegerValue::Unsigned(value),
                },
            ] = arguments.as_slice()
            else {
                return Err(TerminalEffectRejection::new("wrong write payload"));
            };
            assert_eq!(
                *scalar_type,
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap()
            );
            registers.transmit = u8::try_from(*value)
                .map_err(|_| TerminalEffectRejection::new("write out of range"))?;
            registers.events.push(format!("write:{value}"));
            Ok(TerminalEffectResult::Unit)
        } else {
            if !arguments.is_empty() {
                return Err(TerminalEffectRejection::new("read has scalar arguments"));
            }
            let value = registers.status;
            registers.events.push(format!("read:{value}"));
            Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                value: IntegerValue::Unsigned(value.into()),
            }))
        }
    }
}

fn qualify_helper(machine: &str, writes: bool) {
    let produced = produce(machine);
    // External ingress independently decodes and checks the new formal join.
    let artifact =
        CanonicalTerminalArtifact::from_bytes(&produced.to_bytes()).expect("checked transport");
    let program = TerminalProgram::borrow(artifact.view());
    let module = artifact.semantic_module();
    let entry = program.entry().unwrap();
    let [parameter] = entry.structural_parameters else {
        panic!("one placed formal");
    };
    let row = entry.placed_view_inputs().next().unwrap();
    let view = TerminalStructuralValue::minted(9910, parameter.structural_type, Vec::new());
    let backing = TerminalStructuralValue::minted(88001, parameter.structural_type, Vec::new());
    let establishment = TerminalPlacedViewEstablishment {
        input: row.clone(),
        referent: backing,
    };
    if machine == "delegate" {
        let error = terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput::prepare(
            &artifact,
            target::TargetProfile::host(),
            module.entry,
            std::slice::from_ref(&establishment),
        )
        .err()
        .expect("native subordinate custody remains gated");
        assert!(
            format!("{error:?}").contains("PlacedViewInputsRequireCustodyLowering"),
            "{error:?}"
        );
        eprintln!("native placed helper refusal: {error:?}");
    }
    assert!(matches!(
        program.instantiate(
            &[],
            TerminalStructuralInputs {
                arguments: std::slice::from_ref(&view),
                ..Default::default()
            }
        ),
        Err(
            terminal_interpreter::TerminalArtifactInterpretError::Execution(
                terminal_interpreter::TerminalInterpretError::PlacedViewInputsRequireCustody
            )
        )
    ));
    let mut instance = program
        .instantiate(
            &[],
            TerminalStructuralInputs {
                arguments: std::slice::from_ref(&view),
                placed_view_establishments: std::slice::from_ref(&establishment),
                ..Default::default()
            },
        )
        .expect("exact host establishment");
    let registers = Arc::new(Mutex::new(Registers {
        status: 41,
        transmit: 0,
        events: Vec::new(),
    }));
    let imports = instance.unbound_imports().collect::<Vec<_>>();
    assert_eq!(imports.len(), if writes { 2 } else { 1 });
    for boundary in imports {
        let declaration = module
            .boundary_machines
            .iter()
            .find(|declaration| declaration.id == boundary)
            .unwrap();
        let identity = module.identities.text(declaration.identity).unwrap();
        let write = identity.ends_with("::write");
        assert!(write || identity.ends_with("::read"));
        instance
            .bind_import(
                boundary,
                RegisterAdapter {
                    signature: declaration.into(),
                    registers: vec![(view.opaque_identity, Arc::clone(&registers))],
                    write,
                },
            )
            .unwrap();
    }
    let actual = instance
        .invoke(&mut TerminalFuelMeter::unbounded())
        .expect("helper executes");
    assert_eq!(
        actual,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                value: IntegerValue::Unsigned(41)
            }
        ))
    );
    let observed = registers.lock().unwrap();
    assert_eq!(observed.transmit, if writes { 9 } else { 0 });
    assert_eq!(
        observed.events,
        if writes {
            vec!["write:9", "read:41"]
        } else {
            vec!["read:41"]
        }
    );
    eprintln!("{machine}: {actual:?}; original backing={observed:?}");
    drop(observed);
    instance.close().expect("completed loan retires normally");
}

#[test]
fn shared_helper() {
    qualify_helper("delegate", false);
}
#[test]
fn mutable_helper() {
    qualify_helper("delegate_mutable", true);
}
#[test]
fn nested_helper() {
    qualify_helper("forward", true);
}
#[test]
fn later_state_helper() {
    qualify_helper("later", true);
}

fn scalar(value: u128, bits: u16) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

#[test]
fn reordered_views_preserve_origin_and_reject_conflicting_shared_descriptors() {
    let artifact = produce("reordered");
    let program = TerminalProgram::borrow(artifact.view());
    let entry = program.entry().unwrap();
    let rows = entry.placed_view_inputs().cloned().collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    let views = entry
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(position, formal)| {
            TerminalStructuralValue::minted(
                9910 + position as u64,
                formal.structural_type,
                Vec::new(),
            )
        })
        .collect::<Vec<_>>();
    let establishments = rows
        .iter()
        .enumerate()
        .map(|(position, row)| TerminalPlacedViewEstablishment {
            input: row.clone(),
            referent: TerminalStructuralValue::minted(
                88001 + position as u64,
                views[position].structural_type,
                Vec::new(),
            ),
        })
        .collect::<Vec<_>>();
    let originals = [41, 73]
        .into_iter()
        .map(|status| {
            Arc::new(Mutex::new(Registers {
                status,
                transmit: 0,
                events: Vec::new(),
            }))
        })
        .collect::<Vec<_>>();
    let mut instance = program
        .instantiate(
            &[],
            TerminalStructuralInputs {
                arguments: &views,
                placed_view_establishments: &establishments,
                ..Default::default()
            },
        )
        .unwrap();
    for boundary in instance.unbound_imports().collect::<Vec<_>>() {
        let declaration = artifact
            .semantic_module()
            .boundary_machines
            .iter()
            .find(|row| row.id == boundary)
            .unwrap();
        instance
            .bind_import(
                boundary,
                RegisterAdapter {
                    signature: declaration.into(),
                    registers: views
                        .iter()
                        .zip(&originals)
                        .map(|(view, backing)| (view.opaque_identity, Arc::clone(backing)))
                        .collect(),
                    write: false,
                },
            )
            .unwrap();
    }
    let actual = instance
        .invoke(&mut TerminalFuelMeter::unbounded())
        .unwrap();
    assert_eq!(
        actual,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(scalar(73041, 64)))
    );
    assert_eq!(originals[0].lock().unwrap().events, ["read:41"]);
    assert_eq!(originals[1].lock().unwrap().events, ["read:73"]);
    eprintln!("reordered: {actual:?}; original backings read 73 then 41");
    instance.close().unwrap();
    let mut stale = establishments.clone();
    stale[0].input.placement_commitment[0] ^= 1;
    let error = program
        .instantiate(
            &[],
            TerminalStructuralInputs {
                arguments: &views,
                placed_view_establishments: &stale,
                ..Default::default()
            },
        )
        .err()
        .expect("stale placement answers no declared row");
    assert!(format!("{error:?}").contains("PlacedViewInputEstablishmentUnexpected"));
    let duplicates = [establishments[0].clone(), establishments[0].clone()];
    let error = program
        .instantiate(
            &[],
            TerminalStructuralInputs {
                arguments: &views,
                placed_view_establishments: &duplicates,
                ..Default::default()
            },
        )
        .err()
        .expect("duplicate establishment refuses");
    assert!(format!("{error:?}").contains("PlacedViewInputEstablishmentDuplicate"));
    let ambiguous = [views[0].clone(), views[0].clone()];
    let error = program
        .instantiate(
            &[],
            TerminalStructuralInputs {
                arguments: &ambiguous,
                placed_view_establishments: &establishments,
                ..Default::default()
            },
        )
        .err()
        .expect("one descriptor cannot claim two backings");
    assert!(format!("{error:?}").contains("PlacedViewInputEstablishmentAliasing"));
    let same_backing = [
        establishments[0].clone(),
        TerminalPlacedViewEstablishment {
            input: rows[1].clone(),
            referent: establishments[0].referent.clone(),
        },
    ];
    // Shared aliases of the same exact association remain legal.
    program
        .instantiate(
            &[],
            TerminalStructuralInputs {
                arguments: &ambiguous,
                placed_view_establishments: &same_backing,
                ..Default::default()
            },
        )
        .unwrap()
        .close()
        .unwrap();
    for corruption in [0, 1, 2] {
        let mut module = artifact.semantic_module().clone();
        let row = module
            .placed_view_inputs
            .iter_mut()
            .find(|row| row.machine == rows[0].machine)
            .unwrap();
        match corruption {
            0 => row.runtime_position = 999,
            1 => row.source_state_identity = row.schema_identity,
            _ => row.carrier_identity = row.schema_identity,
        }
        let bytes = terminal_codec::encode_module(&module).expect("tampered relation encodes");
        let error = terminal_codec::decode_module(&bytes)
            .err()
            .expect("tampered relation rejects at external ingress");
        assert!(format!("{error:?}").contains("PlacedViewInput"));
        eprintln!("tampered formal relation {corruption}: {error:?}");
    }
}

#[test]
fn exclusive_backing_aliases_refuse_before_effects() {
    for machine in ["alias_entry", "ordinary_alias"] {
        let artifact = produce(machine);
        let program = TerminalProgram::borrow(artifact.view());
        let entry = program.entry().unwrap();
        let views = entry
            .structural_parameters
            .iter()
            .enumerate()
            .map(|(position, formal)| {
                TerminalStructuralValue::minted(
                    if position == 0 { 9910 } else { 88001 },
                    formal.structural_type,
                    Vec::new(),
                )
            })
            .collect::<Vec<_>>();
        let establishments = entry
            .placed_view_inputs()
            .map(|row| TerminalPlacedViewEstablishment {
                input: row.clone(),
                referent: TerminalStructuralValue::minted(
                    88001,
                    views[0].structural_type,
                    Vec::new(),
                ),
            })
            .collect::<Vec<_>>();
        let error = program
            .instantiate(
                &[],
                TerminalStructuralInputs {
                    arguments: &views,
                    placed_view_establishments: &establishments,
                    ..Default::default()
                },
            )
            .err()
            .expect("exclusive backing alias rejects");
        assert!(
            format!("{error:?}").contains("PlacedViewInputEstablishmentAliasing"),
            "{error:?}"
        );
        eprintln!("{machine}: {error:?}");
    }
}

#[test]
fn mixed_parameters_rejoin_checked_dense_coordinate() {
    let artifact = produce("mixed");
    let program = TerminalProgram::borrow(artifact.view());
    let entry = program.entry().unwrap();
    let row = entry.placed_view_inputs().next().unwrap();
    assert_eq!((row.position, row.runtime_position), (2, 1));
    let views = entry
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(position, formal)| {
            TerminalStructuralValue::minted(
                9910 + position as u64,
                formal.structural_type,
                Vec::new(),
            )
        })
        .collect::<Vec<_>>();
    let plain_type = artifact
        .semantic_module()
        .structural_types
        .iter()
        .find(|item| item.id == views[0].structural_type)
        .unwrap();
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &plain_type.shape else {
        panic!("plain record");
    };
    let field = fields[0].id;
    let fields = [terminal_interpreter::TerminalStructuralScalarFieldValue {
        argument_index: 0,
        path: Vec::new(),
        field,
        value: scalar(12, 32),
    }];
    let establishment = TerminalPlacedViewEstablishment {
        input: row.clone(),
        referent: TerminalStructuralValue::minted(88001, views[1].structural_type, Vec::new()),
    };
    let registers = Arc::new(Mutex::new(Registers {
        status: 41,
        transmit: 0,
        events: Vec::new(),
    }));
    let mut instance = program
        .instantiate(
            &[scalar(7, 32)],
            TerminalStructuralInputs {
                arguments: &views,
                scalar_fields: &fields,
                placed_view_establishments: &[establishment],
                ..Default::default()
            },
        )
        .unwrap();
    for boundary in instance.unbound_imports().collect::<Vec<_>>() {
        let declaration = artifact
            .semantic_module()
            .boundary_machines
            .iter()
            .find(|row| row.id == boundary)
            .unwrap();
        let write = artifact
            .semantic_module()
            .identities
            .text(declaration.identity)
            .unwrap()
            .ends_with("::write");
        instance
            .bind_import(
                boundary,
                RegisterAdapter {
                    signature: declaration.into(),
                    registers: vec![(views[1].opaque_identity, Arc::clone(&registers))],
                    write,
                },
            )
            .unwrap();
    }
    let actual = instance
        .invoke(&mut TerminalFuelMeter::unbounded())
        .unwrap();
    assert_eq!(
        actual,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(scalar(60, 64)))
    );
    assert_eq!(registers.lock().unwrap().transmit, 9);
    eprintln!("mixed: {actual:?}; source coordinate 2, runtime coordinate 1");
    instance.close().unwrap();
}

#[test]
fn unit_helper_preserves_mutable_custody() {
    qualify_helper("unit_delegate", true);
}
