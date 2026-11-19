//! Throws many short random inputs at the lexer to show it never panics and never returns nonsense.

mod support;

use omega_lexer::lex;

/*
    The fixtures cover inputs somebody thought of. This covers the ones nobody
    did: tens of thousands of short byte strings built from the characters the
    lexer has rules about, in every order.

    We do not check *what* the lexer returns for each input, since we would
    need a second lexer to know the right answer. We check the things that must
    hold for any input at all:

      - the call returns, which in a test means it did not panic;
      - accepted tokens tile the source, and each relexes to itself;
      - a refusal names a non-empty byte range that lies inside the source,
        and when the source is valid UTF-8 the range does not split a character.

    The generator is a fixed-seed xorshift, so a failure reproduces exactly and
    the test needs no dependency.
*/

/// Bytes the lexer treats specially, plus pieces of multi-byte characters and stray invalid bytes.
const INTERESTING_BYTES: &[u8] = b"abefiorux_AEFX0123456789 \t\r\n\"\\/*.+-<>=!&|:'#%^~,;()[]{}@?$`\x00\x0C\xC3\xA9\xE6\x97\xA5\xF0\x9F\x98\x80\xFF\x80";

const INPUT_COUNT: usize = 60_000;
const LONGEST_INPUT_BYTE_COUNT: u64 = 24;
const GENERATOR_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

struct XorShift {
    state: u64,
}

impl XorShift {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;

        self.state
    }
}

#[test]
fn random_inputs_never_panic_and_always_satisfy_the_invariants() {
    let mut generator = XorShift::new(GENERATOR_SEED);
    let mut accepted_count = 0usize;
    let mut refused_count = 0usize;

    for _ in 0..INPUT_COUNT {
        let input_byte_count = generator.next() % (LONGEST_INPUT_BYTE_COUNT + 1);
        let source_bytes: Vec<u8> = (0..input_byte_count)
            .map(|_| INTERESTING_BYTES[(generator.next() % INTERESTING_BYTES.len() as u64) as usize])
            .collect();

        match lex(&source_bytes) {
            Ok(tokens) => {
                accepted_count += 1;

                if let Some(tiling_problem) = support::describe_tiling_problem(&source_bytes, &tokens) {
                    panic!("{source_bytes:?} was accepted but its tokens do not tile it: {tiling_problem}");
                }

                if let Some(relex_problem) = support::describe_relex_problem(&source_bytes, &tokens) {
                    panic!("{source_bytes:?} was accepted but a token is not self-delimiting: {relex_problem}");
                }
            }
            Err(lex_error) => {
                refused_count += 1;

                let error_start = lex_error.get_span().get_start_offset();
                let error_end = lex_error.get_span().get_end_offset();

                assert!(
                    error_start < error_end && error_end <= source_bytes.len(),
                    "{source_bytes:?} was refused with {lex_error:?}, whose span is empty or outside the source"
                );

                if let Ok(source_text) = std::str::from_utf8(&source_bytes) {
                    assert!(
                        source_text.is_char_boundary(error_start) && source_text.is_char_boundary(error_end),
                        "{source_bytes:?} was refused with {lex_error:?}, whose span splits a character"
                    );
                }
            }
        }
    }

    // Both outcomes have to be common, or the generator is not exercising half of the lexer.
    assert!(accepted_count > INPUT_COUNT / 20, "only {accepted_count} of {INPUT_COUNT} inputs were accepted");
    assert!(refused_count > INPUT_COUNT / 20, "only {refused_count} of {INPUT_COUNT} inputs were refused");

    println!("random inputs: {accepted_count} accepted, {refused_count} refused");
}
