//! A group's stream inputs and expectations belong to its registering Build.
//! Snapshot the evaluated bytes while their loan is live. Replacing a setting
//! releases its previous storage, including across a dependency wait.
use super::*;
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
use source_compiler::registered_tests::TestExpectations;
use terminal_interpreter::TerminalBoundaryStorageBuffer;

#[derive(Default)]
pub(super) struct TestStreams {
    stdin: Option<StreamBytes>,
    stdout: Option<StreamBytes>,
    stderr: Option<StreamBytes>,
}

struct StreamBytes {
    bytes: Arc<[u8]>,
    _storage: Option<EvaluationStorageLease>,
}

impl TestStreams {
    pub(super) fn finish(
        self,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<(Option<Arc<[u8]>>, TestExpectations), String> {
        let publish = |stream: Option<StreamBytes>| {
            stream
                .map(|stream| {
                    if let Some(account) = account {
                        account
                            .charge_result_custody(1, stream.bytes.len() as u64)
                            .map_err(|e| e.to_string())?;
                    }
                    Ok::<_, String>(stream.bytes)
                })
                .transpose()
        };
        Ok((
            publish(self.stdin)?,
            TestExpectations {
                stdout: publish(self.stdout)?,
                stderr: publish(self.stderr)?,
            },
        ))
    }
}

impl TerminalBuildBindingAdapter {
    pub(super) fn configure_test_stream(
        &mut self,
        effect: &TerminalEffect,
        storage: &[TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let refuse = TerminalEffectRejection::new;
        let group = self.take_presented_group(storage)?;
        let TerminalEffect::BoundaryCall {
            arguments,
            byte_sequence_arguments,
            result,
            ..
        } = effect
        else {
            return Err(refuse("test stream configuration requires a boundary call"));
        };
        let [
            TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Unsigned(channel),
                ..
            },
        ] = arguments.as_slice()
        else {
            return Err(refuse("test stream configuration lost its stream selector"));
        };
        let [None, Some(bytes)] = byte_sequence_arguments.as_slice() else {
            return Err(refuse("test stream configuration lost its byte view"));
        };
        if *result != terminal_psi::BoundaryMachineResult::Unit {
            return Err(refuse("test stream configuration must return to the Build"));
        }
        let mut state = lock_state(&self.host.state).map_err(refuse)?;
        let Some(ActivationSelection::TestGroup(group)) = state.executed.get_mut(group) else {
            return Err(refuse(
                "test stream configuration lost its registered group",
            ));
        };
        let slot = match channel {
            0 => &mut group.streams.stdin,
            1 => &mut group.streams.stdout,
            2 => &mut group.streams.stderr,
            _ => return Err(refuse("test stream configuration names an unknown stream")),
        };
        let storage = self
            .host
            .account
            .as_ref()
            .map(|account| account.reserve_storage(1, bytes.len() as u64))
            .transpose()
            .map_err(|e| TerminalEffectRejection::new(format!("test stream storage: {e}")))?;
        *slot = Some(StreamBytes {
            bytes: Arc::from(bytes.as_slice()),
            _storage: storage,
        });
        Ok(TerminalEffectResult::Unit)
    }
}
