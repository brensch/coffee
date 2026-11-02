# Install

## 1. Software (your PC)

```sh
# Rust + targets
rustup target add riscv32imac-unknown-none-elf wasm32-unknown-unknown
cargo install espflash trunk          # flashing + UI bundler

# Build and check everything in emulation
cargo build --release
(cd ui && trunk build --release)
./target/release/coffee-emu test      # all scenarios should pass
./target/release/coffee-emu serve --spawn-engine   # UI at http://localhost:8080
```

On WSL, pass the ESP32's USB device through with `usbipd` (Windows) to get
`/dev/ttyACM0`.

## 2. Bench bring-up (no mains, no machine)

1. Wire the ESP32, MAX31855 + thermocouple, ADS1115 + transducer, and the switch
   inputs on the bench. Power over USB.
2. **Bridge mode (hardware-in-the-loop):**
   `cd firmware/esp32 && cargo run --release --features bridge`, then
   `coffee-emu serve --engine-serial /dev/ttyACM0`.
   The real chip now runs the real engine against the emulated machine. Pull
   shots in the UI and run the scenarios from Experiments.
3. **Machine mode on the bench:** `cargo run --release`. Check the thermocouple
   reads room temperature (and ~37 °C pinched in your fingers). Check the
   pressure reads 0 bar, and the switches toggle. The SSR LEDs should blink
   with heater demand. Capture the log
   (`coffee-emu capture … --out bench.clog`) and inspect it
   (`coffee-emu log bench.clog`).
4. Check the zero-cross path with the ZC detector on mains *in a closed box*:
   the firmware measures a 10.0 ms (50 Hz) or 8.33 ms (60 Hz) half-period.

## 3. Machine install (unplugged; get it checked if unsure)

1. Unplug, drain, let it cool. Photograph all wiring before touching it.
2. Remove the brew thermostat. Fit the M4 thermocouple **hand-tight**. Bridge
   the brew-thermostat wires (GaggiMate GCP guide, BT.A to BT.B).
3. **Keep the steam thermostat (or a 165–170 °C limiter) and thermal fuse in
   series** with the heater SSR.
4. Fit the pressure T-fitting at the pump outlet. Run the pump with the case
   open and check for leaks at room temperature *and* hot.
5. Mount the heater SSR on the chassis with thermal paste. Mount the pump and
   solenoid SSRs with snubbers.
6. Rewire the brew and steam switches as logic inputs. Mount the HLK-PM05 with
   its fuse and MOV.
7. Mount the controller box away from the boiler. Route the thermocouple and
   signal wires away from mains, using fibreglass sleeve near the boiler.
8. **First power-on:** set heater duty to zero (fault the probe, or start with
   the heater SSR disconnected). Check pressure and pump. Then enable the heater
   and watch the first heat-up live in the UI, hand on the plug.
9. Run the calibration logs ([CALIBRATION.md](CALIBRATION.md)), fit the model,
   re-run `coffee-emu test`, and update the engine config.
