use super::{
    assert_diagnostic, depth_eight_nested_source, depth_eleven_source_with,
    depth_nine_nested_source, depth_seven_nested_source, depth_six_nested_source,
    depth_ten_nested_source, depth_twelve_source_with, write_program,
};
use compiler::{CheckedCompileRequest, compile_to_checked};

#[test]
fn source_placement_custody_rejects_a_zero_layout_depth_six_wrapper() {
    let source = depth_six_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    frame: ChestCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-six-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout sixth wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame",
            "outside the exact twenty-four-record",
        ],
    );
}

#[test]
fn source_placement_custody_accepts_seven_nested_projection_record_paths() {
    let source = depth_seven_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    frame: VaultCustody;",
    );
    let main = write_program("depth-seven-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-seven placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_a_missing_depth_seven_projection() {
    let source = depth_seven_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "",
    );
    let main = write_program("depth-seven-hidden", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("missing seventh-level custody projection must fail closed");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame.chest.item.boxed.frame.envelope.header.authority",
            "omits canonical field path",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_depth_seven_projection_drift() {
    for (name, header, envelope, frame, boxed, crate_fields, chest, vault, packet, expected) in [
        (
            "depth-seven-missing-leaf",
            "",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    frame: VaultCustody;",
            vec![
                "Packet.frame.chest.item.boxed.frame.envelope.header.authority",
                "omits canonical field path",
            ],
        ),
        (
            "depth-seven-missing-inner-projection",
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "",
            "    frame: VaultCustody;",
            vec![
                "Packet.frame.chest.item.boxed.frame.envelope.header.authority",
                "omits canonical field path",
            ],
        ),
        (
            "depth-seven-cross-sibling",
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    sibling: VaultCustody;",
            vec![
                "Packet.frame.chest.item.boxed.frame.envelope.header.authority",
                "Packet.sibling",
            ],
        ),
        (
            "depth-seven-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    frame: VaultCustody;",
            vec![
                "Packet.frame.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-seven-wrong-type",
            "    authority: OtherEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    frame: VaultCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-seven-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    frame: VaultCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(
            name,
            &depth_seven_nested_source(
                header,
                envelope,
                frame,
                boxed,
                crate_fields,
                chest,
                vault,
                packet,
            ),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-seven custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_rejects_a_zero_layout_depth_seven_wrapper() {
    let source = depth_seven_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    frame: VaultCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-seven-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout seventh wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame",
            "outside the exact twenty-four-record",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_a_depth_seven_back_edge() {
    let source = depth_seven_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    frame: VaultCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Vault;",
        1,
    );
    let main = write_program("depth-seven-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a seventh-level back-edge must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Placed<Native,Packet>",
            "Packet",
            "field `frame`",
            "neither a supported primitive",
        ],
    );
}

#[test]
fn source_placement_custody_accepts_eight_nested_projection_record_paths() {
    let source = depth_eight_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    vault: VaultCustody;",
        "    frame: StrongboxCustody;",
    );
    let main = write_program("depth-eight-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-eight placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_a_missing_depth_eight_projection() {
    let source = depth_eight_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    vault: VaultCustody;",
        "",
    );
    let main = write_program("depth-eight-hidden", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("missing eighth-level custody projection must fail closed");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame.vault.chest.item.boxed.frame.envelope.header.authority",
            "omits canonical field path",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_depth_eight_projection_drift() {
    for (
        name,
        header,
        envelope,
        frame,
        boxed,
        crate_fields,
        chest,
        vault,
        strongbox,
        packet,
        expected,
    ) in [
        (
            "depth-eight-missing-leaf",
            "",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    frame: StrongboxCustody;",
            vec![
                "Packet.frame.vault.chest.item.boxed.frame.envelope.header.authority",
                "omits canonical field path",
            ],
        ),
        (
            "depth-eight-missing-inner-projection",
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "",
            "    frame: StrongboxCustody;",
            vec![
                "Packet.frame.vault.chest.item.boxed.frame.envelope.header.authority",
                "omits canonical field path",
            ],
        ),
        (
            "depth-eight-cross-sibling",
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    sibling: StrongboxCustody;",
            vec![
                "Packet.frame.vault.chest.item.boxed.frame.envelope.header.authority",
                "Packet.sibling",
            ],
        ),
        (
            "depth-eight-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    frame: StrongboxCustody;",
            vec![
                "Packet.frame.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-eight-wrong-type",
            "    authority: OtherEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    frame: StrongboxCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-eight-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    frame: StrongboxCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(
            name,
            &depth_eight_nested_source(
                header,
                envelope,
                frame,
                boxed,
                crate_fields,
                chest,
                vault,
                strongbox,
                packet,
            ),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-eight custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_rejects_a_zero_layout_depth_eight_wrapper() {
    let source = depth_eight_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    vault: VaultCustody;",
        "    frame: StrongboxCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-eight-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout eighth wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame",
            "outside the exact twenty-four-record",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_a_depth_eight_back_edge() {
    let source = depth_eight_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    vault: VaultCustody;",
        "    frame: StrongboxCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Strongbox;",
        1,
    );
    let main = write_program("depth-eight-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("an eighth-level back-edge must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Placed<Native,Packet>",
            "Packet",
            "field `frame`",
            "neither a supported primitive",
        ],
    );
}

#[test]
fn source_placement_custody_accepts_nine_nested_projection_record_paths() {
    let source = depth_nine_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    vault: VaultCustody;",
        "    strongbox: StrongboxCustody;",
        "    frame: LockboxCustody;",
    );
    let main = write_program("depth-nine-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-nine placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_a_missing_depth_nine_projection() {
    let source = depth_nine_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    vault: VaultCustody;",
        "    strongbox: StrongboxCustody;",
        "",
    );
    let main = write_program("depth-nine-hidden", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("missing ninth-level custody projection must fail closed");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame.strongbox.vault.chest.item.boxed.frame.envelope.header.authority",
            "omits canonical field path",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_depth_nine_projection_drift() {
    for (
        name,
        header,
        envelope,
        frame,
        boxed,
        crate_fields,
        chest,
        vault,
        strongbox,
        lockbox,
        packet,
        expected,
    ) in [
        (
            "depth-nine-missing-leaf",
            "",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    strongbox: StrongboxCustody;",
            "    frame: LockboxCustody;",
            vec![
                "Packet.frame.strongbox.vault.chest.item.boxed.frame.envelope.header.authority",
                "omits canonical field path",
            ],
        ),
        (
            "depth-nine-missing-inner-projection",
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "",
            "    strongbox: StrongboxCustody;",
            "    frame: LockboxCustody;",
            vec![
                "Packet.frame.strongbox.vault.chest.item.boxed.frame.envelope.header.authority",
                "omits canonical field path",
            ],
        ),
        (
            "depth-nine-cross-sibling",
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    strongbox: StrongboxCustody;",
            "    sibling: LockboxCustody;",
            vec![
                "Packet.frame.strongbox.vault.chest.item.boxed.frame.envelope.header.authority",
                "Packet.sibling",
            ],
        ),
        (
            "depth-nine-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    strongbox: StrongboxCustody;",
            "    frame: LockboxCustody;",
            vec![
                "Packet.frame.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-nine-wrong-type",
            "    authority: OtherEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    strongbox: StrongboxCustody;",
            "    frame: LockboxCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-nine-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    strongbox: StrongboxCustody;",
            "    frame: LockboxCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(
            name,
            &depth_nine_nested_source(
                header,
                envelope,
                frame,
                boxed,
                crate_fields,
                chest,
                vault,
                strongbox,
                lockbox,
                packet,
            ),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-nine custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_rejects_a_zero_layout_depth_nine_wrapper() {
    let source = depth_nine_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    vault: VaultCustody;",
        "    strongbox: StrongboxCustody;",
        "    frame: LockboxCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-nine-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout ninth wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame",
            "outside the exact twenty-four-record",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_a_depth_nine_back_edge() {
    let source = depth_nine_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    vault: VaultCustody;",
        "    strongbox: StrongboxCustody;",
        "    frame: LockboxCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Lockbox;",
        1,
    );
    let main = write_program("depth-nine-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a ninth-level back-edge must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Placed<Native,Packet>",
            "Packet",
            "field `frame`",
            "neither a supported primitive",
        ],
    );
}

#[test]
fn source_placement_custody_accepts_ten_nested_projection_record_paths() {
    let source = depth_ten_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    vault: VaultCustody;",
        "    strongbox: StrongboxCustody;",
        "    lockbox: LockboxCustody;",
        "    frame: CofferCustody;",
    );
    let main = write_program("depth-ten-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-ten placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_a_missing_depth_ten_projection() {
    let source = depth_ten_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    vault: VaultCustody;",
        "    strongbox: StrongboxCustody;",
        "    lockbox: LockboxCustody;",
        "",
    );
    let main = write_program("depth-ten-hidden", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("missing tenth-level custody projection must fail closed");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority",
            "omits canonical field path",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_depth_ten_projection_drift() {
    for (
        name,
        header,
        envelope,
        frame,
        boxed,
        crate_fields,
        chest,
        vault,
        strongbox,
        lockbox,
        coffer,
        packet,
        expected,
    ) in [
        (
            "depth-ten-missing-leaf",
            "",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    strongbox: StrongboxCustody;",
            "    lockbox: LockboxCustody;",
            "    frame: CofferCustody;",
            vec![
                "Packet.frame.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority",
                "omits canonical field path",
            ],
        ),
        (
            "depth-ten-missing-inner-projection",
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    strongbox: StrongboxCustody;",
            "",
            "    frame: CofferCustody;",
            vec![
                "Packet.frame.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority",
                "omits canonical field path",
            ],
        ),
        (
            "depth-ten-cross-sibling",
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    strongbox: StrongboxCustody;",
            "    lockbox: LockboxCustody;",
            "    sibling: CofferCustody;",
            vec![
                "Packet.frame.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority",
                "Packet.sibling",
            ],
        ),
        (
            "depth-ten-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    strongbox: StrongboxCustody;",
            "    lockbox: LockboxCustody;",
            "    frame: CofferCustody;",
            vec![
                "Packet.frame.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-ten-wrong-type",
            "    authority: OtherEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    strongbox: StrongboxCustody;",
            "    lockbox: LockboxCustody;",
            "    frame: CofferCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-ten-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    chest: ChestCustody;",
            "    vault: VaultCustody;",
            "    strongbox: StrongboxCustody;",
            "    lockbox: LockboxCustody;",
            "    frame: CofferCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(
            name,
            &depth_ten_nested_source(
                header,
                envelope,
                frame,
                boxed,
                crate_fields,
                chest,
                vault,
                strongbox,
                lockbox,
                coffer,
                packet,
            ),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-ten custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_rejects_a_zero_layout_depth_ten_wrapper() {
    let source = depth_ten_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    vault: VaultCustody;",
        "    strongbox: StrongboxCustody;",
        "    lockbox: LockboxCustody;",
        "    frame: CofferCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-ten-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout tenth wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame",
            "outside the exact twenty-four-record",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_a_depth_ten_back_edge() {
    let source = depth_ten_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    vault: VaultCustody;",
        "    strongbox: StrongboxCustody;",
        "    lockbox: LockboxCustody;",
        "    frame: CofferCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Coffer;",
        1,
    );
    let main = write_program("depth-ten-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a tenth-level back-edge must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Placed<Native,Packet>",
            "Packet",
            "field `frame`",
            "neither a supported primitive",
        ],
    );
}

#[test]
fn source_placement_custody_accepts_eleven_nested_projection_record_paths() {
    let source = depth_eleven_source_with(
        "    authority: Evidence;",
        "    coffer: CofferCustody;",
        "    frame: CasketCustody;",
    );
    let main = write_program("depth-eleven-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-eleven placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_depth_eleven_projection_drift() {
    const LEAF_PATH: &str = "Packet.frame.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    for (name, header, casket, packet, expected) in [
        (
            "depth-eleven-missing-leaf",
            "",
            "    coffer: CofferCustody;",
            "    frame: CasketCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-eleven-missing-inner-projection",
            "    authority: Evidence;",
            "",
            "    frame: CasketCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-eleven-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    coffer: CofferCustody;",
            "    frame: CasketCustody;",
            vec![
                "Packet.frame.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-eleven-wrong-type",
            "    authority: OtherEvidence;",
            "    coffer: CofferCustody;",
            "    frame: CasketCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-eleven-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    coffer: CofferCustody;",
            "    frame: CasketCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(name, &depth_eleven_source_with(header, casket, packet));
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-eleven custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_preserves_depth_first_diagnostic_order_at_depth_eleven() {
    const LEAF_PATH: &str = "Packet.frame.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    let source = depth_eleven_source_with(
        "    authority: Evidence;",
        "    sibling: CofferCustody;",
        "    frame: CasketCustody;",
    );
    let main = write_program("depth-eleven-cross-sibling", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a cross-sibling depth-eleven projection must fail closed");
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(messages.len(), 2, "unexpected diagnostics: {messages:#?}");
    assert!(
        messages[0].contains(LEAF_PATH) && messages[0].contains("omits canonical field path"),
        "the missing depth-first leaf must be diagnosed first: {messages:#?}"
    );
    assert!(
        messages[1].contains("Packet.frame.sibling")
            && messages[1].contains("extra canonical field path"),
        "the extra sibling must be diagnosed after the missing subtree: {messages:#?}"
    );
}

#[test]
fn source_placement_custody_rejects_a_zero_layout_depth_eleven_wrapper() {
    let source = depth_eleven_source_with(
        "    authority: Evidence;",
        "    coffer: CofferCustody;",
        "    frame: CasketCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-eleven-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout eleventh wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame",
            "outside the exact twenty-four-record",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_a_depth_eleven_back_edge() {
    let source = depth_eleven_source_with(
        "    authority: Evidence;",
        "    coffer: CofferCustody;",
        "    frame: CasketCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Casket;",
        1,
    );
    let main = write_program("depth-eleven-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("an eleventh-level back-edge must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Placed<Native,Packet>",
            "Packet",
            "field `frame`",
            "neither a supported primitive",
        ],
    );
}

#[test]
fn source_placement_custody_accepts_twelve_nested_projection_record_paths() {
    let source = depth_twelve_source_with(
        "    authority: Evidence;",
        "    casket: CasketCustody;",
        "    frame: ReliquaryCustody;",
    );
    let main = write_program("depth-twelve-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-twelve placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_depth_twelve_projection_drift() {
    const LEAF_PATH: &str = "Packet.frame.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    for (name, header, reliquary, packet, expected) in [
        (
            "depth-twelve-missing-leaf",
            "",
            "    casket: CasketCustody;",
            "    frame: ReliquaryCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-twelve-missing-inner-projection",
            "    authority: Evidence;",
            "",
            "    frame: ReliquaryCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-twelve-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    casket: CasketCustody;",
            "    frame: ReliquaryCustody;",
            vec![
                "Packet.frame.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-twelve-wrong-type",
            "    authority: OtherEvidence;",
            "    casket: CasketCustody;",
            "    frame: ReliquaryCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-twelve-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    casket: CasketCustody;",
            "    frame: ReliquaryCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(name, &depth_twelve_source_with(header, reliquary, packet));
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-twelve custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_preserves_depth_first_diagnostic_order_at_depth_twelve() {
    const LEAF_PATH: &str = "Packet.frame.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    let source = depth_twelve_source_with(
        "    authority: Evidence;",
        "    sibling: CasketCustody;",
        "    frame: ReliquaryCustody;",
    );
    let main = write_program("depth-twelve-cross-sibling", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a cross-sibling depth-twelve projection must fail closed");
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(messages.len(), 2, "unexpected diagnostics: {messages:#?}");
    assert!(
        messages[0].contains(LEAF_PATH) && messages[0].contains("omits canonical field path"),
        "the missing depth-first leaf must be diagnosed first: {messages:#?}"
    );
    assert!(
        messages[1].contains("Packet.frame.sibling")
            && messages[1].contains("extra canonical field path"),
        "the extra sibling must be diagnosed after the missing subtree: {messages:#?}"
    );
}

#[test]
fn source_placement_custody_rejects_a_zero_layout_depth_twelve_wrapper() {
    let source = depth_twelve_source_with(
        "    authority: Evidence;",
        "    casket: CasketCustody;",
        "    frame: ReliquaryCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-twelve-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout twelfth wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame",
            "outside the exact twenty-four-record",
        ],
    );
}
