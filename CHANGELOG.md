# Changelog

## Unreleased
- Schema v9 migration runs automatically on open (persists analysis roots + per-root hits, evidence kinds; clears stale analysis rows on reruns).
- Evidence/roots summaries surface in list/project info outputs for quick coverage checks without opening artifacts.
- Slice docs/reports now emit analysis summaries (functions/calls/basic blocks/evidence/roots), root coverage (matched vs unmatched), and group evidence by function, calling out unmapped evidence so provenance is clearer. Run/list/project summaries now expose root coverage as well.
