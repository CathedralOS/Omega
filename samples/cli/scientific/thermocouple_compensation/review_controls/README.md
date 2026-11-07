The derivative controls were independently frozen with the unchanged Decimal
reference before repaired Omega execution. The original 23 fixtures and
expected.json remain unchanged. The reported cancellation input must return
ERROR calibration (11); positive and negative minimum nonzero derivatives
(2e-6) must remain accepted.

The separate `continuity/` controls reject exact differences of either sign
at the forbidden 1e-8 boundary and accept the largest permitted difference,
9e-9. Its expectations are separately frozen; the derivative expectations are
unchanged. The validator runs all six controls by default.

Run after native production:

```sh
python3 "$PROJECT/review_controls/validate.py" --route native --program "$PROJECT/build/native/omega-program"
python3 "$PROJECT/review_controls/validate.py" --route both --omega "$OMEGA"
```
