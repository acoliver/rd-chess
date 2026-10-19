use super::*;

#[test]
fn hermite_preserves_signed_values_and_their_derivatives() {
    let profile = [(0., -6.), (10., -2.), (20., 2.)];
    for i in 0..2 {
        for t in [0., 0.25, 0.5, 0.75, 1.] {
            let (z, value, derivative) = sample(&profile, i, t);
            assert!((value - (-6. + 0.4 * z)).abs() < 0.00001);
            assert!((derivative - 0.4).abs() < 0.00001);
        }
    }
}

#[test]
fn radius_bounds_flatten_only_nonpositive_values_and_slopes() {
    let profile = [(0., -6.), (4., -2.), (8., 2.)];
    for (i, t, expected_radius, expected_slope) in [
        (0, 0., 0., 0.),
        (0, 0.5, 0., 0.),
        (1, 0.25, 0., 0.),
        (1, 0.5, 0., 0.),
        (1, 0.75, 1., 1.),
        (1, 1., 2., 1.),
    ] {
        let (_, radius, slope) = sample_radius(&profile, i, t);
        assert!((radius - expected_radius).abs() < 0.00001);
        assert!((slope - expected_slope).abs() < 0.00001);
        if t > 0. && t < 1. && !(i == 1 && (t - 0.5).abs() < 0.0001) {
            let (z0, r0, _) = sample_radius(&profile, i, t - 0.001);
            let (z1, r1, _) = sample_radius(&profile, i, t + 0.001);
            assert!((slope - (r1 - r0) / (z1 - z0)).abs() < 0.0001);
        }
    }
}

#[test]
fn knight_cross_sections_keep_signed_neck_mane_and_ear_centers() {
    let mesh = knight_mesh();
    for (z, center, radius_x, radius_y, y_centers) in [
        (305., -66., 111., 88., &[0.][..]),
        (285., -161., 25., 52., &[0.][..]),
        (475., -17., 21., 16., &[-44., 44.][..]),
    ] {
        let ring: Vec<_> = mesh
            .positions
            .iter()
            .filter(|p| (p[2] - z).abs() < 0.0001)
            .collect();
        assert_eq!(ring.len(), 65 * y_centers.len(), "section at {z}");
        let min_x = ring.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
        let max_x = ring.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
        assert!(
            (min_x - (center - radius_x)).abs() < 0.001,
            "section at {z}: min x {min_x}"
        );
        assert!(
            (max_x - (center + radius_x)).abs() < 0.001,
            "section at {z}: max x {max_x}"
        );
        for &y in y_centers {
            for s in 0..64 {
                let angle = TAU * s as f32 / 64.;
                let expected = Vec3::new(
                    center + radius_x * angle.cos(),
                    y + radius_y * angle.sin(),
                    z,
                );
                assert!(
                    ring.iter()
                        .any(|p| Vec3::from(**p).distance(expected) < 0.001),
                    "missing signed ellipse vertex at {expected}"
                );
            }
        }
    }
}

#[test]
fn knight_sweep_normals_match_actual_surface_tangents() {
    let mesh = knight_mesh();
    // Interior rows of the real neck, mane and ears, away from caps and knot extrema.
    for (z, count) in [(273.75, 1), (260.5, 1), (462., 2)] {
        let indices: Vec<_> = mesh
            .positions
            .iter()
            .enumerate()
            .filter(|(_, p)| (p[2] - z).abs() < 0.0001)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(indices.len(), count * 65, "section at {z}");
        for ring in indices.chunks_exact(65) {
            for &i in &ring[1..64] {
                let around = Vec3::from(mesh.positions[i + 1]) - Vec3::from(mesh.positions[i - 1]);
                let along = Vec3::from(mesh.positions[i + 65]) - Vec3::from(mesh.positions[i - 65]);
                let geometric_normal = around.cross(along).normalize();
                let normal = Vec3::from(mesh.normals[i]);
                assert!((normal.length() - 1.).abs() < 0.00001);
                assert!(
                    geometric_normal.dot(normal) > 0.9995,
                    "normal differs from actual surface at {:?}: agreement {}",
                    mesh.positions[i],
                    geometric_normal.dot(normal)
                );
                assert!(normal.dot(around.normalize()).abs() < 0.0001);
                assert!(normal.dot(along.normalize()).abs() < 0.03);
            }
        }
    }
}
