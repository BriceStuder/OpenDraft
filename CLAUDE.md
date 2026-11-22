# CLAUDE.md

OpenDraft: a 2D-only CAD app in Rust on egui/eframe, with an AutoCAD-2020-style ribbon, dockable panels and a command line. Windows and Linux are both targets (Linux has been run only on Linux Mint, with the 0.1.0-beta.1 and 0.1.0 tarballs, on 24-10-2026). Licence GPL-3.0-or-later. Repo: https://github.com/OpenEngineeringSoftware/OpenDraft, branch `main`.

## Read first
- `docs/ARCHITECTURE.md`: the rules new code follows (crates, commands, Editor, files, geometry). Authoritative.
- `docs/ROADMAP.md`: the ordered plan (phases, sizes, exit tests) for trust, reach and depth, and "Next steps" for feature work; tick items and date them when done.
- `docs/parking_lot.md`: everything known to be missing or left for later, by area (the to-do list).
- `docs/history.md`: the archived development diary (was `docs/ui_plan.md`, frozen 21-11-2026), for the reasoning behind older decisions. Never add to it.
- `2d_commands_reference.md`: the AutoCAD command list we work through as a roadmap.
- `Command_specifications.md` is the user's untracked scratch file: never stage it.
- `private_material/` is git-ignored (specs, PDFs, screenshots, scratch notes and the AutoCAD sample drawings): nothing in code or tests may depend on it, except the optional `bench_features` example, which skips files it cannot find. The sample drawings carry a work login in their headers, so they are never committed; tests use small hand-made data (or the ezdxf fixtures in `opendraft-files/tests/fixtures/`) instead.

## Crates (dependencies point down)
`opendraft-core` (document, entities, pure geometry) <- `opendraft-commands` (registry, undo, Session, Editor, no egui) <- `opendraft-files` (`.odraft` JSON, DXF) / `opendraft-canvas` (camera, hit, snap, grips, paint) / `opendraft-ui` (theme, widgets, ribbon, icons) <- `opendraft-app` (binary `opendraft`: AppState, command table, panels, `automation/` host). `opendraft-api` (wire contract for outside callers, depends only on `opendraft-core`) and `opendraft-mcp` (binary `opendraft-mcp`, the MCP server; depends on `opendraft-api`, never on the app) and `opendraft-agent` (the in-app assistant's loop; depends only on `opendraft-api`, knows no model vendor or drawing) sit beside them; see docs/ARCHITECTURE.md "Automation and MCP" and `docs/mcp.md`.

## How a command is built
A command is `fn(&mut Editor) -> Result<(), Cancelled>` running on its own thread; it asks (`ask_point`, `get_point`, `ask_object`, `ask_selection`, `pickset_or_select`...) and edits through `ed.transaction("Name", |tx| ...)` (one undo step). Maths goes in `opendraft-core` as pure, tested functions; the command only asks and records. To add one: geometry + tests in `opendraft-core`, a file in `opendraft-app/src`, an `editor_command!` line in `commands.rs`, an alias in `cmdline.rs`, a ribbon item in `ribbon_def.rs` (Home tab: `ribbon_def/home.rs`, and take the id off its `NOT_BUILT` list if it is there), an icon codepoint in `opendraft-ui/src/icons.rs` (then `python scripts/export_icons.py` adds its SVG to `assets/icons`; icons are drawn through `opendraft_ui::icon_set`, never the icon font directly), a description in `docs/ARCHITECTURE.md`, and its line taken off `docs/parking_lot.md` if it was listed. Recent examples to copy: `join.rs`, `offset.rs`, `edges.rs`.

## Conventions
- Entities are referred to by `EntityId`, never list position. Arc entity is CCW radians. Polyline `bulges` is empty or one per point (`bulges[i]` shapes segment i to i+1); always use `opendraft_core::bulge`, never assume straight segments.
- No `unwrap`, `todo!`, `dbg!`, prints in production code; `unsafe` forbidden (tests may unwrap).
- Files stay under about 400 production lines; `opendraft-app/tests/file_sizes.rs` enforces 450 and lists `dxf/read.rs` as still too big (it may not grow).
- Keep the family consistent: when you change lints, CI, licence lists or policy files, make the same change in the siblings and in `../OpenAppTemplate` (the scaffold new apps start from).
- `.odraft` has a development phase and a stable phase (see the module notes in `opendraft-files/src/native.rs`). Now it is development: `CURRENT_VERSION` is 0 and schema changes (new entity kinds, renamed fields) are made in place with NO version bump and no conversion. Never raise `CURRENT_VERSION` as part of a feature; setting it to 1 is a deliberate release step the user takes when the format is declared stable. Only from then on: a new entity kind bumps it (unknown kinds are refused, never skipped), an optional field does not, and each older version gets a conversion.
- Use `text::plural` for counts. Match the surrounding code's comment density and naming.

## Checks (CI runs these on Windows and Linux)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Gate commits on the real exit codes (don't pipe cargo into grep and then commit).

## Working with the user (James)
- Commit as James <133605871+JamesHodgkins@users.noreply.github.com> with NO Co-Authored-By trailer and never list Claude as a contributor.
- `main` is protected by a ruleset (pull request required, no force-push, no deletion) and Claude is not a bypass actor: work on a feature branch, push it when asked, and open a PR; the `check` jobs must pass before it merges. Push only when asked; the user usually says yes after each finished feature. Stage with `git add -u` plus explicitly named new files, never `git add -A`; check `git status` first.
- Keep `docs/ARCHITECTURE.md` and `docs/parking_lot.md` (and `docs/ROADMAP.md`'s next steps) up to date in the same change as a feature: describe what was built, take built items off the parking lot, and add what the change leaves out and any check it could not make (its "Checks still owed"). The pull request description carries the story: why it was built that way, what was tried by hand, and measurements; a budget or limit that later code must respect goes in `docs/ARCHITECTURE.md`.
- Follow AutoCAD behaviour and wording for commands unless told otherwise. If a selection exists when a command starts, skip its first selection stage.
- Finish with a short report: what changed, what is not done, and what to take next. Do not start new feature work without confirmation.

## Tooling notes (Windows)
- Shell is Git Bash/PowerShell. Avoid two heredocs in one Bash call; for multi-line Python edit scripts use the Write tool. Files may be CRLF in the working tree (git normalises); edits via the Edit tool work.
- Lucide icon font: `crates/opendraft-ui/assets/fonts/lucide.ttf`; find a glyph's codepoint with fontTools (`getBestCmap`).
- Testing in the real app: `cargo build -p opendraft-app` (always rebuild first), then drive the exe with PowerShell (launch, keys, clicks, screenshots). A click on a selected object's grip starts a grip edit, so pick selection points away from grips.
- Unit tests drive commands with `AppState`: `start_command`, `settle_command`, `answer_object`, `answer_enter`, `select_all` (see `edges.rs` and `join.rs` tests).

## Known gaps (details in `docs/parking_lot.md`)
Lengthen for ellipses and splines, PEDIT's Fit, Spline, Jointype and vertex Tangent, associative arrays (ARRAY makes plain copies), spline edits (Trim, Extend, Break, Join, Offset, Fillet, closed creation, tangents, SPLINEDIT; and so ellipse offset), ellipse Isocircle/PELLIPSE, JOIN of arcs across gaps and its Close option (lines on one line already join across gaps), text, dynamic blocks, block attribute extras (more data fields such as CreateDate and SaveDate, document properties beyond the six `DWGPROPS` summary ones (and none in DXF or DWG files), DXF `FIELD` objects, multiline attributes, `ATTDISP`, Properties rows and `BATTMAN`; attributes and blocks in DXF are checked with ezdxf only, not AutoCAD), hatch gap tolerance, Draw boundary, Reassociate, writing hatch associations to DXF, drawn gradients and `.pat` loading, dimensions (done apart from: DWG import, colours/linetypes/lineweights in styles, arrowheads other than closed, open, tick and none, `ARC_DIMENSION` and jogged radius as real DXF (written as pictures), DIMBREAK, DIMINSPECT, `DIMASSOC` in DXF, annotative scales; the DXF we write for dimensions has not been opened in AutoCAD; order and detail in `docs/parking_lot.md`, "Dimensions"); multileaders (`MLEADER` and its styles are baseline: DXF is written to AutoCAD 2018's layout but not yet opened in AutoCAD, no DWG multileaders, legacy `LEADER` is baseline (straight, text or none; it does not follow its text; no DWG), one leader line, no overrides, block content, spline leaders or MLEADEREDIT/ALIGN; see `private_material/leader_and_mleader_specification.md`); Fillet/Chamfer next to arc segments; Fuzz for Join; fillet/chamfer variables are per user, not saved in the drawing; DXF import reads TEXT, MTEXT and the STYLE table but skips anonymous blocks, DWG import reads TEXT, MTEXT, block attributes and the STYLE table but skips hatches, dimensions, multiline attributes, attributes of inserts on layouts and MTEXT in columns; DXF and DWG import skip fit-point-only splines with end tangents or not cubic; DXF writes R12, or R2000 for layouts, blocks, ellipses, splines, hatches and text; DWG is import only (no DWG export) and skips layouts; the custom coordinate system (`CCS`, AutoCAD's UCS) does not yet reach DIMORDINATE, hatches or MLEADER, DWG reads only the Model tab's UCS, and the view cannot turn (no PLAN); Linux tested only on Linux Mint.
