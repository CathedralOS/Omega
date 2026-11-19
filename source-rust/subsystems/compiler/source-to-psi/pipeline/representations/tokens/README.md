# Tokens

Start at [token_stream.rs](token_stream.rs). `TokenStream` owns ordered token
records, source backing and decoded literal bytes. `TokenScratch`, beside it,
provides optional buffer reuse for stream builders. [lib.rs](lib.rs) only exposes this owner to
Rust callers; it contains no representation logic.

| Follow the field or type | Definition |
| --- | --- |
| `TokenStream.tokens` | [lexemes/token.rs](lexemes/token.rs): compact token records and kind queries |
| `Token.kind` | [lexemes/kinds.rs](lexemes/kinds.rs): lexical categories and numeric spelling metadata |
| `KeywordKind` | [lexemes/keywords.rs](lexemes/keywords.rs): keyword vocabulary and classification |
| `PunctuationKind` | [lexemes/punctuation.rs](lexemes/punctuation.rs): punctuation vocabulary and longest-match lookup |
| `spelling` and `literal_bytes` | [token_stream.rs](token_stream.rs) returns the original UTF-8 spelling for every token. Quoted literals also expose their decoded bytes. |
| `LexedSource`, `LexedSources` | [files/lexed.rs](files/lexed.rs): file identity, path, scope and token-stream batches |

The [lexer](../../00_source-files-to-tokens/lexer.rs) produces these
streams; the [parser](../../01_tokens-to-syntax-trees/parser.rs) consumes
them. Each file gets its text from its token stream. Decoded strings use stream-owned
byte ranges. Buffer recycling, retained-source identity and logical equality
stay with the stream whose storage they describe.

[tests/storage.rs](tests/storage.rs) checks storage and source retention;
[tests/punctuation.rs](tests/punctuation.rs) checks the vocabulary's matching
rules.
