---
name: back-and-forth
disable-model-invocation: true
description: Use only when the user explicitly requests back-and-forth for a review between Codex and Fable 5.2 on High in Claude Code.
---

# Back and forth

1. Pick one claim. Tell Fable what you think, show the evidence, and ask for its strongest counterexample and what would change its mind.
2. Check Fable’s strongest point against the code, test, or other evidence. Resume the same Claude session. Show what you found, say where you changed your mind or still disagree, and ask Fable to revise or defend its answer.
3. Keep trading turns while either of you has a new claim you can check. Finish by telling the user what changed, which evidence decided it, and what’s still open. If you agree and neither of you has verified the claim, say so.

Use the requested reviewer and effort throughout the exchange. If the CLI cannot start or resume that reviewer, report the failed step. In PowerShell, put each message and its evidence in `$opening` or `$reply`. Keep `$session` for every follow-up.

```powershell
$turn = $opening | claude -p --model claude-fable-5-2 --effort high --output-format json | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or $turn.is_error -or -not $turn.session_id) { throw "Claude review did not start. $($turn.result)" }
$session = $turn.session_id
$turn.result

$turn = $reply | claude -p --resume $session --model claude-fable-5-2 --effort high --output-format json | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or $turn.is_error -or $turn.session_id -ne $session) { throw "Claude review did not resume. $($turn.result)" }
$turn.result
```
