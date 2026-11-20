#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CrashCause {
    Trap,
    Abort,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub struct PauseEffects {
    pub suspend: bool,
    pub block: bool,
}

/// A missing bound can be inferred when the declaration permits it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclaredPauseBounds {
    pub suspend: Option<bool>,
    pub block: Option<bool>,
}
impl DeclaredPauseBounds {
    pub fn declared(self) -> PauseEffects {
        PauseEffects {
            suspend: self.suspend == Some(true),
            block: self.block == Some(true),
        }
    }
}

impl PauseEffects {
    pub fn union(
        &mut self,
        other: Self,
    ) -> bool {
        let old = *self;
        self.suspend |= other.suspend;
        self.block |= other.block;
        *self != old
    }
    pub fn intersect(
        self,
        other: Self,
    ) -> Self {
        Self {
            suspend: self.suspend && other.suspend,
            block: self.block && other.block,
        }
    }
}
