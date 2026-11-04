//! Read-only views of this invocation's already captured native input graph.
//! Domain adapters can inspect complete records and follow authenticated typed
//! references; they cannot invent endpoints, storage or Source write access.

use super::reference_outputs::ReferenceOutputs;
use super::{NativeContext, StorageCodec, rejection};
use semantic_vocabulary::CanonicalStructuralPathSegment as Path;
use terminal_interpreter::{TerminalEffectRejection, TerminalOriginalStorageLoan};

pub(in crate::execution::host_imports) struct ReadonlyRecord<'a> {
    native: &'a NativeContext,
    references: &'a ReferenceOutputs,
    argument: usize,
    path: Vec<Path>,
    loan: &'a TerminalOriginalStorageLoan,
    codec: &'a StorageCodec,
    bytes: Vec<u8>,
}

impl<'a> ReadonlyRecord<'a> {
    pub(super) fn new(
        native: &'a NativeContext,
        references: &'a ReferenceOutputs,
        argument: usize,
        loan: &'a TerminalOriginalStorageLoan,
        codec: &'a StorageCodec,
    ) -> Result<Self, TerminalEffectRejection> {
        let (home, offset) = native.loan(loan, codec)?;
        let mut bytes = vec![0; codec.extent];
        native
            .sponsor
            .borrow_mut()
            .with(|process| process.read_home(home, offset, &mut bytes))?;
        Ok(Self {
            native,
            references,
            argument,
            path: Vec::new(),
            loan,
            codec,
            bytes,
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn sole_reference(&self) -> Result<Self, TerminalEffectRejection> {
        let [field] = self.codec.references.as_slice() else {
            return Err(rejection(
                "native input changed its complete single-reference roster",
            ));
        };
        let capture = self.references.input_reference(
            self.native,
            self.argument,
            &self.path,
            self.loan,
            field,
        )?;
        Ok(Self {
            native: self.native,
            references: self.references,
            argument: self.argument,
            path: capture.path,
            loan: capture.loan,
            codec: capture.codec,
            bytes: capture.bytes,
        })
    }
}
