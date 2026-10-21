# Roadmap

**English** · [Français](ROADMAP.fr.md)

Version 1.0.0 is the first public version: everything in it is listed in the
[README](README.md). This page lists what comes next. Nothing here has a date, and the order
may change with feedback ([issues](https://github.com/Palawizard/PalaClips/issues) welcome).

## 1. Compatibility tested on more systems

So far PalaClips has been tested on one PC (Windows 11, NVIDIA RTX 4070 Super, 1440p 240 Hz
plus two other screens) and in a Windows 11 virtual machine without a graphics card. Next:

- AMD (AMF) and Intel (Quick Sync) graphics cards on real hardware: the code supports them,
  but they have never been tried outside of automatic detection.
- Laptops with two graphics chips (integrated + dedicated).
- Other Windows 11 versions and Windows Server 2022; display scaling from 100 to 200 %.
- More anti-cheats: tested with Easy Anti-Cheat (Fortnite); BattlEye and Vanguard to try.
- More audio setups: Bluetooth headsets, USB audio interfaces, virtual devices.

## 2. Encoding on the processor (CPU)

Today PalaClips only encodes on the graphics card (NVIDIA NVENC, AMD AMF, Intel Quick Sync):
a PC without a hardware encoder cannot record. Next: a processor encoder, used when no
hardware encoder works, with a clear warning about the CPU load it adds in game. PalaClips
ships FFmpeg under the LGPL, so the candidates are encoders with a compatible license, such
as OpenH264 or SVT-AV1 (x264 and x265 are GPL).

## 3. Other leads, to be decided

- Signing the installer (Authenticode) so Windows SmartScreen no longer warns.
- More interface languages than French and English.
- Long-run checks: memory over hundreds of saves and multi-hour sessions.
