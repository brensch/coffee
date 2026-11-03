# Vendored footprints

| Footprint | Source | Licence |
|---|---|---|
| `Espressif.pretty/ESP32-C6-MINI-1U.kicad_mod` | [espressif/kicad-libraries](https://github.com/espressif/kicad-libraries) (`footprints/Espressif.pretty`) | CC-BY-SA 4.0 with the KiCad library exception (designs using it aren't covered) |

Checked against Espressif's `ESP32-C6-MINI-1/U` symbol (same repo) and the ESP32-C6-MINI-1/1U datasheet:
- 53 pads: pins 1–35 around the edge on a 0.8 mm pitch (0.4 × 0.8 mm), GND pins 36–48 in the inner row, a 3 × 3 exposed GND pad (49) and four GND corner pads (50–53);
- body 13.2 × 12.5 mm;
- pin numbers and names match the symbol. They're copied into `controller.py` (template `ESP32C6`), and `controller.py` checks the firmware's pin map against them.

Why the MINI-1U rather than the WROOM-1U:
- it keeps the external antenna (U.FL) and is under half the area;
- the -H4 variant is rated to 105 °C (the machine's case runs at 60–70 °C);
- JLCPCB stocks it (C20627095), but had no WROOM-1U in stock in Nov 2026.

The 3D model (`ESP32-C6-MINI-1U.STEP`, 7 MB) isn't vendored; `render3d.py` expects it under `$MODELS/espressif.3dshapes/`.
