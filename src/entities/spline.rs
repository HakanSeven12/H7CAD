use acadrust::entities::Spline;
use glam::Vec3;
use truck_modeling::{
    base::{BoundedCurve, ParametricCurve},
    builder, BSplineCurve, Curve, Edge, KnotVec, Point3,
};

use crate::command::EntityTransform;
use crate::entities::common::{ro_prop as ro, square_grip};
use crate::entities::traits::{Grippable, PropertyEditable, Transformable, TruckConvertible};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};

fn to_truck(spl: &Spline) -> TruckEntity {
    let ctrl_pts: Vec<Point3> = spl
        .control_points
        .iter()
        .map(|p| Point3::new(p.x, p.y, p.z))
        .collect();
    if ctrl_pts.len() < 2 {
        return TruckEntity {
            object: TruckObject::Point(builder::vertex(Point3::new(0.0, 0.0, 0.0))),
            snap_pts: vec![],
            tangent_geoms: vec![],
            key_vertices: vec![],
        };
    }
    let knot_vec = if !spl.knots.is_empty() {
        KnotVec::from(spl.knots.clone())
    } else {
        KnotVec::uniform_knot(spl.degree as usize, ctrl_pts.len() - 1)
    };
    let bspline = BSplineCurve::new(knot_vec, ctrl_pts);
    let (t0, t1) = bspline.range_tuple();
    let p_start = bspline.subs(t0);
    let p_end = bspline.subs(t1);
    let v_start = builder::vertex(p_start);
    let v_end = builder::vertex(p_end);
    let edge = Edge::new(&v_start, &v_end, Curve::BSplineCurve(bspline));

    // Expose fit_points (preferred) or control_points as key_vertices so that
    // Endpoint/Midpoint snap can reach spline construction points.
    let snap_source = if !spl.fit_points.is_empty() {
        &spl.fit_points
    } else {
        &spl.control_points
    };
    let key_vertices: Vec<[f32; 3]> = snap_source
        .iter()
        .map(|p| [p.x as f32, p.y as f32, p.z as f32])
        .collect();

    TruckEntity {
        object: TruckObject::Curve(edge),
        snap_pts: vec![],
        tangent_geoms: vec![],
        key_vertices,
    }
}

fn grips(spline: &Spline) -> Vec<GripDef> {
    spline
        .control_points
        .iter()
        .enumerate()
        .map(|(i, p)| square_grip(i, Vec3::new(p.x as f32, p.y as f32, p.z as f32)))
        .collect()
}

fn properties(spline: &Spline) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            ro("Degree", "degree", spline.degree.to_string()),
            ro(
                "Control Pts",
                "ctrl_pts",
                spline.control_points.len().to_string(),
            ),
            ro("Fit Pts", "fit_pts", spline.fit_points.len().to_string()),
        ],
    }
}

fn apply_geom_prop(_spline: &mut Spline, _field: &str, _value: &str) {}

fn apply_grip(spline: &mut Spline, grip_id: usize, apply: GripApply) {
    if let Some(cp) = spline.control_points.get_mut(grip_id) {
        match apply {
            GripApply::Absolute(p) => {
                cp.x = p.x as f64;
                cp.y = p.y as f64;
                cp.z = p.z as f64;
            }
            GripApply::Translate(d) => {
                cp.x += d.x as f64;
                cp.y += d.y as f64;
                cp.z += d.z as f64;
            }
        }
    }
}

fn apply_transform(spline: &mut Spline, t: &EntityTransform) {
    crate::scene::transform::apply_standard_entity_transform(spline, t, |entity, p1, p2| {
        for cp in &mut entity.control_points {
            crate::scene::transform::reflect_xy_point(&mut cp.x, &mut cp.y, p1, p2);
        }
        for fp in &mut entity.fit_points {
            crate::scene::transform::reflect_xy_point(&mut fp.x, &mut fp.y, p1, p2);
        }
    });
}

impl TruckConvertible for Spline {
    fn to_truck(&self, _document: &acadrust::CadDocument) -> Option<TruckEntity> {
        Some(to_truck(self))
    }
}

impl Grippable for Spline {
    fn grips(&self) -> Vec<GripDef> {
        grips(self)
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        apply_grip(self, grip_id, apply);
    }
}

impl PropertyEditable for Spline {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        properties(self)
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        apply_geom_prop(self, field, value);
    }
}

impl Transformable for Spline {
    fn apply_transform(&mut self, t: &EntityTransform) {
        apply_transform(self, t);
    }
}
