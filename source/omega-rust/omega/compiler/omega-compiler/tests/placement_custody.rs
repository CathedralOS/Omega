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
    compile_to_checked(&main, None).expect("exact depth-eight placement custody should compile");
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
    let diagnostics = compile_to_checked(&main, None)
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None).expect("exact depth-nine placement custody should compile");
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
    let diagnostics = compile_to_checked(&main, None)
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
        let diagnostics =
            compile_to_checked(&main, None).expect_err("depth-nine custody drift must fail closed");
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None).expect("exact depth-ten placement custody should compile");
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
    let diagnostics = compile_to_checked(&main, None)
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
        let diagnostics =
            compile_to_checked(&main, None).expect_err("depth-ten custody drift must fail closed");
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None).expect("exact depth-eleven placement custody should compile");
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None).expect("exact depth-twelve placement custody should compile");
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None).expect("exact depth-thirteen placement custody should compile");
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None).expect("exact depth-fourteen placement custody should compile");
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None).expect("exact depth-fifteen placement custody should compile");
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None).expect("exact depth-sixteen placement custody should compile");
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None)
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None).expect("exact depth-eighteen placement custody should compile");
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None).expect("exact depth-nineteen placement custody should compile");
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None).expect("exact depth-twenty placement custody should compile");
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None)
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None)
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None)
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    compile_to_checked(&main, None)
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
        let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics = compile_to_checked(&main, None)
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
    let diagnostics =
        compile_to_checked(&main, None).expect_err("twenty-fifth-level custody must remain fenced");
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
