# Parking lot

Everything known to be missing, unfinished or left for later, by area, and the checks still owed on what is built. `docs/ROADMAP.md` decides what comes first; `docs/ARCHITECTURE.md` says how what is built works (several of its sections end with their own "Not built" notes, pointed to below).

Keep it current: when something here is built, delete its line in the same change; when a change leaves something out, add it here. The reasoning behind items gathered before 21-11-2026 is in `docs/history.md`. The larger items have a plan of their own at the end. Paths under `private_material/` are the maintainer's own notes and specifications, which are not in the repository.

## Drawing commands

- Blend Curves, gradient hatches from the ribbon, Table and Table Style, Groups (all of the Groups panel), Radius, Angle, Volume and ID Point, Calculator, Isolate and Unisolate, Layer States, and the Base view items: shown dimmed on the Home tab (`NOT_BUILT` in `opendraft-app/src/ribbon_def/home.rs`, kept true by a test). The Arc split button lists only 3-Point and Start, Center, End; AutoCAD lists eleven forms (the ARC command reaches them through its options).
- Ellipse: Isocircle (needs isometric snap) and `PELLIPSE`.
- Spline: closed and periodic creation, end tangents, a fit tolerance above 0, the Object option, SPLINEDIT and multifunction grips.
- Boundary: the Region object type, the dialog's Boundary set and Advanced options (gap tolerance, Object Type), and a polyline that follows an ellipse or spline exactly.
- Construction lines in blocks (refused: they have no size to give a block); writing points, construction lines and rays through the automation API.
- Divide and Measure: the Block option's attribute handling.

## Modify commands

- Spline editing: exact Trim, Extend, Break and Join (knot insertion, curve crossings), Offset (and with it ellipse offset) and Fillet.
- Lengthen for elliptical arcs and splines; a remembered delta as the default.
- PEDIT: Fit, Spline, Jointype and vertex Tangent (and `PLINECONVERTMODE`).
- Associative arrays (ARRAY makes plain copies): an array object edited as a whole, ARRAYEDIT, a contextual ribbon tab, grips; also Base point, Expression, Levels, the rows of polar and path arrays, Tangent and Z direction.
- JOIN: arcs across gaps and the Close option (lines on one line already join across gaps), a Fuzz distance.
- Fillet and Chamfer: next to arc segments of a polyline, a line with a polyline end segment, parallel lines (AutoCAD draws a semicircle), Chamfer's Method option, Undo inside Multiple; the fillet and chamfer variables are per user, not saved in the drawing.
- Trim and Extend: the Edge option and Crossing selection of the objects.
- Offset: the Layer and Multiple options.
- Break: `@` alone for "same as the first point".
- Explode: hatches.
- Polyline widths: Trim, Break, Join, Offset, Fillet, Chamfer and Explode give a hairline polyline (only transforms, stretching, grips and Extend keep widths).
- Draw Order (bring to front, send to back, above and under objects) and object isolation (isolate, hide, end isolation): greyed in the context menu.
- Overkill: comparing polyline segments with loose lines and arcs, redirecting associations to the survivor, a ribbon button, AutoCAD conformance fixtures.
- Match Properties: the Settings option and dialog, window or crossing selection at the destination prompt.
- Grips: AutoCAD's hover menu and the middle-grip options (Add or Remove Vertex, Convert to Arc).

## Selection, snapping and input

- Object snap and tracking: the leftovers listed under "Not built" in `docs/ARCHITECTURE.md`'s object snap section (pausing to take back a tracking point, `POLARMODE` bit 1, Extension past the ends of polylines and splines, `OSOPTIONS`, `OSNAPZ`).
- `ORTHOMODE` saved in the drawing (it is a user setting today).
- Snapping to the shape being drawn: ELLIPSE's first axis and a CV spline's control frame are not passed.
- Window and lasso selection style: the command line does not say "Specify opposite corner", the lasso does not show its kind by the pointer, no option to start a lasso only from empty space, no highlight of what the box or lasso would take.
- Transparent commands (`'Z` inside another command) and Ctrl+Z routed to a command's own Undo option.
- ZOOM: All, Center, Dynamic, Scale and a typed scale; ribbon items for ZOOM Window, Objects and Previous; a typed alias for Zoom Out (its `ZO` went to Zoom Objects); Previous for the views of a layout viewport.
- Measuring: Radius, Angle and the MEASUREGEOM wrapper, hatch and region areas, the DISTANCE, AREA and PERIMETER variables, area units such as square feet.
- Find (and replace), greyed in the context menu.
- Power Select: answers at the command line, colour by name or number, attribute properties.

## Text and fonts

- MTEXT editor: a font menu in the toolbar (the style menu is built), symbols, the ruler's tab kinds (centre, right, decimal) and right indent, distributed alignment, paragraph spacing kinds, lists beyond the basics (renumbering, continuing, nesting), automatic stacking and stack options, find and replace, spell check, columns, `-MTEXT`, and a look at the Paragraph dialog in the running app. Several of these wait for AutoCAD-made samples of their codes.
- Text bounds, snapping, grips and picking lay text out in the `Standard` style, so a style with another width factor or slant is picked slightly off its drawn size; a style-aware `Document::object_bounds` is the fix.
- Installed system fonts: text uses the four bundled DejaVu faces (regular, bold, italic, bold italic) and OD Roman.
- TEXT: a Contents row in Properties, TEXTEDIT on attribute definitions and dimensions, a ribbon item, an editor box readable at any zoom.
- Text styles: Annotative, renaming, a `TEXTSTYLE` variable.
- OD Roman font (`private_material/od_roman_font_specification.md`): the PDF hidden text layer, the DXF round trip (`RomanS.shx` plus OpenDraft XDATA), and the glyphs still to draw: `` $ % / : < > @ [ \ ] ^ ` { | } ~ ``, `Ø`, `±`, `×`.

## Dimensions

In the order to take them (worked out 23-10-2026):

1. DWG dimension import (the reader knows the object types but skips them; needs the DIMSTYLE table object too). Fold in a DXF writer fix: write code 42 (the measurement) and bit 32 of 70, as AutoCAD does.
2. Colours, linetypes and lineweights (`DIMCLRD`, `DIMCLRE`, `DIMCLRT`, `DIMLWD`, `DIMLWE`, `DIMLTYPE`): a dimension's picture has no per-piece properties yet.
3. The standard arrowhead set (dot, box, datum, integral, oblique, open 30 and 90...), then arrowheads from the user's own blocks.
4. `ARC_DIMENSION` and jogged radius as real DXF (both are written as pictures today).
5. DIMBREAK and DIMINSPECT.
6. `DIMASSOC` in DXF (files lose the ties; needs a handle for every entity written).
7. Annotative scales.

Also: Override and Compare in the style manager, the angle between two lines as an associative tie, angular dimensions' Quadrant option and tolerance and alternate text, baseline and continue from angular and ordinate dimensions, DIM's other options, the MTEXT editor for DIMEDIT New, and a style update that follows into dimensions inside blocks. Nothing the DXF writer emits for dimensions has been opened in AutoCAD.

## Leaders and multileaders

- MLEADER: DWG import, Properties editing, block content, several leader lines, spline leaders, MLEADEREDIT, MLEADERALIGN and MLEADERCOLLECT, overrides, annotation scale, ByBlock. The DXF follows AutoCAD 2018's layout but has not been opened in AutoCAD.
- LEADER (legacy): follows its text, spline, tolerance and block annotations, Format, DWG import.

## Hatches

- Gap tolerance, Draw boundary, separate hatches, retained boundaries, exact arcs in traced loops.
- Reassociate, Recreate Boundary, loop grips, writing associations to DXF, remapping on copy, `PICKSTYLE` selection.
- Gradients drawn (see the plan below), transparency, background colour.
- The rest of `acad.pat`/`acadiso.pat` and loading user `.pat` files.
- DWG import of hatches.
- Hatches inside blocks: a block cannot hold one yet (the table refuses it), nor annotative hatches, non-XY planes or nonuniform scale.

More in `docs/ARCHITECTURE.md`, "Hatches".

## Blocks, attributes and fields

- Dynamic blocks (parameters, actions, constraints); in the Block Editor: BASE, Test Block, REFEDIT.
- Block settings: annotative, uniform scaling, allow exploding, a block unit (also from DWG); insertion units scaling.
- Attributes: multiline attributes, `ATTDISP`, BATTMAN, Properties rows, the ribbon's Edit Attributes > Multiple, picking height, rotation and width on screen from the ATTDEF dialog, double-click to edit an insert, extraction, DWG carrying an attribute's own properties.
- Fields (built: Date, Filename, Object and the six `DWGPROPS` summary fields, through `FIELD`, the text editor and `-FIELD`): CreateDate, SaveDate, LastSavedBy, PlotDate, file size and custom document properties; document properties in DXF and DWG (neither is read or written); DXF `FIELD` objects; fields refreshed inside nested blocks; fields in dimension and multileader text; `FIELDDISPLAY`.
- Attributes and blocks in DXF are checked with ezdxf only, not AutoCAD.
- Picture cache for block references (not worth it on the evidence so far: see `docs/ARCHITECTURE.md`).
- External references (xrefs).

## Layers, linetypes and properties

- Current colour, linetype and lineweight: the `CECOLOR`, `CELTYPE` and `CELWEIGHT` names, ByBlock, saving them in DXF and DWG headers; pasting and inserting still use the layer only.
- Linetypes: `.lin` loading and what follows (see the plan below).
- Layer tools: LAYISO and LAYUNISO, Layer States, LAYFRZ's Settings (it freezes in every viewport), LAYOFF and LAYLCK without asking; a filter or search in the Layers panel.
- The layer plot flag (AutoCAD's Plot / No Plot column, DXF group 290): not kept, so a non-plotting layer read from a file is plotted and is written back as plotted (only Defpoints is written not plotted). Objects on Defpoints are plotted too, though AutoCAD never plots them.
- Properties panel parity with AutoCAD, kind by kind: `private_material/properties_gap.md`.
- Colours: the Color Books tab, recent colours kept across sessions, greys 250 to 255 that follow the background.
- Freeze in the Model tab only: no command for it (only the Layers panel and the ribbon's layer list), and Home-page thumbnails still draw those layers.

## Layouts and plotting

- Non-rectangular viewport clips (see the plan below); rotated, twisted and perspective viewports; new viewports taking a layer state.
- The LAYOUT Set option.
- Layout colours beyond the paper: AutoCAD's Drawing Window Colors sets each part (crosshair, grid, snap marker and the rest) per context; OpenDraft chooses them to suit the paper, and only the paper (light or dark, and light paper's colour from six presets; no colour of one's own, which would need a picker limited to light colours) is the user's to pick. Colours are drawn as they are, so pale ones such as yellow (colour 2) are hard to read on light paper of any colour, as in AutoCAD; a screen-only option that darkens very pale line colours on light paper would fix that, at the cost of the sheet no longer previewing the print (issue #192). Model space's background colour is the palette's, not the user's to pick (suggested in #192). A viewport that runs off the sheet draws its contents over the dark surround there, where black lines on light paper all but vanish (AutoCAD's surround is grey); the part off the sheet does not plot.
- The Layout tab's other AutoCAD items: viewport Clip, Lock as a Lock/Unlock list, and the Create View, Modify View, Update and Styles and Standards panels.
- Display and named View plot areas (no saved view state).
- Plotting: `-PAGESETUP`, cancel and progress in the dialog, the last printer remembered, margins and media from the printer, a Page Setup Manager with named setups, collation, plot-style tables; proof on real printers and cutters (`docs/ROADMAP.md`, Phase 3).
- Annotative scales, and `PSLTSCALE`/`MSLTSCALE`.
- Check the R2000 export in AutoCAD (only ezdxf so far).

## Files

- DXF import: anonymous blocks (references to them are skipped and counted, as dynamic blocks are).
- DXF export writes no `FIELD` objects and no hatch associations. (It writes R12, or R2000 when a drawing has layouts, blocks, ellipses, splines, hatches or text.)
- DWG: import only (no export), and import skips layouts, hatches, dimensions, multiline attributes, attributes of inserts on layouts and MTEXT in columns.
- DXF and DWG skip fit-point-only splines with end tangents or not cubic.
- View state (zoom) saved in the file, a warning when the file changed on disk, revert to saved, smaller files for very large drawings, a progress bar for large files, recovery files loaded in the background.
- `.odraft` declared stable: `docs/ROADMAP.md`, Phase 5.

## Interface

- Ribbon: responsive collapse when the window is narrower than the ribbon, pinning a flyout open, contextual tabs for a selected object (only the Block Editor and layouts have contextual tabs), Alt-key hints.
- Command line: history kept between sessions and answers typed inside a command recorded, a user-editable alias table, user-rebindable shortcuts, icons in suggestions.
- Command line strip: dragging the grip straight to a new place (it floats first), a settings button on the grip, Ctrl+9 reopening it where it was.
- Docking: a target above the drawing, docking along the whole window's edge; a panel that shared a floating window returns to it only while another of its panels is still open there.
- Context menu: while a command runs (right click is Enter), right-clicking an object to pick it first, keyboard navigation.
- Clipboard: plain text (as MText) and pictures from other programs, layouts and viewports, a prompt when a same-named definition differs.
- Home page: arrow keys between cards, bundled templates with a sheet layout, several template folders.
- Screen-reader support: eframe's `accesskit` feature is off (root `Cargo.toml`) because its Windows adapter panicked when the dock rearranged panels (accesskit 0.26 with egui 0.31). Try again after an egui upgrade.
- Crosshair in the Block and text editors' own surfaces.
- Settings > Files: search paths for fonts, hatch patterns and linetypes, a plot output folder.
- Units: Direction Control's Pick button, areas in non-decimal units, a unit-aware grid spacing.
- Icons: reloading while the app runs, recolouring for a light theme; the Layout tab's six icons (`assets/icons/layout-*.svg`) are Lucide's outlines, not yet redrawn in the Home tab's style.
- Purge: a confirmation window, previews beyond blocks.
- Crash log: telling the user, when it happens, that a crash was recorded.

## Assistant and automation

- The assistant: measurements beside the picture after a change, reference data and skills as files, fillet and chamfer construction steps, tidying its own stray shapes within a plan, streaming replies, chats saved between runs, a benchmark of tasks. Plans and decisions: `private_material/mcp_harness_roadmap.md`.
- MCP: elicitation for scalar and choice prompts, subscriptions or a long poll instead of polling, view-change events, entity-level change lists, the assistant's own name in Settings, Streamable HTTP (deliberately left: needs OAuth, TLS and a threat model).
- Drawing audit: standards beyond layer names (linetype, colour, text style and height), a "show me" action that selects findings.
- Rendering for the model: the canvas's own look, a layout sheet at a chosen DPI, a picture cache keyed by revision.

## Releases and platforms

- Linux on real machines, Flathub and the AUR: `docs/ROADMAP.md`, Phase 1, and `docs/linux.md`.
- Windows code signing (MSI and exe), an icon and version information for `opendraft-mcp.exe`, an upgrade over a zip install, per-machine install.
- The licence notices shown inside the app; notices for the Linux system libraries loaded at run time.
- Update checks: "Check now" while checks are off, a beta channel opt-in.
- Whether `ring`'s licence (ISC with an OpenSSL-derived part) sits well with GPL-3.0-or-later: check before relying on it.

## Code

- `dxf/read.rs` is over the file-size limit (listed in `opendraft-app/tests/file_sizes.rs`; it may not grow): split it into tokenizer, tables and entities.
- Document mutators (`push`, `insert`, `push_layer`, `replace_attrs`) return `Result`, so release builds also reject a bad edit at the call.
- Performance detail settings (planned in the entity layer cache work, phases 2.6 and 2.7): the skip-tiny-objects threshold (`MIN_DRAW_PX` is a constant), curve smoothness, anti-aliasing, small-text and dense-hatch thresholds; a margin around the kept picture so a fast pan shows no blank strip.
- Performance leftovers: a dimension's or insert's picture is rebuilt every frame it is on screen (2,000 dimensions in view: 14 ms), dashes are rebuilt every paint, and the spatial index's own leftovers (`docs/ARCHITECTURE.md`).

## Checks still owed

Built and covered by tests, but not yet seen working where it matters. Take a line off when it has been checked, and say what was tried in the pull request.

- **In AutoCAD** (open, `AUDIT`, edit, save): the Model-tab freeze's XDATA on LAYER records (it should survive a save), everything the DXF writer emits for dimensions, multileaders, legacy leaders and block attributes, ByBlock, and the R2000 layout export; the DWG reader's object counts against an AutoCAD listing; the ellipse angle convention and the spline prompts; MTEXT tracking at the end of a line; the rectangular array's default spacing; the multileader style's angle constraints and attachment codes.
- **By hand in the real app, the custom coordinate system:** tried 22-11-2026 in the Model tab only (UCS, Rotate 30, the turned grid, axes, icon and crosshair, and LINE with `@100,0` and `@0,50`). Not yet tried: inside a layout viewport, ortho, polar and object snap tracking and grid snap by hand, the dynamic input fields, Object and View, ROTATE and grip rotate, and the commands that orient what they make.
- **By hand in the real app** (tests drive the commands through `AppState` only): DIMEDIT and DIMTEDIT, TEXTEDIT and TEXT typed in place, DIST and AREA, Overkill and Purge, ByBlock, the attribute editor's double-click and the ATTDEF dialog, the warning before a large file opens, the window and lasso selection style, snapping to a polyline while drawing it, the Ortho pointer test, the Paragraph dialog (nobody has seen it), DIVIDE, MEASURE, grips, TRIM and BREAK on points and construction lines, and whether the Settings dialog's Files and Automation pages fit its height without scrolling.
- **On real machines:** Linux desktops beyond Mint (and on Mint: the Home page's templates and file actions, the assistant's key in the Secret Service, the crash log); the Windows exe on a machine with no Visual C++ Redistributable; the MSI's shortcut and file association opening with no console window; physical printers, and CUPS versions other than the one tried; the update check against the real GitHub.
- **With a real model:** the assistant with Mistral on a real drawing, including its warning before it generates shapes (it needs your API key, and sends drawing data to the service).
- **With real files:** DWG polylines (LWPOLYLINE, 2D POLYLINE, VERTEX), rotated or mirrored entities, entities with a Z, true colours on entities and extended data (none of the sample drawings has them); the large DWG object fix against the reporter's file.

---

## Plan: non-rectangular viewport clips

Today a viewport is always a rectangle (`center`, `width`, `height` in `opendraft-core/src/layout.rs`). DXF files with other outlines import as rectangles with a warning.

Plan (medium size, about 8-10 files):

- **Model:** add `Viewport::clip: Option<Vec<Point>>`, a closed polygon on the sheet. `None` keeps the rectangle. `center`, `width` and `height` stay as the polygon's bounding box, so scale, zoom, pan and MVIEW keep working. Add point-in-polygon `contains_paper` and `Viewport::outline()`.
- **Plot (`opendraft-core/src/plot.rs`):** clip each segment against the polygon edges and keep the inside pieces (works for concave outlines). Tests: triangle, concave L, circle.
- **Screen:** egui can only clip to rectangles, so clip each flattened path against the polygon with the same function as the plot. Filled shapes (hatches) need clipping too.
- **Interaction:** picking and "inside the viewport" use point-in-polygon; draw the outline as the border.
- **Files:** `.odraft` gets an optional field (no version bump). DXF write: clip entity (340 handle) with a closed `LWPOLYLINE`. DXF read: read it instead of warning. Check against an ezdxf-built file.
- **Commands:** VPCLIP (clip with a closed polyline or circle; Delete option goes back to a rectangle).

Suggested order:

1. Clipping end to end: model, screen, plot, picking, `.odraft`, DXF, VPCLIP.
2. Later: MVIEW Polygonal and Object options, and grips to drag the outline's vertices (fiddly, least needed).

Not included: a clipped viewport inside a nested viewport.

## Plan: custom coordinate system and view rotation

AutoCAD's UCS under our own name: the **custom coordinate system** (`CCS`, alias `UCS`). Decided with James on 22-11-2026: the Model tab and each layout viewport have their own, never affecting each other; a layout's sheet has none; named systems are shared; every change is an undo step and is saved in the drawing; the automation API stays in world coordinates. What is built is in `docs/ARCHITECTURE.md`, "Custom coordinate system".

Left for pull request 1:

- **DIMORDINATE from the system's origin.** `DimKind::Ordinate` stores only the feature, leader end and datum, measured from the world origin along world axes. It needs the origin and direction it measures from (DXF keeps the origin in group 10 and the direction in group 51), in `.odraft`, the picture (the leader runs along the system's axes), DXF read and write, and DIMORDINATE.
- **Hatch angle and origin** in the system (AutoCAD measures `HPANG` from the system's X axis and puts the default origin at its origin).
- **MLEADER** text level with the system.
- **DXF**: the header (`$UCSORG`, `$UCSXDIR`, `$UCSYDIR`, `$UCSNAME`, and the paper space `$PUCS*` as world), the `UCS` table for the named systems (written empty today), each `VIEWPORT` entity's own system (groups 110, 111, 112, 79); read and write. **DWG**: read the same.
- Polyline and rectangle Object rule: ours takes the segment nearest the pick; AutoCAD takes a polyline's first segment. Kept on purpose; say so if users expect AutoCAD's.

Pull request 2, view rotation: the camera turns (painting, picking, caches, layout viewport twist, plotting); a command to rotate the view and a gesture (not chosen yet); PLAN; View then lines up with a turned view (today it takes the corner of an unturned view).

Later: `UCSICON` settings (the icon cannot be switched off or moved to the origin), a manager dialog (`UCSMAN`), dragging the icon by its grips, the dimension arcs of the dynamic input drawn along a turned system's axes (the Cartesian legs are still screen-aligned), the systems through MCP.

## Plan: hatch gradients

Gradient hatches are read, kept and written (`HatchFill::Gradient`: raw name, angle, shift, one or two colours, tint; DXF 450-470 tail) but drawn as one flat colour, the first stop (`opendraft-canvas/src/view/hatch_paint.rs`, `solid_color`). A solid hatch that carries a gradient tail with `450 = 0` loses the tail on import.

Plan (medium size, mostly in `opendraft-canvas`):

- **Colour per vertex:** the solid mesh (`hatch_cache.rs`, `SolidMesh`) is already triangulated relative to the hatch's middle. Add a gradient coordinate `g` per vertex from the hatch's extent and the gradient angle, and colour each vertex from it. egui interpolates linearly between vertices, so a coarse mesh draws curved profiles badly: subdivide the triangles (or add a grid of interior points) when the kind is not linear.
- **Kinds to draw:** LINEAR first (two colours, `shift` moves the centre, one-colour gradients blend toward white or black by `tint`), then CYLINDER, INVCYLINDER, SPHERICAL, INVSPHERICAL, HEMISPHERICAL, INVHEMISPHERICAL, CURVED, INVCURVED. The specification says not to map them all to linear. The profiles need AutoCAD output to calibrate (see below); until then they should say they are approximate.
- **Plot:** `PlotFill` is one flat colour. Draw a gradient as bands (several fills) or write the first colour and note it.
- **Files:** keep a `450 = 0` tail (and `GradientColor1ACI`/`GradientColor2ACI` XDATA) on solid hatches so a round trip is lossless; read the hatch background colour (`HPBACKGROUNDCOLOR`, XDATA) at the same time.
- **Properties and HATCH:** gradient kind, angle, shift, the two colours, tint, centred; a Gradient option in HATCH and HATCHEDIT (HATCHEDIT refuses gradients today).
- **Checking:** needs a fixture from AutoCAD with each kind at known colours, angle 0 and 45 degrees, centred and not, saved as DXF and exported as an image, so the profiles can be compared by eye and by sampled pixels. We have none yet. ezdxf can write the records but not tell us what AutoCAD draws.

Not included: transparency (needs a document-wide attribute) and mesh/gradient fills inside blocks.

## Plan: linetypes from `.lin` files, and what follows

The table, dashed drawing, scales, DXF/DWG patterns and the Linetype Manager are built (see `docs/ARCHITECTURE.md` and `private_material/autocad_linetype_reference_guide.md`, which is the rulebook). Load in the Manager offers only our stock library. Left for later:

- **`.lin` loading (next, self-contained):** a pure parser in `opendraft-core/src/linetype/lin.rs` (header `*NAME,description`, pattern line `A,len,len,...`, `;` comments, 2 to 12 elements, first not a gap, 47-character description, names unique ignoring case) that returns records or per-entry problems, tested on text only. Then a file picker on the Manager's Load button (list the file's linetypes, pick, load; an existing name is replaced, as AutoCAD's reload does) and a `-LINETYPE` Load option. Test against the user's own `acad.lin` and `acadiso.lin`, which also settles the metric and ISO numbers (the guide still marks those as checked only against a 2017 snapshot). Complex entries (`["GAS",STANDARD,S=.1,...]`, shapes from `.shx`) are read as their dashes and reported, as DXF import does. Export (`.lin` out) and Reload/Rename in the Manager belong here too.
- **Defaults for new objects:** `CELTSCALE` and `PLINEGEN` (today new objects have scale 1 and generation off).
- **ByBlock leftovers:** ByBlock in the DXF/DWG layer and style tables, MLEADER styles that apply it, dimensions' own colour/linetype/lineweight, and `CECOLOR`/`CELTYPE`/`CELWEIGHT` taking ByBlock.
- **Complex linetypes:** text and shape elements are dropped (drawn as plain dashes with a warning). Keeping them for a round trip needs a decoration on each pattern element (`ComplexDecoration`, raw flags) and DXF groups 74/75/340/46/50/44/45/9; drawing them needs the text and shape engines. DWG decodes the complex flag but not the extra fields.
- **Not yet dashed:** block contents, dimensions and hatch outlines draw solid (`HPLINETYPE` governs hatches in AutoCAD).
- **Scale rules:** `PSLTSCALE` and `MSLTSCALE` (need annotative scales); DWG polyline Linetype Generation flag (LWPOLYLINE flag 256) is not decoded.
- **Fitting arithmetic:** how end dashes take up slack, and how a closed shape is phased, is our own choice meeting AutoCAD's documented rules. Lock it down with AutoCAD fixtures at several lengths (lines, arcs, circles, closed polylines), as the guide's acceptance tests describe.
- **Performance:** dashes are rebuilt every paint; cache per object if a large dashed drawing feels slow (the hatch cache is the model).

## Note: automation endpoint name

On Windows the automation endpoint is a named pipe called `od-<instance id>` (`opendraft-app/src/automation/host.rs`, `endpoint_for`); on Linux it is a `<instance id>.sock` file, so the prefix does not appear there. Clients read the endpoint from the instance descriptor rather than building the name, so a rename touches `endpoint_for` and the fake endpoint in `opendraft-mcp/tests/support/mod.rs`. It is a runtime name, not a file or crate name, so the family naming rules did not require changing it.
