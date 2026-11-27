//! Dependency preparations observed during one checked Build activation.
use semantic_vocabulary::PackageKeyIdentity;
use source_provenance::DependencyScope;
use wire::digest_encoder::DomainDigestEncoder;

#[derive(Clone, Debug, PartialEq, Eq, record_storage::Record)]
struct DependencyPreparation {
    purpose: DependencyScope,
    request_ordinal: usize,
    package: PackageKeyIdentity,
    inputs: [u8; 32],
    child_build_source: Option<[u8; 32]>,
    handoff: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, record_storage::Record)]
pub struct DependencyPreparations {
    preparations: Vec<DependencyPreparation>,
}

impl DependencyPreparations {
    pub fn prepared(
        &mut self,
        purpose: DependencyScope,
        request_ordinal: usize,
        package: PackageKeyIdentity,
        inputs: [u8; 32],
        child_build_source: Option<[u8; 32]>,
        handoff: Option<[u8; 32]>,
    ) {
        self.preparations.push(DependencyPreparation {
            purpose,
            request_ordinal,
            package,
            inputs,
            child_build_source,
            handoff,
        });
    }

    pub(super) fn write_identity(&self, digest: &mut DomainDigestEncoder) {
        digest.len(self.preparations.len());
        for event in &self.preparations {
            digest.byte(match event.purpose {
                DependencyScope::Product => 0,
                DependencyScope::Build => 1,
            });
            digest.len(event.request_ordinal);
            digest.raw(&event.package.digest());
            digest.raw(&event.inputs);
            for evidence in [&event.child_build_source, &event.handoff] {
                match evidence {
                    Some(commitment) => {
                        digest.byte(1);
                        digest.raw(commitment);
                    }
                    None => digest.byte(0),
                }
            }
        }
    }
}
