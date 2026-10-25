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
    inspect_linear_result_claims(report, fixture)?;
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

/// Source negatives stop before the retained target trust boundary. This
/// oracle corrupts only the target's proposed root receipt, keeping the
/// accepted abstract source, boundary binding and physical layout unchanged.
fn inspect_linear_result_claims(
    report: &compilation_products::CompileReport,
    fixture: &Path,
) -> Result<(), String> {
    let oracle = fixture.join("expected_native_linear_result_claims.txt");
    let expected = match std::fs::read_to_string(&oracle) {
        Ok(text) => text
            .trim()
            .parse::<usize>()
            .map_err(|error| error.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("read linear result claim oracle: {error}")),
    };
    if expected != 1 {
        return Err("linear opaque result control requires one actual root claim".into());
    }
    let artifact = report
        .retained_native_artifact()
        .ok_or("linear result claim control requires retained native input")?;
    image_emission::verify_retained_target_operations(artifact.object())
        .map_err(|error| format!("unmodified linear target audit failed: {error}"))?;
    for clear in [true, false] {
        let mut changed = 0;
        let outcome =
            artifact
                .object()
                .verify_test_target_operation_mutation(&mut |plan| {
                    for function in &mut plan.functions {
                        for block in &mut function.graph.blocks {
                            for operation in &mut block.operations {
                                let target_operations::TargetUnitOperation::NormalizedForeignCall {
                                    result:
                                        target_operations::TargetBoundaryResult::Structural(home),
                                    ..
                                } = operation
                                else {
                                    continue;
                                };
                                let target_operations::TargetStructuralHomeOrigin::OperationResult {
                            result, ..
                        } = &mut home.origin else { continue };
                                if changed != 0
                                    || result.multiplicity
                                        != terminal_psi::StructuralMultiplicity::Linear
                                    || result.claims.len() != expected
                                    || !result.claims[0].path.is_empty()
                                {
                                    continue;
                                }
                                changed += 1;
                                if clear {
                                    result.claims.clear();
                                } else {
                                    let original = result.claims[0].claim.get();
                                    let replacement = if original == 1 { 2 } else { 1 };
                                    result.claims[0].claim =
                                        semantic_vocabulary::ClaimId::new(replacement)
                                            .expect("nonzero distinct test claim ID");
                                }
                            }
                        }
                    }
                });
        if changed != 1 {
            return Err(format!(
                "linear target control mutated {changed} actual result(s), expected 1"
            ));
        }
        match outcome {
            Err(error) if error.to_string().contains("SourceCustodyMismatch") => {}
            result => {
                return Err(format!(
                    "changed linear result claim was not refused by source custody: {result:?}"
                ));
            }
        }
    }
    Ok(())
}
