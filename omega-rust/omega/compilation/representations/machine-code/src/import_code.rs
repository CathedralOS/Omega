//! Import bodies have already been encoded. Container placement may fill only
//! their declared operands; it cannot request another body from the encoder.

use std::sync::Arc;
use target::{NativeTarget, NormalizedImportLocator};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportCodeIdentity {
    Normalized(NormalizedImportLocator),
    StringBackedBootstrap { library: String, symbol: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportOperandKind {
    X86Relative32,
    X86SignedOrdinal32,
    Aarch64Page21,
    Aarch64PageOffset12 { scale: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportOperandTarget {
    BindingSlot,
    LoaderContext,
    LoaderResolver,
    ResolverEntry,
    BindingOrdinal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportOperand {
    pub offset: usize,
    pub kind: ImportOperandKind,
    pub target: ImportOperandTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportBodyResources {
    pub effects: calling_conventions::StateEffectsEvidence,
    /// Local stack consumed by this body, excluding the foreign loader/callee.
    pub local_stack_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedImportBody {
    pub resources: ImportBodyResources,
    pub bytes: Vec<u8>,
    pub operands: Vec<ImportOperand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedImport {
    pub identity: ImportCodeIdentity,
    pub body: EncodedImportBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedImportProgram {
    pub target: NativeTarget,
    pub resolver: Option<EncodedImportBody>,
    pub imports: Vec<EncodedImport>,
}

impl EncodedImportProgram {
    pub fn empty(target: NativeTarget) -> Self {
        Self {
            target,
            resolver: None,
            imports: Vec::new(),
        }
    }

    pub fn body(&self, identity: &ImportCodeIdentity) -> Option<&EncodedImportBody> {
        let mut matches = self
            .imports
            .iter()
            .filter(|entry| &entry.identity == identity);
        let body = &matches.next()?.body;
        matches.next().is_none().then_some(body)
    }

    /// Composition joins already encoded bodies by exact locator identity.
    pub fn join(&mut self, other: &Self) -> Result<(), &'static str> {
        if self.target != other.target {
            return Err("encoded import programs belong to different targets");
        }
        if self
            .resolver
            .as_ref()
            .zip(other.resolver.as_ref())
            .is_some_and(|(a, b)| a != b)
        {
            return Err("encoded import resolver bodies disagree");
        }
        for entry in &other.imports {
            if let Some(existing) = self
                .imports
                .iter()
                .find(|existing| existing.identity == entry.identity)
                && existing != entry
            {
                return Err("one import identity has different encoded bodies");
            }
        }
        if self.resolver.is_none() {
            self.resolver = other.resolver.clone();
        }
        for entry in &other.imports {
            if !self
                .imports
                .iter()
                .any(|existing| existing.identity == entry.identity)
            {
                self.imports.push(entry.clone());
            }
        }
        Ok(())
    }
}

/// Encoded imports and private function references carried with one machine
/// program through block placement, relaxation and object construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedBodyBindings {
    pub private_references: Arc<[function_identity::MachineFunctionIdentity]>,
    pub imports: Arc<EncodedImportProgram>,
    pub windows_internet: Option<Arc<crate::WindowsInternetProviderCode>>,
}
