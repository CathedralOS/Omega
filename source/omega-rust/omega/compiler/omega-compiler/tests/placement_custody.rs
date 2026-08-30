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
            "must be absent",
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
