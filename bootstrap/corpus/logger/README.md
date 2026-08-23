# logger

Structured logger with case-payload severity levels and weighted score verification.

## What it demonstrates

- `case` members with and without payloads: `Debug`, `Info`, `Warn`, `Error(code: i32)`
- Case dispatch in a sub-machine (`transition level { LogLevel::Debug -> ... }`)
- Payload binding: `LogLevel::Error { code } -> on_error(code)`
- Per-level count accumulation in `[copy]` fields
- Multi-step `let`-bound arithmetic over fields after sub-machine mutations
- Guard ladder verifying both a computed score and individual counts

## Scripted log sequence

| Entry            | info_count | warn_count | error_code_sum |
|------------------|-----------|-----------|---------------|
| Info ×5          | 5         | 0         | 0             |
| Warn ×4          | 5         | 4         | 0             |
| Error { code: 4} | 5         | 4         | 4             |

Weighted score: `info_count*2 + warn_count*5 + error_code_sum*10 = 10+20+40 = 70`.

## Expected exit code

**70** — score==70, info_count==5, warn_count==4, error_count==1 all verify.

## Building

```
cargo run -p omega-cli -- --build-dir samples/logger/build --target windows_x64 samples/logger/main.omg
./samples/logger/build/omega-program.exe
echo $?   # 70
```
