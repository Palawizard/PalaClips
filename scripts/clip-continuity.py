import argparse
import json
import subprocess
from collections import defaultdict

parser = argparse.ArgumentParser(description="Vérifie qu'un MP4 n'a ni trou ni paquet en double : écarts de PTS par flux après le premier paquet (le MP4 fragmenté ramène le premier échantillon de chaque piste à 0)")
parser.add_argument("file")
parser.add_argument("--json", action="store_true", help="sortie JSON")
args = parser.parse_args()

probe = json.loads(subprocess.run(
    ["ffprobe", "-v", "error", "-show_entries",
     "stream=index,codec_type,time_base,r_frame_rate,sample_rate,duration:packet=stream_index,pts",
     "-of", "json", args.file],
    capture_output=True, text=True, check=True).stdout)

pts = defaultdict(list)
for packet in probe["packets"]:
    if packet.get("pts") is not None:
        pts[packet["stream_index"]].append(int(packet["pts"]))

streams = []
for stream in probe["streams"]:
    num, den = (int(part) for part in stream["time_base"].split("/"))
    values = sorted(pts[stream["index"]])
    if stream["codec_type"] == "video":
        rate_num, rate_den = (int(part) for part in stream["r_frame_rate"].split("/"))
        expected = den * rate_den / (num * rate_num)
    else:
        expected = 1024 * den / (num * int(stream["sample_rate"]))
    first_step_ms = round((values[1] - values[0]) * num / den * 1000, 2) if len(values) > 1 else 0
    values = values[1:]
    diffs = [later - earlier for earlier, later in zip(values, values[1:])]
    tolerance = max(1.0, expected * 0.01)
    duplicates = sum(1 for diff in diffs if diff == 0)
    gaps = [diff for diff in diffs if diff > expected + tolerance]
    gap_at = [round((values[i] - values[0]) * num / den, 3) for i, diff in enumerate(diffs) if diff > expected + tolerance]
    short = [diff for diff in diffs if 0 < diff < expected - tolerance]
    streams.append({
        "index": stream["index"],
        "type": stream["codec_type"],
        "packets": len(values),
        "duration_s": float(stream.get("duration", 0)),
        "duplicates": duplicates,
        "gaps": len(gaps),
        "largest_gap_ms": round(max(gaps, default=0) * num / den * 1000, 2),
        "gaps_at_s": gap_at[:10],
        "short_steps": len(short),
        "first_step_ms": first_step_ms,
    })

result = {
    "file": args.file,
    "continuous": all(s["duplicates"] == 0 and s["gaps"] == 0 and s["short_steps"] == 0 for s in streams),
    "streams": streams,
}
if args.json:
    print(json.dumps(result))
else:
    for stream in streams:
        print(f"flux {stream['index']} {stream['type']} : {stream['packets']} paquets, {stream['duration_s']:.3f} s, "
              f"doublons {stream['duplicates']}, trous {stream['gaps']} (max {stream['largest_gap_ms']} ms, à {stream['gaps_at_s']} s), pas courts {stream['short_steps']}, premier pas {stream['first_step_ms']} ms")
    print("CONTINU" if result["continuous"] else "DISCONTINU")
