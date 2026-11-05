# Wiring: v18 low-voltage board + off-board modules (US Gaggia Classic Pro, 120 V)

![wiring diagram](wiring.svg)

This replaces INSTALL.md §5 for the v18 board. The board carries no mains: certified modules
switch the heater, pump and valve (parts and links are in [BUY.md](BUY.md)). INSTALL.md §4
(sensors, plumbing) and §6–§7 still apply.

> ⚠️ **Mains voltage.** Work with the machine unplugged, and cooled if it has been on. Photograph
> the stock wiring before you touch it. The thermal fuse and steam thermostat stay in series with
> the heater: they're the protection that doesn't depend on software. If you aren't comfortable
> with mains wiring, have this checked by an electrician before first power-on.

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
   - The NTC probe goes into the thermostat's M4 hole (INSTALL.md §4).
2. **Steam thermostat and thermal fuse:** leave them, and their wiring, exactly as they are.
3. **Switched live (L):** unplug **C.4 (brown)** from the brew switch. It becomes the live feed for everything we add. GaggiMate uses the same wire as its board's L.
   Confirm it **unplugged**, with the meter on Ω: the plug's **narrow blade (live)** to C.4 reads continuity with the power switch ON and open with it OFF. (The US plug is polarised, so this also proves the fuse below ends up in the live.)
4. **Neutral (N):** unplug **S.5 (white)** from the steam switch. It becomes the neutral feed for the 5 V supply. GaggiMate uses the same wire as its board's N.
   Confirm it unplugged: the plug's **wide blade (neutral)** to S.5 reads continuity, and S.5 to the earth pin reads open.
5. **Pump**
   - Unplug the **grey PMP.B** wire from the pump: the one that does *not* go to the pump's thermal fuse.
   - A new wire from the pump SSR goes onto that pump tab.
   - The pump's white wire stays where it is.
6. **Valve: identify its live wire before you park anything (step 7).**
   - Machine unplugged. With the meter on Ω, find the wire that reads through the valve coil to the valve's neutral: a few hundred Ω to a few kΩ.
   - In GaggiMate's build that is the free end of the grey **PMP.B** wire (it goes to their "V" output). On some machines the valve feed instead comes off a **brew-switch terminal**. If so, that switch wire is the valve live: **don't park it in step 7.**
   - Label it now. It goes to the relay's **NO (14)** contact, through an insulated 6.3 mm male tab if it ends in a receptacle.
   - The valve neutral stays where it is.
   - If no wire shows the coil's resistance, stop: don't guess. Photograph the harness and trace the valve's two wires back from the valve body.
7. **Brew and steam switches** (after step 6)
   - Remove every mains wire from both switches, **except** that you already took the valve live labelled in step 6 to the relay.
   - Insulate each freed end (they already have insulated receptacles; add heat-shrink over any that don't) and tie them back.
   - The switches then carry only the 5 V J7 signals, and the switch lamps go dark.
   - GaggiMate moves S.4 (black) to the S.1 position. In this build S.4 is parked insulated, **not** moved, because the steam switch must not bypass anything: the board owns the heater.
   - **UNSURE:** whether S.4 is part of the steam-thermostat path on a US machine. If parking it breaks the heater circuit, the heater simply won't heat (fail-safe), and the "heater circuit" check below catches it.

## Terminal-by-terminal table

### Mains side

**One wire for everything:** Tensility 30-03445, 14 AWG silicone, UL AWM 3530, 200 °C, 600 V
(BUY.md §8). The heater legs need 14 AWG: TE rates its 6.35 mm FASTONs 15 A on 14 AWG and only
10 A on 16 AWG, against the heater's 11.4 A. Using it everywhere is also the cheapest option.

**Crimps:** TE fully insulated nylon FASTONs for 16-14 AWG only: male **3-520107-2**, female
**3-520140-2** (105 °C, 600 V, 15 A on 14 AWG; TE specs 108-2017 / 108-2043). Solder joints get
**3M FP-301 black** heat-shrink (UL 600 V @ 125 °C; the clear version isn't rated).

| # | From (terminal) | To (terminal) | End at the "from" side / "to" side | Notes |
|---|---|---|---|---|
| M1 | C.4 brown (female receptacle, ex brew switch) | WAGO 221-415, port 1 | **male** FASTON ← C.4 / bare 11 mm strip into the WAGO | Switched L |
| M2 | WAGO, port 2 | fuse holder (HTB-22M-R) **end** terminal | bare / soldered, heat-shrink over the tab | Line to the end (rear) terminal: the live contact then sits deep in the holder when the cap is out |
| M3 | fuse holder **side** terminal | RECOM **brown** lead (L in) | the RECOM lead soldered straight to the tab, heat-shrink | No extra wire. 1 A time-delay ceramic 5×20 (S505-1-R) in the holder |
| M4 | S.5 white (female receptacle, ex steam switch) | RECOM **blue** lead (N in) | **male** FASTON ← S.5 on a 14 AWG pigtail / pigtail soldered to the blue lead, heat-shrink | Neutral: one join, no splice block |
| M6 | WAGO, port 3 | pump SSR (EZ240D5) load tab 1 | bare / **female** FASTON | Load polarity doesn't matter |
| M7 | pump SSR load tab 2 | pump live tab (where grey PMP.B was) | **female** / **female** | |
| M8 | WAGO, port 4 | relay **11 (COM)** | bare / bare in the screw clamp (26–14 AWG) | |
| M9 | relay **14 (NO)** | grey PMP.B free end (female receptacle) | bare / **male** FASTON | Valve live. Leave **12 (NC)** empty |
| M10 | brew-thermostat wire 1 (white, female) | heater SSR **1/L1** | **male** FASTON / bare 14 AWG under the clamp | Schneider allows 20–8 AWG bare ("without cable end", datasheet p2) |
| M11 | brew-thermostat wire 2 (black, female) | heater SSR **2/T1** | **male** FASTON / bare | |
| — | pump white, valve neutral, steam thermostat, thermal fuse, heater, earth | unchanged | — | Don't touch |
| — | WAGO port 5 | spare | — | Lever closed, nothing inserted |

Mechanical:
- **Heater SSR:**
  - Bolt it to the HS25ET heatsink with the two M4 screws. Schneider fits a thermal pad at the factory; **don't** add grease on top of it.
  - The HS25ET is an external-mount sink: as sold, the SSR side faces the panel and the fins stick out through a 70 × 55 mm cutout. Either cut that hole in the rear panel, or mount the sink **inside** on the rear panel with metal M4 screws and nuts as spacers, at least 10 mm off the panel, fins vertical in free air.
  - Either way, the heatsink's earth bond runs through those metal screws: check 6 below measures it.
- **Pump SSR:** bolt it to a flat chassis panel with 8-32 screws (datasheet torque 1.1–1.5 N·m). It dissipates < 0.5 W at 0.45 A; the panel is its heatsink.
- **Relay and supply:** clip the relay onto the DIN rail and screw the rail to the side panel, away from the boiler (the relay is rated to 60 °C ambient). Tie the RECOM next to it.
- **WAGO:** away from the boiler too (rated to 85 °C).
- **M10/M11 joints:** the nylon FASTONs are rated 105 °C, and the boiler runs at 93–145 °C. With the thermostat removed, the two stock receptacles are free: pull them at least 50 mm away from the boiler and tie the joint to the frame there. The wire itself is rated 200 °C, so no sleeve is needed. If the stock wires don't reach 50 mm, stop: don't leave a nylon joint on the boiler.
- **Parked switch wires:** the freed receptacles are insulated already. On machines with **monoblock** connectors (GaggiMate notes some GCP/E24s have them), the receptacles come out bare: put a piece of 3/8" FP-301 heat-shrink over each before tying it back.

**Crimp count:** male 3-520107-2: **5** (M1, M4, M9, M10, M11). Female 3-520140-2: **9** (M6, M7 ×2, J3 ×2, J7 ×4).

### Low-voltage side

Every board connector is a JST **GH** 1.25 mm 3-pin side-entry header (SM03B-GHS-TB). Every cable is a pre-crimped JST GH 3-pin lead (GHR-03V-S housing), about 300 mm, with the far end stripped (BUY.md §9).

#### Joining the GH leads

GH leads are thin (**26–28 AWG**; the ones in BUY.md are 28 AWG). Crimps and screw clamps on wire that thin come loose on a machine the pump shakes. So:
1. **Splice every GH lead to a short pigtail** of the 14 AWG silicone (about 80 mm) where it meets a module terminal. Twist, solder, and cover with 1/4" FP-301 heat-shrink. The GH leads are JST AGHGH28K305 jumpers cut in half (one contact end each, ~150 mm).
2. **Strain-relieve the joint** with a cable tie to the holder.
3. **The 14 AWG pigtail goes to the module:**
   - **Heater SSR input (screw clamp, 24–12 AWG):** strip 8 mm and clamp the pigtail. A ferrule is better.
   - **Pump SSR input (6.3 mm Faston tabs):** female FASTON (3-520140-2) on the pigtail.
   - **Relay coil A1/A2 (screw clamp):** the pigtail, with a ferrule if you have one.
   - **Brew/steam switch tabs:** female FASTONs on pigtails.
   - **RECOM output, thermistor, transducer:** solder + heat-shrink directly; no pigtail needed.
4. Tug-test every joint.

| Board pin | Signal | Goes to | Notes |
|---|---|---|---|
| J1.1 | +5 V in | RECOM **red** (+Vout) | Solder + heat-shrink the 28 AWG GH lead to the 18 AWG RECOM lead |
| J1.2 | GND | RECOM **black** (−Vout) | |
| J1.3 | GND | RECOM **black** too (recommended) | Joining J1.2 and J1.3 to the same black lead halves the ground-return resistance of the thin lead. Or cut it back and insulate it |
| J2.1 | +5 V | heater SSR **3 (+)** input | Folded strands or 0.25 mm² ferrule (see above) |
| J2.2 | OUT (pulled low = ON) | heater SSR **4 (−)** input | |
| J2.3 | GND | — | Unused. Cut back and insulate |
| J3.1 | +5 V | pump SSR **3 (+)** input tab | Female FASTON on a 14 AWG pigtail |
| J3.2 | OUT | pump SSR **4 (−)** input tab | |
| J3.3 | GND | — | Unused |
| J8.1 | +5 V | relay **A1 (+)** | Input 38 mA typical at 5 V, range 4.2–7 V (Phoenix datasheet, BUY.md §4) |
| J8.2 | OUT | relay **A2 (−)** | Polarity matters: the module has an LED and a suppression diode |
| J8.3 | GND | — | Unused |
| J7.1 | BREW | brew switch, terminal **C.1** | Female FASTON on a 14 AWG pigtail |
| J7.2 | STEAM | steam switch, terminal **S.4** | |
| J7.3 | GND | brew switch **C.2** *and* steam switch **S.5** | Splice the one GND lead into two short tails (solder + heat-shrink). C.1/C.2 and S.4/S.5 are the pole positions GaggiMate uses for its LV button wires; **UNSURE** for your switch: check with a meter that each pair closes when its switch is pressed |
| J5.1 | NTC | Molex 218537-3919 lead (PTFE, 28 AWG) | No polarity. Solder + heat-shrink, with the joint well away from the boiler (the probe's 300 mm leads reach). The firmware defaults are this probe's (Steinhart-Hart from Molex's table, BUY.md §10a) |
| J5.2 | GND | thermistor lead | |
| J5.3 | — | — | Unused |
| J6.1 | +5 V | transducer supply wire | GaggiMate's 0–1.6 MPa sensor has no datasheet: identify its wires on the bench first (INSTALL §3) |
| J6.2 | signal | transducer output wire | ~0.5 V at 0 bar |
| J6.3 | GND | transducer ground wire | Solder + heat-shrink each splice |
| J4 | USB-C | flashing / bench power | **USB and J1 may be connected at the same time.** The board feeds VBUS in through a Schottky diode (D2) and J1 through a P-MOSFET. With both connected, the 5 V module supplies the board and nothing back-feeds the PC. With the case open, still flash and bench-test with the machine unplugged |
| U4 | W.FL / MHF III | Taoglas CAB.A02 pigtail (MHF III → SMA bulkhead) + Taoglas GW.15.2113 2 dBi SMA dipole (BUY.md §11) | Mount the SMA bulkhead in a **plastic** part: on the steel case the dipole's gain can exceed the module's 2.33 dBi limit (Taoglas plots). Press the MHF III plug straight down onto the socket; don't force it |

## Checks before mains (multimeter, machine unplugged)

1. **Supply input:** across RECOM brown and blue (through the fuse), expect a high reading (several kΩ or more), not a short.
2. **Heater SSR open:** the 1/L1–2/T1 load terminals read open or very high on the Ω range.
3. **Heater circuit intact:** with the **machine's power switch ON** (still unplugged), temporarily short the two brew-thermostat wires (SSR out of circuit). From C.4 through the heater to neutral you should now read the heater's ~10 Ω. Remove the short afterwards. If it reads open, a parked wire (e.g. S.4) was part of the heater path. Stop and re-check against GaggiMate's diagram before going further.
3b. **Steam thermostat in series with the SSR:** with the short from check 3 still on (reading ~10 Ω), unplug **one** steam-thermostat lead: the reading must go **open**. Plug it back and confirm ~10 Ω again. This proves the 145 °C thermostat still guards the heater path the SSR switches.
4. **Pump SSR and relay open:** pump SSR load tabs open; relay 11–14 open, 11–12 closed.
4a. **Pump and valve still connected after the switch wires were parked** (step 7), **and the pump's thermal fuse back in its clip** if the plumbing step removed it (GaggiMate): pump SSR tab 2 (M7) through the pump to the neutral WAGO reads the pump coil (a few hundred Ω to a few kΩ, not open); relay 14 (M9) through the valve coil to the neutral WAGO reads the valve coil (not open). An open reading means a parked wire was part of that path.
5. **Isolation:** no continuity from any mains wire, SSR load terminal or relay contact to:
   - J1–J8 pins;
   - the RECOM red/black leads;
   - the switch dry contacts.
   No continuity from any mains wire to the chassis.
6. **Earth:** chassis to the plug's earth pin < 0.5 Ω, and **the heater SSR's heatsink** to the plug's earth pin < 0.5 Ω. That bond runs through its mounting screws: scrape paint under them if it reads high.
7. **Switches:** J7.1–J7.3 closes when brew is pressed; J7.2–J7.3 closes when steam is pressed.
8. **Thermistor** (do this before INSTALL §4 fits it): **~50 kΩ** between its two leads at 25 °C (Molex 218537-3919: 50 kΩ ± 1 %; about 63 kΩ at 20 °C), and **open (> 20 MΩ, the meter's top range)** from either lead to the M4 screw body. If either lead shows continuity to the body, don't use it: the boiler is earthed, so that would ground the NTC input.

9. **After the case goes back together:** every earth wire you unplugged is back on. GaggiMate's install guide warns that the top cover (cup warmer) has **two** earth wires. Measure each panel, the top cover included, to the plug's earth pin: < 0.5 Ω.

## First power-on

Follow INSTALL.md §6:
1. The board boots from the RECOM output (≈ 5.0 V at J1). The heater SSR's LED comes on within a second: the firmware heats from boot.
2. A flush (UI or `coffee flush`) runs the pump and opens the valve together: both the pump SSR and the relay LED light. There is no heater-only or pump-only command.
3. Listen for pump hum while the pump is "off". The EZ240D5's 0.1 mA leakage should give none. If the pump hums, something is bypassing the SSR.
