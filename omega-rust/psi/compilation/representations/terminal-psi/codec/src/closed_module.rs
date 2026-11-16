//! A closed module keeps its scalar descriptors, canonical execution bytes
//! and their semantic identity together. Closure does not show that a
//! checker accepted the program or that its producer can be trusted. The
//! encoding and the closed type catalog belong to `terminal_wire`; this crate
//! owns only the seal that keeps them beside the module.

use std::sync::Arc;

use terminal_psi::{TerminalModule, TerminalPsiIdentity};

use crate::{ClosedTypeCatalog, CodecError, EvidenceSeals, ObligationEvidenceSeal};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedTerminalModule {
    module: Arc<TerminalModule>,
    types: ClosedTypeCatalog,
    bytes: Arc<[u8]>,
    identity: TerminalPsiIdentity,
}

impl ClosedTerminalModule {
    pub fn new(module: TerminalModule) -> Result<Self, CodecError> {
        let (bytes, types) = terminal_wire::sealing::encode_closed_execution_module(&module)?;
        Ok(Self::from_encoding(module, bytes, types))
    }

    pub fn from_semantic_bytes(bytes: &[u8]) -> Result<Self, CodecError> {
        let (module, bytes, types) = terminal_wire::sealing::decode_closed_parts(bytes)?;
        Ok(Self::from_encoding(module, bytes, types))
    }

    fn from_encoding(module: TerminalModule, bytes: Vec<u8>, types: ClosedTypeCatalog) -> Self {
        let identity = TerminalPsiIdentity {
            vocabulary_marker: module.vocabulary_marker,
            program_fingerprint: terminal_wire::sealing::encoded_fingerprint(&module, &bytes),
        };
        Self {
            module: Arc::new(module),
            types,
            bytes: bytes.into(),
            identity,
        }
    }

    pub fn module(&self) -> &TerminalModule {
        &self.module
    }

    pub const fn closed_types(&self) -> &ClosedTypeCatalog {
        &self.types
    }

    pub fn semantic_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn identity(&self) -> TerminalPsiIdentity {
        self.identity
    }

    /// Binds evidence to this exact closed module without encoding or
    /// closing it again. The owner retains the bytes behind this identity.
    pub fn evidence_seals(&self, seals: Vec<ObligationEvidenceSeal>) -> EvidenceSeals {
        EvidenceSeals {
            module: self.identity,
            seals,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, MachineId, ScalarType, StructuralTypeId, ValueId, WidthParameter,
    };
    use terminal_psi::{
        StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityTableBuilder,
    };

    fn parameterized() -> IntegerType {
        IntegerType::parameterized(
            IntegerSign::Unsigned,
            WidthParameter {
                owner: MachineId::new(1).unwrap(),
                value: ValueId::new(3).unwrap(),
            },
        )
    }

    fn module(integer: IntegerType) -> TerminalModule {
        let mut identities = TerminalIdentityTableBuilder::default();
        let identity = identities.intern("closed_width_test");
        let mut module = TerminalModule::declarations();
        module.identities = identities.build();
        module.structural_types.push(StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            quotient_carrier: None,
            id: StructuralTypeId::new(1).unwrap(),
            identity,
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)),
            scalar_restriction: None,
            requires: Vec::new(),
        });
        module
    }

    #[test]
    fn a_dormant_symbolic_type_is_refused_by_the_complete_encoder() {
        assert!(matches!(
            ClosedTerminalModule::new(module(parameterized())),
            Err(CodecError::IntegerWidthClosure(_))
        ));
    }

    #[test]
    fn closing_and_retaining_keep_the_original_module_storage_and_bytes() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let module = module(integer);
        let storage = module.structural_types.as_ptr();
        let closed = ClosedTerminalModule::new(module).unwrap();
        let retained = closed.clone();
        assert_eq!(closed.module().structural_types.as_ptr(), storage);
        assert_eq!(retained.module().structural_types.as_ptr(), storage);
        assert!(std::ptr::eq(
            closed.semantic_bytes(),
            retained.semantic_bytes()
        ));
        assert_eq!(closed.identity(), retained.identity());
        assert_eq!(
            closed
                .closed_types()
                .scalar(ScalarType::Integer(integer))
                .unwrap()
                .as_checking(),
            ScalarType::Integer(integer)
        );
        assert!(
            closed
                .closed_types()
                .integer(IntegerType::new(IntegerSign::Unsigned, 13).unwrap())
                .is_err(),
            "an independently closed foreign input is not a retained module leaf"
        );
    }

    #[test]
    fn closed_module_evidence_preserves_the_original_identity_and_seals() {
        let module = module(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
        let original = EvidenceSeals::for_module(&module, Vec::new()).unwrap();
        let closed = ClosedTerminalModule::new(module).unwrap();
        let evidence = closed.evidence_seals(original.seals.clone());
        assert_eq!(evidence.module, closed.identity());
        assert_eq!(evidence, original);
    }
}
