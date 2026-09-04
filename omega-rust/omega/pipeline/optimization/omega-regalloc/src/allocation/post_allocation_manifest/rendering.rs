//! Human rendering projected from the authoritative manifest record.

use std::fmt::Write;

use super::{PostAllocationOptimizationManifest, PostAllocationSelectedTransformation};

impl PostAllocationOptimizationManifest {
    pub fn render_text(&self) -> String {
        let mut output = String::new();
        writeln!(output, "Omega post-allocation optimization manifest").unwrap();
        writeln!(output, "stage: validated register homes").unwrap();
        writeln!(output, "manifest identity: {}", hex(&self.identity.bytes())).unwrap();
        writeln!(
            output,
            "pre-physical manifest: {}",
            hex(&self.pre_physical.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "target: {:?}/{:?}",
            self.target.architecture, self.target.object_format
        )
        .unwrap();
        writeln!(output, "selected plan: {}", hex(&self.selected.bytes())).unwrap();
        match self.selected_lowering_completion {
            Some(identity) => writeln!(
                output,
                "selected-lowering completion: {}",
                hex(&identity.bytes())
            )
            .unwrap(),
            None => writeln!(output, "selected-lowering completion: not run").unwrap(),
        }
        writeln!(
            output,
            "selected transformations: {}",
            self.selected_transformations.len()
        )
        .unwrap();
        for (index, transformation) in self.selected_transformations.iter().enumerate() {
            let (kind, identity) = match transformation {
                PostAllocationSelectedTransformation::FixedViewCopy(identity) => {
                    ("fixed-view-copy", identity.bytes())
                }
                PostAllocationSelectedTransformation::LiteralFold(identity) => {
                    ("literal-fold", identity.bytes())
                }
                PostAllocationSelectedTransformation::PressureRematerialization(identity) => {
                    ("pressure-rematerialization", identity.bytes())
                }
            };
            writeln!(
                output,
                "selected transformation {index}: {kind} {}",
                hex(&identity)
            )
            .unwrap();
        }
        writeln!(output, "register homes: {}", hex(&self.homes.bytes())).unwrap();
        writeln!(
            output,
            "allocator availability: {}",
            hex(&self.allocator_availability.bytes())
        )
        .unwrap();
        writeln!(output, "spills: not required for validated home plan").unwrap();
        writeln!(output, "frame: unavailable").unwrap();
        writeln!(output, "emission: unavailable").unwrap();
        writeln!(output, "publication: unavailable").unwrap();
        writeln!(output, "functions: {}", self.statistics.functions).unwrap();
        writeln!(
            output,
            "structural Unit functions: {}",
            self.statistics.structural_unit_functions
        )
        .unwrap();
        writeln!(output, "assignments: {}", self.statistics.assignments).unwrap();
        writeln!(
            output,
            "distinct physical views: {}",
            self.statistics.distinct_physical_views
        )
        .unwrap();
        writeln!(
            output,
            "virtual interferences: {}",
            self.statistics.virtual_interferences
        )
        .unwrap();
        writeln!(
            output,
            "fixed-view transitions: {}",
            self.statistics.fixed_view_transitions
        )
        .unwrap();
        output
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}
