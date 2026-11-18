//! Plan both compiler-owned Windows exports and their register effects.
use calling_conventions::{MachineRegister as R, StateEffectsEvidence};
use post_allocation_machine::adapters::{AdapterOperation, Scalar32Operation};
use target::Architecture;

use physical_recipes::windows_internet as recipes;
use recipes::WindowsInternetExports;
pub use recipes::{Condition, PlannedAuxiliaryExport, Step};
pub type WindowsInternetProgram = recipes::WindowsInternetProgram<crate::PhysicalRecipeOrigin>;
#[derive(Debug)]
struct AuxiliaryProgram(Vec<Step>);
impl AuxiliaryProgram {
    fn scalar(&mut self, operation: Scalar32Operation) {
        self.0
            .push(Step::Instruction(AdapterOperation::X86Scalar32(operation)));
    }
    fn returns(&mut self) {
        self.0.push(Step::Instruction(AdapterOperation::Return));
    }
    fn label(&mut self, label: usize) {
        self.0.push(Step::Label(label));
    }
    fn branch(&mut self, condition: Option<Condition>, target: usize) {
        self.0.push(Step::Branch { condition, target });
    }
    fn effects(&self) -> StateEffectsEvidence {
        use calling_conventions::{MachineState as S, MachineStateSet, RegisterSet};
        self.0
            .iter()
            .fold(StateEffectsEvidence::empty(), |effects, step| {
                let next = match step {
                    Step::Instruction(operation) => operation.effects(Architecture::X86_64),
                    Step::Label(_) => StateEffectsEvidence::empty(),
                    Step::Branch { condition, .. } => StateEffectsEvidence::new(
                        RegisterSet::new([]),
                        RegisterSet::new([]),
                        MachineStateSet::new(condition.iter().map(|_| S::Flags)),
                        MachineStateSet::new([S::InstructionPointer]),
                    ),
                };
                effects.compose(&next)
            })
    }
}

pub fn plan_windows_internet_provider() -> WindowsInternetProgram {
    use Scalar32Operation as S;
    let mut make = AuxiliaryProgram(Vec::new());
    for (label, limit, mask, shift) in [
        (0, 0x80, 0x00ff_ffff, 24),
        (1, 0x1_0000, 0x0000_ffff, 16),
        (2, 0x100_0000, 0x0000_00ff, 8),
    ] {
        make.scalar(S::Compare {
            register: R::X86Rcx,
            value: limit,
        });
        make.branch(Some(Condition::UnsignedGreaterOrEqual), label);
        make.scalar(S::Move {
            destination: R::X86Rax,
            source: R::X86Rdx,
        });
        make.scalar(S::And {
            register: R::X86Rax,
            value: mask,
        });
        make.scalar(S::ShiftLeft {
            register: R::X86Rcx,
            bits: shift,
        });
        make.scalar(S::Or {
            destination: R::X86Rax,
            source: R::X86Rcx,
        });
        make.branch(None, 3);
        make.label(label);
    }
    make.scalar(S::Move {
        destination: R::X86Rax,
        source: R::X86Rcx,
    });
    make.scalar(S::Or {
        destination: R::X86Rax,
        source: R::X86Rdx,
    });
    make.label(3);
    make.scalar(S::ByteSwap {
        register: R::X86Rax,
    });
    make.returns();

    let mut host = AuxiliaryProgram(Vec::new());
    host.scalar(S::Move {
        destination: R::X86Rax,
        source: R::X86Rcx,
    });
    host.scalar(S::ByteSwap {
        register: R::X86Rax,
    });
    host.scalar(S::Move {
        destination: R::X86Rcx,
        source: R::X86Rax,
    });
    host.scalar(S::Test {
        register: R::X86Rcx,
        value: 0x8000_0000,
    });
    host.branch(Some(Condition::NotEqual), 0);
    host.scalar(S::And {
        register: R::X86Rax,
        value: 0x00ff_ffff,
    });
    host.returns();
    host.label(0);
    host.scalar(S::Move {
        destination: R::X86Rdx,
        source: R::X86Rcx,
    });
    host.scalar(S::And {
        register: R::X86Rdx,
        value: 0xc000_0000,
    });
    host.scalar(S::Compare {
        register: R::X86Rdx,
        value: 0x8000_0000,
    });
    host.branch(Some(Condition::NotEqual), 1);
    host.scalar(S::And {
        register: R::X86Rax,
        value: 0x0000_ffff,
    });
    host.returns();
    host.label(1);
    host.scalar(S::And {
        register: R::X86Rax,
        value: 0x0000_00ff,
    });
    host.returns();
    WindowsInternetProgram::from_planner(
        crate::PhysicalRecipeOrigin(()),
        WindowsInternetExports {
            make_address: make.finish(),
            local_address: host.finish(),
        },
    )
}

impl AuxiliaryProgram {
    fn finish(self) -> PlannedAuxiliaryExport {
        let effects = self.effects();
        PlannedAuxiliaryExport {
            steps: self.0,
            effects,
        }
    }
}
