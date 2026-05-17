// Diagnostic: simulate Scene::entity_index() build on a DXF/DWG file
// and report per-type stats for indexed / unbounded / unindexable
// entities, plus how many would survive a hypothetical view query.
//
// cargo run --release --example dump_index_stats -- <file>

use acadrust::EntityType;
use std::collections::BTreeMap;

fn entity_kind(e: &EntityType) -> &'static str {
    match e {
        EntityType::Line(_) => "Line",
        EntityType::LwPolyline(_) => "LwPolyline",
        EntityType::Polyline2D(_) => "Polyline2D",
        EntityType::Polyline3D(_) => "Polyline3D",
        EntityType::Polyline(_) => "Polyline",
        EntityType::Circle(_) => "Circle",
        EntityType::Arc(_) => "Arc",
        EntityType::Ellipse(_) => "Ellipse",
        EntityType::Spline(_) => "Spline",
        EntityType::Text(_) => "Text",
        EntityType::MText(_) => "MText",
        EntityType::Hatch(_) => "Hatch",
        EntityType::Wipeout(_) => "Wipeout",
        EntityType::Insert(_) => "Insert",
        EntityType::Viewport(_) => "Viewport",
        EntityType::Block(_) => "Block",
        EntityType::BlockEnd(_) => "BlockEnd",
        EntityType::Dimension(_) => "Dimension",
        EntityType::Leader(_) => "Leader",
        EntityType::MLine(_) => "MLine",
        EntityType::Point(_) => "Point",
        EntityType::Solid(_) => "Solid",
        EntityType::Face3D(_) => "Face3D",
        EntityType::RasterImage(_) => "RasterImage",
        EntityType::Solid3D(_) => "Solid3D",
        EntityType::Region(_) => "Region",
        EntityType::Body(_) => "Body",
        EntityType::Ole2Frame(_) => "Ole2Frame",
        EntityType::Ray(_) => "Ray",
        EntityType::XLine(_) => "XLine",
        _ => "Other",
    }
}

fn is_unindexable(e: &EntityType) -> bool {
    matches!(
        e,
        EntityType::Insert(_)
            | EntityType::Viewport(_)
            | EntityType::Block(_)
            | EntityType::BlockEnd(_)
    )
}

fn world_aabb_f64(e: &EntityType) -> Option<[f64; 4]> {
    let bb = e.as_entity().bounding_box();
    let (xmin, ymin, xmax, ymax) = (bb.min.x, bb.min.y, bb.max.x, bb.max.y);
    if xmin == xmax && ymin == ymax {
        return None;
    }
    if !xmin.is_finite() || !ymin.is_finite() || !xmax.is_finite() || !ymax.is_finite() {
        return None;
    }
    Some([xmin, ymin, xmax, ymax])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).expect("usage: dump_index_stats <file>");
    let mut doc = if path.to_lowercase().ends_with(".dwg") {
        acadrust::io::dwg::DwgReader::from_file(&path)?.read()?
    } else {
        acadrust::io::dxf::DxfReader::from_file(&path)?.read()?
    };
    let base_dir = std::path::Path::new(&path)
        .parent()
        .unwrap_or(std::path::Path::new("."));
    let _ = H7CAD::io::xref::resolve_xrefs(&mut doc, base_dir);

    let model_block = doc.header.model_space_block_handle;
    println!("file: {}", path);
    println!("model_space_block_handle: {:?}", model_block);
    println!("total entities: {}", doc.entities().count());
    println!();

    let mut total: BTreeMap<&str, usize> = BTreeMap::new();
    let mut indexed: BTreeMap<&str, usize> = BTreeMap::new();
    let mut unbounded: BTreeMap<&str, usize> = BTreeMap::new();
    let mut unindexable_count: BTreeMap<&str, usize> = BTreeMap::new();
    let mut top_level: BTreeMap<&str, usize> = BTreeMap::new();
    let mut block_internal: BTreeMap<&str, usize> = BTreeMap::new();
    let mut union: Option<[f64; 4]> = None;

    for e in doc.entities() {
        let k = entity_kind(e);
        *total.entry(k).or_default() += 1;
        let owner = e.common().owner_handle;
        if owner == model_block {
            *top_level.entry(k).or_default() += 1;
        } else {
            *block_internal.entry(k).or_default() += 1;
        }
        if is_unindexable(e) {
            *unindexable_count.entry(k).or_default() += 1;
            continue;
        }
        match world_aabb_f64(e) {
            Some(ab) => {
                *indexed.entry(k).or_default() += 1;
                union = Some(match union {
                    None => ab,
                    Some(u) => [
                        u[0].min(ab[0]),
                        u[1].min(ab[1]),
                        u[2].max(ab[2]),
                        u[3].max(ab[3]),
                    ],
                });
            }
            None => *unbounded.entry(k).or_default() += 1,
        }
    }

    println!("──── per type ────────────────────────────────────────────────");
    println!(
        "{:<14} {:>6} {:>6} {:>6} {:>6} {:>10} {:>10}",
        "Type", "total", "idx", "unb", "skip", "top-lvl", "blk-int"
    );
    let mut all_keys: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    all_keys.extend(total.keys());
    for k in &all_keys {
        println!(
            "{:<14} {:>6} {:>6} {:>6} {:>6} {:>10} {:>10}",
            k,
            total.get(k).copied().unwrap_or(0),
            indexed.get(k).copied().unwrap_or(0),
            unbounded.get(k).copied().unwrap_or(0),
            unindexable_count.get(k).copied().unwrap_or(0),
            top_level.get(k).copied().unwrap_or(0),
            block_internal.get(k).copied().unwrap_or(0),
        );
    }

    println!();
    println!("union of indexed AABBs: {:?}", union);

    // Sample top-level entity bboxes by type
    println!();
    println!("──── sample top-level entity bboxes (first 5 per type) ────────");
    let mut shown: BTreeMap<&str, usize> = BTreeMap::new();
    for e in doc.entities() {
        let k = entity_kind(e);
        if e.common().owner_handle != model_block {
            continue;
        }
        let c = shown.entry(k).or_insert(0);
        if *c >= 5 {
            continue;
        }
        *c += 1;
        let bb = e.as_entity().bounding_box();
        println!(
            "  {:<12} h={:?} bb=({:.3},{:.3})→({:.3},{:.3})",
            k,
            e.common().handle,
            bb.min.x, bb.min.y, bb.max.x, bb.max.y,
        );
    }

    Ok(())
}
