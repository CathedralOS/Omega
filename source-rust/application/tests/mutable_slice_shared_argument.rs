//! A mutable byte view lent to a shared-view parameter compiles natively.
//!
//! `fill(view: @mutable [u8])` calls `at(@view, 3)` where `at` takes `@[u8]`.
//! The source compiler copies the view's descriptor and lends the copy, so
//! native instruction selection must accept a shared borrow of that copied
//! descriptor; it used to refuse it with "borrowed call argument does not
//! match its retained source geometry and ABI".

#[path = "support/program.rs"]
mod program;
mod support;

#[test]
fn a_mutable_byte_view_lends_to_a_shared_view_parameter_on_both_engines() {
    program::assert_fixture_silent_return(
        &support::repository_root()
            .join("source-rust/application/tests/fixtures/views/run/mutable_slice_shared_argument"),
    );
}
