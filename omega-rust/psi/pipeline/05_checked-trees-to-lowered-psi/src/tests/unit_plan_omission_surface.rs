//! When the scalar graph declines a machine on a by-design deferral (e.g.
//! `multi-state structural formals` — the Unit graph is meant to own the
//! control shape), the surfaced diagnostic must still carry the Unit plan's
//! omission ledger: that ledger names the operative wall inside local
//! construction (the statement and the gate that stopped it), which the bare
//! scalar decline hides. `DIAGNOSTIC-CHANNEL-FIDELITY` — without this surface
//! a worker sees "no source-independent checked scalar control plan" for a
//! machine whose real blocker is a statement-sequence wall.

use crate::TerminalMachineSelection;
use crate::lower_machine;

/// A multi-state machine with a borrowed fixed array formal and a call-valued
/// element store defers to the Unit graph for control (`multi-state structural
/// formals`) and declines inside local construction at `call count agreement`
/// — the scalar decline must wrap the omission so the operative wall is named.
#[test]
fn scalar_deferral_surfaces_unit_plan_omission() {
    let source = r#"
        machine hex_digit_char(digit: u64) -> u8 {
            transition (digit < 10) {
                true -> (((48 as u8 in Wrapping) + (digit as u8 in Wrapping)) as u8)
                false -> (((55 as u8 in Wrapping) + (digit as u8 in Wrapping)) as u8)
            }
        }
        machine write_hex_upper(out: &mut [u8; 17], value: u64, width: u64) -> u64 {
            transition {
                _ -> fill(out, value, width, 0)
            }
            state fill(out: &mut [u8; 17], value: u64, width: u64, position: u64) -> u64 {
                transition (position < width && position < out.len) {
                    true -> write_digit(out, value, width, position)
                    false -> (width)
                }
            }
            state write_digit(out: &mut [u8; 17], value: u64, width: u64, position: u64) -> u64 {
                let shift_base: u64 = ((((width as u64 in Wrapping) - (1 as u64 in Wrapping)) as u64 in Wrapping) - (position as u64 in Wrapping)) as u64;
                let shift: u64 = ((shift_base as u64 in Wrapping) * (4 as u64 in Wrapping)) as u64;
                let digit: u64 = (((value as u64 in Wrapping) >> (shift as u64 in Wrapping)) % 16) as u64;
                out[position] = hex_digit_char(digit);
                transition {
                    _ -> fill(out, value, width, ((position as u64 in Wrapping) + (1 as u64 in Wrapping)) as u64)
                }
            }
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let error = lower_machine(&checked, TerminalMachineSelection::Name("write_hex_upper"))
        .expect_err("the machine's unit local construction still declines");
    let rendered = error.to_string();
    assert!(
        rendered.contains("has no admitted body (local construction stopped at"),
        "the scalar deferral must surface the unit plan omission; got: {rendered}"
    );
}
