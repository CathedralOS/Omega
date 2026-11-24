//! Writes the judged mathematical signature into the Psi tables the encoding
//! specifies for it (`wiki/spec/terminal-psi/encoding.md`, "A mathematical
//! carrier row").
//!
//! Only the carrier table is written. Each data declaration gets one row that
//! names its kernel type under the data's source type identity. The laws'
//! certificates are not published: an indexed theorem root row names an
//! evidence formal and an Id formation row names captured source subjects,
//! and a law over plain data has neither (`TASKS.md`, PROOF-KERNEL-CORE).

use terminal_psi::{TerminalMathematicalCarrier, TerminalModule};

use crate::checking::laws::SignatureJudgment;

/// Adds the carrier of each judged data declaration to `module`. A carrier
/// row already present under the same identity must carry the same key.
pub(super) fn publish(
    module: &mut TerminalModule,
    judgment: &SignatureJudgment,
) -> Result<(), &'static str> {
    if judgment.carriers.is_empty() {
        return Ok(());
    }
    let mut identities = std::mem::take(&mut module.identities).into_builder();
    let mut published = Ok(());
    for carrier in &judgment.carriers {
        let type_identity = identities.intern(&carrier.type_identity);
        let rows = &mut module.types.mathematical_carriers;
        match rows.iter().find(|row| row.type_identity == type_identity) {
            Some(row) if row.carrier_key == carrier.carrier_key => {}
            Some(_) => {
                published =
                    Err("a data declaration's type already has another mathematical carrier");
                break;
            }
            None => rows.push(TerminalMathematicalCarrier {
                type_identity,
                carrier_key: carrier.carrier_key.clone(),
            }),
        }
    }
    module.identities = identities.build();
    // The same order stage 05 gives the carriers of theorem parameters.
    module
        .types
        .mathematical_carriers
        .sort_unstable_by_key(|row| row.type_identity);
    published
}
