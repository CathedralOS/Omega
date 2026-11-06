//! Semantic obligations for the actual bytes and intervals the CPU will use.
//! These expressions do not emit instructions or choose a register allocation.
use super::Error;
use super::expressions::{Binary as B, Expr, Word};
use super::inputs::Inputs;
use installation_evidence::InstalledInterruptStackRequirement;
use runtime_abi::static_interrupts::{InstalledInterruptFrame, InstalledInterruptProtocol};
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId};
use semantic_vocabulary::OperationId;

fn c(value: u64) -> Expr {
    Expr::constant(value)
}
fn b(kind: B, left: Expr, right: Expr) -> Expr {
    Expr::binary(kind, left, right)
}
fn eq(left: Expr, right: Expr) -> Expr {
    b(B::Equal, left, right)
}
fn le(left: Expr, right: Expr) -> Expr {
    b(B::AtMost, left, right)
}
fn add(left: Expr, right: Expr) -> Expr {
    b(B::Add, left, right)
}
fn mul(left: Expr, right: Expr) -> Expr {
    b(B::Multiply, left, right)
}
fn sub(left: Expr, right: Expr) -> Expr {
    b(B::Subtract, left, right)
}
fn and(left: Expr, right: Expr) -> Expr {
    b(B::And, left, right)
}
fn shift(value: Expr, bits: u64) -> Expr {
    b(B::Left, value, c(bits))
}
fn bounded(pointer: Expr, length: Expr, stride: u32) -> [Expr; 2] {
    [
        le(length.clone(), c(u64::MAX / u64::from(stride))),
        le(
            pointer.clone(),
            add(pointer, mul(length, c(u64::from(stride)))),
        ),
    ]
}

#[derive(Clone, Copy)]
pub(super) enum ReadKind {
    Gate,
    Code(usize),
    Task,
    Stack,
    Arrival,
    Table,
    Control,
}
pub(super) struct ReadInterval {
    pub kind: ReadKind,
    pub address: Expr,
    pub bytes: u32,
}
pub(super) struct EntryInput<'a> {
    pub gate: u32,
    pub address: Expr,
    pub requirement: &'a InstalledInterruptStackRequirement,
    pub stack: calling_conventions::EntryStack,
}
pub(super) struct EntryContract {
    pub dedicated: bool,
    pub before_publication: Vec<Expr>,
    pub selector_guards: Vec<Expr>,
    pub matching_arrival: Expr,
    pub matched_arrival: Expr,
    pub matching_stack: Expr,
    pub matched_stack: Vec<Expr>,
    pub acknowledgement_root: Option<Expr>,
}
pub(super) struct Contract {
    pub home: Expr,
    pub before_publication: Vec<Expr>,
    pub table_index: Expr,
    pub table_limit: Expr,
    pub each_gate: Expr,
    pub arrival_index: Expr,
    pub arrival_limit: Expr,
    pub stack_index: Expr,
    pub stack_limit: Expr,
    pub each_stack: Vec<Expr>,
    pub stack_top: Expr,
    pub entries: Vec<EntryContract>,
    pub pairs: Vec<super::pairs::PairContract>,
    pub read_intervals: Vec<ReadInterval>,
}
impl Contract {
    pub(super) fn derive(
        operation: OperationId,
        input: &Inputs,
        entries: &[EntryInput<'_>],
        validate: selected_instructions::SelectedInstructionId,
    ) -> Result<Self, Error> {
        let layout = &input.layout;
        let [table, gdt, task, stacks, arrivals, _context] = &input.pointers;
        let home = Expr::new(Word::Frame(FrameStorageSlotId::Local(
            LocalStorageSlotId::Boundary { operation },
        )));
        let table_index = home.clone().offset(64).load(8);
        let arrival_index = home.clone().offset(72).load(8);
        let stack_index = home.clone().offset(80).load(8);
        let gate_pointer = add(table.clone(), mul(table_index.clone(), c(16)));
        let arrival = add(
            arrivals.clone(),
            mul(arrival_index.clone(), c(u64::from(layout.arrival_stride))),
        );
        let stack = add(
            stacks.clone(),
            mul(stack_index.clone(), c(u64::from(layout.stack_stride))),
        );
        let stack_ist = stack.clone().offset(layout.stack_ist).load(1);
        let backing = stack.clone().offset(layout.stack_words).load(8);
        let length = stack.clone().offset(layout.stack_words + 8).load(8);
        let end = add(backing.clone(), mul(length.clone(), c(8)));
        let tss_word = add(task.clone(), mul(stack_ist.clone(), c(8))).offset(28);
        let top = tss_word.clone().load(8);
        let mut before_publication = vec![Expr::new(Word::Call(validate))];
        before_publication.extend(bounded(
            stacks.clone(),
            input.stack_length.clone(),
            layout.stack_stride,
        ));
        before_publication.extend(bounded(
            arrivals.clone(),
            input.arrival_length.clone(),
            layout.arrival_stride,
        ));
        before_publication.extend([
            Expr::new(Word::Cpu {
                gdt: gdt.clone(),
                task: task.clone(),
                scratch: home.clone().offset(48),
                bytes: layout.gdt_bytes,
            }),
            eq(input.arrival_length.clone(), c(entries.len() as u64)),
        ]);
        let mut roots = entries.iter().filter_map(|entry| {
            let requirement = entry.requirement;
            requirement
                .arrival
                .as_ref()
                .map(|arrival| arrival.physical_entry)
        });
        let root_interval = if let Some(root) = roots.next() {
            if roots.any(|other| other != root)
                || entries
                    .iter()
                    .any(|entry| entry.requirement.arrival.is_none())
            {
                return Err(Error::Mismatch(
                    "root interval lost its complete exact arrival roster",
                ));
            }
            let observation = |kind| {
                Expr::new(Word::RootBoundary {
                    kind,
                    instruction: None,
                })
            };
            let start = observation(super::expressions::RootBoundary::RetainedStart);
            let end = observation(super::expressions::RootBoundary::ActivationBase);
            before_publication.push(le(start.clone(), end.clone()));
            Some((start, end))
        } else {
            None
        };
        let mut permitted = c(0);
        let mut contracts = Vec::with_capacity(entries.len());
        let mut read_intervals = vec![
            ReadInterval {
                kind: ReadKind::Gate,
                address: gate_pointer.clone().offset(5),
                bytes: 1,
            },
            ReadInterval {
                kind: ReadKind::Task,
                address: tss_word,
                bytes: 8,
            },
            ReadInterval {
                kind: ReadKind::Stack,
                address: stack.clone(),
                bytes: layout.stack_stride,
            },
            ReadInterval {
                kind: ReadKind::Arrival,
                address: arrival.clone(),
                bytes: layout.arrival_stride,
            },
            ReadInterval {
                kind: ReadKind::Table,
                address: table.clone(),
                bytes: layout.table_bytes,
            },
            ReadInterval {
                kind: ReadKind::Control,
                address: home.clone(),
                bytes: 128,
            },
        ];
        for (ordinal, entry) in entries.iter().enumerate() {
            let EntryInput {
                gate,
                address,
                requirement,
                stack,
            } = entry;
            let gate = *gate;
            let dedicated = match stack {
                calling_conventions::EntryStack::Dedicated { .. } => true,
                calling_conventions::EntryStack::Interrupted => false,
                calling_conventions::EntryStack::ProviderSelected => {
                    return Err(Error::Mismatch("gate has no selected stack domain"));
                }
            };
            permitted = b(
                B::Or,
                permitted,
                eq(table_index.clone(), c(u64::from(gate / 16))),
            );
            let ist = table.clone().offset(gate + 4).load(1);
            let selector = table.clone().offset(gate + 2).load(2);
            let descriptor = add(gdt.clone(), selector.clone());
            let selector_guards = vec![
                selector.clone(),
                eq(and(selector.clone(), c(7)), c(0)),
                le(selector, c(u64::from(layout.gdt_bytes - 8))),
            ];
            let entry_address = b(
                B::Or,
                b(
                    B::Or,
                    table.clone().offset(gate).load(2),
                    shift(table.clone().offset(gate + 6).load(2), 16),
                ),
                shift(table.clone().offset(gate + 8).load(4), 32),
            );
            let mut entry_guards = vec![
                eq(table.clone().offset(gate + 5).load(1), c(0x8e)),
                eq(table.clone().offset(gate + 12).load(4), c(0)),
                eq(entry_address, address.clone()),
            ];
            if dedicated {
                entry_guards.extend([ist.clone(), le(ist.clone(), c(7))]);
            } else {
                entry_guards.push(eq(ist.clone(), c(0)));
            }
            entry_guards.extend(selector_guards.iter().cloned());
            entry_guards.extend([
                eq(and(descriptor.clone().offset(5).load(1), c(0xfc)), c(0x98)),
                eq(and(descriptor.clone().offset(6).load(1), c(0x60)), c(0x20)),
            ]);
            let acknowledgement =
                requirement.entry_plan.protocol == InstalledInterruptProtocol::Acknowledgement;
            if acknowledgement
                && (requirement.entry_plan.frame != InstalledInterruptFrame::NoErrorCode
                    || !(32..=255).contains(&(gate / 16)))
            {
                return Err(Error::Mismatch(
                    "returning installation does not retain an external no-error entry",
                ));
            }
            let frame_tag = match requirement.entry_plan.frame {
                InstalledInterruptFrame::NoErrorCode => layout.no_error_tag,
                InstalledInterruptFrame::ErrorCode => layout.error_tag,
                InstalledInterruptFrame::OpaqueFatal => layout.opaque_fatal_tag,
            };
            let mut matched_stack = vec![backing.clone()];
            matched_stack.extend(bounded(backing.clone(), length.clone(), 8));
            if let Some((root_start, root_end)) = &root_interval {
                matched_stack.push(b(
                    B::Or,
                    le(end.clone(), root_start.clone()),
                    le(root_end.clone(), backing.clone()),
                ));
            }
            matched_stack.extend([
                eq(
                    and(
                        top.clone(),
                        c(u64::from(requirement.alignment.checked_sub(1).ok_or(
                            Error::Mismatch("installed stack alignment is zero"),
                        )?)),
                    ),
                    c(0),
                ),
                le(backing.clone(), top.clone()),
                le(top.clone(), end.clone()),
                le(
                    c(requirement.required_bytes().ok_or(Error::Overflow)?),
                    sub(top.clone(), backing.clone()),
                ),
            ]);
            let acknowledgement_root = if acknowledgement {
                let root = sub(
                    top.clone(),
                    c(u64::from(requirement.entry_plan.root_slot_from_top)),
                );
                matched_stack.push(le(backing.clone(), root.clone()));
                Some(root)
            } else {
                None
            };
            contracts.push(EntryContract {
                dedicated,
                before_publication: entry_guards,
                selector_guards,
                matching_arrival: eq(
                    arrival.clone().offset(layout.arrival_vector).load(2),
                    c(u64::from(gate / 16)),
                ),
                matched_arrival: eq(
                    arrival.clone().offset(layout.arrival_frame).load(
                        u8::try_from(layout.frame_bytes).map_err(|_| {
                            Error::Mismatch("installation frame tag exceeds a word")
                        })?,
                    ),
                    c(frame_tag),
                ),
                matching_stack: eq(stack_ist.clone(), ist),
                matched_stack,
                acknowledgement_root,
            });
            read_intervals.push(ReadInterval {
                kind: ReadKind::Code(ordinal),
                address: descriptor.offset(5),
                bytes: 2,
            });
        }
        let pairs = super::pairs::derive(entries, table, &stack, &home, layout)?;
        Ok(Self {
            pairs,
            home,
            before_publication,
            table_index,
            table_limit: c(u64::from(layout.table_bytes / 16)),
            each_gate: b(
                B::Or,
                permitted,
                eq(and(gate_pointer.offset(5).load(1), c(128)), c(0)),
            ),
            arrival_index,
            arrival_limit: input.arrival_length.clone(),
            stack_index,
            stack_limit: input.stack_length.clone(),
            each_stack: vec![stack_ist.clone(), le(stack_ist, c(7))],
            stack_top: top,
            entries: contracts,
            read_intervals,
        })
    }
}
