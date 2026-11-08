//! Recover a coupled component through the same canonical artifact and native
//! admission owners. This envelope contains no serialized validation wrapper.
use super::*;
use semantic_vocabulary::PackageKeyIdentity;

const MAGIC: &[u8; 8] = b"OMGCMP01";
const MAX_BYTES: usize = 128 * 1024 * 1024;

impl IndependentComponentProduct {
    pub fn encode_retained(&self) -> Result<Option<Vec<u8>>, String> {
        let Some(inputs) = &self.native_derivation_inputs else {
            return Ok(None);
        };
        let inputs = inputs.as_ref().map_err(Clone::clone)?;
        let description = self.description.description();
        let size = MAGIC.len() + 32 + 16;
        let size = size
            .checked_add(description.len())
            .and_then(|size| size.checked_add(inputs.len()))
            .filter(|size| *size <= MAX_BYTES)
            .ok_or("component retention record exceeds its byte bound")?;
        let mut bytes = Vec::with_capacity(size);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&self.description.package().digest());
        bytes.extend_from_slice(&(description.len() as u64).to_le_bytes());
        bytes.extend_from_slice(description);
        bytes.extend_from_slice(&(inputs.len() as u64).to_le_bytes());
        bytes.extend_from_slice(inputs);
        Ok(Some(bytes))
    }

    pub fn recover_retained(package: PackageKeyIdentity, bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err("component retention record exceeds its byte bound".to_owned());
        }
        let mut reader = Reader { bytes, position: 0 };
        if reader.take(MAGIC.len())? != MAGIC || reader.take(32)? != package.digest() {
            return Err("component retention schema or package identity differs".to_owned());
        }
        let description_bytes = reader.section()?;
        let native_inputs = reader.section()?;
        if reader.position != bytes.len() {
            return Err("component retention has trailing bytes".to_owned());
        }
        let (_, artifact) =
            component_description::decode_component_description_with_artifact(description_bytes)
                .map_err(|reason| format!("retained component description: {reason:?}"))?;
        let description = IndependentComponentDescription::new(
            package,
            artifact.view().identity(),
            description_bytes.to_vec(),
        );
        let (retained, permissions) =
            terminal_psi_to_abstract_operations::recover_native_derivation_inputs(
                Arc::new(artifact),
                native_inputs,
            )
            .map_err(|diagnostics| {
                diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.message)
                    .collect::<Vec<_>>()
                    .join("; ")
            })?;
        Self::new(description, retained, permissions)
            .map_err(str::to_owned)
            .map(|product| product.with_native_derivation_inputs(Ok(native_inputs.to_vec())))
    }
}

struct Reader<'bytes> {
    bytes: &'bytes [u8],
    position: usize,
}
impl<'bytes> Reader<'bytes> {
    fn take(&mut self, length: usize) -> Result<&'bytes [u8], String> {
        let end = self
            .position
            .checked_add(length)
            .ok_or("component retention length overflow")?;
        let slice = self
            .bytes
            .get(self.position..end)
            .ok_or("truncated component retention record")?;
        self.position = end;
        Ok(slice)
    }
    fn section(&mut self) -> Result<&'bytes [u8], String> {
        let length = u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|reason| format!("component retention section width: {reason}"))?,
        );
        self.take(
            usize::try_from(length)
                .map_err(|reason| format!("component retention section length: {reason}"))?,
        )
    }
}
