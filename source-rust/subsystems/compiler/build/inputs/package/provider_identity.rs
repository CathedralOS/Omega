//! Package provider identity binds the declarations, their package-relative source and the selected arguments.

use filesystem_authority::observation_codec::push_framed;
use retained_product_selections::{
    RetainedProductApplication, RetainedProviderSelection, RetainedProviderSubject,
};
use semantic_vocabulary::source_selections::{CompositionMode, ProductTarget};
use source_admission::{RetainedDeclaration, SourceFile, SourceOrigin, SourceResolutionStratum};
use source_provenance::DependencyScope;
use source_text::SourceId;
use symbols::SymbolKind;

const UNQUALIFIED_TARGET: u64 = u64::MAX;

struct SelectionSources {
    sources: Vec<SourceFile>,
}

impl SelectionSources {
    fn collect(rows: &[RetainedProviderSelection]) -> Self {
        let mut pool = Self {
            sources: Vec::new(),
        };
        for row in rows {
            for anchor in row.subject.declarations().chain([
                &row.provider,
                row.origin.activation(),
                row.origin.author(),
            ]) {
                pool.intern_source(anchor.source());
            }
            for application in [row.subject.application(), row.provider_application.as_ref()]
                .into_iter()
                .flatten()
            {
                pool.intern_application(application);
            }
        }
        pool
    }

    fn intern_application(&mut self, application: &RetainedProductApplication) {
        let mut graph = application.graph.clone();
        graph.remap_source_spans(|span| {
            let source = application
                .sources
                .get(span.source_id)
                .expect("captured application source coordinates belong to their retained map");
            source_text::SourceSpan::new(self.intern_source(source), span.span)
        });
    }

    fn intern_source(&mut self, source: &SourceFile) -> SourceId {
        let mut source = source.clone();
        source.source_id = SourceId(0);
        let index = match self.sources.iter().position(|existing| *existing == source) {
            Some(index) => index,
            None => {
                let index = self.sources.len();
                self.sources.push(source);
                index
            }
        };
        SourceId(index)
    }

    fn application(&self, application: Option<&RetainedProductApplication>, output: &mut Vec<u8>) {
        let Some(application) = application else {
            output.push(0);
            return;
        };
        output.push(1);
        push_framed(output, application.identity.as_bytes());
        let mut graph = application.graph.clone();
        graph.remap_source_spans(|span| {
            let mut source = application
                .sources
                .get(span.source_id)
                .expect("captured application retains its source coordinates")
                .clone();
            source.source_id = SourceId(0);
            let index = self
                .sources
                .iter()
                .position(|existing| *existing == source)
                .expect("selection source pool contains every closed application coordinate");
            source_text::SourceSpan::new(SourceId(index), span.span)
        });
        let limits = record_storage::RecordLimits {
            bytes: usize::MAX,
            allocation: usize::MAX,
            elements: usize::MAX,
            depth: usize::MAX,
        };
        let bytes = record_storage::encode(&graph, limits)
            .expect("owned graph encoding has no external resource limits");
        push_framed(output, &bytes);
    }

    fn anchor(&self, anchor: &RetainedDeclaration, output: &mut Vec<u8>) {
        let source_position = self
            .sources
            .iter()
            .position(|source| source == anchor.source())
            .expect("selection source pool contains every retained anchor");
        integer(output, source_position as u64);
        integer(output, anchor.span().start as u64);
        integer(output, anchor.span().end as u64);
        output.push(kind_tag(anchor.kind()));
    }

    fn rows(
        &self,
        rows: &[RetainedProviderSelection],
        observation: Option<[u8; 32]>,
        families: bool,
    ) -> Vec<u8> {
        let mut output = Vec::new();
        match observation {
            None => output.push(0),
            Some(digest) => {
                output.push(1);
                output.extend_from_slice(&digest);
            }
        }
        integer(&mut output, rows.len() as u64);
        for row in rows {
            match &row.subject {
                RetainedProviderSubject::Declaration {
                    declaration,
                    identity,
                    ..
                } => {
                    if families {
                        output.push(0);
                    }
                    self.anchor(declaration, &mut output);
                    push_framed(&mut output, identity.as_bytes());
                }
                RetainedProviderSubject::OperatorFamily(family) => {
                    output.push(1);
                    push_framed(&mut output, family.canonical_path.as_bytes());
                    integer(&mut output, family.coordinates().len() as u64);
                    for member in family.coordinates() {
                        self.anchor(&member.declaration, &mut output);
                        push_framed(&mut output, member.requirement_identity.as_bytes());
                        integer(&mut output, member.static_parameter_count as u64);
                    }
                }
            }
            self.anchor(&row.provider, &mut output);
            push_framed(&mut output, row.provider_identity.as_bytes());
            if let RetainedProviderSubject::Declaration { application, .. } = &row.subject {
                self.application(application.as_ref(), &mut output);
            }
            self.application(row.provider_application.as_ref(), &mut output);
            integer(
                &mut output,
                row.target.map_or(UNQUALIFIED_TARGET, ProductTarget::code),
            );
            output.push(match row.composition_mode {
                CompositionMode::Fused => 0,
                CompositionMode::Independent => 1,
            });
            self.anchor(row.origin.activation(), &mut output);
            self.anchor(row.origin.author(), &mut output);
            integer(&mut output, row.origin.occurrence().start as u64);
            integer(&mut output, row.origin.occurrence().end as u64);
            match row.origin.event_ordinal() {
                None => output.push(0),
                Some(ordinal) => {
                    output.push(1);
                    integer(&mut output, ordinal);
                }
            }
        }
        output
    }
}

pub(super) fn retained_declarations_digest(anchors: &[&RetainedDeclaration]) -> [u8; 32] {
    let mut pool = SelectionSources {
        sources: Vec::new(),
    };
    for anchor in anchors {
        pool.intern_source(anchor.source());
    }
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega-retained-declarations-v1\0");
    digest_sources(&pool.sources, &mut digest);
    let mut rows = Vec::new();
    integer(&mut rows, anchors.len() as u64);
    for anchor in anchors {
        pool.anchor(anchor, &mut rows);
    }
    digest.bytes(&rows);
    digest.finish()
}

fn has_families(rows: &[RetainedProviderSelection]) -> bool {
    rows.iter()
        .any(|row| matches!(row.subject, RetainedProviderSubject::OperatorFamily(_)))
}

fn digest_sources(sources: &[SourceFile], digest: &mut crate::digest_encoder::DomainDigestEncoder) {
    digest.u64(sources.len() as u64);
    for source in sources {
        let relative = source
            .path
            .strip_prefix(&source.package_root)
            .unwrap_or(&source.path);
        digest.bytes(relative.as_os_str().as_encoded_bytes());
        let mut owner = Vec::new();
        source_owner(source, &mut owner);
        digest.bytes(&owner);
        digest.raw(&crate::digest_encoder::content_identity(
            source.source.as_bytes(),
        ));
    }
}

pub(super) fn package_provider_selections_digest(
    rows: &[RetainedProviderSelection],
    observation: Option<[u8; 32]>,
) -> [u8; 32] {
    let pool = SelectionSources::collect(rows);
    let families = has_families(rows);
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(if families {
        b"omega-package-provider-choices-v4\0"
    } else {
        b"omega-package-provider-choices-v3\0"
    });
    digest_sources(&pool.sources, &mut digest);
    digest.bytes(&pool.rows(rows, observation, families));
    digest.finish()
}

pub(super) fn product_applications_digest(
    rows: &[Option<&RetainedProductApplication>],
) -> [u8; 32] {
    let mut pool = SelectionSources {
        sources: Vec::new(),
    };
    for application in rows.iter().flatten() {
        pool.intern_application(application);
    }
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega-package-product-applications-v1\0");
    digest_sources(&pool.sources, &mut digest);
    let mut records = Vec::new();
    integer(&mut records, rows.len() as u64);
    for application in rows {
        pool.application(*application, &mut records);
    }
    digest.bytes(&records);
    digest.finish()
}

fn integer(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}
fn kind_tag(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Machine => 0,
        SymbolKind::Trait => 1,
        SymbolKind::Conformance => 2,
        SymbolKind::Data => 3,
        SymbolKind::Operator => 4,
        SymbolKind::State => 5,
        _ => unreachable!("provider selection anchors are admitted declaration roles"),
    }
}
fn source_owner(source: &SourceFile, output: &mut Vec<u8>) {
    match source.package_identity {
        None => output.push(0),
        Some(owner) => {
            output.push(1);
            output.extend_from_slice(&owner.digest());
        }
    }
    output.push(match source.dependency_scope {
        DependencyScope::Product => 0,
        DependencyScope::Build => 1,
    });
    output.push(match source.origin {
        SourceOrigin::User => 0,
        SourceOrigin::Toolchain => 1,
        SourceOrigin::ToolchainContract => 2,
    });
    output.push(match source.resolution_stratum {
        SourceResolutionStratum::Base => 0,
        SourceResolutionStratum::CurrentActivationExtension => 1,
        SourceResolutionStratum::DependencyActivationExtension => 2,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use retained_product_selections::{RetainedOperatorCoordinate, RetainedOperatorFamily};
    use source_declarations::declaration_selection::BuildSelectionOrigin;
    use source_text::Span;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn source_identity_distinguishes_toolchain_contracts_from_core() {
        for (origin, tag) in [
            (SourceOrigin::User, 0),
            (SourceOrigin::Toolchain, 1),
            (SourceOrigin::ToolchainContract, 2),
        ] {
            let file = SourceFile {
                source_id: SourceId(0),
                package_root: PathBuf::from("system"),
                path: PathBuf::from("system").join("entry.omg"),
                package_identity: None,
                dependency_scope: DependencyScope::Product,
                origin,
                resolution_stratum: SourceResolutionStratum::Base,
                source: Arc::from("machine entry() {}"),
            };
            let mut owner = Vec::new();
            source_owner(&file, &mut owner);
            assert_eq!(owner, [0, 0, tag, 0]);
        }
    }

    #[test]
    fn provider_identity_binds_the_selected_target_and_each_operator_overload() {
        let file = SourceFile {
            source_id: SourceId(0),
            package_root: "root".into(),
            path: "root/source.omg".into(),
            package_identity: None,
            dependency_scope: DependencyScope::Product,
            origin: SourceOrigin::User,
            resolution_stratum: SourceResolutionStratum::Base,
            source: "build small wide".into(),
        };
        let anchor = |start, end, kind| {
            RetainedDeclaration::from_retained_parts(file.clone(), Span::new(start, end), kind)
                .unwrap()
        };
        let build = anchor(0, 5, SymbolKind::Machine);
        let first = anchor(6, 11, SymbolKind::Operator);
        let second = anchor(12, 16, SymbolKind::Operator);
        let origin = BuildSelectionOrigin::from_retained_parts(
            build.clone(),
            build.clone(),
            Span::new(0, 5),
            Some(0),
        )
        .unwrap();
        let single = RetainedProviderSelection {
            subject: RetainedProviderSubject::Declaration {
                declaration: build.clone(),
                identity: "one".into(),
                application: None,
            },
            provider: build,
            provider_identity: "provider".into(),
            provider_application: None,
            target: Some(ProductTarget::LinuxX86_64),
            composition_mode: CompositionMode::Fused,
            origin,
        };
        let observation = Some([0x73; 32]);
        let ordinary =
            package_provider_selections_digest(std::slice::from_ref(&single), observation);
        let mut unqualified = single.clone();
        unqualified.target = None;
        assert_ne!(
            package_provider_selections_digest(&[unqualified], observation),
            ordinary
        );
        let members = vec![
            RetainedOperatorCoordinate {
                declaration: first,
                requirement_identity: "family::one".into(),
                static_parameter_count: 0,
            },
            RetainedOperatorCoordinate {
                declaration: second,
                requirement_identity: "family::two".into(),
                static_parameter_count: 2,
            },
        ];
        let family = |members| RetainedProviderSelection {
            subject: RetainedProviderSubject::OperatorFamily(
                RetainedOperatorFamily::new("Buffer::index".into(), members).unwrap(),
            ),
            ..single.clone()
        };
        let original = [single.clone(), family(members.clone())];
        let mut changed = members;
        changed[1].static_parameter_count += 1;
        assert_ne!(
            package_provider_selections_digest(&original, observation),
            package_provider_selections_digest(&[single.clone(), family(changed)], observation)
        );
    }
}
