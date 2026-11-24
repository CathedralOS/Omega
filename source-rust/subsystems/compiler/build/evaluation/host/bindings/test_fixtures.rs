//! The registering Build lends a group its fixture settings. Capture bytes
//! while the supplied view is live, then publish the owned selections on return.
use super::*;
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
use source_compiler::registered_tests::{
    TestExpectations, TestFileExpectation, TestInvocationInputs,
};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct TestFixture {
    stdin: Option<Retained<Arc<[u8]>>>,
    stdout: Option<Retained<Arc<[u8]>>>,
    stderr: Option<Retained<Arc<[u8]>>>,
    initial_files: BTreeMap<Vec<u8>, Retained<Arc<[u8]>>>,
    expected_files: BTreeMap<Vec<u8>, Retained<TestFileExpectation>>,
}

struct Retained<T> {
    value: T,
    size: u64,
    _storage: Option<EvaluationStorageLease>,
}

impl<T> Retained<T> {
    fn publish(self, account: Option<&EvaluationInvocationAccount>) -> Result<T, String> {
        if let Some(account) = account {
            account
                .charge_result_custody(1, self.size)
                .map_err(|e| e.to_string())?;
        }
        Ok(self.value)
    }
}

impl TestFixture {
    pub(super) fn finish(
        self,
        private_filesystem: bool,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<(TestInvocationInputs, TestExpectations), String> {
        let stream = |v: Option<Retained<Arc<[u8]>>>| v.map(|v| v.publish(account)).transpose();
        let inputs = TestInvocationInputs {
            private_filesystem,
            stdin: stream(self.stdin)?,
            files: self
                .initial_files
                .into_iter()
                .map(|(path, file)| Ok((path, file.publish(account)?)))
                .collect::<Result<_, String>>()?,
        };
        let expected = TestExpectations {
            stdout: stream(self.stdout)?,
            stderr: stream(self.stderr)?,
            files: self
                .expected_files
                .into_iter()
                .map(|(path, file)| Ok((path, file.publish(account)?)))
                .collect::<Result<_, String>>()?,
        };
        Ok((inputs, expected))
    }
}

impl TerminalBuildBindingAdapter {
    pub(super) fn configure_test_fixture(
        &mut self,
        group: usize,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let refuse = TerminalEffectRejection::new;
        let TerminalEffect::BoundaryCall {
            arguments,
            byte_sequence_arguments,
            result,
            ..
        } = effect
        else {
            return Err(refuse(
                "test fixture configuration requires a boundary call",
            ));
        };
        let [
            TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Unsigned(setting),
                ..
            },
        ] = arguments.as_slice()
        else {
            return Err(refuse("test fixture configuration lost its selector"));
        };
        let [None, Some(path), Some(bytes)] = byte_sequence_arguments.as_slice() else {
            return Err(refuse("test fixture configuration lost its byte views"));
        };
        if *result != terminal_psi::BoundaryMachineResult::Unit {
            return Err(refuse(
                "test fixture configuration must return to the Build",
            ));
        }
        if *setting <= 2 {
            if !path.is_empty() {
                return Err(refuse("a standard stream cannot name a file"));
            }
        } else if !filesystem_authority::canonical_filesystem_metadata_path_is_canonical(
            path, false,
        ) || path.len() >= 4096
            || path.split(|b| *b == b'/').any(|p| p.len() > 255)
        {
            return Err(refuse(
                "a test fixture path must be a canonical relative file name",
            ));
        }
        let size = (path.len() as u64)
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| refuse("test fixture is too large"))?;
        let lease = self
            .host
            .account
            .as_ref()
            .map(|account| account.reserve_storage(1, size))
            .transpose()
            .map_err(|e| TerminalEffectRejection::new(format!("test fixture storage: {e}")))?;
        let mut state = lock_state(&self.host.state).map_err(refuse)?;
        let Some(ActivationSelection::TestGroup(group)) = state.executed.get_mut(group) else {
            return Err(refuse(
                "test fixture configuration lost its registered group",
            ));
        };
        let fixture = &mut group.fixture;
        match setting {
            0..=3 => {
                let retained = Retained {
                    value: Arc::from(bytes.as_slice()),
                    size,
                    _storage: lease,
                };
                match setting {
                    0 => fixture.stdin = Some(retained),
                    1 => fixture.stdout = Some(retained),
                    2 => fixture.stderr = Some(retained),
                    _ => {
                        fixture.initial_files.insert(path.clone(), retained);
                    }
                }
            }
            4..=6 => {
                let value = match setting {
                    4 if bytes.is_empty() => TestFileExpectation::Observe,
                    5 if bytes.is_empty() => TestFileExpectation::Missing,
                    6 => TestFileExpectation::Bytes(Arc::from(bytes.as_slice())),
                    _ => return Err(refuse("only a file byte expectation accepts contents")),
                };
                fixture.expected_files.insert(
                    path.clone(),
                    Retained {
                        value,
                        size,
                        _storage: lease,
                    },
                );
            }
            _ => return Err(refuse("unknown test fixture setting")),
        }
        Ok(TerminalEffectResult::Unit)
    }
}
