#!/usr/bin/env python3
"""Pin-function check for JLC assembly: for every part JLC places, compare what JLC's own symbol calls
each pin with what our schematic connects to that pad (after jlc_cpl.PAD_MAP). Pad numbers can agree
while functions don't (an LED numbered 1 = anode by one library and 1 = cathode by the other): the
position file would then put the part on the board reversed. Exits 1 on any mismatch.

    python3 jlc_pins.py        # reads build/jlc-120v/bom.csv, build/parts-120v.json, build/easyeda-cache
"""
import csv, json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
B = os.path.join(HERE, "build")
sys.path.insert(0, HERE)
from jlc_cpl import PAD_MAP, PROXY   # noqa: E402

PASSIVE = ("R_", "C_")                    # unpolarised two-terminal parts: nothing to check
SYN = [  # names that mean the same function
    {"K", "-", "CATHODE", "C"}, {"A", "+", "ANODE"},
    {"G", "GATE"}, {"S", "SOURCE"}, {"D", "DRAIN"},
    {"VIN", "IN", "VI", "INPUT"}, {"VOUT", "OUT", "VO", "OUTPUT"}, {"GND", "VSS", "GROUND", "PAD", "EP"},
    {"EN", "CE", "ENABLE"}, {"NC", "N/C", "DNC"},
    {"VBUS"}, {"CC1"}, {"CC2"}, {"DP1", "D+", "DP"}, {"DN1", "D-", "DN"}, {"DP2", "D+", "DP"}, {"DN2", "D-", "DN"},
    {"SHIELD", "SHELL", "EH", "MH", "SH", "GND"}, {"SBU1"}, {"SBU2"},
    # v32 mains parts: triac main terminals (ST: A1/A2), the IRM module's outputs, the opto's pin 5
    # (onsemi: substrate, do not connect; JLC's symbol: NC)
    {"MT1", "A1", "T1"}, {"MT2", "A2", "T2"}, {"-VO", "-V", "V-", "-VOUT"}, {"+VO", "+V", "V+", "+VOUT"}, {"SUB", "NC"},
]


def norm(n):
    n = n.upper().strip()
    n = re.sub(r"_[AB]\d+$", "", n)          # our USB names: GND_A1, VBUS_A4 ...
    n = re.sub(r"^(GND|NC)\d+$", r"\1", n)   # our module names: GND36, NC4 ...
    return n


def same(a, b):
    if norm(a) == norm(b):
        return True
    if "/" in a:                              # a shared symbol pin, e.g. TI's "ADJ/GND" (GND on the fixed part)
        return any(same(x, b) for x in a.split("/"))
    a, b = norm(a), norm(b)
    if a == b:
        return True
    return any(a in g and b in g for g in SYN)


def jlc_pins(code):
    """{pad number: pin name} from JLC's symbol (EasyEDA record or, if only that exists, the /svgs dump)."""
    path = os.path.join(B, "easyeda-cache", code + ".json")
    if os.path.exists(path):
        d = json.load(open(path))["result"]["dataStr"]
        out = {}
        for s in d["shape"]:
            if s.startswith("P~"):
                parts = s.split("^^")
                num = parts[0].split("~")[3]
                name = parts[3].split("~")[4] if len(parts) > 3 and len(parts[3].split("~")) > 4 else ""
                out[num] = name
        return out
    svgs = os.path.join(B, "easyeda-cache", code + ".svgs.json")
    if os.path.exists(svgs):
        svg = json.load(open(svgs))["result"][0]["svg"]
        out = {}
        for m in re.finditer(r'<g[^>]*c_partid="part_pin"[^>]*>(.*?)</g>', svg, re.S):
            head = svg[m.start():m.start() + 300]
            num = re.search(r'c_spicepin="([^"]+)"', head).group(1)
            texts = re.findall(r'<text[^>]*>([^<]*)</text>', m.group(1))
            out[num] = texts[0] if texts else ""
        return out
    return None


def main():
    spec = json.load(open(os.path.join(B, "parts-120v.json")))["parts"]
    bad, checked, skipped = [], 0, []
    for line in csv.DictReader(open(os.path.join(os.environ.get("JLC_DIR", os.path.join(B, "jlc-120v")), "bom.csv"))):
        code = line["LCSC Part #"]
        for ref in line["Designator"].split(","):
            p = spec[ref]
            if p["footprint"].split(":")[1].startswith(PASSIVE):
                continue
            pins = jlc_pins(code)
            if pins is None and code in PROXY:
                pins = jlc_pins(PROXY[code])        # same package and pinout, documented in jlc_cpl.PROXY
            if pins is None:
                skipped.append(f"{ref} ({code}): no JLC record cached")
                continue
            pmap = PAD_MAP.get(code, {})
            ours = p["pin_names"]
            for num, jname in pins.items():
                bpads = [pmap.get(num, num)]
                if bpads[0] not in ours:
                    split = re.findall(r"[AB]\d+", num)          # USB-C: one JLC pin "A1B12" = our pads A1 and B12
                    if len(split) > 1 and "".join(split) == num:
                        bpads = split
                    elif num.isdigit() and jname.upper() in ("EH", "SHELL", "MH") and "S1" in ours:
                        bpads = ["S1"]                            # USB-C shell legs: JLC numbers 1-4, ours all "S1"
                for bpad in bpads:
                    if bpad not in ours:
                        continue                       # JLC symbol pin with no pad of that number on our part
                    oname = ours[bpad]
                    if not jname or jname.isdigit() or oname.isdigit():
                        continue                       # numbered-only pins (connectors): numbers already match
                    checked += 1
                    if not same(jname, oname):
                        bad.append(f"{ref} ({code}) pad {bpad}: JLC calls it '{jname}', schematic '{oname}' (net {p['pins'].get(bpad)})")
    for s in skipped:
        print("SKIPPED", s)
    for b in bad:
        print("MISMATCH", b)
    print(f"{checked} pin functions compared, {len(bad)} mismatches, {len(skipped)} parts without a JLC record")
    sys.exit(1 if bad or skipped else 0)


if __name__ == "__main__":
    main()
