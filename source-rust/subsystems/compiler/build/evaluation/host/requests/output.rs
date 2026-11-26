//! Output retained by one Build across dependency waits and released on normal return.
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};

#[derive(Default)]
pub(crate) struct BuildRequestOutput {
    pub(crate) log: BuildLog,
    stream_bytes: u64,
}

impl BuildRequestOutput {
    pub(super) fn write_stream(
        &mut self,
        bytes: &[u8],
        limit: u64,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<(), String> {
        let consumed = self
            .stream_bytes
            .checked_add(u64::try_from(bytes.len()).map_err(|_| "Build stream size overflowed")?)
            .filter(|total| *total <= limit)
            .ok_or("Build diagnostic sink byte limit exceeded")?;
        self.log.append(bytes, false, account)?;
        self.stream_bytes = consumed;
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct BuildLog {
    bytes: Vec<u8>,
    storage: Vec<EvaluationStorageLease>,
}

impl BuildLog {
    pub(super) fn append(
        &mut self,
        bytes: &[u8],
        newline: bool,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<(), String> {
        let appended = bytes
            .len()
            .checked_add(usize::from(newline))
            .ok_or("Build log size overflowed")?;
        let count = u64::try_from(appended).map_err(|_| "Build log size overflowed")?;
        if appended == 0 {
            return Ok(());
        }
        let storage = account
            .map(|account| {
                let storage = account.reserve_site(0, 1, count)?;
                account.charge_build_log_bytes(count)?;
                Ok::<_, evaluation_contracts::EvaluationAccountingError>(storage)
            })
            .transpose()
            .map_err(|error| format!("Build log accounting: {error}"))?;
        self.bytes
            .try_reserve(appended)
            .map_err(|_| "Build log allocation was refused")?;
        if let Some(storage) = storage {
            self.storage
                .try_reserve(1)
                .map_err(|_| "Build log reservation allocation was refused")?;
            self.storage.push(storage);
        }
        self.bytes.extend_from_slice(bytes);
        if newline {
            self.bytes.push(b'\n');
        }
        Ok(())
    }

    pub(crate) fn publish(self, account: &EvaluationInvocationAccount) -> Result<Vec<u8>, String> {
        let bytes = u64::try_from(self.bytes.len()).map_err(|_| "Build log size overflowed")?;
        account
            .charge_result_custody(u64::from(bytes != 0), bytes)
            .map_err(|error| error.to_string())?;
        Ok(self.bytes)
    }
}
