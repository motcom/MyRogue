#!/usr/bin/env python3
"""Generate deterministic game sound effects using only the Python standard library."""

import argparse
from array import array
import math
from pathlib import Path
import random
import sys
import wave

SAMPLE_RATE = 44100
TAU = 2.0 * math.pi
DEFAULT_OUTPUT = Path(__file__).resolve().parent.parent / "data" / "sound"


def noise(duration, seed, cutoff):
    rng = random.Random(seed)
    alpha = 1.0 - math.exp(-TAU * cutoff / SAMPLE_RATE)
    low = 0.0
    samples = []
    for _ in range(round(duration * SAMPLE_RATE)):
        low += alpha * (rng.uniform(-1.0, 1.0) - low)
        samples.append(low)
    return samples


def envelope(t, decay, attack=0.001):
    return (1.0 - math.exp(-t / attack)) * math.exp(-t / decay)


def melee_attack():
    duration = 0.30
    air = noise(duration, seed=101, cutoff=6500)
    body = noise(duration, seed=102, cutoff=1100)
    samples = []
    for i, (hiss, low) in enumerate(zip(air, body)):
        t = i / SAMPLE_RATE
        # A short swish followed by a low, dry strike.
        swing = 0.9 * (hiss - low) * math.exp(-((t - 0.045) / 0.025) ** 2)
        hit = t - 0.082
        impact = 0.0
        if hit >= 0.0:
            phase = TAU * (82.0 * hit + 65.0 * 0.018 * (1.0 - math.exp(-hit / 0.018)))
            impact = (1.2 * low + 0.45 * math.sin(phase)) * envelope(hit, 0.033)
            impact += 0.22 * hiss * envelope(hit, 0.008)
        samples.append(swing + impact)
    return samples, 0.72


def gunshot():
    duration = 0.44
    air = noise(duration, seed=201, cutoff=14500)
    bass = noise(duration, seed=202, cutoff=800)
    samples = []
    for i, (crack, rumble) in enumerate(zip(air, bass)):
        t = i / SAMPLE_RATE
        phase = TAU * (52.0 * t + 140.0 * 0.015 * (1.0 - math.exp(-t / 0.015)))
        blast = 1.8 * crack * envelope(t, 0.014, 0.0003)
        blast += (1.3 * rumble + 0.42 * math.sin(phase)) * envelope(t, 0.047)
        # Quiet mechanical rattle after the initial report.
        for offset, frequency, gain in [(0.042, 2100.0, 0.13), (0.083, 3200.0, 0.065)]:
            click = t - offset
            if click >= 0.0:
                blast += gain * (math.sin(TAU * frequency * click) + crack) * envelope(click, 0.009)
        samples.append(blast)
    dry = samples[:]
    delay = round(0.027 * SAMPLE_RATE)
    for i in range(delay, len(samples)):
        samples[i] += dry[i - delay] * 0.13
    return samples, 0.80


def crossbow():
    duration = 0.38
    air = noise(duration, seed=301, cutoff=7500)
    wood = noise(duration, seed=302, cutoff=2100)
    samples = []
    for i, (hiss, click) in enumerate(zip(air, wood)):
        t = i / SAMPLE_RATE
        # A plucked string, a wooden snap, and the bolt leaving the bow.
        phase = TAU * (132.0 * t + 20.0 * 0.020 * (1.0 - math.exp(-t / 0.020)))
        string = sum(math.sin(harmonic * phase) / harmonic ** 1.4 for harmonic in range(1, 7))
        string *= 0.42 * envelope(t, 0.065)
        snap = (0.75 * click + 0.25 * hiss) * envelope(t, 0.009, 0.0005)
        fly = t - 0.018
        whoosh = 0.0 if fly < 0.0 else 0.48 * hiss * envelope(fly, 0.025, 0.006)
        samples.append(string + snap + whoosh)
    return samples, 0.65


def reload():
    # Magazine release, insertion, then a short slide/bolt pull and latch.
    duration = 0.68
    metal = noise(duration, seed=401, cutoff=10500)
    body = noise(duration, seed=402, cutoff=1700)
    samples = []
    for i, (hiss, low) in enumerate(zip(metal, body)):
        t = i / SAMPLE_RATE
        sample = 0.0
        for offset, frequency, gain, decay in [
            (0.025, 2300.0, 0.45, 0.012),
            (0.22, 850.0, 0.65, 0.025),
            (0.49, 1850.0, 0.55, 0.014),
            (0.55, 3100.0, 0.36, 0.010),
        ]:
            click = t - offset
            if click >= 0:
                sample += gain * (0.65*hiss + 0.6*low + 0.35*math.sin(TAU*frequency*click)) * envelope(click, decay)
        drag = t - 0.38
        if 0 <= drag <= 0.11:
            sample += 0.19 * hiss * math.sin(math.pi*drag/0.11)**2
        samples.append(sample)
    return samples, 0.60


def throw():
    duration = 0.28
    air = noise(duration, seed=501, cutoff=6500)
    samples = []
    for i, hiss in enumerate(air):
        t = i / SAMPLE_RATE
        swish = hiss * math.exp(-((t - 0.10) / 0.050) ** 2)
        release = 0.13 * (hiss + math.sin(TAU * 1600 * t)) * envelope(t, 0.014)
        samples.append(swish + release)
    return samples, 0.52


def explosion():
    duration = 0.95
    air = noise(duration, seed=601, cutoff=12500)
    bass = noise(duration, seed=602, cutoff=260)
    samples = []
    for i, (crack, rumble) in enumerate(zip(air, bass)):
        t = i / SAMPLE_RATE
        phase = TAU * (36 * t + 65 * 0.035 * (1 - math.exp(-t / 0.035)))
        sample = 1.4 * crack * envelope(t, 0.025, 0.0004)
        sample += (2.4 * rumble + 0.5 * math.sin(phase)) * envelope(t, 0.17)
        sample += 0.35 * crack * envelope(max(0, t - 0.06), 0.12, 0.02)
        samples.append(sample)
    return samples, 0.78


def flashbang():
    duration = 0.62
    air = noise(duration, seed=701, cutoff=14000)
    samples = []
    for i, crack in enumerate(air):
        t = i / SAMPLE_RATE
        sample = 1.3 * crack * envelope(t, 0.020, 0.0004)
        sample += 0.20 * math.sin(TAU * 130 * t) * envelope(t, 0.045)
        # A quiet, short metallic ring distinguishes flash from a bass explosion.
        sample += 0.055 * (math.sin(TAU * 2100 * t) + 0.5 * math.sin(TAU * 2850 * t)) * envelope(t, 0.13, 0.003)
        samples.append(sample)
    return samples, 0.68


def write_wav(path, samples, peak):
    # Remove DC, gently saturate transients, and leave headroom in the PCM output.
    mean = sum(samples) / len(samples)
    samples = [math.tanh(1.4 * (sample - mean)) for sample in samples]
    fade = round(0.003 * SAMPLE_RATE)
    for i in range(fade):
        samples[i] *= i / fade
        samples[-1 - i] *= i / fade
    gain = peak / max(abs(sample) for sample in samples)
    pcm = array("h", (round(sample * gain * 32767) for sample in samples))
    if sys.byteorder != "little":
        pcm.byteswap()
    with wave.open(str(path), "wb") as output:
        output.setnchannels(1)
        output.setsampwidth(2)
        output.setframerate(SAMPLE_RATE)
        output.writeframes(pcm.tobytes())
    print(f"{path.name}: {len(samples) / SAMPLE_RATE:.2f}s, mono PCM16, {SAMPLE_RATE}Hz")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    for name, generate in [
        ("melee_attack.wav", melee_attack),
        ("gunshot.wav", gunshot),
        ("crossbow.wav", crossbow),
        ("reload.wav", reload),
        ("throw.wav", throw),
        ("explosion.wav", explosion),
        ("flashbang.wav", flashbang),
    ]:
        samples, peak = generate()
        write_wav(args.output_dir / name, samples, peak)


if __name__ == "__main__":
    main()
