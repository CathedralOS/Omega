//! Literal-indexed scalar stores locate their referent from the borrowed root
//! pointer plus a static byte offset — never through a stored pointer or
//! descriptor — and keep exact width, neighbor, and custody rules.

use super::{NativeTarget, artifact, ieee_stores, native_function, primitive_stores};

#[test]
fn indexed_stores_publish_on_hosted_targets() {
    for source in [
        "data Record [copy] { value: u16; }
        machine forward(records: &write [Record; 2], value: u16) {
            records[1].value = value;
        }",
        "machine forward(values: &mut [u64; 4], value: u64) {
            values[2] = value;
        }",
    ] {
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let _ = primitive_stores::published_text(source, target);
        }
    }
}

#[test]
fn indexed_element_field_stores_update_original_caller_storage() {
    for access in ["write", "mut"] {
        for assigned in ["value", "17"] {
            let source = format!(
                "data Record [copy] {{ value: u16; }}
                machine forward(records: &{access} [Record; 2], value: u16) {{
                    records[1].value = {assigned};
                }}"
            );
            let (bytes, entry) = primitive_stores::published_text(&source, NativeTarget::host());
            let expected = if assigned == "17" { 17 } else { 0xbeef };
            native_function::assert_c_text(
                &bytes,
                entry,
                &format!(
                    r#"
                #include <stdint.h>
                #include <string.h>
                typedef struct {{ uint16_t value; }} Record;
                extern void omega_entry(uint16_t value, Record *records);
                int main(void) {{
                    struct {{ uint64_t before; Record records[2]; uint64_t after; }} frame;
                    memset(&frame, 0xa5, sizeof frame);
                    frame.records[1].value = {expected};
                    unsigned char expected[sizeof frame];
                    memcpy(expected, &frame, sizeof frame);
                    frame.records[1].value = 0;
                    omega_entry(0xbeef, frame.records);
                    return memcmp(expected, &frame, sizeof frame) != 0;
                }}
            "#
                ),
            );
        }
    }
}

#[test]
fn indexed_scalar_element_stores_preserve_exact_widths_and_neighbors() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    for (scalar, native_scalar, runtime_value, literal) in [
        ("i8", "int8_t", "-128", "-17"),
        ("i16", "int16_t", "-32768", "-17"),
        ("i32", "int32_t", "-2147483647-1", "-17"),
        ("i64", "int64_t", "-9223372036854775807ll-1", "-17"),
        ("u8", "uint8_t", "255", "17"),
        ("u16", "uint16_t", "65535", "17"),
        ("u32", "uint32_t", "4294967295u", "17"),
        ("u64", "uint64_t", "18364758544493064720ull", "17"),
        ("bool", "_Bool", "1", "true"),
        ("bool", "_Bool", "0", "false"),
    ] {
        for access in ["write", "mut"] {
            for assigned in ["value", literal] {
                let source = format!(
                    "machine forward(values: &{access} [{scalar}; 4], value: {scalar}) {{
                        values[2] = {assigned};
                    }}"
                );
                let (bytes, entry) =
                    primitive_stores::published_text(&source, NativeTarget::host());
                let expected = if assigned == "value" {
                    runtime_value
                } else {
                    match literal {
                        "true" => "1",
                        "false" => "0",
                        _ => literal,
                    }
                };
                native_function::assert_c_text(
                    &bytes,
                    entry,
                    &format!(
                        r#"
                    #include <stdint.h>
                    #include <string.h>
                    extern void omega_entry({native_scalar} value, {native_scalar} *values);
                    int main(void) {{
                        struct {{ uint64_t before; {native_scalar} values[4]; uint64_t after; }} frame;
                        memset(&frame, 0xa5, sizeof frame);
                        frame.values[2] = ({native_scalar})({expected});
                        unsigned char expected[sizeof frame];
                        memcpy(expected, &frame, sizeof frame);
                        frame.values[2] = ({native_scalar})(!({expected}));
                        omega_entry(({native_scalar})({runtime_value}), frame.values);
                        return memcmp(expected, &frame, sizeof frame) != 0;
                    }}
                "#
                    ),
                );
            }
        }
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: indexed element stores require a supported Linux or macOS native host");
}

#[test]
fn indexed_ieee_element_stores_preserve_runtime_payloads() {
    for (scalar, native_scalar, native_bits, payloads) in [
        (
            "f32",
            "float",
            "uint32_t",
            "0u, 0x80000000u, 1u, 0x7f800000u, 0xff800000u, 0x7fc12345u, 0xffc54321u",
        ),
        (
            "f64",
            "double",
            "uint64_t",
            "0ull, 0x8000000000000000ull, 1ull, 0x7ff0000000000000ull, 0xfff0000000000000ull, 0x7ff8123456789abcull, 0xfff854321fedcba9ull",
        ),
    ] {
        for access in ["write", "mut"] {
            let source = format!(
                "machine forward(values: &{access} [{scalar}; 4], value: {scalar}) {{
                    values[2] = value;
                }}"
            );
            for target in [
                NativeTarget::linux_x64(),
                NativeTarget::linux_arm64(),
                NativeTarget::macos_arm64(),
                NativeTarget::windows_x64(),
            ] {
                let (bytes, entry) = primitive_stores::published_text(&source, target);
                if target != NativeTarget::host() {
                    continue;
                }
                ieee_stores::assert_ieee_c_text(
                    &bytes,
                    entry,
                    &format!(
                        r#"
                    #include <stdint.h>
                    #include <string.h>
                    extern void omega_entry({native_scalar} value, {native_scalar} *values);
                    int main(void) {{
                        const {native_bits} payloads[] = {{ {payloads} }};
                        for (unsigned occurrence = 0; occurrence < sizeof payloads / sizeof payloads[0]; ++occurrence) {{
                            struct {{ uint64_t before; {native_scalar} values[4]; uint64_t after; }} frame;
                            memset(&frame, 0xa5, sizeof frame);
                            memcpy(&frame.values[2], &payloads[occurrence], sizeof({native_bits}));
                            unsigned char expected[sizeof frame];
                            memcpy(expected, &frame, sizeof frame);
                            memset(&frame.values[2], 0x5a, sizeof({native_bits}));
                            {native_scalar} value;
                            memcpy(&value, &payloads[occurrence], sizeof value);
                            omega_entry(value, frame.values);
                            if (memcmp(expected, &frame, sizeof frame) != 0) return occurrence + 1;
                        }}
                        return 0;
                    }}
                    "#
                    ),
                );
            }
        }
    }
}

#[test]
fn nested_container_index_stores_reach_the_projected_element() {
    for access in ["write", "mut"] {
        let source = format!(
            "data Record [copy] {{ value: u16; }}
            data Container [copy] {{ record: Record; records: [Record; 2]; }}
            machine forward(root: &{access} Container, value: u16) {{
                root.records[1].value = value;
            }}"
        );
        let (bytes, entry) = primitive_stores::published_text(&source, NativeTarget::host());
        native_function::assert_c_text(
            &bytes,
            entry,
            r#"
            #include <stdint.h>
            #include <string.h>
            typedef struct { uint16_t value; } Record;
            typedef struct { Record record; Record records[2]; } Container;
            extern void omega_entry(uint16_t value, Container *root);
            int main(void) {
                struct { uint64_t before; Container root; uint64_t after; } frame;
                memset(&frame, 0xa5, sizeof frame);
                frame.root.records[1].value = 0xbeef;
                unsigned char expected[sizeof frame];
                memcpy(expected, &frame, sizeof frame);
                frame.root.records[1].value = 0;
                omega_entry(0xbeef, &frame.root);
                return memcmp(expected, &frame, sizeof frame) != 0;
            }
        "#,
        );
    }
}

#[test]
fn callee_indexed_store_writes_through_the_call_boundary() {
    let source = "machine stamp(values: &write [u16; 4], value: u16) {
        values[2] = value;
    }
    machine forward(values: &mut [u16; 4], value: u16) {
        stamp(&write values, value);
    }";
    let (bytes, entry) = primitive_stores::published_text(source, NativeTarget::host());
    native_function::assert_c_text(
        &bytes,
        entry,
        r#"
        #include <stdint.h>
        #include <string.h>
        extern void omega_entry(uint16_t value, uint16_t *values);
        int main(void) {
            struct { uint64_t before; uint16_t values[4]; uint64_t after; } frame;
            memset(&frame, 0xa5, sizeof frame);
            frame.values[2] = 0xbeef;
            unsigned char expected[sizeof frame];
            memcpy(expected, &frame, sizeof frame);
            frame.values[2] = 0;
            omega_entry(0xbeef, frame.values);
            return memcmp(expected, &frame, sizeof frame) != 0;
        }
    "#,
    );
}

#[test]
fn write_self_indexed_store_reaches_the_caller_referent() {
    let source = "data Bag [copy] { items: [u16; 4]; }
    machine Bag::fill(&write self, value: u16) {
        self.items[2] = value;
    }
    machine forward(bag: &mut Bag, value: u16) {
        bag.fill(value);
    }";
    let (bytes, entry) = primitive_stores::published_text(source, NativeTarget::host());
    native_function::assert_c_text(
        &bytes,
        entry,
        r#"
        #include <stdint.h>
        #include <string.h>
        typedef struct { uint16_t items[4]; } Bag;
        extern void omega_entry(uint16_t value, Bag *bag);
        int main(void) {
            struct { uint64_t before; Bag bag; uint64_t after; } frame;
            memset(&frame, 0xa5, sizeof frame);
            frame.bag.items[2] = 0xbeef;
            unsigned char expected[sizeof frame];
            memcpy(expected, &frame, sizeof frame);
            frame.bag.items[2] = 0;
            omega_entry(0xbeef, &frame.bag);
            return memcmp(expected, &frame, sizeof frame) != 0;
        }
    "#,
    );
}

#[test]
fn live_element_subloan_admits_parent_receiver_calls() {
    for access in ["write", "mut"] {
        let source = format!(
            "data Record [copy] {{ value: u16; }}
            machine Record::replace(&write self, value: u16) {{ self.value = value; }}
            machine forward(records: &{access} [Record; 2], value: u16) {{
                let held: &write Record = &write records[1];
                records[0].replace(value);
                held.replace(value);
            }}"
        );
        let (bytes, entry) = primitive_stores::published_text(&source, NativeTarget::host());
        native_function::assert_c_text(
            &bytes,
            entry,
            r#"
            #include <stdint.h>
            #include <string.h>
            typedef struct { uint16_t value; } Record;
            extern void omega_entry(uint16_t value, Record *records);
            int main(void) {
                struct { uint64_t before; Record records[2]; uint64_t after; } frame;
                memset(&frame, 0xa5, sizeof frame);
                frame.records[0].value = 0xbeef;
                frame.records[1].value = 0xbeef;
                unsigned char expected[sizeof frame];
                memcpy(expected, &frame, sizeof frame);
                frame.records[0].value = 0;
                frame.records[1].value = 0;
                omega_entry(0xbeef, frame.records);
                return memcmp(expected, &frame, sizeof frame) != 0;
            }
        "#,
        );
    }
}

/// A runtime scalar index whose bound arrives as an ordinary machine contract
/// produces `WriteOnlyIndexedPrimitiveStore` — the path terminates at the
/// array, the index stays a `u64` operand, and the `index < extent` bounds
/// obligation is retained. `requires` is the contract-fact spelling that
/// replaced the revoked `u64 [0..=3]` suffix here; the remaining corpus
/// migration belongs to `REMOVE-BRACKETED-RANGE-ANNOTATIONS`. Omega admission
/// carries the operation into the verified abstract inventory and the
/// ordinary target route now realizes it — a target-lowering refusal fails
/// inside `published_text` rather than being tolerated.
#[test]
fn contract_bound_runtime_index_store_executes_on_host() {
    for access in ["write", "mut"] {
        let source = format!(
            "machine forward(values: &{access} [u16; 4], index: u64)
            requires index <= 3
            {{
                values[index] = 17;
            }}"
        );
        let artifact = artifact(&source);
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let (path, obligation) = module
            .machines
            .iter()
            .flat_map(|machine| machine.blocks.iter())
            .flat_map(|block| block.operations.iter())
            .find_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::WriteOnlyIndexedPrimitiveStore {
                    path,
                    obligation,
                    ..
                } => Some((path, *obligation)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{access}: Terminal Psi keeps the indexed store"));
        assert!(
            path.is_empty(),
            "{access}: the runtime selector is an operand; the path terminates at the array"
        );
        let _ = obligation;
        // Omega admission + optimization inventory accept the operation.
        let _optimized = super::optimize(&artifact);
        // The realized host function writes through the caller-selected
        // element and leaves its neighbors untouched.
        let (bytes, entry) = primitive_stores::published_text(&source, NativeTarget::host());
        native_function::assert_c_text(
            &bytes,
            entry,
            r#"
            #include <stdint.h>
            #include <string.h>
            extern void omega_entry(uint64_t index, uint16_t *values);
            int main(void) {
                struct { uint64_t before; uint16_t values[4]; uint64_t after; } frame;
                memset(&frame, 0xa5, sizeof frame);
                frame.values[2] = 17;
                unsigned char expected[sizeof frame];
                memcpy(expected, &frame, sizeof frame);
                frame.values[2] = 0;
                omega_entry(2, frame.values);
                return memcmp(expected, &frame, sizeof frame) != 0;
            }
        "#,
        );
    }
}

/// A contract-bound runtime index lowers to caller storage: the emitted
/// address is the array base plus index times the element width, so a literal,
/// a parameter, and a computed operand all land on the caller-selected element
/// at its exact width while every neighboring byte stays untouched.
#[test]
fn runtime_indexed_stores_observe_caller_selected_elements() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    for access in ["write", "mut"] {
        for (expression, rendered) in [
            ("17", "17"),
            ("value", "value"),
            ("~value", "((uint16_t)~value)"),
        ] {
            let source = format!(
                "machine forward(values: &{access} [u16; 4], index: u64, value: u16)
                requires index <= 3
                {{
                    values[index] = {expression};
                }}"
            );
            let (bytes, entry) = primitive_stores::published_text(&source, NativeTarget::host());
            native_function::assert_c_text(
                &bytes,
                entry,
                &format!(
                    r#"
                    #include <stdint.h>
                    #include <string.h>
                    extern void omega_entry(uint64_t index, uint16_t value, uint16_t *values);
                    int main(void) {{
                        for (uint64_t index = 0; index < 4; ++index) {{
                            struct {{ uint64_t before; uint16_t values[4]; uint64_t after; }} frame;
                            memset(&frame, 0xa5, sizeof frame);
                            uint16_t value = (uint16_t)(0xbeef - index);
                            frame.values[index] = (uint16_t)({rendered});
                            unsigned char expected[sizeof frame];
                            memcpy(expected, &frame, sizeof frame);
                            frame.values[index] = 0;
                            omega_entry(index, value, frame.values);
                            if (memcmp(expected, &frame, sizeof frame) != 0) return (int)index + 1;
                        }}
                        return 0;
                    }}
                "#
                ),
            );
        }
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: runtime-indexed stores require a supported Linux or macOS native host");
}

/// The runtime index crosses the call boundary: the callee recovers the
/// borrowed parameter address, scales it by the element width, and the
/// caller-selected element moves while its neighbors stay untouched.
#[test]
fn callee_runtime_indexed_store_writes_through_the_call_boundary() {
    for access in ["write", "mut"] {
        let source = format!(
            "machine stamp(values: &write [u16; 4], index: u64, value: u16)
            requires index <= 3
            {{
                values[index] = value;
            }}
            machine forward(values: &{access} [u16; 4], index: u64, value: u16)
            requires index <= 3
            {{
                stamp(&write values, index, value);
            }}"
        );
        let (bytes, entry) = primitive_stores::published_text(&source, NativeTarget::host());
        native_function::assert_c_text(
            &bytes,
            entry,
            r#"
            #include <stdint.h>
            #include <string.h>
            extern void omega_entry(uint64_t index, uint16_t value, uint16_t *values);
            int main(void) {
                for (uint64_t index = 0; index < 4; ++index) {
                    struct { uint64_t before; uint16_t values[4]; uint64_t after; } frame;
                    memset(&frame, 0xa5, sizeof frame);
                    frame.values[index] = (uint16_t)(0xbeef - index);
                    unsigned char expected[sizeof frame];
                    memcpy(expected, &frame, sizeof frame);
                    frame.values[index] = 0;
                    omega_entry(index, (uint16_t)(0xbeef - index), frame.values);
                    if (memcmp(expected, &frame, sizeof frame) != 0) return (int)index + 1;
                }
                return 0;
            }
        "#,
        );
    }
}

/// The runtime-indexed store's write authority and bounds certificate are
/// pinned through independent verification: a shared-borrow destination or a
/// mistyped index fails validation outright, while substituting either the
/// certified index or the bounds obligation leaves the reconstructed
/// `index < extent` proposition without matching evidence.
#[test]
fn runtime_indexed_store_rejects_authority_and_bound_substitution() {
    let unsigned_64 = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .unwrap(),
    );
    for access in ["write", "mut"] {
        let source = format!(
            "machine forward(values: &{access} [u16; 4], index: u64, spare: u64, value: u16)
            requires index <= 3
            {{
                values[index] = value;
            }}"
        );
        let artifact = artifact(&source);
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let bundle = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
        for mutation in 0..4 {
            let mut changed = module.clone();
            let machine = changed
                .machines
                .iter_mut()
                .find(|machine| {
                    machine.blocks.iter().any(|block| {
                        block.operations.iter().any(|operation| {
                            matches!(
                                operation.kind,
                                terminal_psi::OperationKind::WriteOnlyIndexedPrimitiveStore { .. }
                            )
                        })
                    })
                })
                .unwrap();
            match mutation {
                // A shared borrow cannot carry the write.
                0 => {
                    machine.structural_parameters[0].access =
                        terminal_psi::StructuralAccess::SharedBorrow
                }
                _ => {
                    let operation = machine
                        .blocks
                        .iter_mut()
                        .flat_map(|block| block.operations.iter_mut())
                        .find(|operation| {
                            matches!(
                                operation.kind,
                                terminal_psi::OperationKind::WriteOnlyIndexedPrimitiveStore { .. }
                            )
                        })
                        .unwrap();
                    let terminal_psi::OperationKind::WriteOnlyIndexedPrimitiveStore {
                        index,
                        value,
                        obligation,
                        ..
                    } = &mut operation.kind
                    else {
                        unreachable!("matched the indexed store above")
                    };
                    match mutation {
                        // The index operand is the exact u64 selector.
                        1 => *index = *value,
                        // A defined u64 the certificate never covered: the
                        // reconstructed `spare < extent` has no evidence.
                        2 => {
                            *index = machine
                                .parameters
                                .iter()
                                .find(|declaration| {
                                    declaration.scalar_type == unsigned_64
                                        && declaration.id != *index
                                })
                                .unwrap()
                                .id
                        }
                        // A bounds obligation no evidence row discharges.
                        _ => {
                            *obligation = semantic_vocabulary::ObligationId::new(u64::MAX).unwrap()
                        }
                    }
                }
            }
            match mutation {
                0 | 1 => assert!(
                    terminal_verifier::validate_module(&changed).is_err(),
                    "{access}: mutation {mutation}"
                ),
                _ => {
                    assert!(
                        terminal_verifier::validate_module(&changed).is_ok(),
                        "{access}: mutation {mutation} stays inside representation rules"
                    );
                    assert!(
                        terminal_verifier::verify_module(
                            &changed,
                            &bundle,
                            &proof_admission::AdmissionProfile::default()
                        )
                        .is_err(),
                        "{access}: mutation {mutation}"
                    );
                }
            }
        }
    }
}

/// The index bound is checked, never assumed: an index with no in-extent
/// proof — or one whose contract bound cannot discharge `index < extent` —
/// rejects during checking before any bounds obligation is emitted.
#[test]
fn runtime_indexed_stores_reject_indices_without_checked_bounds() {
    for source in [
        "machine forward(values: &write [u16; 4], index: u64) {
            values[index] = 17;
        }",
        "machine forward(values: &mut [u16; 4], index: u64) {
            values[index] = 17;
        }",
        "machine forward(values: &write [u16; 4], index: u64)
        requires index <= 7
        {
            values[index] = 17;
        }",
    ] {
        let error = crate::front_end::checked_program_result(source)
            .expect_err("an index without a checked bound cannot store");
        let rendered = format!("{error:?}");
        assert!(
            rendered.contains("cannot prove index"),
            "unexpected diagnostic: {rendered}"
        );
    }
}

#[test]
fn indexed_store_contracts_reject_access_and_index_substitution() {
    for (source, indexed_field_store) in [
        (
            "data Record [copy] { value: u16; }
            machine forward(records: &write [Record; 2], value: u16) {
                records[1].value = value;
            }",
            true,
        ),
        (
            "machine forward(values: &write [u16; 4], value: u16) {
                values[2] = value;
            }",
            false,
        ),
    ] {
        let module = terminal_codec::decode_module(artifact(source).semantic_bytes()).unwrap();
        for mutation in 0..2 {
            let mut changed = module.clone();
            let store = changed
                .machines
                .iter_mut()
                .find(|machine| {
                    machine.blocks.iter().any(|block| {
                        block.operations.iter().any(|operation| {
                            matches!(
                                operation.kind,
                                terminal_psi::OperationKind::StructuralScalarFieldStore { .. }
                                    | terminal_psi::OperationKind::WriteOnlyPrimitiveStore { .. }
                            )
                        })
                    })
                })
                .unwrap();
            match mutation {
                0 => {
                    store.structural_parameters[0].access =
                        terminal_psi::StructuralAccess::SharedBorrow
                }
                _ => {
                    let operation = store
                        .blocks
                        .iter_mut()
                        .flat_map(|block| block.operations.iter_mut())
                        .find(|operation| {
                            matches!(
                                operation.kind,
                                terminal_psi::OperationKind::StructuralScalarFieldStore { .. }
                                    | terminal_psi::OperationKind::WriteOnlyPrimitiveStore { .. }
                            )
                        })
                        .unwrap();
                    match &mut operation.kind {
                        terminal_psi::OperationKind::StructuralScalarFieldStore {
                            path, ..
                        } if indexed_field_store => {
                            path[0] = terminal_psi::StructuralPathSegment::FixedIndex(7);
                        }
                        terminal_psi::OperationKind::WriteOnlyPrimitiveStore { path, .. }
                            if !indexed_field_store =>
                        {
                            path[0] =
                                semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(7);
                        }
                        _ => unreachable!("matched the opposite store kind"),
                    }
                }
            }
            assert!(
                terminal_verifier::validate_module(&changed).is_err(),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn write_only_indexed_reads_reject_during_checking() {
    for source in [
        "data Record [copy] { value: u16; }
        machine forward(records: &write [Record; 2]) -> u16 {
            records[1].value
        }",
        "machine forward(values: &write [u16; 4]) -> u16 {
            values[2]
        }",
    ] {
        let error = crate::front_end::checked_program_result(source)
            .expect_err("a write-only indexed read cannot grant observation");
        let rendered = format!("{error:?}");
        assert!(
            rendered.contains("write-only"),
            "unexpected diagnostic: {rendered}"
        );
    }
}
