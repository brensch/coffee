# Architecture

```
                 Browser UI (Leptos, same build everywhere)
                          │  HTTP + WebSocket (coffee-api)
       ┌──────────────────┼──────────────────────────┐
  ESP32 firmware     coffee-engine (PC)        coffee-emu lab API
  (planned: Wi-Fi)   serves UI + API     ⇄     truth, faults, scenarios, time control
        │                  │ lockstep (coffee-io::wire, stdio/TCP/USB)
   real pins          coffee-emu ── coffee-physics ── coffee-scenario
```

## Timing contract

- The engine is ticked **once per mains half-cycle at the zero crossing** (100 Hz at 50 Hz mains). The returned `ActuatorFrame` applies to the half-cycle that is starting.
- **Heater:** a zero-cross SSR conducts for whole half-cycles. The engine turns its heater duty into half-cycle on/off decisions with a sigma-delta modulator.
- **Switching latency:** zero-cross SSRs can only turn on near 0 V, so the firmware applies the frame decided at crossing *k* about 1.5 ms before crossing *k+1*. Every command therefore takes effect one half-cycle later. The emulator models exactly this (`sim.actuator_delay_half_cycles = 1`).
- **Pump:** the vibratory pump only strokes on the positive half-cycle (it has an internal diode). Zero-cross detectors don't report polarity, so pump decisions are made per full mains cycle, on even ticks.

## Frames carry raw peripheral data

- `SensorFrame.max31855` is the chip's 32-bit word, fault bits included.
- `pressure_adc` is the raw ADS1115 count.

So sensor decoding and fault detection run identically on silicon and in emulation.

## One product, two shells

`coffee-node` holds everything with behaviour: the engine, shot recording, the profile library (built-in profiles compiled in), config, the HTTP API as `Node::handle_http(method, path, body)`, telemetry events, and the binary log and shot formats. The shells only provide plumbing:

| | PC (`coffee-engine`) | ESP32 (`firmware/esp32c6`) |
|---|---|---|
| Frames | emulator over stdio/TCP | real pins, or USB bridge |
| Storage | files under `--data` (memory if omitted) | RAM (flash backend next) |
| HTTP | axum: `/api/*` → `handle_http`; `/api/ws` streams `drain_events()` | not yet (planned: picoserve → `handle_http`) |
| Log | `--log file.clog` | streamed over USB → `coffee-emu capture` |

## History formats

- **Tick log (`.clog`, [`tlog`](../crates/coffee-node/src/tlog.rs)).** Only the hardware boundary is stored: actuator bits, raw sensor words, switches and timing. Engine-derived values are reproducible by replay, so they're left out. Ticks store only changed fields, with deltas as zig-zag varints, in self-contained 1-second chunks so captures can start mid-stream. About 2.7 B/tick.
- **Shots (`.cshot`, [`shotpack`](../crates/coffee-node/src/shotpack.rs)).** A JSON metadata header, then quantised, delta-varint columns. More than 8× smaller than JSON. The API still serves JSON to the UI.

## Three run modes, one engine

| Mode | Engine runs on | Sensors come from |
|---|---|---|
| Simulation | PC (`coffee-engine`) | `coffee-emu` over stdio or TCP |
| Hardware-in-the-loop | ESP32 (`--features bridge`) | `coffee-emu` over USB serial |
| Real machine | ESP32 | Real pins |

The emulator never links the engine; it only speaks the wire protocol.

## Engine control

- **Pressure:** a PID whose output is a *flow command* (ml/s), turned into a stroke duty by the inverse pump curve (Ulka E5 datasheet). It adds setpoint-rate feed-forward on ramps (flow needed to fill system + puck compliance) and uses back-calculation anti-windup against flow and pressure limits. Exit conditions read a slower-filtered pressure, so vibratory-pump ripple can't end a phase early.
- **Temperature is a cascade on the water, not the probe.**
  - The probe sits on the boiler wall. An observer integrates C_w·dTw/dt = h·(T_wall − Tw) − q·c_p·(Tw − T_in) to estimate the hidden water temperature. In emulation it tracks the true water within about 0.5 °C.
  - While water flows, the wall target becomes setpoint + q·c_p·ΔT/h (feed-forward) + k·(setpoint − Tw_est) (outer loop). The inner PID runs ×4 stiffer while flowing.
  - The boost fades out over the predicted last 8 s of a weight-stopped shot, so heat stored in the wall doesn't overshoot the water afterwards.
- **Ready = the water is on target** (judged on the water estimate, tolerating up to 2.5 °C hot after a shot). The engine also estimates the group/portafilter temperature. While it's cold, the UI warns how much cooler the first shot will land and offers a **warm-up flush** (`Command::Flush`: steady flow through the empty basket, no weight stop, not recorded as a shot). Set `ready_requires_heat_soak = true` to make Ready wait for the portafilter. In the emulator, brewing at water-ready costs about 5 °C at the puck; a 40 s flush recovers most of it 4 min sooner than waiting. Both numbers rest on guessed portafilter couplings (CALIBRATION.md test 9).
- **Profile exits:** sensor-based exits are held off for 0.5 s per phase, because the valve vents pressure trapped from the previous shot.
- **Yield** keeps updating for 4 s after the stop while drips land.
- **Grind advice** compares against a shot you mark as the reference.

Emulated results for the classic profile on the E24 model (`tools/iterate.sh`): true water averages 92.4 °C during extraction (target 93), never below 89; at most 94.6 °C after the shot; pressure holds 9.0 ± 0.1 bar; settled yield 36.0 g. These are only as true as the model's `thermal.*` values until those are fitted.

## API

Route tables are in [`crates/coffee-api/src/lib.rs`](../crates/coffee-api/src/lib.rs); the implementation is `Node::handle_http` in [`crates/coffee-node`](../crates/coffee-node/src/lib.rs). Telemetry streams over `/api/ws` at about 25 Hz. The lab's ground truth streams over `/lab/ws`.
