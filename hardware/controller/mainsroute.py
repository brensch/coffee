"""Grid router for the mains side (runs inside pcbnew, called from layout.py).

Freerouting 2.1 throws inside its maze search on our mains nets, so the mains copper is routed here:
A* on a 0.1 mm grid, 8 directions (0/45/90 only), two layers, with a turn penalty so runs come out
straight. Clearances are the same rules layout.write_rules gives the DRC:
  2.0 mm between mains nets; 0.5 mm between same-potential nets, and between items that both sit
  inside one triac/opto/PSU courtyard (that part's own pin pitch governs).
Multi-pin nets grow a tree (each pad joins the nearest copper already on the net). The heater
nets are routed on both layers and then widened segment by segment as far as clearance allows.
"""
import heapq
import math

import numpy as np
import pcbnew
from pcbnew import FromMM as mm, VECTOR2I as V

RES = 0.1
EDGE_GUARD = 0.8
COURT_GROW = 1.0
STRICT = set(filter(None, __import__("os").environ.get("STRICT_NETS", "VALVE_L").split(",")))   # nets the DRC caught: no growth
SELV_CLR = 0.2           # the 0.2 mm rule; tracks start on the 0.1 grid within 0.05 of pad centres
VIA_D, VIA_DRILL = 1.2, 0.6
SQ2 = math.sqrt(2)
DIRS = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)]


class Router:
    def __init__(self, board, spec, W, H, band_x, mains_parts, same_groups, selv_x=None):
        self.b, self.W, self.H, self.bx = board, W, H, band_x
        self.sx = selv_x if selv_x is not None else band_x + 7.0
        self.mains = set(spec["mains_nets"])
        self.group = {}
        for i, g in enumerate(same_groups):
            for n in g:
                self.group[n] = i
        self.nx, self.ny = int(round(W / RES)) + 1, int(round(H / RES)) + 1
        self.X = np.arange(self.nx)[:, None] * RES
        self.Y = np.arange(self.ny)[None, :] * RES
        self.courts = {}
        for fp in board.GetFootprints():
            r = fp.GetReference()
            if r in mains_parts:
                lay = pcbnew.B_CrtYd if fp.IsFlipped() else pcbnew.F_CrtYd
                bb = fp.GetCourtyard(lay).BBox()
                self.courts[r] = tuple(pcbnew.ToMM(v) for v in (bb.GetLeft(), bb.GetTop(), bb.GetRight(), bb.GetBottom()))
        # KiCad applies the 0.5 mm pin-pitch rule to a whole track *segment* that touches the courtyard, so a
        # straight run leaving a pin keeps it for a while outside the courtyard too. For that part's own pads the
        # router uses the courtyard grown by COURT_GROW; the final KiCad DRC is the real check.
        g = COURT_GROW
        self.court_mask = {r: (self.X >= c[0] - g) & (self.X <= c[2] + g) & (self.Y >= c[1] - g) & (self.Y <= c[3] + g) for r, c in self.courts.items()}
        self.court_raw = {r: (self.X >= c[0]) & (self.X <= c[2]) & (self.Y >= c[1]) & (self.Y <= c[3]) for r, c in self.courts.items()}
        # obstacles: dict(kind, net, layers, geometry, courts)
        self.obs = []
        for fp in board.GetFootprints():
            for p in fp.Pads():
                bb = p.GetBoundingBox()
                x0, y0, x1, y1 = (pcbnew.ToMM(v) for v in (bb.GetLeft(), bb.GetTop(), bb.GetRight(), bb.GetBottom()))
                round_ = p.GetShape() in (pcbnew.PAD_SHAPE_CIRCLE, pcbnew.PAD_SHAPE_OVAL)
                layers = [l for l in (pcbnew.F_Cu, pcbnew.B_Cu) if p.IsOnLayer(l)]
                self.obs.append(dict(kind="pad", net=p.GetNetname(), layers=layers, rect=(x0, y0, x1, y1), round=round_,
                                     ref=fp.GetReference(), num=p.GetNumber(), courts=self._touch((x0, y0, x1, y1))))
        self.pads = {}
        for o in self.obs:
            if o["net"]:
                self.pads.setdefault(o["net"], []).append(o)
        self.added = []   # board items we created (tracks, vias), for rip-up
        self.base_obs = list(self.obs)

    failed = set()

    def load_vias(self):
        """Vias already on the board (the module's thermal vias) as obstacles."""
        for t in self.b.GetTracks():
            if t.Type() == pcbnew.PCB_VIA_T:
                c = (pcbnew.ToMM(t.GetPosition().x), pcbnew.ToMM(t.GetPosition().y))
                self.obs.append(dict(kind="via", net=t.GetNetname(), layers=[pcbnew.F_Cu, pcbnew.B_Cu], c=c, d=pcbnew.ToMM(t.GetWidth()), courts=[]))
        self.base_obs = list(self.obs)

    def load_tracks(self):
        """Existing board copper (tracks, vias) as obstacles: for repair passes on a routed board."""
        for t in self.b.GetTracks():
            net = t.GetNetname()
            if t.Type() == pcbnew.PCB_VIA_T:
                c = (pcbnew.ToMM(t.GetPosition().x), pcbnew.ToMM(t.GetPosition().y))
                d = pcbnew.ToMM(t.GetWidth())
                self.obs.append(dict(kind="via", net=net, layers=[pcbnew.F_Cu, pcbnew.B_Cu], c=c, d=d, courts=[]))
            else:
                A = (pcbnew.ToMM(t.GetStart().x), pcbnew.ToMM(t.GetStart().y)); B = (pcbnew.ToMM(t.GetEnd().x), pcbnew.ToMM(t.GetEnd().y))
                self.obs.append(dict(kind="seg", net=net, layers=[t.GetLayer()], a=A, b=B, w=pcbnew.ToMM(t.GetWidth()), courts=[]))

    def join(self, net, src_pads, targets, w=0.3):
        """Connect each pad in src_pads to any cell in targets (set of (i, j, layer))."""
        blk = self.blocked(net, w / 2)
        vb = self.blocked("", 0.3)              # vias keep clear of everything, same-net vias included
        vblk = vb[pcbnew.F_Cu] | vb[pcbnew.B_Cu]
        for t in targets:
            blk[t[2]][t[0], t[1]] = False
        n = 0
        for o in src_pads:
            src = set(self.centre(o, o["layers"]))
            for c in src:
                blk[c[2]][c[0], c[1]] = False
            path = self.astar(src, targets, blk, vblk)
            if path:
                self.emit(net, path, w, 0.6)
                n += 1
        return n

    def fanout(self, net="GND", w=0.3, vd=0.6):
        """Every SMD pad of `net`: a short track to its own via, placed before the signals are routed,
        so each pad reaches whichever layer's pour is solid there."""
        import math
        blk = self.blocked(net, w / 2)
        vb = self.blocked("", vd / 2)
        vblk = vb[pcbnew.F_Cu] | vb[pcbnew.B_Cu]
        n = 0
        for o in self.pads.get(net, []):
            if len(o["layers"]) != 1:
                continue                            # through-hole: already on both layers
            if not (o.get("ref", "")[:1] in ("C", "R") and len([p for p in self.obs if p["kind"] == "pad" and p.get("ref") == o["ref"]]) == 2):
                continue                            # two-pad passives only: the module grounds through its pad array
            L = o["layers"][0]
            x0, y0, x1, y1 = o["rect"]
            cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
            if cx < self.sx:
                continue
            half = max(x1 - x0, y1 - y0) / 2
            done = False
            for rr in (half + 0.45, half + 0.7, half + 1.0, half + 1.4):
                for k in range(16):
                    a = k * math.pi / 8
                    vi, vj = int(round((cx + rr * math.cos(a)) / RES)), int(round((cy + rr * math.sin(a)) / RES))
                    if not (0 <= vi < self.nx and 0 <= vj < self.ny) or vblk[vi, vj]:
                        continue
                    ci, cj = int(round(cx / RES)), int(round(cy / RES))
                    steps = max(abs(vi - ci), abs(vj - cj))
                    if any(blk[L][ci + round((vi - ci) * t / steps), cj + round((vj - cj) * t / steps)] for t in range(1, steps + 1)):
                        continue
                    # 0/45/90 only: go straight if aligned, else a 45 then straight
                    di, dj = vi - ci, vj - cj
                    if di and dj and abs(di) != abs(dj):
                        continue
                    path = [(ci, cj, L), (vi, vj, L), (vi, vj, pcbnew.B_Cu if L == pcbnew.F_Cu else pcbnew.F_Cu)]
                    self.emit(net, path, w, vd)
                    # block this via spot for later fanouts
                    r_ = int(math.ceil((vd + 0.2) / RES))
                    vblk[max(0, vi - r_):vi + r_ + 1, max(0, vj - r_):vj + r_ + 1] = True
                    n += 1; done = True
                    break
                if done:
                    break
        self.base_obs = list(self.obs)
        return n

    def join_cells(self, net, src, targets, w=0.3):
        """Route from any cell in src to any cell in targets (both sets of (i, j, layer))."""
        blk = self.blocked(net, w / 2)
        vb = self.blocked("", 0.3)
        vblk = vb[pcbnew.F_Cu] | vb[pcbnew.B_Cu]
        for t in list(targets) + list(src):
            blk[t[2]][t[0], t[1]] = False
        path = self.astar(src, targets, blk, vblk)
        if path:
            self.emit(net, path, w, 0.6)
            return True
        return False

    def vias(self):
        return sum(1 for it in self.added if it.Type() == pcbnew.PCB_VIA_T)

    def reset(self):
        """rip up everything this router added."""
        for it in self.added:
            self.b.Remove(it)
        self.added = []
        self.obs = list(self.base_obs)

    def _touch(self, r):
        return [k for k, c in self.courts.items() if r[0] < c[2] and c[0] < r[2] and r[1] < c[3] and c[1] < r[3]]

    # ---------------------------------------------------------------- clearance fields
    def _req(self, net, o):
        """required edge-to-edge clearance between `net` and obstacle o (outside any shared courtyard)."""
        if net not in self.mains:                       # SELV
            return SELV_CLR if not (o["net"] and o["net"] in self.mains) else 2.0
        if not o["net"] or o["net"] not in self.mains:
            return 0.5
        g1, g2 = self.group.get(net), self.group.get(o["net"])
        if g1 is not None and g1 == g2:
            return 0.5
        return 2.0

    def _dist(self, o, X, Y):
        if o["kind"] == "seg":
            (ax, ay), (bx, by) = o["a"], o["b"]
            dx, dy = bx - ax, by - ay
            L2 = dx * dx + dy * dy
            t = np.clip(((X - ax) * dx + (Y - ay) * dy) / L2, 0, 1) if L2 > 0 else 0
            return np.hypot(X - ax - t * dx, Y - ay - t * dy) - o["w"] / 2
        if o["kind"] == "via":
            return np.hypot(X - o["c"][0], Y - o["c"][1]) - o["d"] / 2
        x0, y0, x1, y1 = o["rect"]
        r = min(x1 - x0, y1 - y0) / 2 if o["round"] else 0.0
        cx = np.clip(X, x0 + r, x1 - r)
        cy = np.clip(Y, y0 + r, y1 - r)
        return np.hypot(X - cx, Y - cy) - r

    def blocked(self, net, half):
        """per-layer boolean grid: True where a track (or via) of half-width `half` on `net` can't go."""
        out = {}
        base = np.zeros((self.nx, self.ny), bool)
        if net in self.mains:
            base |= self.X > self.bx - half - 0.05                                # isolation band
        else:
            base |= self.X < self.sx + half + 0.05
        base |= (self.X < EDGE_GUARD + half) | (self.Y < EDGE_GUARD + half) | (self.Y > self.H - EDGE_GUARD - half) | (self.X > self.W - EDGE_GUARD - half)
        for L in (pcbnew.F_Cu, pcbnew.B_Cu):
            out[L] = base.copy()
        for o in self.obs:
            if o["net"] == net:
                continue
            req = self._req(net, o)
            if o["kind"] == "pad":
                x0, y0, x1, y1 = o["rect"]
            elif o["kind"] == "seg":
                x0, x1 = sorted((o["a"][0], o["b"][0])); y0, y1 = sorted((o["a"][1], o["b"][1]))
                x0 -= o["w"] / 2; x1 += o["w"] / 2; y0 -= o["w"] / 2; y1 += o["w"] / 2
            else:
                x0, y0, x1, y1 = o["c"][0] - o["d"] / 2, o["c"][1] - o["d"] / 2, o["c"][0] + o["d"] / 2, o["c"][1] + o["d"] / 2
            m = req + half + 0.02
            i0, i1 = max(0, int((x0 - m) / RES)), min(self.nx, int((x1 + m) / RES) + 2)
            j0, j1 = max(0, int((y0 - m) / RES)), min(self.ny, int((y1 + m) / RES) + 2)
            if i0 >= i1 or j0 >= j1:
                continue
            X, Y = self.X[i0:i1], self.Y[:, j0:j1]
            d = self._dist(o, X, Y)
            lim = np.full(d.shape, req + half + 0.02)
            if req > 0.5:
                own = [o["ref"]] if o["kind"] == "pad" and o.get("ref") in self.courts and net not in STRICT else []
                for c in (own or o["courts"]):        # both inside one part's courtyard: 0.5 mm
                    m_ = self.court_mask[c] if own else self.court_raw[c]
                    lim = np.where(m_[i0:i1, j0:j1], 0.5 + half + 0.02, lim)
            hit = d < lim
            for L in o["layers"]:
                out[L][i0:i1, j0:j1] |= hit
        return out

    # ---------------------------------------------------------------- search
    def cells_of(self, o, layer):
        x0, y0, x1, y1 = o["rect"] if o["kind"] == "pad" else (0, 0, 0, 0)
        res = []
        for i in range(int(math.ceil(x0 / RES)), int(x1 / RES) + 1):
            for j in range(int(math.ceil(y0 / RES)), int(y1 / RES) + 1):
                if self._dist(o, i * RES, j * RES) <= -0.05:
                    res.append((i, j, layer))
        return res

    def centre(self, o, layers):
        """the pad's centre grid point on each allowed layer (tracks leave from the middle of the pad)."""
        x0, y0, x1, y1 = o["rect"]
        i, j = int(round((x0 + x1) / 2 / RES)), int(round((y0 + y1) / 2 / RES))
        return [(i, j, L) for L in o["layers"] if L in layers]

    def astar(self, sources, targets, blk, vblk, turn=0.6, via_cost=10.0):
        """sources/targets: sets of (i, j, layer). Returns list of (i, j, layer) or None."""
        tset = set(targets)
        tx = [t[0] for t in targets]; ty = [t[1] for t in targets]
        bx0, bx1, by0, by1 = min(tx), max(tx), min(ty), max(ty)

        def h(i, j):
            dx = max(bx0 - i, 0, i - bx1); dy = max(by0 - j, 0, j - by1)
            return RES * (max(dx, dy) + (SQ2 - 1) * min(dx, dy))
        pq, best, prev = [], {}, {}
        for s in sources:
            st = (s[0], s[1], s[2], 8)
            best[st] = 0.0
            heapq.heappush(pq, (h(s[0], s[1]), 0.0, st))
        layers = (pcbnew.F_Cu, pcbnew.B_Cu)
        while pq:
            f, g, st = heapq.heappop(pq)
            if g > best.get(st, 1e18) + 1e-9:
                continue
            i, j, L, d = st
            if (i, j, L) in tset:
                path = [st]
                while path[-1] in prev:
                    path.append(prev[path[-1]])
                return [(p[0], p[1], p[2]) for p in reversed(path)]
            for k, (di, dj) in enumerate(DIRS):
                ni, nj = i + di, j + dj
                if not (0 <= ni < self.nx and 0 <= nj < self.ny):
                    continue
                if blk[L][ni, nj] and (ni, nj, L) not in tset:
                    continue
                c = RES * (SQ2 if di and dj else 1.0)
                if d != 8 and d != k:
                    c += turn * min((k - d) % 8, (d - k) % 8)
                ns = (ni, nj, L, k)
                ng = g + c
                if ng < best.get(ns, 1e18) - 1e-9:
                    best[ns] = ng; prev[ns] = st
                    heapq.heappush(pq, (ng + h(ni, nj), ng, ns))
            if not vblk[i, j]:
                oL = layers[1] if L == layers[0] else layers[0]
                ns = (i, j, oL, 8)
                ng = g + via_cost
                if ng < best.get(ns, 1e18) - 1e-9:
                    best[ns] = ng; prev[ns] = st
                    heapq.heappush(pq, (ng + h(i, j), ng, ns))
        return None

    # ---------------------------------------------------------------- copper
    def emit(self, net, path, w, vd=VIA_D):
        ni = self.b.GetNetsByName()[net]
        segs = []
        run = [path[0]]
        for p in path[1:]:
            if p[2] != run[-1][2]:              # layer change: via
                segs.append(run); run = [p]
                c = (p[0] * RES, p[1] * RES)
                v = pcbnew.PCB_VIA(self.b)
                v.SetPosition(V(mm(c[0]), mm(c[1]))); v.SetWidth(mm(vd)); v.SetDrill(mm(VIA_DRILL if vd >= 1.0 else 0.3))
                v.SetNet(ni); v.SetLocked(True); self.b.Add(v); self.added.append(v)
                self.obs.append(dict(kind="via", net=net, layers=[pcbnew.F_Cu, pcbnew.B_Cu], c=c, d=vd, track=v, courts=self._touch((c[0] - .6, c[1] - .6, c[0] + .6, c[1] + .6))))
                continue
            run.append(p)
        segs.append(run)
        out = []
        for run in segs:
            if len(run) < 2:
                continue
            pts = [run[0]]
            for a, b_, c in zip(run, run[1:], run[2:]):
                if (b_[0] - a[0], b_[1] - a[1]) != (c[0] - b_[0], c[1] - b_[1]):
                    pts.append(b_)
            pts.append(run[-1])
            L = run[0][2]
            for a, b_ in zip(pts, pts[1:]):
                A, B = (a[0] * RES, a[1] * RES), (b_[0] * RES, b_[1] * RES)
                t = pcbnew.PCB_TRACK(self.b)
                t.SetStart(V(mm(A[0]), mm(A[1]))); t.SetEnd(V(mm(B[0]), mm(B[1])))
                t.SetWidth(mm(w)); t.SetLayer(L); t.SetNet(ni); t.SetLocked(True); self.b.Add(t); self.added.append(t)
                o = dict(kind="seg", net=net, layers=[L], a=A, b=B, w=w, track=t)
                x0, x1 = sorted((A[0], B[0])); y0, y1 = sorted((A[1], B[1]))
                o["courts"] = self._touch((x0 - w / 2, y0 - w / 2, x1 + w / 2, y1 + w / 2))
                self.obs.append(o); out.append(o)
        return out

    def route_net(self, net, w, layers=None, turn=0.6, optional=False, via_d=None, only=None):
        """Connect every pad of `net`. Returns the number of pads that could not be joined.
        optional: a parallel copy on one layer (heater): only pads present on that layer, failures ignored."""
        pads = self.pads.get(net, [])
        if only:
            pads = [o for o in pads if f"{o['ref']}.{o.get('num')}" in only]
        if layers and len(layers) == 1:
            pads = [o for o in pads if layers[0] in o["layers"]]
        if len(pads) < 2:
            return 0
        layers = layers or [pcbnew.F_Cu, pcbnew.B_Cu]
        vd = via_d or (VIA_D if net in self.mains else 0.6)
        blk = self.blocked(net, w / 2)
        vblk = self.blocked(net, vd / 2)
        vblk = vblk[pcbnew.F_Cu] | vblk[pcbnew.B_Cu]
        if len(layers) == 1:
            other = pcbnew.B_Cu if layers[0] == pcbnew.F_Cu else pcbnew.F_Cu
            blk[other] = np.ones_like(blk[other])
            vblk = np.ones_like(vblk)
        cells = lambda o: self.centre(o, layers)
        tree = set(cells(pads[0]))
        left = pads[1:]
        fails = 0
        retried = set()
        while left:
            # nearest pad to the tree first
            cx = np.mean([t[0] for t in tree]); cy = np.mean([t[1] for t in tree])
            left.sort(key=lambda o: (id(o) in retried, math.hypot((o["rect"][0] + o["rect"][2]) / 2 / RES - cx, (o["rect"][1] + o["rect"][3]) / 2 / RES - cy)))
            o = left.pop(0)
            src = set(cells(o))
            if not src:
                fails += 1
                continue
            if src & tree:
                tree |= src
                continue
            for c in src:                       # our own pad's cells are never blocked for us
                blk[c[2]][c[0], c[1]] = False
            for t in tree:
                blk[t[2]][t[0], t[1]] = False
            path = self.astar(src, tree, blk, vblk, turn=turn)
            if path is None and id(o) not in retried and left:
                retried.add(id(o))          # try again once the tree has grown towards it
                left.append(o)
                continue
            if path is None:
                if optional:
                    continue
                fails += 1
                self.failed.add(net)
                print(f"  mainsroute: {net}: could not reach {o['ref']} pad at {o['rect'][:2]}", flush=True)
                continue
            self.emit(net, path, w, vd)
            tree |= set(path) | src
            # the new copper is ours: cells along it are fine for later branches of this net
        return fails

    def widen(self, nets, widths=(4.0, 3.0, 2.5)):
        """Fatten segments of `nets` where the wider track still clears everything."""
        for o in [o for o in self.obs if o["kind"] == "seg" and o["net"] in nets and "track" in o]:
            for w in widths:
                if w <= o["w"]:
                    break
                ok = True
                L = o["layers"][0]
                n = max(2, int(math.dist(o["a"], o["b"]) / 0.1))
                for k in range(n + 1):
                    t = k / n
                    x, y = o["a"][0] + t * (o["b"][0] - o["a"][0]), o["a"][1] + t * (o["b"][1] - o["a"][1])
                    if x > self.bx - w / 2 - 0.05 or x < EDGE_GUARD + w / 2 - 0.3 or y < EDGE_GUARD + w / 2 or y > self.H - EDGE_GUARD - w / 2:
                        ok = False; break
                    for q in self.obs:
                        if q is o or q["net"] == o["net"] or L not in q["layers"]:
                            continue
                        req = self._req(o["net"], q)
                        if req > 0.5 and any(self.courts[c][0] <= x <= self.courts[c][2] and self.courts[c][1] <= y <= self.courts[c][3] for c in q["courts"]):
                            req = 0.5
                        if float(self._dist(q, x, y)) < req + w / 2 + 0.02:
                            ok = False; break
                    if not ok:
                        break
                if ok:
                    o["track"].SetWidth(mm(w)); o["w"] = w
                    break
