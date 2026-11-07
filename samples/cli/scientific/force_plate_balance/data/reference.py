"""Independent frozen oracle: rational dot products, no Omega execution."""
from fractions import Fraction
from pathlib import Path
import re
ROOT = Path(__file__).resolve().parent
POSITIONS = [(-200,-100),(200,-100),(200,100),(-200,100)]
HEADER = "force_plate_balance positions_mm=(-200,-100),(200,-100),(200,100),(-200,100) support_mm=150,75"
SYNTAX = "INVALID_SYNTAX expected seven signed integers <=1000000"
def reference(text):
    rows = [HEADER]
    for line in text.splitlines():
        if len(line.encode()) + 1 > 256:
            rows.append("INVALID_LINE_TOO_LONG"); continue
        words = line.split()
        if len(words) != 7 or any(not re.fullmatch(r"-?[0-9]+", word) or abs(int(word)) > 1000000 for word in words):
            rows.append(SYNTAX); continue
        values = list(map(int, words)); loads = values[:4]
        force = sum(loads)
        moments = [sum(load * position[axis] for load, position in zip(loads, POSITIONS)) for axis in range(2)]
        residuals = [force-values[4], moments[0]-values[5], moments[1]-values[6]]
        if min(loads) < 0: status = "INVALID_NEGATIVE_LOAD"
        elif max(loads) > 1000: status = "INVALID_SATURATED_CELL"
        elif force == 0: status = "INVALID_ZERO_FORCE"
        elif any(residuals): status = "INVALID_EQUILIBRIUM"
        elif abs(Fraction(moments[0],force)) > 150 or abs(Fraction(moments[1],force)) > 75: status = "OUTSIDE_SUPPORT"
        else: status = "SUPPORTED"
        row = "loads=" + ",".join(map(str,loads)) + " force=" + str(force)
        row += " moments=" + ",".join(map(str,moments)) + " residuals=" + ",".join(map(str,residuals))
        if force > 0 and status not in ("INVALID_NEGATIVE_LOAD","INVALID_SATURATED_CELL"):
            row += " cop_mm=" + ",".join(str(moment)+"/"+str(force) for moment in moments)
        rows.append(row+" "+status)
    return "\n".join(rows)+"\n"
if __name__ == "__main__":
    expected = reference((ROOT/"frames.txt").read_text())
    assert expected == (ROOT/"expected_stdout.txt").read_text(), "frozen oracle differs"
    print("rational reference agrees with frozen stdout; exact tolerance = 0")
