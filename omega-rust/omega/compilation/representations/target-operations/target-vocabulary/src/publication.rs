//! Source identities and literal payloads needed by native publication.
//!
//! These rows contain no executable body. Target lowering records them while
//! it owns the current source function; later forms share the sealed roster
//! rather than reopening that function to recover an ordinal or quoted bytes.

use std::sync::Arc;

use native_vocabulary::provenance::PsiProvenance;
use optimization_core::digest_encoder::DomainDigestEncoder;
use semantic_vocabulary::{MachineId, OperationId, PlaceId};

pub const NATIVE_PUBLICATION_DOMAIN: &[u8] = b"omega.native-publication-facts.v1\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeByteLiteral {
    pub operation: OperationId,
    pub place: PlaceId,
    pub bytes: Arc<[u8]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFunctionPublication {
    pub machine: MachineId,
    pub operation_count: u32,
    /// Ordered by site, with one exact original operation ordinal per site.
    /// Several control edges can belong to the same source operation.
    pub sites: Vec<(PsiProvenance, u32)>,
    /// Ordered by operation. Empty literals still have an explicit row.
    pub byte_literals: Vec<NativeByteLiteral>,
}

impl NativeFunctionPublication {
    pub fn ordinal(&self, site: PsiProvenance) -> Option<u32> {
        self.sites
            .binary_search_by_key(&site, |row| row.0)
            .ok()
            .map(|position| self.sites[position].1)
    }

    pub fn byte_literal(&self, operation: OperationId) -> Option<&NativeByteLiteral> {
        self.byte_literals
            .binary_search_by_key(&operation, |row| row.operation)
            .ok()
            .map(|position| &self.byte_literals[position])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePublicationFacts {
    functions: Arc<[NativeFunctionPublication]>,
    identity: [u8; 32],
}

impl NativePublicationFacts {
    pub fn seal(functions: Vec<NativeFunctionPublication>) -> Result<Self, &'static str> {
        if functions
            .windows(2)
            .any(|rows| rows[0].machine >= rows[1].machine)
        {
            return Err("publication functions are unordered or repeated");
        }
        for function in &functions {
            if function.sites.windows(2).any(|rows| rows[0].0 >= rows[1].0) {
                return Err("publication source sites are unordered or repeated");
            }
            if function
                .sites
                .iter()
                .any(|row| row.1 >= function.operation_count)
            {
                return Err("publication source ordinal exceeds its function");
            }
            if function
                .byte_literals
                .windows(2)
                .any(|rows| rows[0].operation >= rows[1].operation)
            {
                return Err("publication literals are unordered or repeated");
            }
            if function.byte_literals.iter().any(|row| {
                function
                    .ordinal(PsiProvenance::Operation(row.operation))
                    .is_none()
            }) {
                return Err("publication literal has no source occurrence");
            }
        }
        let mut digest = DomainDigestEncoder::new(b"");
        encode(&functions, |bytes| digest.raw(bytes));
        Ok(Self {
            functions: functions.into(),
            identity: digest.finish(),
        })
    }

    pub fn functions(&self) -> &[NativeFunctionPublication] {
        &self.functions
    }

    pub fn function(&self, machine: MachineId) -> Option<&NativeFunctionPublication> {
        self.functions
            .binary_search_by_key(&machine, |row| row.machine)
            .ok()
            .map(|position| &self.functions[position])
    }

    pub const fn identity_bytes(&self) -> [u8; 32] {
        self.identity
    }

    pub fn encode(&self, bytes: &mut Vec<u8>) {
        encode(&self.functions, |part| bytes.extend_from_slice(part));
    }
}

impl Default for NativePublicationFacts {
    fn default() -> Self {
        Self::seal(Vec::new()).expect("the empty publication roster is ordered")
    }
}

fn encode(functions: &[NativeFunctionPublication], mut write: impl FnMut(&[u8])) {
    write(NATIVE_PUBLICATION_DOMAIN);
    write(&(functions.len() as u64).to_le_bytes());
    for function in functions {
        write(&function.machine.get().to_le_bytes());
        write(&function.operation_count.to_le_bytes());
        write(&(function.sites.len() as u64).to_le_bytes());
        for (site, ordinal) in &function.sites {
            let (tag, identity) = match site {
                PsiProvenance::Operation(operation) => (0, operation.get()),
                PsiProvenance::Edge(edge) => (1, edge.get()),
            };
            write(&[tag]);
            write(&identity.to_le_bytes());
            write(&ordinal.to_le_bytes());
        }
        write(&(function.byte_literals.len() as u64).to_le_bytes());
        for literal in &function.byte_literals {
            write(&literal.operation.get().to_le_bytes());
            write(&literal.place.get().to_le_bytes());
            write(&(literal.bytes.len() as u64).to_le_bytes());
            write(&literal.bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function() -> NativeFunctionPublication {
        let operation = OperationId::new(1).unwrap();
        NativeFunctionPublication {
            machine: MachineId::new(1).unwrap(),
            operation_count: 2,
            sites: vec![
                (PsiProvenance::Operation(operation), 0),
                (
                    PsiProvenance::Edge(semantic_vocabulary::EdgeId::new(1).unwrap()),
                    1,
                ),
            ],
            byte_literals: vec![NativeByteLiteral {
                operation,
                place: PlaceId::new(1).unwrap(),
                bytes: Arc::from(b"kept".as_slice()),
            }],
        }
    }

    #[test]
    fn publication_shares_payloads_and_commits_exact_sites_and_bytes() {
        let row = function();
        let bytes = Arc::clone(&row.byte_literals[0].bytes);
        let facts = NativePublicationFacts::seal(vec![row.clone()]).unwrap();
        let copy = facts.clone();
        assert!(Arc::ptr_eq(&facts.functions, &copy.functions));
        assert!(Arc::ptr_eq(
            &bytes,
            &facts.functions()[0].byte_literals[0].bytes
        ));
        let mut encoded = Vec::new();
        facts.encode(&mut encoded);
        assert_eq!(
            facts.identity_bytes(),
            optimization_core::digest_encoder::content_identity(&encoded)
        );
        let mut ordinal = row.clone();
        ordinal.sites[0].1 = 1;
        assert_ne!(
            facts.identity_bytes(),
            NativePublicationFacts::seal(vec![ordinal])
                .unwrap()
                .identity_bytes()
        );
        let mut literal = row;
        literal.byte_literals[0].bytes = Arc::from(b"other".as_slice());
        assert_ne!(
            facts.identity_bytes(),
            NativePublicationFacts::seal(vec![literal])
                .unwrap()
                .identity_bytes()
        );
    }

    #[test]
    fn publication_refuses_duplicate_missing_and_out_of_range_source_rows() {
        let original = function();
        let mut duplicate = original.clone();
        duplicate.sites.push(duplicate.sites[1]);
        let mut missing = original.clone();
        missing.sites.remove(0);
        let mut outside = original.clone();
        outside.sites[0].1 = outside.operation_count;
        let mut literal = original.clone();
        literal.byte_literals.push(literal.byte_literals[0].clone());
        for row in [duplicate, missing, outside, literal] {
            assert!(NativePublicationFacts::seal(vec![row]).is_err());
        }
        assert!(NativePublicationFacts::seal(vec![original.clone(), original]).is_err());
    }
}
