use std::fs;
use std::path::PathBuf;

use omega_compiler::compile_to_checked;
use psi_access_plans::{AccessPlan, PlacementPlan, validate_placement_plan};
use psi_layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport};

fn write_program(name: &str, source: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "omega-placement-custody-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create placement-custody test directory");
    let main = directory.join("main.omg");
    fs::write(&main, source).expect("write placement-custody test program");
    main
}

fn source(custody_fields: &str) -> String {
    format!(
        r#"
use omega::language::core::layout;

pub data Evidence {{}}
pub data OtherEvidence {{}}
pub data CopyEvidence [copy] {{}}
pub data Packet {{
    bits: u32;
    authority [erased]: Evidence;
}}

pub data Native {{
    entries: [FieldEntry; 64];
    services: [u64; 32];
}}

machine Native::plan(&mut self, schema: Schema) -> PlacementPlan {{
    self.entries[0] = FieldEntry {{
        key: schema.fields[0].key,
        placement: FieldPlan::At {{ offset: 0 }},
    }};
    PlacementPlan {{
        layout: Plan {{
            entries: self.entries,
            entry_count: 1,
            size_fixed: 4,
            size_is_dynamic: false,
            align: 4,
        }},
        access: AccessPlan::inaccessible(schema),
        reach: BoundaryReach {{
            services: self.services,
            service_count: 0,
        }},
    }}
}}

data PacketCustody {{
{custody_fields}
}}

PacketNativeCustody:
    PacketCustody satisfies PlacementCustody<Native, Packet>;

machine retain_plan(view: &Placed<Native, Packet>) {{}}

data Main {{}}
machine Main::main(&mut self) {{}}
"#
    )
}

fn nested_source(header_custody_fields: &str, packet_custody_fields: &str) -> String {
    format!(
        r#"
use omega::language::core::layout;

pub data Evidence {{}}
pub data OtherEvidence {{}}
pub data CopyEvidence [copy] {{}}
pub data Header {{
    bits: u32;
    authority [erased]: Evidence;
}}
pub data Plain {{ bits: u32; }}
pub data Packet {{
    header: Header;
    sibling: Plain;
}}

pub data Native {{
    entries: [FieldEntry; 64];
    services: [u64; 32];
}}

machine Native::plan(&mut self, schema: Schema) -> PlacementPlan {{
    self.entries[0] = FieldEntry {{
        key: schema.fields[0].key,
        placement: FieldPlan::At {{ offset: 0 }},
    }};
    self.entries[1] = FieldEntry {{
        key: schema.fields[1].key,
        placement: FieldPlan::At {{ offset: 4 }},
    }};
    PlacementPlan {{
        layout: Plan {{
            entries: self.entries,
            entry_count: 2,
            size_fixed: 8,
            size_is_dynamic: false,
            align: 4,
        }},
        access: AccessPlan::inaccessible(schema),
        reach: BoundaryReach {{
            services: self.services,
            service_count: 0,
        }},
    }}
}}

data HeaderCustody {{
{header_custody_fields}
}}
data PacketCustody {{
{packet_custody_fields}
}}

PacketNativeCustody:
    PacketCustody satisfies PlacementCustody<Native, Packet>;

machine retain_plan(view: &Placed<Native, Packet>) {{}}

data Main {{}}
machine Main::main(&mut self) {{}}
"#
    )
}

fn depth_two_nested_source(
    header_custody_fields: &str,
    envelope_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    format!(
        r#"
use omega::language::core::layout;

pub data Evidence {{}}
pub data OtherEvidence {{}}
pub data CopyEvidence [copy] {{}}
pub data Header {{
    bits: u32;
    authority [erased]: Evidence;
}}
pub data Envelope {{
    header: Header;
    marker: u32;
}}
pub data Plain {{ bits: u32; }}
pub data Packet {{
    envelope: Envelope;
    sibling: Plain;
}}

pub data Native {{
    entries: [FieldEntry; 64];
    services: [u64; 32];
}}

machine Native::plan(&mut self, schema: Schema) -> PlacementPlan {{
    self.entries[0] = FieldEntry {{
        key: schema.fields[0].key,
        placement: FieldPlan::At {{ offset: 0 }},
    }};
    self.entries[1] = FieldEntry {{
        key: schema.fields[1].key,
        placement: FieldPlan::At {{ offset: 8 }},
    }};
    PlacementPlan {{
        layout: Plan {{
            entries: self.entries,
            entry_count: 2,
            size_fixed: 12,
            size_is_dynamic: false,
            align: 4,
        }},
        access: AccessPlan::inaccessible(schema),
        reach: BoundaryReach {{
            services: self.services,
            service_count: 0,
        }},
    }}
}}

data HeaderCustody {{
{header_custody_fields}
}}
data EnvelopeCustody {{
{envelope_custody_fields}
}}
data PacketCustody {{
{packet_custody_fields}
}}

PacketNativeCustody:
    PacketCustody satisfies PlacementCustody<Native, Packet>;

machine retain_plan(view: &Placed<Native, Packet>) {{}}

data Main {{}}
machine Main::main(&mut self) {{}}
"#
    )
}

fn depth_three_nested_source(
    header_custody_fields: &str,
    envelope_custody_fields: &str,
    frame_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    format!(
        r#"
use omega::language::core::layout;

pub data Evidence {{}}
pub data OtherEvidence {{}}
pub data CopyEvidence [copy] {{}}
pub data Header {{
    bits: u32;
    authority [erased]: Evidence;
}}
pub data Envelope {{
    header: Header;
    marker: u32;
}}
pub data Frame {{
    envelope: Envelope;
    flag: u32;
}}
pub data Plain {{ bits: u32; }}
pub data Packet {{
    frame: Frame;
    sibling: Plain;
}}

pub data Native {{
    entries: [FieldEntry; 64];
    services: [u64; 32];
}}

machine Native::plan(&mut self, schema: Schema) -> PlacementPlan {{
    self.entries[0] = FieldEntry {{
        key: schema.fields[0].key,
        placement: FieldPlan::At {{ offset: 0 }},
    }};
    self.entries[1] = FieldEntry {{
        key: schema.fields[1].key,
        placement: FieldPlan::At {{ offset: 12 }},
    }};
    PlacementPlan {{
        layout: Plan {{
            entries: self.entries,
            entry_count: 2,
            size_fixed: 16,
            size_is_dynamic: false,
            align: 4,
        }},
        access: AccessPlan::inaccessible(schema),
        reach: BoundaryReach {{
            services: self.services,
            service_count: 0,
        }},
    }}
}}

data HeaderCustody {{
{header_custody_fields}
}}
data EnvelopeCustody {{
{envelope_custody_fields}
}}
data FrameCustody {{
{frame_custody_fields}
}}
data PacketCustody {{
{packet_custody_fields}
}}

PacketNativeCustody:
    PacketCustody satisfies PlacementCustody<Native, Packet>;

machine retain_plan(view: &Placed<Native, Packet>) {{}}

data Main {{}}
machine Main::main(&mut self) {{}}
"#
    )
}

fn depth_four_nested_source(
    header_custody_fields: &str,
    envelope_custody_fields: &str,
    frame_custody_fields: &str,
    boxed_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_three_nested_source(
        header_custody_fields,
        envelope_custody_fields,
        frame_custody_fields,
        packet_custody_fields,
    )
    .replacen(
        "pub data Packet {\n    frame: Frame;\n    sibling: Plain;\n}",
        "pub data Boxed {\n    frame: Frame;\n    marker: u32;\n}\npub data Packet {\n    frame: Boxed;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 12", "offset: 16", 1)
    .replacen("size_fixed: 16", "size_fixed: 20", 1)
    .replacen(
        &format!("data PacketCustody {{\n{packet_custody_fields}\n}}"),
        &format!(
            "data BoxedCustody {{\n{boxed_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_five_nested_source(
    header_custody_fields: &str,
    envelope_custody_fields: &str,
    frame_custody_fields: &str,
    boxed_custody_fields: &str,
    crate_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_four_nested_source(
        header_custody_fields,
        envelope_custody_fields,
        frame_custody_fields,
        boxed_custody_fields,
        packet_custody_fields,
    )
    .replacen(
        "pub data Packet {\n    frame: Boxed;\n    sibling: Plain;\n}",
        "pub data Crate {\n    boxed: Boxed;\n    marker: u32;\n}\npub data Packet {\n    frame: Crate;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 16", "offset: 20", 1)
    .replacen("size_fixed: 20", "size_fixed: 24", 1)
    .replacen(
        &format!("data PacketCustody {{\n{packet_custody_fields}\n}}"),
        &format!(
            "data CrateCustody {{\n{crate_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_six_nested_source(
    header_custody_fields: &str,
    envelope_custody_fields: &str,
    frame_custody_fields: &str,
    boxed_custody_fields: &str,
    crate_custody_fields: &str,
    chest_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_five_nested_source(
        header_custody_fields,
        envelope_custody_fields,
        frame_custody_fields,
        boxed_custody_fields,
        crate_custody_fields,
        packet_custody_fields,
    )
    .replacen(
        "pub data Packet {\n    frame: Crate;\n    sibling: Plain;\n}",
        "pub data Chest {\n    item: Crate;\n    marker: u32;\n}\npub data Packet {\n    frame: Chest;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 20", "offset: 24", 1)
    .replacen("size_fixed: 24", "size_fixed: 28", 1)
    .replacen(
        &format!("data PacketCustody {{\n{packet_custody_fields}\n}}"),
        &format!(
            "data ChestCustody {{\n{chest_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_seven_nested_source(
    header_custody_fields: &str,
    envelope_custody_fields: &str,
    frame_custody_fields: &str,
    boxed_custody_fields: &str,
    crate_custody_fields: &str,
    chest_custody_fields: &str,
    vault_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_six_nested_source(
        header_custody_fields,
        envelope_custody_fields,
        frame_custody_fields,
        boxed_custody_fields,
        crate_custody_fields,
        chest_custody_fields,
        packet_custody_fields,
    )
    .replacen(
        "pub data Packet {\n    frame: Chest;\n    sibling: Plain;\n}",
        "pub data Vault {\n    chest: Chest;\n    marker: u32;\n}\npub data Packet {\n    frame: Vault;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 24", "offset: 28", 1)
    .replacen("size_fixed: 28", "size_fixed: 32", 1)
    .replacen(
        &format!("data PacketCustody {{\n{packet_custody_fields}\n}}"),
        &format!(
            "data VaultCustody {{\n{vault_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

#[test]
fn source_placement_custody_accepts_the_exact_erased_field_projection() {
    let main = write_program("exact", &source("    authority: Evidence;"));
    compile_to_checked(&main, None).expect("exact placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_a_missing_erased_field() {
    let main = write_program("missing", &source(""));
    let diagnostics =
        compile_to_checked(&main, None).expect_err("missing placement custody must fail closed");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "custody-carried",
            "Packet.authority",
            "omits",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_an_extra_represented_field() {
    let main = write_program(
        "represented",
        &source("    authority: Evidence;\n    bits: u32;"),
    );
    let diagnostics = compile_to_checked(&main, None)
        .expect_err("represented placement fields must remain absent from custody");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.bits",
            "represented at offset 0 with width 4",
            "must be absent",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_an_extra_non_schema_field() {
    let main = write_program(
        "extra",
        &source("    authority: Evidence;\n    spare: Evidence;"),
    );
    let diagnostics =
        compile_to_checked(&main, None).expect_err("extra custody paths must fail closed");
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
    let diagnostics =
        compile_to_checked(&main, None).expect_err("custody field types must agree exactly");
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
    let diagnostics =
        compile_to_checked(&main, None).expect_err("custody multiplicity must agree exactly");
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
    let mut checked = compile_to_checked(&main, None).expect("baseline custody must compile");
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

    let diagnostics = psi_validation::validate_program(&checked.typed)
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
    compile_to_checked(&main, None).expect("exact nested placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_a_missing_nested_leaf() {
    let main = write_program(
        "nested-missing",
        &nested_source("", "    header: HeaderCustody;"),
    );
    let diagnostics =
        compile_to_checked(&main, None).expect_err("missing nested custody leaf must fail closed");
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics =
        compile_to_checked(&main, None).expect_err("nested custody leaf type must agree exactly");
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None).expect("exact depth-two placement custody should compile");
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
        .expect_err("a zero-layout nested wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.envelope",
            "represented at offset 0 with width 4",
            "outside the exact seven-record",
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None).expect("exact depth-three placement custody should compile");
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
        .expect_err("a zero-layout third wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame",
            "represented at offset 0 with width 8",
            "outside the exact seven-record",
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
    compile_to_checked(&main, None).expect("exact depth-four placement custody should compile");
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
        let diagnostics =
            compile_to_checked(&main, None).expect_err("depth-four custody drift must fail closed");
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
    let diagnostics = compile_to_checked(&main, None)
        .expect_err("a zero-layout fourth wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame",
            "outside the exact seven-record",
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
    compile_to_checked(&main, None).expect("exact depth-five placement custody should compile");
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
    let diagnostics = compile_to_checked(&main, None)
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
        let diagnostics =
            compile_to_checked(&main, None).expect_err("depth-five custody drift must fail closed");
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
    let diagnostics = compile_to_checked(&main, None)
        .expect_err("a zero-layout fifth wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame",
            "outside the exact seven-record",
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
    compile_to_checked(&main, None).expect("exact depth-six placement custody should compile");
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
    let diagnostics = compile_to_checked(&main, None)
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
        let diagnostics =
            compile_to_checked(&main, None).expect_err("depth-six custody drift must fail closed");
        assert_diagnostic(&diagnostics, &["Native::plan", expected[0], expected[1]]);
    }
}

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
    let diagnostics = compile_to_checked(&main, None)
        .expect_err("a zero-layout sixth wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame",
            "outside the exact seven-record",
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
    compile_to_checked(&main, None).expect("exact depth-seven placement custody should compile");
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
    let diagnostics = compile_to_checked(&main, None)
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
        .expect_err("a zero-layout seventh wrapper must remain outside the custody cohort");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame",
            "outside the exact seven-record",
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
    let diagnostics = compile_to_checked(&main, None)
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
fn source_placement_custody_keeps_an_eighth_record_level_fenced() {
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
        "pub data Packet {\n    frame: Vault;\n    sibling: Plain;\n}",
        "pub data Strongbox {\n    vault: Vault;\n    marker: u32;\n}\npub data Packet {\n    frame: Strongbox;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 28", "offset: 32", 1)
    .replacen("size_fixed: 32", "size_fixed: 36", 1)
    .replacen(
        "data PacketCustody {\n    frame: VaultCustody;\n}",
        "data StrongboxCustody {\n    vault: VaultCustody;\n}\ndata PacketCustody {\n    frame: StrongboxCustody;\n}",
        1,
    );
    let main = write_program("depth-eight-fenced", &source);
    let diagnostics =
        compile_to_checked(&main, None).expect_err("eighth-level custody must remain fenced");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.frame",
            "outside the exact seven-record",
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
            ["Native::plan", "outside the exact seven-record"],
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
            ["Native::plan", "outside the exact seven-record"],
        ),
    ];
    for (name, source, expected) in cases {
        let main = write_program(name, &source);
        let diagnostics = compile_to_checked(&main, None)
            .expect_err("unsupported depth-three aggregate spines must fail closed");
        assert_diagnostic(&diagnostics, &expected);
    }
}

fn assert_diagnostic(diagnostics: &[psi_diagnostics::Diagnostic], fragments: &[&str]) {
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        fragments.iter().all(|fragment| rendered.contains(fragment)),
        "diagnostic did not contain {fragments:?}:\n{rendered}"
    );
}
