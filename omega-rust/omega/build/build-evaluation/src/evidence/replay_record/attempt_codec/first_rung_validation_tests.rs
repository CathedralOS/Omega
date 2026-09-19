//! Attempt codec tests.

use super::{
    AttemptShape, Decoder, Encoder, ShapeAuthorizedPath, ShapeLogicalInput,
    ShapeLogicalInputResolution, ShapeLogicalOutput, ShapeMetadata, ShapeMutableBytes,
    ShapeObservedRegion, ShapeResult, ShapeReturnedPath, ShapeRootedPath, ShapeScalar,
};
use crate::BuildCanonicalSourceMetadataIdentity;
use crate::evidence::replay_record::BuildFilesystemReplayRecordLimits;
use crate::evidence::replay_record::rehydration::decode_canonical_source_metadata_identity;
use crate::evidence::replay_record::shape_validation::validate_first_rung;

static METADATA_CARRIER: [u8; checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES] =
    [0; checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES];

fn empty_shape(operation: u16, result: ShapeResult) -> AttemptShape<'static> {
    AttemptShape {
        operation,
        provider: 2,
        result,
        post_error: 0,
        scalars: Vec::new(),
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_paths: Vec::new(),
        returned_paths: Vec::new(),
        returned_path_count: 0,
        observed_regions: Vec::new(),
        metadata: Vec::new(),
        mutable_byte_resolutions: Vec::new(),
        mutable_i64_resolutions: Vec::new(),
        mutable_bytes: Vec::new(),
        mutable_i64s: Vec::new(),
        authorized_paths: Vec::new(),
        inputs: Vec::new(),
        output: None,
        retired: Vec::new(),
        refusal_count: 0,
        refusals: Vec::new(),
    }
}

fn exact_input_output_shapes() -> Vec<AttemptShape<'static>> {
    let mut open = empty_shape(2, ShapeResult::Handle(1));
    open.scalars = vec![(1, ShapeScalar::I32(0))];
    open.rooted_paths = vec![ShapeRootedPath {
        ordinal: 0,
        root: 0,
        bytes: b"main.omg",
    }];
    open.authorized_paths = vec![ShapeAuthorizedPath {
        ordinal: 0,
        access: 0,
        root: 0,
        bytes: b"main.omg",
    }];
    open.output = Some(ShapeLogicalOutput {
        kind: 0,
        identity: 1,
        source: 0,
        source_identity: None,
    });

    let mut read = empty_shape(4, ShapeResult::Scalar(0));
    read.scalars = vec![(2, ShapeScalar::U64(0))];
    read.observed_regions = vec![ShapeObservedRegion {
        ordinal: 1,
        kind: 0,
        offset: 0,
        length: 0,
    }];
    read.mutable_byte_resolutions = vec![(1, b"")];
    read.mutable_bytes = vec![ShapeMutableBytes {
        ordinal: 1,
        pre: b"",
        post: b"",
    }];
    read.inputs = vec![ShapeLogicalInput {
        ordinal: 0,
        kind: 0,
        resolution: ShapeLogicalInputResolution::Resolved(1),
    }];

    let mut source_close = empty_shape(8, ShapeResult::Scalar(0));
    source_close.inputs = read.inputs.clone();
    source_close.retired = vec![1];

    let mut create = empty_shape(1, ShapeResult::Handle(2));
    create.scalars = vec![(
        1,
        ShapeScalar::I32(checked_interpreter::FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE),
    )];
    create.rooted_paths = vec![ShapeRootedPath {
        ordinal: 0,
        root: 1,
        bytes: b"generated.omg",
    }];
    create.authorized_paths = vec![ShapeAuthorizedPath {
        ordinal: 0,
        access: 1,
        root: 1,
        bytes: b"generated.omg",
    }];
    create.output = Some(ShapeLogicalOutput {
        kind: 0,
        identity: 2,
        source: 0,
        source_identity: None,
    });

    let mut write = empty_shape(5, ShapeResult::Scalar(7));
    write.byte_operands = vec![(1, b"payload")];
    write.inputs = vec![ShapeLogicalInput {
        ordinal: 0,
        kind: 0,
        resolution: ShapeLogicalInputResolution::Resolved(2),
    }];

    let mut output_close = empty_shape(8, ShapeResult::Scalar(0));
    output_close.inputs = write.inputs.clone();
    output_close.retired = vec![2];

    vec![open, read, source_close, create, write, output_close]
}

fn exact_descriptor_metadata_shapes() -> Vec<AttemptShape<'static>> {
    let mut shapes = exact_input_output_shapes();
    let mut metadata = empty_shape(39, ShapeResult::Scalar(0));
    metadata.metadata = vec![ShapeMetadata {
        ordinal: 1,
        kind: 1,
        device: 1,
        mode: 0o100444,
        link_count: 1,
        inode: 2,
        user: 3,
        group: 4,
        referenced_device: 0,
        access_time: 5,
        modification_time: 6,
        change_time: 7,
        birth_time: 8,
        size: 23,
        blocks_512: 8,
        preferred_block_size: 4096,
    }];
    metadata.mutable_byte_resolutions = vec![(1, &METADATA_CARRIER)];
    metadata.mutable_bytes = vec![ShapeMutableBytes {
        ordinal: 1,
        pre: &METADATA_CARRIER,
        post: &METADATA_CARRIER,
    }];
    metadata.inputs = vec![ShapeLogicalInput {
        ordinal: 0,
        kind: 0,
        resolution: ShapeLogicalInputResolution::Resolved(1),
    }];
    shapes[1] = metadata;
    shapes
}

#[test]
fn source_output_lifetime_projections_preserve_interleaved_membership() {
    use crate::evidence::replay_record::shape_validation::source_output_membership;
    let original = exact_input_output_shapes();
    let mut second = original[..3].to_vec();
    second[0].result = ShapeResult::Handle(3);
    second[0].output.as_mut().unwrap().identity = 3;
    second[1].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(3);
    second[2].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(3);
    second[2].retired = vec![3];
    let shapes = vec![
        original[0].clone(),
        original[1].clone(),
        original[3].clone(),
        original[4].clone(),
        second[0].clone(),
        second[1].clone(),
        original[2].clone(),
        original[5].clone(),
        second[2].clone(),
    ];
    validate_first_rung(&shapes).expect("Source and Output lifetimes compose");
    let membership = source_output_membership(&shapes).unwrap();
    assert_eq!(membership.source_events, vec![vec![0, 1, 6], vec![4, 5, 8]]);
    assert_eq!(membership.output_attempts, vec![2, 3, 7]);

    let mut reused = shapes.clone();
    reused[4].output.as_mut().unwrap().identity = 2;
    reused[4].result = ShapeResult::Handle(2);
    assert!(validate_first_rung(&reused).is_err());
    let mut retired = shapes.clone();
    retired.swap(1, 6);
    assert!(validate_first_rung(&retired).is_err());
    let mut wrong_kind = shapes.clone();
    wrong_kind[5].inputs[0].kind = 1;
    assert!(validate_first_rung(&wrong_kind).is_err());
    let mut missing_close = shapes.clone();
    missing_close.pop();
    assert!(validate_first_rung(&missing_close).is_err());
    let mut wrong_root = shapes;
    wrong_root[4].rooted_paths[0].root = 1;
    assert!(validate_first_rung(&wrong_root).is_err());
}

#[test]
fn native_last_error_membership_requires_original_stream_adjacency() {
    let native = native_query_chain_shapes();
    let output = exact_input_output_shapes();
    let error = empty_shape(35, ShapeResult::Scalar(0));
    let mut shapes = vec![
        native[0].clone(),
        error.clone(),
        output[3].clone(),
        native[1].clone(),
        error.clone(),
        output[4].clone(),
        native[2].clone(),
        output[5].clone(),
    ];
    validate_first_rung(&shapes).expect("native errors follow their exact preceding native event");
    shapes.swap(4, 5);
    assert!(
        validate_first_rung(&shapes).is_err(),
        "Output interrupts native error association"
    );
}

#[test]
fn output_write_authorization_lane_rejects_during_recovery_validation() {
    let mut shapes = exact_input_output_shapes();
    assert!(validate_first_rung(&shapes).is_ok());
    shapes[4].authorized_paths.push(ShapeAuthorizedPath {
        ordinal: 0,
        access: 1,
        root: 1,
        bytes: b"generated.omg",
    });
    assert!(validate_first_rung(&shapes).is_err());
}

#[test]
fn output_descriptor_overlap_rejects_during_recovery_validation() {
    let mut shapes = exact_input_output_shapes();
    shapes[3].result = ShapeResult::Handle(1);
    shapes[3].output.as_mut().unwrap().identity = 1;
    shapes[4].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(1);
    shapes[5].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(1);
    shapes[5].retired[0] = 1;
    assert!(validate_first_rung(&shapes).is_err());
}

#[test]
fn positioned_output_write_requires_one_exact_nonnegative_offset() {
    let mut shapes = exact_input_output_shapes();
    shapes[4].operation = 7;
    shapes[4].scalars = vec![(2, ShapeScalar::I64(3))];
    assert!(validate_first_rung(&shapes).is_ok());

    for scalars in [
        Vec::new(),
        vec![(1, ShapeScalar::I64(3))],
        vec![(2, ShapeScalar::I64(-1))],
        vec![(2, ShapeScalar::I64(3)), (3, ShapeScalar::I64(4))],
    ] {
        let mut malformed = shapes.clone();
        malformed[4].scalars = scalars;
        assert!(validate_first_rung(&malformed).is_err());
    }

    let mut sequential_with_offset = exact_input_output_shapes();
    sequential_with_offset[4].scalars = vec![(2, ShapeScalar::I64(3))];
    assert!(validate_first_rung(&sequential_with_offset).is_err());

    let mut sparse_over_ceiling = shapes;
    sparse_over_ceiling[4].scalars = vec![(
        2,
        ShapeScalar::I64(
            i64::try_from(checked_interpreter::MAX_FILESYSTEM_REPLAY_RETAINED_BYTES).unwrap(),
        ),
    )];
    assert!(validate_first_rung(&sparse_over_ceiling).is_err());
}

#[test]
fn empty_output_file_requires_exact_create_close_pair() {
    let mut shapes = exact_input_output_shapes();
    shapes.remove(4);
    assert!(validate_first_rung(&shapes).is_ok());

    let mut missing_close = shapes.clone();
    missing_close.pop();
    assert!(validate_first_rung(&missing_close).is_err());

    let mut extra_operation = shapes;
    extra_operation.insert(4, empty_shape(12, ShapeResult::Scalar(0)));
    assert!(validate_first_rung(&extra_operation).is_err());
}

#[test]
fn output_sync_requires_exact_success_and_descriptor_lineage() {
    let mut shapes = exact_input_output_shapes();
    let mut sync = empty_shape(43, ShapeResult::Scalar(0));
    sync.inputs = shapes[4].inputs.clone();
    shapes.insert(4, sync);
    assert!(validate_first_rung(&shapes).is_ok());

    let mut failed = shapes.clone();
    failed[4].result = ShapeResult::Scalar(-1);
    assert!(validate_first_rung(&failed).is_err());

    let mut wrong_descriptor = shapes.clone();
    wrong_descriptor[4].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
    assert!(validate_first_rung(&wrong_descriptor).is_err());

    let mut spoofed_lane = shapes;
    spoofed_lane[4].scalars = vec![(1, ShapeScalar::I32(0))];
    assert!(validate_first_rung(&spoofed_lane).is_err());
}

#[test]
fn output_duplicate_requires_exact_lineage_and_immediate_retirement() {
    let mut shapes = exact_input_output_shapes();
    let mut duplicate = empty_shape(45, ShapeResult::Handle(3));
    duplicate.inputs = shapes[4].inputs.clone();
    duplicate.output = Some(ShapeLogicalOutput {
        kind: 0,
        identity: 3,
        source: 1,
        source_identity: Some(2),
    });
    let mut duplicate_close = empty_shape(8, ShapeResult::Scalar(0));
    duplicate_close.inputs = vec![ShapeLogicalInput {
        ordinal: 0,
        kind: 0,
        resolution: ShapeLogicalInputResolution::Resolved(3),
    }];
    duplicate_close.retired = vec![3];
    shapes.insert(4, duplicate);
    shapes.insert(5, duplicate_close);
    assert!(validate_first_rung(&shapes).is_ok());

    let mut wrong_source = shapes.clone();
    wrong_source[4].output.as_mut().unwrap().source = 0;
    assert!(validate_first_rung(&wrong_source).is_err());

    let mut wrong_source_identity = shapes.clone();
    wrong_source_identity[4]
        .output
        .as_mut()
        .unwrap()
        .source_identity = Some(9);
    assert!(validate_first_rung(&wrong_source_identity).is_err());

    let mut wrong_result = shapes.clone();
    wrong_result[4].result = ShapeResult::Handle(4);
    assert!(validate_first_rung(&wrong_result).is_err());

    let mut failed = shapes.clone();
    failed[4].result = ShapeResult::Scalar(-1);
    failed[4].post_error = 9;
    assert!(validate_first_rung(&failed).is_err());

    let mut wrong_close_lineage = shapes.clone();
    wrong_close_lineage[5].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(2);
    assert!(validate_first_rung(&wrong_close_lineage).is_err());

    let mut missing_close = shapes;
    missing_close.remove(5);
    assert!(validate_first_rung(&missing_close).is_err());
}

#[test]
fn output_set_length_requires_exact_nonnegative_length_and_lineage() {
    let mut shapes = exact_input_output_shapes();
    let mut set_length = empty_shape(41, ShapeResult::Scalar(0));
    set_length.scalars = vec![(1, ShapeScalar::I64(3))];
    set_length.inputs = shapes[4].inputs.clone();
    shapes.insert(5, set_length);
    assert!(validate_first_rung(&shapes).is_ok());

    let mut negative = shapes.clone();
    negative[5].scalars = vec![(1, ShapeScalar::I64(-1))];
    assert!(validate_first_rung(&negative).is_err());

    let mut wrong_ordinal = shapes.clone();
    wrong_ordinal[5].scalars = vec![(0, ShapeScalar::I64(3))];
    assert!(validate_first_rung(&wrong_ordinal).is_err());

    let mut wrong_descriptor = shapes;
    wrong_descriptor[5].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
    assert!(validate_first_rung(&wrong_descriptor).is_err());
}

#[test]
fn output_set_file_permissions_requires_exact_success_mode_and_lineage() {
    let mut shapes = exact_input_output_shapes();
    let mut permissions = empty_shape(17, ShapeResult::Scalar(0));
    permissions.scalars = vec![(1, ShapeScalar::U32(0o755))];
    permissions.inputs = shapes[4].inputs.clone();
    shapes.insert(5, permissions);
    assert!(validate_first_rung(&shapes).is_ok());

    let mut failed = shapes.clone();
    failed[5].result = ShapeResult::Scalar(-1);
    assert!(validate_first_rung(&failed).is_err());

    let mut wrong_type = shapes.clone();
    wrong_type[5].scalars = vec![(1, ShapeScalar::I32(0o755))];
    assert!(validate_first_rung(&wrong_type).is_err());

    let mut wrong_ordinal = shapes.clone();
    wrong_ordinal[5].scalars = vec![(0, ShapeScalar::U32(0o755))];
    assert!(validate_first_rung(&wrong_ordinal).is_err());

    let mut wrong_descriptor = shapes;
    wrong_descriptor[5].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
    assert!(validate_first_rung(&wrong_descriptor).is_err());
}

#[test]
fn output_set_file_times_requires_exact_unchanged_carrier_and_lineage() {
    const TIMES: [u8; 32] = [7; 32];
    const CHANGED_TIMES: [u8; 32] = [8; 32];
    const SHORT_TIMES: [u8; 31] = [7; 31];

    let mut shapes = exact_input_output_shapes();
    let mut times = empty_shape(42, ShapeResult::Scalar(0));
    times.mutable_byte_resolutions = vec![(1, &TIMES)];
    times.mutable_bytes = vec![ShapeMutableBytes {
        ordinal: 1,
        pre: &TIMES,
        post: &TIMES,
    }];
    times.inputs = shapes[4].inputs.clone();
    shapes.insert(5, times);
    assert!(validate_first_rung(&shapes).is_ok());

    let mut failed = shapes.clone();
    failed[5].result = ShapeResult::Scalar(-1);
    assert!(validate_first_rung(&failed).is_err());

    let mut changed_post = shapes.clone();
    changed_post[5].mutable_bytes[0].post = &CHANGED_TIMES;
    assert!(validate_first_rung(&changed_post).is_err());

    let mut wrong_ordinal = shapes.clone();
    wrong_ordinal[5].mutable_byte_resolutions[0].0 = 0;
    assert!(validate_first_rung(&wrong_ordinal).is_err());

    let mut short = shapes.clone();
    short[5].mutable_byte_resolutions[0].1 = &SHORT_TIMES;
    short[5].mutable_bytes[0].pre = &SHORT_TIMES;
    short[5].mutable_bytes[0].post = &SHORT_TIMES;
    assert!(validate_first_rung(&short).is_err());

    let mut wrong_descriptor = shapes;
    wrong_descriptor[5].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
    assert!(validate_first_rung(&wrong_descriptor).is_err());
}

#[test]
fn output_seek_requires_exact_recomputed_result_and_lineage() {
    let mut shapes = exact_input_output_shapes();
    let mut seek = empty_shape(10, ShapeResult::Scalar(5));
    seek.scalars = vec![(1, ShapeScalar::I64(-2)), (2, ShapeScalar::I32(2))];
    seek.inputs = shapes[4].inputs.clone();
    shapes.insert(5, seek);
    assert!(validate_first_rung(&shapes).is_ok());

    let mut wrong_result = shapes.clone();
    wrong_result[5].result = ShapeResult::Scalar(4);
    assert!(validate_first_rung(&wrong_result).is_err());

    let mut bad_whence = shapes.clone();
    bad_whence[5].scalars[1] = (2, ShapeScalar::I32(9));
    assert!(validate_first_rung(&bad_whence).is_err());

    let mut wrong_descriptor = shapes;
    wrong_descriptor[5].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
    assert!(validate_first_rung(&wrong_descriptor).is_err());
}

#[test]
fn descriptor_metadata_chain_validates_exact_kind_lineage_and_retirement() {
    let mut shapes = exact_descriptor_metadata_shapes();
    assert!(validate_first_rung(&shapes).is_ok());

    shapes[1].metadata[0].kind = 0;
    assert!(validate_first_rung(&shapes).is_err());

    let mut shapes = exact_descriptor_metadata_shapes();
    shapes[1].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
    assert!(validate_first_rung(&shapes).is_err());

    let mut shapes = exact_descriptor_metadata_shapes();
    shapes.remove(2);
    assert!(validate_first_rung(&shapes).is_err());
}

static NATIVE_QUERY_FINAL_PATH: &[u8] = b"C:\\pkg\\main.omg";
static NATIVE_QUERY_CARRIER: [u8; 16] = [0; 16];
static NATIVE_QUERY_POST: &[u8] = b"C:\\pkg\\main.omg\0";

fn native_query_chain_shapes() -> Vec<AttemptShape<'static>> {
    let mut open = empty_shape(28, ShapeResult::Handle(7));
    open.scalars = vec![
        (1, ShapeScalar::U32(0)),
        (2, ShapeScalar::U32(0x7)),
        (3, ShapeScalar::I64(0)),
        (4, ShapeScalar::U32(3)),
        (5, ShapeScalar::U32(0x0200_0000)),
    ];
    open.rooted_paths = vec![ShapeRootedPath {
        ordinal: 0,
        root: 0,
        bytes: b"pkg/main.omg",
    }];
    open.authorized_paths = vec![ShapeAuthorizedPath {
        ordinal: 0,
        access: 0,
        root: 0,
        bytes: b"pkg/main.omg",
    }];
    open.inputs = vec![ShapeLogicalInput {
        ordinal: 6,
        kind: 1,
        resolution: ShapeLogicalInputResolution::Null,
    }];
    open.output = Some(ShapeLogicalOutput {
        kind: 1,
        identity: 7,
        source: 0,
        source_identity: None,
    });

    let mut query = empty_shape(31, ShapeResult::Scalar(15));
    query.scalars = vec![(2, ShapeScalar::U64(16)), (3, ShapeScalar::U32(0))];
    query.returned_paths = vec![ShapeReturnedPath {
        ordinal: 1,
        kind: 2,
        completeness: 0,
        bytes: NATIVE_QUERY_FINAL_PATH,
    }];
    query.mutable_byte_resolutions = vec![(1, &NATIVE_QUERY_CARRIER)];
    query.mutable_bytes = vec![ShapeMutableBytes {
        ordinal: 1,
        pre: &NATIVE_QUERY_CARRIER,
        post: NATIVE_QUERY_POST,
    }];
    query.inputs = vec![ShapeLogicalInput {
        ordinal: 0,
        kind: 1,
        resolution: ShapeLogicalInputResolution::Resolved(7),
    }];

    let mut close = empty_shape(29, ShapeResult::Scalar(1));
    close.inputs = query.inputs.clone();
    close.retired = vec![7];

    vec![open, query, close]
}

#[test]
fn native_query_chain_validates_exact_acquisition_queries_and_release() {
    let shapes = native_query_chain_shapes();
    assert!(validate_first_rung(&shapes).is_ok());

    // A native-handle failure suffix composes after the closed chain.
    let mut with_failure = native_query_chain_shapes();
    let mut failure = empty_shape(29, ShapeResult::Scalar(0));
    failure.post_error = 6;
    failure.inputs = vec![ShapeLogicalInput {
        ordinal: 0,
        kind: 1,
        resolution: ShapeLogicalInputResolution::Unknown,
    }];
    with_failure.push(failure);
    assert!(validate_first_rung(&with_failure).is_ok());

    // The constrained acquisition contract admits no Output root, no
    // write authorization, and no deferred-deletion flag.
    let mut wrong_root = shapes.clone();
    wrong_root[0].rooted_paths[0].root = 1;
    assert!(validate_first_rung(&wrong_root).is_err());

    let mut wrong_access = shapes.clone();
    wrong_access[0].authorized_paths[0].access = 1;
    assert!(validate_first_rung(&wrong_access).is_err());

    let mut delete_on_close = shapes.clone();
    delete_on_close[0].scalars[4] = (5, ShapeScalar::U32(0x0600_0000));
    assert!(validate_first_rung(&delete_on_close).is_err());

    let mut descriptor_output = shapes.clone();
    descriptor_output[0].output.as_mut().unwrap().kind = 0;
    assert!(validate_first_rung(&descriptor_output).is_err());

    let mut borrowed_template = shapes.clone();
    borrowed_template[0].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
    assert!(validate_first_rung(&borrowed_template).is_err());

    // A chain needs at least one final-path observation on the identity.
    let mut error_only = vec![
        shapes[0].clone(),
        empty_shape(35, ShapeResult::Scalar(0)),
        shapes[2].clone(),
    ];
    assert!(validate_first_rung(&error_only).is_err());
    error_only[1].post_error = 5;
    error_only[1].result = ShapeResult::Scalar(5);
    assert!(validate_first_rung(&error_only).is_err());

    let mut substituted = shapes.clone();
    substituted[1].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
    assert!(validate_first_rung(&substituted).is_err());

    let mut wrong_kind = shapes.clone();
    wrong_kind[1].inputs[0].kind = 0;
    assert!(validate_first_rung(&wrong_kind).is_err());

    let mut wrong_result = shapes.clone();
    wrong_result[1].result = ShapeResult::Scalar(16);
    assert!(validate_first_rung(&wrong_result).is_err());

    static TAMPERED_POST: &[u8] = b"C:\\pkg\\main.omg\x01";
    let mut tampered_post = shapes.clone();
    tampered_post[1].mutable_bytes[0].post = TAMPERED_POST;
    assert!(validate_first_rung(&tampered_post).is_err());

    let mut failed_close = shapes.clone();
    failed_close[2].result = ShapeResult::Scalar(0);
    failed_close[2].post_error = 6;
    assert!(validate_first_rung(&failed_close).is_err());

    let mut unretired = shapes.clone();
    unretired[2].retired.clear();
    assert!(validate_first_rung(&unretired).is_err());

    let mut missing_close = shapes.clone();
    missing_close.pop();
    assert!(validate_first_rung(&missing_close).is_err());

    let mut late_use = shapes.clone();
    late_use.push(empty_shape(35, ShapeResult::Scalar(0)));
    assert!(validate_first_rung(&late_use).is_err());

    let mut reused_identity = native_query_chain_shapes();
    reused_identity.extend(native_query_chain_shapes());
    assert!(validate_first_rung(&reused_identity).is_err());
}

#[test]
fn canonical_source_metadata_identity_round_trips_and_rejects_unknown_tags() {
    let expected = BuildCanonicalSourceMetadataIdentity::new(7, [0xa5; 32]);
    let mut encoder = Encoder::new(64);
    encoder.byte(1);
    encoder.u32(expected.policy_version());
    encoder.fixed(&expected.source_content_commitment());
    let bytes = encoder.finish().expect("encode metadata identity");
    let mut decoder = Decoder::new(&bytes, BuildFilesystemReplayRecordLimits::new(64, 1));
    assert_eq!(
        decode_canonical_source_metadata_identity(&mut decoder).expect("decode metadata identity"),
        Some(expected)
    );
    decoder.finish().expect("consume exact identity bytes");

    let mut decoder = Decoder::new(&[2], BuildFilesystemReplayRecordLimits::new(1, 1));
    assert!(decode_canonical_source_metadata_identity(&mut decoder).is_err());
}
