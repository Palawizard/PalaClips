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

## 2. Windows 10 (2004 and later)

PalaClips needs Windows 11 today. Windows 10 version 2004 (build 19041) and later could work:

- Per-app audio capture is not officially documented before Windows 11, but it already works
  from build 19041: OBS (with the win-capture-audio plugin) and GStreamer rely on it.
- The yellow border Windows draws around a captured screen probably cannot be hidden on
  Windows 10.
- Testing needs a real Windows 10 machine. With processor encoding (next section), a virtual
  machine without a graphics card could be used too.

## 3. Encoding on the processor (CPU)

Today PalaClips only encodes on the graphics card (NVIDIA NVENC, AMD AMF, Intel Quick Sync):
a PC without a hardware encoder cannot record. Next: a processor encoder, used when no
hardware encoder works, with a clear warning about the CPU load it adds in game. PalaClips
ships FFmpeg under the LGPL, so the candidates are encoders with a compatible license, such
as OpenH264 or SVT-AV1 (x264 and x265 are GPL).

## 4. Markers

- A hotkey to drop a marker while recording or while the replay is running.
- Markers shown on the player's timeline, to jump straight to them.
- Export to DaVinci Resolve as a marker EDL, so they land on the editing timeline.

## 5. Compress to a target size

- A dedicated page in the sidebar, plus a button from the gallery and the player.
- Size in MB, with presets (20 MB for Discord, 500 MB for Nitro) or a free value.
- Bitrate computed from the size and the length; sound mixed into a single track.
- If the target is too low for good quality, resolution and frame rate go down automatically.
- The final size is checked, and the video is encoded again if it is over the target.
- Also accepts a video dragged in from File Explorer.

## 6. Capture and Audio in the sidebar

- Capture and Audio become pages of their own: Status, Gallery, Capture, Audio, Settings.
- A warning when leaving a page with a change that is not applied yet.
- On the Status screen, the source and the audio tracks open these pages when clicked.

## 7. Other leads, to be decided

- Signing the installer (Authenticode) so Windows SmartScreen no longer warns.
- More interface languages than French and English.
- Long-run checks: memory over hundreds of saves and multi-hour sessions.
