import argparse
import array
import json
import subprocess

parser = argparse.ArgumentParser(description="Mesure le décalage son − image d'un MP4 PalaClips à partir des flashs et bips de probe-av-emitter")
parser.add_argument("file")
parser.add_argument("--audio", type=int, default=0, help="index de la piste audio (0 = mix, 1 = micro)")
parser.add_argument("--window", type=int, default=30, help="taille des fenêtres de résultats en secondes")
parser.add_argument("--json", action="store_true", help="sortie JSON")
args = parser.parse_args()


def decode(*options):
    return subprocess.run(["ffmpeg", "-v", "error", "-i", args.file, *options, "-"], capture_output=True, check=True).stdout


rate = subprocess.run(
    ["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=r_frame_rate", "-of", "csv=p=0", args.file],
    capture_output=True, text=True, check=True).stdout.strip().split("/")
fps = int(rate[0]) / int(rate[1])

frame_bytes = 64 * 36
video = decode("-map", "0:v:0", "-vf", "scale=64:36,format=gray", "-f", "rawvideo")
brightness = [sum(video[i:i + frame_bytes]) / frame_bytes for i in range(0, len(video), frame_bytes)]
flashes = [i / fps for i in range(1, len(brightness)) if brightness[i] > 128 >= brightness[i - 1]]

samples = array.array("f")
samples.frombytes(decode("-map", f"0:a:{args.audio}", "-ac", "1", "-ar", "48000", "-f", "f32le"))
peak = max((abs(value) for value in samples), default=0.0)
threshold = max(0.01, 0.3 * peak)
beeps = []
quiet = 0
for index, value in enumerate(samples):
    if abs(value) > threshold:
        if quiet >= 4_800:
            beeps.append(index / 48_000)
        quiet = 0
    else:
        quiet += 1

pairs = []
for flash in flashes:
    beep = next((beep for beep in beeps if -0.2 <= beep - flash < 0.8), None)
    if beep is not None:
        pairs.append((flash, (beep - flash) * 1_000))

windows = []
for start in range(0, int(len(brightness) / fps) + 1, args.window):
    offsets = [offset for flash, offset in pairs if start <= flash < start + args.window]
    if offsets:
        windows.append({
            "start_s": start,
            "pairs": len(offsets),
            "min_ms": round(min(offsets), 1),
            "mean_ms": round(sum(offsets) / len(offsets), 1),
            "max_ms": round(max(offsets), 1),
        })
offsets = [offset for _, offset in pairs]
result = {
    "file": args.file,
    "audio_track": args.audio,
    "flashes": len(flashes),
    "beeps": len(beeps),
    "pairs": len(pairs),
    "mean_ms": round(sum(offsets) / len(offsets), 1) if offsets else None,
    "windows": windows,
}
if args.json:
    print(json.dumps(result))
else:
    print(f"{args.file}, piste audio {args.audio} : {len(flashes)} flashs, {len(beeps)} bips, {len(pairs)} paires (positif = son en retard)")
    for window in windows:
        print(f"  {window['start_s']:>5} s : {window['pairs']:>3} paires, min {window['min_ms']:+.1f} ms, moy {window['mean_ms']:+.1f} ms, max {window['max_ms']:+.1f} ms")
