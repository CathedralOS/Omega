---
name: back-and-forth
description: Use when explicitly asked to run a back-and-forth review between Codex and Fable 5.2 on High in Claude Code.
---

# Back and forth

1. Pick one claim. Tell Fable what you think, show the evidence, and ask for its strongest counterexample and what would change its mind.
2. Check Fable’s strongest point against the code, test, or other evidence. Resume the same Claude session. Show what you found, say where you changed your mind or still disagree, and ask Fable to revise or defend its answer.
3. Keep trading turns while either of you has a new claim you can check. Finish by telling the user what changed, which evidence decided it, and what’s still open. If you agree and neither of you has verified the claim, say so.

In PowerShell, put each message and its evidence in `$opening` or `$reply`. Keep `$session` for every follow-up.

```powershell
$turn = $opening | claude -p --model claude-fable-5-2 --effort high --output-format json | ConvertFrom-Json
$session = $turn.session_id
$turn.result

$turn = $reply | claude -p --resume $session --model claude-fable-5-2 --effort high --output-format json | ConvertFrom-Json
$turn.result
```
