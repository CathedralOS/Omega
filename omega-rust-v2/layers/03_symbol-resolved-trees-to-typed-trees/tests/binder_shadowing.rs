mod support;
use support::check;

#[test]
fn local_value_names_compete_with_generic_values_after_kind_alias_expansion() {
    let kinds = "public data Identity<T> = T; public data Byte = Identity<u8>;";
    for prefix in ["", "const "] {
        for body in [
            "(size:u8) {}",
            "() {let size:u8=3;}",
            "(limit:u8)->u8 {match limit {size -> size,}}",
            "() {machine inner<size:Byte>() {}}",
        ] {
            let source = format!("use kinds\\Byte; machine measure<{prefix}size:Byte>{body}");
            let error = check(&[("measure.omg", &source), ("kinds/bounds.omg", kinds)]).unwrap_err();
            assert_eq!(error.message, "local binding size shadows a visible value binding");
            assert_eq!(error.file, "measure.omg");
            let second_name = source.match_indices("size").nth(1).unwrap().0;
            assert_eq!(error.span.get_start_offset(), second_name);
            assert_eq!(error.span.slice(source.as_bytes()), Some(b"size".as_slice()));
        }
    }
}

#[test]
fn a_type_binder_and_a_value_binder_can_share_a_spelling() {
    let kinds = "data Identity<T> = T; data Sort = Identity<core\\Type<0>>; data Byte = Identity<u8>;";
    for prefix in ["", "const "] {
        for machine in [
            format!("machine both<{prefix}Entry:Sort>(Entry:Entry)->Entry {{Entry}}"),
            format!("machine local<{prefix}Entry:Sort>(value:Entry)->Entry {{let Entry:Entry=value; Entry}}"),
            format!("machine arm<{prefix}Entry:Sort>(value:Entry)->Entry {{match value {{Entry -> Entry,}}}}"),
            format!("machine outer<size:Byte>() {{machine inner<{prefix}size:Sort>() {{}}}}"),
            format!("machine outer<{prefix}Entry:Sort>() {{machine inner<Entry:Byte>() {{}}}}"),
        ] {
            let source = format!("{kinds} {machine}");
            check(&[("names.omg", &source)]).unwrap_or_else(|error| panic!("{source}: {error:?}"));
        }
    }
}
