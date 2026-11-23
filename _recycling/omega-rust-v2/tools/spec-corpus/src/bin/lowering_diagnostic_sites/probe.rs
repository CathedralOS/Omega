//! Both compiled and parsed by the scanner test: rustc supplies the expected coordinates.
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

#[track_caller]
fn bad(reason: &str) -> LoweringError {
    LoweringError::new((), reason)
}

mod inner {
    use super::LoweringError;

    #[track_caller]
    pub(super) fn bad(reason: &str) -> LoweringError {
        LoweringError::new((), reason)
    }

    pub(super) fn unqualified() -> LoweringError {
        bad("within inline module")
    }
}

mod branches {
    use super::inner::bad as unsupported_node;

    pub(super) mod patterns {
        use super::super::LoweringError;
        use super::unsupported_node as forwarded;

        pub(in super::super) fn refusals() -> [LoweringError; 2] {
            [
                LoweringError::new((), "inline literal"),
                forwarded("through parent import"),
            ]
        }
    }
}

fn branches() -> [LoweringError; 2] {
    branches::patterns::refusals()
}

#[rustfmt::skip]
pub fn run() -> [LoweringError; 11] {
    // Return in source order so extra candidates (including shadowed calls) fail the test.
    let inside = inner::unqualified();
    let a = self::LoweringError::new(
        (), "multi-line arguments");
    let b = self::LoweringError::
        new((), "split path");
    let c = self::bad(
        "qualified wrapper");
    let bad = |reason| LoweringError::new((), reason);
    let d = bad("ordinary closure");
    let e = inner::bad("qualified inline module helper");
    let f = {
        let result = bad("block fn, before its declaration");
        #[track_caller]
        fn bad(reason: &str) -> LoweringError { LoweringError::new((), reason) }
        result
    };
    let g = {
        fn bad(reason: &str) -> LoweringError { LoweringError::new((), reason) }
        bad("same spelling, ordinary function")
    };
    let h = {
        let (bad,) = (|reason| LoweringError::new((), reason),);
        bad("destructured closure")
    };
    let [literal, forwarded] = branches();
    [inside, literal, forwarded, a, b, c, d, e, f, g, h]
}
