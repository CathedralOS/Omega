//! Source-produced native description round trips and publication controls.
//! The source-free verifier checks semantic inventory; exact native-product
//! custody additionally binds emitter facts that Terminal alone cannot derive.

use std::collections::BTreeSet;
use std::path::Path;

use component_description::component_description::ObligationKind;
use component_description::{
    COMPONENT_DESCRIPTION_SCHEMA_V3, ComponentVerificationRequest, decode_component_description,
    encode_component_description, verify_component,
};

pub(super) fn inspect(
    report: &compilation_products::CompileReport,
    fixture: &Path,
    build_dir: &Path,
    capture: Option<&super::artifact_capture::Capture>,
) -> Result<(), String> {
    let oracle = fixture.join("expected_native_component_descriptions.txt");
    if !oracle.exists() {
        return Ok(());
    }
    let expected: usize = std::fs::read_to_string(&oracle)
        .map_err(|error| format!("read native component description oracle: {error}"))?
        .trim()
        .parse()
        .map_err(|error| format!("parse native component description count: {error}"))?;
    let products = report
        .runtime_provisions()
        .ok_or("native component description oracle requires runtime products")?
        .component_products();
    if expected == 0 || products.len() != expected {
        return Err(format!(
            "native component description count {} differs from nonzero oracle {expected}",
            products.len()
        ));
    }
    for (ordinal, product) in products.iter().enumerate() {
        if let Some(capture) = capture {
            capture.observe(
                &format!("component-{ordinal}.image"),
                product.image().image().contents(),
            )?;
            capture.observe(
                &format!("component-{ordinal}.description"),
                product.description(),
            )?;
        }
        let request = ComponentVerificationRequest {
            expected_subject: product.selection().provider(),
            accepted_schemas: BTreeSet::from([COMPONENT_DESCRIPTION_SCHEMA_V3]),
            // These fixtures use ordinary checked providers without assumptions.
            accepted_assumptions: BTreeSet::new(),
        };
        let verified = verify_component(product.description(), &request)
            .map_err(|error| format!("native component description consumer: {error}"))?;
        let demand = product
            .image()
            .stack_demand()
            .map_err(|error| format!("{error:?}"))?;
        let stack = format!(
            "stack-provision:{}:{}:{}",
            demand.entry().get(),
            demand.ceiling_bytes(),
            demand.stack_alignment()
        );
        if verified.realization_identity() != Some(product.locator().identity_digest())
            || verified.artifact() != product.artifact().as_ref()
            || verified
                .obligations()
                .iter()
                .filter(|row| row.kind == ObligationKind::StackProvision)
                .map(|row| row.identity.as_str())
                .collect::<Vec<_>>()
                != [stack.as_str()]
        {
            return Err("native description lost its exact image, artifact or stack demand".into());
        }
        // A valid description for a sibling export is still the wrong native
        // realization, even when both exports share the same Terminal subject.
        if products.iter().enumerate().any(|(other, sibling)| {
            other != ordinal && product.verify_description(sibling.description()).is_ok()
        }) {
            return Err("native description accepted a sibling export substitution".into());
        }
        let directory = build_dir.join(format!("description-controls-{ordinal}"));
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let image_path = directory.join(product.locator().image_filename());
        let description_path = directory.join(product.description_filename());
        std::fs::write(&image_path, product.image().image().contents())
            .map_err(|error| error.to_string())?;
        std::fs::write(&description_path, product.description())
            .map_err(|error| error.to_string())?;
        let receipt = native_artifact::NativeComponentPublicationReceipt::verify(
            std::sync::Arc::clone(product),
            image_path,
        )?;
        let decoded = decode_component_description(product.description())
            .map_err(|error| format!("decode native description: {error:?}"))?;
        if ordinal == 0
            && fixture
                .join("expected_component_identity_controls.txt")
                .exists()
        {
            inspect_description_identity_controls(product.description(), &decoded, &request)?;
        }
        let mut missing_identity = decoded.clone();
        missing_identity.realization_identity = None;
        let mut missing_stack = decoded;
        missing_stack
            .obligations
            .retain(|row| row.kind != ObligationKind::StackProvision);
        for changed in [missing_identity, missing_stack] {
            let changed = encode_component_description(&changed);
            std::fs::write(&description_path, changed).map_err(|error| error.to_string())?;
            match receipt.revalidate() {
                Err(error) if error.contains("component description differs") => {}
                outcome => {
                    return Err(format!(
                        "altered native metadata was not rejected: {outcome:?}"
                    ));
                }
            }
        }
        std::fs::remove_file(&description_path).map_err(|error| error.to_string())?;
        match receipt.revalidate() {
            Err(error) if error.contains("component description publication") => {}
            outcome => {
                return Err(format!(
                    "missing native metadata was not rejected: {outcome:?}"
                ));
            }
        }
        std::fs::write(&description_path, product.description())
            .map_err(|error| error.to_string())?;
        receipt.revalidate()?;
        std::fs::remove_dir_all(directory).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Exercise the published byte consumer on this fixture's actual component,
/// including a derived export longer than a small source identifier. No checked
/// IR or admitted compiler evidence is manufactured or changed.
fn inspect_description_identity_controls(
    bytes: &[u8],
    decoded: &component_description::component_description::ComponentDescription,
    request: &ComponentVerificationRequest,
) -> Result<(), String> {
    use component_description::ComponentVerificationRejection;
    use component_description::component_description::DescriptionDecodeRejection;

    if !decoded
        .exports
        .iter()
        .any(|export| export.identity.len() > 1024)
    {
        return Err("description identity control lacks an actual long derived export".into());
    }
    // Schema V3 starts with magic, schema, frontier and the embedded artifact.
    // Select the first text field in the ordinary import/export wire roster.
    let identity_length = 8 + 4 + 1 + 4 + decoded.artifact_bytes.len() + 4 + 4;
    let text_start = identity_length + 4;
    let length = u32::from_le_bytes(
        bytes
            .get(identity_length..text_start)
            .ok_or("missing text length")?
            .try_into()
            .map_err(|_| "invalid text length")?,
    ) as usize;
    if length == 0 || text_start + length > bytes.len() {
        return Err("description identity control lacks a complete text field".into());
    }
    let mut excessive_length = bytes.to_vec();
    excessive_length[identity_length..text_start].copy_from_slice(&u32::MAX.to_le_bytes());
    if !matches!(
        decode_component_description(&excessive_length),
        Err(DescriptionDecodeRejection::Corrupt("truncated input"))
    ) {
        return Err("description consumer accepted a length beyond the retained bytes".into());
    }
    if !matches!(
        decode_component_description(&bytes[..text_start + length - 1]),
        Err(DescriptionDecodeRejection::Corrupt("truncated input"))
    ) {
        return Err("description consumer accepted truncated identity bytes".into());
    }
    let mut invalid_utf8 = bytes.to_vec();
    invalid_utf8[text_start] = 0xff;
    if !matches!(
        decode_component_description(&invalid_utf8),
        Err(DescriptionDecodeRejection::IdentityInvalid("not utf-8"))
    ) {
        return Err("description consumer accepted invalid UTF-8 identity bytes".into());
    }
    if !matches!(
        decode_component_description(&vec![0; 8 * 1024 * 1024 + 1]),
        Err(DescriptionDecodeRejection::RosterBoundExceeded(
            "description byte length"
        ))
    ) {
        return Err("description consumer accepted a payload beyond its wire envelope".into());
    }
    let mut changed = decoded.clone();
    changed
        .exports
        .first_mut()
        .ok_or("missing actual export")?
        .identity
        .push_str(":changed");
    let changed = encode_component_description(&changed);
    decode_component_description(&changed)
        .map_err(|error| format!("changed full identity did not form: {error:?}"))?;
    if !matches!(
        verify_component(&changed, request),
        Err(ComponentVerificationRejection::UnexpectedDerivedExport(_))
    ) {
        return Err("description consumer accepted a different complete export identity".into());
    }
    Ok(())
}
