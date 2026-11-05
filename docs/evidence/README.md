# Parts evidence

Everything looked at when choosing the off-board and on-board parts: the chosen part, its
alternatives and the rejects, each with the datasheet quotes and buy links it was judged on. When a
requirement changes (price, a rating, availability), change the choice here instead of
researching again.

- `parts.jsonl`: one JSON object per line, one line per candidate part (see the schema below).
- `pdf/`: local copies of the datasheets, named `<maker>_<mpn>.pdf` (git-ignored: the files are
  the makers' copyright; each record keeps the source URL and a SHA-256 to re-fetch and match).
- `check.py`: validates the file (required fields, one `chosen` per role, statuses, dates).

## Record

```json
{
  "id": "pump-ssr/crydom-ez240d5",
  "role": "pump-ssr",
  "maker": "Sensata Crydom",
  "mpn": "EZ240D5",
  "status": "chosen",
  "reason": "zero-cross, 0.1 mA leakage, 150 mA min load: fits the 0.35-0.45 A Ulka",
  "requirements": "pump: 0.35-0.45 A half-wave, zero-cross, off-state leakage <= 1 mA, input 3-5 V",
  "specs": [
    {"spec": "min load current", "value": "150 mA", "quote": "Min. Load Current 150 mA",
     "source": "https://...pdf", "page": "p2", "verified": true}
  ],
  "datasheets": [
    {"title": "Crydom EZ Series", "rev": "022912", "url": "https://...pdf",
     "local": "pdf/crydom_ez240d5.pdf", "sha256": "..."}
  ],
  "buy": [
    {"vendor": "Enrgtech", "url": "https://...", "price_usd": 30.0, "pack": 1,
     "stock": "15", "seen": "2026-11-04", "note": "GBP 23.05, FX estimate"}
  ],
  "checked": "2026-11-04",
  "notes": ""
}
```

**Fields**
- `id`: `<role>/<maker-mpn>`, lowercase, unique.
- `role`: what the part is for. Use the same role name for every candidate for that job, e.g.
  `heater-ssr`, `pump-ssr`, `valve-switch`, `psu-5v`, `fuse`, `fuse-holder`, `wire-14awg`,
  `ntc-probe`, `pressure-sensor`, `antenna`, `pigtail`, `board/U5`.
- `status`: `chosen`, `alternative` (would work, not picked) or `rejected` (fails a requirement:
  say which in `reason`).
- `specs[].verified`: `true` only if read in the maker's own datasheet or product page.
  Listings, distributor parametrics and search snippets are `false`, and say so in `quote`/`source`.
- `buy[].price_usd`: the unit price as seen; `pack` is how many come in one purchase.
- Dates are ISO (`YYYY-MM-DD`).
