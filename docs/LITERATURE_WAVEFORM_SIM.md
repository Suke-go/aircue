# Literature-grounded waveform simulation

This research branch replaces AirCue's ad-hoc preset library with a reproducible
simulation tool whose waveform families come from prior speaker-driven air-haptic
and synthetic-jet work.

## Why this exists

The purpose is not to claim that these electrical drive signals reproduce identical
airflow on the AirCue emitter. The purpose is to start the search from drive families
that have already been studied, then evaluate them under one common low-order model
before selecting conditions for CFD and real-world validation.

## Implemented waveform families

### AirWave (Gupta et al., UbiComp 2013)

AirWave describes driving the speaker with negative current for 50 ms and then
reversing current for 100 ms to generate the vortex.

Implemented as:

- 50 ms negative plateau
- 100 ms positive plateau

Use:

    python tools/literature_waveform_sim.py render airwave --out out/airwave.csv --wav

### SHITARA (Kojima et al., 2023)

SHITARA Eq. (7) smooths a square input using

    Y(z) = b / (1 - (b - 1) z^-1) X(z)

and reports the sweep:

    b = 1, 0.004, 0.003, 0.002, 0.001

The simulator implements the published transfer function exactly, but leaves
the square-pulse duration configurable because the b sweep is the part used as
the paper-grounded experimental variable.

Use:

    python tools/literature_waveform_sim.py shitara-sweep --out-dir out/shitara

### SynJets (Shen, Harrison, Shultz, TOCHI 2024)

The paper separates temporal patterns into:

- continuous jet
- modulated jet
- impulse jet
- single-pulse jet

The simulator implements these as waveform families. The carrier frequency is a
parameter rather than a hardcoded paper value because SynJets tunes operation around
the actuator/device resonance.

The modulated family exposes modulation rate and depth. SynJets explicitly discusses
slow modulation (<= 5 Hz) and faster 10-50 Hz patterns as perceptually different
regimes.

The impulse family uses an exponentially decaying carrier envelope, following the
paper's described impulse-jet stimulus construction.

Use:

    python tools/literature_waveform_sim.py render synjet-modulated \
      --carrier-hz 40 --modulation-hz 10 --depth 0.8 \
      --duration-ms 500 --out out/synjet_mod.csv --wav

### PWM-derived synthetic-jet family (Kordik & Travnicek, 2018)

The paper studies non-harmonic excitation and varies frequency and duty cycle to
maximize momentum flux. The implementation here is deliberately labelled a **proxy**
because the exact electrical PWM implementation should be reconstructed from the
paper before making a replication claim.

## Optional low-order actuator screening

Pass a JSON file with measured/datasheet values:

    {
      "resonance_hz": 80.0,
      "q": 1.2,
      "displacement_per_volt_mm": 0.25,
      "diaphragm_diameter_mm": 72.0,
      "port_diameter_mm": 20.0,
      "xmax_mm": 4.0,
      "speaker_count": 1
    }

Then:

    python tools/literature_waveform_sim.py render airwave \
      --actuator-config actuator.json \
      --out out/airwave.csv

The screening model is intentionally simple:

1. second-order loudspeaker displacement/velocity response;
2. area-ratio estimate of port velocity following the same continuity idea used in
   SHITARA;
3. positive slug length and L/D ("formation number") as descriptors.

It does **not** model vortex roll-up, propagation, secondary vortices, pressure at the
skin, or acoustic radiation. Those are the point at which CFD / physical validation
becomes necessary.

## Recommended research use

1. Generate the literature-grounded families.
2. Sweep only each paper's meaningful parameters:
   - SHITARA: smoothing coefficient b;
   - SynJets: carrier / modulation / envelope;
   - PWM family: frequency / duty;
   - AirWave: intake/ejection duration only when intentionally extending the paper.
3. Use the low-order model to reject actuator-limit violations and grossly implausible
   conditions.
4. Send the surviving conditions to CFD.
5. Validate a small set on the physical emitter.
6. Only after that optimize masking against application audio.

Do not use the current 12 AirCue presets as the scientific search space.

## Sources

- Gupta et al. AirWave: Non-contact Haptic Feedback Using Air Vortex Rings.
  UbiComp 2013.
- Kojima et al. SHITARA: Sending Haptic Induced Touchable Alarm by Ring-shaped
  Air vortex. 2023.
- Shen, Harrison, Shultz. Expressive, Scalable, Mid-air Haptics with Synthetic Jets.
  ACM TOCHI 2024.
- Kordik & Travnicek. Novel method for synthetic jet excitation by non-harmonic
  input waveform. International Journal of Heat and Fluid Flow, 2018.
