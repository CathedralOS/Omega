use super::{accepted, guest_discard::host_request};
use std::{cell::Cell, rc::Rc};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionStatus,
    TerminalImportAdapter, TerminalImportSignature, TerminalInstance, TerminalInstantiation,
    TerminalProgram, TerminalSuspension,
};

// Both calls park at the single instruction inside visit. Source identity
// must not distinguish polls of one visit or conflate two actual visits.
const PRODUCER: &str = r"
boundary trait Gate { machine park() reaches none crashes none suspends; }
machine visit() reaches Gate invokes Gate suspends { suspend Gate::park(); }
machine main() -> u8 reaches Gate invokes Gate suspends {
    suspend visit();
    suspend visit();
    7
}";

struct Gate {
    signature: TerminalImportSignature,
    ready: Rc<Cell<bool>>,
}

impl TerminalImportAdapter for Gate {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalEffectHandler for Gate {
    fn poll_effect_readiness(
        &mut self,
        _: &TerminalEffect,
    ) -> Result<bool, TerminalEffectRejection> {
        Ok(self.ready.get())
    }
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        assert!(self.ready.replace(false));
        Ok(())
    }
}

fn parked(instance: &mut TerminalInstance<'_, '_>) -> TerminalSuspension {
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::unbounded())
        .unwrap();
    let TerminalExecutionStatus::AwaitingEffect(park) = status else {
        panic!("producer should be parked: {status:?}")
    };
    park
}

#[test]
fn a_park_belongs_to_one_visit_in_one_activation() {
    let checked = accepted(PRODUCER).publish_invocation().unwrap();
    let program = TerminalProgram::borrow(checked.artifact.view()).unwrap();
    let gate = program.imports().next().unwrap();
    let supplies = program
        .admit_runtime_supplies(host_request(&[gate.id]))
        .unwrap();
    let start = || {
        let ready = Rc::new(Cell::new(false));
        let mut instance = program
            .instantiate(TerminalInstantiation {
                installation: Some(&supplies),
                ..Default::default()
            })
            .unwrap();
        instance
            .bind_import(
                gate.id,
                Gate {
                    signature: TerminalImportSignature::from(gate),
                    ready: Rc::clone(&ready),
                },
            )
            .unwrap();
        (instance, ready)
    };
    let (mut left, advance) = start();
    let (mut right, advance_right) = start();
    let first = parked(&mut left);
    let other = parked(&mut right);
    assert_eq!(first.site, other.site);
    assert_ne!(first, other);
    let mut meter = terminal_work::WorkMeter::unbounded();
    assert_eq!(
        left.recheck_suspension(&first, &mut meter).unwrap(),
        Some(TerminalExecutionStatus::AwaitingEffect(first.clone()))
    );

    advance.set(true);
    assert_eq!(left.recheck_suspension(&other, &mut meter).unwrap(), None);
    assert!(
        advance.get(),
        "a notification for the other producer consumed readiness"
    );
    let Some(TerminalExecutionStatus::AwaitingEffect(second)) =
        left.recheck_suspension(&first, &mut meter).unwrap()
    else {
        panic!("first visit should advance to the second");
    };
    assert_eq!(second.site, first.site);
    assert_ne!(second, first);
    assert_eq!(left.recheck_suspension(&first, &mut meter).unwrap(), None);
    assert_eq!(parked(&mut right), other);
    advance_right.set(true);
    let _second_right = parked(&mut right);
    for (mut instance, ready) in [(left, advance), (right, advance_right)] {
        ready.set(true);
        let result = instance
            .invoke(&mut terminal_work::WorkMeter::unbounded())
            .unwrap();
        assert!(
            matches!(
                result,
                TerminalExecutionStatus::Complete(
                    terminal_interpreter::TerminalExecutionResult::Scalar(
                        terminal_interpreter::TerminalScalarValue::Integer {
                            value: semantic_vocabulary::IntegerValue::Unsigned(7),
                            ..
                        }
                    )
                )
            ),
            "{result:?}"
        );
        instance.close().unwrap();
    }
}
