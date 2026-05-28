# Source Files To Tokens

Input: loaded source files.

Output: token streams.

Primary responsibility: preserve source identity and split text into tokens.

Semantic nouns:

- Places: not known.
- Values: not known.
- Facts: not known.
- Loans: not known.
- Moves: not known.
- Drops: not known.
- Calls: not known.
- Transitions: not known.
- Effects: not known.
- Boundary edges: not known, except `boundary` as token text.

Must not own: language meaning, import semantics, symbol resolution.

Known gaps: none; this stage should stay intentionally boring.
