#!/usr/bin/env python3
"""Execute one supplied entry at its recorded placement; control stubs are labelled."""
from pathlib import Path
import argparse, datetime, hashlib, json, shutil, subprocess, time
ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
g = parser.add_mutually_exclusive_group(required=True)
g.add_argument('--control', choices=['iret', 'clobber-gpr', 'wrong-return', 'marker-iret'])
g.add_argument('--payload', type=Path)
g.add_argument('--placement', type=Path)
parser.add_argument('--oracle', choices=['memory', 'debug-marker', 'gdtr-call', 'task-register-call', 'segment-selector-call'])
parser.add_argument('--load-address', type=lambda x: int(x, 0), default=2097152)
parser.add_argument('--entry-offset', type=lambda x: int(x, 0), default=0)
parser.add_argument('--corrupt-neighbor', action='store_true', help='damage a neighboring byte in an ordinary-call oracle')
parser.add_argument('--label', required=True)
parser.add_argument('--output-root', type=Path, default=ROOT.parents[1] / 'build/native-entry-cpu')
parser.add_argument('--timeout', type=float, default=12)
args = parser.parse_args()
placement = None
if args.placement:
    placement = json.loads(args.placement.read_text())
    args.payload = args.placement.parent / 'payload.bin'
    args.load_address = int(placement['load_address'])
    args.entry_offset = int(placement['entry_offset'])
oracle = args.oracle or ('debug-marker' if args.control == 'marker-iret' else 'memory')
if not args.label.replace('-', '').replace('_', '').isalnum():
    parser.error('label needs simple filename characters')
if args.corrupt_neighbor and oracle not in ['gdtr-call', 'task-register-call', 'segment-selector-call']:
    parser.error('--corrupt-neighbor requires an ordinary-call oracle')
if args.control and oracle in ['gdtr-call', 'task-register-call', 'segment-selector-call']:
    parser.error('ordinary-call oracles require a compiled source payload')
if placement is not None and oracle in ['gdtr-call', 'task-register-call', 'segment-selector-call'] and placement.get('entry_control') != 'CallReturn':
    parser.error('ordinary-call oracle requires the recorded CallReturn entry')
if placement is not None and oracle not in ['gdtr-call', 'task-register-call', 'segment-selector-call'] and placement.get('entry_control', 'InterruptReturn') != 'InterruptReturn':
    parser.error('interrupt oracle requires the recorded interrupt entry')
harness = {'gdtr-call': 'gdtr_call.S', 'task-register-call': 'task_register_call.S', 'segment-selector-call': 'segment_selector_call.S'}.get(oracle, 'harness.S')
run_directory = args.output_root.resolve() / args.label
run_directory.mkdir(parents=True, exist_ok=False)
llds = sorted((Path.home() / '.rustup/toolchains').glob('*/lib/rustlib/*/bin/rust-lld'))
lld = next((x for x in llds if '/stable-' in str(x)), llds[0] if llds else None)
lld = shutil.which('ld.lld') or lld
if lld is None:
    raise SystemExit('An existing ld.lld or Rust rust-lld is required')
clang = shutil.which('clang')
qemu = shutil.which('qemu-system-x86_64')
if not clang or not qemu:
    raise SystemExit('clang and qemu-system-x86_64 are required')
receipt = {'kind': 'bringup-control' if args.control else 'supplied-entry', 'control': args.control, 'corrupt_neighbor': args.corrupt_neighbor, 'load_address': hex(args.load_address), 'entry_offset': args.entry_offset, 'oracle': oracle, 'witness_address': '0x300000' if oracle == 'memory' else None, 'commands': [], 'source_hashes': {}, 'tools': {name: {'path': str(path), 'sha256': hashlib.sha256(Path(path).read_bytes()).hexdigest()} for (name, path) in [('clang', clang), ('rust-lld', lld), ('qemu', qemu)]}}

receipt['expected_calls' if oracle in ['gdtr-call', 'task-register-call', 'segment-selector-call'] else 'expected_deliveries'] = 8

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def run_command(args):
    receipt['commands'].append([str(x) for x in args])
    result = subprocess.run(args, cwd=run_directory, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    with (run_directory / 'build.log').open('a') as f:
        f.write(result.stdout)
    if result.returncode:
        raise RuntimeError(f'Command exit{result.returncode}: {args}; see {run_directory}/build.log')
try:
    for name in [harness, 'boot.inc', 'link.ld', 'control.S', 'control.ld', 'run.py']:
        receipt['source_hashes'][name] = digest(ROOT / name)
    if args.control:
        flags = {'iret': [], 'clobber-gpr': ['-DCLOBBER_GPR=1'], 'wrong-return': ['-DWRONG_RETURN=1'], 'marker-iret': ['-DDEBUG_MARKER=1']}[args.control]
        run_command([clang, '-target', 'x86_64-unknown-none-elf', '-c', *flags, ROOT / 'control.S', '-o', run_directory / 'control.o'])
        run_command([lld, '-flavor', 'gnu', '-m', 'elf_x86_64', '--oformat=binary', '-T', ROOT / 'control.ld', run_directory / 'control.o', '-o', run_directory / 'payload.bin'])
    else:
        supplied = args.payload.resolve()
        receipt['supplied_path'] = str(supplied)
        shutil.copyfile(supplied, run_directory / 'payload.bin')
    payload = run_directory / 'payload.bin'
    if args.entry_offset < 0 or args.entry_offset >= payload.stat().st_size:
        raise ValueError('Entry offset outside supplied bytes')
    if args.load_address < 2097152 or args.load_address + payload.stat().st_size >= 33554432:
        raise ValueError('Payload outside reserved test range')
    if oracle == 'memory' and args.load_address <= 3145735 and (args.load_address + payload.stat().st_size > 3145728):
        raise ValueError('Payload overlaps memory witness')
    receipt['payload_sha256'] = digest(payload)
    if placement is not None:
        if receipt['payload_sha256'] != placement['payload_sha256']:
            raise ValueError('Payload differs from its extracted placement receipt')
        receipt['placement_sha256'] = digest(args.placement)
        receipt['elf_sha256'] = placement['elf_sha256']
    run_command([clang, '-target', 'x86_64-unknown-none-elf', '-c', f'-DENTRY_OFFSET={args.entry_offset}', *(['-DORACLE_DEBUG_MARKER=1'] if oracle == 'debug-marker' else []), *(['-DCORRUPT_NEIGHBOR=1'] if args.corrupt_neighbor else []), ROOT / harness, '-o', run_directory / 'harness.o'])
    run_command([lld, '-flavor', 'gnu', '-m', 'elf_x86_64', '--oformat=binary', '--defsym', f'PAYLOAD_ADDRESS={args.load_address}', '-T', ROOT / 'link.ld', run_directory / 'harness.o', '-o', run_directory / 'kernel.bin'])
    receipt['image_sha256'] = digest(run_directory / 'kernel.bin')
    qemu_command = [qemu, '-machine', 'pc', '-accel', 'tcg', '-cpu', 'qemu64', '-m', '64M', '-display', 'none', '-serial', 'none', '-monitor', 'none', '-no-reboot', '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04', '-debugcon', f'file:{run_directory}/debug.log', '-global', 'isa-debugcon.iobase=0xe9', '-kernel', str(run_directory / 'kernel.bin')]
    receipt['commands'].append(qemu_command)
    start = time.monotonic()
    try:
        result = subprocess.run(qemu_command, cwd=run_directory, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=args.timeout)
        receipt['exit_code'] = result.returncode
        (run_directory / 'qemu.log').write_bytes(result.stdout)
    except subprocess.TimeoutExpired as ex:
        receipt['exit_code'] = None
        receipt['timed_out'] = True
        (run_directory / 'qemu.log').write_bytes(ex.stdout or b'')
    receipt['elapsed_seconds'] = time.monotonic() - start
    receipt['debug_output'] = (run_directory / 'debug.log').read_text(errors='replace') if (run_directory / 'debug.log').exists() else ''
    receipt['passed'] = receipt['exit_code'] == 33 and receipt['debug_output'] == ('B' + 'T.' * 8 + 'P' if oracle == 'debug-marker' else 'B........P')
except Exception as ex:
    receipt['error'] = str(ex)
    receipt['passed'] = False
finally:
    receipt['recorded_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    (run_directory / 'result.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps({k: v for (k, v) in receipt.items() if k not in ['commands', 'source_hashes']}, indent=2))
raise SystemExit(0 if receipt['passed'] else 1)
