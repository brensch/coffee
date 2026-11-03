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

- The engine is ticked **once per mains cycle** (60 Hz in the US, `EngineConfig::mains_hz`), timed by the ESP32-C6's crystal. There is no zero-cross detector. The control period is `dt = 1 / mains_hz`.
- **Zero-crossing switching happens in hardware.** Every load (heater, pump, valve) is driven by a MOC3063 zero-cross opto-triac, which fires its triac only at a zero crossing. An output raised anywhere in a cycle takes effect at the next crossing, and an output dropped stops at the next current zero. So a load asserted for one crystal window conducts for exactly one full mains cycle, whatever the window's phase against the grid.
- **Switching latency:** the frame for cycle *k* (sensors averaged over that cycle) is ticked at the boundary, and the firmware applies the decision at once, for cycle *k+1*. Lockstep emulation models exactly that, so `sim.actuator_delay_cycles` is 0 (it adds *extra* delay for experiments).
- **Heater:** burst firing. The heater duty goes through a sigma-delta modulator into whole-cycle on/off decisions.
- **Pump:** the vibratory pump strokes only on the positive half wave (it has an internal diode), and a one-period window always holds exactly one positive-going crossing. So "pump on for one cycle" is **one stroke**. Pump duty is the fraction of cycles with a stroke. The flow estimate is the stroke fraction × the pump curve at the measured pressure, and pumped volume adds `curve(P) / mains_hz` per stroke.
- **Off-nominal grid:** if the grid is not exactly at `mains_hz`, the crystal window slips against it by one crossing per beat period (1/|Δf|; 20 s at ±0.05 Hz). The window where that happens gives 0 or 2 strokes, so the stroke count drifts by at most one per beat period (<0.2 %; `coffee_board::grid` tests). Running the pump continuously delivers exactly the grid's positive half waves.

## Frames carry calibrated millivolts

- `SensorFrame.ntc_mv`: the NTC divider node (100k B3950 to GND, 8.2k 0.1 % from 3.3 V). It is the ADC1 reading averaged over 32 evenly spaced samples in the cycle, curve-calibrated from eFuse.
- `SensorFrame.pressure_mv`: the transducer after its 10k/20k divider, averaged the same way. A uniform average over exactly one mains period removes the pump's pressure ripple.
- Switches arrive debounced (stable for 3 cycles) in `inputs`.

The NTC and pressure models, fault detection (open is the node at the rail, a short is the node at ground, and a disconnected transducer reads below 200 mV) and calibration all live in `coffee-core::sensors`. They run identically on silicon and in emulation. The pure firmware logic (cycle clock, oversampling, debounce, status LED, grid model) is in `coffee-board`, which is unit-tested on the host.

## One product, two shells

`coffee-node` holds everything with behaviour: the engine, shot recording, the profile library (built-in profiles compiled in), config, the HTTP API as `Node::handle_http(method, path, body)`, telemetry events, and the binary log and shot formats. The shells only provide plumbing:

| | PC (`coffee-engine`) | ESP32 (`firmware/esp32`) |
|---|---|---|
| Frames | emulator over stdio/TCP | real pins, or USB bridge |
| Storage | files under `--data` (memory if omitted) | flash key/value store; writes deferred while brewing |
| HTTP | axum: `/api/*` → `handle_http`; `/api/ws` streams `drain_events()` | queued to the control task → `prepare_http`; encoded by the HTTP worker; `/api/ws` streams events |
| Log | `--log file.clog` | streamed over USB → `coffee-emu capture` |

### Firmware ownership: the control task owns the node

On the ESP32 the control task (high-priority interrupt executor) owns the `Node` by value. Nothing else holds it and there is no lock, so the network can never delay a tick. The control task awaits only its own timers: the ADC sample points and the cycle tick.

- **Requests in** (`coffee_board::link`). HTTP workers and the USB console queue `(method, path, body)` into a bounded channel (8 deep) and wait on their own reply slot, with a 1.5 s timeout.
  - A full queue is answered 503 straight away, and a missed reply 504.
  - Replies carry a sequence number, so a late reply can't be mistaken for the next one.
- **Handled after the outputs.** Once per cycle, after the outputs are applied, the control task handles at most 2 requests, and only while it's within 1 ms of starting (the check runs before each request). The handler is `Node::prepare_http`, which routes the request and makes owned copies (history columns, shot list, a decoded shot, the profile library). The JSON encoding (`PreparedHttp::finish`) runs in the HTTP worker.
- **State out.** Node events go to a pub/sub channel that never blocks the publisher (WebSocket clients lag rather than block). The latest telemetry is a snapshot (`link::Latest`) for `/api/device`, the USB `status` command and the first WebSocket message.
- **Flash.** The control task never touches flash: node saves go to the store's RAM map, and a background task writes them out only while no shot is running.

## History formats

- **Tick log (`.clog` v2, [`tlog`](../crates/coffee-node/src/tlog.rs)).** Only the hardware boundary is stored: actuator bits, sensor millivolts, switches and timing. Engine-derived values are reproducible by replay, so they're left out.
  - Ticks store only changed fields, with deltas as zig-zag varints.
  - Timestamps are predicted from the crystal grid, so on-grid ticks cost nothing.
  - Chunks are self-contained and 1 s long, so a capture can start mid-stream.
  - A steady tick is 1 byte; a noisy one 2–3.
- **Shots (`.cshot`, [`shotpack`](../crates/coffee-node/src/shotpack.rs)).** A JSON metadata header, then quantised, delta-varint columns. More than 8× smaller than JSON. The API still serves JSON to the UI.

## Three run modes, one engine

| Mode | Engine runs on | Sensors come from |
|---|---|---|
| Simulation | PC (`coffee-engine`) | `coffee-emu` over stdio or TCP |
| Hardware-in-the-loop | ESP32 (`--features bridge`) | `coffee-emu` over USB serial (the `Hello` carries the mains frequency) |
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
