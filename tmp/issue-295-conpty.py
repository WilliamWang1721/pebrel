"""One-off native input probe; deliberately not part of the product PR."""
import ctypes as c
from ctypes import wintypes as w
import json
from pathlib import Path
import subprocess
import sys
import threading
import time

k = c.WinDLL('kernel32', use_last_error=True)
u = c.WinDLL('user32', use_last_error=True)
P = c.c_void_p

def bind(dll, name, restype, *args):
    fn = getattr(dll, name)
    fn.restype, fn.argtypes = restype, list(args)
    return fn

def checked(ok):
    if not ok:
        raise c.WinError(c.get_last_error())
    return ok

class Coord(c.Structure):
    _fields_ = [('x', w.SHORT), ('y', w.SHORT)]

class Startup(c.Structure):
    _fields_ = [('cb', w.DWORD), ('reserved', P), ('desktop', P), ('title', P)] + [(n, w.DWORD) for n in ('x', 'y', 'xs', 'ys', 'xc', 'yc', 'fill', 'flags')] + [('show', w.WORD), ('reserved_size', w.WORD), ('reserved_bytes', P), ('stdin', w.HANDLE), ('stdout', w.HANDLE), ('stderr', w.HANDLE)]

class StartupEx(c.Structure):
    _fields_ = [('startup', Startup), ('attributes', P)]

class Process(c.Structure):
    _fields_ = [('process', w.HANDLE), ('thread', w.HANDLE), ('pid', w.DWORD), ('tid', w.DWORD)]

class Key(c.Structure):
    _fields_ = [('down', w.BOOL), ('repeat', w.WORD), ('vk', w.WORD), ('scan', w.WORD), ('char', w.WORD), ('modifiers', w.DWORD)]

class EventBody(c.Union):
    _fields_ = [('key', Key), ('padding', c.c_byte * 16)]

class Event(c.Structure):
    _fields_ = [('type', w.WORD), ('body', EventBody)]

read = bind(k, 'ReadFile', w.BOOL, w.HANDLE, P, w.DWORD, P, P)
write = bind(k, 'WriteFile', w.BOOL, w.HANDLE, P, w.DWORD, P, P)
close = bind(k, 'CloseHandle', w.BOOL, w.HANDLE)

def send(handle, data):
    count = w.DWORD()
    checked(write(handle, data, len(data), c.byref(count), None))
    assert count.value == len(data)

def child(mode, report):
    create_file = bind(k, 'CreateFileW', w.HANDLE, w.LPCWSTR, w.DWORD, w.DWORD, P, w.DWORD, w.DWORD, w.HANDLE)
    handle = create_file('CONIN$', 0xC0000000, 3, None, 3, 0, None)
    assert handle != c.c_void_p(-1).value
    set_mode = bind(k, 'SetConsoleMode', w.BOOL, w.HANDLE, w.DWORD)
    checked(set_mode(handle, 0x200 if mode == 'bytes' else 0))
    Path(str(report) + '.ready').write_text('ready')
    count = w.DWORD()
    if mode == 'bytes':
        buf = c.create_string_buffer(1)
        checked(read(handle, buf, 1, c.byref(count), None))
        result = {'hex': buf.raw[:count.value].hex()}
    else:
        read_input = bind(k, 'ReadConsoleInputW', w.BOOL, w.HANDLE, P, w.DWORD, P)
        event = Event()
        while True:
            checked(read_input(handle, c.byref(event), 1, c.byref(count)))
            if event.type == 1 and event.body.key.down:
                key = event.body.key
                result = {name: getattr(key, name) for name in ('vk', 'scan', 'char', 'modifiers')}
                break
    Path(report).write_text(json.dumps(result))
    close(handle)

def probe(dll_path, mode, payload, report):
    runtime = c.WinDLL(str(dll_path.resolve()), use_last_error=True)
    create_pty = bind(runtime, 'CreatePseudoConsole', c.c_long, Coord, w.HANDLE, w.HANDLE, w.DWORD, P)
    close_pty = bind(runtime, 'ClosePseudoConsole', None, P)
    pipe = bind(k, 'CreatePipe', w.BOOL, P, P, P, w.DWORD)
    ir, iw, ore, ow = (w.HANDLE() for _ in range(4))
    checked(pipe(c.byref(ir), c.byref(iw), None, 0))
    checked(pipe(c.byref(ore), c.byref(ow), None, 0))
    send(iw, b'\x1b[?61c')
    hp = P()
    assert create_pty(Coord(80, 24), ir, ow, 4, c.byref(hp)) == 0
    close(ir)
    close(ow)
    output = bytearray()
    def drain():
        buf = c.create_string_buffer(4096)
        count = w.DWORD()
        while read(ore, buf, len(buf), c.byref(count), None) and count.value:
            output.extend(buf.raw[:count.value])
    reader = threading.Thread(target=drain, daemon=True)
    reader.start()
    init = bind(k, 'InitializeProcThreadAttributeList', w.BOOL, P, w.DWORD, w.DWORD, P)
    update = bind(k, 'UpdateProcThreadAttribute', w.BOOL, P, w.DWORD, c.c_size_t, P, c.c_size_t, P, P)
    delete = bind(k, 'DeleteProcThreadAttributeList', None, P)
    size = c.c_size_t()
    init(None, 1, 0, c.byref(size))
    attributes = c.create_string_buffer(size.value)
    checked(init(attributes, 1, 0, c.byref(size)))
    checked(update(attributes, 0, 0x20016, hp, c.sizeof(hp), None, None))
    startup = StartupEx()
    startup.startup.cb = c.sizeof(startup)
    startup.startup.flags = 0x100
    startup.attributes = c.cast(attributes, P)
    process = Process()
    create = bind(k, 'CreateProcessW', w.BOOL, w.LPCWSTR, w.LPWSTR, P, P, w.BOOL, w.DWORD, P, w.LPCWSTR, P, P)
    command = c.create_unicode_buffer(subprocess.list2cmdline([sys.executable, str(Path(__file__).resolve()), '--child', mode, str(report.resolve())]))
    checked(create(None, command, None, None, False, 0x80000, None, None, c.byref(startup), c.byref(process)))
    delete(attributes)
    close(process.thread)
    wait = bind(k, 'WaitForSingleObject', w.DWORD, w.HANDLE, w.DWORD)
    terminate = bind(k, 'TerminateProcess', w.BOOL, w.HANDLE, w.UINT)
    try:
        ready = Path(str(report) + '.ready')
        deadline = time.monotonic() + 20
        while not ready.exists() and time.monotonic() < deadline:
            time.sleep(0.02)
        assert ready.exists(), output.decode(errors='replace')
        send(iw, payload)
        assert wait(process.process, 20000) == 0, output.decode(errors='replace')
        return json.loads(report.read_text())
    finally:
        terminate(process.process, 1)
        close(process.process)
        close(iw)
        close_pty(hp)
        reader.join(timeout=2)
        close(ore)

if __name__ == '__main__':
    if sys.argv[1] == '--child':
        child(sys.argv[2], sys.argv[3])
    else:
        dll = Path(sys.argv[1])
        folder = Path(sys.argv[2]).resolve()
        folder.mkdir(parents=True, exist_ok=True)
        lookup = bind(u, 'VkKeyScanW', c.c_short, w.WCHAR)
        scan = bind(u, 'MapVirtualKeyW', w.UINT, w.UINT, w.UINT)
        mapping = lookup('/')
        assert mapping == 191, f'This comparison requires the runner US layout: {mapping}'
        sc = scan(mapping, 0)
        fixed = f'\x1b[191;{sc};0;1;8;1_\x1b[191;{sc};0;0;8;1_'.encode()
        result = {}
        for name, payload in [('legacy', b'\x1f'), ('win32', fixed)]:
            for mode in ('native', 'bytes'):
                label = name + '-' + mode
                result[label] = probe(dll, mode, payload, folder / (label + '.json'))
                print(label, result[label], flush=True)
        (folder / 'summary.json').write_text(json.dumps(result, indent=2))
        assert result['legacy-native']['vk'] == 189
        assert result['legacy-native']['modifiers'] == 24
        assert result['win32-native']['vk'] == 191
        assert result['win32-native']['scan'] == sc
        assert result['win32-native']['modifiers'] == 8
        assert result['legacy-bytes']['hex'] == result['win32-bytes']['hex'] == '1f'
