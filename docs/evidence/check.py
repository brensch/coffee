#!/usr/bin/env python3
"""Validate docs/evidence/parts.jsonl: required fields, statuses, dates, one `chosen` per role.

    python3 docs/evidence/check.py            # prints a per-role summary; exit 1 on errors
"""
import json, os, re, sys
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
REQ = ("id", "role", "maker", "mpn", "status", "reason", "specs", "datasheets", "buy", "checked")
DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")

errors, roles, ids = [], defaultdict(list), set()
for n, line in enumerate(open(os.path.join(HERE, "parts.jsonl")), 1):
    if not line.strip():
        continue
    try:
        r = json.loads(line)
    except json.JSONDecodeError as e:
        errors.append(f"line {n}: bad JSON ({e})"); continue
    for k in REQ:
        if k not in r:
            errors.append(f"line {n} {r.get('id')}: missing {k}")
    if r.get("id") in ids:
        errors.append(f"line {n}: duplicate id {r.get('id')}")
    ids.add(r.get("id"))
    if r.get("status") not in ("chosen", "alternative", "rejected"):
        errors.append(f"line {n} {r.get('id')}: status {r.get('status')!r}")
    if not DATE.match(str(r.get("checked", ""))):
        errors.append(f"line {n} {r.get('id')}: checked date {r.get('checked')!r}")
    for s in r.get("specs", []):
        if not {"spec", "value", "source", "verified"} <= s.keys():
            errors.append(f"line {n} {r.get('id')}: spec missing fields: {s}")
    for b in r.get("buy", []):
        if "seen" in b and not DATE.match(str(b["seen"])):
            errors.append(f"line {n} {r.get('id')}: buy date {b['seen']!r}")
    roles[r.get("role")].append(r)

for role, rs in sorted(roles.items()):
    chosen = [r for r in rs if r.get("status") == "chosen"]
    if len(chosen) > 1:
        errors.append(f"role {role}: {len(chosen)} chosen ({', '.join(r['id'] for r in chosen)})")
    c = chosen[0] if chosen else None
    price = min((b.get("price_usd") for b in (c or {}).get("buy", []) if b.get("price_usd") is not None), default=None)
    unver = sum(1 for s in (c or {}).get("specs", []) if not s.get("verified"))
    print(f"{role:22} {('-> ' + c['maker'] + ' ' + c['mpn']) if c else '(none chosen)':45} "
          f"{'$%.2f' % price if price is not None else '':>9}  {len(rs)} candidates"
          f"{'  (' + str(unver) + ' unverified specs)' if unver else ''}")
for e in errors:
    print("ERROR", e)
sys.exit(1 if errors else 0)
