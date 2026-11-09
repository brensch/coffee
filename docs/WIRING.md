# Wiring: v39 board in its holder + heater SSR (US Gaggia Classic Pro, 120 V)

![wiring diagram](wiring.svg)

This replaces INSTALL.md §5. **The board carries mains for the small loads only**: its four 6.3 mm spade
tabs (along its top edge) take live and neutral in and switch the pump and the 3-way valve with on-board
triacs; its own certified supply (Mean Well IRM-10-5) makes the 5 V. It sits in the printed holder
(hardware/holder/README.md): mains wires leave straight out under its roof, low-voltage leads out of the bottom. The 11 A heater stays on the external SSR on the
rear panel, driven from the board's J2. Parts and links are in [BUY.md](BUY.md). INSTALL.md §4
(sensors, plumbing) and §6–§7 still apply. *(The diagram above still shows the v30 module layout.)*

> ⚠️ **Mains voltage.** Work with the machine unplugged, and cooled if it has been on. Photograph
> the stock wiring before you touch it. The thermal fuse and steam thermostat stay in series with
> the heater: they're the protection that doesn't depend on software. **The board's top half (spades,
> fuse, triacs, the supply's AC end, and their solder joints on the back) is live whenever the machine
> is plugged in with the power rocker on**: it lives in its holder behind the cover, never against
> the chassis, and you never touch it plugged in. If you aren't comfortable with mains wiring, have this checked by an electrician before
> first power-on.

## Sources and confidence

- Machine wire names (C.4, S.5, PMP.B, BT, ST) and colours come from GaggiMate's Classic Pro guide,
  <https://docs.gaggimate.eu/docs/installation-gcp> (opened 2026-11-04), and its 100/120 V diagram
  "SIN035R 100/120V". On that US diagram:
  - the brew thermostat (107 °C) has a **white** and a **black** wire;
  - the steam thermostat (145 °C) has a **grey** and a **black** wire;
  - the thermal fuse is 184 °C;
  - the pump's two wires are **white** and **grey** (grey = PMP.B, the switched live);
  - the brew-switch line feed is **C.4 brown**, and the steam switch carries **S.5 white** and **S.4 black**.
- **US steam-switch difference** (GaggiMate, §4): *"On a US machine the double white will be on
  the other side. Follow the labeling and modifications of the switch positions regardless of
  that!"* So go by GaggiMate's position labels (S.1–S.5), not by where the white wires sit.
- GaggiMate's heater method differs from ours, but the circuit is equivalent:
  - **GaggiMate:** bridges the two brew-thermostat wires and puts its SSR in the steam-thermostat wires.
  - **This build:** puts the SSR in the brew-thermostat wires and leaves the steam thermostat and thermal fuse untouched.
- Items marked **UNSURE** are not confirmed for a US machine. Check each with a meter, machine unplugged.

## What changes in the machine

1. **Brew thermostat**
   - Unscrew it from the left side of the boiler.
   - Keep its two spade wires (white, black). They go to the heater SSR's load terminals, so the SSR takes the thermostat's place.
   - The M4 thermocouple screws into the thermostat's hole (INSTALL.md §4).
2. **Steam thermostat and thermal fuse:** leave them, and their wiring, exactly as they are.
3. **Live (L):** unplug **C.4 (brown)** from the brew switch. It becomes the board's live (**J13 L**): its female receptacle plugs straight onto the tab. GaggiMate uses the same wire as its board's L.
   Confirm it **unplugged**, with the meter on Ω: the plug's **narrow blade (live)** to C.4 reads continuity with the power switch ON and open with it OFF.
4. **Neutral (N):** unplug **S.5 (white)** from the steam switch. It becomes the board's neutral (**J12 N**, for the on-board supply only). GaggiMate uses the same wire as its board's N.
   Confirm it unplugged: the plug's **wide blade (neutral)** to S.5 reads continuity, and S.5 to the earth pin reads open.
5. **Pump**
   - Unplug the **grey PMP.B** wire from the pump: the one that does *not* go to the pump's thermal fuse.
   - A new 18 AWG lead (female receptacle both ends) goes from the board's **J14 P** to that pump tab.
   - The pump's white wire stays where it is.
6. **Valve: identify its live wire before you park anything (step 7).**
   - Machine unplugged. With the meter on Ω, find the wire that reads through the valve coil to the valve's neutral: a few hundred Ω to a few kΩ.
   - In GaggiMate's build that is the free end of the grey **PMP.B** wire (it goes to their "V" output). On some machines the valve feed instead comes off a **brew-switch terminal**. If so, that switch wire is the valve live: **don't park it in step 7.**
   - Label it now. It goes onto the board's **J15 3** (its female receptacle fits the tab directly).
   - The valve neutral stays where it is.
   - If no wire shows the coil's resistance, stop: don't guess. Photograph the harness and trace the valve's two wires back from the valve body.
7. **Brew and steam switches** (after step 6)
   - Remove every mains wire from both switches, **except** that you already took the valve live labelled in step 6 to the board's J15.
   - Insulate each freed end (they already have insulated receptacles; add heat-shrink over any that don't) and tie them back.
   - The switches then carry only the 5 V J7 signals, and the switch lamps go dark.
   - GaggiMate moves S.4 (black) to the S.1 position. In this build S.4 is parked insulated, **not** moved, because the steam switch must not bypass anything: the board owns the heater.
   - **UNSURE:** whether S.4 is part of the steam-thermostat path on a US machine. If parking it breaks the heater circuit, the heater simply won't heat (fail-safe), and the "heater circuit" check below catches it.

8. **Power switch: leave it exactly as it is.** It switches mains to the whole machine, the controller
   included (since v35): rocker off means everything is off, the board too. Don't move any of its wires.
   Over-the-air firmware updates are armed from the rockers instead (steam on, then brew held 3 s), so
   nobody on the network can replace the firmware.

9. **Front-panel lamps:** unplug the three 120 V lamps (power, coffee temp, steam temp) from the harness, insulate and tie back their freed wires, and fit the red LEDs in the lamp positions (BUY.md #15).

## Terminal-by-terminal table

### Mains side

**18 AWG silicone** for every mains run you add, **fully insulated 6.3 mm receptacles** onto the
board's tabs and the machine's tabs, **forks** on the heater SSR's screw terminals. The board's tabs
are labelled on the silkscreen: **3, P, L, N** left to right along its top edge.

| # | From | To | Ends | Notes |
|---|---|---|---|---|
| M1 | C.4 brown (female receptacle, ex brew switch) | board **J13 L** | its own receptacle | Live, switched by the power rocker. Feeds the board's 2 A fuse, then the supply, pump and valve triacs |
| M2 | S.5 white (female receptacle, ex steam switch) | board **J12 N** | its own receptacle | Neutral, for the on-board supply only |
| M3 | board **J14 P** | pump live tab (where grey PMP.B was) | female / female | The pump's other wire (white) stays as it is |
| M4 | grey PMP.B free end (the valve live, step 6) | board **J15 3** | its own receptacle | The valve neutral stays as it is |
| M5 | brew-thermostat wire 1 (white, female) | heater SSR **1/L1** | male spade / fork | The SSR replaces the thermostat |
| M6 | brew-thermostat wire 2 (black, female) | heater SSR **2/T1** | male spade / fork | |
| — | pump white, valve neutral, steam thermostat, thermal fuse, heater, earth | unchanged | — | Don't touch |

Mechanical:
- **The board in its holder** (hardware/holder/README.md): the frame sticks to the side panel with six magnets;
  the board slides up into it from below. Push the four receptacles straight onto the board's spades (the channel
  over the board's top edge takes the push), then put the cover on (its fingers hold the board up) and lock it with
  the two bars that slide across. The four mains wires leave straight out under the roof, away from the low-voltage
  leads below. The board never touches the chassis.
- **Heater SSR:** flat on the inside of the rear panel at the ventilation slots, two M4 bolts with nuts and washers through the slots (or cable ties through them); its factory thermal pad against the panel. Clip its clear terminal cover on.
- **Joints on the brew-thermostat wires (M5/M6):** pull them away from the boiler before tying them down; tape or heat-shrink any bare metal.
- **Parked switch wires:** insulate each freed end (tape or heat-shrink) and tie it back; GaggiMate notes some machines have monoblock connectors whose receptacles come out bare.

### Low-voltage side

JST **GH** 1.25 mm side-entry headers along the board's **bottom** edge, each a different size so no lead
fits another's socket (docs/SWAP-SAFETY.md): **J2** heater SSR (4-pin), **J6** pressure (3-pin), **J10**
scale (5-pin), **J7** front panel (6-pin). **J5** thermocouple is a screw terminal in the bottom-right corner,
and **J4** USB-C is at the bottom left.

#### Leads

**Buy ready-made GH leads**: "JST GH 1.25 mm, n-pin, with latch, single-ended", about 300 mm, one each of 3,
4, 5 and 6 pins. The plug end clicks into the board; you join the bare far end to the device (solder and
heat-shrink, or a 6.3 mm receptacle for a switch tab). Go by pin position, not wire colour, and label both
ends. Avoid look-alikes that don't fit GH sockets: Molex PicoBlade ("MX1.25"), JST SH (1.0 mm), ZH (1.5 mm).

**The thermocouple clamps straight into J5**, its two bare wires under the screws. **Never splice it:** its
wires are thermocouple alloys all the way to J5, and any joint to copper away from the board is a second
junction whose temperature the board doesn't measure. If the 0.5 m lead is short, use K-type extension wire,
alloy to the same alloy.

| Board pin | Signal | Goes to | Notes |
|---|---|---|---|
| J2.1 | +5 V | heater SSR **3 (+)** | 4-pin housing, two contacts: pins 3–4 stay empty |
| J2.2 | heater OUT (pulled low = on) | heater SSR **4 (−)** | |
| J7.1 | BREW | brew switch, terminal **C.1** | Female spade on the lead's far end |
| J7.2 | STEAM | steam switch, terminal **S.4** | |
| J7.3 | GND | brew switch **C.2**, steam switch **S.5**, and the three LED cathodes | One wire spliced into short tails. **UNSURE** for your switches: check each pair closes when its rocker is on |
| J7.4 / .5 / .6 | LED power / coffee / steam | LED **anodes** in the three lamp positions | **LEDs with their own resistor in the lead** (BUY.md #15: pre-wired, 5 V rated). The board has 330 Ω too; the LED's own resistor keeps a front-panel lead harmless anywhere |
| J10.1 | DOUT1 | HX711 #1 DOUT (left load cell) | 5-pin lead to the tray base |
| J10.2 | SCK | both HX711s' SCK (shared) | |
| J10.3 | GND | HX711 board GND | |
| J10.4 | DOUT2 | HX711 #2 DOUT (right load cell) | |
| J10.5 | +3.3 V | HX711 board **VCC and VDD, linked at the board** | The scale runs from 3.3 V only (HX711 works from 2.6 V), so there's no 5 V on this socket. DOUT swings 0–3.3 V, safe for the ESP32-C6. The lead runs behind the water tank, away from the mains wiring |
| J5.1 | TC+ | thermocouple **positive** (chromel: non-magnetic; yellow in ANSI, green in IEC) | Screw terminal. Route the lead away from the heater and mains wiring; tie it to the holder's bottom so the wires can't pull out |
| J5.2 | TC− | thermocouple **negative** (alumel: the wire a magnet pulls; red in ANSI, white in IEC) | The probe's own bare wire under the screw. Polarity matters: reversed, the reading falls as the boiler heats and the firmware faults it |
| J6.1 | signal | transducer output wire | ~0.5 V at 0 bar. Identify its wires on the bench first (INSTALL §3) |
| J6.2 | +5 V | transducer supply wire | |
| J6.3 | GND | transducer ground wire | |
| J4 | USB-C | first flash, logs, bench power | At the board's bottom-left, reached with the board out of its holder. **USB may be plugged in with mains on:** D2 keeps the on-board supply out of the PC. Bench power over USB runs everything but the mains section |
| U4 | W.FL / MHF III | Taoglas CAB.A02 pigtail + GW.15.2113 2 dBi dipole (BUY.md #13) | Mount the SMA bulkhead in a **plastic** part |

## Checks before mains (multimeter, machine unplugged)

1. **Supply input:** across the board's **J13 L** and **J12 N** tabs, expect a high reading (the IRM module's input, several kΩ or more), not a short.
2. **Heater SSR open:** the 1/L1–2/T1 load terminals read open or very high on the Ω range.
3. **Heater circuit intact:** with the **machine's power switch ON** (still unplugged), temporarily short the two brew-thermostat wires (SSR out of circuit). From C.4 through the heater to neutral you should now read the heater's ~10 Ω. Remove the short afterwards. If it reads open, a parked wire (e.g. S.4) was part of the heater path. Stop and re-check against GaggiMate's diagram before going further.
3b. **Steam thermostat in series with the SSR:** with the short from check 3 still on (reading ~10 Ω), unplug **one** steam-thermostat lead: the reading must go **open**. If it stays at ~10 Ω, **stop**: the steam thermostat isn't in the SSR's heater path, so don't power on until it is. Plug it back and confirm ~10 Ω again. This proves the 145 °C thermostat still guards the heater path the SSR switches.
4. **Triacs open:** **J13 L** to **J14 P**, and **J13 L** to **J15 3**, read open or very high (the snubbers are 100 Ω in series with 22 nF: open at DC).
4a. **Pump and valve still connected after the switch wires were parked** (step 7), measured to S.5 with the power switch ON:
   - board **J14 P** through the pump reads its coil. The Ulka has a diode in series, so use diode mode or try both lead polarities before calling it open;
   - board **J15 3** through the valve coil reads a few hundred Ω to a few kΩ.
   An open reading means a parked wire was part of that path. **Look** that the pump's thermal fuse is back in its clip if the plumbing step took it out (a loose fuse still conducts).
5. **Isolation:** no continuity from any of the board's four spade tabs (or any mains wire) to:
   - any pin of J2, J5, J6, J7, J10, or the USB-C shell;
   - the switch dry contacts and the heater SSR's input terminals (3, 4).
   No continuity from any mains wire or spade to the chassis.
6. **Earth:** touch the meter leads together first and subtract that reading. Chassis to the plug's earth pin < 0.5 Ω, and **the heater SSR's metal base** to the earth pin < 0.5 Ω (through its M4 bolts: scrape paint under them if it reads high).
7. **Switches:** J7.1–J7.3 closes with brew pressed; J7.2–J7.3 with steam pressed (J7.3 is GND).
8. **Thermocouple** (do this before INSTALL §4 fits it): a few ohms to a few tens of ohms between its two wires (it's one loop of thin alloy wire), and **open (> 20 MΩ, the meter's top range)** from either wire to the M4 screw body. If either wire shows continuity to the body, it's a *grounded* probe: don't use it (the listing says ungrounded; a grounded one puts the earthed boiler on the board's input). Find TC− with a magnet: alumel is magnetic.

9. **After the case goes back together:** every earth wire you unplugged is back on. GaggiMate's install guide warns that the top cover (cup warmer) has **two** earth wires. Measure each panel, the top cover included, to the plug's earth pin: < 0.5 Ω.

## First power-on

Follow INSTALL.md §6:
1. The board boots from its own supply. It runs its power-on self-test first (each output in turn), then with the power rocker on it heats: the UI shows the heater on and the power meter reads ~1.37 kW.
2. A flush (UI or `coffee flush`) runs the pump and opens the valve together: you hear the pump and see water at the group. There is no heater-only or pump-only command.
3. Listen for pump hum while the pump is "off". Each triac channel leaks about 1 mA through its snubber, far below what moves the pump; a hum means something is bypassing the triac (the firmware's stroke detector flags it too).
