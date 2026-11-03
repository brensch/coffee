# Controller board v2

One board that drives a Gaggia Classic Pro: an ESP32-S3, the 5 V supply, zero-cross sensing,
three zero-cross triacs (heater, pump, valve), and the thermocouple and pressure inputs. The mains
side and the SELV side are split by a 7 mm copper-free band with milled slots. The design and its
calculations are in [docs/CIRCUIT.md](../../docs/CIRCUIT.md), and the install plan is in
[docs/INSTALL.md](../../docs/INSTALL.md).

## Look at it
- `build/coffee-controller-230v.kicad_pcb`: open it in KiCad 7 or later.
- `build/coffee-controller-230v.png` / `.pdf` / `-top.svg` / `-bottom.svg`: quick views.
- `build/coffee-controller-230v-drc.rpt`: DRC report. 0 errors; the warnings left are
  silkscreen overlaps, two thermal-spoke counts and one dangling stub.
- `build/coffee-controller-230v-fab/`: gerbers, drill file and pick-and-place CSV.
- `build/bom-230v.csv`: the BOM.
- 120 V: the same files, named `-120v`.

## Rebuild
```sh
python3 -m venv .venv && .venv/bin/pip install -r requirements.txt
.venv/bin/python controller.py            # schematic: ERC + design checks -> netlist, BOM, parts.json
source ~/opt/kienv.sh && python3 layout.py # place, route (Freerouting, 90° corners), fill, DRC, fab outputs
MAINS_V=120 .venv/bin/python controller.py && python3 layout.py --mains 120
```

`layout.py` needs:
- **KiCad 7's pcbnew Python and footprint libraries.** No root access? Extract the Ubuntu debs:
  `apt-get download kicad kicad-footprints …`, then `dpkg -x` each one, then source an env file
  that sets `PYTHONPATH`, `LD_LIBRARY_PATH` and `KICAD7_FOOTPRINT_DIR`.
- **Freerouting 2.1.0** (`fr-2.1.0.jar`, Java 21). Don't use the 2.5.0 native CLI: it hangs at
  0% CPU after an internal crash.

**Status:** generated, routed and DRC-checked by code. Not yet reviewed by a person, and not yet
built. Review the mains side before ordering.
