#!/usr/bin/env python3
"""Check service identity and call ceilings against actual bundled core declarations."""
import argparse
from pathlib import Path
import runpy
import signal
import struct
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tests/bootstrap'))
from compiled import add_invocation_budget, execute, pack, stamp
from conformance.vectors import epsilon_request
from independent.byte_input_cases import byte_input_cases


def cases():
    encode = runpy.run_path(str(ROOT / 'source/application/construction/construct.py'))['encode']
    core = ROOT / 'source/library/core'
    app_import = b'use {dep}/read.omg;\n'
    core_import = b'use {omega}/language/core/process_authority.omg;\n'
    helper = core_import + b'public machine read_value()->u8 reaches |process_authority|::StandardOutput {42}'
    packages = [('app', 0), ('provider_free_library', 0), ('omega_language_core', 1)]
    sites = [(0, 1, 'dep')]

    def trial(name, app, expected=None, *, helper_source=helper, edges=sites, roles=packages):
        rows = [(0, app), (1, helper_source), (2, (core / 'process_authority.omg').read_bytes())]
        paths = ['main.omg', 'read.omg', 'process_authority.omg']
        packet, _, _, units = encode(roles, edges, rows, 0, b'main', paths=paths)
        if expected is None:
            return name, packet, None
        unit, token = expected
        coordinate = units[unit][1] + rows[unit][1].rindex(token)
        return name, packet, b'\0' + struct.pack('<II', 1, coordinate)

    relay = b'machine relay()->u8 {|read|::read_value()}\n'
    main = b'machine main()->u8 reaches |process_authority|::StandardOutput + |process_authority|::StandardOutput {relay()}'
    app = app_import + core_import + relay + main
    yield trial('same-service', app)
    yield trial('inferred', app.replace(b' reaches |process_authority|::StandardOutput + |process_authority|::StandardOutput', b''))
    yield trial('separate-ceilings', app.replace(b'|process_authority|::StandardOutput + |process_authority|::StandardOutput',
                b'|process_authority|::StandardInput reaches |process_authority|::StandardOutput'))
    yield trial('pure', app.replace(b'|process_authority|::StandardOutput + |process_authority|::StandardOutput', b'none'),
                helper_source=helper.replace(b'reaches |process_authority|::StandardOutput', b'reaches none'))
    yield trial('empty', app.replace(b'|process_authority|::StandardOutput + |process_authority|::StandardOutput', b'none'), (0, b'relay()'))
    yield trial('wrong-service', app.replace(b'|process_authority|::StandardOutput + |process_authority|::StandardOutput', b'|process_authority|::StandardInput'), (0, b'relay()'))
    yield trial('same-spelling-other-owner', app.replace(core_import, b'boundary trait StandardOutput {}\n').replace(b'|process_authority|::StandardOutput', b'StandardOutput'),
                (0, b'relay()'))
    yield trial('absent-edge', app, (0, b'use {dep}'), edges=[])
    yield trial('untrusted-core', app, (0, b'use {omega}'), roles=[*packages[:2], ('omega_language_core', 0)])
    yield trial('ordinary-name', app.replace(b'|process_authority|::StandardOutput + |process_authority|::StandardOutput', b'read_value'),
                (0, b'read_value'))
    yield trial('mixed-empty', app.replace(b'|process_authority|::StandardOutput + |process_authority|::StandardOutput', b'none + |process_authority|::StandardOutput'),
                (0, b'none'))
    yield trial('duplicate-declaration', app.replace(relay, b'boundary trait A {} boundary trait A {} ' + relay),
                (0, b'A {}'))
    yield trial('reserved-type', app.replace(relay, b'boundary trait u97 {} ' + relay), (0, b'u97'))
    yield trial('retired-marker', app.replace(relay, b'boundary trait Block {} ' + relay)
                .replace(b'|process_authority|::StandardOutput + |process_authority|::StandardOutput', b'Block'), (0, b'Block'))
    yield trial('private-import', app, (0, b'|read|::read_value'), helper_source=helper.replace(b'public machine', b'machine'))

    # A compiler role and an explicit reach ceiling cannot install ProcessExit.
    terminal = (b'use {omega}/language/core/process_exit.omg; machine main()->u8 reaches |process_exit|::ProcessExit '
                b'{ |process_exit|::ProcessExit::exit_process(253); }')
    declaration = (core / 'process_exit.omg').read_bytes()
    units = [(0, terminal), (1, declaration), (1, (core / 'never.omg').read_bytes())]
    packet, _, _, rows = encode([('app', 0), ('omega_language_core', 1)], [], units, 0, b'main',
                                paths=['main.omg', 'process_exit.omg', 'never.omg'])
    coordinate = rows[1][1] + declaration.index(b'boundary trait ProcessExit')
    yield 'uninstalled-exit', packet, b'\0' + struct.pack('<II', 2, coordinate)



def terminal_cases():
    encode = runpy.run_path(str(ROOT / 'source/application/construction/construct.py'))['encode']
    core = ROOT / 'source/library/core'
    terminal = (core / 'process_exit.omg').read_bytes()
    bottom = (core / 'never.omg').read_bytes()
    names = [('app', 0), ('omega_language_core', 1)]

    def trial(name, app, outcome, *, service=terminal, result=bottom, standalone=False):
        units = [(0, app)] if standalone else [(0, app), (1, service), (1, result)]
        paths = ['main.omg'] if standalone else ['main.omg', 'process_exit.omg', 'never.omg']
        packet, _, _, rows = encode(names[:1] if standalone else names, [], units, 0, b'main', paths=paths)
        if isinstance(outcome, tuple):
            code, unit, spelling = outcome
            source_coordinate = rows[unit][1] + units[unit][1].rindex(spelling)
            outcome = b'\0' + struct.pack('<II', code, source_coordinate)
        return name, packet, outcome

    import_exit = b'use {omega}/language/core/process_exit.omg;\n'
    import_never = b'use {omega}/language/core/never.omg;\n'
    marker = b'machine main()->u8 reaches |process_exit|::ProcessExit {42}'
    # Empty-brace arms complete a Unit result. Calling both choices makes
    # an accidental write on the early-return path observable.
    empty_arm = (b'data Cell [copy] {n:u8;} '
                 b'machine choose(c:@mutable Cell,skip:bool){transition skip {true->{},_->put(c)} '
                 b'machine put(c:@mutable Cell){c.n=42;}} '
                 b'machine main()->u8 {let mutable early:Cell;early.n=7;let mutable late:Cell;'
                 b'choose(@mutable early,true);choose(@mutable late,false);'
                 b'transition early.n==7 {true->late.n,_->1}}')
    yield trial('empty-arm-choice', empty_arm, 42, standalone=True)
    yield trial('empty-arm-entry', b'machine main(){transition {_-> { /* explicit Unit */ }}}',
                0, standalone=True)
    yield trial('empty-arm-value', b'machine f(flag:bool)->u8 {transition flag {true->42,_-> {}}} '
                b'machine main()->u8{42}', (1, 0, b'{}'), standalone=True)
    yield trial('nonempty-terminal-braces', b'machine main(){transition {_-> {42}}}',
                (1, 0, b'42'), standalone=True)
    yield trial('incomplete-terminal-braces', b'machine main(){transition {_-> {',
                (1, 0, b''), standalone=True)
    yield trial('missing-arm-target', b'machine main(){transition {_-> }}',
                (1, 0, b'}}'), standalone=True)
    yield trial('terminal-declaration', import_exit + marker, None)
    yield trial('inhabited-lookalike-result', import_exit + marker,
                (2, 1, b'boundary trait ProcessExit'), result=b'public data Never {}')
    yield trial('wrong-requirement-parameter', import_exit + marker,
                (2, 1, b'boundary trait ProcessExit'), service=terminal.replace(b'return_code: i32', b'return_code: u32'))
    yield trial('missing-requirement-reach', import_exit + marker, (2, 1, b'boundary trait ProcessExit'),
                service=terminal.replace(b'reaches\n        ProcessExit', b'reaches none'))
    stop = b'machine stop()->|never|::Never crashes Trap {crash Trap;}\n'
    relay = b'machine relay()->|never|::Never crashes Trap {stop();}\n'
    dispatch = (b'machine choose(stop_now:bool)->u8 crashes Trap {'
                b'transition stop_now {true -> stopped(), _ -> ordinary()} '
                b'machine stopped()->u8 crashes Trap {relay();} machine ordinary()->u8{42}}\n')
    body = import_never + stop + relay + dispatch
    yield trial('uninhabited-untaken', body + b'machine main()->u8 crashes Trap {choose(false)}', None)
    yield trial('uninhabited-taken', body + b'machine main()->u8 crashes Trap {choose(true)}', 'trap')
    yield trial('uninhabited-value-context', import_never + stop + b'machine main()->u8 crashes Trap {stop()}', 'trap')
    yield trial('uninhabited-unrelated-name', b'data Void requires false {}\nmachine fail()->Void crashes Trap '
                b'{crash Trap;} machine main()->u8 crashes Trap {fail();}', 'trap')
    yield trial('zero-uninhabited', import_never + b'machine main()->u8 {let mutable value:|never|::Never;42}', (1, 0, b'let'))
    yield trial('construct-uninhabited', import_never + b'machine wrong()->|never|::Never {|never|::Never {}} machine main()->u8{42}',
                (1, 0, b'|never|::Never {}'))
    yield trial('normal-result-lookalike', b'data Never [copy] {} machine fake()->Never {Never {}} '
                b'machine main()->u8 {fake()}', (1, 0, b'fake()'))


    call = import_exit + b'machine main()->u8 reaches |process_exit|::ProcessExit {|process_exit|::ProcessExit::exit_process('
    yield trial('terminal-argument-type', call + b'true);}', (1, 0, b'true'))
    yield trial('terminal-argument-carrier', call + b'255u32);}', (1, 0, b'255u32'))
    yield trial('terminal-missing-argument', call + b');}', (1, 0, b'|process_exit|::ProcessExit::exit_process'))
    yield trial('terminal-extra-argument', call + b'0, 1);}', (1, 0, b'|process_exit|::ProcessExit::exit_process'))
    yield trial('terminal-valid-negative', call + b'-2147483648i32);}', (2, 1, b'boundary trait ProcessExit'))
    yield trial('terminal-expression-type', call + b'false)}', (1, 0, b'false'))

    yield trial('pure-unit-entry', b'machine main(){}', 0, standalone=True)
    yield trial('unit-entry-calls-helper', b'machine helper(){} machine main(){helper();}', 0)
    yield trial('unit-entry-trap', b'machine main() crashes Trap {crash Trap;}', 'trap')
    yield trial('unit-entry-uninstalled', import_exit + b'machine main() reaches |process_exit|::ProcessExit {|process_exit|::ProcessExit::exit_process(16);}',
                (2, 1, b'boundary trait ProcessExit'))
    yield trial('entry-requires-parameter', b'machine main(value:u8){}', (2, 0, b'machine main'))
    yield trial('entry-unsupported-result', b'machine main()->i32{42}', (2, 0, b'machine main'))


def provider_cases():
    encode = runpy.run_path(str(ROOT / 'source/application/construction/construct.py'))['encode']
    core = ROOT / 'source/library/core'
    service = (core / 'process_exit.omg').read_bytes()
    bottom = (core / 'never.omg').read_bytes()
    primitive = (core / 'process_exit_native.omg').read_bytes()
    provider = (ROOT / 'source/library/system/process_exit.omg').read_bytes()
    packages = [('app', 0), ('omega_language_core', 1), ('system', 2)]
    paths = ['main.omg', 'process_exit.omg', 'never.omg', 'process_exit_native.omg', 'process_exit.omg']
    marker = b'use {omega}/language/core/process_exit.omg; machine main()->u8 reaches |process_exit|::ProcessExit {42}'

    def trial(name, app=marker, outcome=None, *, boundary=primitive, wrapper=provider, result=bottom):
        units = [(0, app), (1, service), (1, result), (1, boundary), (2, wrapper)]
        packet, _, _, rows = encode(packages, [(0, 2, 'stdlib')], units, 0, b'main', paths=paths)
        if outcome:
            code, unit, spelling = outcome
            coordinate = rows[unit][1] + units[unit][1].rindex(spelling)
            outcome = b'\0' + struct.pack('<II', code, coordinate)
        return name, packet, outcome

    yield trial('unselected-provider')
    yield trial('forwarded-name', wrapper=provider.replace(b'return_code', b'status'))
    body = b'{|process_exit|::ProcessExit::exit_process(253);}'
    yield trial('provider-does-not-install', marker.replace(b'{42}', body), (2, 1, b'boundary trait ProcessExit'))
    direct = b'use {omega}/language/core/process_exit_native.omg; machine main()->u8 {|process_exit_native|::ProcessExitPrimitive(253);}'
    yield trial('primitive-does-not-install', direct, (2, 3, b'public boundary machine'))
    selected = b'use {stdlib}/process_exit.omg; machine main()->u8 {|process_exit|::NativeProcessExit::exit_process(253);}'
    yield trial('implementation-does-not-install', selected, (2, 4, b'NativeProcessExit satisfies'))
    yield trial('member-is-owned', b'machine main()->u8 {exit_process(253);}', (1, 0, b'exit_process'))
    yield trial('wrong-forwarded-value', outcome=(2, 4, b'exit_process(return_code'),
                wrapper=provider.replace(b'ProcessExitPrimitive(return_code)', b'ProcessExitPrimitive(7)'))
    yield trial('shadowed-forwarding-call', outcome=(2, 4, b'exit_process(return_code'),
                wrapper=provider.replace(b'public NativeProcessExit satisfies', b'machine ProcessExitPrimitive(x:i32)->i32{x}\npublic NativeProcessExit satisfies')
                .replace(b'|exit_primitive|::ProcessExitPrimitive(return_code)', b'ProcessExitPrimitive(return_code)'))
    yield trial('wrong-forwarded-place', outcome=(2, 4, b'exit_process(return_code'),
                wrapper=provider.replace(b'ProcessExitPrimitive(return_code)', b'ProcessExitPrimitive(other)'))
    yield trial('wrong-primitive-requirement', outcome=(1, 3, b'ProcessExitPrimitive('),
                boundary=primitive.replace(b'satisfies |process_exit|::ProcessExit::exit_process', b'satisfies |process_exit|::ProcessExit::other'))
    yield trial('wrong-provider-result', outcome=(1, 4, b'exit_process(return_code'),
                wrapper=provider.replace(b') -> |never|::Never', b') -> u8'))
    yield trial('wrong-provider-owner', outcome=(2, 4, b'NativeProcessExit satisfies'),
                wrapper=provider.replace(b'public NativeProcessExit satisfies |process_exit|::ProcessExit',
                                         b'boundary trait Other {} public NativeProcessExit satisfies Other'))
    # An ordinary local machine with the primitive's spelling has no physical identity.
    yield trial('ordinary-primitive-spelling',
                b'machine ProcessExitPrimitive(x:u8)->u8{x} machine main()->u8{ProcessExitPrimitive(42)}')


    relay = b'machine status(code:i32)->i32 {code}\n'
    compare = (relay + b'machine main()->u8 {let lo:i32=status(-2147483648i32); '
               b'let hi:i32=status(2147483647i32); let neg:i32=status(-1); '
               b'let zero:u32=0; transition lo < neg && neg < zero && zero < hi && '
               b'status(-0) == 0 && hi < 4294967295u32 && -1i32 != 255u8 '
               b'{true -> good(), _ -> bad()} machine good()->u8{42} machine bad()->u8{9}}')
    yield trial('signed-exit-status', compare)
    for name, value, spelling in [
        ('positive-status-overflow', b'2147483648i32', b'2147483648i32'),
        ('negative-status-overflow', b'-2147483649i32', b'-2147483649i32'),
        ('unsigned-status-suffix', b'-1u32', b'1u32'),
    ]:
        yield trial(name, relay + b'machine main()->u8{let bad:i32=status(' + value + b');42}',
                    (1, 0, spelling))

    # The construction writer passes its output byte to Console::write_byte as i32.
    yield trial('widen-output-byte', b'machine status(byte:u8)->i32 {byte as i32} '
                b'machine id(code:i32)->i32{code as i32} machine main()->u8 {'
                b'transition id(status(255)) == 255i32 && id(-1) < status(0) '
                b'{true -> yes(), _ -> no()} machine yes()->u8{42} machine no()->u8{9}}')
    yield trial('unsigned-to-signed-unproved', b'machine status(x:u32)->i32{x as i32} machine main()->u8{42}',
                (2, 0, b'x as i32'))


def byte_output_cases():
    encode = runpy.run_path(str(ROOT / 'source/application/construction/construct.py'))['encode']
    core = (ROOT / 'source/library/core/numeric_conversion.omg').read_bytes()
    signature = b'public machine narrow_i32_to_u8_wrapping('
    start = core.index(signature)
    function = core[start:core.index(b'\n}', start) + 2]

    def trial(name, source, outcome):
        packet, _, _, rows = encode([('app', 0)], [], [(0, source)], 0, b'main', paths=['main.omg'])
        if isinstance(outcome, tuple):
            code, spelling = outcome
            outcome = b'\0' + struct.pack('<II', code, rows[0][1] + source.index(spelling))
        return name, packet, outcome

    # The standalone driver supplies the actual core routine, then runs each
    # result on Alpha. Byte conversion itself has no Console or process reach.
    values = [-2147483648, -2147483647, -257, -256, -255, -1, 0, 1,
              127, 128, 255, 256, 257, 65535, 2147483647]
    comparisons = ' && '.join(f'narrow_i32_to_u8_wrapping({n}) == {n % 256}' for n in values)
    yield trial('core-write-byte-conversion', function + b'\nmachine main()->u8 {transition ' +
                comparisons.encode() + b' {true -> yes(), _ -> no()} '
                b'machine yes()->u8{42} machine no()->u8{9}}', 42)
    yield trial('radix-mask', b'machine f(x:i32)->u8{(0B1111_1111 & x) as u8} machine main()->u8 {'
                b'let wide:u64=0xFFFF_FFFF_FFFF_FFFFu64; transition f(-0O401) == 255 && '
                b'wide > 0xFFFF_FFFFu32 && 0o52u8 == 0b10_1010u8 '
                b'{true -> yes(), _ -> no()} machine yes()->u8{42} machine no()->u8{9}}', 42)
    yield trial('full-positive-mask', b'machine f(x:i32)->i32{x & 0x7FFF_FFFF} '
                b'machine main()->u8 {transition f(-1)==2147483647i32 && f(-2147483648)==0 '
                b'{true -> yes(), _ -> no()} machine yes()->u8{42} machine no()->u8{9}}', 42)
    yield trial('zero-mask-still-evaluates', b'machine fail()->i32 crashes Trap {crash Trap;} '
                b'machine main()->u8 crashes Trap {(0i32 & fail()) as u8}', 'trap')
    yield trial('unproved-byte-cast', b'machine f(x:i32)->u8{x as u8} machine main()->u8{f(7)}', (2, b'x as'))
    yield trial('nine-bit-mask', b'machine f(x:i32)->u8{(x & 0x1FF) as u8} machine main()->u8{f(7)}', (2, b'(x &'))
    yield trial('noncontiguous-mask', b'machine f(x:i32)->u8{(x & 0xAA) as u8} machine main()->u8{f(7)}', (2, b'&'))
    yield trial('overflow-hex', b'machine main()->u8{0x100u8}', (1, b'0x100'))
    yield trial('overflow-wide-hex', b'machine main()->u8{let x:u64=0x1_0000_0000_0000_0000u64;42}', (1, b'0x1'))
    yield trial('hex-carrier', b'machine main()->u8{0x2Au32}', (1, b'0x2A'))

def build_scope_cases():
    encode = runpy.run_path(str(ROOT / 'source/application/construction/construct.py'))['encode']
    packages = [('application', 0), ('library', 0), ('toolchain', 1)]
    product = [(0, b'machine main()->u8{42}'), (1, b'public machine value()->u8{9}')]
    paths = ['main.omg', 'nested/value.omg']
    default = (ROOT / 'source/library/core/defaults/build.omg').read_bytes()
    authored = b'machine build(builder:@mutable BuildPlan){builder.product=BuildProduct::Application{name:"application"};}'
    builds = [(2, 'defaults/build.omg', default), (0, 'build.omg', authored), (0, 'm/extra.omg', b'')]
    selected = [(0, 1), (1, 0), (2, 0)]
    wire, _, _, _ = encode(packages, [], product, 0, b'main', paths=paths,
                          build_sources=builds, build_entries=selected)
    buffer_length, = struct.unpack_from('<I', wire, 32)
    arena = wire[-buffer_length:]
    entries = 10 + 3 * len(packages) + 5 * len(product)
    units = entries + 2 * len(packages)
    malformed = b'\0' + struct.pack('<I', 3)

    def word(index, new):
        changed = bytearray(wire)
        struct.pack_into('<I', changed, 4 * index, new)
        return bytes(changed)

    def pending(start):
        return b'\0' + struct.pack('<II', 2, start)

    yield 'scoped-authored-build', wire, pending(arena.index(authored))
    for name, index, value in [
        ('scoped-foreign-requester', entries, 1), ('scoped-dangling-entry', entries + 1, len(builds)),
        ('scoped-foreign-build', units + 5, 1), ('scoped-shadowed-default', entries + 1, 0),
        ('scoped-untrusted-default', 10 + 2 * 3 + 2, 0),
        ('scoped-source-outside', units + 1, buffer_length + 1),
        ('scoped-path-outside', units + 4, buffer_length + 1),
    ]:
        yield name, word(index, value), malformed
    for name, replacement in [('scoped-traversal', b'./extra.omg'), ('scoped-nul', b'm/extra.om\0'),
                              ('scoped-backslash', b'm\\extra.omg')]:
        yield name, wire.replace(b'm/extra.omg', replacement), malformed
    changed = bytearray(wire)
    changed[4 * (units + 13):4 * (units + 15)] = wire[4 * (units + 8):4 * (units + 10)]
    yield 'scoped-duplicate-path', bytes(changed), malformed
    yield 'scoped-truncated', wire[:-1], malformed
    yield 'scoped-trailing', wire + b'\0', malformed
    fallback, _, _, _ = encode(packages, [], product, 0, b'main', paths=paths,
                               build_sources=builds[:1], build_entries=[(requester, 0) for requester in range(3)])
    size, = struct.unpack_from('<I', fallback, 32)
    yield 'scoped-selected-default', fallback, pending(fallback[-size:].index(default))


def rooted_application_cases():
    encode = runpy.run_path(str(ROOT / 'source/application/construction/construct.py'))['encode']
    core = ROOT / 'source/library/core'
    service = (core / 'process_exit.omg').read_bytes()
    wrapper = (ROOT / 'source/library/system/process_exit.omg').read_bytes()
    default = (core / 'defaults/build.omg').read_bytes()
    packages = [('app', 0), ('core', 1)]
    product_core = [(1, service), (1, (core / 'never.omg').read_bytes()),
                    (1, (core / 'process_exit_native.omg').read_bytes())]
    paths = ['main.omg', 'process_exit.omg', 'never.omg', 'process_exit_native.omg']
    bind = b'Build::bind(builder, alpha_bootstrap::ProgramEntry, |main_file|::entry_start);'
    selection = b'Build::select_provider::<|process_exit|::ProcessExit,|main_file|::NativeProcessExit>(builder,alpha_bootstrap);'
    config = (b'use main.omg as main_file;use {omega}/language/core/process_exit.omg; machine build(builder:@mutable BuildPlan){'
              b'builder.product=BuildProduct::Application{name:"app"};' + bind + selection + b'}')
    pure = b'public machine entry_start()->u8 {42}'
    effect = b'public machine entry_start() reaches |process_exit|::ProcessExit {|process_exit|::ProcessExit::exit_process(-257i32);}'

    def trial(name, build=config, entry=pure, outcome='root', *, fallback=default, extra=(), entry_unit=0):
        product = [(0, wrapper + b'\n' + entry), *product_core]
        builds = [(0, 'build.omg', build), (1, 'defaults/build.omg', fallback), *extra]
        wire, _, _, units = encode(packages, [], product, entry_unit, b'entry_start', paths=paths,
                                  build_sources=builds, build_entries=[(0, 0), (1, 1)])
        if outcome == 42:
            return name, wire, 42
        if outcome == 'service':
            position = units[1][1] + service.index(b'boundary trait ProcessExit')
        else:
            length, = struct.unpack_from('<I', wire, 32)
            position = wire[-length:].index(fallback if entry_unit else build)
        return name, wire, b'\0' + struct.pack('<II', 2, position)

    yield trial('build-bound-entry', outcome=42)
    yield trial('build-entry-without-selection', config.replace(selection, b''), outcome=42)
    yield trial('build-selection-grants-no-domain', entry=effect, outcome='service')
    yield trial('build-missing-provider-no-domain', config.replace(selection, b''), effect, 'service')
    yield trial('build-unknown-provider', config.replace(b'NativeProcessExit>', b'NoSuchProvider>'))
    yield trial('build-handle-shadows-file-alias', config.replace(b'builder', b'main_file'))
    yield trial('build-handle-shadows-target', config.replace(b'builder', b'alpha_bootstrap'))
    yield trial('build-other-receiver', config.replace(b'builder.product', b'other.product'))
    yield trial('build-bare-operation', config.replace(b'builder.product', b'product'))
    yield trial('build-wrong-entry-file', entry_unit=1)
    yield trial('build-wrong-role-name', config.replace(b'"app"', b'"other"'))
    yield trial('build-other-target', config.replace(b'alpha_bootstrap::ProgramEntry', b'other::ProgramEntry'))
    yield trial('build-unresolved-entry', config.replace(b'|main_file|::entry_start)', b'|main_file|::absent)'))
    yield trial('build-unresolved-import', config.replace(b'use {omega}/language/core/process_exit.omg;', b''))
    yield trial('build-unfinished', config[:-1] + b'Build::unknown(builder);}')
    # The same local helper and declaration can appear in three deployments.
    # Only execution of its Alpha selection supplies this target's provider.
    helper = b'machine configure(handle: @mutable BuildPlan){' + selection.replace(b'builder', b'handle') + b'}'
    hosted = selection.replace(b'alpha_bootstrap', b'linux_x86_64').replace(b'builder', b'handle')
    called = config.replace(selection, b'configure(builder);') + helper[:-1] + hosted + b'}'
    yield trial('build-called-helper', called, effect, 'service')
    yield trial('build-hosted-helper', called.replace(selection.replace(b'builder', b'handle'), b''), effect, 'service')
    yield trial('build-unreached-helper', called.replace(b'configure(builder);', b''), effect, 'service')
    yield trial('build-pure-through-helper', called, outcome=42)
    yield trial('build-recursive-helper', called[:-1] + b'configure(handle);}')
    yield trial('build-wrong-helper-argument', called.replace(b'configure(builder);', b'configure(other);'))
    yield trial('build-duplicate-target', called[:-1] + hosted + b'}')
    yield trial('build-unknown-target', called.replace(b'linux_x86_64', b'unknown_target'))
    yield trial('build-unresolved-hosted-provider', called.replace(hosted, hosted.replace(b'NativeProcessExit', b'Missing')))
    yield trial('build-ill-typed-unreached-helper', called.replace(b'configure(builder);', b'').replace(
        hosted, hosted.replace(b'NativeProcessExit', b'Missing')))

    yield trial('build-nonempty-default', fallback=b'machine build(builder:@mutable BuildPlan){Build::unknown(builder);}')
    yield trial('build-additional-unit', extra=[(0, 'helper.omg', b'machine helper(){}')])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path, help='new directory for input, results and Alpha tapes')
    parser.add_argument("--case", action="append", help="run the named case (repeatable; default: all)")
    route = parser.add_mutually_exclusive_group()
    route.add_argument('--constructor', type=Path, help='compiled Epsilon construction executable')
    route.add_argument('--evaluator', type=Path, help='source-built Epsilon evaluator')
    add_invocation_budget(parser)
    args = parser.parse_args()
    work = args.output.resolve()
    work.mkdir(parents=True)
    if args.evaluator:
        evaluator = pack('bootstrap/epsilon/interpreter/epsilon_compiler.delta.sources', work / 'evaluator.delta')
        compiler = pack('bootstrap/omega/compiler/omega_compiler.epsilon.sources', work / 'omega.epsilon')
        compiler += (ROOT / 'bootstrap/omega/construct_omega.epsilon').read_bytes()
    subjects = [*cases(), *terminal_cases(), *provider_cases(), *byte_output_cases(), *byte_input_cases(ROOT), *build_scope_cases(), *rooted_application_cases()]
    unknown = set(args.case or ()) - {name for name, _, _ in subjects}
    if unknown:
        parser.error(f"unknown cases {sorted(unknown)}")
    for name, packet, expected in subjects:
        if args.case and name not in args.case:
            continue
        (work / f'{name}.input').write_bytes(packet)
        if args.evaluator:
            request = epsilon_request(evaluator, compiler, packet)
            (work / f'{name}.eereq').write_bytes(request)
            runner = args.evaluator
        elif args.constructor:
            runner, request = args.constructor, packet
        else:
            continue
        code, observed = execute(runner.resolve(), request, args.seconds)
        (work / f'{name}.output').write_bytes(observed)
        if code:
            raise SystemExit(f'{name}: host exit {code}')
        if expected is None or isinstance(expected, int) or expected == 'trap':
            if not observed.startswith(bytes(5)) or len(observed) <= 5:
                raise SystemExit(f'{name}: expected compiled tape, got {observed[:32].hex()}')
            tape = work / f'{name}.tape'
            tape.write_bytes(observed[5:])
            program = work / name
            stamp(tape, program)
            code, output = execute(program, timeout=args.seconds)
            if expected == 'trap':
                if output or (code != -signal.SIGILL and code & 0xffffffff != 0xc000001d):
                    raise SystemExit(f'{name}: expected Alpha Trap, got {code} with {output!r}')
                print(f'{name}: Alpha Trap', flush=True)
            else:
                expected_status = 42 if expected is None else expected
                if (code, output) != (expected_status, b''):
                    raise SystemExit(f'{name}: expected Alpha exit {expected_status} and empty stdout, got {code} and {output!r}')
                print(f'{name}: Alpha exit {code}', flush=True)
        elif observed != expected:
            raise SystemExit(f'{name}: expected {expected.hex()}, got {observed[:32].hex()}')
        else:
            if len(observed) == 5:
                print(f'{name}: malformed framing', flush=True)
            else:
                print(f'{name}: refusal at byte {int.from_bytes(observed[5:], "little")}', flush=True)


if __name__ == '__main__':
    main()
