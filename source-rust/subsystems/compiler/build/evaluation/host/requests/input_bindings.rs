//! Authenticate PackageBinding values against the immutable inputs of one Build.
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
use package_compilation::source_imports::SuppliedBuildLibrary;
use semantic_vocabulary::StructuralTypeId;
use std::sync::{Arc, Mutex};
use terminal_interpreter::{TerminalEffectRejection, TerminalStructuralValue};

/// Selects this exact admitted library, including its frozen source and Product dependencies.
#[derive(Clone)]
pub struct FrozenBuildBinding {
    libraries: Arc<[SuppliedBuildLibrary]>,
    ordinal: usize,
}

impl FrozenBuildBinding {
    pub(crate) fn new(libraries: Arc<[SuppliedBuildLibrary]>, ordinal: usize) -> Self {
        Self { libraries, ordinal }
    }

    pub fn library(&self) -> &SuppliedBuildLibrary {
        &self.libraries[self.ordinal]
    }
}

struct IssuedBinding {
    marker: TerminalStructuralValue,
    binding: FrozenBuildBinding,
    _storage: Option<EvaluationStorageLease>,
}

pub(super) struct InputBindings {
    libraries: Arc<[SuppliedBuildLibrary]>,
    issued: Mutex<Vec<IssuedBinding>>,
}

impl InputBindings {
    pub fn new(libraries: Arc<[SuppliedBuildLibrary]>) -> Self {
        Self {
            libraries,
            issued: Mutex::default(),
        }
    }

    pub fn select(
        &self,
        alias: &str,
        result: StructuralTypeId,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<TerminalStructuralValue, TerminalEffectRejection> {
        let ordinal = self
            .libraries
            .iter()
            .position(|library| library.dependency().alias == alias)
            .ok_or_else(|| {
                TerminalEffectRejection::new(format!(
                    "Build has no incoming package binding `{alias}`"
                ))
            })?;
        let mut issued = self
            .issued
            .lock()
            .map_err(|_| TerminalEffectRejection::new("Build input bindings lock was poisoned"))?;
        if let Some(row) = issued.iter().find(|row| row.binding.ordinal == ordinal) {
            return Ok(row.marker.clone());
        }
        let identity = u64::try_from(issued.len())
            .ok()
            .and_then(|index| (super::FIRST_REGISTRATION_IDENTITY + 1).checked_add(index))
            .ok_or_else(|| TerminalEffectRejection::new("Build input binding markers exhausted"))?;
        let marker = TerminalStructuralValue::minted(identity, result, Vec::new());
        let storage = account
            .map(|account| account.reserve_site(0, 1, 0))
            .transpose()
            .map_err(|error| {
                TerminalEffectRejection::new(format!("Build input binding storage: {error:?}"))
            })?;
        issued
            .try_reserve(1)
            .map_err(|_| TerminalEffectRejection::new("cannot retain Build input binding"))?;
        issued.push(IssuedBinding {
            marker: marker.clone(),
            binding: FrozenBuildBinding::new(Arc::clone(&self.libraries), ordinal),
            _storage: storage,
        });
        Ok(marker)
    }

    pub fn require(
        &self,
        marker: &TerminalStructuralValue,
    ) -> Result<FrozenBuildBinding, TerminalEffectRejection> {
        let issued = self
            .issued
            .lock()
            .map_err(|_| TerminalEffectRejection::new("Build input bindings lock was poisoned"))?;
        marker
            .opaque_identity
            .checked_sub(super::FIRST_REGISTRATION_IDENTITY + 1)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| issued.get(index))
            .filter(|row| &row.marker == marker)
            .map(|row| row.binding.clone())
            .ok_or_else(|| {
                TerminalEffectRejection::new(
                    "Build dependency registration requires a PackageBinding this activation issued",
                )
            })
    }
}
