//! The native facet delegates to the one source-custodied native choice codec.
use super::PackageBuildHandoff;
pub fn encode(bundle: &PackageBuildHandoff) -> Vec<u8> {
    super::native_selection_codec::encode_package_native_bindings(
        bundle.native_bindings(),
        bundle.native_evaluation().map(|value| value.as_ref()),
    )
}
pub fn decode(bytes: &[u8], bundle: PackageBuildHandoff) -> Option<PackageBuildHandoff> {
    let (rows, evaluation) = super::native_selection_codec::decode_package_native_bindings(bytes)?;
    bundle.with_checked_native_bindings(rows, evaluation).ok()
}
pub fn digest(bundle: &PackageBuildHandoff) -> [u8; 32] {
    super::native_selection_codec::package_native_bindings_digest(
        bundle.native_bindings(),
        bundle.native_evaluation().map(|value| value.as_ref()),
    )
}
