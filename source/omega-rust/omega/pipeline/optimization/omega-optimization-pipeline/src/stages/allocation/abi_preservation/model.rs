use omega_register_model::PreservationConvention;

/// Exact target-owned preservation row selected for planning facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameAbiPreservationConvention {
    SystemVAMD64,
    MicrosoftX64,
    Aapcs64,
    DarwinAapcs64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SelectedAbiPreservation<'model> {
    pub(crate) kind: FrameAbiPreservationConvention,
    pub(crate) convention: &'model PreservationConvention,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbiPreservationSelectionError {
    UnsupportedTargetConvention,
}
