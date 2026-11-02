# Community profiles

Imported from public sources and converted to coffee's profile format. They are **not** under this
repository's MIT/Apache license; each folder keeps its source's license:

| Folder | Source | License | Converted with |
|---|---|---|---|
| `decent/` | [decentespresso/de1app](https://github.com/decentespresso/de1app), `de1plus/profiles/*.tcl` (espresso profiles only) | **GPL-3.0** | `coffee-emu import-decent` |
| `gaggimate/` | [jniebuhr/gaggimate](https://github.com/jniebuhr/gaggimate), `data/p/*.json` | **CC BY-NC-SA 4.0** (non-commercial, share-alike) | `coffee-emu import-gaggimate` |

Authors are kept in each profile's `author` field. The engine loads this folder as a read-only
library (`coffee-engine --profile-library`, default `profiles/community`).

Conversion limits (reported as warnings by the importers): per-step temperature changes become one
temperature for the whole shot; GaggiMate's "maintain current pressure/flow" is approximated.
How each profile behaves on the emulated Gaggia Classic Pro: [docs/PROFILES.md](../../docs/PROFILES.md).
