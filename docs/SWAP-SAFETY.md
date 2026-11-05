# Mis-plug safety (v30 connectors)

The board has six connectors in two sizes, bundled by where the devices are in the machine. JST GH has
one key, so any lead fits any socket of its size:

| Size | Socket | Lead goes to | Pins 1 / 2 / 3 / 4 / 5 / 6 / 7 |
|---|---|---|---|
| 3-pin | **J1** 5 V in | the 5 V supply module | – / +5V / GND |
| 3-pin | **J6** pressure | the transducer | SIG / +5V / GND |
| 3-pin | **J5** thermistor | the NTC probe | GND / – / NTC |
| 7-pin | **J2** SSRs | heater SSR, pump and valve SSR boards (+5V daisy-chained) | – / +5V / heater OUT / – / – / pump OUT / valve OUT |
| 7-pin | **J7** front | the three rockers and their LEDs | PWR / BREW / STM / GND / LED P / LED C / LED S |
| 7-pin | **J10** scale | the dual HX711 board in the tray base (3.3 V only) | – / – / DOUT1 / SCK / GND / DOUT2 / 3V3 |

**Every lead in every same-size socket is harmless.** No mains load switches on. No supply is shorted. No
GPIO is overdriven or shorted. No chip gets more than a pull-up's ~0.6 mA through its protection
diodes. The firmware can't be fooled into leaving standby. You find out you swapped them because the device doesn't work, not because
something breaks. This covers multi-lead swaps too (below).

`hardware/controller/swap_check.py` checks this from the real netlist (`build/parts-120v.json`).
`controller.py` runs it on every build and fails if any case is hazardous.

## History

v29 claimed this with a simpler checker. An independent review found these hazards:
- **Front lead in J2:** power + brew rockers on joined +5V to the heater output through the lead's common.
- **Front lead in J10:** with a rocker on, LEDs sat straight across +5V and 3V3.
- **Scale lead in J7:** the power rocker read "on" and the lamp drives fed the HX711 through its diodes.
- **Pressure lead in J1:** it could be powered with its output on GND.

The old checker tested each two-pin element alone, so it never saw series paths through a lead's
floating common. v30 fixes the board, the leads and the checker.

## What v30 changed

- **Front-panel LEDs carry their own resistor** (pre-wired 5 V LEDs, BUY.md #15). An LED in the wrong
  socket is then a current-limited load wherever it lands, never a bare diode across a supply.
- **Scale from 3.3 V only.** VCC and VDD are linked at the HX711 board. There's no 5 V on J10 for a
  wrong lead to meet, and the freed pin went to the pinout.
- **Re-pinned sockets:**
  - J1 pin 1 is now empty.
  - J5 is now GND / – / NTC.
  - The heater moved from J2.1 to J2.3.
  - J10 was re-ordered.

  The pinout came from an exhaustive search: every layout of J2, J7 and J10 (420 × 420 × 1260) and every
  layout of J1, J5 and J6. The one chosen keeps the front-panel lead as it was.
- **Firmware:**
  - the scale's SCK is held high in standby, which powers the HX711s down;
  - the lamps are dark in standby, even with a fault;
  - a "start shot" sent while off is refused, never latched for when the rocker goes on;
  - OTA is only accepted with the power rocker off and the steam rocker on, so nobody on the network
    can install firmware without the standby gating.

  The heater, pump and valve were already off in standby.

A second independent review of v30 found no hardware hazard. It found the latched start, the
unauthenticated OTA, and that SCK-low was the wrong standby state; all three are fixed above.

## The model

**Board pins** are classed by net:
- **strong:** +5V, 3V3, GND, and an output drain, which is 0 V when that output is on;
- **SCK:** a push-pull GPIO;
- **medium:** lamp drives, which are a GPIO behind 330 Ω;
- **weak:** switch inputs (1k + 10k pull-up), DOUT inputs (pull-up), the pressure divider, and the NTC
  node (4.7k pull-up);
- **empty.**

DOUT, SCK, the switch inputs and the NTC node must never see more than 3.6 V from a medium or strong
source.

**Leads** are modelled as wired in WIRING.md:
- **Front:** three rockers to the common (each open or closed), and three LEDs with their resistor.
- **SSR:** the heater SSR input and two SSR boards, all from the shared +5V wire. Worst case, the boards
  and the heater input also conduct in reverse.
- **Scale:** an HX711 with protection diodes from every pin to its rail and ground.
- **Transducer:** the same diode model as the scale.
- **NTC:** a resistor.
- **Supply:** a floating 5 V source. It drives nothing until one side is tied to the board.

**States enumerated:**
- every rocker combination;
- each output on or off, independently;
- each lamp high or low, independently;
- SCK high or low;
- the board powered (from USB, or from the supply wherever it is) or unpowered.

Current is followed through floating nodes (the front lead's common, the SSRs' chained +5V), across
diodes and device inputs.

An SSR or SSR board counts as **on** when a medium or strong source (mA) puts **1.0 V or more** across it.
They're only guaranteed off below about 1 V, so the undefined region up to their 3 V turn-on counts as on.
Weak sources (pull-ups, ≤ 0.3 mA) can't turn on an SSR: the boards need 5 mA, the heater SSR a few mA.

The same weak sources can push up to ~0.6 mA through a protection diode (below). That's within what CMOS
inputs tolerate, so the checker ignores it.

`python3 swap_check.py --bare-leds` repeats every case with the front LEDs fitted *without* their own
resistor. v30 passes that too, so the LED resistor is a second layer, not the only one.

**Standby is part of the safety case.** The engine leaves standby only when the power rocker reads on.
No API, console or config command clears it. In standby the heater, pump, valve and lamps are off and
SCK is held high. At boot the rocker's debounce starts open, so the engine is in standby from its first
tick. Until then the outputs are held off by the gate pull-downs and the lamp, SCK and status pins are
low or high-Z. So when the front lead is
not in J7, the board is in standby, as long as whatever is in J7 can't pull the power input low. The
checker verifies that for every lead in J7, and analyses every lead-in-wrong-socket case with that
standby state. When the front lead *is* in J7, the rocker can be on, so the other two 7-pin leads are
analysed in every output state.

Each lead-in-socket case depends only on that lead, that socket, and the firmware and power state. That
state is enumerated or pinned as above, so a swap of two or three leads at once is covered by its
single-lead cases.

## The 7-pin group

- **SSR lead in the front socket.** Its pin 1 is empty and faces the power input, so the board stays in
  standby. The +5V wire and the heater return land on the brew and steam inputs, which can only source
  ~0.3 mA from their pull-ups. The pump and valve returns land on lamp drives, low in standby. Nothing
  can push mA through any SSR input.
- **SSR lead in the scale socket.** The +5V wire lands on the empty J10.2, so the chain floats. The
  heater return lands on DOUT1 and the pump return on DOUT2, both weak pulled-up inputs that can't sink
  current. The valve return lands on 3V3. Even if the valve board conducted backwards, the chain only
  reaches ~2.6 V with nowhere to sink, and no SSR has a path. J10's SCK and GND face the lead's empty
  pins 4 and 5.
- **Front lead in the SSR socket.** The board is in standby, so every output is off. The rockers join
  J2.1 (empty), J2.2 (+5V) and J2.3 (heater drain, off) to the common on the empty J2.4: +5V meets only
  an open drain. The LEDs face J2.5 (empty) and the two off drains.
- **Front lead in the scale socket.** The board is in standby with SCK held high. The rockers can join
  J10.1, J10.2 (empty) and DOUT1 to the common on SCK. DOUT1 is an input that's already pulled high. The
  LEDs go from GND, DOUT2 and 3V3 to the common at 3.3 V: reverse-biased or at 0 V, so none conducts.
  That holds even with bare LEDs.
- **Scale lead in the front socket.** Its pins 1–2 are empty and face the power and brew inputs, so the
  board stays in standby with the lamps low. The HX711's GND, DOUT2 and supply land on the three lamp
  drives, SCK on the panel's GND, and DOUT1 on the steam input. Everything it touches is at 0 V or weakly
  pulled up. The steam input's pull-up leaks ~0.3 mA into DOUT1's clamp; the chip stays unpowered.
- **Scale lead in the SSR socket.** Its empty pins 1–2 face J2.1 and the SSRs' +5V, so 5 V reaches
  nothing. Its other pins land on the three drains and two empty pins: only sinks, no source, so the
  HX711 is unpowered.

## The 3-pin group

- **Supply lead in the pressure socket.** Its +5V lands on the board's +5V and its GND on GND: the board
  simply runs from it. D2 still keeps it out of a USB host.
- **Supply lead in the thermistor socket.** Its +5V lands on the empty J5.2. Its GND touches only the NTC
  node, so the floating supply drives nothing.
- **Pressure lead in J1.** J1.2 is live whenever the board is powered: Q4 conducts both ways. The
  transducer's supply and ground land on +5V and GND, and its output on the empty J1.1. It just runs,
  into nothing.
- **Pressure lead in the thermistor socket.** The transducer's output lands on GND, its supply on the
  empty pin, and its ground on the NTC node. About 0.6 mA flows from R15 through the transducer's
  ground-side diode, so the NTC node sits at ~0.6 V. That reads as about 140 °C, a plausible temperature,
  not a fault. What keeps it safe is J6: it then holds no transducer (the supply lead, the thermistor or
  nothing), so pressure reads ~0 V, the PRESSURE_SENSOR fault holds, and the heater stays off.
- **Thermistor lead in J1 or the pressure socket.** It's a resistor. In J1 it goes from the empty pin to
  GND, so nothing. In J6 it goes from SIG to GND: pressure reads ~0, a sensor fault.

## What the model doesn't cover

- **Off-centre plugs.** A 3-pin plug can be forced off-centre into a 7-pin header. The latch won't
  engage, so don't.
- **Your own wiring.** The analysis assumes each lead is wired at the device end exactly as WIRING.md
  says, including the LEDs' own resistors and the scale on 3.3 V. A lead wired wrong at the device end
  isn't something any pinout can catch. Label both ends.
- **Firmware.** Safety in two swapped cases (front lead in J2, scale lead in J7) relies on the standby
  behaviour above. With standby forced off, those two are the only failures. It is covered by
  `standby_keeps_every_output_and_lamp_off` in `coffee-core`. The SCK gating and the OTA gate are in
  `firmware/esp32/src/control.rs` and `net.rs`.
- **Reset, download mode, a blank flash.** The connector-facing GPIOs are high-Z at worst and the output
  gates are pulled down. The review checked this by hand; the checker doesn't model it.
