#[path = "terminal_cleanup_resume.rs"]
mod cleanup_resume;
#[path = "terminal_extent_view.rs"]
mod extent_view;
#[path = "terminal_mutable_boundary.rs"]
mod mutable_boundary;

/// Portable provisions supply semantic test sinks, never native host services.
pub(super) fn observe_portable(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    words: &[&str],
) -> Result<(), String> {
    match words {
        ["cleanup-resume"] => cleanup_resume::observe(artifact),
        ["mutable-boundary"] => mutable_boundary::observe(artifact),
        ["external-extent-view"] => extent_view::observe(artifact, false),
        ["external-extent-view-write"] => extent_view::observe(artifact, true),
        _ => Err("unknown portable Terminal fixture provision".to_owned()),
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux {
    //! Explicit trusted corpus provisions for actual libc carrier exchange and owned entry arrivals.
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use terminal_codec::CanonicalTerminalArtifact;
    use terminal_fuel::TerminalFuelMeter;
    use terminal_interpreter::{
        TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
        TerminalExecutionResult, TerminalExecutionStatus, TerminalImportAdapter,
        TerminalImportSignature, TerminalProgram, TerminalScalarValue, TerminalStructuralInputs,
        TerminalStructuralValue,
    };
    use terminal_psi::BoundaryMachineResult;

    use runtime_abi::HostInternetAddress;
    #[derive(Default)]
    struct Values {
        next: u64,
        addresses: BTreeMap<u64, HostInternetAddress>,
        observed: Vec<u32>,
    }
    struct Adapter {
        signature: TerminalImportSignature,
        values: Arc<Mutex<Values>>,
    }
    impl TerminalImportAdapter for Adapter {
        fn implemented_signature(&self) -> TerminalImportSignature {
            self.signature.clone()
        }
    }
    impl TerminalEffectHandler for Adapter {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Err(TerminalEffectRejection::new("typed result required"))
        }
        fn handle_effect_result(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall {
                arguments,
                structural_arguments,
                result,
                ..
            } = effect
            else {
                return Err(TerminalEffectRejection::new("unexpected effect"));
            };
            let mut values = self.values.lock().unwrap();
            match result {
                BoundaryMachineResult::Structural(result) => {
                    let [
                        TerminalScalarValue::Integer {
                            value: IntegerValue::Unsigned(network),
                            ..
                        },
                        TerminalScalarValue::Integer {
                            value: IntegerValue::Unsigned(host),
                            ..
                        },
                    ] = arguments.as_slice()
                    else {
                        return Err(TerminalEffectRejection::new(
                            "make requires network and host",
                        ));
                    };
                    if !structural_arguments.is_empty() {
                        return Err(TerminalEffectRejection::new("unexpected make carrier"));
                    }
                    // The declared C function returns its actual value. Nothing in
                    // the adapter knows the guest's expected host-number oracle.
                    let address = HostInternetAddress::make(
                        u32::try_from(*network).unwrap(),
                        u32::try_from(*host).unwrap(),
                    )
                    .expect("Linux supplies native inet operations");
                    values.next += 1;
                    let identity = 90_000 + values.next;
                    values.addresses.insert(identity, address);
                    Ok(TerminalEffectResult::Structural(
                        TerminalStructuralValue::minted(
                            identity,
                            result.structural_type,
                            result.qualifications.clone(),
                        ),
                    ))
                }
                BoundaryMachineResult::Scalar(_) => {
                    let [address] = structural_arguments.as_slice() else {
                        return Err(TerminalEffectRejection::new(
                            "host requires one opaque carrier",
                        ));
                    };
                    if !arguments.is_empty() {
                        return Err(TerminalEffectRejection::new("unexpected host scalar"));
                    }
                    let address = values
                        .addresses
                        .remove(&address.opaque_identity)
                        .ok_or_else(|| {
                            TerminalEffectRejection::new("guest lost the original opaque value")
                        })?;
                    let host = address
                        .host_number()
                        .expect("Linux supplies native inet operations");
                    values.observed.push(host);
                    Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                        value: IntegerValue::Unsigned(host.into()),
                    }))
                }
                _ => Err(TerminalEffectRejection::new("unexpected result")),
            }
        }
    }

    fn exchange(artifact: &CanonicalTerminalArtifact, expected: &[u32]) {
        let program = TerminalProgram::borrow(artifact.view());
        let mut instance = program
            .instantiate(&[], TerminalStructuralInputs::default())
            .unwrap();
        let values = Arc::new(Mutex::new(Values::default()));
        for boundary in instance.unbound_imports().collect::<Vec<_>>() {
            let declaration = artifact
                .semantic_module()
                .boundary_machines
                .iter()
                .find(|row| row.id == boundary)
                .unwrap();
            let mut incompatible: TerminalImportSignature = declaration.into();
            incompatible
                .scalar_parameters
                .push(semantic_vocabulary::ScalarType::Boolean);
            assert!(
                instance
                    .bind_import(
                        boundary,
                        Adapter {
                            signature: incompatible,
                            values: Arc::clone(&values),
                        }
                    )
                    .is_err(),
                "an incompatible adapter must reject before an effect"
            );
            assert!(values.lock().unwrap().addresses.is_empty());
            instance
                .bind_import(
                    boundary,
                    Adapter {
                        signature: declaration.into(),
                        values: Arc::clone(&values),
                    },
                )
                .unwrap();
        }
        let actual = instance
            .invoke(&mut TerminalFuelMeter::unbounded())
            .unwrap();
        assert_eq!(
            actual,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                TerminalScalarValue::Boolean(true)
            ))
        );
        let values = values.lock().unwrap();
        assert_eq!(values.observed, expected);
        assert!(
            values.addresses.is_empty(),
            "both affine occurrences consumed"
        );
        eprintln!("Terminal libc observations: {:?}", values.observed);
        instance.close().unwrap();
    }
    fn opaque_entry(artifact: &CanonicalTerminalArtifact) {
        let program = TerminalProgram::borrow(artifact.view());
        let [parameter] = program.entry().unwrap().structural_parameters else {
            panic!("one opaque ingress")
        };
        let original = HostInternetAddress::make(10, 66051).unwrap();
        let argument =
            TerminalStructuralValue::minted(90001, parameter.structural_type, Vec::new());
        let mut instance = program
            .instantiate(
                &[],
                TerminalStructuralInputs {
                    arguments: std::slice::from_ref(&argument),
                    ..Default::default()
                },
            )
            .unwrap();
        let status = instance
            .invoke(&mut TerminalFuelMeter::unbounded())
            .unwrap();
        let TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(returned)) =
            status
        else {
            panic!("opaque returned value")
        };
        assert_eq!(
            returned.value, argument,
            "whole host payload survives changed storage homes"
        );
        assert!(returned.claims.is_empty());
        assert_eq!(original.host_number(), Some(66051));
        instance.close().unwrap();
    }

    fn materialized_entry(artifact: &CanonicalTerminalArtifact, relocated: bool) {
        let program = TerminalProgram::borrow(artifact.view());
        let [parameter] = program.entry().unwrap().structural_parameters else {
            panic!("one materialized input")
        };
        let declaration = artifact
            .semantic_module()
            .structural_types
            .iter()
            .find(|item| item.id == parameter.structural_type)
            .unwrap();
        let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
            panic!("plain record")
        };
        let argument =
            TerminalStructuralValue::minted(90001, parameter.structural_type, Vec::new());
        let scalar_fields = [terminal_interpreter::TerminalStructuralScalarFieldValue {
            argument_index: 0,
            path: Vec::new(),
            field: fields[0].id,
            value: TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                value: IntegerValue::Unsigned(1),
            },
        }];
        let mut instance = program
            .instantiate(
                &[],
                TerminalStructuralInputs {
                    arguments: std::slice::from_ref(&argument),
                    scalar_fields: &scalar_fields,
                    ..Default::default()
                },
            )
            .unwrap();
        let TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(returned)) =
            instance
                .invoke(&mut TerminalFuelMeter::unbounded())
                .unwrap()
        else {
            panic!("completed materialized result")
        };
        if relocated {
            assert_ne!(
                returned.value.opaque_identity, argument.opaque_identity,
                "materialized contents retain their relocated guest backing, not an abstract host token"
            );
        }
        instance.close().unwrap();
    }

    /// Fixture oracle words select a closed trusted host provision. They never
    /// synthesize a result or change the artifact's import signatures/authority.
    pub(super) fn observe(
        artifact: &CanonicalTerminalArtifact,
        words: &[&str],
    ) -> Result<(), String> {
        // Harness assertion failures become a red fixture outcome with a diagnostic,
        // rather than terminating the corpus before its other authored controls.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match words {
            ["libc-inet", numbers] => {
                let expected = numbers
                    .split(',')
                    .map(str::parse::<u32>)
                    .collect::<Result<Vec<_>, _>>()
                    .expect("libc-inet needs comma-separated u32 observations");
                exchange(artifact, &expected);
            }
            ["opaque-entry"] => opaque_entry(artifact),
            ["materialized-entry"] => materialized_entry(artifact, false),
            ["relocated-materialized-entry"] => materialized_entry(artifact, true),
            _ => panic!("unknown Terminal fixture provision"),
        }));
        result.map_err(|failure| {
            failure
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| failure.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                .unwrap_or_else(|| "Terminal provision failed".to_owned())
        })
    }
}

pub(super) fn observe(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    words: &[&str],
) -> Result<(), String> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return linux::observe(artifact, words);
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (artifact, words);
        Err("libc-inet and owned entry corpus provisions currently require Linux x86_64".to_owned())
    }
}
