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
- Testing needs a real Windows 10 machine. Since processor encoding (1.1.0), a virtual
  machine without a graphics card can be used too.

## 3. Markers

- A hotkey to drop a marker while recording or while the replay is running.
- Markers shown on the player's timeline, to jump straight to them.
- Export to DaVinci Resolve as a marker EDL, so they land on the editing timeline.

## 4. Other leads, to be decided

- Signing the installer (Authenticode) so Windows SmartScreen no longer warns.
- More interface languages than French and English.
- Long-run checks: memory over hundreds of saves and multi-hour sessions.

## Done

- **Version 1.3.0**: the recording can have its own quality while the replay keeps running;
  Replay & quality estimates the size of clips and recordings; no more maximum of app tracks;
  settings changes show at once everywhere in the app.
- **Version 1.2.0**: Capture and Audio are pages of their own in the sidebar, with a warning
  when leaving a page with a change not applied yet; the clip can keep recording up to 10 min
  after the hotkey (another press extends it); videos are compressed to a target size (20 MB
  Discord, 50 / 500 MB Nitro or any size) from the Compress page, the gallery, the player or by
  dragging files from File Explorer.
- **Encoding on the processor (1.1.0)**: when no hardware encoder works, PalaClips encodes on
  the processor, in H.264 with the encoder built into Windows or in AV1 with SVT-AV1, with a
  warning about the load in game. Choose it in Settings → Replay & quality → Encoder.
