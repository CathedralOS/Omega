import hashlib, json
from pathlib import Path
from reference import expected, tiny_cases
root = Path(__file__).resolve().parents[1]
cases = {
    "nontrivial": [4, 3, 5, 3, 4, 5, 8, 6, 10, 9, 12, 13, 14, 9, 16],
    "negative": [2, 2, 2, 3, 1, 2, -4, 1, 6, 2, -8, 3, 5, 7, -9],
    "greedy_trap": [1, 1, 0, 1, 1, 0, 1, 2, 0, 2, 9, 0, 0, 0, 0],
    "ties": [2, 1, 2, 1, 2, 2] + [0]*9,
    "zero": [0]*15,
    "bounds": [9, 9, 9, 9, 9, 9, -99, 99, 99, 99, -99, 99, 99, 99, -99],
    "infeasible": [2, 1, 0, 2, 2, 0] + [1]*9,
    "surplus": [2, 2, 1, 1, 1, 1] + [1]*9,
}
records = []
for name, values in cases.items():
    code, output = expected(values)
    content = " ".join(map(str, values)) + "\n"
    (root/'data'/f'{name}.txt').write_text(content)
    (root/'data'/f'{name}.expected.txt').write_text(output)
    records.append(dict(name=name, values=values, exit=code, stdout=output))
for index, values in enumerate(tiny_cases()):
    code, output = expected(values)
    records.append(dict(name=f'tiny-{index}', values=values, exit=code, stdout=output))
invalid = {
    "empty": "", "short": "1 2 3\n", "extra": "0 "*16+"\n",
    "negative_supply": "-1 "+"0 "*14+"\n",
    "large_margin": "10 "+"0 "*14+"\n",
    "large_cost": "0 "*6+"100 "+"0 "*8+"\n",
    "bare_sign": "- "+"0 "*14+"\n",
    "junk": "x "+"0 "*14+"\n",
    "huge": "999999999999999999 "+"0 "*14+"\n",
    "full": " "*256+"0 "*15+"\n",
}
for name, content in invalid.items():
    (root/'data'/f'invalid_{name}.txt').write_text(content)
manifest = dict(tolerance=0, reference_sha256=hashlib.sha256((root/'validation/reference.py').read_bytes()).hexdigest(),
                cases=records, invalid=invalid,
                invalid_stdout="INVALID: expected 15 integers; margins 0..9, costs -99..99; one line below 256 bytes\n")
(root/'validation/oracles.json').write_text(json.dumps(manifest, indent=2)+"\n")
print(f"Frozen {len(records)} exact output oracles, {len(invalid)} invalid runtime inputs")
print(hashlib.sha256((root/'validation/oracles.json').read_bytes()).hexdigest())
