"""Freeze supplementary review controls before Omega runtime execution."""
from pathlib import Path
import hashlib, json
from reference import simulate
root = Path(__file__).resolve().parents[1]
original = json.loads((root / 'validation/oracle.json').read_text())
# Correcting oracle segmentation/precedence must preserve all frozen outcomes.
for name, oracle in original['cases'].items():
    text = (root / 'data' / f'{name}.stdin').read_text()
    expected, status = simulate(text)
    assert expected.encode() == (root / 'data' / f'{name}.stdout').read_bytes()
    assert status == oracle['exit']
controls = {}
for label, prefix in [('cr', '\r'), ('vt', '\v'), ('ff', '\f'),
                      ('nel', '\u0085'), ('unicode_separator', '\u2028'), ('nul', '\0')]:
    controls[f'review_lf_only_{label}'] = f'R0:A\n{prefix}V0:0:A:C:0\nV0:0:A:G:1\n'
controls['review_overlong_513th'] = 'R0:A\n' + 'V0:0:A:A:0\n' * 511 + 'X' * 65 + '\n'
controls['review_nul_reference'] = 'R0:A\0\nV0:0:A:C:0\n'
manifest = {'model': original['model'], 'numeric_tolerance': 0,
            'comparison': original['comparison'], 'review_findings': ['LF-only segmentation', 'line-before-record capacity'],
            'original_oracle_sha256': hashlib.sha256((root / 'validation/oracle.json').read_bytes()).hexdigest(),
            'cases': {}}
for name, text in controls.items():
    expected, status = simulate(text)
    (root / 'data' / f'{name}.stdin').write_text(text)
    (root / 'data' / f'{name}.stdout').write_text(expected)
    manifest['cases'][name] = {'exit': status,
        'input_sha256': hashlib.sha256(text.encode()).hexdigest(),
        'stdout_sha256': hashlib.sha256(expected.encode()).hexdigest()}
(root / 'validation/review_oracle.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(f'Original 41 outcomes unchanged; froze {len(controls)} independent review controls')
