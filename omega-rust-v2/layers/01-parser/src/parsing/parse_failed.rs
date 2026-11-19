//! What every grammar function returns when it cannot go on.

/*
    :SmallFailure
    What every grammar function returns: the thing it parsed, or the fact that
    it failed. The error itself is not in the result. It is written into the
    parser when it is found, and `get_error` hands it over once parsing has
    stopped.

    The reason is size. A `ParseError` is 40 bytes, so a `Result` carrying one
    is returned through memory: every grammar function would write its answer
    to the stack and every caller would read it back, on every call, to carry
    an error that almost never exists. `ParseFailed` has no size, so the common
    results are one byte or a number, and travel in registers. Experiment 5.
*/
#[derive(Clone, Copy, Debug)]
pub(crate) struct ParseFailed;

pub(crate) type Parsed<T = ()> = Result<T, ParseFailed>;
