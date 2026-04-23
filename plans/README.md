# Plans

Planning artifacts for multi-step work. Folder layout IS the status signal — no separate status fields to keep in sync.

## Lifecycle

- `plans/<feature>/` — active (in-flight, ready-to-implement, or mid-review)
- `plans/archive/<feature>/` — shipped; move the folder here when the work lands on `main`, as part of (or immediately following) the merge

## Backlog

`plans/BACKLOG.md` — single entry point for deferred items that don't warrant their own plan folder yet. Prefer adding a bullet here over scattering `followups.md` files across feature folders. Longer-form deferred notes tied to a specific shipped feature can stay in that feature's archive folder, but link them from `BACKLOG.md`.
