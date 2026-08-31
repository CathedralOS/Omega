//! Independent settlement of exact hosted source and checked ProgramEntry custody.

use crate::tests::fixtures::hosted::hosted_custody;
use crate::{NativeProgramEntrySettlement, validate_native_program_entry_settlement};

#[test]
fn independently_settles_exact_hosted_source_and_entry() {
    let (artifact, receipt, source) = hosted_custody();
    let settlement = validate_native_program_entry_settlement(
        &artifact,
        &receipt,
        NativeProgramEntrySettlement::new(&source, None),
        omega_target::NativeTarget::windows_x64(),
    )
    .expect("independent ProgramEntry settlement");

    assert_eq!(settlement.source(), &source);
    assert_eq!(settlement.checked_entry(), &receipt);
    assert_eq!(
        settlement.target(),
        omega_target::NativeTarget::windows_x64()
    );
    assert!(settlement.semantic_boundary_entry_plan().is_none());
    assert!(settlement.storage_entry().is_none());
}
