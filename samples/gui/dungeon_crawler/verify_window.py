import argparse
import ctypes
import hashlib
import os
from pathlib import Path
import plistlib
import signal
import subprocess
import time


POINTER = ctypes.c_void_p
INTEGER = ctypes.c_long
CORE = ctypes.CDLL('/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation')
GRAPHICS = ctypes.CDLL('/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics')
ACCESS = ctypes.CDLL('/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices')
IMAGE = ctypes.CDLL('/System/Library/Frameworks/ImageIO.framework/ImageIO')
ctypes.CDLL('/System/Library/Frameworks/AppKit.framework/AppKit')
OBJECTIVE = ctypes.CDLL('/usr/lib/libobjc.A.dylib')


class ScreenPoint(ctypes.Structure):
    _fields_ = [('horizontal', ctypes.c_double), ('vertical', ctypes.c_double)]


def declare(library, name, result, *arguments):
    function = getattr(library, name)
    function.argtypes = list(arguments)
    function.restype = result
    return function


string = declare(CORE, 'CFStringCreateWithCString', POINTER, POINTER, ctypes.c_char_p, ctypes.c_uint32)
release = declare(CORE, 'CFRelease', None, POINTER)
data_pointer = declare(CORE, 'CFDataGetBytePtr', POINTER, POINTER)
data_length = declare(CORE, 'CFDataGetLength', INTEGER, POINTER)
property_data = declare(CORE, 'CFPropertyListCreateData', POINTER, POINTER, POINTER, INTEGER, ctypes.c_ulong, POINTER)
window_list = declare(GRAPHICS, 'CGWindowListCopyWindowInfo', POINTER, ctypes.c_uint32, ctypes.c_uint32)
current_session = declare(GRAPHICS, 'CGSessionCopyCurrentDictionary', POINTER)
trusted = declare(ACCESS, 'AXIsProcessTrusted', ctypes.c_bool)
selector = declare(OBJECTIVE, 'sel_registerName', POINTER, ctypes.c_char_p)
class_named = declare(OBJECTIVE, 'objc_getClass', POINTER, ctypes.c_char_p)
send = ctypes.CFUNCTYPE(POINTER, POINTER, POINTER)(('objc_msgSend', OBJECTIVE))
send_index = ctypes.CFUNCTYPE(POINTER, POINTER, POINTER, ctypes.c_ulong)(('objc_msgSend', OBJECTIVE))
send_number = ctypes.CFUNCTYPE(INTEGER, POINTER, POINTER)(('objc_msgSend', OBJECTIVE))
send_text = ctypes.CFUNCTYPE(ctypes.c_char_p, POINTER, POINTER)(('objc_msgSend', OBJECTIVE))
key_source = declare(GRAPHICS, 'CGEventSourceCreate', POINTER, ctypes.c_int)
key_event = declare(GRAPHICS, 'CGEventCreateKeyboardEvent', POINTER, POINTER, ctypes.c_uint16, ctypes.c_bool)
post_event = declare(GRAPHICS, 'CGEventPost', None, ctypes.c_uint32, POINTER)
mouse_event = declare(GRAPHICS, 'CGEventCreateMouseEvent', POINTER, POINTER, ctypes.c_uint32, ScreenPoint, ctypes.c_uint32)
run_loop = declare(CORE, 'CFRunLoopRunInMode', ctypes.c_int, POINTER, ctypes.c_double, ctypes.c_bool)


def cf_string(value):
    return string(None, value.encode(), 0x08000100)


def wait_for(predicate, message, seconds=8):
    deadline = time.monotonic() + seconds
    mode = cf_string('kCFRunLoopDefaultMode')
    try:
        while time.monotonic() < deadline:
            run_loop(mode, 0.01, True)
            value = predicate()
            if value:
                return value
        raise AssertionError(message)
    finally:
        release(mode)


def running_pid(executable):
    workspace = send(class_named(b'NSWorkspace'), selector(b'sharedWorkspace'))
    applications = send(workspace, selector(b'runningApplications'))
    for index in range(send_number(applications, selector(b'count'))):
        application = send_index(applications, selector(b'objectAtIndex:'), index)
        url = send(application, selector(b'executableURL'))
        path = send(url, selector(b'path')) if url else None
        encoded = send_text(path, selector(b'UTF8String')) if path else None
        if encoded and os.fsdecode(encoded) == str(executable):
            return send_number(application, selector(b'processIdentifier'))
    return None


def visible_window(pid):
    windows = window_list(17, 0)
    data = property_data(None, windows, 100, 0, None)
    try:
        rows = plistlib.loads(ctypes.string_at(data_pointer(data), data_length(data)))
        return next((row for row in rows if row.get('kCGWindowOwnerPID') == pid
                     and row.get('kCGWindowName') == 'HOLLOW / OMEGA'), None)
    finally:
        release(data)
        release(windows)


def press_key(source, code, down):
    event = key_event(source, code, down)
    assert event, 'Could not construct native keyboard event'
    try:
        post_event(0, event)
    finally:
        release(event)


def image_digest(path):
    url_create = declare(CORE, 'CFURLCreateFromFileSystemRepresentation', POINTER, POINTER, ctypes.c_char_p, INTEGER, ctypes.c_bool)
    source_create = declare(IMAGE, 'CGImageSourceCreateWithURL', POINTER, POINTER, POINTER)
    image_create = declare(IMAGE, 'CGImageSourceCreateImageAtIndex', POINTER, POINTER, ctypes.c_size_t, POINTER)
    provider = declare(GRAPHICS, 'CGImageGetDataProvider', POINTER, POINTER)
    copy_data = declare(GRAPHICS, 'CGDataProviderCopyData', POINTER, POINTER)
    width_of = declare(GRAPHICS, 'CGImageGetWidth', ctypes.c_size_t, POINTER)
    height_of = declare(GRAPHICS, 'CGImageGetHeight', ctypes.c_size_t, POINTER)
    stride_of = declare(GRAPHICS, 'CGImageGetBytesPerRow', ctypes.c_size_t, POINTER)
    encoded = os.fsencode(path)
    url = url_create(None, encoded, len(encoded), False)
    source = source_create(url, None)
    image = image_create(source, 0, None)
    assert image, 'Native window capture could not be decoded'
    data = copy_data(provider(image))
    try:
        width, height, stride = width_of(image), height_of(image), stride_of(image)
        pixels = ctypes.string_at(data_pointer(data), data_length(data))
        return width, height, hashlib.sha256(pixels[stride * min(30, height):]).hexdigest()
    finally:
        for value in [data, image, source, url]:
            release(value)


def capture(pid, path):
    window = wait_for(lambda: visible_window(pid), 'Game window is not on screen')
    subprocess.run(['screencapture', '-x', '-o', '-l', str(window['kCGWindowNumber']), str(path)], check=True, timeout=5)
    return image_digest(path)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--app', type=Path, default=Path(__file__).parent / 'build/native/dungeon_crawler.app')
    parser.add_argument('--cycles', type=int, default=2)
    arguments = parser.parse_args()
    application = arguments.app.resolve()
    executable = application / 'Contents/MacOS/dungeon_crawler'
    session = current_session()
    data = property_data(None, session, 100, 0, None)
    try:
        state = plistlib.loads(ctypes.string_at(data_pointer(data), data_length(data)))
        assert not state.get('CGSSessionScreenIsLocked'), 'Unlock the macOS desktop before running the native UI regression'
    finally:
        release(data)
        release(session)
    assert trusted(), 'Accessibility permission is required for the close-button regression'
    assert not running_pid(executable), 'Close the existing game before running its UI regression'
    assert 1 <= arguments.cycles <= 5
    output = Path(__file__).parent / 'build'
    for cycle in range(arguments.cycles):
        pid = None
        launcher = subprocess.Popen(['open', '-W', '-n', str(application)])
        try:
            pid = wait_for(lambda: running_pid(executable), 'Native application did not launch')
            window = wait_for(lambda: visible_window(pid), 'Original-identifier game never became on screen')
            print(f'cycle {cycle + 1}: on-screen window {window["kCGWindowNumber"]}, pid {pid}', flush=True)
            before = capture(pid, output / f'window-{cycle + 1}-before.png')
            assert before[0] >= 256 and before[1] >= 192, f'Game frame is unexpectedly small: {before[:2]}'
            source = key_source(1)
            try:
                press_key(source, 2, True)
                wait_for(lambda: changed_capture(pid, output / f'window-{cycle + 1}-after.png', before), 'Keyboard input did not change rendered pixels', seconds=3)
                print(f'cycle {cycle + 1}: keyboard movement changed framebuffer', flush=True)
            finally:
                press_key(source, 2, False)
                release(source)
            window = wait_for(lambda: visible_window(pid), 'Game window vanished before close')
            bounds = window['kCGWindowBounds']
            point = ScreenPoint(bounds['X'] + 14, bounds['Y'] + 14)
            for kind in [1, 2]:
                event = mouse_event(None, kind, point, 0)
                assert event, 'Could not construct native close-button mouse event'
                try:
                    post_event(0, event)
                finally:
                    release(event)
            wait_for(lambda: not running_pid(executable), 'Close button left a hidden running game')
            assert launcher.wait(timeout=5) == 0, 'Application launch failed'
            print(f'cycle {cycle + 1}: close button exited without force-kill', flush=True)
        finally:
            if pid and running_pid(executable) == pid:
                os.kill(pid, signal.SIGKILL)
            if launcher.poll() is None:
                launcher.terminate()
                launcher.wait(timeout=5)
    print(f'PASS: {arguments.cycles} original-identifier launch/input/close cycles', flush=True)


def changed_capture(pid, path, before):
    after = capture(pid, path)
    return after if after[:2] == before[:2] and after[2] != before[2] else None


if __name__ == '__main__':
    main()