// Hatch GPU buffers — renders hatch fills as shader-driven bounding-box quads.
//
// Group 1 bindings per hatch:
//   binding 0 — HatchUniformData  (64 bytes)   mode, color, angle_offset, scale, gradient params
//   binding 1 — BoundaryData      (16384 bytes) boundary polygon vertices
//   binding 2 — FamilyBatchData   (1296 bytes)  up to 16 line families + 128 dash values

use crate::scene::hatch_model::{HatchModel, HatchPattern, PatFamily, MAX_HATCH_BOUNDARY_VERTS};
use iced::wgpu;
use iced::wgpu::util::DeviceExt;

// ── Constants ──────────────────────────────────────────────────────────────

pub const MAX_FAMILIES: usize = 16;
pub const MAX_DASHES: usize = 128;

// ── Vertex ────────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HatchVertex {
    pub pos: [f32; 3],
    pub _pad: f32,
}

impl HatchVertex {
    pub fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<HatchVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }
    }
}

// ── Uniform structs ───────────────────────────────────────────────────────

/// Per-hatch parameters (binding 0).  Must be 80 bytes (16-byte aligned).
///
/// `mode` encoding:
///   0 → Pattern  (families in FamilyBatchData)
///   1 → Solid
///   2 → Gradient (grad_cos/sin/min/range used)
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HatchUniformData {
    pub color: [f32; 4],     //  0: primary RGBA / gradient start
    pub color2: [f32; 4],    // 16: gradient end color
    pub mode: u32,           // 32: 0=pattern, 1=solid, 2=gradient
    pub vertex_count: u32,   // 36: boundary vertex count
    pub angle_offset: f32,   // 40: pattern rotation (radians)
    pub scale: f32,          // 44: pattern scale multiplier
    pub grad_cos: f32,       // 48: gradient direction cos
    pub grad_sin: f32,       // 52: gradient direction sin
    pub grad_min: f32,       // 56: gradient proj_min
    pub grad_range: f32,     // 60: gradient proj_range
    pub origin: [f32; 2],    // 64: hatch-local origin (boundary AABB centre).
    //                       //     The pattern fragment shader subtracts this
    //                       //     from xz before the perp/perp_step quotient
    //                       //     so f32 doesn't catastrophically cancel when
    //                       //     world coords are large and pattern spacing
    //                       //     is small (e.g. UTM drawing + sub-mm hatch).
    pub _pad: [f32; 2],      // 72: 16-byte alignment
} // total 80 bytes

/// Boundary polygon (binding 1).  Matches WGSL `Boundary`.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BoundaryData {
    pub verts: [[f32; 4]; MAX_HATCH_BOUNDARY_VERTS], // world XY in .xy, .zw unused
}

/// One line family packed for the shader (48 bytes).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineFamilyGpu {
    pub cos_a: f32,      // cos(angle_rad)
    pub sin_a: f32,      // sin(angle_rad)
    pub x0: f32,         // family origin x
    pub y0: f32,         // family origin y
    pub dx: f32,         // step vector x
    pub dy: f32,         // step vector y
    pub perp_step: f32,  // -dx*sin_a + dy*cos_a  (perpendicular spacing)
    pub along_step: f32, //  dx*cos_a + dy*sin_a  (along-line phase shift per step)
    pub line_width: f32, // |perp_step| * 0.08
    pub period: f32,     // sum of |dash values|  (0 = solid)
    pub n_dashes: u32,   // number of dash entries (0 = solid)
    pub dash_off: u32,   // index into FamilyBatchData::dash_values
}

/// All line families + dash values for one hatch (binding 2).
/// Total size: 16×48 + 32×16 + 4×4 = 768 + 512 + 16 = 1296 bytes.
/// dash_values is [f32; 4]×32 so each element is vec4<f32> (16-byte stride, valid in uniform).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FamilyBatchData {
    pub families: [LineFamilyGpu; MAX_FAMILIES], // 768 bytes
    pub dash_values: [[f32; 4]; 32],             // 512 bytes (128 f32s as 32×vec4)
    pub n_families: u32,                         //   4 bytes
    pub _pad: [u32; 3],                          //  12 bytes
} // 1296 bytes total

// ── Per-hatch GPU handle ───────────────────────────────────────────────────

pub struct HatchGpu {
    pub vertex_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    /// Mirrors `HatchModel.vp_scissor`. Forwarded to the pipeline so each
    /// frame's `compute_hatch_scissors` can translate the world rect into
    /// a pixel scissor.
    pub vp_scissor: Option<[f32; 4]>,
    /// World-space XY bounding rect [min_x, min_y, max_x, max_y] of the
    /// boundary polygon. Used by the per-frame LOD pass to skip hatches
    /// whose entire footprint projects to less than ~2 px (Phase 3.3).
    pub world_aabb: [f32; 4],
    _uniform_buf: wgpu::Buffer,
    _boundary_buf: wgpu::Buffer,
    _family_buf: wgpu::Buffer,
}

impl HatchGpu {
    pub fn new(device: &wgpu::Device, model: &HatchModel, bgl1: &wgpu::BindGroupLayout) -> Self {
        // ── Decode pattern mode ──────────────────────────────────────────
        let (mode, color2, grad_cos, grad_sin) = match &model.pattern {
            HatchPattern::Solid => (1u32, [0.0f32; 4], 0.0f32, 0.0f32),
            HatchPattern::Pattern(_) => (0u32, [0.0f32; 4], 0.0f32, 0.0f32),
            HatchPattern::Gradient { angle_deg, color2 } => {
                let r = angle_deg.to_radians();
                (2u32, *color2, r.cos(), r.sin())
            }
        };

        // ── Bounding box ─────────────────────────────────────────────────
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        );
        for &[x, y] in model.boundary.iter() {
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }

        let max_spacing = match &model.pattern {
            HatchPattern::Pattern(families) => families
                .iter()
                .map(|f| perp_spacing(f).abs())
                .fold(0.0f32, f32::max),
            _ => 5.0,
        };
        let diag = ((max_x - min_x).powi(2) + (max_y - min_y).powi(2)).sqrt();
        let pad = (diag * 0.8 + max_spacing * 2.0 * model.scale).max(1.0);
        // ── Pattern phase anchor.
        //
        // Boundary verts are already stored as small f32 offsets from
        // `model.world_origin` (an f64 anchor near the AABB centre), so
        // the in_polygon ray-cast and the pattern math both run at full
        // f32 precision in the fragment shader. The remaining question
        // is *which* offset-rel point we hand to the vertex shader as
        // `h.origin` (so the quad lands at the correct WCS position
        // after `view_proj * (v.pos + h.origin)`).
        //
        // For pattern alignment across adjacent hatches with the same
        // family, snap `h.origin` to the family's perp/along grid in
        // f64. Without the snap, each hatch's pattern phase is per-hatch
        // and stripes don't continue across hatch boundaries (you see
        // pattern misalignment along a road etc).
        let mut origin = model.world_origin;
        if let HatchPattern::Pattern(families) = &model.pattern {
            if let Some(fam) = families.first() {
                // Must mirror the shader's pattern math exactly. QCAD PAT
                // convention (matching build_family_batch): `dy` is the
                // perpendicular spacing and `dx` is the along-line phase
                // shift, both in family-local space. Multiplied by
                // `model.scale` in the fragment shader. Combined angle
                // (family + pattern_angle) rotates the perp/along axes.
                let angle = (fam.angle_deg.to_radians() + model.angle_offset) as f64;
                let cos_a = angle.cos();
                let sin_a = angle.sin();
                let scale = model.scale as f64;
                let perp_step = fam.dy as f64 * scale;
                let along_step = fam.dx as f64 * scale;
                let o_perp = -origin[0] * sin_a + origin[1] * cos_a;
                let o_along = origin[0] * cos_a + origin[1] * sin_a;
                let snapped_perp = if perp_step.abs() > 1e-12 {
                    (o_perp / perp_step).round() * perp_step
                } else {
                    o_perp
                };
                let snapped_along = if along_step.abs() > 1e-12 {
                    (o_along / along_step).round() * along_step
                } else {
                    o_along
                };
                origin = [
                    -snapped_perp * sin_a + snapped_along * cos_a,
                    snapped_perp * cos_a + snapped_along * sin_a,
                ];
            }
        }
        // Boundary buffer + quad now live in "snap-shifted" hatch-local
        // coords: subtract any drift between model.world_origin and the
        // snapped origin so the in_polygon ray-cast in fragment shader
        // (which reads the boundary verts straight) lines up with v.xz.
        let drift = [
            (model.world_origin[0] - origin[0]) as f32,
            (model.world_origin[1] - origin[1]) as f32,
        ];
        let (x0, x1, y0, y1) = (
            min_x + drift[0] - pad,
            max_x + drift[0] + pad,
            min_y + drift[1] - pad,
            max_y + drift[1] + pad,
        );

        let quad = [
            HatchVertex {
                pos: [x0, y0, 0.0],
                _pad: 0.0,
            },
            HatchVertex {
                pos: [x1, y0, 0.0],
                _pad: 0.0,
            },
            HatchVertex {
                pos: [x1, y1, 0.0],
                _pad: 0.0,
            },
            HatchVertex {
                pos: [x0, y0, 0.0],
                _pad: 0.0,
            },
            HatchVertex {
                pos: [x1, y1, 0.0],
                _pad: 0.0,
            },
            HatchVertex {
                pos: [x0, y1, 0.0],
                _pad: 0.0,
            },
        ];
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("hatch.vbuf"),
            contents: bytemuck::cast_slice(&quad),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // ── Gradient: projection range (in snapped-local space) ──────────
        let (grad_min, grad_range) = if mode == 2 {
            let projs: Vec<f32> = model
                .boundary
                .iter()
                .filter(|v| v[0].is_finite() && v[1].is_finite())
                .map(|&[x, y]| (x + drift[0]) * grad_cos + (y + drift[1]) * grad_sin)
                .collect();
            if projs.is_empty() {
                (0.0, 1.0)
            } else {
                let proj_min = projs.iter().cloned().fold(f32::INFINITY, f32::min);
                let proj_max = projs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                (proj_min, (proj_max - proj_min).max(1e-4))
            }
        } else {
            (0.0, 1.0)
        };

        // ── HatchUniformData ─────────────────────────────────────────────
        let n = model.boundary.len().min(MAX_HATCH_BOUNDARY_VERTS);
        let uniform_data = HatchUniformData {
            color: model.color,
            color2,
            mode,
            vertex_count: n as u32,
            angle_offset: model.angle_offset,
            scale: model.scale,
            grad_cos,
            grad_sin,
            grad_min,
            grad_range,
            origin: [origin[0] as f32, origin[1] as f32],
            _pad: [0.0; 2],
        };

        // ── BoundaryData (in snapped-local space) ────────────────────────
        let mut boundary_data = BoundaryData {
            verts: [[0.0; 4]; MAX_HATCH_BOUNDARY_VERTS],
        };
        for (i, &[x, y]) in model
            .boundary
            .iter()
            .take(MAX_HATCH_BOUNDARY_VERTS)
            .enumerate()
        {
            // Preserve NaN separators — they signal path breaks to the
            // in_polygon ray-cast, must not be shifted into finite numbers.
            if x.is_finite() && y.is_finite() {
                boundary_data.verts[i] = [x + drift[0], y + drift[1], 0.0, 0.0];
            } else {
                boundary_data.verts[i] = [f32::NAN, f32::NAN, 0.0, 0.0];
            }
        }

        // ── FamilyBatchData ───────────────────────────────────────────────
        let family_batch = build_family_batch(&model.pattern);

        // ── GPU buffers ───────────────────────────────────────────────────
        let _uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("hatch.uniforms"),
            contents: bytemuck::bytes_of(&uniform_data),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let _boundary_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("hatch.boundary"),
            contents: bytemuck::bytes_of(&boundary_data),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let _family_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("hatch.families"),
            contents: bytemuck::bytes_of(&family_batch),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hatch.bind_group1"),
            layout: bgl1,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: _uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: _boundary_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: _family_buf.as_entire_binding(),
                },
            ],
        });

        Self {
            vertex_buffer,
            bind_group,
            vp_scissor: model.vp_scissor,
            world_aabb: [min_x, min_y, max_x, max_y],
            _uniform_buf,
            _boundary_buf,
            _family_buf,
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Perpendicular spacing between adjacent parallel lines.
/// QCAD PAT format stores (dx, dy) in line-local coordinates: dy = perpendicular, dx = along-line.
fn perp_spacing(f: &PatFamily) -> f32 {
    f.dy
}

/// Pack pattern families into the GPU batch struct.
fn build_family_batch(pattern: &HatchPattern) -> FamilyBatchData {
    let mut batch = FamilyBatchData {
        families: [LineFamilyGpu {
            cos_a: 0.0,
            sin_a: 0.0,
            x0: 0.0,
            y0: 0.0,
            dx: 0.0,
            dy: 0.0,
            perp_step: 0.0,
            along_step: 0.0,
            line_width: 0.0,
            period: 0.0,
            n_dashes: 0,
            dash_off: 0,
        }; MAX_FAMILIES],
        dash_values: [[0.0; 4]; 32],
        n_families: 0,
        _pad: [0; 3],
    };

    let HatchPattern::Pattern(families) = pattern else {
        return batch;
    };

    let mut dash_cursor: usize = 0;

    for (fi, family) in families.iter().take(MAX_FAMILIES).enumerate() {
        let angle_r = family.angle_deg.to_radians();
        let cos_a = angle_r.cos();
        let sin_a = angle_r.sin();
        // QCAD PAT local-frame convention: dy = perpendicular spacing, dx = along-line phase shift.
        let perp_step = family.dy;
        let along_step = family.dx;
        let line_width = 0.0_f32; // unused: shader uses screen-space derivative for 1px lines

        // Dash pattern: collect up to available space.
        let n_avail = MAX_DASHES.saturating_sub(dash_cursor);
        let n_dashes = family.dashes.len().min(n_avail);
        let period: f32 = family.dashes[..n_dashes].iter().map(|d| d.abs()).sum();

        let dash_off = dash_cursor as u32;
        for &d in &family.dashes[..n_dashes] {
            batch.dash_values[dash_cursor / 4][dash_cursor % 4] = d;
            dash_cursor += 1;
        }

        batch.families[fi] = LineFamilyGpu {
            cos_a,
            sin_a,
            x0: family.x0,
            y0: family.y0,
            dx: family.dx,
            dy: family.dy,
            perp_step,
            along_step,
            line_width,
            period: if n_dashes > 0 { period } else { 0.0 },
            n_dashes: n_dashes as u32,
            dash_off,
        };
        batch.n_families += 1;

        if dash_cursor >= MAX_DASHES {
            break;
        }
    }

    batch
}
