//! Publish the completed product and any Build-output companions retained in its report.

use crate::CompileReport;
use std::path::Path;

pub fn publish_compilation(
    report: CompileReport,
    build_dir: &Path,
) -> Result<CompileReport, String> {
    report.publish(build_dir)
}
