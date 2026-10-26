import argparse
import array
import json
import math
import os
import subprocess
import tempfile

parser = argparse.ArgumentParser(description="Cherche les sons de PalaClips (forme exacte, corrélation normalisée) dans chaque piste audio d'un MP4")
parser.add_argument("file")
parser.add_argument("--threshold", type=float, default=0.6, help="corrélation normalisée minimale (0 à 1)")
parser.add_argument("--json", action="store_true", help="sortie JSON")
args = parser.parse_args()

RATE = 16_000
VOLUME = 0.22
SOUNDS = {
    "sauvegarde": [(880.0, 0.0, 0.11), (1318.5, 0.075, 0.2)],
    "début d'enregistrement": [(523.25, 0.0, 0.1), (783.99, 0.075, 0.2)],
    "fin d'enregistrement": [(783.99, 0.0, 0.1), (523.25, 0.075, 0.2)],
}


def sound_samples(notes):
    length = max(start + duration for _, start, duration in notes)
    samples = []
    for index in range(int(length * RATE)):
        time = index / RATE
        value = 0.0
        for frequency, start, duration in notes:
            if start <= time < start + duration:
                local = time - start
                attack = min(local / 0.004, 1.0)
                release = min((duration - local) / 0.03, 1.0)
                value += math.sin(local * frequency * math.tau) * attack * release * math.exp(-local * 9.0)
        samples.append(value * VOLUME)
    return samples


def decode(stream):
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", args.file, "-map", f"0:a:{stream}", "-ac", "1", "-ar", str(RATE), "-f", "f32le", "-"],
                         capture_output=True, check=True).stdout
    samples = array.array("f")
    samples.frombytes(raw)
    return samples


def correlate(stream, template_path, length):
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", args.file, "-f", "f32le", "-ar", str(RATE), "-ac", "1", "-i", template_path,
                          "-filter_complex", f"[0:a:{stream}]aformat=sample_fmts=flt:sample_rates={RATE}:channel_layouts=mono,apad=pad_len={length}[signal];"
                          f"[signal][1:a]afir=dry=1:wet=1:irnorm=-1:irgain=1[out]",
                          "-map", "[out]", "-f", "f32le", "-"], capture_output=True, check=True).stdout
    values = array.array("f")
    values.frombytes(raw)
    return values


probe = subprocess.run(
    ["ffprobe", "-v", "error", "-select_streams", "a", "-show_entries", "stream_tags=handler_name", "-of", "csv=p=0", args.file],
    capture_output=True, text=True, check=True).stdout.splitlines()

templates = {}
with tempfile.TemporaryDirectory() as folder:
    for name, notes in SOUNDS.items():
        samples = sound_samples(notes)
        path = os.path.join(folder, f"{len(templates)}.raw")
        with open(path, "wb") as file:
            array.array("f", reversed(samples)).tofile(file)
        templates[name] = (path, len(samples), math.sqrt(sum(value * value for value in samples)))

    tracks = []
    for stream, title in enumerate(probe):
        signal = decode(stream)
        energy = [0.0]
        for value in signal:
            energy.append(energy[-1] + value * value)
        candidates = []
        for name, (path, length, norm) in templates.items():
            correlation = correlate(stream, path, length)
            for end in range(length - 1, min(len(correlation), len(signal))):
                start = end - length + 1
                local = energy[end + 1] - energy[start]
                if local <= 1e-9:
                    continue
                score = correlation[end] / (norm * math.sqrt(local))
                if score >= args.threshold:
                    candidates.append((score, start, name, local))
        candidates.sort(reverse=True)
        found = []
        for score, start, name, local in candidates:
            if all(abs(start - hit["start"]) > RATE * 0.3 for hit in found):
                level = 10 * math.log10(local / templates[name][1]) if local > 0 else -140.0
                found.append({"start": start, "sound": name, "at_s": round(start / RATE, 3), "score": round(score, 2), "rms_db": round(level, 1)})
        found.sort(key=lambda hit: hit["start"])
        for hit in found:
            del hit["start"]
        tracks.append({"index": stream, "title": title, "duration_s": round(len(signal) / RATE, 2), "chimes": found})

if args.json:
    print(json.dumps(tracks, ensure_ascii=False))
else:
    for track in tracks:
        hits = ", ".join(f"{hit['sound']} à {hit['at_s']} s (corrélation {hit['score']})" for hit in track["chimes"]) or "aucun son de PalaClips"
        print(f"piste {track['index'] + 1} « {track['title']} » ({track['duration_s']} s) : {hits}")
