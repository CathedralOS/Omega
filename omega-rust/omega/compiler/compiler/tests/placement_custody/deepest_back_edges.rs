use super::{
    assert_diagnostic, depth_nineteen_source_with, depth_three_nested_source,
    depth_twenty_four_source_with, depth_twenty_one_source_with, depth_twenty_source_with,
    depth_twenty_three_source_with, depth_twenty_two_source_with, write_program,
};
use compiler::{CheckedCompileRequest, compile_to_checked};

#[test]
fn source_placement_custody_rejects_a_depth_nineteen_back_edge() {
    let source = depth_nineteen_source_with(
        "    authority: Evidence;",
        "    cathedral: CathedralCustody;",
        "    frame: AbbeyCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Abbey;",
        1,
    );
    let main = write_program("depth-nineteen-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a nineteenth-level back-edge must remain outside the custody cohort");
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
fn source_placement_custody_accepts_twenty_nested_projection_record_paths() {
    let source = depth_twenty_source_with(
        "    authority: Evidence;",
        "    abbey: AbbeyCustody;",
        "    frame: MonasteryCustody;",
    );
    let main = write_program("depth-twenty-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-twenty placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_depth_twenty_projection_drift() {
    const LEAF_PATH: &str = "Packet.frame.abbey.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    for (name, header, monastery, packet, expected) in [
        (
            "depth-twenty-missing-leaf",
            "",
            "    abbey: AbbeyCustody;",
            "    frame: MonasteryCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-twenty-missing-inner-projection",
            "    authority: Evidence;",
            "",
            "    frame: MonasteryCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-twenty-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    abbey: AbbeyCustody;",
            "    frame: MonasteryCustody;",
            vec![
                "Packet.frame.abbey.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-twenty-wrong-type",
            "    authority: OtherEvidence;",
            "    abbey: AbbeyCustody;",
            "    frame: MonasteryCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-twenty-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    abbey: AbbeyCustody;",
            "    frame: MonasteryCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(name, &depth_twenty_source_with(header, monastery, packet));
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-twenty custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_preserves_depth_first_diagnostic_order_at_depth_twenty() {
    const LEAF_PATH: &str = "Packet.frame.abbey.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    let source = depth_twenty_source_with(
        "    authority: Evidence;",
        "    sibling: AbbeyCustody;",
        "    frame: MonasteryCustody;",
    );
    let main = write_program("depth-twenty-cross-sibling", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a cross-sibling depth-twenty projection must fail closed");
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
fn source_placement_custody_rejects_a_zero_layout_depth_twenty_wrapper() {
    let source = depth_twenty_source_with(
        "    authority: Evidence;",
        "    abbey: AbbeyCustody;",
        "    frame: MonasteryCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-twenty-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout twentieth wrapper must remain outside the custody cohort");
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
fn source_placement_custody_rejects_a_depth_twenty_back_edge() {
    let source = depth_twenty_source_with(
        "    authority: Evidence;",
        "    abbey: AbbeyCustody;",
        "    frame: MonasteryCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Monastery;",
        1,
    );
    let main = write_program("depth-twenty-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a twentieth-level back-edge must remain outside the custody cohort");
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
fn source_placement_custody_accepts_twenty_one_nested_projection_record_paths() {
    let source = depth_twenty_one_source_with(
        "    authority: Evidence;",
        "    monastery: MonasteryCustody;",
        "    frame: PrioryCustody;",
    );
    let main = write_program("depth-twenty-one-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-twenty-one placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_depth_twenty_one_projection_drift() {
    const LEAF_PATH: &str = "Packet.frame.monastery.abbey.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    for (name, header, priory, packet, expected) in [
        (
            "depth-twenty-one-missing-leaf",
            "",
            "    monastery: MonasteryCustody;",
            "    frame: PrioryCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-twenty-one-missing-inner-projection",
            "    authority: Evidence;",
            "",
            "    frame: PrioryCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-twenty-one-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    monastery: MonasteryCustody;",
            "    frame: PrioryCustody;",
            vec![
                "Packet.frame.monastery.abbey.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-twenty-one-wrong-type",
            "    authority: OtherEvidence;",
            "    monastery: MonasteryCustody;",
            "    frame: PrioryCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-twenty-one-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    monastery: MonasteryCustody;",
            "    frame: PrioryCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(name, &depth_twenty_one_source_with(header, priory, packet));
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-twenty-one custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_preserves_depth_first_diagnostic_order_at_depth_twenty_one() {
    const LEAF_PATH: &str = "Packet.frame.monastery.abbey.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    let source = depth_twenty_one_source_with(
        "    authority: Evidence;",
        "    sibling: MonasteryCustody;",
        "    frame: PrioryCustody;",
    );
    let main = write_program("depth-twenty-one-cross-sibling", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a cross-sibling depth-twenty-one projection must fail closed");
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
fn source_placement_custody_rejects_a_zero_layout_depth_twenty_one_wrapper() {
    let source = depth_twenty_one_source_with(
        "    authority: Evidence;",
        "    monastery: MonasteryCustody;",
        "    frame: PrioryCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-twenty-one-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout twenty-first wrapper must remain outside the custody cohort");
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
fn source_placement_custody_rejects_a_depth_twenty_one_back_edge() {
    let source = depth_twenty_one_source_with(
        "    authority: Evidence;",
        "    monastery: MonasteryCustody;",
        "    frame: PrioryCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Priory;",
        1,
    );
    let main = write_program("depth-twenty-one-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a twenty-first-level back-edge must remain outside the custody cohort");
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
fn source_placement_custody_accepts_twenty_two_nested_projection_record_paths() {
    let source = depth_twenty_two_source_with(
        "    authority: Evidence;",
        "    priory: PrioryCustody;",
        "    frame: CloisterCustody;",
    );
    let main = write_program("depth-twenty-two-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-twenty-two placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_depth_twenty_two_projection_drift() {
    const LEAF_PATH: &str = "Packet.frame.priory.monastery.abbey.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    for (name, header, cloister, packet, expected) in [
        (
            "depth-twenty-two-missing-leaf",
            "",
            "    priory: PrioryCustody;",
            "    frame: CloisterCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-twenty-two-missing-inner-projection",
            "    authority: Evidence;",
            "",
            "    frame: CloisterCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-twenty-two-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    priory: PrioryCustody;",
            "    frame: CloisterCustody;",
            vec![
                "Packet.frame.priory.monastery.abbey.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-twenty-two-wrong-type",
            "    authority: OtherEvidence;",
            "    priory: PrioryCustody;",
            "    frame: CloisterCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-twenty-two-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    priory: PrioryCustody;",
            "    frame: CloisterCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(
            name,
            &depth_twenty_two_source_with(header, cloister, packet),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-twenty-two custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_preserves_depth_first_diagnostic_order_at_depth_twenty_two() {
    const LEAF_PATH: &str = "Packet.frame.priory.monastery.abbey.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    let source = depth_twenty_two_source_with(
        "    authority: Evidence;",
        "    sibling: PrioryCustody;",
        "    frame: CloisterCustody;",
    );
    let main = write_program("depth-twenty-two-cross-sibling", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a cross-sibling depth-twenty-two projection must fail closed");
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
fn source_placement_custody_rejects_a_zero_layout_depth_twenty_two_wrapper() {
    let source = depth_twenty_two_source_with(
        "    authority: Evidence;",
        "    priory: PrioryCustody;",
        "    frame: CloisterCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-twenty-two-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout twenty-second wrapper must remain outside the custody cohort");
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
fn source_placement_custody_rejects_a_depth_twenty_two_back_edge() {
    let source = depth_twenty_two_source_with(
        "    authority: Evidence;",
        "    priory: PrioryCustody;",
        "    frame: CloisterCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Cloister;",
        1,
    );
    let main = write_program("depth-twenty-two-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a twenty-second-level back-edge must remain outside the custody cohort");
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
fn source_placement_custody_accepts_twenty_three_nested_projection_record_paths() {
    let source = depth_twenty_three_source_with(
        "    authority: Evidence;",
        "    cloister: CloisterCustody;",
        "    frame: AbbeySeatCustody;",
    );
    let main = write_program("depth-twenty-three-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-twenty-three placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_depth_twenty_three_projection_drift() {
    const LEAF_PATH: &str = "Packet.frame.cloister.priory.monastery.abbey.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    for (name, header, abbey_seat, packet, expected) in [
        (
            "depth-twenty-three-missing-leaf",
            "",
            "    cloister: CloisterCustody;",
            "    frame: AbbeySeatCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-twenty-three-missing-inner-projection",
            "    authority: Evidence;",
            "",
            "    frame: AbbeySeatCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-twenty-three-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    cloister: CloisterCustody;",
            "    frame: AbbeySeatCustody;",
            vec![
                "Packet.frame.cloister.priory.monastery.abbey.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-twenty-three-wrong-type",
            "    authority: OtherEvidence;",
            "    cloister: CloisterCustody;",
            "    frame: AbbeySeatCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-twenty-three-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    cloister: CloisterCustody;",
            "    frame: AbbeySeatCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(
            name,
            &depth_twenty_three_source_with(header, abbey_seat, packet),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-twenty-three custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_preserves_depth_first_diagnostic_order_at_depth_twenty_three() {
    const LEAF_PATH: &str = "Packet.frame.cloister.priory.monastery.abbey.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    let source = depth_twenty_three_source_with(
        "    authority: Evidence;",
        "    sibling: CloisterCustody;",
        "    frame: AbbeySeatCustody;",
    );
    let main = write_program("depth-twenty-three-cross-sibling", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a cross-sibling depth-twenty-three projection must fail closed");
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
fn source_placement_custody_rejects_a_zero_layout_depth_twenty_three_wrapper() {
    let source = depth_twenty_three_source_with(
        "    authority: Evidence;",
        "    cloister: CloisterCustody;",
        "    frame: AbbeySeatCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-twenty-three-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout twenty-third wrapper must remain outside the custody cohort");
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
fn source_placement_custody_rejects_a_depth_twenty_three_back_edge() {
    let source = depth_twenty_three_source_with(
        "    authority: Evidence;",
        "    cloister: CloisterCustody;",
        "    frame: AbbeySeatCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: AbbeySeat;",
        1,
    );
    let main = write_program("depth-twenty-three-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a twenty-third-level back-edge must remain outside the custody cohort");
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
fn source_placement_custody_accepts_twenty_four_nested_projection_record_paths() {
    let source = depth_twenty_four_source_with(
        "    authority: Evidence;",
        "    abbey_seat: AbbeySeatCustody;",
        "    frame: ChapterHouseCustody;",
    );
    let main = write_program("depth-twenty-four-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-twenty-four placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_depth_twenty_four_projection_drift() {
    const LEAF_PATH: &str = "Packet.frame.abbey_seat.cloister.priory.monastery.abbey.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    for (name, header, chapter_house, packet, expected) in [
        (
            "depth-twenty-four-missing-leaf",
            "",
            "    abbey_seat: AbbeySeatCustody;",
            "    frame: ChapterHouseCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-twenty-four-missing-inner-projection",
            "    authority: Evidence;",
            "",
            "    frame: ChapterHouseCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-twenty-four-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    abbey_seat: AbbeySeatCustody;",
            "    frame: ChapterHouseCustody;",
            vec![
                "Packet.frame.abbey_seat.cloister.priory.monastery.abbey.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-twenty-four-wrong-type",
            "    authority: OtherEvidence;",
            "    abbey_seat: AbbeySeatCustody;",
            "    frame: ChapterHouseCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-twenty-four-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    abbey_seat: AbbeySeatCustody;",
            "    frame: ChapterHouseCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(
            name,
            &depth_twenty_four_source_with(header, chapter_house, packet),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-twenty-four custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_preserves_depth_first_diagnostic_order_at_depth_twenty_four() {
    const LEAF_PATH: &str = "Packet.frame.abbey_seat.cloister.priory.monastery.abbey.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    let source = depth_twenty_four_source_with(
        "    authority: Evidence;",
        "    sibling: AbbeySeatCustody;",
        "    frame: ChapterHouseCustody;",
    );
    let main = write_program("depth-twenty-four-cross-sibling", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a cross-sibling depth-twenty-four projection must fail closed");
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
fn source_placement_custody_rejects_a_zero_layout_depth_twenty_four_wrapper() {
    let source = depth_twenty_four_source_with(
        "    authority: Evidence;",
        "    abbey_seat: AbbeySeatCustody;",
        "    frame: ChapterHouseCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-twenty-four-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout twenty-fourth wrapper must remain outside the custody cohort");
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
fn source_placement_custody_rejects_a_depth_twenty_four_back_edge() {
    let source = depth_twenty_four_source_with(
        "    authority: Evidence;",
        "    abbey_seat: AbbeySeatCustody;",
        "    frame: ChapterHouseCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: ChapterHouse;",
        1,
    );
    let main = write_program("depth-twenty-four-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a twenty-fourth-level back-edge must remain outside the custody cohort");
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
fn source_placement_custody_keeps_a_twenty_fifth_record_level_fenced() {
    let source = depth_twenty_four_source_with(
        "    authority: Evidence;",
        "    abbey_seat: AbbeySeatCustody;",
        "    frame: ChapterHouseCustody;",
    )
    .replacen(
        "pub data Packet {\n    frame: ChapterHouse;\n    sibling: Plain;\n}",
        "pub data Scriptorium {\n    chapter_house: ChapterHouse;\n    marker: u32;\n}\npub data Packet {\n    frame: Scriptorium;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 96", "offset: 100", 1)
    .replacen("size_fixed: 100", "size_fixed: 104", 1)
    .replacen(
        "data PacketCustody {\n    frame: ChapterHouseCustody;\n}",
        "data ScriptoriumCustody {\n    chapter_house: ChapterHouseCustody;\n}\ndata PacketCustody {\n    frame: ScriptoriumCustody;\n}",
        1,
    );
    let main = write_program("depth-twenty-five-fenced", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("twenty-fifth-level custody must remain fenced");
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
fn source_placement_custody_keeps_array_and_case_spines_fenced_at_depth_three() {
    let baseline = depth_three_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "",
    );
    let cases = [
        (
            "depth-three-array-fenced",
            baseline.replacen("    header: Header;", "    header: [Header; 1];", 1),
            ["Native::plan", "outside the exact twenty-four-record"],
        ),
        (
            "depth-three-case-fenced",
            baseline.replacen(
                "    authority [erased]: Evidence;\n}",
                "    authority [erased]: Evidence;\n    case Alternate;\n}",
                1,
            ),
            [
                "placed view `Placed<Native,Packet>`",
                "schema data `Packet` field `frame`",
            ],
        ),
        (
            "depth-three-generic-fenced",
            baseline
                .replacen("pub data Header {", "pub data Header<T> {", 1)
                .replacen("    header: Header;", "    header: Header<u32>;", 1),
            ["Native::plan", "outside the exact twenty-four-record"],
        ),
    ];
    for (name, source, expected) in cases {
        let main = write_program(name, &source);
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("unsupported depth-three aggregate spines must fail closed");
        assert_diagnostic(&diagnostics, &expected);
    }
}
