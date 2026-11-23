//! Real `#[track_caller]` method calls, compiled by rustc and parsed by the scanner test.
//! Rustc chooses which `refuse` each `self.refuse()` reaches; the scanner records only where
//! each call is written, so a returned location must be checked against that position list.
use std::panic::Location;

pub struct LoweringError(pub &'static Location<'static>);

impl LoweringError {
    #[track_caller]
    fn new(
        _: (),
        _: &str,
    ) -> Self {
        Self(Location::caller())
    }
}

mod remote {
    // Implemented for every type, including `&mut Steps`. For a `&mut self` receiver rustc
    // selects this method before the inherent `Steps::refuse(&self)`.
    pub trait Policy {
        fn refuse(&mut self) -> &'static str {
            "external trait"
        }
    }

    impl<T> Policy for T {}
}

use remote::Policy;

pub struct Steps;

impl Steps {
    #[track_caller]
    fn refuse(&self) -> LoweringError {
        LoweringError::new((), "inherent")
    }

    fn direct(&self) -> LoweringError {
        self.refuse()
    }

    #[rustfmt::skip]
    fn split(&self) -> LoweringError {
        self
            .refuse()
    }

    fn closure(
        &self,
        selected: Option<()>,
    ) -> Result<(), LoweringError> {
        selected.ok_or_else(|| self.refuse())
    }

    fn external(&mut self) -> &'static str {
        self.refuse()
    }
}

pub struct Unrelated;

impl Unrelated {
    fn refuse(&self) -> &'static str {
        "same-named unrelated method"
    }

    fn call(&self) -> &'static str {
        self.refuse()
    }
}

pub fn run() -> ([LoweringError; 3], [&'static str; 2]) {
    let mut steps = Steps;
    let closure = steps.closure(None).expect_err("no operand selected");
    ([steps.direct(), steps.split(), closure], [steps.external(), Unrelated.call()])
}
