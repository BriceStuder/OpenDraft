# Parking lot

Everything known to be missing, unfinished or left for later, by area, and the checks still owed on what is built. `docs/ROADMAP.md` decides what comes first; `docs/ARCHITECTURE.md` says how what is built works (several of its sections end with their own "Not built" notes, pointed to below).

Keep it current: when something here is built, delete its line in the same change; when a change leaves something out, add it here. The reasoning behind items gathered before 21-11-2026 is in `docs/history.md`. The larger items have a plan of their own at the end. Paths under `private_material/` are the maintainer's own notes and specifications, which are not in the repository.

## Drawing commands

- Blend Curves, gradient hatches from the ribbon, Table and Table Style, Groups (all of the Groups panel), Radius, Angle, Volume and ID Point, Calculator, Isolate and Unisolate, Layer States, and the Base view items: shown dimmed on the Home tab (`NOT_BUILT` in `opendraft-app/src/ribbon_def/home.rs`, kept true by a test). The Arc split button lists only 3-Point and Start, Center, End; AutoCAD lists eleven forms (the ARC command reaches them through its options).
- LINE SINGLE and LINE MIDDLE (SL, ML): within either one the way of drawing cannot be switched.
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
- Fillet and Chamfer: next to arc segments of a polyline, a line with a polyline end segment, parallel polyline segments, rays and construction lines (two parallel lines get AutoCAD's half circle), Chamfer's Method option, Undo inside Multiple; the fillet and chamfer variables are per user, not saved in the drawing.
- Trim and Extend: the Project and eRase options, and a typed Crossing keyword at the object prompt (a click in empty space already starts the crossing window). The Edge option's implied extension covers lines, arcs, elliptical arcs and the ends of open polylines; other edges (splines, text) are used as they are.
- Offset: the Layer and Multiple options.
- Explode: hatches.
- Polyline widths: Trim, Break, Join, Offset, Fillet, Chamfer and Explode give a hairline polyline (only transforms, stretching, grips and Extend keep widths).
- Draw Order (bring to front, send to back, above and under objects) and object isolation (isolate, hide, end isolation): greyed in the context menu.
- Overkill: comparing polyline segments with loose lines and arcs, redirecting associations to the survivor, a ribbon button, AutoCAD conformance fixtures.
- Match Properties: the Settings option and dialog, window or crossing selection at the destination prompt.
- Grips: AutoCAD's hover menu and the middle-grip options (Add or Remove Vertex, Convert to Arc).

## Selection, snapping and input

- Object snap and tracking: the leftovers listed under "Not built" in `docs/ARCHITECTURE.md`'s object snap section (pausing to take back a tracking point, `POLARMODE` bit 1, Extension past the ends of splines, `OSOPTIONS`, `OSNAPZ`).
- Snapping to the shape being drawn: ELLIPSE's first axis and a CV spline's control frame are not passed.
- Window and lasso selection style: the command line does not say "Specify opposite corner", the lasso does not show its kind by the pointer, no option to start a lasso only from empty space, no highlight of what the box or lasso would take.
- Transparent commands (`'Z` inside another command) and Ctrl+Z routed to a command's own Undo option.
- ZOOM: All, Center, Dynamic, Scale and a typed scale; ribbon items for ZOOM Window, Objects and Previous; Previous for the views of a layout viewport.
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

- Dynamic blocks: the data slots and the evaluation of a linear parameter with Move and Stretch, with its end grip, exist in core (`BlockDef::dynamic`, `Insert::dynamic`, `block/dynamic_eval.rs`; see `docs/ARCHITECTURE.md`, "Blocks"), and `.odraft` saves and reads them, but nothing can author one or show it in Properties, DXF export writes each reference as an ordinary block of its current shape, with a note, and no DXF or DWG keeps the parameters (MI3's write half), and the grip is drawn like any other. Authoring exists (`BPARAMETER`, `BACTION`, `BACTIONSET`, `RESETBLOCK`: see `docs/ARCHITECTURE.md`) but not: editing or deleting a parameter or action after it is made (BPARAMETER and BACTION only add; change one by `BACTIONSET` for its objects, or undo), the choice of an action's base point, remembering the last choices (`dynamic_block_memory`), a Midpoint base behaviour or label location, and asking before BSAVE changes references (it reports instead). Properties shows the values (a numeric field even for a list or range: no drop-down, and Power Select does not list them). A rotation parameter with the rotate action, and a flip parameter with the flip action, a scale action on a linear parameter, and a polar parameter (a point at a distance and an angle) with the stretch, move, scale and rotate actions on it, an XY parameter (a corner at a horizontal and a vertical distance) and a point parameter (a location) with the move and stretch that follow them, are built (M6, first five slices). A visibility parameter, with named states that each hide some objects, is built too (M7, first slice): `BPARAMETER` Visibility, `BVSTATE`, `BVHIDE` and `BVSHOW`, a drop-down in Properties, the native file and DXF import (a block's other states come in with the objects AutoCAD wrote as invisible). Not built for it: the Block Editor showing only the current state's objects (it shows every object and each command names its state, and a new object is shown in every state), the visibility grip and its menu (AutoCAD's lookup-style grip; states are picked in Properties), ribbon buttons and icons for `BVSTATE`, `BVHIDE` and `BVSHOW`, reordering states, nested visibility and visibility in Power Select. A lookup parameter (a table of named rows, each giving values to input linear, rotation or flip parameters) is built too (M7, second slice): picking a row sets the inputs, and the row shown is the one the inputs match, or "Custom". It is a different shape from AutoCAD's (the table is in the parameter, not in a lookup action; nothing is stored in a reference; its inputs are one or more linear, rotation or flip parameters, never a polar, XY or point parameter, and there are no output-only columns for attributes, so a lookup that sets an attribute is left out on import). `BPARAMETER` Lookup and `BLOOKUPTABLE` (Add, Delete, List) author it; there is no table dialog, no ribbon button and no lookup grip, and a lookup is not edited after BPARAMETER except by its rows (inputs cannot change). Still to build, in the order of the plan: the rest of the vocabulary (array, alignment, base point), a linear parameter's start grip and Midpoint base behaviour, a rotation's own base type for its action (the action always turns about the parameter's base point), constraints and user parameters. A rotation's zero direction is the block's +X; the parameter's grip is drawn there whatever way AutoCAD's pointed. In the Block Editor: BASE, Test Block, REFEDIT.
- Block object ids (`BlockEntity::id`) are not saved in `.odraft`: reading numbers a definition's objects in order. A block edited with BEDIT can have ids that are not 0, 1, 2... in memory (a new object takes the editor's next id), so it does not equal its own reloaded copy until the file keeps the ids.
- Block settings: annotative, uniform scaling, allow exploding, a block unit (also from DWG); insertion units scaling.
- Attributes: multiline attributes, `ATTDISP`, BATTMAN, Properties rows, the ribbon's Edit Attributes > Multiple, picking height, rotation and width on screen from the ATTDEF dialog, double-click to edit an insert, extraction, DWG carrying an attribute's own properties.
- Fields (built: Date, Filename, Object and the six `DWGPROPS` summary fields, through `FIELD`, the text editor and `-FIELD`): CreateDate, SaveDate, LastSavedBy, PlotDate, file size and custom document properties; document properties in DXF and DWG (neither is read or written); DXF `FIELD` objects; fields refreshed inside nested blocks; fields in dimension and multileader text; `FIELDDISPLAY`.
- Attributes and blocks in DXF are checked with ezdxf only, not AutoCAD.
- Picture cache for block references (not worth it on the evidence so far: see `docs/ARCHITECTURE.md`).
- External references (xrefs).

## Layers, linetypes and properties

- Current colour, linetype and lineweight: the `CECOLOR`, `CELTYPE` and `CELWEIGHT` names, ByBlock, saving them in DXF and DWG headers; pasting and inserting still use the layer only.
- Linetypes: `.lin` loading and what follows (see the plan below).
- Layer tools: LAYISO and LAYUNISO, Layer States, LAYFRZ's Settings (it freezes in every viewport), LAYOFF and LAYLCK without asking; a filter or search in the Layers panel; giving a resized name column back its "fill the room" width (only fitting it to its names), and resizing the other panels' columns.
- The layer plot flag: no switch for it in the ribbon's layer list, and no command sets it (AutoCAD has none either, beyond `-LAYER`'s Plot option). A block inserted inside another block on a No Plot layer still plots (only the top-level insert's layer is checked).
- Properties panel parity with AutoCAD, kind by kind: `private_material/properties_gap.md`.
- Rectangular polylines: corners can be rounded or bevelled only when all four are alike, and a bevel only with the same distance along both sides (RECTANGLE's Chamfer allows two); the Radius and Chamfer rows show only on corners already rounded or bevelled, so square corners cannot be rounded from Properties, nor rounded corners bevelled. The oblong (slot) is recognised only as four vertices (not with a straight side split in two); oblongs take no line width, and their width has no remembered default. RECTANGLE still makes a `Rectangle` object when the rectangle is level, square-cornered and hairline; DXF writes it as a polyline, so it comes back as a rectangular polyline.
- Colours: the Color Books tab, recent colours kept across sessions, greys 250 to 255 that follow the background.
- Freeze in the Model tab only: no command for it (only the Layers panel and the ribbon's layer list), and Home-page thumbnails still draw those layers.

## Layouts and plotting

- Non-rectangular viewport clips (see the plan below); rotated, twisted and perspective viewports; new viewports taking a layer state.
- The LAYOUT Set option.
- Layout colours beyond the paper: AutoCAD's Drawing Window Colors sets each part (crosshair, grid, snap marker and the rest) per context; OpenDraft chooses them to suit the paper, and only the paper (light or dark, and light paper's colour from six presets; no colour of one's own, which would need a picker limited to light colours) is the user's to pick. Colours are drawn as they are, so pale ones such as yellow (colour 2) are hard to read on light paper of any colour, as in AutoCAD; a screen-only option that darkens very pale line colours on light paper would fix that, at the cost of the sheet no longer previewing the print (issue #192). Model space's background colour is the palette's, not the user's to pick (suggested in #192). A viewport that runs off the sheet draws its contents over the dark surround there, where black lines on light paper all but vanish (AutoCAD's surround is grey); the part off the sheet does not plot.
- The Layout tab's other AutoCAD items: viewport Clip, Lock as a Lock/Unlock list, and the Create View, Modify View, Update and Styles and Standards panels.
- Display and named View plot areas (no saved view state).
- Plotting: `-PAGESETUP`, cancel and progress in the dialog, the last printer remembered, a printer's own minimum margins (read from the system, to allow editing a printer's margins down to them; only PDF and SVG margins can be edited now) and media from the printer, a Page Setup Manager with named setups, collation, plot-style tables; proof on real printers and cutters (`docs/ROADMAP.md`, Phase 3).
- Annotative scales, and `PSLTSCALE`/`MSLTSCALE`.
- Check the R2000 export in AutoCAD (only ezdxf so far).

## Files

- DXF and DWG import: an object the file marks invisible is dropped (and counted), not kept hidden, because objects have no visibility of their own; hiding and showing by layer is the only way to keep one.
- DXF and DWG import: an anonymous block a reference places (a modified dynamic block's `*U`) comes in as an ordinary block with its picture, except a DXF reference of a dynamic block understood in full, which is rebuilt as a reference to its dynamic definition (see the next line). DWG import has no decoder: its dynamic definitions and modified references arrive as static blocks, and it does not tell a named dynamic definition apart.
- DXF dynamic blocks: import builds a dynamic block from the decoded definition for linear parameters (value lists, grips) with stretch and move actions, for rotation parameters (no list or increment of angles) with rotate actions about the parameter's base point, and for flip parameters with flip actions, for scale actions about the linear parameter's start point, for XY and point parameters with the moves and stretches that follow them, and for polar parameters (value sets: an angle increment is read, a list of distances or angles is not) with their polar stretch, stretch, move, scale and rotate actions, and rebuilds a modified reference as a reference to it with its recorded values. Not built (named in one note, the block kept as its default picture when nothing else is left): polar, lookup (its table is not imported; it does not stop references being rebuilt), visibility, rotation with a list or increment of angles, a polar parameter with a list of distances or angles, a rotate or scale action about a point of its own, array, point, XY, alignment and base point parameters, and polar stretch and lookup actions. A reference to a definition that has any of those, apart from a lookup, keeps its anonymous block's picture. A stretch frame of more than two points becomes its bounding rectangle. Decoded but not read into the app: value ranges (min, max, increment; only value lists are read). Not decoded, kept by DXF name: visibility, array, alignment and base point parameters and their actions, the Block Properties Table, and value ranges (min, max, increment; only value lists are read). A modified reference's rotation is read as the angle of its grip point from the definition's (the same two points as a linear parameter's record); the one rotated sample, `rotate_example_1` in `dyn_blocks_1`, has a turn of nothing, so a turn is not checked against AutoCAD. Whether a modified reference is flipped is not read (the sample's one flip record ends in a `70` of 0, which I took to be not flipped but could not confirm), so a reference to a definition that has a flip keeps its anonymous block's picture. Instance records for array, visibility, move, base point and alignment have not been read. DWG has no decoder at all.
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
- Navigation gizmo, level button (issue #214): working while another command runs (a transparent `'ROTATEVIEW`, which nests two point prompts) and offering to align the CCS too (it only turns the view).
- Navigation gizmo: Esc during an arrow drag putting the view back (Ctrl+middle-drag has it), a gizmo in the Block Editor and the text editors' surfaces, its size following the interface scale, and keyboard access.
- Settings > Files: search paths for fonts, hatch patterns and linetypes, a plot output folder.
- Units: Direction Control's Pick button, areas in non-decimal units, a unit-aware grid spacing.
- Icons: reloading while the app runs, recolouring for a light theme; the Layout tab's six icons (`assets/icons/layout-*.svg`) are Lucide's outlines, not yet redrawn in the Home tab's style.
- Purge: a confirmation window, previews beyond blocks.
- Crash log: telling the user, when it happens, that a crash was recorded.
- Translations (issue #138): the setup (Settings > Display > Language), the ribbon, the menus, the palettes, the status bar, the command prompts (with values in them, through `Options::worded`) and the command names (bilingual, `_LINE` always works) are built, with French. Still to wrap: the few prompts that write their options in their own text (`-PLOT`'s "[Fit]", ATTDEF's modes, `-OVERKILL`), the command-line text of the dialogs a command opens ("Block Definition", "Edit text"), the context menu's "Repeat" and Recent Input, which name a command by its label rather than its command name; the shortcut test of prompt options reads the sources, so options joined at run time are checked only through its hand-written list; the messages of the other commands (dimensions, text, blocks, layers, layouts, plot, files, hatch and the rest: step 7b), the names of undo steps after "Undo:" (translated only when the same word is a key elsewhere), the internal validation messages (spline, ellipse and polyline checks), dialogs; then the complete French file.

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

- **BEDIT and BSAVE by hand:** the Block Editor now keeps each object's block id as its id (`Document::add_with_id`) and BSAVE reads it back. Tests drive BEDIT, BSAVE and BCLOSE through `AppState` and check the ids, but nobody has used it in the running app since the change.
- **Dynamic block authoring by hand:** tried 24-11-2026 in the running app (Windows, dark theme): -BLOCK of a bar and a post, BEDIT, BPARAMETER and BACTION (Stretch) by typing, the schema overlay (orange parameter line with ticks and name, the Stretch1 label and its frame), the Dynamic ribbon panel and its icons, BSAVE and BCLOSE, selecting the reference (two grips: the insertion grip and the parameter's end grip, both drawn as an ordinary grip), dragging the end grip with the mouse (the ghost and the result shortened the bar and carried the post), undo, save as `.odraft` and reopen (the stretched reference came back stretched), and RESETBLOCK from the Insert tab. Tried again after M4: the Custom section in Properties (the reopened file showed 2.0000; typing 3.5 stretched the bar and said "Changed Distance1") and the diamond grip. Not tried: the light theme, a list or increment value set, a rotated, mirrored or scaled insert, a typed point for the grip (a typed point at a grip stretch is relative to the grip, as for any grip), Move actions, several inserts, BEDIT of a dynamic block that already has values, and a large drawing.
- **Dynamic block evaluation and export against AutoCAD:** evaluation, the grip and the DXF export were tried in the running app (see the authoring and imported-block lines), and a DXF written from a dynamic block was reopened in OpenDraft only, not opened in AutoCAD (the same owed check as the other DXF notes). Evaluated pictures were compared with AutoCAD's own anonymous blocks only for the three linear-stretch instances in `dyn_blocks_2`; other kinds of parameter and action are not built, so they are not compared.
- **Anonymous block import from a real AutoCAD file:** the `*U` naming and ownership rules come from Autodesk's developer blog, not a specification. The sample DXFs hold some and import ran on them (24-11-2026), but the pictures were not compared in AutoCAD, and DWG import of placed `*U` blocks is tested with hand-made data only. Open a drawing with a stretched dynamic block (DXF and DWG) in both programs, check the picture matches, then take this line off.
- **Rotation parameter by hand:** tried 24-11-2026 in the running app (Windows, dark theme): `-BLOCK` of two lines, BEDIT, `BPARAMETER` Rotation (base, radius, defaults) and `BACTION` Rotate, the circle and radius line and the action's name drawn over the drawing, BSAVE and BCLOSE, the Custom row `Angle1` in Properties (typing 45 turned the picture 45 degrees about the base and moved the diamond grip), and dragging the grip (the ghost followed the cursor and the commit set `Angle1` to the pointer's angle). Not tried: an angle list or increment in the UI, a rotated, scaled or mirrored insert, a rotation beside a linear parameter, the light theme, saving and reopening a rotation in the app (unit-tested only), DXF export of a rotated reference, and an imported rotation with a turn (only the `rotate_example_1` definition was imported, with a turn of nothing).
- **Flip parameter by hand:** tried 24-11-2026 in the running app (Windows, dark theme): `-BLOCK` of an L of two lines, BEDIT, `BPARAMETER` Flip (a vertical mirror line) and `BACTION` Flip, BSAVE, BCLOSE, the Custom row `Flip1` reading No, and typing Yes (the L mirrored about the line). Not tried: dragging the flip grip with the mouse (unit-tested: dropping it left of the line, from base to end, flips), a mirror line that is not through the block's base point in the app, a flipped insert that is itself rotated, scaled or mirrored, flip beside other parameters, a flip action whose objects include inserts or text, the light theme, and an imported flip (the definition `flip_example_1` imports; a flipped reference was not available).
- **Scale action by hand:** tried 24-11-2026 in the running app (Windows, dark theme): `-BLOCK` of a circle and a line, BEDIT, `BPARAMETER` (linear) and `BACTION` Scale on both objects, BSAVE, BCLOSE, and typing 2 into `Distance1` (the line's 4 became 2 and the circle halved about the line's start). On `dyn_blocks_2`, the two modified references of `scale_example_1` (values giving 4.67923 and 1.50620 across) evaluate to the same size and position as AutoCAD's own anonymous blocks `Anonymous_U10` and `Anonymous_U11`. Not tried: a scale beside a stretch or move on one parameter, a scale about a point of its own (AutoCAD's independent base, left out on import), X and Y scale factors on their own (the scale action here is uniform), a scaled or mirrored insert, text and dimensions inside a scaled block, and the light theme.
- **Polar parameter by hand:** tried 24-11-2026 in the running app (Windows, dark theme): `-BLOCK` of an L of two lines, BEDIT, `BPARAMETER` Polar (base, end, all the defaults) and `BACTION` Stretch with a frame round the end, BSAVE, BCLOSE, the Custom rows `Distance1` 4.0000 and `Angle1` 0 with the diamond grip at the end, and typing 45 into `Angle1` (the end turned to 45 degrees about the base and kept its distance of 4). On `dyn_blocks_2`, the two modified references of `polar_stretch_example_1` evaluate to the same size and position as AutoCAD's own anonymous blocks (3.11073 by 3.11073 and 3.01134 by 5.21579). Not tried: dragging the polar grip with the mouse (unit-tested), a distance or angle list or increment typed in the UI, a polar beside other parameters, a scaled or mirrored insert, the light theme, and the Properties message after an edit names the parameter (`Distance1`) even when the angle row was edited. AutoCAD's polar stretch also turns and scales objects of its selection that are not in the frame; here the polar stretch is the stretch of what is in the frame by the end's displacement, which is all `polar_stretch_example_1` uses, so a polar stretch with objects outside its frame may differ.
- **XY and point parameters by hand:** tried 24-11-2026 in the running app (Windows, dark theme): `-BLOCK` of an L of two lines, BEDIT, `BPARAMETER` XY (a base and a corner) and `BACTION` Move on both objects, BSAVE, BCLOSE, the Custom rows `Distance1` and `Y Distance1` (both 1.0000) with the diamond grip on the corner, and typing 3 into `Distance1` (the whole block moved 2 to the right and the grip followed). The point parameter was authored and evaluated only in unit tests. Not tried in the app: a point parameter, dragging the grips, XY or point beside other parameters, a scale on an XY parameter (not built: AutoCAD allows it), a list or increment of distances typed in the UI. Not checked against AutoCAD: `move_example_1` (a point parameter with a move) imports as a dynamic block, but neither sample file has a modified reference of an XY or point parameter to compare with, so an XY reference's end point (read like a polar one) is unchecked, and a modified reference of a point parameter is not read (its record layout is not known), so it keeps its anonymous block's picture. An XY parameter with a list or increment of distances is left out on import (the codes of its two value sets were guessed from the polar parameter's).
- **Lookup by hand:** tried 24-11-2026 in the running app (Windows, dark theme): a two-line block, a linear parameter with a stretch, `BPARAMETER` Lookup (one input, row "Short" = 2), `BLOOKUPTABLE` Add ("Long" = 8), BSAVE, BCLOSE, then the Custom row showed "Custom" for the default 4 and picking "Long" set Distance1 to 8 and stretched the bar. Not tried in the app: save and reopen (round-tripped in tests only), a rotation or flip input, several inputs, the light theme. Not checked against AutoCAD: `length_example_1` in `dyn_blocks_2` imports its four rows ("2.5mm" to "10mm") with the right values, read by taking the table's one input column, named `UpdatedDistance`, to be the block's one linear parameter; a table with other columns (an angle's property name is not known, nor is a table with several inputs) is left out and named.
- **Visibility by hand:** tried 24-11-2026 in the running app (Windows, dark theme): `-BLOCK` of a line and a circle, BEDIT, `BPARAMETER` Visibility, `BVSTATE` New twice, `BVHIDE` in two states, BSAVE, BCLOSE, then picking states from the Custom row's drop-down in Properties (the line vanished in "Round"; undo brought it back). Save and reopen was tried the same day (a reference set to a state that hides the circle came back hiding it). Not tried in the app: a reference with other parameters beside the visibility, two references in different states selected together, the light theme, the Block Editor's ribbon (it has no buttons for these commands yet). Not checked against AutoCAD: the import reads a visibility parameter as AutoCAD wrote it in `dyn_blocks_1` (`331` handles the objects it governs, each state a `303` name with `332` handles of the objects it shows, the first state the default; the instance record's `1` is the state shown) and `visibility_example_1` and its modified reference come in with the right states, but no picture was compared with AutoCAD's own. An object AutoCAD marks invisible that no state governs is dropped and counted, as before. A visibility parameter that did not read leaves hidden objects visible in the picture kept.
- **Imported dynamic blocks by hand:** tried 24-11-2026 in the running app (Windows, dark theme) on `dyn_blocks_2.dxf`: `length_example_1` came in dynamic, its modified references showed `Distance1` 10.0000 in Properties with a diamond end grip, and typing 7.5 shortened the line (`Changed Distance1`). The evaluated pictures of the three modified references (5, 10 and 2.5) had the same bounds as AutoCAD's own anonymous blocks. Not tried: a modified reference of a block with two parameters, a rotated or scaled insert of an imported block, a stretch with a polygon frame, a move action, an angle offset (read as radians, not checked), a value list that has to snap, DXF files from other AutoCAD versions, and saving the imported drawing and reopening it in the running app (a unit test saves an imported drawing as `.odraft` and reads it back with the same blocks, objects and value; the app's own Save As and Open were not driven).
- **Dynamic block decoder against AutoCAD:** the DXF layout in `docs/ARCHITECTURE.md` was read from two sample drawings (not committed: they hold a work login) and the decoder is tested with hand-made records shaped like them. For linear, polar and scale the stored value matched the picture of the anonymous block; the lookup table and the edges were read, not compared with AutoCAD's own listing. Try more drawings (other AutoCAD versions, nested and attributed dynamic blocks, two parameters on one block) and check the names and values it prints.
- **In AutoCAD** (open, `AUDIT`, edit, save): the Model-tab freeze's XDATA on LAYER records (it should survive a save), everything the DXF writer emits for dimensions, multileaders, legacy leaders and block attributes, ByBlock, and the R2000 layout export; the DWG reader's object counts against an AutoCAD listing; the ellipse angle convention and the spline prompts; MTEXT tracking at the end of a line; the rectangular array's default spacing; the multileader style's angle constraints and attachment codes.
- **Custom coordinate systems in files:** an ordinate dimension made in a turned system has not been checked against one AutoCAD made (only the sign of group 51 against ezdxf); a DXF with a turned UCS has not been opened in AutoCAD (ezdxf reads ours cleanly), and no DWG with a turned UCS has been read: every local sample has the World UCS, so the decoded values are proven only to keep the header aligned. The paper space `$PUCS*` variables are not written (AutoCAD's defaults are World).
- **By hand in the real app, the custom coordinate system:** tried 22-11-2026: Rotate, Object (on a line) and View in the Model tab; the turned grid, axes, icon and crosshair; grid snap to the turned grid; ortho holding a line along the turned X axis, with the dynamic input fields; LINE with typed relative points; the refusal on a layout's sheet; a viewport's own system (entered with MSPACE, turned 45 degrees while the Model tab's stayed at 30). Not yet tried: polar and object snap tracking, ROTATE and grip rotate, and the commands that orient what they make (TEXT, RECTANG, DIMLINEAR, DIMORDINATE, ARRAYRECT, HATCH, MLEADER), which are covered by tests only.
- **By hand in the real app, view rotation:** tried 22-11-2026 on the Model tab: `ROTATEVIEW 30` (grid, axes icon and a rectangle turned the right way), `PLAN` (level, extents), Ctrl+middle-drag (about a quarter turn counter-clockwise), and picking a line in a turned view (its Properties and grips right). Not tried: Shift steps and Esc by hand, `ROTATEVIEW` Points, a viewport and a layout's sheet refusing, text, hatches and snap markers in a turned view, and a DXF with a turned viewport opened in AutoCAD.

- **By hand in the real app, the navigation gizmo:** not seen running at all (tests cover its geometry and mesh only): how it looks in both palettes, the hover and drag cursors, the arrow following Ctrl+middle-drag and `ROTATEVIEW`, a sector click and Home (which follows the current coordinate system, its tooltip too) in the Model tab and in a viewport, with a turned system as well as the world one, the level button (its look, tooltip, snapping to a line's ends, Esc, dimmed during a command), that it hides on a layout's sheet and in a locked viewport, that the pointer over it draws no crosshair or selection on the drawing, and the frame readout beside it.
- **By hand in the real app** (tests drive the commands through `AppState` only): DIMEDIT and DIMTEDIT, TEXTEDIT and TEXT typed in place, DIST and AREA, Overkill and Purge, ByBlock, the attribute editor's double-click and the ATTDEF dialog, the warning before a large file opens, the window and lasso selection style, snapping to a polyline while drawing it, the Ortho pointer test, the Paragraph dialog (nobody has seen it), DIVIDE, MEASURE, grips, TRIM and BREAK on points and construction lines. The Settings dialog's pages were checked 26-11-2026 at 100% scale on Windows (none scrolls); not at other interface scales or on Linux.
- **On real machines:** the 64-bit ARM Linux builds (.deb, tarball and AppImage) on a Raspberry Pi; so far they have only run in CI on an ARM runner (issue #164); Linux desktops beyond Mint (and on Mint: the Home page's templates and file actions, the assistant's key in the Secret Service, the crash log); the Windows exe on a machine with no Visual C++ Redistributable; the MSI's shortcut and file association opening with no console window; physical printers, and CUPS versions other than the one tried; the update check against the real GitHub.
- **The `.lin` parser on third-party files:** run 26-11-2026 on AutoCAD's own `acad.lin` (45 definitions) and `acadiso.lin` (59), both read with nothing skipped (the seven complex ones with a note), and the 38 `acadiso.lin` patterns that are in the stock library have the same numbers (`cargo run -p opendraft-core --example lin_check`, which skips files it cannot find in `private_material/`). Not yet run on a `.lin` made by other software, nor the imperial file's numbers against `Linetypes.dxf` (the DXF test already holds our imperial table to it).
- **Loading linetypes from a file, by hand:** tried 26-11-2026 in the running app (Windows, dark theme): LINETYPE, Load, File (the native dialog, typing the path of `acadiso.lin`), the list and its note about seven linetypes with text or shapes, ticking BORDER and BORDER2, OK, and both appearing in the Manager with "Loaded 2 linetypes" on the status bar. Not tried: the file dialog on Linux, the light theme, a file with unreadable entries in the window (unit-tested), replacing a linetype that objects use, and Manager OK then undo after a load.
- **`-LINETYPE` by hand:** tried 26-11-2026 in the running app (Windows, dark theme): `-LINETYPE`, Load, `BORDER*,DASHED,NOPE`, the path of `acadiso.lin` (typed with forward slashes), a note that nothing matched NOPE, the Reload question for DASHED answered Yes, "3 linetypes loaded, 1 reloaded", and the Linetype Manager listing BORDER, BORDER2 and BORDERX2 and DASHED with AutoCAD's description. Not tried: `?`, `Create` and `Set` in the app (unit-tested), the light theme, Linux, and typing a path with backslashes: the keystrokes sent by the test tool (`SendKeys`) did not reach the command line as backslashes (`a\b\c` showed `abc`); that may be the tool and the keyboard layout rather than the app, but check it with a real keyboard, since typed Windows paths depend on it.
- **Language detection on Linux:** the Language setting's "System" choice (`sys_locale`, `language.rs`) has been tried on Windows only; not yet on a Linux desktop set to a real French locale (`fr_FR.UTF-8`), where the ribbon should come up in French.
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

Pull request 1 (the system) is built. Left out of it:

- **DWG**: the UCS table (named systems, object type 0x3F) and viewports' own systems are not decoded; only the header's model-space UCS is (and DWG skips layouts anyway). DXF does all three.
- Polyline and rectangle Object rule: ours takes the segment nearest the pick; AutoCAD takes a polyline's first segment. Kept on purpose; say so if users expect AutoCAD's.

Pull request 2 (view rotation: the camera turns, Ctrl+middle-drag, `ROTATEVIEW`, `PLAN`, viewport twist in files) is built; see `docs/ARCHITECTURE.md`, "View rotation". Left out of it:

- No ribbon item or icon for `ROTATEVIEW` and `PLAN` (ZOOM has none either); no `UCSICON`-style or status-bar control; the mouse pivot is always the canvas centre.
- The Model tab's twist is not saved in `.odraft` or DXF (only viewports').
- The MTEXT editor window and its ruler are laid out on the screen's axes, so editing text in a turned view is awkward; PDSIZE point shapes (cross, X) turn with the view.
- The API/MCP view and the plot preview have no twist.

Later: `UCSICON` settings (the icon cannot be switched off or moved to the origin), a manager dialog (`UCSMAN`), dragging the icon by its grips, the dimension arcs of the dynamic input drawn along a turned system's axes (the Cartesian legs are still screen-aligned), the systems through MCP, the legacy LEADER's text (it is level with the world).

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

- **`.lin` loading (built, see `docs/ARCHITECTURE.md`):** the parser, the Manager's Load from a file and `-LINETYPE` Load and `?`. Left: `-LINETYPE` Create and Set (Set needs the current linetype, `CELTYPE`), a file dialog (AutoCAD's `~`) and a default or searched `.lin` for `-LINETYPE`'s file prompt, remembering the folder of the last file, a prompt before a Manager replacement changes a linetype that objects use (it replaces at once; OK on the Manager is the undo step), and a file's own text and shape elements. Complex entries (`["GAS",STANDARD,S=.1,...]`, shapes from `.shx`) are read as their dashes and reported, as DXF import does. Export (`.lin` out) and Reload/Rename in the Manager belong here too.
- **Defaults for new objects:** `CELTSCALE` and `PLINEGEN` (today new objects have scale 1 and generation off).
- **ByBlock leftovers:** ByBlock in the DXF/DWG layer and style tables, MLEADER styles that apply it, dimensions' own colour/linetype/lineweight, and `CECOLOR`/`CELTYPE`/`CELWEIGHT` taking ByBlock.
- **Complex linetypes:** text and shape elements are dropped (drawn as plain dashes with a warning). Keeping them for a round trip needs a decoration on each pattern element (`ComplexDecoration`, raw flags) and DXF groups 74/75/340/46/50/44/45/9; drawing them needs the text and shape engines. DWG decodes the complex flag but not the extra fields.
- **Not yet dashed:** block contents, dimensions and hatch outlines draw solid (`HPLINETYPE` governs hatches in AutoCAD).
- **Scale rules:** `PSLTSCALE` and `MSLTSCALE` (need annotative scales); DWG polyline Linetype Generation flag (LWPOLYLINE flag 256) is not decoded.
- **Fitting arithmetic:** how end dashes take up slack, and how a closed shape is phased, is our own choice meeting AutoCAD's documented rules. Lock it down with AutoCAD fixtures at several lengths (lines, arcs, circles, closed polylines), as the guide's acceptance tests describe.
- **Performance:** dashes are rebuilt every paint; cache per object if a large dashed drawing feels slow (the hatch cache is the model).

## Note: automation endpoint name

On Windows the automation endpoint is a named pipe called `od-<instance id>` (`opendraft-app/src/automation/host.rs`, `endpoint_for`); on Linux it is a `<instance id>.sock` file, so the prefix does not appear there. Clients read the endpoint from the instance descriptor rather than building the name, so a rename touches `endpoint_for` and the fake endpoint in `opendraft-mcp/tests/support/mod.rs`. It is a runtime name, not a file or crate name, so the family naming rules did not require changing it.
