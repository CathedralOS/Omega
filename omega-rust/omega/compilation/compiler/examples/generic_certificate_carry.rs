//! The generics experiment of the lowered-form checker, on real programs.
//!
//! For each `main.omg` given on the command line it checks the fixture with
//! the ordinary pipeline (the only way to obtain the frozen typed trees and
//! the bound entry) and elaborates the entry's closure. For every machine in
//! that closure that stage 04 generated for a static application of a
//! generic machine, it then:
//!
//! 1. elaborates the generic machine once with its implementation and value
//!    parameters rigid, and discharges it against the requirements'
//!    contracts and its own `requires`;
//! 2. elaborates and discharges one conformance witness per selected
//!    implementation, which is that implementation's refinement proof;
//! 3. carries the generic body's certificates into the generated machine
//!    with `lowered_discharge::reverify_specialization`, where the kernel
//!    decides each against the generated machine's own goal and hypotheses.
//!
//! `concrete` lines show, for comparison, what a search over the generated
//! machine alone proves, with the selected implementation's own contract.
//!
//! Tab-separated lines:
//!
//!     program <path> current=<accepted|rejected> route=<..>
//!     generic <path> <first specialization> <obligation> <kind> <proved|refused> <detail>
//!     concrete <path> <specialization> <obligation> <kind> <proved|refused> <detail>
//!     witness <path> <specialization> <obligation> <kind> <proved|refused> <detail>
//!     carried <path> <specialization> <obligation> <kind> <accepted|refused> <detail>
//!
//! A program the current checker rejects still has typed trees when stage
//! 04 produced them, and elaboration proves nothing, so the same three
//! steps run on it and the `generic` lines show what the lowered-form
//! checker refuses. This is a measuring tool, not a gate, and not part of
//! the shipped CLI.

#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[path = "../tests/support/generic_certificate_carry.rs"]
mod generic_certificate_carry;

use std::path::Path;

use generic_certificate_carry::probe;

fn main() {
    for argument in std::env::args().skip(1) {
        let main = Path::new(&argument);
        let mut lines = Vec::new();
        let line =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| probe(main, &mut lines)))
                .unwrap_or_else(|_| "panicked".to_owned());
        println!("program\t{argument}\t{line}");
        for line in lines {
            println!("{line}");
        }
    }
}
