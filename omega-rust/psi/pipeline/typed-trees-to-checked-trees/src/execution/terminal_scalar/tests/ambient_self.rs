//! A borrowed `self` stays ambient on the attachment carrier, but states may
//! still read through it: the receiver is one machine-level operand shared by
//! every state, so it never needs a per-edge transfer lane.
use super::checked_program_result;

#[test]
fn scalar_graph_admits_ambient_borrowed_self_field_reads() {
    let source = r#"
        data Filter { width: u64; }
        machine Filter::count(&self, alignment: u64) -> u64
        crashes Abort
        {
            transition alignment > 0 {
                true -> divide(alignment)
                false -> violated(alignment)
            }
            state violated(&self, alignment: u64) -> u64 {
                crash Abort;
            }
            state divide(&self, alignment: u64) -> u64 {
                transition {
                    _ -> (self.width / alignment)
                }
            }
        }
    "#;
    let checked = checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("::count"))
        .unwrap();
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine.symbol)
        .expect("scalar machine reading a borrowed receiver keeps its graph");
    assert_eq!(graph.states.len(), 3);
}
