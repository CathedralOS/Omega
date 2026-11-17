//! Completed compiler-owned images. File publication may only copy these bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCompanionImage {
    filename: &'static str,
    contents: Vec<u8>,
}
impl NativeCompanionImage {
    pub fn filename(&self) -> &str { self.filename }
    pub fn contents(&self) -> &[u8] { &self.contents }
}

pub(crate) fn emit(object: &crate::ObjectArtifact) -> Vec<NativeCompanionImage> {
    object.windows_internet.as_ref().map(|code| NativeCompanionImage {
        filename: "omega_host.dll",
        contents: image_pe::emit_windows_internet_provider(code),
    }).into_iter().collect()
}
