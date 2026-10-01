"""Isolated native acceptance checks; never included in a product PR."""
import json
import ctypes
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import time

scenario, binary, source, output = sys.argv[1:]
output = Path(output).resolve()
output.mkdir(parents=True, exist_ok=True)
results = []
processes = []
permission_diagnostics = {}


def run(args, **kwargs):
    result = subprocess.run(args, capture_output=True, text=True, timeout=40, **kwargs)
    if result.returncode:
        raise AssertionError(result.stderr.strip())
    return result.stdout


def apple(body, pid=None):
    if pid is not None:
        body = f'tell application "System Events"\nset p to first application process whose unix id is {pid}\n' + body + '\nend tell'
    return run(['osascript', '-e', body])


class Point(ctypes.Structure):
    _fields_ = [('x', ctypes.c_double), ('y', ctypes.c_double)]


def native_input(x, y, scroll=False):
    cg = ctypes.CDLL('/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics')
    cf = ctypes.CDLL('/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation')
    cg.CGWarpMouseCursorPosition.argtypes = [Point]
    cg.CGEventPost.argtypes = [ctypes.c_uint32, ctypes.c_void_p]
    cf.CFRelease.argtypes = [ctypes.c_void_p]
    cg.CGWarpMouseCursorPosition(Point(x, y))
    if scroll:
        cg.CGEventCreateScrollWheelEvent.restype = ctypes.c_void_p
        cg.CGEventCreateScrollWheelEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_int32]
        event = cg.CGEventCreateScrollWheelEvent(None, 0, 1, -350)
        cg.CGEventPost(0, event)
        cf.CFRelease(event)
    else:
        cg.CGEventCreateMouseEvent.restype = ctypes.c_void_p
        cg.CGEventCreateMouseEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint32, Point, ctypes.c_uint32]
        for event_type in (1, 2):
            event = cg.CGEventCreateMouseEvent(None, event_type, Point(x, y), 0)
            cg.CGEventPost(0, event)
            cf.CFRelease(event)
            time.sleep(.08)
    time.sleep(.5)


def shot(name):
    run(['screencapture', '-x', str(output / (name + '.png'))])


def wait_for(check, label, seconds=30):
    deadline = time.monotonic() + seconds
    last = None
    while time.monotonic() < deadline:
        try:
            value = check()
            if value:
                results.append(label)
                return value
        except (subprocess.SubprocessError, OSError, ValueError, AssertionError) as error:
            last = str(error)
        time.sleep(.5)
    raise AssertionError(f'{label}: {last}')


def bundle(folder):
    app = output / folder / 'Pebrel.app'
    executable = app / 'Contents/MacOS/pebrel'
    executable.parent.mkdir(parents=True)
    shutil.copy2(binary, executable)
    plist = plistlib.loads((Path(source) / 'packaging/macos/Info.plist').read_bytes())
    manifest = (Path(source) / 'nebula_app/Cargo.toml').read_text()
    version = re.search(r'^version\s*=\s*"([^"]+)"', manifest, re.MULTILINE).group(1)
    plist.update(CFBundleName='Pebrel', CFBundleDisplayName='Pebrel', CFBundleIdentifier='io.github.kuddev.pebrel', CFBundleShortVersionString=version, CFBundleVersion=version)
    (app / 'Contents/Info.plist').write_bytes(plistlib.dumps(plist))
    run(['codesign', '--force', '--deep', '--sign', '-', str(app)])
    run(['/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister', '-f', str(app)])
    return executable


def notification_authorization_status(request=False):
    # Use the actual synthetic bundle, avoiding a second registration with the same ID.
    app = output / 'notification/Pebrel.app'
    executable = app / 'Contents/MacOS/permission-probe'
    source_file = output / 'authorization-probe.m'
    source_file.write_text('''#import <Cocoa/Cocoa.h>
#import <UserNotifications/UserNotifications.h>
#include <stdio.h>
static NSInteger request_error = 0;
static BOOL request_granted = NO;
static BOOL error_is_un = NO;
void print_status(void) {
    [[UNUserNotificationCenter currentNotificationCenter] getNotificationSettingsWithCompletionHandler:^(UNNotificationSettings *settings) {
        printf("%ld,%ld,%d,%d,%d\\n", (long)settings.authorizationStatus, (long)request_error, request_granted, error_is_un, [[[NSBundle mainBundle] bundleIdentifier] isEqualToString:@"io.github.kuddev.pebrel"]);
        fflush(stdout);
        exit(0);
    }];
}
int main(int argc, const char **argv) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSApp finishLaunching];
        if (argc > 1) {
            [NSApp activateIgnoringOtherApps:YES];
            [[UNUserNotificationCenter currentNotificationCenter] requestAuthorizationWithOptions:UNAuthorizationOptionAlert | UNAuthorizationOptionSound completionHandler:^(BOOL granted, NSError *error) { request_error = error.code; request_granted = granted; error_is_un = [error.domain isEqualToString:@"UNErrorDomain"]; print_status(); }];
        } else { print_status(); }
        [NSApp run];
    }
    return 1;
}
''')
    if not executable.exists():
        run(['clang', str(source_file), '-o', str(executable), '-framework', 'Cocoa', '-framework', 'UserNotifications'])
        run(['codesign', '--force', '--sign', '-', '--identifier', 'io.github.kuddev.pebrel', str(executable)])
        run(['codesign', '--force', '--deep', '--sign', '-', '--identifier', 'io.github.kuddev.pebrel', str(app)])
    if not request:
        values = [int(v) for v in run([str(executable)]).strip().split(',')]
        permission_diagnostics['permission_probe_has_expected_bundle_identifier'] = bool(values[4])
        return values[0]
    proc = subprocess.Popen([str(executable), 'request'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        deadline = time.monotonic() + 25
        while proc.poll() is None and time.monotonic() < deadline:
            flags = apple('''tell application "System Events"
set sawPebrel to false
set sawProbe to false
set sawAllow to false
set sawNotifications to false
set clickedPermission to false
repeat with procRef in application processes
try
repeat with win in windows of procRef
set labels to {}
set allowElement to missing value
repeat with e in entire contents of win
try
set end of labels to value of e as text
end try
try
set end of labels to name of e as text
if role of e is "AXButton" and (name of e is "Allow" or name of e is "Allow Notifications") then set allowElement to e
end try
end repeat
set textLabels to labels as text
if textLabels contains "Pebrel" then set sawPebrel to true
if textLabels contains "permission-probe" then set sawProbe to true
if textLabels contains "notification" then set sawNotifications to true
if allowElement is not missing value then set sawAllow to true
if (textLabels contains "Pebrel" or textLabels contains "permission-probe") and textLabels contains "notification" and allowElement is not missing value then
click allowElement
set clickedPermission to true
end if
end repeat
end try
end repeat
return (sawPebrel as integer) & "," & (sawProbe as integer) & "," & (sawAllow as integer) & "," & (sawNotifications as integer) & "," & (clickedPermission as integer) as text
end tell''').strip()
            values = [bool(int(v)) for v in flags.split(',')]
            for label, value in zip(['permission_prompt_mentions_Pebrel', 'permission_prompt_mentions_probe', 'permission_prompt_has_Allow', 'permission_prompt_mentions_notifications', 'synthetic_permission_prompt_accepted'], values):
                permission_diagnostics[label] = permission_diagnostics.get(label, False) or value
            time.sleep(.5)
        stdout, _ = proc.communicate(timeout=5)
        return [int(v) for v in stdout.strip().split(',')]
    finally:
        if proc.poll() is None:
            proc.kill()
            proc.wait()


def screen_words():
    # The CI desktop is synthetic; OCR stays local and is never exported as a raw dump.
    helper = output.parent / 'qa-screen-words'
    if not helper.exists():
        source = output.parent / 'qa-screen-words.swift'
        source.write_text("""import Foundation
import Vision
import ImageIO
let url = URL(fileURLWithPath: CommandLine.arguments[1])
let source = CGImageSourceCreateWithURL(url as CFURL, nil)!
let image = CGImageSourceCreateImageAtIndex(source, 0, nil)!
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.recognitionLanguages = ["en-US"]
try VNImageRequestHandler(cgImage: image, options: [:]).perform([request])
let words: [[String: Any]] = (request.results ?? []).compactMap { item in
    guard let candidate = item.topCandidates(1).first else { return nil }
    let box = item.boundingBox
    return ["text": candidate.string, "x": box.midX * Double(image.width), "y": (1 - box.midY) * Double(image.height)]
}
let json = try JSONSerialization.data(withJSONObject: words)
print(String(data: json, encoding: .utf8)!)
""")
        run(['swiftc', str(source), '-o', str(helper)])
    screen = output.parent / 'qa-ocr-frame.png'
    run(['screencapture', '-x', str(screen)])
    return json.loads(run([str(helper), str(screen)]))


def enable_test_notification_permission(executable):
    # Configure only the registered synthetic Pebrel app, after actual delivery.
    current_status = notification_authorization_status()
    permission_diagnostics['notification_authorization_status_after_actual_delivery'] = current_status
    if current_status in (2, 3, 4):
        return
    run(['open', 'x-apple.systempreferences:com.apple.Notifications-Settings.extension?bundleId=io.github.kuddev.pebrel'])
    def allow():
        if notification_authorization_status() in (2, 3, 4):
            return True
        bounds = [float(v) for v in apple('tell application \"System Events\" to tell application process \"System Settings\" to return (position of window 1) & (size of window 1)').strip().split(',')]
        left, top, width, height = bounds
        words = [word for word in screen_words() if left + width * .3 < word['x'] < left + width and top + 70 < word['y'] < top + height - 30]
        toggle = next((word for word in words if word['text'].lower() == 'allow notifications'), None)
        if toggle:
            native_input(left + width - 40, toggle['y'])
            permission_diagnostics['synthetic_notification_permission_toggle_clicked'] = True
            return notification_authorization_status() in (2, 3, 4)
        row = next((word for word in words if word['text'] == 'Pebrel'), None)
        if row:
            native_input(row['x'], row['y'])
            permission_diagnostics['synthetic_Pebrel_notification_row_clicked'] = True
            shot('00-pebrel-notification-permission-detail')
            return False
        outcome = apple('''tell application "System Events"
tell application process "System Settings"
set frontmost to true
set win to window 1
set allElements to entire contents of win
set isPebrel to false
repeat with e in allElements
try
set elementText to ""
try
set elementText to elementText & (name of e as text)
end try
try
set elementText to elementText & (value of e as text)
end try
try
set elementText to elementText & (description of e as text)
end try
if elementText contains "Pebrel" then set isPebrel to true
end try
end repeat
if isPebrel then
repeat with e in allElements
try
if name of e is "Allow notifications" or description of e is "Allow notifications" then
if role of e is "AXCheckBox" or role of e is "AXSwitch" then
if value of e is 0 then click e
return "enabled Pebrel notifications"
end if
end if
end try
end repeat
end if
set winPosition to position of win
set winSize to size of win
repeat with e in allElements
try
set elementText to ""
try
set elementText to elementText & (name of e as text)
end try
try
set elementText to elementText & (value of e as text)
end try
try
set elementText to elementText & (description of e as text)
end try
if elementText contains "Pebrel" then
set elementPosition to position of e
set elementSize to size of e
set px to item 1 of elementPosition
set py to item 2 of elementPosition
if px > (item 1 of winPosition) + (item 1 of winSize) * 0.3 and py > (item 2 of winPosition) + 70 and py < (item 2 of winPosition) + (item 2 of winSize) - 40 then
return "click," & (px + (item 1 of elementSize) / 2) & "," & (py + (item 2 of elementSize) / 2)
end if
end if
end try
end repeat
repeat with e in allElements
try
if role of e is "AXScrollArea" then
set ep to position of e
if item 1 of ep > (item 1 of winPosition) + (item 1 of winSize) * 0.3 then
set es to size of e
return "scroll," & ((item 1 of ep) + (item 1 of es) / 2) & "," & ((item 2 of ep) + (item 2 of es) / 2)
end if
end if
end try
end repeat
return "waiting"
end tell
end tell''').strip()
        if outcome.startswith('click,'):
            _, x, y = outcome.split(',')
            native_input(float(x), float(y))
            permission_diagnostics['synthetic_Pebrel_notification_row_clicked'] = True
        if outcome.startswith('scroll,'):
            _, x, y = outcome.split(',')
            native_input(float(x), float(y), scroll=True)
            permission_diagnostics['notification_app_list_scrolled'] = True
        return outcome == 'enabled Pebrel notifications'
    try:
        wait_for(allow, 'enabled notification permission for the synthetic Pebrel test bundle', 25)
        permission_diagnostics['synthetic_notification_permission_enabled'] = True
    except (AssertionError, subprocess.SubprocessError):
        permission_diagnostics['synthetic_notification_permission_enabled'] = False
    shot('00-notification-permission-settings')
    apple('tell application "System Settings" to quit')


def start(executable, name, configured=True, args=(), launch_services=False):
    env = dict(os.environ)
    for key in ['PEBREL_CONFIG_DIR', 'NEBULA_CONFIG_DIR', 'PEBREL_CONFIG_FILE', 'NEBULA_CONFIG_FILE', 'PEBREL_GPUI_CONFIG', 'NEBULA_GPUI_CONFIG']:
        env.pop(key, None)
    if configured:
        config = output / (name + '-config')
        config.mkdir()
        (config / 'pebrel_settings.txt').write_text('language=en-US\ntheme=Nord\nopacity=1\nblur=off\nrestore_session=false\nresume_ai=false\nauto_check_updates=off\nkeep_session=false\n')
        env['PEBREL_CONFIG_DIR'] = str(config)
    log = (output / (name + '.log')).open('w')
    env['PEBREL_EXTRA_LOG_TARGETS'] = 'pebrel'
    command = [str(executable), '-vv', *args]
    if launch_services:
        command = ['open', '-n', '-W', '--stdout', str(output / (name + '.log')), '--stderr', str(output / (name + '.err.log')), '--env', 'PEBREL_CONFIG_DIR=' + env['PEBREL_CONFIG_DIR'], '--env', 'PEBREL_EXTRA_LOG_TARGETS=pebrel', str(executable.parents[2]), '--args', '-vv', *args]
    p = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT)
    processes.append((p, log))
    return p, env


def key(pid, key, modifiers='command down'):
    apple(f'set frontmost of p to true\ndelay 0.2\nkeystroke {json.dumps(key)} using {{{modifiers}}}', pid)
    time.sleep(.8)


def snapshot(executable, env):
    response = json.loads(run([str(executable), 'ctl', 'snapshot', '--timeout-ms', '3000'], env=env))
    assert response.get('ok', False), response
    return response['result']


def choose(pid, button):
    # RFD 0.17 uses CFUserNotificationDisplayAlert for parentless dialogs.
    # macOS hosts that alert in a system UI process, rather than Pebrel's PID.
    script = f'''tell application "System Events"
repeat with proc in application processes
try
repeat with win in windows of proc
if exists button "Use portable mode" of win then
if exists button "Use normal mode" of win then
if exists button "Quit" of win then
click button {json.dumps(button)} of win
return "clicked startup choice"
end if
end if
end if
end repeat
end try
end repeat
return ""
end tell'''
    wait_for(lambda: apple(script), 'native startup choice: ' + button)


try:
    if scenario == 'menu':
        executable = bundle('menu')
        p, env = start(executable, 'menu')
        wait_for(lambda: len(snapshot(executable, env)['windows']) == 1, 'one initial window')
        names = apple('get name of every menu bar item of menu bar 1 of p', p.pid)
        (output / 'menu-names.txt').write_text(names)
        for name in ['Pebrel', 'File', 'Edit', 'View', 'Window']:
            assert name in names, names
        apple('set frontmost of p to true\nclick menu bar item "File" of menu bar 1 of p', p.pid)
        shot('01-file-menu')
        apple('click menu item "New tab" of menu 1 of menu bar item "File" of menu bar 1 of p', p.pid)
        wait_for(lambda: len(snapshot(executable, env)['windows'][0]['tabs']) == 2, 'native New tab creates a tab')
        key(p.pid, 'w', 'command down, shift down')
        wait_for(lambda: len(snapshot(executable, env)['windows'][0]['tabs']) == 1, 'command-shift-W closes only a tab')
        key(p.pid, 'n')
        wait_for(lambda: len(snapshot(executable, env)['windows']) == 2, 'command-N creates a window')
        key(p.pid, 'w')
        wait_for(lambda: len(snapshot(executable, env)['windows']) == 1, 'command-W closes only a window')
        apple('set frontmost of p to true\nclick menu item "About Pebrel" of menu 1 of menu bar item "Pebrel" of menu bar 1 of p', p.pid)
        shot('02-about-home')
        key(p.pid, 'q')
        wait_for(lambda: p.poll() is not None, 'command-Q completes graceful application exit', 45)
        assert p.returncode == 0
    elif scenario == 'portable':
        executable = bundle('quit-choice')
        p, env = start(executable, 'quit-choice', False)
        shot('01-startup-dialog')
        choose(p.pid, 'Quit')
        wait_for(lambda: p.poll() is not None, 'Quit choice exits before runtime')
        assert p.returncode == 0
        assert not (executable.parents[2].parent / 'Pebrel Data').exists()
        executable = bundle('portable')
        p, env = start(executable, 'portable', False)
        choose(p.pid, 'Use portable mode')
        data = executable.parents[2].parent / 'Pebrel Data'
        wait_for(lambda: (data / '.pebrel-portable').is_file(), 'portable choice creates adjacent marker')
        wait_for(lambda: (data / 'runtime.port').is_file(), 'portable resident endpoint published', 60)
        wait_for(lambda: len(snapshot(executable, env)['windows']) == 1, 'portable runtime accepts matching CLI')
        shot('02-portable-running')
        (data / 'acceptance-sentinel.txt').write_text('retained after move')
        key(p.pid, 'q')
        wait_for(lambda: p.poll() is not None, 'portable graceful exit')
        assert p.returncode == 0
        moved = output / 'moved'
        shutil.move(str(output / 'portable'), moved)
        executable = moved / 'Pebrel.app/Contents/MacOS/pebrel'
        p, env = start(executable, 'moved', False)
        wait_for(lambda: len(snapshot(executable, env)['windows']) == 1, 'moved portable app starts without prompt')
        assert (moved / 'Pebrel Data/acceptance-sentinel.txt').read_text() == 'retained after move'
        key(p.pid, 'q')
        wait_for(lambda: p.poll() is not None, 'moved app graceful exit')
        executable = bundle('normal')
        p, env = start(executable, 'normal', False)
        choose(p.pid, 'Use normal mode')
        wait_for(lambda: len(snapshot(executable, env)['windows']) == 1, 'normal choice starts installed storage')
        assert not (output / 'normal/Pebrel Data').exists()
        key(p.pid, 'q')
        wait_for(lambda: p.poll() is not None, 'normal app graceful exit')
    elif scenario == 'notification':
        run(['open', '-a', '/System/Library/CoreServices/NotificationCenter.app'])
        wait_for(lambda: apple('tell application \"System Events\" to exists application process \"NotificationCenter\"').strip() == 'true', 'native Notification Center is running')
        executable = bundle('notification')
        permission_diagnostics['initial_notification_authorization_status'] = notification_authorization_status()
        notice = 'Pebrel foreground acceptance 20260930'
        p, env = start(executable, 'notification', launch_services=True, args=['-e', '/bin/zsh', '-l', '-c', f'for attempt in 1 2 3 4 5; do sleep 20; printf "\\033]9;{notice}\\007"; done; sleep 45'])
        wait_for(lambda: len(snapshot(executable, env)['windows']) == 1, 'notification source window exists')
        app_pid = wait_for(lambda: int(run(['pgrep', '-f', re.escape(str(executable))]).strip().splitlines()[0]), 'Launch Services started the registered Pebrel application')
        apple('set frontmost of p to true', app_pid)
        wait_for(lambda: 'system toast source' in (output / 'notification.log').read_text(), 'foreground OSC 9 reaches system delivery', 35)
        enable_test_notification_permission(executable)
        apple('set frontmost of p to true', app_pid)
        shot('01-foreground-notification')
        def center_labels():
            return apple('''tell application "System Events"
set labels to {}
repeat with processName in {"NotificationCenter", "ControlCenter"}
if exists application process processName then
repeat with e in (entire contents of application process processName)
try
set end of labels to value of e as text
end try
try
set end of labels to name of e as text
end try
try
set end of labels to description of e as text
end try
end repeat
end if
end repeat
return labels as text
end tell''')
        try:
            labels = wait_for(lambda: (labels if notice in (labels := center_labels()) else None), 'native notification banner exposes notice', 30)
        except AssertionError:
            # System preferences can suppress banners; inspect the actual notification list as well.
            clock_position = apple('''tell application "System Events"
repeat with processName in {"ControlCenter", "SystemUIServer"}
if exists application process processName then
repeat with itemRef in menu bar items of menu bar 1 of application process processName
try
if description of itemRef contains "Clock" then
set clockPoint to position of itemRef
set clockSize to size of itemRef
return "click," & ((item 1 of clockPoint) + (item 1 of clockSize) / 2) & "," & ((item 2 of clockPoint) + (item 2 of clockSize) / 2)
end if
end try
end repeat
end if
end repeat
error "Clock accessibility description not found"
end tell''').strip()
            _, x, y = clock_position.split(',')
            native_input(float(x), float(y))
            permission_diagnostics['notification_center_click_x'] = float(x)
            permission_diagnostics['notification_center_click_y'] = float(y)
            shot('02-notification-center-opened')
            labels = wait_for(lambda: (labels if notice in (labels := center_labels()) else None), 'native Notification Center list exposes foreground notice', 20)
        (output / 'notification-center.txt').write_text(labels)
        assert notice in labels, 'Notification Center did not expose the delivered banner'
        assert 'toast failed' not in (output / 'notification.log').read_text()
        results.append('Notification Center exposes the foreground notice')
    else:
        raise ValueError(scenario)
finally:
    shot('99-final')
    diagnostics = dict(permission_diagnostics)
    if scenario == 'portable':
        diagnostics['process_exit_codes'] = [p.poll() for p, _ in processes]
        root = output / 'portable' / 'Pebrel Data'
        diagnostics['portable_endpoint'] = (root / 'runtime.port').is_file()
        diagnostics['portable_owner_lock'] = (root / 'runtime.port.lock').is_file()
        diagnostics['portable_process_panicked'] = 'panicked at' in (output / 'portable.log').read_text() if (output / 'portable.log').exists() else False
    if scenario == 'portable' and (output / 'portable.log').exists():
        portable_log = (output / 'portable.log').read_text()
        diagnostics['gpui_missing_application_ivar'] = 'ivar' in portable_log and 'panicked at' in portable_log
    if scenario == 'notification' and (output / 'notification.log').exists():
        notification_log = (output / 'notification.log').read_text()
        try:
            center_text = center_labels()
            diagnostics['pebrel_notification_permission_prompt'] = 'Pebrel' in center_text and 'Allow' in center_text
        except (AssertionError, subprocess.SubprocessError):
            diagnostics['notification_center_accessibility_failed'] = True
        try:
            diagnostics['notification_authorization_status'] = notification_authorization_status()
        except (AssertionError, subprocess.SubprocessError, ValueError, OSError):
            diagnostics['notification_authorization_probe_failed'] = True
        diagnostics['foreground_policy_install_failed'] = 'Could not enable foreground' in notification_log
        diagnostics['native_dispatch_failed'] = 'toast failed' in notification_log
        diagnostics['native_bundle_registration_missing'] = 'require a registered' in notification_log
        diagnostics['native_activation_failed'] = 'activation listener failed' in notification_log
    (output / 'acceptance.json').write_text(json.dumps({'scenario': scenario, 'checks': results, 'diagnostics': diagnostics}, indent=2))
    for p, log in processes:
        if p.poll() is None:
            p.terminate()
            try:
                p.wait(timeout=5)
            except subprocess.TimeoutExpired:
                p.kill()
                p.wait()
        log.close()
