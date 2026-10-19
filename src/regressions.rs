use super::*;
use crate::geometry::append_box;
use glam::Vec3;

#[test]
fn tiles_are_centered_on_the_saved_square_coordinates() {
    let mesh = board_mesh();
    for rank in 0..8 {
        for file in 0..8 {
            let tile = &mesh.positions[(rank * 8 + file) * 4..][..4];
            let center = tile.iter().copied().map(Vec3::from).sum::<Vec3>() / 4.;
            assert!((center.x - (file as f32 * 1000. - 3500.)).abs() < 0.001);
            assert!((center.y - (rank as f32 * 1000. - 3500.)).abs() < 0.001);
        }
    }
}

#[test]
fn box_winding_agrees_with_outward_face_normals() {
    let mut mesh = Mesh {
        positions: vec![],
        normals: vec![],
        uvs: vec![],
        indices: vec![],
    };
    append_box(&mut mesh, [0.; 3], [1.; 3]);
    for triangle in mesh.indices.chunks_exact(3) {
        let [a, b, c] = triangle_points(triangle, &mesh.positions);
        let normal = (b - a).cross(c - a).normalize();
        assert!(normal.dot(Vec3::from(mesh.normals[triangle[0] as usize])) > 0.99);
        assert!(normal.dot(a) > 0.);
    }
}

fn triangle_points(triangle: &[u32], points: &[[f32; 3]]) -> [Vec3; 3] {
    std::array::from_fn(|i| Vec3::from(points[triangle[i] as usize]))
}

#[test]
fn sculpts_have_upright_chess_proportions() {
    for mesh in piece_meshes() {
        let height = mesh.positions.iter().map(|p| p[2]).fold(0., f32::max);
        assert!((900. ..=1900.).contains(&height), "sculpt height: {height}");
    }
}

#[test]
fn sculptures_have_non_degenerate_triangles_and_consistent_unit_normals() {
    for (kind, mesh) in piece_meshes().iter().enumerate() {
        for normal in &mesh.normals {
            assert!((Vec3::from(*normal).length() - 1.).abs() < 0.0001);
        }
        for triangle in mesh.indices.chunks_exact(3) {
            let [a, b, c] = triangle_points(triangle, &mesh.positions);
            let cross = (b - a).cross(c - a);
            assert!(
                cross.length() > 0.01,
                "degenerate triangle in kind {kind}: {triangle:?}"
            );
            for index in triangle {
                assert!(
                    cross
                        .normalize()
                        .dot(Vec3::from(mesh.normals[*index as usize]))
                        > 0.,
                    "winding/normal mismatch in kind {kind}: {triangle:?}"
                );
            }
        }
    }
}

#[test]
fn turned_bodies_have_finely_sampled_profiles_and_closed_undersides() {
    for mesh in piece_meshes() {
        let levels: std::collections::BTreeSet<_> = mesh
            .positions
            .iter()
            .filter(|p| p[2] < 600.)
            .map(|p| (p[2] * 100.) as i32)
            .collect();
        assert!(
            levels.len() > 75,
            "sparse turned base: {} levels",
            levels.len()
        );
        let mut underside_area = 0.;
        for triangle in mesh.indices.chunks_exact(3) {
            let [a, b, c] = triangle_points(triangle, &mesh.positions);
            if a.z == 0. && b.z == 0. && c.z == 0. {
                let cross = (b - a).cross(c - a);
                assert!(cross.z < 0., "underside faces inward");
                underside_area += cross.length() / 2.;
            }
        }
        assert!(
            underside_area > 300_000.,
            "missing or incomplete underside cap"
        );
    }
}

#[test]
fn facade_quantization_does_not_reintroduce_degenerate_triangles() {
    let scene = render_scene(&Position::initial(342)).unwrap();
    for (kind, mesh) in scene.geometry().meshes().iter().enumerate() {
        for lod in mesh.lods() {
            for triangle in lod.indices().chunks_exact(3) {
                let [a, b, c] = std::array::from_fn(|i| {
                    Vec3::from(
                        lod.vertices()[triangle[i] as usize]
                            .position_millimeters
                            .map(|v| v as f32),
                    )
                });
                assert!(
                    (b - a).cross(c - a).length_squared() > 0.,
                    "quantized degenerate mesh {kind}: {triangle:?}"
                );
            }
        }
    }
}

#[test]
fn marble_is_dielectric_and_gold_inlay_is_metallic() {
    let scene = render_scene(&Position::initial(342)).unwrap();
    let materials = scene.resources().materials();
    for material in &materials[..6] {
        assert_eq!(material.metalness_micro(), 0);
    }
    for index in [0, 1, 2, 5] {
        assert!((180_000..=300_000).contains(&materials[index].roughness_micro()));
    }
    assert_eq!(materials[6].metalness_micro(), 1_000_000);
    assert_eq!(materials[6].roughness_micro(), 260_000);
}

#[test]
fn bishop_slot_is_open_above_its_closed_floor() {
    let mesh = &piece_meshes()[3];
    let plane = Vec3::new(1., 0., -1.3).normalize();
    let offset = -1.3 * 400. / (1_f32 + 1.3 * 1.3).sqrt();
    for triangle in mesh.indices.chunks_exact(3) {
        let p = triangle_points(triangle, &mesh.positions).map(|mut p| {
            p.z /= 3.;
            p
        });
        let center = (p[0] + p[1] + p[2]) / 3.;
        if center.z > 386. && center.z < 456. {
            assert!(
                (center.dot(plane) - offset).abs() >= 10.9,
                "triangle bridges mitre slot"
            );
        }
    }
    assert!(mesh.normals.iter().any(|n| n[2] > 0.99));
}
