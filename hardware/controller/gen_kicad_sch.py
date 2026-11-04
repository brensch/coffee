#!/usr/bin/env python3
"""Write a native KiCad 7 project (.kicad_sch + .kicad_pro, and the PCB linked to it) from the SKiDL
schematic's parts data (build/parts-120v.json), for tools that need a real schematic file (Quilter).

Each part becomes a symbol (box with numbered pins, its real reference, value, footprint, MPN and
description); every pin carries a global label with its net name, unconnected pins a no-connect
flag. Connectivity is therefore exactly the SKiDL netlist; check_sch() proves it by exporting the
netlist back out with kicad-cli and comparing net by net.

    source ~/opt/kienv.sh && python3 gen_kicad_sch.py [board.kicad_pcb] [out_dir]
"""
import json, os, re, subprocess, sys, uuid

import pcbnew

HERE = os.path.dirname(os.path.abspath(__file__))
G = 2.54
U = lambda: str(uuid.uuid4())


def q(s):
    return '"' + str(s).replace("\\", "\\\\").replace('"', '\\"') + '"'


def font(hide=False):
    return f"(effects (font (size 1.27 1.27)){' hide' if hide else ''})"


def symbol_def(name, pins):
    """A box with the pins split left/right; pin connection points on the 2.54 mm grid. `pins` is
    one (number, copy) per footprint pad, so a pad number used by several pads (SOT-223 tab, USB-C
    shell, the module's split ground pad) gets a stacked pin per pad and the pin count matches."""
    nums = sorted(pins, key=lambda nk: (len(nk[0]), nk[0], nk[1]))
    left, right = nums[: (len(nums) + 1) // 2], nums[(len(nums) + 1) // 2:]
    h = max(len(left), len(right)) * G / 2 + G / 2
    out = [f"    (symbol {q('coffee:' + name)} (in_bom yes) (on_board yes)",
           f"      (property \"Reference\" \"U\" (at 0 {h + 1.27:.2f} 0) {font()})",
           f"      (property \"Value\" {q(name)} (at 0 {-h - 1.27:.2f} 0) {font()})",
           f"      (property \"Footprint\" \"\" (at 0 0 0) {font(True)})",
           f"      (property \"Datasheet\" \"\" (at 0 0 0) {font(True)})",
           f"      (symbol {q(name + '_0_1')} (rectangle (start -5.08 {h:.2f}) (end 5.08 {-h:.2f}) (stroke (width 0.254) (type default)) (fill (type background))))",
           f"      (symbol {q(name + '_1_1')}"]
    pos = {}
    for side, lst, x, ang in (("L", left, -7.62, 0), ("R", right, 7.62, 180)):
        for i, n in enumerate(lst):
            y = (len(lst) - 1) * G / 2 - i * G
            y = round(y / 1.27) * 1.27
            pos[n] = (x, y, side)
            out.append(f"        (pin passive line (at {x:.2f} {y:.2f} {ang}) (length 2.54) (name {q(n[0])} {font()}) (number {q(n[0])} {font()}))")
    out.append("      )")
    out.append("    )")
    return "\n".join(out), pos, h


def write(spec, out_dir, project, pads=None):
    root = U()
    libs, body, sym_uuid = [], [], {}
    parts = sorted(spec["parts"].items(), key=lambda kv: (re.sub(r"\d", "", kv[0]), int(re.sub(r"\D", "", kv[0]) or 0)))
    x, y, col_h = 25.4, 25.4, 0
    for ref, p in parts:
        nums = pads.get(ref) if pads else None
        if not nums:
            nums = list(p["pins"])
        seen = {}
        pins = []
        for n in nums:
            pins.append((n, seen.get(n, 0))); seen[n] = seen.get(n, 0) + 1
        d, pos, h = symbol_def(ref, pins)
        libs.append(d)
        if y + 2 * h + 20 > 400:
            x += 76.2; y = 25.4
        X, Y = x, y + h + 7.62
        X, Y = round(X / G) * G, round(Y / G) * G
        su = U(); sym_uuid[ref] = su
        props = [("Reference", ref, h + 1.27, False), ("Value", p["value"], -h - 1.27, False),
                 ("Footprint", p["footprint"], -h - 3.81, True), ("Datasheet", "", 0, True),
                 ("MPN", p.get("mpn", ""), 0, True), ("Description", p.get("desc", ""), 0, True)]
        body.append(f"  (symbol (lib_id {q('coffee:' + ref)}) (at {X:.2f} {Y:.2f} 0) (unit 1) (in_bom yes) (on_board yes) (dnp no) (uuid {su})")
        for k, v, dy, hide in props:
            body.append(f"    (property {q(k)} {q(v)} (at {X:.2f} {Y - dy:.2f} 0) {font(hide)})")
        for n in sorted({n for n, _ in pins}):
            body.append(f"    (pin {q(n)} (uuid {U()}))")
        body.append(f"    (instances (project {q(project)} (path {q('/' + root)} (reference {q(ref)}) (unit 1))))")
        body.append("  )")
        for n, (px, py, side) in pos.items():
            ax, ay = X + px, Y - py              # library y is up, schematic y is down
            net = p["pins"].get(n[0])
            if net:
                ang, just = (180, "right") if side == "L" else (0, "left")
                body.append(f"  (global_label {q(net)} (shape passive) (at {ax:.2f} {ay:.2f} {ang}) (fields_autoplaced)"
                            f" (effects (font (size 1.27 1.27)) (justify {just})) (uuid {U()})"
                            f" (property \"Intersheetrefs\" \"${{INTERSHEET_REFS}}\" (at {ax:.2f} {ay:.2f} 0) {font(True)}))")
            else:
                body.append(f"  (no_connect (at {ax:.2f} {ay:.2f}) (uuid {U()}))")
        y = Y + h + 12.7
    sch = ["(kicad_sch (version 20230121) (generator coffee_gen_kicad_sch)", f"  (uuid {root})", '  (paper "A1")',
           "  (lib_symbols", *libs, "  )", *body, '  (sheet_instances (path "/" (page "1")))', ")"]
    path = os.path.join(out_dir, project + ".kicad_sch")
    open(path, "w").write("\n".join(sch) + "\n")
    open(os.path.join(out_dir, project + ".kicad_pro"), "w").write(json.dumps({
        "meta": {"filename": project + ".kicad_pro", "version": 1},
        "sheets": [[root, ""]], "schematic": {"legacy_lib_dir": "", "legacy_lib_list": []},
        "board": {"design_settings": {"defaults": {"board_outline_line_width": 0.15}}}}, indent=2))
    return path, root, sym_uuid


def link_pcb(pcb_in, pcb_out, root, sym_uuid):
    """Give every footprint the path of its schematic symbol, so KiCad sees one project."""
    b = pcbnew.LoadBoard(pcb_in)
    for fp in b.GetFootprints():
        su = sym_uuid.get(fp.GetReference())
        if su:
            fp.SetPath(pcbnew.KIID_PATH(f"/{su}"))
    pcbnew.SaveBoard(pcb_out, b)


def check_sch(sch, spec):
    net = sch[:-len(".kicad_sch")] + "-check.net"
    subprocess.run(["kicad-cli", "sch", "export", "netlist", "--format", "kicadsexpr", "-o", net, sch], check=True, capture_output=True)
    txt = open(net).read()
    got = {}
    for m in re.finditer(r'\(net \(code "?\d+"?\) \(name "([^"]*)"\)(.*?)\)\s*(?=\(net |\)\s*\)\s*$)', txt, re.S):
        name = m.group(1).lstrip("/")
        for r, n in re.findall(r'\(node \(ref "([^"]+)"\) \(pin "([^"]+)"\)', m.group(2)):
            got[(r, n)] = name
    bad = []
    for ref, p in spec["parts"].items():
        for n, want in p["pins"].items():
            g = got.get((ref, n))
            if want and g != want:
                bad.append(f"{ref}.{n}: schematic {g!r}, SKiDL {want!r}")
    return bad, len(got)


if __name__ == "__main__":
    spec = json.load(open(os.path.join(HERE, "build", "parts-120v.json")))
    pcb_in = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "build", "coffee-controller-120v.kicad_pcb")
    out = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "build", "kicad-project")
    os.makedirs(out, exist_ok=True)
    project = "coffee-controller"
    # one symbol pin per numbered copper pad of the footprint (unnumbered = mechanical holes)
    pads = {}
    for fp in pcbnew.LoadBoard(pcb_in).GetFootprints():
        pads[fp.GetReference()] = [p.GetNumber() for p in fp.Pads() if p.GetNumber()]
    sch, root, sym = write(spec, out, project, pads)
    link_pcb(pcb_in, os.path.join(out, project + ".kicad_pcb"), root, sym)
    bad, n = check_sch(sch, spec)
    print(f"wrote {out}/{project}.kicad_sch/.kicad_pcb/.kicad_pro: {len(spec['parts'])} symbols; "
          f"round-trip netlist: {n} pins, {len(bad)} mismatches")
    for b in bad[:20]:
        print("  " + b)
    sys.exit(1 if bad else 0)
