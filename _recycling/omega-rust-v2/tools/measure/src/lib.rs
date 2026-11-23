/*
    What every layer's measurement tools share.

    Each layer has a tool that says how fast it is: `lex_throughput` for the
    lexer, `parse_throughput` for the parser, and so on. They all do the same
    few things around the part that differs. They read a command line, find
    source files, time some work over several passes, hash the output so two
    builds can be shown to agree, and print numbers. This crate is those few
    things, so that a new layer writes only the part that is about the layer.

    The piece that matters most is the last line a tool prints:

        RESULT per_file_fastest_mbs=735.2 per_file_median_mbs=726.0 fingerprint=b7778a2e5cd741fb

    That line is for a program to read. `compare-builds`, beside this crate,
    builds two versions of a layer, runs each one's tool, and compares these
    lines. Everything a tool prints above that line is for a person.
*/

/*
    How a key says which way is better.

    `compare-builds` knows nothing about lexers or parsers, so it cannot know
    whether a bigger number is good news. The key's ending tells it:

        _mbs              megabytes per second. Higher is better.
        _ms, _us, _ns     a time. Lower is better.
        anything else     text, such as a fingerprint. Only "the same" or
                          "different" means anything.

    We put the unit in the name, where a person reading the line also sees it,
    and did not add a second field for it that the two could disagree about.

    Report the fastest pass, not only the median. Other work on the machine can
    make a pass slower and cannot make one faster, so the fastest pass is the
    number that survives a busy machine. See experiment 9 in the lexer's
    EXPERIMENTS.md, where a second build running alongside pulled whole rounds
    down by 15 percent.
*/

//! A tool written with these pieces, start to finish:
//!
//! ```
//! use measure::{Fingerprint, PassTimings, ResultLine, megabytes_per_second};
//! use std::time::Instant;
//!
//! let source_bytes = b"let a = 1;".repeat(1000);
//! let pass_count = 3;
//!
//! // Time the same work several times. One pass is never enough: the first is often slow, and
//! // any of them can be interrupted.
//! let mut pass_timings = PassTimings::new(pass_count);
//! let mut fingerprint = Fingerprint::new();
//!
//! for _ in 0..pass_count {
//!     let start_time = Instant::now();
//!     let space_count = source_bytes.iter().filter(|&&source_byte| source_byte == b' ').count();
//!
//!     pass_timings.record_seconds(start_time.elapsed().as_secs_f64());
//!
//!     // Hash what the work produced, so two builds can be shown to have produced the same.
//!     fingerprint = Fingerprint::new();
//!     fingerprint.mix(space_count as u64);
//! }
//!
//! // The part for a person can say anything. The RESULT line goes last.
//! let mut result_line = ResultLine::new();
//!
//! result_line.add_number("count_fastest_mbs", megabytes_per_second(source_bytes.len(), pass_timings.get_fastest_seconds()));
//! result_line.add_text("fingerprint", &fingerprint.get_hex_text());
//!
//! println!("{}", result_line.to_line());
//!
//! // `compare-builds` reads it back out of whatever else was printed.
//! let printed_output = format!("some table\n{}\n", result_line.to_line());
//! let read_back = ResultLine::find_in_output(&printed_output).expect("Expected a RESULT line in the output.");
//!
//! assert_eq!(read_back.get_entries().len(), 2);
//! assert_eq!(ResultLine::is_higher_better("count_fastest_mbs"), Some(true));
//! assert_eq!(ResultLine::is_higher_better("fingerprint"), None);
//! ```

#![forbid(unsafe_code)]

mod fingerprint;
mod pass_timings;
mod result_line;
mod source_files;
mod tool_arguments;

pub use fingerprint::Fingerprint;
pub use pass_timings::{PassTimings, megabytes_per_second};
pub use result_line::ResultLine;
pub use source_files::collect_source_files;
pub use tool_arguments::ToolArguments;
