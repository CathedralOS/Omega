use super::{
    assert_diagnostic, depth_five_nested_source, depth_four_nested_source, depth_six_nested_source,
    depth_three_nested_source, depth_two_nested_source, nested_source, source, write_program,
};
use access_plans::{AccessPlan, PlacementPlan, validate_placement_plan};
use compiler::{CheckedCompileRequest, compile_to_checked};
use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport};

#[test]
fn source_placement_custody_rejects_an_extra_non_schema_field() {
    let main = write_program(
        "extra",
        &source("    authority: Evidence;\n    spare: Evidence;"),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("extra custody paths must fail closed");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "custody projection has no `PacketCustody.spare` path",
            "extra canonical field path",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_the_wrong_exact_type() {
    let main = write_program("wrong-type", &source("    authority: OtherEvidence;"));
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("custody field types must agree exactly");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.authority",
            "exact type",
            "OtherEvidence",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_the_wrong_multiplicity() {
    let main = write_program(
        "wrong-multiplicity",
        &source("    authority: CopyEvidence;"),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("custody multiplicity must agree exactly");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.authority",
            "multiplicity Affine",
            "multiplicity Unrestricted",
        ],
    );
}

#[test]
fn placement_custody_revalidation_rejects_policy_decision_drift() {
    let main = write_program("policy-drift", &source("    authority: Evidence;"));
    let mut checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("baseline custody must compile")
        .into_program();
    let plan = checked
        .typed
        .placed_view_plans
        .first_mut()
        .expect("fixture must derive one placed view");
    let mut layout = plan.placement.layout().clone();
    layout.entries.push(LayoutFieldEntryReport {
        field: "authority".to_owned(),
        member_identity: None,
        placement: LayoutPlacementReport::At { offset: 4 },
    });
    layout.size = Some(8);
    let access = AccessPlan::inaccessible(&layout).expect("mutated layout access plan");
    plan.placement = validate_placement_plan(PlacementPlan {
        layout,
        access,
        reach: plan.placement.reach().clone(),
    })
    .expect("structurally valid drifted placement plan");

    let diagnostics = validation::validate_program(&checked.typed)
        .expect_err("a changed normalized policy decision must invalidate custody");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.authority",
            "represented at offset 4",
            "must be absent",
        ],
    );
}

#[test]
fn source_placement_custody_accepts_one_nested_projection_record_path() {
    let main = write_program(
        "nested-exact",
        &nested_source("    authority: Evidence;", "    header: HeaderCustody;"),
    );
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact nested placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_a_missing_nested_leaf() {
    let main = write_program(
        "nested-missing",
        &nested_source("", "    header: HeaderCustody;"),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("missing nested custody leaf must fail closed");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.header.authority",
            "custody-carried",
            "omits canonical field path",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_a_cross_sibling_projection() {
    let main = write_program(
        "nested-cross-sibling",
        &nested_source("    authority: Evidence;", "    sibling: HeaderCustody;"),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("custody projection paths cannot move across siblings");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.header.authority",
            "omits canonical field path",
            "Packet.sibling",
            "represented at offset 4 with width 4",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_a_nested_represented_sibling() {
    let main = write_program(
        "nested-represented",
        &nested_source(
            "    authority: Evidence;\n    bits: u32;",
            "    header: HeaderCustody;",
        ),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("represented nested fields must remain absent from custody");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.header.bits",
            "contained in `Packet.header`",
            "represented at offset 0 with width 4",
            "must be absent",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_the_wrong_nested_leaf_type() {
    let main = write_program(
        "nested-wrong-type",
        &nested_source(
            "    authority: OtherEvidence;",
            "    header: HeaderCustody;",
        ),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("nested custody leaf type must agree exactly");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.header.authority",
            "exact type",
            "OtherEvidence",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_the_wrong_nested_leaf_multiplicity() {
    let main = write_program(
        "nested-wrong-multiplicity",
        &nested_source("    authority: CopyEvidence;", "    header: HeaderCustody;"),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("nested custody leaf multiplicity must agree exactly");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.header.authority",
            "multiplicity Affine",
            "multiplicity Unrestricted",
        ],
    );
}

#[test]
fn source_placement_custody_accepts_two_nested_projection_record_paths() {
    let main = write_program(
        "depth-two-exact",
        &depth_two_nested_source(
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
        ),
    );
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-two placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_a_missing_depth_two_leaf() {
    let main = write_program(
        "depth-two-missing",
        &depth_two_nested_source(
            "",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
        ),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("missing depth-two custody leaf must fail closed");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.envelope.header.authority",
            "custody-carried",
            "omits canonical field path",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_a_cross_sibling_depth_two_projection() {
    let main = write_program(
        "depth-two-cross-sibling",
        &depth_two_nested_source(
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    sibling: EnvelopeCustody;",
        ),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("depth-two custody paths cannot move across siblings");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.envelope.header.authority",
            "omits canonical field path",
            "Packet.sibling",
            "represented at offset 8 with width 4",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_a_depth_two_represented_sibling() {
    let main = write_program(
        "depth-two-represented",
        &depth_two_nested_source(
            "    authority: Evidence;\n    bits: u32;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
        ),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("represented depth-two fields must remain absent from custody");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.envelope.header.bits",
            "contained in `Packet.envelope`",
            "represented at offset 0 with width 8",
            "must be absent",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_a_represented_intermediate_sibling() {
    let main = write_program(
        "depth-two-intermediate-represented",
        &depth_two_nested_source(
            "    authority: Evidence;",
            "    header: HeaderCustody;\n    marker: u32;",
            "    envelope: EnvelopeCustody;",
        ),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("represented intermediate fields must remain absent from custody");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.envelope.marker",
            "contained in `Packet.envelope`",
            "must be absent",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_a_zero_layout_depth_two_wrapper() {
    let source = depth_two_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-two-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout nested wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.envelope",
            "represented at offset 0 with width 4",
            "outside the exact twenty-four-record",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_the_wrong_depth_two_leaf_type() {
    let main = write_program(
        "depth-two-wrong-type",
        &depth_two_nested_source(
            "    authority: OtherEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
        ),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("depth-two custody leaf type must agree exactly");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.envelope.header.authority",
            "exact type",
            "OtherEvidence",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_the_wrong_depth_two_leaf_multiplicity() {
    let main = write_program(
        "depth-two-wrong-multiplicity",
        &depth_two_nested_source(
            "    authority: CopyEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
        ),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("depth-two custody leaf multiplicity must agree exactly");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.envelope.header.authority",
            "multiplicity Affine",
            "multiplicity Unrestricted",
        ],
    );
}

#[test]
fn source_placement_custody_accepts_three_nested_projection_record_paths() {
    let main = write_program(
        "depth-three-exact",
        &depth_three_nested_source(
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
        ),
    );
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-three placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_a_missing_depth_three_leaf() {
    let main = write_program(
        "depth-three-missing",
        &depth_three_nested_source(
            "",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
        ),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("missing depth-three custody leaf must fail closed");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame.envelope.header.authority",
            "custody-carried",
            "omits canonical field path",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_a_missing_depth_three_projection_record() {
    let main = write_program(
        "depth-three-missing-projection-record",
        &depth_three_nested_source(
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "",
            "    frame: FrameCustody;",
        ),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("an omitted depth-three projection record must fail closed");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame.envelope.header.authority",
            "custody-carried",
            "omits canonical field path",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_a_cross_sibling_depth_three_projection() {
    let main = write_program(
        "depth-three-cross-sibling",
        &depth_three_nested_source(
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    sibling: FrameCustody;",
        ),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("depth-three custody paths cannot move across siblings");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame.envelope.header.authority",
            "omits canonical field path",
            "Packet.sibling",
            "represented at offset 12 with width 4",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_a_depth_three_represented_leaf() {
    let main = write_program(
        "depth-three-represented",
        &depth_three_nested_source(
            "    authority: Evidence;\n    bits: u32;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
        ),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("represented depth-three leaves must remain absent from custody");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame.envelope.header.bits",
            "contained in `Packet.frame`",
            "represented at offset 0 with width 12",
            "must be absent",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_the_wrong_depth_three_leaf_type_and_multiplicity() {
    for (name, leaf, expected) in [
        (
            "depth-three-wrong-type",
            "    authority: OtherEvidence;",
            ["exact type", "OtherEvidence"],
        ),
        (
            "depth-three-wrong-multiplicity",
            "    authority: CopyEvidence;",
            ["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(
            name,
            &depth_three_nested_source(
                leaf,
                "    header: HeaderCustody;",
                "    envelope: EnvelopeCustody;",
                "    frame: FrameCustody;",
            ),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-three custody leaf identity must agree exactly");
        assert_diagnostic(
            &diagnostics,
            &[
                "Native::plan",
                "Packet.frame.envelope.header.authority",
                expected[0],
                expected[1],
            ],
        );
    }
}

#[test]
fn source_placement_custody_rejects_a_zero_layout_depth_three_wrapper() {
    let source = depth_three_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-three-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout third wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame",
            "represented at offset 0 with width 8",
            "outside the exact twenty-four-record",
        ],
    );
}

#[test]
fn source_placement_custody_accepts_four_nested_projection_record_paths() {
    let source = depth_four_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    frame: BoxedCustody;",
    );
    let main = write_program("depth-four-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-four placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_depth_four_projection_drift() {
    for (name, header, envelope, frame, boxed, packet, expected) in [
        (
            "depth-four-missing-leaf",
            "",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    frame: BoxedCustody;",
            vec![
                "Packet.frame.frame.envelope.header.authority",
                "omits canonical field path",
            ],
        ),
        (
            "depth-four-missing-projection",
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "",
            "    frame: BoxedCustody;",
            vec![
                "Packet.frame.frame.envelope.header.authority",
                "omits canonical field path",
            ],
        ),
        (
            "depth-four-cross-sibling",
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    sibling: BoxedCustody;",
            vec![
                "Packet.frame.frame.envelope.header.authority",
                "Packet.sibling",
            ],
        ),
        (
            "depth-four-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    frame: BoxedCustody;",
            vec!["Packet.frame.frame.envelope.header.bits", "must be absent"],
        ),
        (
            "depth-four-wrong-type",
            "    authority: OtherEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    frame: BoxedCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-four-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    frame: BoxedCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(
            name,
            &depth_four_nested_source(header, envelope, frame, boxed, packet),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-four custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_rejects_a_zero_layout_depth_four_wrapper() {
    let source = depth_four_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    frame: BoxedCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-four-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout fourth wrapper must remain outside the custody cohort");
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
fn source_placement_custody_accepts_five_nested_projection_record_paths() {
    let source = depth_five_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    frame: CrateCustody;",
    );
    let main = write_program("depth-five-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-five placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_a_missing_depth_five_projection() {
    let source = depth_five_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "",
    );
    let main = write_program("depth-five-hidden", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("missing fifth-level custody projection must fail closed");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame.boxed.frame.envelope.header.authority",
            "omits canonical field path",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_depth_five_projection_drift() {
    for (name, header, envelope, frame, boxed, crate_fields, packet, expected) in [
        (
            "depth-five-missing-leaf",
            "",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    frame: CrateCustody;",
            vec![
                "Packet.frame.boxed.frame.envelope.header.authority",
                "omits canonical field path",
            ],
        ),
        (
            "depth-five-missing-inner-projection",
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "",
            "    frame: CrateCustody;",
            vec![
                "Packet.frame.boxed.frame.envelope.header.authority",
                "omits canonical field path",
            ],
        ),
        (
            "depth-five-cross-sibling",
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    sibling: CrateCustody;",
            vec![
                "Packet.frame.boxed.frame.envelope.header.authority",
                "Packet.sibling",
            ],
        ),
        (
            "depth-five-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    frame: CrateCustody;",
            vec![
                "Packet.frame.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-five-wrong-type",
            "    authority: OtherEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    frame: CrateCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-five-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    frame: CrateCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(
            name,
            &depth_five_nested_source(header, envelope, frame, boxed, crate_fields, packet),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-five custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

#[test]
fn source_placement_custody_rejects_a_zero_layout_depth_five_wrapper() {
    let source = depth_five_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    frame: CrateCustody;",
    )
    .replacen(
        "    bits: u32;\n    authority [erased]: Evidence;",
        "    phantom [erased]: OtherEvidence;\n    authority [erased]: Evidence;",
        1,
    );
    let main = write_program("depth-five-zero-wrapper", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a zero-layout fifth wrapper must remain outside the custody cohort");
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
fn source_placement_custody_accepts_six_nested_projection_record_paths() {
    let source = depth_six_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    frame: ChestCustody;",
    );
    let main = write_program("depth-six-exact", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact depth-six placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_a_missing_depth_six_projection() {
    let source = depth_six_nested_source(
        "    authority: Evidence;",
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "",
    );
    let main = write_program("depth-six-hidden", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("missing sixth-level custody projection must fail closed");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame.item.boxed.frame.envelope.header.authority",
            "omits canonical field path",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_depth_six_projection_drift() {
    for (name, header, envelope, frame, boxed, crate_fields, chest, packet, expected) in [
        (
            "depth-six-missing-leaf",
            "",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    frame: ChestCustody;",
            vec![
                "Packet.frame.item.boxed.frame.envelope.header.authority",
                "omits canonical field path",
            ],
        ),
        (
            "depth-six-missing-inner-projection",
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "",
            "    frame: ChestCustody;",
            vec![
                "Packet.frame.item.boxed.frame.envelope.header.authority",
                "omits canonical field path",
            ],
        ),
        (
            "depth-six-cross-sibling",
            "    authority: Evidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    sibling: ChestCustody;",
            vec![
                "Packet.frame.item.boxed.frame.envelope.header.authority",
                "Packet.sibling",
            ],
        ),
        (
            "depth-six-represented-leaf",
            "    authority: Evidence;\n    bits: u32;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    frame: ChestCustody;",
            vec![
                "Packet.frame.item.boxed.frame.envelope.header.bits",
                "must be absent",
            ],
        ),
        (
            "depth-six-wrong-type",
            "    authority: OtherEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    frame: ChestCustody;",
            vec!["exact type", "OtherEvidence"],
        ),
        (
            "depth-six-wrong-multiplicity",
            "    authority: CopyEvidence;",
            "    header: HeaderCustody;",
            "    envelope: EnvelopeCustody;",
            "    frame: FrameCustody;",
            "    boxed: BoxedCustody;",
            "    item: CrateCustody;",
            "    frame: ChestCustody;",
            vec!["multiplicity Affine", "multiplicity Unrestricted"],
        ),
    ] {
        let main = write_program(
            name,
            &depth_six_nested_source(header, envelope, frame, boxed, crate_fields, chest, packet),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect_err("depth-six custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}
