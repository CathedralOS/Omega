# L0 Lexer contract

Status: **signed off by Jarod, 2026-11-18.** His words: "If it's to spec, then
it's to spec I believe." Eleven of the rules below come straight from the
spec. Six, marked *gap*, fill places where the spec is silent; they were
accepted with the sign-off on the basis that they contradict nothing in the
spec and that all 132 spec examples lex under them. If one later clashes with
the spec or with a later layer, it comes back as an owner question in PLAN.md.

## What the layer is

    lex(source bytes) -> tokens, or one error
    lex_into(source bytes, token list) -> the same, into a list the caller reuses

- **Input:** the bytes of one source file.
- **Output:** a list of tokens. Each token is a kind plus a byte range.
- **Refusal:** the first rule a file breaks stops lexing and is reported with
  its kind and byte range. The lexer reports one error per file.

The lexer decides where tokens begin and end and nothing else. It does not
know what any word means.

## Rules

"Spec" names the section the rule comes from. "Gap" means the spec is silent
and the rule is taken from the evidence listed under [Gap decisions](#gap-decisions).
Fixtures are under `fixtures/`; byte-level cases that cannot be written as
readable files are tests in `tests/byte_cases.rs`.

| # | Rule | Source | Accepts | Rejects |
| --- | --- | --- | --- | --- |
| R1 | Source is valid UTF-8. | Spec: [Lexical profile](../../wiki/spec/language/data_and_literals.md#lexical-profile) | every fixture | `invalid_utf8_rejects` |
| R2 | Every source byte belongs to exactly one token, and tokens are in source order with no gaps. Whitespace and comments are tokens. | Design (makes the round-trip check possible) | tiling check on every fixture and spec example | — |
| R3 | Whitespace is space, tab, CR and LF only. | Spec: Lexical profile | `whitespace_and_comments`, `crlf_line_endings_lex` | `form_feed_rejects`, `no_break_space_rejects`, `byte_order_mark_rejects` |
| R4 | A word matches `[A-Za-z_][A-Za-z0-9_]*`. The lexer has no keyword list: `machine`, `true` and `foo` are all words. | Spec: Lexical profile for the shape; *gap G1* for keywords | `words` | `non_ascii_identifier` |
| R5 | Bytes outside ASCII appear only inside comments and quoted literals. | Spec: Lexical profile | `whitespace_and_comments`, `quoted` | `non_ascii_identifier` |
| R6 | `//` starts a comment that runs to the end of the line. | *Gap G2* | `whitespace_and_comments` | — |
| R7 | `/*` starts a comment that runs to the matching `*/`. Block comments nest. | *Gap G2* | `whitespace_and_comments` | `unterminated_block_comment`, `unterminated_nested_block_comment` |
| R8 | An integer is decimal digits, or `0x`, `0b`, `0o` followed by digits of that radix. `_` may appear among the digits. | Spec: [Integer radix spellings](../../wiki/spec/language/data_and_literals.md#integer-radix-spellings); lowercase prefixes only is *gap G4* | `integers` | `invalid_suffix_uppercase_radix` |
| R9 | A radix prefix needs at least one digit, and a decimal digit too large for the radix rejects. | Implied by R8 | `integers` | `missing_radix_digits`, `missing_radix_digits_underscore`, `digit_outside_radix_binary`, `digit_outside_radix_octal` |
| R10 | A float is decimal digits, then `.` and at least one digit, then optionally `e`, an optional sign and at least one digit. Either the fraction or the exponent may be absent, not both. | *Gap G3* | `floats` | `missing_exponent_digits`, `invalid_suffix_uppercase_exponent` |
| R11 | A number may end in a suffix: `i`, `u`, `i` or `u` followed by a positive decimal width with no leading zero, `f32`, or `f64`. Any other letters directly after a number reject. | Spec: Integer radix spellings ("`17u13` or `41u257`", "bare `i`/`u`"); [modules](../../wiki/spec/language/modules.md) ("a literal suffix such as `7u32` is lexical"); `f32` and `f64` are the only float formats the spec names | `integers`, `floats` | `invalid_suffix_usize`, `invalid_suffix_letters`, `invalid_suffix_zero_width`, `invalid_suffix_leading_zero_width`, `invalid_suffix_f16` |
| R12 | A quoted literal is `"`, bytes, `"`. Its escapes are `\n \r \t \0 \\ \"` and `\xNN` with two hex digits. | Spec: [Quoted bytes](../../wiki/spec/language/data_and_literals.md#quoted-bytes) | `quoted` | `unknown_escape`, `invalid_hex_escape`, `short_hex_escape`, `unterminated_quoted` |
| R13 | A codepoint escape (`\u`) rejects, and a raw CR or LF inside quotes rejects. | Spec: Quoted bytes | — | `codepoint_escape`, `newline_in_quoted`, `carriage_return_in_quoted_rejects` |
| R14 | There is no raw literal form: `r"` and `r#"` reject. | Spec: Lexical profile ("no … raw-payload literal form") | `words` (a lone `r` is a word) | `raw_literal_prefix`, `raw_literal_prefix_hashes` |
| R15 | Punctuation is a closed set, listed below. The lexer always takes the longest spelling that matches. | Spec: [expressions](../../wiki/spec/language/expressions.md#operator-declarations) ("the token vocabulary is closed", "lexing takes the longest operator"); the list itself is *gap G5* | `punctuation` | — |
| R16 | There is no `>>` token. Two adjacent `>` are two tokens. | Spec: expressions ("no `>>` token is added") | `punctuation` | — |
| R17 | Any other ASCII character rejects. | Implied by R3, R4, R15 | — | `unknown_character_at`, `unknown_character_question`, `unknown_character_dollar`, `unknown_character_backslash`, `unknown_character_backtick`, `nul_byte_rejects` |

### Punctuation vocabulary (R15)

    +   +%  +|  +!  +=          -   -%  -|  -!  -=  ->
    *   *%  *|  *!  *=          /   /%  /|  /!  /=
    %   %%  %|  %!  %=          <   <=  <<  <<%  <<|  <<!
    >   >=                      =   ==      !   !=
    &   &&      |   ||          :   ::      .   ..  ..=
    '   ^   ~   #   ,   ;       (   )   [   ]   {   }

## What this layer does not check

These are real rules, owned by later layers:

- **Which words are reserved** and whether a word may be used as a name. (L1)
- **Combining adjacent `>` tokens** into a right shift, and its `%`, `|`, `!`
  policy suffix. (L1)
- **Whether a suffix fits its literal**, for example `1.5u8`. (L3)
- **A number's value**, its range, and overflow. (L3)
- **What `'` means.** It is one punctuation token here.

## Gap decisions

Each row is a rule the spec does not state. "Evidence" is what the proposal
rests on. The survey is of the 19,442 `.omg` files under V1's `source`,
`samples` and `tests/fixtures`.

| # | Question | Accepted rule | Evidence |
| --- | --- | --- | --- |
| G1 | Which words does the lexer treat as keywords? | None. All words are one token kind and the parser decides. | The spec has no reserved-word list and says "no identifier is reserved for builtin types". V1's lexer hard-codes 28 keywords, including `state`, which the spec retires. |
| G2 | What is comment syntax? | `//` line comments and nesting `/* */` block comments. | `//` appears on 51 lines of spec examples. The spec never shows `/*`; V1's lexer accepts it with nesting, and 4 surveyed files use it. |
| G3 | How is a float spelled? | Digits are required on both sides of the `.`. No `.5`, no `1.`. | Survey: 0 files use a leading-dot or trailing-dot float; 15 use an exponent. V1's lexer accepts both dotted forms. |
| G4 | Are `0X`, `0B`, `0O` and `E` accepted? | No, lowercase only. | The spec spells them lowercase. Survey: 0 files use an uppercase radix prefix. V1's lexer accepts uppercase. |
| G5 | What is the punctuation vocabulary? | V1's table with `>>` removed. | The spec states the vocabulary is closed but lists it nowhere in one place. |
| G6 | Where may `_` appear in a number? | Anywhere after the first digit, including doubled and trailing. | The spec says "digit underscores" and shows `0x1_0000u32`. V1 is equally permissive. |

## Where this differs from V1's lexer

- **V1 has a `>>` token; the spec says there is none.** V2 follows the spec.
- **V1 classifies 28 keywords.** V2's lexer classifies none (G1).
- **V1 defers malformed numbers.** It lexes `0x`, `1e` and `7usize` as tokens
  with flags for a later stage to reject. V2 rejects them here.
- **V1 accepts `.5`, `1.`, `0X1F` and `1E5`.** V2 does not (G3, G4).
