import argparse
import array
import json
import math
import subprocess

parser = argparse.ArgumentParser(description="Niveau de chaque fréquence, seconde par seconde, sur chaque piste audio d'un MP4 (test d'isolation par appli)")
parser.add_argument("file")
parser.add_argument("--hz", type=float, action="append", required=True, help="fréquence à mesurer (répétable)")
parser.add_argument("--json", action="store_true", help="sortie JSON")
args = parser.parse_args()

RATE = 48_000

probe = subprocess.run(
    ["ffprobe", "-v", "error", "-select_streams", "a", "-show_entries", "stream_tags=handler_name", "-of", "csv=p=0", args.file],
    capture_output=True, text=True, check=True).stdout.splitlines()


def level_db(block, hz):
    coefficient = 2 * math.cos(2 * math.pi * hz / RATE)
    previous = before = 0.0
    for sample in block:
        previous, before = sample + coefficient * previous - before, previous
    power = previous * previous + before * before - coefficient * previous * before
    amplitude = 2 * math.sqrt(max(power, 0.0)) / len(block)
    return round(20 * math.log10(amplitude), 1) if amplitude > 1e-6 else None


tracks = []
for index, title in enumerate(probe):
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", args.file, "-map", f"0:a:{index}", "-ac", "1", "-ar", str(RATE), "-f", "f32le", "-"],
                         capture_output=True, check=True).stdout
    samples = array.array("f")
    samples.frombytes(raw)
    seconds = [{str(hz): level_db(samples[start:start + RATE], hz) for hz in args.hz}
               for start in range(0, len(samples) - RATE + 1, RATE)]
    tracks.append({"index": index, "title": title, "seconds": seconds})

if args.json:
    print(json.dumps(tracks))
else:
    for track in tracks:
        print(f"piste audio {track['index']} « {track['title']} » (dBFS, vide = rien)")
        for second, levels in enumerate(track["seconds"]):
            cells = "  ".join(f"{hz} Hz {'' if value is None else f'{value:+.0f}':>5}" for hz, value in levels.items())
            print(f"  {second:>4} s  {cells}")
