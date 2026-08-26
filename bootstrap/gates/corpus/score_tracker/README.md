# Score Tracker

A two-player score tracker driven by a stream of case-payload events.
Exercises two-field case payloads (`Score(player, points)`), fixed-array
indexed writes (`self.scores[0]`), `[copy]` aggregate state, and
case dispatch routing to scalar-argument substates. Runs to exit **70**.

```
omega --target windows_x64 --build-dir build samples/score_tracker/main.omg
./build/omega-program.exe   # exit 70
```

Event sequence:
```
Score(0, 30)  -> scores[0] = 30
Score(1, 20)  -> scores[1] = 20
Bonus(0)      -> scores[0] = 60   (doubled)
Score(1, 10)  -> scores[1] = 30
Reset(1)      -> scores[1] = 0
Score(1, 10)  -> scores[1] = 10
```

Final: `scores[0]=60 + scores[1]=10 = total=70`.

Features exercised:
- `data Event { case Score(player: i32, points: i32); case Bonus(...); case Reset(...); }`
- `transition event { Event::Score { player, points } -> ... }`
- `self.scores[0] = self.scores[0] + points` (fixed-array indexed read-modify-write)
- `data Tracker [copy]` as an accumulator field
