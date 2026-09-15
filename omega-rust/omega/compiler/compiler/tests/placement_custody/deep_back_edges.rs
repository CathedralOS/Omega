use super::{
    assert_diagnostic, depth_eighteen_source_with, depth_fifteen_source_with,
    depth_fourteen_source_with, depth_nineteen_source_with, depth_seventeen_source_with,
    depth_sixteen_source_with, depth_thirteen_source_with, depth_twelve_source_with, write_program,
};
use compiler::{CheckedCompileRequest, compile_to_checked};

#[test]
fn source_placement_custody_rejects_a_depth_twelve_back_edge() {
    let source = depth_twelve_source_with(
        "    authority: Evidence;",
        "    casket: CasketCustody;",
        "    frame: ReliquaryCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Casket;",
        1,
    );
    let main = write_program("depth-twelve-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a twelfth-level back-edge must remain outside the custody cohort");
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
fn source_placement_custody_accepts_thirteen_nested_projection_record_paths() {
    let source = depth_thirteen_source_with(
        "    authority: Evidence;",
        "    reliquary: ReliquaryCustody;",
        "    frame: ShrineCustody;",
    );
    let main = write_program("depth-thirteen-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-thirteen placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_depth_thirteen_projection_drift() {
    const LEAF_PATH: &str = "Packet.frame.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    for (name, header, shrine, packet, expected) in [
        (
            "depth-thirteen-missing-leaf",
            "",
            "    reliquary: ReliquaryCustody;",
            "    frame: ShrineCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-thirteen-missing-inner-projection",
            "    authority: Evidence;",
            "",
            "    frame: ShrineCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-thirteen-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    reliquary: ReliquaryCustody;",
            "    frame: ShrineCustody;",
            vec![
                "Packet.frame.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-thirteen-wrong-type",
            "    authority: OtherEvidence;",
            "    reliquary: ReliquaryCustody;",
            "    frame: ShrineCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-thirteen-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    reliquary: ReliquaryCustody;",
            "    frame: ShrineCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(name, &depth_thirteen_source_with(header, shrine, packet));
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-thirteen custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_preserves_depth_first_diagnostic_order_at_depth_thirteen() {
    const LEAF_PATH: &str = "Packet.frame.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    let source = depth_thirteen_source_with(
        "    authority: Evidence;",
        "    sibling: ReliquaryCustody;",
        "    frame: ShrineCustody;",
    );
    let main = write_program("depth-thirteen-cross-sibling", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a cross-sibling depth-thirteen projection must fail closed");
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
fn source_placement_custody_rejects_a_zero_layout_depth_thirteen_wrapper() {
    let source = depth_thirteen_source_with(
        "    authority: Evidence;",
        "    reliquary: ReliquaryCustody;",
        "    frame: ShrineCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-thirteen-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout thirteenth wrapper must remain outside the custody cohort");
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
fn source_placement_custody_rejects_a_depth_thirteen_back_edge() {
    let source = depth_thirteen_source_with(
        "    authority: Evidence;",
        "    reliquary: ReliquaryCustody;",
        "    frame: ShrineCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Shrine;",
        1,
    );
    let main = write_program("depth-thirteen-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a thirteenth-level back-edge must remain outside the custody cohort");
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
fn source_placement_custody_accepts_fourteen_nested_projection_record_paths() {
    let source = depth_fourteen_source_with(
        "    authority: Evidence;",
        "    shrine: ShrineCustody;",
        "    frame: SanctumCustody;",
    );
    let main = write_program("depth-fourteen-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-fourteen placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_depth_fourteen_projection_drift() {
    const LEAF_PATH: &str = "Packet.frame.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    for (name, header, sanctum, packet, expected) in [
        (
            "depth-fourteen-missing-leaf",
            "",
            "    shrine: ShrineCustody;",
            "    frame: SanctumCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-fourteen-missing-inner-projection",
            "    authority: Evidence;",
            "",
            "    frame: SanctumCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-fourteen-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    shrine: ShrineCustody;",
            "    frame: SanctumCustody;",
            vec![
                "Packet.frame.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-fourteen-wrong-type",
            "    authority: OtherEvidence;",
            "    shrine: ShrineCustody;",
            "    frame: SanctumCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-fourteen-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    shrine: ShrineCustody;",
            "    frame: SanctumCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(name, &depth_fourteen_source_with(header, sanctum, packet));
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-fourteen custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_preserves_depth_first_diagnostic_order_at_depth_fourteen() {
    const LEAF_PATH: &str = "Packet.frame.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    let source = depth_fourteen_source_with(
        "    authority: Evidence;",
        "    sibling: ShrineCustody;",
        "    frame: SanctumCustody;",
    );
    let main = write_program("depth-fourteen-cross-sibling", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a cross-sibling depth-fourteen projection must fail closed");
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
fn source_placement_custody_rejects_a_zero_layout_depth_fourteen_wrapper() {
    let source = depth_fourteen_source_with(
        "    authority: Evidence;",
        "    shrine: ShrineCustody;",
        "    frame: SanctumCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-fourteen-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout fourteenth wrapper must remain outside the custody cohort");
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
fn source_placement_custody_rejects_a_depth_fourteen_back_edge() {
    let source = depth_fourteen_source_with(
        "    authority: Evidence;",
        "    shrine: ShrineCustody;",
        "    frame: SanctumCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Sanctum;",
        1,
    );
    let main = write_program("depth-fourteen-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a fourteenth-level back-edge must remain outside the custody cohort");
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
fn source_placement_custody_accepts_fifteen_nested_projection_record_paths() {
    let source = depth_fifteen_source_with(
        "    authority: Evidence;",
        "    sanctum: SanctumCustody;",
        "    frame: TabernacleCustody;",
    );
    let main = write_program("depth-fifteen-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-fifteen placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_depth_fifteen_projection_drift() {
    const LEAF_PATH: &str = "Packet.frame.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    for (name, header, tabernacle, packet, expected) in [
        (
            "depth-fifteen-missing-leaf",
            "",
            "    sanctum: SanctumCustody;",
            "    frame: TabernacleCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-fifteen-missing-inner-projection",
            "    authority: Evidence;",
            "",
            "    frame: TabernacleCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-fifteen-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    sanctum: SanctumCustody;",
            "    frame: TabernacleCustody;",
            vec![
                "Packet.frame.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-fifteen-wrong-type",
            "    authority: OtherEvidence;",
            "    sanctum: SanctumCustody;",
            "    frame: TabernacleCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-fifteen-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    sanctum: SanctumCustody;",
            "    frame: TabernacleCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(name, &depth_fifteen_source_with(header, tabernacle, packet));
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-fifteen custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_preserves_depth_first_diagnostic_order_at_depth_fifteen() {
    const LEAF_PATH: &str = "Packet.frame.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    let source = depth_fifteen_source_with(
        "    authority: Evidence;",
        "    sibling: SanctumCustody;",
        "    frame: TabernacleCustody;",
    );
    let main = write_program("depth-fifteen-cross-sibling", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a cross-sibling depth-fifteen projection must fail closed");
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
fn source_placement_custody_rejects_a_zero_layout_depth_fifteen_wrapper() {
    let source = depth_fifteen_source_with(
        "    authority: Evidence;",
        "    sanctum: SanctumCustody;",
        "    frame: TabernacleCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-fifteen-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout fifteenth wrapper must remain outside the custody cohort");
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
fn source_placement_custody_rejects_a_depth_fifteen_back_edge() {
    let source = depth_fifteen_source_with(
        "    authority: Evidence;",
        "    sanctum: SanctumCustody;",
        "    frame: TabernacleCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Tabernacle;",
        1,
    );
    let main = write_program("depth-fifteen-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a fifteenth-level back-edge must remain outside the custody cohort");
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
fn source_placement_custody_accepts_sixteen_nested_projection_record_paths() {
    let source = depth_sixteen_source_with(
        "    authority: Evidence;",
        "    tabernacle: TabernacleCustody;",
        "    frame: ChapelCustody;",
    );
    let main = write_program("depth-sixteen-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-sixteen placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_depth_sixteen_projection_drift() {
    const LEAF_PATH: &str = "Packet.frame.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    for (name, header, chapel, packet, expected) in [
        (
            "depth-sixteen-missing-leaf",
            "",
            "    tabernacle: TabernacleCustody;",
            "    frame: ChapelCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-sixteen-missing-inner-projection",
            "    authority: Evidence;",
            "",
            "    frame: ChapelCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-sixteen-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    tabernacle: TabernacleCustody;",
            "    frame: ChapelCustody;",
            vec![
                "Packet.frame.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-sixteen-wrong-type",
            "    authority: OtherEvidence;",
            "    tabernacle: TabernacleCustody;",
            "    frame: ChapelCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-sixteen-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    tabernacle: TabernacleCustody;",
            "    frame: ChapelCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(name, &depth_sixteen_source_with(header, chapel, packet));
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-sixteen custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_preserves_depth_first_diagnostic_order_at_depth_sixteen() {
    const LEAF_PATH: &str = "Packet.frame.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    let source = depth_sixteen_source_with(
        "    authority: Evidence;",
        "    sibling: TabernacleCustody;",
        "    frame: ChapelCustody;",
    );
    let main = write_program("depth-sixteen-cross-sibling", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a cross-sibling depth-sixteen projection must fail closed");
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
fn source_placement_custody_rejects_a_zero_layout_depth_sixteen_wrapper() {
    let source = depth_sixteen_source_with(
        "    authority: Evidence;",
        "    tabernacle: TabernacleCustody;",
        "    frame: ChapelCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-sixteen-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout sixteenth wrapper must remain outside the custody cohort");
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
fn source_placement_custody_rejects_a_depth_sixteen_back_edge() {
    let source = depth_sixteen_source_with(
        "    authority: Evidence;",
        "    tabernacle: TabernacleCustody;",
        "    frame: ChapelCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Chapel;",
        1,
    );
    let main = write_program("depth-sixteen-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a sixteenth-level back-edge must remain outside the custody cohort");
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
fn source_placement_custody_accepts_seventeen_nested_projection_record_paths() {
    let source = depth_seventeen_source_with(
        "    authority: Evidence;",
        "    chapel: ChapelCustody;",
        "    frame: BasilicaCustody;",
    );
    let main = write_program("depth-seventeen-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-seventeen placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_depth_seventeen_projection_drift() {
    const LEAF_PATH: &str = "Packet.frame.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    for (name, header, basilica, packet, expected) in [
        (
            "depth-seventeen-missing-leaf",
            "",
            "    chapel: ChapelCustody;",
            "    frame: BasilicaCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-seventeen-missing-inner-projection",
            "    authority: Evidence;",
            "",
            "    frame: BasilicaCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-seventeen-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    chapel: ChapelCustody;",
            "    frame: BasilicaCustody;",
            vec![
                "Packet.frame.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-seventeen-wrong-type",
            "    authority: OtherEvidence;",
            "    chapel: ChapelCustody;",
            "    frame: BasilicaCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-seventeen-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    chapel: ChapelCustody;",
            "    frame: BasilicaCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(name, &depth_seventeen_source_with(header, basilica, packet));
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-seventeen custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_preserves_depth_first_diagnostic_order_at_depth_seventeen() {
    const LEAF_PATH: &str = "Packet.frame.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    let source = depth_seventeen_source_with(
        "    authority: Evidence;",
        "    sibling: ChapelCustody;",
        "    frame: BasilicaCustody;",
    );
    let main = write_program("depth-seventeen-cross-sibling", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a cross-sibling depth-seventeen projection must fail closed");
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
fn source_placement_custody_rejects_a_zero_layout_depth_seventeen_wrapper() {
    let source = depth_seventeen_source_with(
        "    authority: Evidence;",
        "    chapel: ChapelCustody;",
        "    frame: BasilicaCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-seventeen-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout seventeenth wrapper must remain outside the custody cohort");
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
fn source_placement_custody_rejects_a_depth_seventeen_back_edge() {
    let source = depth_seventeen_source_with(
        "    authority: Evidence;",
        "    chapel: ChapelCustody;",
        "    frame: BasilicaCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Basilica;",
        1,
    );
    let main = write_program("depth-seventeen-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a seventeenth-level back-edge must remain outside the custody cohort");
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
fn source_placement_custody_accepts_eighteen_nested_projection_record_paths() {
    let source = depth_eighteen_source_with(
        "    authority: Evidence;",
        "    basilica: BasilicaCustody;",
        "    frame: CathedralCustody;",
    );
    let main = write_program("depth-eighteen-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-eighteen placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_depth_eighteen_projection_drift() {
    const LEAF_PATH: &str = "Packet.frame.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    for (name, header, cathedral, packet, expected) in [
        (
            "depth-eighteen-missing-leaf",
            "",
            "    basilica: BasilicaCustody;",
            "    frame: CathedralCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-eighteen-missing-inner-projection",
            "    authority: Evidence;",
            "",
            "    frame: CathedralCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-eighteen-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    basilica: BasilicaCustody;",
            "    frame: CathedralCustody;",
            vec![
                "Packet.frame.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-eighteen-wrong-type",
            "    authority: OtherEvidence;",
            "    basilica: BasilicaCustody;",
            "    frame: CathedralCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-eighteen-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    basilica: BasilicaCustody;",
            "    frame: CathedralCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(name, &depth_eighteen_source_with(header, cathedral, packet));
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-eighteen custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_preserves_depth_first_diagnostic_order_at_depth_eighteen() {
    const LEAF_PATH: &str = "Packet.frame.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    let source = depth_eighteen_source_with(
        "    authority: Evidence;",
        "    sibling: BasilicaCustody;",
        "    frame: CathedralCustody;",
    );
    let main = write_program("depth-eighteen-cross-sibling", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a cross-sibling depth-eighteen projection must fail closed");
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
fn source_placement_custody_rejects_a_zero_layout_depth_eighteen_wrapper() {
    let source = depth_eighteen_source_with(
        "    authority: Evidence;",
        "    basilica: BasilicaCustody;",
        "    frame: CathedralCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-eighteen-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout eighteenth wrapper must remain outside the custody cohort");
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
fn source_placement_custody_rejects_a_depth_eighteen_back_edge() {
    let source = depth_eighteen_source_with(
        "    authority: Evidence;",
        "    basilica: BasilicaCustody;",
        "    frame: CathedralCustody;",
    )
    .replacen(
        "pub data Envelope {\n    header: Header;",
        "pub data Envelope {\n    header: Cathedral;",
        1,
    );
    let main = write_program("depth-eighteen-back-edge", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("an eighteenth-level back-edge must remain outside the custody cohort");
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
fn source_placement_custody_accepts_nineteen_nested_projection_record_paths() {
    let source = depth_nineteen_source_with(
        "    authority: Evidence;",
        "    cathedral: CathedralCustody;",
        "    frame: AbbeyCustody;",
    );
    let main = write_program("depth-nineteen-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-nineteen placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_depth_nineteen_projection_drift() {
    const LEAF_PATH: &str = "Packet.frame.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    for (name, header, abbey, packet, expected) in [
        (
            "depth-nineteen-missing-leaf",
            "",
            "    cathedral: CathedralCustody;",
            "    frame: AbbeyCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-nineteen-missing-inner-projection",
            "    authority: Evidence;",
            "",
            "    frame: AbbeyCustody;",
            vec![LEAF_PATH, "omits canonical field path"],
        ),
        (
            "depth-nineteen-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    cathedral: CathedralCustody;",
            "    frame: AbbeyCustody;",
            vec![
                "Packet.frame.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-nineteen-wrong-type",
            "    authority: OtherEvidence;",
            "    cathedral: CathedralCustody;",
            "    frame: AbbeyCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-nineteen-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    cathedral: CathedralCustody;",
            "    frame: AbbeyCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(name, &depth_nineteen_source_with(header, abbey, packet));
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-nineteen custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_preserves_depth_first_diagnostic_order_at_depth_nineteen() {
    const LEAF_PATH: &str = "Packet.frame.cathedral.basilica.chapel.tabernacle.sanctum.shrine.reliquary.casket.coffer.lockbox.strongbox.vault.chest.item.boxed.frame.envelope.header.authority";
    let source = depth_nineteen_source_with(
        "    authority: Evidence;",
        "    sibling: CathedralCustody;",
        "    frame: AbbeyCustody;",
    );
    let main = write_program("depth-nineteen-cross-sibling", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a cross-sibling depth-nineteen projection must fail closed");
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
fn source_placement_custody_rejects_a_zero_layout_depth_nineteen_wrapper() {
    let source = depth_nineteen_source_with(
        "    authority: Evidence;",
        "    cathedral: CathedralCustody;",
        "    frame: AbbeyCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-nineteen-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout nineteenth wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame",
            "outside the exact twenty-four-record",
        ],
    );
}
