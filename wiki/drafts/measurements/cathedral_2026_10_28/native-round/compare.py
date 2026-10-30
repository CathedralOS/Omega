import hashlib, json, os, re, statistics, sys
from pathlib import Path
sys.path.insert(0, str(Path('tools').resolve()))
from cache_gate import Gate
from compiler_profile import require_optimized
from benchmark.benchmark import capture_compile
root = Path('build/ui-native-round').resolve()
output = root / 'graph-comparison'
output.mkdir()
project = root / 'omega-ui/tests/native_component/exports'
compilers = {'full': root / 'omega-detailed.exe', 'scoped': root / 'omega-scoped.exe'}
caches = {'full': root / 'cache-detailed', 'scoped': root / 'cache-scoped'}
record = dict(status='failed', compilers={name: require_optimized(path) for name, path in compilers.items()}, runs=[])
record['inputs'] = {str(path.relative_to(root / 'omega-ui')): hashlib.sha256(path.read_bytes()).hexdigest() for path in (root / 'omega-ui').rglob('*.omg') if 'build' not in path.relative_to(root / 'omega-ui').parts}
expected = json.loads((root / 'instrumented/result.json').read_text())['runs'][1]['artifacts']
locks = {}
try:
 for name, compiler in compilers.items():
  code, log = Gate(compiler, output, 'linux_x86_64').prepare(project, caches[name], name)
  assert code == 0, log[-4000:]
  locks[name] = (project / 'omega.lock').read_bytes()
 for index, name in enumerate(['full','scoped','full','scoped','scoped','full','full','scoped','scoped','full']):
  (project / 'omega.lock').write_bytes(locks[name])
  os.environ['LOCALAPPDATA'] = str(caches[name])
  os.environ['OMEGA_PHASE_PROFILE'] = '1'
  destination = output / str(index)
  command = [str(compilers[name]), '--offline', '--timings', '--target', 'linux_x86_64', '--build-dir', str(destination), str(project / 'main.omg')]
  code, wall, peak, log = capture_compile(command, str(project))
  (output / f'{index}.log').write_text(log, encoding='utf-8')
  row = dict(name=name, warmup=index<2, wall_ms=wall, peak_bytes=peak, exit_code=code, command=command)
  record['runs'].append(row)
  assert code == 0, log[-5000:]
  artifacts = {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in destination.iterdir() if path.suffix in {'.so','.component'} or path.name in {'omega-program','omega-components.tsv'}}
  assert artifacts == expected, ('native artifacts changed', artifacts, expected)
  row['artifacts'] = artifacts
  row['phases'] = {name: float(value) for value, name in re.findall(r'^\s*([\d.]+) ms\s+\d+ x\s+(\S+)\s*$', log, re.MULTILINE)}
  if index >= 2:
   assert 'source.graph.loaded' in log and 'component.cache.hit' in log
  print(name, index, round(wall,3), 'graph', row['phases'].get('05.payload.graph'), flush=True)
 for name in compilers:
  assert hashlib.sha256(compilers[name].read_bytes()).hexdigest() == record['compilers'][name]['sha256']
  values = [row['wall_ms'] for row in record['runs'] if row['name']==name and not row['warmup']]
  record[name] = dict(min=min(values), max=max(values), mean=statistics.mean(values), median=statistics.median(values))
 assert all(hashlib.sha256((root / 'omega-ui' / name).read_bytes()).hexdigest()==value for name,value in record['inputs'].items())
 record['status']='passed'
finally:
 (output / 'result.json').write_text(json.dumps(record, indent=2), encoding='utf-8')
