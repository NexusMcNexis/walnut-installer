#!/usr/bin/env python3
"""Writes tests/fixtures/shortcuts_two_games.vdf with an independent binary
VDF writer (not the Rust code under test), and prints the app ids Python's
zlib.crc32 gives, which the Rust tests compare against."""
import struct
import zlib
from pathlib import Path


def s(k, v):
    return b"\x01" + k.encode() + b"\0" + v.encode() + b"\0"


def i(k, v):
    return b"\x02" + k.encode() + b"\0" + struct.pack("<i", v)


def m(k, body):
    return b"\x00" + k.encode() + b"\0" + body + b"\x08"


def app_id(exe, name):
    return zlib.crc32((exe + name).encode()) | 0x80000000


def signed(u):
    return u - (1 << 32) if u >= (1 << 31) else u


a_exe, a_name = '"/home/player/Games/Other/other.x86_64"', "Other Game"
b_exe, b_name = '"/usr/bin/retroarch"', "RetroArch — Ünïcode"
a = i("appid", signed(app_id(a_exe, a_name))) + s("AppName", a_name) + s("Exe", a_exe) + s("StartDir", '"/home/player/Games/Other/"') + s("icon", "") + s("LaunchOptions", "-fast") + i("IsHidden", 0) + i("LastPlayTime", 1764000000) + m("tags", s("0", "favorite") + s("1", "Indie"))
b = i("appid", signed(app_id(b_exe, b_name))) + s("AppName", b_name) + s("Exe", b_exe) + s("StartDir", '"/usr/bin/"') + i("AllowOverlay", 1) + m("tags", b"")
out = Path(__file__).with_name("shortcuts_two_games.vdf")
out.write_bytes(m("shortcuts", m("0", a) + m("1", b)) + b"\x08")
print(out.name, out.stat().st_size, "bytes")
for exe, name in [(a_exe, a_name), ('"/tmp/home/.local/share/hello/hello.sh"', "Hello Walnut"), ('"/opt/x/y"', "Ünïcode Game")]:
    print(repr(exe), repr(name), app_id(exe, name))
