//! CLI diagnostics for required or stale trust admissions.

pub(crate) fn report_unsettled_admissions(settlement: &compiler::TrustAdmissionSettlement) {
    for (label, admissions) in [
        ("unresolved", settlement.unresolved()),
        ("stale", settlement.unused()),
    ] {
        for admission in admissions {
            eprintln!(
                "{label} trust admission `{}` [{}]{}",
                admission.commitment(),
                admission.digest(),
                admission
                    .report_identity()
                    .map(|identity| format!(" (report {identity:016x})"))
                    .unwrap_or_default(),
            );
        }
    }
    eprintln!("use omega --accept-admissions <root.omg> to accept this exact set");
}
