/*
Publishing an artifact records the producer's retained custody; receiving one
independently checks that custody against the instructions and calling plans.
Both paths assemble the same children. Only receiver replay decodes or
re-encodes instructions, and its result is compared with the published children.
This choice is internal to artifact construction, never an admission option.
*/

#[derive(Clone, Copy)]
pub(crate) enum PhysicalEvidencePass {
    Publication,
    Receiver,
}

impl PhysicalEvidencePass {
    pub(crate) const fn is_receiver(self) -> bool {
        matches!(self, Self::Receiver)
    }

    pub(crate) fn verify(
        self,
        replay: impl FnOnce() -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        match self {
            Self::Publication => Ok(()),
            Self::Receiver => replay(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PhysicalEvidencePass;

    #[test]
    fn publication_does_not_run_receiver_checks() {
        PhysicalEvidencePass::Publication
            .verify(|| panic!("publication must not replay its own output"))
            .expect("recording producer custody");
        assert_eq!(
            PhysicalEvidencePass::Receiver.verify(|| Err("corrupt instruction")),
            Err("corrupt instruction")
        );
        PhysicalEvidencePass::Receiver
            .verify(|| Ok(()))
            .expect("valid receiver replay");
    }
}
