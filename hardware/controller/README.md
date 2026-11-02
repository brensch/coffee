# Controller board

Schematic as code (SKiDL). The design, the calculations and the layout rules are in
[docs/CIRCUIT.md](../../docs/CIRCUIT.md).

```sh
python3 -m venv .venv && .venv/bin/pip install -r requirements.txt
.venv/bin/python controller.py                 # 230 V: ERC + design checks -> build/
MAINS_V=120 .venv/bin/python controller.py     # 120 V variant
```

`build/` holds the generated KiCad netlist (import it into KiCad's PCB editor: File → Import → Netlist)
and the BOM CSVs. Generation fails if the ERC, the isolation check, the firmware pin-map
check or the boot-safe SSR check fails.
