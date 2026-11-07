"""Author expected fixtures BEFORE compiler execution; never bless observed output."""
from pathlib import Path
import hashlib, json
from reference import simulate, words
root = Path(__file__).resolve().parents[1]
data = root / 'data'
cases = {
    'repeats': 'R0:AAAA\nR1:ACACAC\nV0:3:A:-:0\nV0:1:A:-:1\nV0:4:-:A:2\nV0:2:-:A:3\nV0:0:A:C:4\nV0:0:A:G:5\nV1:4:AC:-:6\nV1:0:AC:-:7\nV1:6:-:AC:0\nV1:2:-:AC:1\n',
    'boundaries': 'R0:ACGT\nR1:AAAA\nV0:0:-:T:0\nV0:4:-:G:1\nV0:3:T:-:2\nV0:0:AC:TG:3\nV0:1:C:CT:4\nV1:3:A:-:5\n',
    'invalid_runtime': 'R0:ACGT\nR0:AAAA\nV1:0:A:C:0\nV0:8:-:A:0\nV0:3:TT:-:0\nV0:1:A:T:0\nV0:1:C:C:0\nV0:1:CCCCC:A:0\nV0:1:N:A:0\nV2:1:C:A:0\nV0:1:C:A:8\nV0:1:C:A:0junk\nR1:AAAA\nV0:1:C:A:0\n',
    'final_eof': 'R0:AC\nV0:2:-:T:0',
    'line_capacity': 'R0:AAAA\n' + 'X' * 65 + '\n',
    'record_capacity': 'R0:A\n' + 'V0:0:A:A:0\n' * 513,
    'capacity': 'R0:ACGT\n' + ''.join(f'V0:0:-:{word}:0\n' for word in words('ACGT', 3)[1:]),
}
# Exhaust every valid input position, deletion <=2 and inserted AC word <=2
# for all nonempty AC references of length <=3. Chunk to avoid ledger capacity.
enumerated = 0
for reference in words('AC', 3)[1:]:
    records = []
    for position in range(len(reference) + 1):
        for width in range(min(2, len(reference) - position) + 1):
            for inserted in words('AC', 2):
                deleted = reference[position:position + width]
                records.append(f'V0:{position}:{deleted or "-"}:{inserted or "-"}:{enumerated % 8}\n')
                enumerated += 1
    for chunk in range(0, len(records), 24):
        cases[f'enumeration_{reference}_{chunk // 24:02}'] = f'R0:{reference}\n' + ''.join(records[chunk:chunk+24])
manifest = {'model': 'synthetic-haploid-bounded-edits-v1', 'numeric_tolerance': 0,
            'comparison': 'exact UTF-8 stdout bytes and process exit',
            'enumerated_records': enumerated, 'cases': {}}
for name, text in sorted(cases.items()):
    expected, status = simulate(text)
    (data / f'{name}.stdin').write_text(text)
    (data / f'{name}.stdout').write_text(expected)
    manifest['cases'][name] = {'exit': status,
        'input_sha256': hashlib.sha256(text.encode()).hexdigest(),
        'stdout_sha256': hashlib.sha256(expected.encode()).hexdigest()}
(root / 'validation/oracle.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(f'Frozen {len(cases)} cases, {enumerated} independently enumerated input edits; exact tolerance 0')
