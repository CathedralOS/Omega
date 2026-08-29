use super::*;

static PRE_STATE: [u8; 8] = *b"........";
static POST_STATE: [u8; 8] = *b"target..";
static WRONG_PRE_STATE: [u8; 8] = *b"changed!";
static MUTATED_TAIL: [u8; 8] = *b"target!?";

fn exact_read_link_shape() -> AttemptShape<'static> {
    AttemptShape {
        operation: 21,
        provider: 2,
        result: ShapeResult::Scalar(6),
        post_error: 0,
        scalars: vec![(2, ShapeScalar::U64(8))],
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_paths: vec![ShapeRootedPath {
            ordinal: 0,
            root: 0,
            bytes: b"links/tool",
        }],
        returned_paths: vec![ShapeReturnedPath {
            ordinal: 1,
            kind: 0,
            completeness: 0,
            bytes: b"target",
        }],
        returned_path_count: 1,
        observed_regions: Vec::new(),
        metadata: Vec::new(),
        mutable_byte_resolutions: vec![(1, &PRE_STATE)],
        mutable_i64_resolution_count: 0,
        mutable_bytes: vec![ShapeMutableBytes {
            ordinal: 1,
            pre: &PRE_STATE,
            post: &POST_STATE,
        }],
        mutable_i64_count: 0,
        authorized_paths: vec![ShapeAuthorizedPath {
            ordinal: 0,
            access: 0,
            root: 0,
            bytes: b"links/tool",
        }],
        inputs: Vec::new(),
        output: None,
        retired: Vec::new(),
        refusal_count: 0,
    }
}

#[test]
fn source_read_link_is_one_complete_source_event() {
    let shape = exact_read_link_shape();
    validate_source_read_link_shape(&shape).expect("exact read_link shape");
    validate_first_rung(&[shape]).expect("read_link counts as one Source event");
}

#[test]
fn source_read_link_rehydrates_the_exact_attempt() {
    let shape = exact_read_link_shape();
    let record = rehydrate_source_read_link_shape(&shape).expect("rehydrate read_link");
    let source = psi_checked_interpreter::FilesystemSourceInputReplayRecord::new(vec![
        psi_checked_interpreter::FilesystemSourceInputReplayEventRecord::ReadLink(record),
    ])
    .expect("typed Source event");
    let replay = psi_checked_interpreter::FilesystemReplay::from_source_input_record(source)
        .expect("exact typed replay");
    let [attempt] = replay.attempts() else {
        panic!("read_link rehydrates as exactly one attempt")
    };
    assert_eq!(attempt.operation_tag(), 21);
    assert_eq!(
        attempt.result(),
        Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
            6
        ))
    );
    assert_eq!(attempt.returned_paths()[0].bytes(), b"target");
    assert_eq!(attempt.mutable_byte_operands()[0].post_bytes(), &POST_STATE);
}

#[test]
fn source_read_link_accepts_only_exact_truncation_evidence() {
    static TRUNCATED_POST: [u8; 8] = *b"targ....";
    let mut shape = exact_read_link_shape();
    shape.result = ShapeResult::Scalar(4);
    shape.scalars = vec![(2, ShapeScalar::U64(4))];
    shape.returned_paths[0].completeness = 1;
    shape.returned_paths[0].bytes = b"targ";
    shape.mutable_bytes[0].post = &TRUNCATED_POST;
    validate_source_read_link_shape(&shape).expect("exact limited read_link");

    shape.scalars = vec![(2, ShapeScalar::U64(5))];
    assert!(validate_source_read_link_shape(&shape).is_err());
}

#[test]
fn source_read_link_rejects_spoofed_structural_lanes() {
    let mut malformed = exact_read_link_shape();
    malformed.provider = 1;
    assert!(validate_source_read_link_shape(&malformed).is_err());

    let mut malformed = exact_read_link_shape();
    malformed.scalars.push((3, ShapeScalar::U64(0)));
    assert!(validate_source_read_link_shape(&malformed).is_err());

    let mut malformed = exact_read_link_shape();
    malformed.rooted_paths[0].root = 1;
    assert!(validate_source_read_link_shape(&malformed).is_err());

    let mut malformed = exact_read_link_shape();
    malformed.returned_paths[0].kind = 1;
    assert!(validate_source_read_link_shape(&malformed).is_err());

    let mut malformed = exact_read_link_shape();
    malformed.authorized_paths[0].access = 1;
    assert!(validate_source_read_link_shape(&malformed).is_err());

    let mut malformed = exact_read_link_shape();
    malformed.metadata.push(ShapeMetadata {
        ordinal: 1,
        kind: 0,
        device: 0,
        mode: 0,
        link_count: 0,
        inode: 0,
        user: 0,
        group: 0,
        referenced_device: 0,
        access_time: 0,
        modification_time: 0,
        change_time: 0,
        birth_time: 0,
        size: 0,
        blocks_512: 0,
        preferred_block_size: 0,
    });
    assert!(validate_source_read_link_shape(&malformed).is_err());
}

#[test]
fn source_read_link_rejects_incoherent_result_and_carrier_evidence() {
    let mut malformed = exact_read_link_shape();
    malformed.result = ShapeResult::Scalar(5);
    assert!(validate_source_read_link_shape(&malformed).is_err());

    let mut malformed = exact_read_link_shape();
    malformed.mutable_byte_resolutions[0].1 = &WRONG_PRE_STATE;
    assert!(validate_source_read_link_shape(&malformed).is_err());

    let mut malformed = exact_read_link_shape();
    malformed.mutable_bytes[0].post = &MUTATED_TAIL;
    assert!(validate_source_read_link_shape(&malformed).is_err());

    let mut malformed = exact_read_link_shape();
    malformed.returned_paths[0].bytes = b"tar\0et";
    assert!(validate_source_read_link_shape(&malformed).is_err());
}
