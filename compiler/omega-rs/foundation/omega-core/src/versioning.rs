//! Shared naming scheme for the builtin `Versioned<T>` container (frozen
//! decision 14, chapter 21 "Version Matching").
//!
//! RECONCILIATION (2026-06-14): chapter 21's new "Decided Model" repositions
//! this container. Live-state hot-swap is now single-step `Upgradable<Old,
//! New>` + an owned `replace` plan (NOT this era-union container); the
//! multi-era coexistence this container implements is reassigned to WIRE DATA
//! (chapter 20). `Versioned<T>` is best understood as the wire-decode era
//! matcher -- do NOT extend it into the live-state upgrade path. Full analysis
//! + stage-4 guidance: wiki/architecture/versioned_data_stage3_reconciliation.md.
//!
//! `Versioned<T>` is the only value shape that carries an era tag:
//! `{ era: u32, payload: union-of-eras }`. It exists for every `data` type
//! that declares `version vN { ... }` blocks, is constructed only by boundary
//! machinery (no source-level constructor), and is the only legal subject for
//! version match arms (`Counter::v1(old) -> ...`).
//!
//! The container is implemented by SYNTHESIS: each versioned data type gets a
//! compiler-generated ordinary data definition named `Versioned<Counter>`
//! whose fields are the era tag plus one payload field per era shape. The
//! names below are the contract between the layers that cooperate on it:
//! the parser (which desugars version match arms into era compares + payload
//! member reads), the symbol-resolved lowering (which synthesizes the
//! definition), the typed lowering (which resolves version-arm membership
//! markers into era equality), and validation (which keeps the fields
//! read-only).
//!
//! INTERIM LAYOUT NOTE: the frozen design says the payload is a UNION of the
//! declared era shapes (static max size). This stage lays the payload fields
//! out as a plain struct (sum of sizes) because nothing can construct a
//! container yet, so the overlap is unobservable. The true union layout lands
//! with the first boundary constructor (wire decode / storage read).

/// The name of the synthesized container type for a versioned data type:
/// `Versioned<Counter>`. Spelled exactly like the source-level generic
/// reference so the type-reference fold and the synthesized definition agree.
pub fn versioned_container_name(data_name: &str) -> String {
    format!("Versioned<{data_name}>")
}

/// The payload type name of a container name: `Versioned<Counter>` ->
/// `Counter`. Returns `None` when the name is not a synthesized container.
pub fn versioned_container_payload(container_name: &str) -> Option<&str> {
    container_name
        .strip_prefix("Versioned<")?
        .strip_suffix('>')
}

/// The read-only era tag field. Era numbering follows frozen decision 10's
/// wire-era assignment so a future wire decoder can store the discriminator
/// unchanged: declared `version vN` blocks count up from era 0 in declaration
/// order, and the CURRENT shape's era is the number of declared blocks. A
/// zero-initialized container therefore reads as the OLDEST declared era with
/// a zero-initialized payload.
pub const VERSIONED_ERA_FIELD: &str = "era";

/// The synthesized payload field holding a historical era shape
/// (`__payload_v1: Counter::v1`). Internal: reachable in source only through
/// the whole-value binding of a version match arm.
pub fn versioned_payload_field_name(version_name: &str) -> String {
    format!("__payload_{version_name}")
}

/// The synthesized payload field holding the CURRENT shape
/// (`__payload_current: Counter`).
pub const VERSIONED_CURRENT_PAYLOAD_FIELD: &str = "__payload_current";

/// The marker member the parser appends to a version match arm's membership
/// domain path (`raw in Counter::v1::<marker>` / `raw in Counter::<marker>`),
/// so the typed lowering can tell a version arm from case/domain membership
/// without re-parsing. Unspellable as a meaningful domain, and claimed before
/// the case/domain lowerings run.
pub const VERSION_ARM_MARKER: &str = "__versioned_arm";

/// A historical-version selector segment: `v` followed by one or more ASCII
/// digits (`v1`, `v2`, ...). Canonical spelling shared by `version vN` blocks,
/// version-scoped machine paths, version-shape type names (`Counter::v1`),
/// and version match arms.
pub fn is_version_selector(name: &str) -> bool {
    let Some(rest) = name.strip_prefix('v') else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit())
}

/// The version-shape suffix of a historical type name: `Counter::v1` ->
/// `(Counter, v1)`. Returns `None` for names without a version selector tail.
pub fn split_version_shape_name(type_name: &str) -> Option<(&str, &str)> {
    let (data_name, version_name) = type_name.rsplit_once("::")?;
    is_version_selector(version_name).then_some((data_name, version_name))
}

/// The migration machine name chapter 21 prescribes for an era:
/// `Counter::from_v1` migrates a `Counter::v1` value up the chain.
pub fn migration_machine_name(data_name: &str, version_name: &str) -> String {
    format!("{data_name}::from_{version_name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_names_round_trip() {
        let name = versioned_container_name("Counter");
        assert_eq!(name, "Versioned<Counter>");
        assert_eq!(versioned_container_payload(&name), Some("Counter"));
        assert_eq!(versioned_container_payload("Counter"), None);
    }

    #[test]
    fn version_selectors() {
        assert!(is_version_selector("v1"));
        assert!(is_version_selector("v12"));
        assert!(!is_version_selector("v"));
        assert!(!is_version_selector("version"));
        assert!(!is_version_selector("current"));
        assert_eq!(
            split_version_shape_name("Counter::v1"),
            Some(("Counter", "v1"))
        );
        assert_eq!(split_version_shape_name("Counter"), None);
        assert_eq!(split_version_shape_name("Counter::from_v1"), None);
    }
}
