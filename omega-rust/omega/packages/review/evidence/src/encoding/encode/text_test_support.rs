//! Exercise named traversal on the existing populated component fixture corpus.

use super::encoder::text::Writer;
use super::*;
use crate::encoding::{
    PackagePolicyRecoveryLimits, recovery::decode_policy_text_scalars as binary,
};
use crate::record::*;

pub(in crate::encoding) enum Component<'value> {
    PublicApi(&'value PackagePolicyPublicApi),
    Callables(&'value PackagePolicyCallables),
    SelectedProviders(&'value PackagePolicySelectedProviders),
    TerminalPermissions(&'value PackagePolicyTerminalPermissions),
    Representation(&'value PackagePolicyRepresentation),
    Calling(&'value PackagePolicyCallingPlan),
    Physical(&'value PackagePolicyPhysicalCallingContract),
    External(&'value PackagePolicyExternalExecutableSupply),
}

pub(in crate::encoding) fn component(value: Component<'_>) {
    meaning(|encoder| match value {
        Component::PublicApi(value) => public_api::public_api(encoder, value),
        Component::Callables(value) => callable_policy::policy(encoder, value),
        Component::SelectedProviders(value) => selected_providers::policy(encoder, value),
        Component::TerminalPermissions(value) => terminal_permissions::policy(encoder, value),
        Component::Representation(value) => representation::policy(encoder, value),
        Component::Calling(value) => calling::encode_application(encoder, value),
        Component::Physical(value) => {
            values::physical_calling_policy::encode_physical(encoder, value)
        }
        Component::External(value) => values::external_policy::policy(encoder, value),
    });
}

pub(in crate::encoding) fn meaning(
    encode: impl Fn(&mut encoder::Encoder) -> Result<(), PackageReviewEncodingError>,
) {
    let mut original = encoder::Encoder::policy_bounded(4 * 1024 * 1024);
    encode(&mut original).unwrap();
    let original = original.finish().unwrap();
    let mut named = encoder::Encoder::policy_text(Writer::new(32 * 1024 * 1024, None));
    named.field("component", |encoder| encode(encoder)).unwrap();
    let named = named.finish_text().unwrap();
    let (recovered, _) = binary(&named, PackagePolicyRecoveryLimits::default()).unwrap();
    assert_eq!(
        original, recovered,
        "named events must encode the same complete scalar stream"
    );
    let mut compare = encoder::Encoder::policy_text(Writer::new(named.len(), Some(&named)));
    compare
        .field("component", |encoder| encode(encoder))
        .unwrap();
    assert!(compare.finish_text().unwrap().is_empty());
}
