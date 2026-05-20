# HeaderVariables Audit

`acadrust::document::HeaderVariables` — 232 total.

## Pass (no matching feature)

- **ISOLINES** — contour line count on curved surfaces (no curved
  wireframe generator yet)
- **surface_u_density / surface_v_density / surface_type** — defaults
  for SURFACE / REVSURF / EDGESURF commands (those commands don't
  exist yet)
- **lens_length / camera_height / camera_display** — perspective
  camera (no perspective viewport yet)
- **north_direction / latitude / longitude / timezone** — sun /
  geographic data (no sun-shadow renderer)
- **LIMMIN/LIMMAX** — drawing limits (no bounded-grid mode)
- **INSBASE** — drawing's "save as block" anchor (no WBLOCK / "save
  selection as block" flow)

### Remaining (intentionally not surfaced)
- ~70 `dim_*` defaults — render-time `DimStyle` reference is already
  authoritative; these only seed a new DimStyle when a fresh DIMSTYLE
  NEW flow is added.
- `*_control_handle`, `*_dict_handle` — internal table-navigation
  handles; direct `document.layers` / `document.dim_styles` / etc.
  accessors bypass them. acadrust read/write preserves them.
- AutoCAD UI prefs (`blip_mode`, `drag_mode`, `pick_style`, etc.) —
  H7CAD has its own UI prefs and ignores AutoCAD's. Round-tripped.
