# Independent electrical review of v49, and what was done

An independent reviewer (a fresh agent given only the repo, 2026-11-11) reviewed the v49 circuit (the routed v45 board
with the 800 V valve triac and 200 Ω opto resistors), its firmware agreement and the wiring docs. It re-derived pinouts
and ratings from the makers' datasheets where it could fetch them, and modified nothing.

**Verdict: nothing needs a layout change and nothing blocks the board order.** One electrical risk can't be closed on
paper (S1) and has a same-footprint fallback. The only must-fix was in the docs.

## Must-fix

| # | Finding | Done |
|---|---|---|
| M1 | INSTALL §1 and BUY pointed at the v33 order files (still in `build/`), with the v33 board size and part count | Fixed: both point at `coffee-v49-*` |

## Should-fix

| # | Finding | Done |
|---|---|---|
| S1 | **Zero-cross optos re-firing into inductive loads is unproven.** `sim/run.py` models each triac as an ideal switch: no latching current, no opto drop-out, no inhibit window. Each time a triac drops out below its latching current the opto has to re-trigger before the voltage across it passes the opto's inhibit level (12 V typ, no minimum). At worst-case datasheet values that leaves about 6 µs on the valve and 2–3 µs on the pump; typical parts have several times more. The same arrangement works inside the G3MB-202P relays used on these loads. If it fails, the valve runs on half-waves (buzz, hot coil) or the pump skips strokes | Open: two first-article tests added to CIRCUIT.md ("What needs a human"). Fallbacks, both same footprint: a random-phase 800 V opto for U9; a T410-800B for Q5 |
| S2 | Firmware: flash writes run inside one critical section against the 250 ms watchdog (`store.rs`, `control.rs`). A slow erase resets the chip mid-write. Outputs fall off, so it is safe, but it looks like random reboots during Wi-Fi setup, shot save or OTA | Open |
| S3 | WIRING said USB may be plugged in with mains on; INSTALL says never | Fixed: WIRING now gives INSTALL's rule |
| S4 | The US machine has two white wires on the steam switch; WIRING says where one goes. Partly inferred: the stock diagram is an image the reviewer couldn't read | Open: check on the machine |
| S5 | WIRING check 4 expected "open" from L to P, where R54 reads 51 kΩ | Fixed: 51 kΩ expected (also proves the fuse); leakage figures corrected |
| S6 | Thermocouple colour guidance (ANSI/IEC) doesn't match cheap M4 probes | Fixed: magnet and warm-tip test, not colour |
| S7 | INSTALL's OTA arming step used the power rocker, which has cut the board's mains since v35 | Fixed: steam then brew, held 3 s, as the firmware does |

## Notes, left as they are

- **Fuse and triac:** a pump or valve wire shorted to the chassis while on kills the triac before F1 opens, and a bolted
  fault can exceed F1's 50 A breaking rating; the house breaker backs it up. Accepted, as in REVIEW-v32.
- **Pump gate has no gate resistor:** a sensitive T435 could fire once mid-cycle when the opto LED turns on off-zero. A
  partial extra stroke; harmless.
- **I2C recovery:** a watchdog reset mid-read can leave the MCP3421 holding SDA low until a power cycle. A software
  timeout and nine clock pulses at start-up would clear it.
- **Shorted thermocouple** reads as board temperature and looks valid; the stock 145 °C thermostat carries that fault,
  as designed.
- **Temperature headroom:** the opto and the supply are fine at 70 °C and have no margin above about 85 °C.
- **Accuracy:** thermocouple offset up to about 1 °C before calibration; pressure about 0.18 bar before the power-up zero.
- **Stale text:** CIRCUIT.md's sensor table still shows 7-pin J7/J10 and GPIO14; SWAP-SAFETY.md argues from the removed
  standby input; `docs/wiring.svg` shows v30; BUY lists no insulated male tabs for the SSR leads.

## Checked and found correct

- **Pinouts against datasheets:** MOC3083, T4 triacs (the 800 V part has the 600 V part's gate, holding and dV/dt
  figures), IRM-10-5 (geometry and DC polarity), TLV1117LV33, MCP3421, TMP117, ESP32-C6-MINI-1U (all 53 pads), B5819W.
- **Board against netlist:** every pad net of all 82 parts matches.
- **Reset behaviour:** outputs sit at 0.225 V against the FETs' 0.65 V minimum threshold; strap pins boot correctly.
- **Firmware:** pin map, output polarity, I2C addresses and ADC settings match the board; the watchdog resets a hang;
  an open probe faults in about 1.1 s.
- **Order files:** the BOM differs from v45 only in Q6 and R49/R50; the placement file is identical.

## Could not verify

- The real valve and pump coil inductances and the opto's turn-on time (S1 depends on them).
- The Littelfuse, Murata and onsemi AN-3004 documents (downloads refused); those ratings rest on the evidence store.
- Pin numbering on the IRM and DPAK drawings, JST GH pin-1 handedness against ready-made leads, and part rotations in
  JLC's placement preview.
