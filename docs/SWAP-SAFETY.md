# Mis-plug safety (v29 connectors)

> **Review result (2026-11-05): the claim below is wrong for 3 of the 12 single swaps.** The checker
> tests each two-pin element on its own, so it misses series paths through the front lead's floating
> common (all switch returns and LED cathodes):
> - **Front lead in J2:** power + brew rockers on join J2.2 (+5V) to J2.1 (heater drain) through the
>   common. When the heater output turns on, +5V is shorted to GND through Q1.
> - **Front lead in J10:** with the power rocker on, the common sits at GND. The coffee and steam LEDs
>   then sit straight across +5V and 3V3 with no resistor and burn. With the steam rocker on, the LEDs
>   sink into SCK (GPIO17).
> - **Scale lead in J7:** DOUT1 lands on GND. IO14 reads about 0.5 V, i.e. the power rocker reads ON, and
>   the lamp GPIOs feed the HX711 through its protection diodes.
> - **Pressure lead in J1 is live:** when the board is powered from USB or from J6, Q4 conducts back to
>   J1.2, powering the transducer with its output shorted to GND.
>
> Also, `swap_check.py` is not yet run by the build. Until the pinout is fixed: **label both ends of
> every lead.** The text below is the original (flawed) analysis.

The board has six connectors in two sizes, bundled by where the devices are in the machine:

| Size | Socket | Lead goes to | Pins 1 / 2 / 3 / 4 / 5 / 6 / 7 |
|---|---|---|---|
| 3-pin | **J1** 5 V in | the 5 V supply module | GND / +5V / GND |
| 3-pin | **J6** pressure | the transducer | SIG / +5V / GND |
| 3-pin | **J5** thermistor | the NTC probe | NTC / – / GND |
| 7-pin | **J2** SSRs | the back panel: heater SSR, pump and valve SSR boards | heater OUT / +5V / – / – / – / pump OUT / valve OUT |
| 7-pin | **J7** front | the three rockers and their LEDs | PWR / BREW / STM / GND / LED P / LED C / LED S |
| 7-pin | **J10** scale | the dual HX711 board in the tray base | GND / – / SCK / DOUT1 / DOUT2 / +5V / 3V3 |

Same-size leads fit each other's sockets: JST GH has one key. The pinouts are chosen so that **every
lead in every wrong same-size socket is harmless**: no mains load switches on, no supply is shorted,
no board pin is overdriven, no LED runs without its resistor, no HX711 is powered backwards. You
find out you swapped them because the device doesn't work, not because something breaks.

`hardware/controller/swap_check.py` checks this from the real netlist (`build/parts-120v.json`) on
every build and exits 1 if any combination is hazardous. Current result: all 12 combinations harmless.

## The model

Each socket pin is classed by what the board puts on it:
- **strong supplies:** +5V, 3V3;
- **strong sinks:** GND, and an output transistor's drain, which is low whenever that output is on;
- **LED drive:** a GPIO through 330 Ω, 0 or 3.3 V, up to ~10 mA;
- **switch input:** 1k in series, then a 10k pull-up to 3.3 V, so it can only source ~0.3 mA;
- **direct GPIO:** the scale's SCK (an output) and DOUT1/2 (inputs), with no series resistor, so 5 V must never reach them;
- **empty**.

Each lead is modelled by the device at its end:
- **heater SSR input:** switches on from 3 V at a few mA;
- **SSR boards:** VCC and IN tied together on +5V, GND on the drain; they need about 4 V;
- **rocker switch** to the panel's common;
- **LED,** anode to cathode, with no resistor in the lead;
- **HX711 board:** must only ever be powered the right way round, from its ground on a solid ground;
- **5 V supply:** an external source;
- **transducer and NTC.**

## The 7-pin group

- **SSR lead in the front socket.** The SSR inputs land on the switch inputs (pins 1, 2, 3) and LED drives (6, 7). The strongest thing any of them can see is 3.3 V through 330 Ω or ~0.3 mA from a pull-up. The heater SSR needs mA at ≥ 3 V across *its* pair, which lands on two switch inputs (~0 V between them). The SSR boards need ~4 V. Nothing switches on.
- **SSR lead in the scale socket.** The lead's shared +5V wire (pin 2) lands on the scale socket's empty pin 2. With no supply on that wire, no SSR input can be powered.
- **Front lead in the SSR socket.** The front panel's common (pin 4) lands on the SSR socket's empty pin 4. Every switch and LED in that lead closes through the common, so none of them completes a circuit.
- **Front lead in the scale socket.** The common lands on DOUT1, a GPIO input. The switches connect it to GND (pin 1), to nothing (pin 2) or to SCK (pin 3), all logic-level. The LED anodes land on DOUT2, +5V and 3V3. At most an LED from +5V into a GPIO input: 5 V minus the LED's ~2 V is under the pin's 3.6 V limit, and there's no return path for real current.
- **Scale lead in the SSR socket.** The scale lead's empty wire (pin 2) is the only one facing the SSR socket's +5V. Every other HX711 pin lands on a drain or an empty pin, so the HX711 gets no supply at all.
- **Scale lead in the front socket.** The HX711's pins land on switch inputs and LED drives: weak, current-limited, ≤ 3.3 V. Nothing is overdriven.

The single shared +5V pin on the SSR socket is deliberate. With two +5V pins, at least one HX711 pin always ended up on +5V while another sat on a drain. That puts current through the chip's protection diodes into the output transistor whenever that output was on, and it was the one combination that failed.

## The 3-pin group

- **Supply lead in the pressure socket.** The supply's +5V lands on the board's +5V (pin 2): the board simply runs from it. D2 still keeps it out of a USB host. Its grounds land on SIG (behind 47k) and GND.
- **Supply lead in the thermistor socket.** The supply's +5V lands on the empty pin 2, so nothing is powered.
- **Pressure lead in J1.** The transducer's +5V wire lands on the board's power input, with no supply on it; its signal lands on GND. Unpowered, no harm.
- **Pressure lead in the thermistor socket.** The transducer is unpowered. Its signal wire loads the NTC node, so the temperature reads wrong, the plausibility checks fault, and the heater stays off.
- **NTC lead in J1 or the pressure socket.** It's a 50k resistor, harmless anywhere (0.1 mA at 5 V). In the pressure socket the pressure reads low and the board faults on it.

## What the model doesn't cover

- **Off-centre plugs.** A 3-pin plug can be forced off-centre into a 7-pin header. The latch won't engage, but don't do it.
- **Reverse-biased LEDs.** These are within their 5 V rating in every case above.
- **Your own wiring.** The analysis assumes each lead is wired to its device the way WIRING.md says. A lead wired wrong at the device end isn't something any pinout can catch. Label both ends.
