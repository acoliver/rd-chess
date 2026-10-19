use super::{Mesh, PI, TAU, Vec3, empty, merge, sample, sample_radius};

pub(in super::super) fn ellipsoid(m: &mut Mesh, c: [f32; 3], r: [f32; 3]) {
    let mut sphere = empty();
    let sides = 64;
    let rings = 32;
    for j in 0..=rings {
        let t = PI * j as f32 / rings as f32;
        for i in 0..=sides {
            let a = TAU * i as f32 / sides as f32;
            let v = Vec3::new(t.sin() * a.cos(), t.sin() * a.sin(), -t.cos());
            sphere.append(
                (Vec3::from(c) + v * Vec3::from(r)).to_array(),
                (v / Vec3::from(r)).normalize().to_array(),
                [i as f32 / sides as f32, j as f32 / rings as f32],
            );
        }
    }
    for j in 0..rings {
        for i in 0..sides {
            let a = (j * (sides + 1) + i) as u32;
            let b = a + sides as u32 + 1;
            if j > 0 {
                sphere.tri(a, a + 1, b);
            }
            if j + 1 < rings {
                sphere.tri(a + 1, b + 1, b);
            }
        }
    }
    merge(m, sphere);
}

pub(super) fn rounded_box(c: [f32; 3], h: [f32; 3], bevel: f32) -> Mesh {
    let mut m = empty();
    let core = Vec3::from(h) - Vec3::splat(bevel);
    for axis in 0..3 {
        for sign in [-1., 1.] {
            let base = m.positions.len() as u32;
            let steps = 8;
            for j in 0..=steps {
                for i in 0..=steps {
                    let mut p = Vec3::ZERO;
                    p[axis] = h[axis] * sign;
                    p[(axis + 1) % 3] = h[(axis + 1) % 3] * (2. * i as f32 / steps as f32 - 1.);
                    p[(axis + 2) % 3] = h[(axis + 2) % 3] * (2. * j as f32 / steps as f32 - 1.);
                    let inner = p.clamp(-core, core);
                    let n = (p - inner).normalize();
                    m.append(
                        (Vec3::from(c) + inner + n * bevel).to_array(),
                        n.to_array(),
                        [i as f32 / steps as f32, j as f32 / steps as f32],
                    );
                }
            }
            for j in 0..steps {
                for i in 0..steps {
                    let a = base + (j * (steps + 1) + i) as u32;
                    let b = a + steps as u32 + 1;
                    if sign > 0. {
                        m.tri(a, a + 1, b);
                        m.tri(a + 1, b + 1, b);
                    } else {
                        m.tri(a, b, a + 1);
                        m.tri(a + 1, b, b + 1);
                    }
                }
            }
        }
    }
    m
}

pub(super) fn loft(m: &mut Mesh, sections: &[(f32, f32, f32, f32)]) {
    loft_offset(m, sections, 0.);
}
pub(super) fn loft_offset(m: &mut Mesh, sections: &[(f32, f32, f32, f32)], y: f32) {
    let x: Vec<_> = sections.iter().map(|s| (s.0, s.1)).collect();
    let rx: Vec<_> = sections.iter().map(|s| (s.0, s.2)).collect();
    let ry: Vec<_> = sections.iter().map(|s| (s.0, s.3)).collect();
    let mut shape = empty();
    let sides = 64;
    let mut rows = Vec::new();
    for i in 0..sections.len() - 1 {
        for j in 0..8 {
            rows.push((i, j as f32 / 8.));
        }
    }
    rows.push((sections.len() - 2, 1.));
    for &(i, t) in &rows {
        let (z, c, dc) = sample(&x, i, t);
        let (_, a, da) = sample_radius(&rx, i, t);
        let (_, b, db) = sample_radius(&ry, i, t);
        for s in 0..=sides {
            let angle = TAU * s as f32 / sides as f32;
            let (sin, cos) = angle.sin_cos();
            let n = Vec3::new(
                b * cos,
                a * sin,
                -b * cos * (dc + da * cos) - a * db * sin * sin,
            )
            .normalize();
            shape.append(
                [c + a * cos, y + b * sin, z],
                n.to_array(),
                [s as f32 / sides as f32, z / 550.],
            );
        }
    }
    for r in 0..rows.len() - 1 {
        for s in 0..sides {
            let a = (r * (sides + 1) + s) as u32;
            let b = a + sides as u32 + 1;
            shape.tri(a, a + 1, b);
            shape.tri(a + 1, b + 1, b);
        }
    }
    for (row, sign) in [(0, -1.), (rows.len() - 1, 1.)] {
        let (i, t) = rows[row];
        let (z, c, _) = sample(&x, i, t);
        let base = shape.positions.len() as u32;
        shape.append([c, y, z], [0., 0., sign], [0.5, 0.5]);
        for s in 0..=sides {
            let p = shape.positions[row * (sides + 1) + s];
            shape.append(p, [0., 0., sign], [s as f32 / sides as f32, 0.]);
        }
        for s in 0..sides as u32 {
            if sign > 0. {
                shape.tri(base, base + s + 1, base + s + 2);
            } else {
                shape.tri(base, base + s + 2, base + s + 1);
            }
        }
    }
    merge(m, shape);
}

// Clip a convex smooth shell and close the cut with a separate, hard-normal cap.
pub(super) fn clip(source: &Mesh, normal: Vec3, distance: f32) -> Mesh {
    let mut m = empty();
    let mut boundary = Vec::new();
    for tri in source.indices.chunks_exact(3) {
        let mut polygon = Vec::new();
        for e in 0..3 {
            let a = tri[e] as usize;
            let b = tri[(e + 1) % 3] as usize;
            let p = Vec3::from(source.positions[a]);
            let q = Vec3::from(source.positions[b]);
            let n = Vec3::from(source.normals[a]);
            let k = Vec3::from(source.normals[b]);
            let da = p.dot(normal) - distance;
            let db = q.dot(normal) - distance;
            if da <= 0. {
                polygon.push((p, n));
            }
            if (da < 0. && db > 0.) || (da > 0. && db < 0.) {
                let t = da / (da - db);
                let v = p.lerp(q, t);
                polygon.push((v, n.lerp(k, t).normalize()));
                boundary.push(v);
            }
        }
        for i in 1..polygon.len().saturating_sub(1) {
            let vertices = [polygon[0], polygon[i], polygon[i + 1]];
            if (vertices[1].0 - vertices[0].0)
                .cross(vertices[2].0 - vertices[0].0)
                .length()
                < 0.1
            {
                continue;
            }
            let base = m.positions.len() as u32;
            for (p, n) in vertices {
                m.append(p.to_array(), n.to_array(), [p.x / 500., p.z / 550.]);
            }
            m.tri(base, base + 1, base + 2);
        }
    }
    boundary.sort_by(|a, b| {
        a.x.total_cmp(&b.x)
            .then(a.y.total_cmp(&b.y))
            .then(a.z.total_cmp(&b.z))
    });
    boundary.dedup_by(|a, b| a.distance(*b) < 0.01);
    if boundary.len() > 2 {
        let center = boundary.iter().copied().sum::<Vec3>() / boundary.len() as f32;
        let u = normal.any_orthonormal_vector();
        let v = normal.cross(u);
        boundary.sort_by(|a, b| {
            let a = *a - center;
            let b = *b - center;
            a.dot(v)
                .atan2(a.dot(u))
                .total_cmp(&b.dot(v).atan2(b.dot(u)))
        });
        for i in 0..boundary.len() {
            let a = boundary[i];
            let b = boundary[(i + 1) % boundary.len()];
            if (a - center).cross(b - center).length() < 0.1 {
                continue;
            }
            let base = m.positions.len() as u32;
            for p in [center, a, b] {
                m.append(p.to_array(), normal.to_array(), [p.x / 500., p.z / 550.]);
            }
            m.tri(base, base + 1, base + 2);
        }
    }
    m
}
