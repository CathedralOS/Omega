use target::{
    ForeignLocatorCandidate, NormalizedImportLocatorRef, TargetProfile, normalize_foreign_locator,
};

pub(crate) fn check(mut assert_case: impl FnMut(&str, &[NormalizedImportLocatorRef<'_>], bool)) {
    let by_name = |library: &[u8], export: &[u8]| {
        normalize_foreign_locator(
            ForeignLocatorCandidate::PeByName {
                library: library.to_vec(),
                export: export.to_vec(),
            },
            TargetProfile::WindowsX64,
        )
        .unwrap()
    };
    let dll = b"omega_host.dll";
    let by_ordinal = |ordinal| {
        normalize_foreign_locator(
            ForeignLocatorCandidate::PeByOrdinal {
                library: dll.to_vec(),
                ordinal,
            },
            TargetProfile::WindowsX64,
        )
        .unwrap()
    };
    let local = by_name(dll, b"inet_lnaof");
    let make = by_name(dll, b"inet_makeaddr");
    let ordinal_local = by_ordinal(1);
    let ordinal_make = by_ordinal(2);
    let external = by_name(dll, b"application_export");
    let external_ordinal = by_ordinal(3);
    let alternate_case = by_name(b"OMEGA_HOST.DLL", b"application_export");
    let external_known_name = by_name(b"OMEGA_HOST.DLL", b"inet_makeaddr");
    let arm64 = normalize_foreign_locator(
        ForeignLocatorCandidate::PeByName {
            library: dll.to_vec(),
            export: b"inet_makeaddr".to_vec(),
        },
        TargetProfile::WindowsArm64,
    )
    .unwrap();
    let other = by_name(b"application.dll", b"application_export");
    for (label, roster, expected) in [
        ("no imports", &[][..], false),
        ("other library", &[&other][..], false),
        ("named local", &[&local][..], true),
        ("named make", &[&make][..], true),
        ("ordinal local", &[&ordinal_local][..], true),
        ("ordinal make", &[&ordinal_make][..], true),
        (
            "known exports with unrelated DLL",
            &[&make, &ordinal_local, &other][..],
            true,
        ),
        ("no x64 helper on arm64", &[&arm64][..], false),
        ("external spelling alone", &[&external_known_name][..], false),
        ("unknown name", &[&external][..], false),
        (
            "case variant of the same DLL",
            &[&make, &alternate_case][..],
            false,
        ),
        ("unknown ordinal", &[&external_ordinal][..], false),
        (
            "known name then unknown ordinal",
            &[&local, &external_ordinal][..],
            false,
        ),
        (
            "unknown name then known ordinal",
            &[&external, &ordinal_make][..],
            false,
        ),
    ] {
        let locators = roster
            .iter()
            .copied()
            .map(NormalizedImportLocatorRef::Foreign)
            .collect::<Vec<_>>();
        assert_case(label, &locators, expected);
    }
}
