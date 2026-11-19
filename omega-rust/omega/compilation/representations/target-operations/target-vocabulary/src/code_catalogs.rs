//! Callable and implementation contracts retained by native publication.
//!
//! These are the existing semantic records, not executable bodies. Their
//! canonical wire bytes are prepared once by the wire owner and shared with
//! the records, so artifact writing cannot introduce a second catalog format.

use std::sync::Arc;

use terminal_psi::{TerminalCallableCatalog, TerminalDynamicDispatchCatalog};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCodeCatalogs(Arc<CatalogRows>);

#[derive(Debug, PartialEq, Eq)]
struct CatalogRows {
    dynamic_dispatch: TerminalDynamicDispatchCatalog,
    callables: TerminalCallableCatalog,
    dynamic_bytes: Vec<u8>,
    callable_bytes: Vec<u8>,
}

impl NativeCodeCatalogs {
    /// Freeze the existing records and their canonical publication bytes.
    /// Encoding checks scalar closure; it does not establish callable authority
    /// or replace source correspondence and artifact contract validation.
    pub fn seal(
        dynamic_dispatch: TerminalDynamicDispatchCatalog,
        callables: TerminalCallableCatalog,
    ) -> Result<Self, terminal_wire::CodecError> {
        let dynamic_bytes = terminal_wire::encode_dynamic_dispatch_catalog(&dynamic_dispatch)?;
        let callable_bytes = terminal_wire::encode_callable_catalog(&callables)?;
        Ok(Self(Arc::new(CatalogRows {
            dynamic_dispatch,
            callables,
            dynamic_bytes,
            callable_bytes,
        })))
    }

    pub fn dynamic_dispatch(&self) -> &TerminalDynamicDispatchCatalog {
        &self.0.dynamic_dispatch
    }

    pub fn callables(&self) -> &TerminalCallableCatalog {
        &self.0.callables
    }

    pub(super) fn encode(&self, mut write: impl FnMut(&[u8])) {
        for bytes in [&self.0.dynamic_bytes, &self.0.callable_bytes] {
            write(&(bytes.len() as u64).to_le_bytes());
            write(bytes);
        }
    }
}

impl Default for NativeCodeCatalogs {
    fn default() -> Self {
        Self::seal(Default::default(), Default::default())
            .expect("empty semantic catalogs have a canonical encoding")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{ContractId, ServiceId};
    use terminal_psi::{
        TerminalCallableEnvelope, TerminalCallableTermination, TerminalDeclarationEnvelope,
    };

    #[test]
    fn catalogs_share_records_and_the_existing_complete_encoding() {
        let callables = TerminalCallableCatalog {
            declaration_envelopes: vec![TerminalDeclarationEnvelope {
                contract: ContractId::new(9).unwrap(),
                envelope: TerminalCallableEnvelope {
                    service_reach: vec![ServiceId::new(4).unwrap()],
                    invocations: Vec::new(),
                    suspends: true,
                    blocks: false,
                    termination: TerminalCallableTermination::NotGuaranteed,
                },
            }],
            ..Default::default()
        };
        let catalogs = NativeCodeCatalogs::seal(Default::default(), callables.clone()).unwrap();
        let copy = catalogs.clone();
        assert!(Arc::ptr_eq(&catalogs.0, &copy.0));
        assert_eq!(catalogs.callables(), &callables);
        assert_eq!(
            catalogs.0.callable_bytes,
            terminal_wire::encode_callable_catalog(&callables).unwrap()
        );
        let mut bytes = Vec::new();
        catalogs.encode(|part| bytes.extend_from_slice(part));
        assert!(bytes.ends_with(&catalogs.0.callable_bytes));
    }
}
