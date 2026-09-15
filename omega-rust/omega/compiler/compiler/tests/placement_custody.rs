//! Fixtures shared by the placement custody tests: written programs and
//! nested sources of every depth.

#[path = "placement_custody/deep_back_edges.rs"]
mod deep_back_edges;
#[path = "placement_custody/deepest_back_edges.rs"]
mod deepest_back_edges;
#[path = "placement_custody/mid_depth_back_edges.rs"]
mod mid_depth_back_edges;
#[path = "placement_custody/placement_sources.rs"]
mod placement_sources;
#[path = "placement_custody/shallow_projection_paths.rs"]
mod shallow_projection_paths;

use std::fs;
use std::path::PathBuf;

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
    let mut owned_entries: [FieldEntry; 64];
    owned_entries[0] = FieldEntry {{
        key: schema.fields[0].key,
        placement: FieldPlan::At {{ offset: 0 }},
    }};
    PlacementPlan {{
        layout: Plan {{
            entries: owned_entries,
            entry_count: 1,
            size_fixed: 4,
            size_is_dynamic: false,
            align: 4,
        }},
        access: AccessPlan::inaccessible(&schema),
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
    let mut owned_entries: [FieldEntry; 64];
    owned_entries[0] = FieldEntry {{
        key: schema.fields[0].key,
        placement: FieldPlan::At {{ offset: 0 }},
    }};
    owned_entries[1] = FieldEntry {{
        key: schema.fields[1].key,
        placement: FieldPlan::At {{ offset: 4 }},
    }};
    PlacementPlan {{
        layout: Plan {{
            entries: owned_entries,
            entry_count: 2,
            size_fixed: 8,
            size_is_dynamic: false,
            align: 4,
        }},
        access: AccessPlan::inaccessible(&schema),
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
    let mut owned_entries: [FieldEntry; 64];
    owned_entries[0] = FieldEntry {{
        key: schema.fields[0].key,
        placement: FieldPlan::At {{ offset: 0 }},
    }};
    owned_entries[1] = FieldEntry {{
        key: schema.fields[1].key,
        placement: FieldPlan::At {{ offset: 8 }},
    }};
    PlacementPlan {{
        layout: Plan {{
            entries: owned_entries,
            entry_count: 2,
            size_fixed: 12,
            size_is_dynamic: false,
            align: 4,
        }},
        access: AccessPlan::inaccessible(&schema),
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
    let mut owned_entries: [FieldEntry; 64];
    owned_entries[0] = FieldEntry {{
        key: schema.fields[0].key,
        placement: FieldPlan::At {{ offset: 0 }},
    }};
    owned_entries[1] = FieldEntry {{
        key: schema.fields[1].key,
        placement: FieldPlan::At {{ offset: 12 }},
    }};
    PlacementPlan {{
        layout: Plan {{
            entries: owned_entries,
            entry_count: 2,
            size_fixed: 16,
            size_is_dynamic: false,
            align: 4,
        }},
        access: AccessPlan::inaccessible(&schema),
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

fn depth_eight_nested_source(
    header_custody_fields: &str,
    envelope_custody_fields: &str,
    frame_custody_fields: &str,
    boxed_custody_fields: &str,
    crate_custody_fields: &str,
    chest_custody_fields: &str,
    vault_custody_fields: &str,
    strongbox_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_seven_nested_source(
        header_custody_fields,
        envelope_custody_fields,
        frame_custody_fields,
        boxed_custody_fields,
        crate_custody_fields,
        chest_custody_fields,
        vault_custody_fields,
        packet_custody_fields,
    )
    .replacen(
        "pub data Packet {\n    frame: Vault;\n    sibling: Plain;\n}",
        "pub data Strongbox {\n    vault: Vault;\n    marker: u32;\n}\npub data Packet {\n    frame: Strongbox;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 28", "offset: 32", 1)
    .replacen("size_fixed: 32", "size_fixed: 36", 1)
    .replacen(
        &format!("data PacketCustody {{\n{packet_custody_fields}\n}}"),
        &format!(
            "data StrongboxCustody {{\n{strongbox_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_nine_nested_source(
    header_custody_fields: &str,
    envelope_custody_fields: &str,
    frame_custody_fields: &str,
    boxed_custody_fields: &str,
    crate_custody_fields: &str,
    chest_custody_fields: &str,
    vault_custody_fields: &str,
    strongbox_custody_fields: &str,
    lockbox_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_eight_nested_source(
        header_custody_fields,
        envelope_custody_fields,
        frame_custody_fields,
        boxed_custody_fields,
        crate_custody_fields,
        chest_custody_fields,
        vault_custody_fields,
        strongbox_custody_fields,
        packet_custody_fields,
    )
    .replacen(
        "pub data Packet {\n    frame: Strongbox;\n    sibling: Plain;\n}",
        "pub data Lockbox {\n    strongbox: Strongbox;\n    marker: u32;\n}\npub data Packet {\n    frame: Lockbox;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 32", "offset: 36", 1)
    .replacen("size_fixed: 36", "size_fixed: 40", 1)
    .replacen(
        &format!("data PacketCustody {{\n{packet_custody_fields}\n}}"),
        &format!(
            "data LockboxCustody {{\n{lockbox_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_ten_nested_source(
    header_custody_fields: &str,
    envelope_custody_fields: &str,
    frame_custody_fields: &str,
    boxed_custody_fields: &str,
    crate_custody_fields: &str,
    chest_custody_fields: &str,
    vault_custody_fields: &str,
    strongbox_custody_fields: &str,
    lockbox_custody_fields: &str,
    coffer_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_nine_nested_source(
        header_custody_fields,
        envelope_custody_fields,
        frame_custody_fields,
        boxed_custody_fields,
        crate_custody_fields,
        chest_custody_fields,
        vault_custody_fields,
        strongbox_custody_fields,
        lockbox_custody_fields,
        packet_custody_fields,
    )
    .replacen(
        "pub data Packet {\n    frame: Lockbox;\n    sibling: Plain;\n}",
        "pub data Coffer {\n    lockbox: Lockbox;\n    marker: u32;\n}\npub data Packet {\n    frame: Coffer;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 36", "offset: 40", 1)
    .replacen("size_fixed: 40", "size_fixed: 44", 1)
    .replacen(
        &format!("data PacketCustody {{\n{packet_custody_fields}\n}}"),
        &format!(
            "data CofferCustody {{\n{coffer_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

#[allow(clippy::too_many_arguments)]
fn depth_eleven_nested_source(
    header_custody_fields: &str,
    envelope_custody_fields: &str,
    frame_custody_fields: &str,
    boxed_custody_fields: &str,
    crate_custody_fields: &str,
    chest_custody_fields: &str,
    vault_custody_fields: &str,
    strongbox_custody_fields: &str,
    lockbox_custody_fields: &str,
    coffer_custody_fields: &str,
    casket_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_ten_nested_source(
        header_custody_fields,
        envelope_custody_fields,
        frame_custody_fields,
        boxed_custody_fields,
        crate_custody_fields,
        chest_custody_fields,
        vault_custody_fields,
        strongbox_custody_fields,
        lockbox_custody_fields,
        coffer_custody_fields,
        packet_custody_fields,
    )
    .replacen(
        "pub data Packet {\n    frame: Coffer;\n    sibling: Plain;\n}",
        "pub data Casket {\n    coffer: Coffer;\n    marker: u32;\n}\npub data Packet {\n    frame: Casket;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 40", "offset: 44", 1)
    .replacen("size_fixed: 44", "size_fixed: 48", 1)
    .replacen(
        &format!("data PacketCustody {{\n{packet_custody_fields}\n}}"),
        &format!(
            "data CasketCustody {{\n{casket_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_eleven_source_with(
    header_custody_fields: &str,
    casket_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_eleven_nested_source(
        header_custody_fields,
        "    header: HeaderCustody;",
        "    envelope: EnvelopeCustody;",
        "    frame: FrameCustody;",
        "    boxed: BoxedCustody;",
        "    item: CrateCustody;",
        "    chest: ChestCustody;",
        "    vault: VaultCustody;",
        "    strongbox: StrongboxCustody;",
        "    lockbox: LockboxCustody;",
        casket_custody_fields,
        packet_custody_fields,
    )
}

fn depth_twelve_source_with(
    header_custody_fields: &str,
    reliquary_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_eleven_source_with(
        header_custody_fields,
        "    coffer: CofferCustody;",
        "    frame: CasketCustody;",
    )
    .replacen(
        "pub data Packet {\n    frame: Casket;\n    sibling: Plain;\n}",
        "pub data Reliquary {\n    casket: Casket;\n    marker: u32;\n}\npub data Packet {\n    frame: Reliquary;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 44", "offset: 48", 1)
    .replacen("size_fixed: 48", "size_fixed: 52", 1)
    .replacen(
        "data PacketCustody {\n    frame: CasketCustody;\n}",
        &format!(
            "data ReliquaryCustody {{\n{reliquary_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_thirteen_source_with(
    header_custody_fields: &str,
    shrine_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_twelve_source_with(
        header_custody_fields,
        "    casket: CasketCustody;",
        "    frame: ReliquaryCustody;",
    )
    .replacen(
        "pub data Packet {\n    frame: Reliquary;\n    sibling: Plain;\n}",
        "pub data Shrine {\n    reliquary: Reliquary;\n    marker: u32;\n}\npub data Packet {\n    frame: Shrine;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 48", "offset: 52", 1)
    .replacen("size_fixed: 52", "size_fixed: 56", 1)
    .replacen(
        "data PacketCustody {\n    frame: ReliquaryCustody;\n}",
        &format!(
            "data ShrineCustody {{\n{shrine_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_fourteen_source_with(
    header_custody_fields: &str,
    sanctum_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_thirteen_source_with(
        header_custody_fields,
        "    reliquary: ReliquaryCustody;",
        "    frame: ShrineCustody;",
    )
    .replacen(
        "pub data Packet {\n    frame: Shrine;\n    sibling: Plain;\n}",
        "pub data Sanctum {\n    shrine: Shrine;\n    marker: u32;\n}\npub data Packet {\n    frame: Sanctum;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 52", "offset: 56", 1)
    .replacen("size_fixed: 56", "size_fixed: 60", 1)
    .replacen(
        "data PacketCustody {\n    frame: ShrineCustody;\n}",
        &format!(
            "data SanctumCustody {{\n{sanctum_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_fifteen_source_with(
    header_custody_fields: &str,
    tabernacle_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_fourteen_source_with(
        header_custody_fields,
        "    shrine: ShrineCustody;",
        "    frame: SanctumCustody;",
    )
    .replacen(
        "pub data Packet {\n    frame: Sanctum;\n    sibling: Plain;\n}",
        "pub data Tabernacle {\n    sanctum: Sanctum;\n    marker: u32;\n}\npub data Packet {\n    frame: Tabernacle;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 56", "offset: 60", 1)
    .replacen("size_fixed: 60", "size_fixed: 64", 1)
    .replacen(
        "data PacketCustody {\n    frame: SanctumCustody;\n}",
        &format!(
            "data TabernacleCustody {{\n{tabernacle_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_sixteen_source_with(
    header_custody_fields: &str,
    chapel_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_fifteen_source_with(
        header_custody_fields,
        "    sanctum: SanctumCustody;",
        "    frame: TabernacleCustody;",
    )
    .replacen(
        "pub data Packet {\n    frame: Tabernacle;\n    sibling: Plain;\n}",
        "pub data Chapel {\n    tabernacle: Tabernacle;\n    marker: u32;\n}\npub data Packet {\n    frame: Chapel;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 60", "offset: 64", 1)
    .replacen("size_fixed: 64", "size_fixed: 68", 1)
    .replacen(
        "data PacketCustody {\n    frame: TabernacleCustody;\n}",
        &format!(
            "data ChapelCustody {{\n{chapel_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_seventeen_source_with(
    header_custody_fields: &str,
    basilica_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_sixteen_source_with(
        header_custody_fields,
        "    tabernacle: TabernacleCustody;",
        "    frame: ChapelCustody;",
    )
    .replacen(
        "pub data Packet {\n    frame: Chapel;\n    sibling: Plain;\n}",
        "pub data Basilica {\n    chapel: Chapel;\n    marker: u32;\n}\npub data Packet {\n    frame: Basilica;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 64", "offset: 68", 1)
    .replacen("size_fixed: 68", "size_fixed: 72", 1)
    .replacen(
        "data PacketCustody {\n    frame: ChapelCustody;\n}",
        &format!(
            "data BasilicaCustody {{\n{basilica_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_eighteen_source_with(
    header_custody_fields: &str,
    cathedral_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_seventeen_source_with(
        header_custody_fields,
        "    chapel: ChapelCustody;",
        "    frame: BasilicaCustody;",
    )
    .replacen(
        "pub data Packet {\n    frame: Basilica;\n    sibling: Plain;\n}",
        "pub data Cathedral {\n    basilica: Basilica;\n    marker: u32;\n}\npub data Packet {\n    frame: Cathedral;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 68", "offset: 72", 1)
    .replacen("size_fixed: 72", "size_fixed: 76", 1)
    .replacen(
        "data PacketCustody {\n    frame: BasilicaCustody;\n}",
        &format!(
            "data CathedralCustody {{\n{cathedral_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_nineteen_source_with(
    header_custody_fields: &str,
    abbey_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_eighteen_source_with(
        header_custody_fields,
        "    basilica: BasilicaCustody;",
        "    frame: CathedralCustody;",
    )
    .replacen(
        "pub data Packet {\n    frame: Cathedral;\n    sibling: Plain;\n}",
        "pub data Abbey {\n    cathedral: Cathedral;\n    marker: u32;\n}\npub data Packet {\n    frame: Abbey;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 72", "offset: 76", 1)
    .replacen("size_fixed: 76", "size_fixed: 80", 1)
    .replacen(
        "data PacketCustody {\n    frame: CathedralCustody;\n}",
        &format!(
            "data AbbeyCustody {{\n{abbey_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_twenty_source_with(
    header_custody_fields: &str,
    monastery_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_nineteen_source_with(
        header_custody_fields,
        "    cathedral: CathedralCustody;",
        "    frame: AbbeyCustody;",
    )
    .replacen(
        "pub data Packet {\n    frame: Abbey;\n    sibling: Plain;\n}",
        "pub data Monastery {\n    abbey: Abbey;\n    marker: u32;\n}\npub data Packet {\n    frame: Monastery;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 76", "offset: 80", 1)
    .replacen("size_fixed: 80", "size_fixed: 84", 1)
    .replacen(
        "data PacketCustody {\n    frame: AbbeyCustody;\n}",
        &format!(
            "data MonasteryCustody {{\n{monastery_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_twenty_one_source_with(
    header_custody_fields: &str,
    priory_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_twenty_source_with(
        header_custody_fields,
        "    abbey: AbbeyCustody;",
        "    frame: MonasteryCustody;",
    )
    .replacen(
        "pub data Packet {\n    frame: Monastery;\n    sibling: Plain;\n}",
        "pub data Priory {\n    monastery: Monastery;\n    marker: u32;\n}\npub data Packet {\n    frame: Priory;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 80", "offset: 84", 1)
    .replacen("size_fixed: 84", "size_fixed: 88", 1)
    .replacen(
        "data PacketCustody {\n    frame: MonasteryCustody;\n}",
        &format!(
            "data PrioryCustody {{\n{priory_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_twenty_two_source_with(
    header_custody_fields: &str,
    cloister_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_twenty_one_source_with(
        header_custody_fields,
        "    monastery: MonasteryCustody;",
        "    frame: PrioryCustody;",
    )
    .replacen(
        "pub data Packet {\n    frame: Priory;\n    sibling: Plain;\n}",
        "pub data Cloister {\n    priory: Priory;\n    marker: u32;\n}\npub data Packet {\n    frame: Cloister;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 84", "offset: 88", 1)
    .replacen("size_fixed: 88", "size_fixed: 92", 1)
    .replacen(
        "data PacketCustody {\n    frame: PrioryCustody;\n}",
        &format!(
            "data CloisterCustody {{\n{cloister_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_twenty_three_source_with(
    header_custody_fields: &str,
    abbey_seat_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_twenty_two_source_with(
        header_custody_fields,
        "    priory: PrioryCustody;",
        "    frame: CloisterCustody;",
    )
    .replacen(
        "pub data Packet {\n    frame: Cloister;\n    sibling: Plain;\n}",
        "pub data AbbeySeat {\n    cloister: Cloister;\n    marker: u32;\n}\npub data Packet {\n    frame: AbbeySeat;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 88", "offset: 92", 1)
    .replacen("size_fixed: 92", "size_fixed: 96", 1)
    .replacen(
        "data PacketCustody {\n    frame: CloisterCustody;\n}",
        &format!(
            "data AbbeySeatCustody {{\n{abbey_seat_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn depth_twenty_four_source_with(
    header_custody_fields: &str,
    chapter_house_custody_fields: &str,
    packet_custody_fields: &str,
) -> String {
    depth_twenty_three_source_with(
        header_custody_fields,
        "    cloister: CloisterCustody;",
        "    frame: AbbeySeatCustody;",
    )
    .replacen(
        "pub data Packet {\n    frame: AbbeySeat;\n    sibling: Plain;\n}",
        "pub data ChapterHouse {\n    abbey_seat: AbbeySeat;\n    marker: u32;\n}\npub data Packet {\n    frame: ChapterHouse;\n    sibling: Plain;\n}",
        1,
    )
    .replacen("offset: 92", "offset: 96", 1)
    .replacen("size_fixed: 96", "size_fixed: 100", 1)
    .replacen(
        "data PacketCustody {\n    frame: AbbeySeatCustody;\n}",
        &format!(
            "data ChapterHouseCustody {{\n{chapter_house_custody_fields}\n}}\ndata PacketCustody {{\n{packet_custody_fields}\n}}"
        ),
        1,
    )
}

fn assert_diagnostic(diagnostics: &[diagnostics::Diagnostic], fragments: &[&str]) {
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
