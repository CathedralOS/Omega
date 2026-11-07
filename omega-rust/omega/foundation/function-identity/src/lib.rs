/*
Compiler-private callback function identities (`MachineFunctionIdentity`)
scoped to their owning Terminal Psi program.

Callback placements name call-site occurrences inside a specific Terminal Psi
program rather than source declarations or machine addresses. Pairing the
caller's `TerminalPsiIdentity` (2-byte little-endian `VocabularyMarker` +
32-byte `SemanticFingerprint`) with the 8-byte little-endian `placement_index`
produces a fixed 42-byte identity (`to_bytes` / `from_bytes`) and deterministic
private linker symbol (`__omega_callback_v<vocab>_<fingerprint>_p<index>`) so
identical local placement indices never alias across independently compiled
programs.

An all-zero program fingerprint (`MachineFunctionIdentity::default()`) marks an
absent identity and is rejected by `callback_thunk`, `from_bytes`, and
`private_symbol`.
*/

#![forbid(unsafe_code)]

pub(crate) use optimization_core::codec_bytes;

use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MachineFunctionIdentity {
    caller: TerminalPsiIdentity,
    placement_index: usize,
}

impl MachineFunctionIdentity {
    pub fn callback_thunk(caller: TerminalPsiIdentity, placement_index: usize) -> Option<Self> {
        let identity = Self {
            caller,
            placement_index,
        };
        identity.is_valid().then_some(identity)
    }

    pub const fn caller(self) -> TerminalPsiIdentity {
        self.caller
    }

    pub fn callback_thunk_placement_index(self) -> Option<usize> {
        self.is_valid().then_some(self.placement_index)
    }

    pub fn is_valid(self) -> bool {
        self.caller.program_fingerprint.as_bytes() != &[0; 32]
    }

    pub fn to_bytes(self) -> [u8; 42] {
        let mut bytes = [0; 42];
        crate::codec_bytes::patch_encoded(
            &mut bytes,
            0,
            self.caller.vocabulary_marker.get().to_le_bytes(),
        );
        bytes[2..34].copy_from_slice(self.caller.program_fingerprint.as_bytes());
        crate::codec_bytes::patch_encoded(
            &mut bytes,
            34,
            (self.placement_index as u64).to_le_bytes(),
        );
        bytes
    }

    pub fn from_bytes(bytes: [u8; 42]) -> Option<Self> {
        let vocabulary_marker = VocabularyMarker::new(u16::from_le_bytes([bytes[0], bytes[1]]))?;
        let mut fingerprint = [0; 32];
        fingerprint.copy_from_slice(&bytes[2..34]);
        let mut placement = [0; 8];
        placement.copy_from_slice(&bytes[34..]);
        Self::callback_thunk(
            TerminalPsiIdentity {
                vocabulary_marker,
                program_fingerprint: SemanticFingerprint::from_bytes(fingerprint),
            },
            usize::try_from(u64::from_le_bytes(placement)).ok()?,
        )
    }

    pub fn private_symbol(self) -> Option<String> {
        self.is_valid().then(|| {
            format!(
                "__omega_callback_v{}_{}_p{}",
                self.caller.vocabulary_marker.get(),
                self.caller.program_fingerprint,
                self.placement_index,
            )
        })
    }
}

impl Default for MachineFunctionIdentity {
    fn default() -> Self {
        Self {
            caller: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([0; 32]),
            },
            placement_index: 0,
        }
    }
}

#[cfg(test)]
mod tests;
