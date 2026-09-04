/// An inclusive scalar range established while decoding untrusted wire bytes.
///
/// The bounds are normalized compile-time integers. `signed` selects the
/// comparison algebra used after the wire scalar has been decoded at its
/// declared width. A missing range remains the ZII/no-extra-check case on the
/// wire-read operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireScalarRange {
    pub minimum: i64,
    pub maximum: i64,
    pub signed: bool,
}
