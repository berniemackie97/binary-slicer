# Changelog

## Unreleased
- Schema v8 migration runs automatically on open (persists analysis roots, evidence kinds; clears stale analysis rows on reruns).
- Evidence/roots summaries surface in list/project info outputs for quick coverage checks without opening artifacts.
- Slice docs/reports now emit analysis summaries (functions/calls/basic blocks/evidence/roots) and group evidence by function, calling out unmapped evidence so provenance is clearer.
