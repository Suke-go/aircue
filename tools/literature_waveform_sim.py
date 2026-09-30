#!/usr/bin/env python3
"""
Literature-grounded drive-waveform generator + low-order actuator proxy for AirCue.

This tool intentionally separates:
  1) drive waveform families directly motivated by prior work, and
  2) an optional low-order loudspeaker/cavity proxy used only for screening.

It is NOT a CFD solver and it is NOT calibrated to a specific AirCue emitter.
Use --help and docs/LITERATURE_WAVEFORM_SIM.md.
"""
from __future__ import annotations

import argparse
import csv
import json
import math
import wave
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Callable, Iterable, List, Tuple

RATE = 48_000


@dataclass
class Waveform:
    name: str
    source: str
    samples: List[float]
    params: dict


@dataclass
class ActuatorConfig:
    # These parameters MUST be measured or taken from the actual driver datasheet.
    resonance_hz: float
    q: float
    displacement_per_volt_mm: float
    diaphragm_diameter_mm: float
    port_diameter_mm: float
    xmax_mm: float
    speaker_count: int = 1


def ms_samples(ms: float, rate: int = RATE) -> int:
    return max(1, int(round(ms * rate / 1000.0)))


def clamp(v: float, lo: float = -1.0, hi: float = 1.0) -> float:
    return max(lo, min(hi, v))


def normalize_peak(xs: Iterable[float], peak: float = 1.0) -> List[float]:
    xs = list(xs)
    p = max((abs(x) for x in xs), default=0.0)
    return xs if p == 0 else [x * peak / p for x in xs]


def airwave_bipolar(intake_ms: float = 50.0, eject_ms: float = 100.0) -> Waveform:
    """AirWave protocol: negative current for 50 ms, then reversed for 100 ms."""
    xs = [-1.0] * ms_samples(intake_ms) + [1.0] * ms_samples(eject_ms)
    return Waveform(
        "airwave_bipolar",
        "Gupta et al., UbiComp 2013",
        xs,
        {"intake_ms": intake_ms, "eject_ms": eject_ms},
    )


def shitara_rounded_square(
    b: float,
    on_ms: float = 20.0,
    tail_ms: float = 80.0,
) -> Waveform:
    """
    SHITARA Eq. (7), implemented from the published transfer function:
        Y(z) = b / (1 - (b - 1) z^-1) X(z)
    using a unit square as X.

    on_ms is intentionally configurable because the paper's key reproduced
    parameter is b; do not treat the default duration as a paper value.
    """
    if not (0 < b <= 1):
        raise ValueError("b must be in (0, 1].")
    x = [1.0] * ms_samples(on_ms) + [0.0] * ms_samples(tail_ms)
    y: List[float] = []
    prev = 0.0
    for xn in x:
        yn = b * xn + (b - 1.0) * prev
        y.append(yn)
        prev = yn
    return Waveform(
        f"shitara_b{b:g}",
        "Kojima et al., SHITARA 2023, Eq. (7)",
        normalize_peak(y),
        {"b": b, "on_ms": on_ms, "tail_ms": tail_ms},
    )


def synjet_continuous(carrier_hz: float, duration_ms: float = 250.0) -> Waveform:
    """Continuous synjet: carrier oscillation maintained over the stimulus."""
    n = ms_samples(duration_ms)
    xs = [math.sin(2.0 * math.pi * carrier_hz * i / RATE) for i in range(n)]
    return Waveform(
        "synjet_continuous",
        "Shen et al., TOCHI 2024, Sec. 4.3.1",
        xs,
        {"carrier_hz": carrier_hz, "duration_ms": duration_ms},
    )


def synjet_modulated(
    carrier_hz: float,
    modulation_hz: float,
    depth: float = 0.8,
    duration_ms: float = 500.0,
) -> Waveform:
    """Amplitude-modulated synjet. Paper discusses modulation from slow <=5 Hz to 10-50 Hz."""
    if not 0 <= depth <= 1:
        raise ValueError("depth must be in [0, 1].")
    n = ms_samples(duration_ms)
    xs = []
    for i in range(n):
        t = i / RATE
        env = (1.0 - depth) + depth * 0.5 * (1.0 + math.sin(2.0 * math.pi * modulation_hz * t))
        xs.append(env * math.sin(2.0 * math.pi * carrier_hz * t))
    return Waveform(
        "synjet_modulated",
        "Shen et al., TOCHI 2024, Sec. 4.3.2",
        xs,
        {
            "carrier_hz": carrier_hz,
            "modulation_hz": modulation_hz,
            "depth": depth,
            "duration_ms": duration_ms,
        },
    )


def synjet_impulse(
    carrier_hz: float,
    decay_ms: float = 70.0,
    duration_ms: float = 250.0,
) -> Waveform:
    """Impulse jet: carrier with exponentially decaying modulation (Shen et al., Sec. 7.5.3)."""
    n = ms_samples(duration_ms)
    tau = max(decay_ms / 1000.0, 1e-6)
    xs = []
    for i in range(n):
        t = i / RATE
        env = math.exp(-t / tau)
        xs.append(env * math.sin(2.0 * math.pi * carrier_hz * t))
    return Waveform(
        "synjet_impulse",
        "Shen et al., TOCHI 2024, Secs. 4.3.3 and 7.5.3",
        xs,
        {"carrier_hz": carrier_hz, "decay_ms": decay_ms, "duration_ms": duration_ms},
    )


def synjet_single_pulse(carrier_hz: float) -> Waveform:
    """Single-pulse jet: exactly one carrier oscillation."""
    duration_s = 1.0 / carrier_hz
    n = max(2, int(round(duration_s * RATE)))
    xs = [math.sin(2.0 * math.pi * i / n) for i in range(n)]
    return Waveform(
        "synjet_single_pulse",
        "Shen et al., TOCHI 2024, Sec. 4.3.4",
        xs,
        {"carrier_hz": carrier_hz},
    )


def pwm_like_synthetic_jet(
    frequency_hz: float,
    duty: float,
    duration_ms: float = 250.0,
) -> Waveform:
    """
    Zero-mean two-level pulse-width-like excitation for exploratory comparison.

    Kordik & Travnicek (2018) varied frequency and duty cycle of a PWM-derived
    voltage waveform to maximize synthetic-jet momentum flux. The exact electrical
    implementation should be checked against the paper before claiming replication;
    this function is a parameterized proxy, not an exact reproduction.
    """
    if not 0.05 <= duty <= 0.95:
        raise ValueError("duty must be between 0.05 and 0.95.")
    n = ms_samples(duration_ms)
    period = RATE / frequency_hz
    # Negative level chosen so each ideal period has approximately zero DC.
    low = -duty / (1.0 - duty)
    xs = []
    for i in range(n):
        phase = (i % period) / period
        xs.append(1.0 if phase < duty else low)
    return Waveform(
        "pwm_like_synthetic_jet",
        "Kordik & Travnicek, IJHFF 2018 (proxy family)",
        normalize_peak(xs),
        {"frequency_hz": frequency_hz, "duty": duty, "duration_ms": duration_ms},
    )


FAMILIES: dict[str, Callable[..., Waveform]] = {
    "airwave": airwave_bipolar,
    "shitara": shitara_rounded_square,
    "synjet-continuous": synjet_continuous,
    "synjet-modulated": synjet_modulated,
    "synjet-impulse": synjet_impulse,
    "synjet-single": synjet_single_pulse,
    "pwm-proxy": pwm_like_synthetic_jet,
}


def simulate_actuator(w: Waveform, cfg: ActuatorConfig) -> dict:
    """
    Low-order screening model.

    Loudspeaker: normalized 2nd-order mass-spring-damper response.
    Port-flow proxy: SHITARA-style area-ratio relation,
        U_exit ~= n * (D_diaphragm / D_port)^2 * U_piston.

    The gain is calibrated by displacement_per_volt_mm. This is only a screening
    model; vortex formation, separation, propagation, and acoustic radiation are
    not solved.
    """
    if cfg.resonance_hz <= 0 or cfg.q <= 0:
        raise ValueError("resonance_hz and q must be > 0.")
    w0 = 2.0 * math.pi * cfg.resonance_hz
    dt = 1.0 / RATE
    # Choose forcing gain so quasi-static displacement is displacement_per_volt_mm.
    static_m_per_v = cfg.displacement_per_volt_mm / 1000.0
    forcing = (w0 * w0) * static_m_per_v
    damping = w0 / cfg.q

    x = 0.0
    v = 0.0
    xpos: List[float] = []
    vel: List[float] = []
    exit_vel: List[float] = []

    area_ratio = cfg.speaker_count * (
        cfg.diaphragm_diameter_mm / cfg.port_diameter_mm
    ) ** 2

    def acc(x_: float, v_: float, u_: float) -> float:
        return forcing * u_ - damping * v_ - (w0 * w0) * x_

    for u in w.samples:
        # RK4 integration of x'=v, v'=a(x,v,u), holding u over one audio sample.
        k1x, k1v = v, acc(x, v, u)
        k2x = v + 0.5 * dt * k1v
        k2v = acc(x + 0.5 * dt * k1x, v + 0.5 * dt * k1v, u)
        k3x = v + 0.5 * dt * k2v
        k3v = acc(x + 0.5 * dt * k2x, v + 0.5 * dt * k2v, u)
        k4x = v + dt * k3v
        k4v = acc(x + dt * k3x, v + dt * k3v, u)
        x += dt * (k1x + 2*k2x + 2*k3x + k4x) / 6.0
        v += dt * (k1v + 2*k2v + 2*k3v + k4v) / 6.0
        xpos.append(x)
        vel.append(v)
        exit_vel.append(area_ratio * v)

    xmax = cfg.xmax_mm / 1000.0
    positive_slug_m = sum(max(u, 0.0) * dt for u in exit_vel)
    formation_number = positive_slug_m / (cfg.port_diameter_mm / 1000.0)
    return {
        "peak_displacement_mm": max((abs(z) for z in xpos), default=0.0) * 1000.0,
        "xmax_exceeded": max((abs(z) for z in xpos), default=0.0) > xmax,
        "peak_piston_velocity_m_s": max((abs(z) for z in vel), default=0.0),
        "peak_exit_velocity_proxy_m_s": max((abs(z) for z in exit_vel), default=0.0),
        "positive_slug_length_m": positive_slug_m,
        "formation_number_proxy": formation_number,
        "duration_ms": len(w.samples) * 1000.0 / RATE,
    }


def signal_descriptors(xs: List[float]) -> dict:
    if not xs:
        return {}
    peak = max(abs(x) for x in xs)
    rms = math.sqrt(sum(x*x for x in xs) / len(xs))
    deriv = [xs[i] - xs[i-1] for i in range(1, len(xs))]
    deriv_rms = math.sqrt(sum(x*x for x in deriv) / max(1, len(deriv)))
    return {
        "peak": peak,
        "rms": rms,
        "crest_factor": peak / rms if rms else 0.0,
        "signed_integral_samples": sum(xs),
        "abs_derivative_rms": deriv_rms,
    }


def write_csv(path: Path, w: Waveform, sim: dict | None = None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as f:
        wr = csv.writer(f)
        wr.writerow(["time_s", "drive"])
        for i, x in enumerate(w.samples):
            wr.writerow([i / RATE, x])
    meta = {
        "name": w.name,
        "source": w.source,
        "params": w.params,
        "signal_descriptors": signal_descriptors(w.samples),
        "simulation": sim,
    }
    path.with_suffix(".json").write_text(json.dumps(meta, indent=2), encoding="utf-8")


def write_wav(path: Path, w: Waveform) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    xs = normalize_peak(w.samples, 0.95)
    with wave.open(str(path), "wb") as wf:
        wf.setnchannels(1)
        wf.setsampwidth(2)
        wf.setframerate(RATE)
        frames = bytearray()
        for x in xs:
            q = int(round(clamp(x) * 32767))
            frames.extend(q.to_bytes(2, byteorder="little", signed=True))
        wf.writeframes(bytes(frames))


def load_cfg(path: Path) -> ActuatorConfig:
    raw = json.loads(path.read_text(encoding="utf-8"))
    return ActuatorConfig(**raw)


def build_from_args(a: argparse.Namespace) -> Waveform:
    if a.family == "airwave":
        return airwave_bipolar(a.intake_ms, a.eject_ms)
    if a.family == "shitara":
        return shitara_rounded_square(a.b, a.on_ms, a.tail_ms)
    if a.family == "synjet-continuous":
        return synjet_continuous(a.carrier_hz, a.duration_ms)
    if a.family == "synjet-modulated":
        return synjet_modulated(a.carrier_hz, a.modulation_hz, a.depth, a.duration_ms)
    if a.family == "synjet-impulse":
        return synjet_impulse(a.carrier_hz, a.decay_ms, a.duration_ms)
    if a.family == "synjet-single":
        return synjet_single_pulse(a.carrier_hz)
    if a.family == "pwm-proxy":
        return pwm_like_synthetic_jet(a.frequency_hz, a.duty, a.duration_ms)
    raise ValueError(a.family)


def add_family_args(p: argparse.ArgumentParser) -> None:
    p.add_argument("family", choices=sorted(FAMILIES))
    p.add_argument("--intake-ms", type=float, default=50.0)
    p.add_argument("--eject-ms", type=float, default=100.0)
    p.add_argument("--b", type=float, default=0.004)
    p.add_argument("--on-ms", type=float, default=20.0)
    p.add_argument("--tail-ms", type=float, default=80.0)
    p.add_argument("--carrier-hz", type=float, default=40.0)
    p.add_argument("--modulation-hz", type=float, default=10.0)
    p.add_argument("--depth", type=float, default=0.8)
    p.add_argument("--decay-ms", type=float, default=70.0)
    p.add_argument("--frequency-hz", type=float, default=40.0)
    p.add_argument("--duty", type=float, default=0.5)
    p.add_argument("--duration-ms", type=float, default=250.0)


def main() -> None:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)

    r = sub.add_parser("render", help="Render one literature-grounded waveform.")
    add_family_args(r)
    r.add_argument("--out", type=Path, required=True, help="CSV output path.")
    r.add_argument("--wav", action="store_true")
    r.add_argument("--actuator-config", type=Path)

    m = sub.add_parser("manifest", help="Print implemented waveform families.")
    s = sub.add_parser("shitara-sweep", help="Generate the b values used in SHITARA Fig. 5.")
    s.add_argument("--out-dir", type=Path, required=True)
    s.add_argument("--on-ms", type=float, default=20.0)
    s.add_argument("--tail-ms", type=float, default=80.0)
    s.add_argument("--actuator-config", type=Path)

    args = ap.parse_args()
    if args.cmd == "manifest":
        print(json.dumps({
            "families": {
                "airwave": "50 ms intake + 100 ms reversed ejection (paper protocol)",
                "shitara": "Eq. 7 rounded square; b sweep {0.001,0.002,0.003,0.004,1}",
                "synjet-continuous": "continuous carrier",
                "synjet-modulated": "AM carrier; modulation regimes discussed in paper",
                "synjet-impulse": "exponentially decaying carrier envelope",
                "synjet-single": "one carrier oscillation",
                "pwm-proxy": "PWM-derived family inspired by Kordik & Travnicek 2018",
            }
        }, indent=2))
        return

    cfg = load_cfg(args.actuator_config) if getattr(args, "actuator_config", None) else None

    if args.cmd == "render":
        w = build_from_args(args)
        sim = simulate_actuator(w, cfg) if cfg else None
        write_csv(args.out, w, sim)
        if args.wav:
            write_wav(args.out.with_suffix(".wav"), w)
        print(json.dumps({"waveform": w.name, "csv": str(args.out), "simulation": sim}, indent=2))
        return

    if args.cmd == "shitara-sweep":
        args.out_dir.mkdir(parents=True, exist_ok=True)
        rows = []
        for b in [0.001, 0.002, 0.003, 0.004, 1.0]:
            w = shitara_rounded_square(b, args.on_ms, args.tail_ms)
            sim = simulate_actuator(w, cfg) if cfg else None
            out = args.out_dir / f"shitara_b{b:g}.csv"
            write_csv(out, w, sim)
            rows.append({"b": b, **signal_descriptors(w.samples), **(sim or {})})
        (args.out_dir / "summary.json").write_text(json.dumps(rows, indent=2), encoding="utf-8")
        print(json.dumps(rows, indent=2))


if __name__ == "__main__":
    main()
