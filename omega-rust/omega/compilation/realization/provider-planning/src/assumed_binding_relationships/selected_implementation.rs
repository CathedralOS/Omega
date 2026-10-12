//! Seal-owned selected source code and its original applied callable family.
//!
//! Selection already owns the exact checked row and realization receipt. This
//! join adds the source unit revisions to the existing canonical callable
//! producer, which retains the original template and complete static tuple.
//! Source-free physical leaves retain their separate settled binding receipt.
//! This source component is not a complete semantic dependency commitment,
//! receiving consent, a full relationship identity or runtime installation.

use package_compilation::CheckedSourceCustody;
use symbols::SymbolHandle;

use super::PreparedAssumedBindingRelationship;
use crate::digest_encoder::DomainDigestEncoder;

/// Source revision of the selected checked implementation. The source family
/// and selected physical member have distinct source coordinates. Invocation
/// regions remain original declaration coordinates in the canonical family;
/// representative caller lifetimes cannot become shared-body identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingSelectedImplementationSource {
    source_family: [u8; 32],
    selected_body: [u8; 32],
    application: [u8; 32],
    digest: [u8; 32],
}
impl AssumedBindingSelectedImplementationSource {
    pub const fn source_family(self) -> [u8; 32] {
        self.source_family
    }
    pub const fn selected_body(self) -> [u8; 32] {
        self.selected_body
    }
    pub const fn application(self) -> [u8; 32] {
        self.application
    }
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

/// Consume the original selected owner beside the already produced source seal.
/// Absence remains unavailable; neither a foreign locator nor a native function
/// name is converted into a source body. No body checking or selection repeats.
pub fn retain_assumed_binding_selected_implementation_sources(
    relationships: &mut [PreparedAssumedBindingRelationship],
    custody: &CheckedSourceCustody,
) {
    for relationship in relationships {
        relationship.selected_implementation_source = capture(relationship, custody);
    }
}

fn capture(
    relationship: &PreparedAssumedBindingRelationship,
    custody: &CheckedSourceCustody,
) -> Option<AssumedBindingSelectedImplementationSource> {
    let checked = relationship.declaration_owner();
    if !custody.owns_sources(&checked.symbols) {
        return None;
    }
    let application = relationship.supplier_application()?;
    let row = crate::selected_applications::selected_checked_requirement_row(
        checked,
        relationship.selected_supplier(),
        relationship.supplier_row,
    )
    .ok()?;
    if row.requirement_template() != application.requirement_template()
        || row.adapter().symbol != application.realization_template()
    {
        return None;
    }
    let typed = &checked.typed;
    let instance = typed.machine(application.realization_instance())?;
    // Dependency capture has already joined this exact executable body to its
    // retained checked topology. An unmaterialized template is not source code
    // selected for this relationship.
    checked
        .facts
        .operational
        .machine_index_by_symbol(instance.symbol)?;
    let source_family = revision(relationship, custody, row.source_template())?;
    let selected_body = revision(relationship, custody, row.adapter().symbol)?;
    let callable = typed.machine_callable_identity_text(instance)?;
    if callable.is_empty() {
        return None;
    }
    let supplied = relationship.application_content()?.supplied();
    let mut application_encoder =
        DomainDigestEncoder::new(b"omega.assumed-binding-selected-source-application.v1\0");
    application_encoder.bytes(callable.as_bytes());
    application_encoder.raw(&supplied);
    let application = application_encoder.finish();
    let mut encoder =
        DomainDigestEncoder::new(b"omega.assumed-binding-selected-source-implementation.v1\0");
    encoder.raw(&source_family);
    encoder.raw(&selected_body);
    encoder.raw(&application);
    encoder.raw(
        relationship
            .selected_supplier()
            .plan
            .identity_digest()
            .as_bytes(),
    );
    Some(AssumedBindingSelectedImplementationSource {
        source_family,
        selected_body,
        application,
        digest: encoder.finish(),
    })
}

fn revision(
    relationship: &PreparedAssumedBindingRelationship,
    custody: &CheckedSourceCustody,
    symbol: SymbolHandle,
) -> Option<[u8; 32]> {
    let checked = relationship.declaration_owner();
    let machine = checked.typed.machine(symbol)?;
    let callable = checked.typed.machine_callable_identity_text(machine)?;
    if callable.is_empty() {
        return None;
    }
    let source = custody.declaration_source(&checked.symbols, symbol)?;
    let mut encoder =
        DomainDigestEncoder::new(b"omega.assumed-binding-selected-source-revision.v1\0");
    encoder.raw(&source.unit().revision_identity());
    encoder.bytes(callable.as_bytes());
    encoder.u64(source.start());
    encoder.u64(source.end());
    Some(encoder.finish())
}
