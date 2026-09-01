use crate::tests::*;

use super::realization::RealizedCase;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct PublishedSnapshot {
    action_count: usize,
    realization_manifest: Vec<u8>,
    fragment_manifest: Vec<u8>,
    text_manifest: Vec<u8>,
    text_bytes: Vec<u8>,
    object_manifest: Vec<u8>,
    object_bytes: Vec<u8>,
    artifact_record: Vec<u8>,
    artifact_manifest: Vec<u8>,
    callable_record: Vec<u8>,
    callable_manifest: Vec<u8>,
}

pub(super) fn publish(realized: RealizedCase) -> PublishedSnapshot {
    let RealizedCase {
        case,
        semantic,
        proof,
        selections,
        realization,
    } = realized;
    let selection_identity = selections.identity();
    let action_count = realization.optimization().action_count();
    let realization_record = realization.manifest().record();
    assert_eq!(realization_record.target, case.target);
    assert_eq!(realization_record.selections, selection_identity);
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(&realization_record.encode()),
        Ok(realization_record.clone())
    );
    let realization_manifest = realization_record.encode();

    let fragments = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(realization)),
    )
    .unwrap();
    let fragment_record = fragments.manifest().record();
    assert_eq!(
        fragment_record.source_kind,
        FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
            optimization: case.rule,
        }
    );
    assert_eq!(fragment_record.target, case.target);
    assert_eq!(fragment_record.selections, selection_identity);
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&fragment_record.encode()),
        Ok(fragment_record.clone())
    );
    assert_eq!(
        validate_optimized_function_fragment_emission(&fragments).unwrap(),
        fragments.custody()
    );
    let fragment_manifest = fragment_record.encode();

    let text = stage_optimized_relocation_free_text_section(fragments).unwrap();
    let text_record = text.manifest().record();
    assert_eq!(text_record.target, case.target);
    assert_eq!(text_record.selections, selection_identity);
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&text_record.encode()),
        Ok(text_record.clone())
    );
    assert_eq!(
        validate_optimized_relocation_free_text_section(&text).unwrap(),
        text.custody()
    );
    let text_manifest = text_record.encode();
    let text_bytes = text.text_section().bytes.clone();
    assert!(!text_bytes.is_empty());

    let object = stage_optimized_relocation_free_object_container(text).unwrap();
    let object_record = object.manifest().record();
    assert_eq!(object_record.target, case.target);
    assert_eq!(object_record.selections, selection_identity);
    assert_eq!(object.object().target.object_format, case.object_format);
    assert_eq!(object.object().text_section.name, case.text_section);
    assert_eq!(object.object().text_section.bytes, text_bytes);
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&object_record.encode()),
        Ok(object_record.clone())
    );
    assert_eq!(
        validate_optimized_relocation_free_object_container(&object).unwrap(),
        object.custody()
    );
    let object_manifest = object_record.encode();
    let object_bytes = object.container().bytes.clone();
    assert!(!object_bytes.is_empty());

    let artifact =
        stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
            .unwrap();
    assert_eq!(artifact.artifact().target, case.target);
    assert_eq!(artifact.artifact().selections, selection_identity);
    assert_eq!(
        validate_optimized_object_artifact(&artifact).unwrap(),
        artifact.custody()
    );
    let artifact_record = artifact.artifact().encode();
    let artifact_manifest = artifact.manifest().record().encode();

    let callable = stage_validated_optimized_ordinary_callable_entry(artifact).unwrap();
    assert_eq!(callable.entry().target, case.target);
    assert_eq!(callable.entry().selections, selection_identity);
    assert_eq!(callable.entry().calling_policy, case.calling_policy);
    assert_eq!(callable.entry().exit_policy, case.exit_policy);
    assert_eq!(
        validate_optimized_ordinary_callable_entry(&callable).unwrap(),
        callable.custody()
    );
    let callable_record = callable.entry().encode().unwrap();
    let callable_manifest = callable.manifest().record().encode();

    PublishedSnapshot {
        action_count,
        realization_manifest,
        fragment_manifest,
        text_manifest,
        text_bytes,
        object_manifest,
        object_bytes,
        artifact_record,
        artifact_manifest,
        callable_record,
        callable_manifest,
    }
}
