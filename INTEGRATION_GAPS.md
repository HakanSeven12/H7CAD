# acadrust Integration Gaps

Missing or incomplete integrations between acadrust entity definitions and H7CAD rendering/interaction systems. Ordered by priority within each category.

Legend: ✅ Done · ⚠️ Partial · ❌ Not done

---

## Entity Field Gaps (acadrust fields ignored in tessellation)

### High Impact

| Status | Entity | Ignored Field(s) | Effect |
|---|---|---|---|
| ✅ | **INSERT** | `column_count`, `row_count`, `column_spacing`, `row_spacing` | MINSERT handled by `acadrust::explode_from_document()` → `array_transforms()` |
| ✅ | **LWPolyline** | `constant_width`, vertex `start_width` / `end_width` | Implemented in `src/scene/mod.rs:3239` |
| ✅ | **Polyline (legacy)** | `Vertex2D.start_width` / `end_width` | Implemented in `src/scene/mod.rs` |

### Medium Impact

| Status | Entity | Ignored Field(s) | Effect |
|---|---|---|---|
| ✅ | **Dimension** | `DIMSCALE`, `DIMASZ`, `DIMEXO`, `DIMEXE` (from dimstyle) | Implemented in `src/scene/tessellate.rs:263` |
| ✅ | **Spline** | `weights` (rational NURBS control point weights) | NurbsCurve<Vector4> used when weights present — `src/entities/spline.rs` |
| ✅ | **Spline** | `flags.closed` / `flags.periodic` | Implemented in `src/entities/spline.rs:48` |
| ✅ | **Hatch** | `BoundaryEdge::Spline` tessellation | Implemented in `src/scene/mod.rs:1802` |
| ✅ | **MultiLeader** | `MultiLeaderPathType::Spline` | Implemented in `src/entities/multileader.rs:76` |
| ✅ | **RasterImage** | `clip_boundary` (polygonal/rectangular) | Implemented in `src/entities/raster_image.rs` — polygonal + rectangular clip |

### Low Impact

| Status | Entity | Ignored Field(s) | Effect |
|---|---|---|---|
| ✅ | **Arc / Circle / Line / LwPolyline / Polyline2D** | `thickness` | Extrudes along WCS normal — `src/entities/{arc,circle,line,lwpolyline,polyline}.rs` |
| ❌ | **LWPolyline** | `plinegen` flag | Linetype pattern resets at each vertex instead of continuing |

---

## Entity Type Gaps (entire types with missing subsystems)

### Renders Nothing / Placeholder Only

| Status | Entity | Notes |
|---|---|---|
| ✅ | **Face3D** | Full tessellation, grips, properties panel — `src/entities/mesh.rs` |
| ❌ | **OLE2Frame** | Bounding box + X mark only; no interactivity |

### Wire Fallback Only (no full mesh)

| Status | Entity | Missing |
|---|---|---|
| ⚠️ | **Solid3D / Region / Body** | `src/scene/solid3d_tess.rs` exists with MeshModel support; no real mesh from ACIS data |

### Partial Render

| Status | Entity | What works | What's missing |
|---|---|---|---|
| ⚠️ | **Viewport** (paper space) | Frame (active=gold, inactive=cyan), CPU projection + Cohen-Sutherland clipping, perspective, layer freeze per-viewport, MSPACE switching, pan/zoom/orbit | GPU scissor rect for pixel-level boundary clipping; stencil masking for overlapping viewports |

---

## Systemic Gap — OCS→WCS Transform

✅ **Done.** The DXF arbitrary-axis algorithm is implemented in `src/scene/transform.rs` (`ocs_axes`, `ocs_point_to_wcs`). Applied to:

| Status | Entity | Location |
|---|---|---|
| ✅ | **Arc** | `src/entities/arc.rs` |
| ✅ | **Circle** | `src/entities/circle.rs` |
| ✅ | **Line** | `src/entities/line.rs` |
| ✅ | **Point** | `src/entities/point.rs` |
| ✅ | **Ellipse** | `src/entities/ellipse.rs` |
| ✅ | **LwPolyline** | `src/entities/lwpolyline.rs` |
| ✅ | **Spline** | DXF spec: spline control points are already in WCS (normal field unused) |
| ✅ | **Polyline2D** | `src/entities/polyline.rs` — elevation + normal applied |
| ✅ | **Polyline3D** | Vertices already in WCS per DXF spec (no OCS transform needed) |
| ✅ | **AttributeDefinition / AttributeEntity** | OCS→WCS applied to insertion snap point — `src/entities/attribute.rs` |
| ✅ | **Hatch** | OCS→WCS applied to all boundary edge types — `src/scene/tessellate.rs` |
| ✅ | **MLine / Leader** | Vertices already in WCS per DXF spec; normal field unused for coord transform |

**Impact:** Low for typical 2D plan files (nearly all normals are `(0,0,1)`); high for 3D DXF files with entities on non-horizontal planes.

The DXF arbitrary-axis algorithm (`src/scene/transform.rs:73`):
```
if |Wx| < 1/64 and |Wy| < 1/64:
    Ax = (0,1,0) × N
else:
    Ax = (1,0,0) × N
Ax = normalize(Ax)
Ay = N × Ax
```
Then transform each OCS point: `WCS = x*Ax + y*Ay + z*N`

---

## Render Style Gaps (color, linetype, lineweight resolution)

### High Impact

| Status | Gap | Effect | Location |
|---|---|---|---|
| ✅ | **ByBlock color** resolved through INSERT chain | Implemented via `render_style_for_block_sub()` | `src/scene/render.rs:243` |
| ✅ | **ByBlock linetype** resolved through INSERT chain | Implemented | `src/scene/render.rs:260` |

### Medium Impact

| Status | Gap | Effect | Location |
|---|---|---|---|
| ✅ | **ByBlock lineweight** resolved from INSERT entity | Implemented | `src/scene/render.rs:266` |

---

## Text & Style Gaps

### Medium Impact

| Status | Gap | Effect | Location |
|---|---|---|---|
| ✅ | **TextStyle `is_backward`** flag | Negative width_factor applied | `src/entities/text.rs:60` |
| ✅ | **TextStyle `is_upside_down`** flag | Rotation offset by π applied | `src/entities/text.rs:63` |

### Low-Medium Impact

| Status | Gap | Effect | Location |
|---|---|---|---|
| ✅ | **Complex linetype text shapes** | `LtSeg::Text` calls `cxf::tessellate_text_ex`, `LtSeg::Shape` calls `emit_shape` — `src/scene/complex_lt.rs` |

---

## Polyline3D Vertex Type Gap

✅ **Done.** `src/entities/polyline.rs:305` — VertexFlags (SPLINE_VERTEX flag 8, SPLINE_CONTROL flag 16) properly detected and used for wire/snap point selection.

---

## Snap Point Gaps

### Critical / High

| Status | Entity | Missing Snap | Location |
|---|---|---|---|
| ✅ | **INSERT** | `Insertion` snap point | `src/scene/mod.rs` — appended after explode |
| ✅ | **INSERT** | Nested entity snap points | All sub-entity WireModels are flat-mapped into the scene wire list — snap sees them all |
| ✅ | **Hatch** | Snap points for circular arc boundaries | Center snaps added for CircularArc/EllipticArc edges — `src/scene/tessellate.rs` |

### Medium

| Status | Entity | Missing Snap | Location |
|---|---|---|---|
| ✅ | **Dimension** | Node snap hints on defpoints | `src/scene/tessellate.rs` — `dimension_snap_pts()` |
| ✅ | **Spline** | Fit/control points in snap_pts | `src/entities/spline.rs:38` |
| ✅ | **MultiLeader** | Vertices and connection points in snap_pts | `src/entities/multileader.rs` |
| ✅ | **MLine** | Vertices in snap_pts | `src/entities/mline.rs` |

### Low

| Status | Entity | Missing Snap | Location |
|---|---|---|---|
| ⚠️ | **Ellipse** (partial arc) | Endpoints not in pre-baked snap_pts | Arc endpoints emitted only as `Center`; functional via wire tessellation but semantically wrong |
| ✅ | **Hatch** | Elevation Z | Snap Z uses `hatch.elevation - world_offset.z` — `src/scene/tessellate.rs:627` |

---

## Grip Gaps

| Status | Entity | Missing Grip | Location |
|---|---|---|---|
| ✅ | **LWPolyline** | Midpoint grips for arc segments | `src/entities/lwpolyline.rs:162` |

---

## Text Rendering Gaps

| Status | Gap | Effect | Location |
|---|---|---|---|
| ✅ | **TextStyle `is_backward`** flag applied | Negative width_factor | `src/entities/text.rs:61` |
| ✅ | **TextStyle `is_upside_down`** flag applied | Rotation offset | `src/entities/text.rs:64` |
| ⚠️ | **Unicode characters** not in CXF fonts | Glyph lookup implemented; missing characters silently dropped without warning | `src/scene/cxf.rs:74` |

---

## DXF Reader Unit Gaps (acadrust bugs we work around)

These are fixed in our post-load `fix_dxf_dimension_rotations()` in `src/io/mod.rs`.

| Status | Entity | Field | DXF Code | Fix Location |
|---|---|---|---|---|
| ✅ | **Dimension (Linear)** | `rotation` | 50 | `src/io/mod.rs:221` |
| ✅ | **Dimension (all)** | `text_rotation` | 53 | `src/scene/tessellate.rs::dimension_text_natural_rotation()` |
| ✅ | **AttributeEntity** | `rotation` | 50 | `src/io/mod.rs:224` |
| ✅ | **AttributeDefinition** | `rotation` | 50 | `src/io/mod.rs:219` |
| ❌ | **Shape** | `rotation` | 50 | Shape not currently rendered — low priority |

---

## Coverage Summary

| Subsystem | Coverage |
|---|---|
| Tessellation | 34/41 entity types fully, 4 legacy fallback, 3 missing |
| Snap points | 36/41 (Face3D, Solid3D, Region, Body, OLE2Frame missing) |
| Grip points | 36/41 (same 5 missing) |
| Properties panel | 36/41 (same 5 missing) |
| Hit testing | 41/41 (all via fallback) |

## Gap Status Summary

| Status | Count |
|---|---|
| ✅ Done | 44 |
| ⚠️ Partial | 3 |
| ❌ Not done | 2 |
| **Total** | **49** |

### Remaining gaps by priority

**Medium:** Viewport GPU scissor rect (pixel-level boundary clipping for overlapping viewports)

**Low:** LWPolyline plinegen (GPU shader change needed) · OLE2Frame improvement · Shape rotation
