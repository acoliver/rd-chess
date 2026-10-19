#[cfg(test)]
#[path = "sculpt_tests.rs"]
mod tests;

use super::Mesh;
use glam::Vec3;
use std::f32::consts::{PI, TAU};

fn empty() -> Mesh {
    Mesh {
        positions: vec![],
        normals: vec![],
        uvs: vec![],
        indices: vec![],
    }
}
fn merge(m: &mut Mesh, other: Mesh) {
    let base = m.positions.len() as u32;
    m.positions.extend(other.positions);
    m.normals.extend(other.normals);
    m.uvs.extend(other.uvs);
    m.indices
        .extend(other.indices.into_iter().map(|i| i + base));
}

// Shape-preserving Hermite tangents keep the small beads from overshooting into adjacent collars.
fn tangent(p: &[(f32, f32)], i: usize) -> f32 {
    let slope = |a: usize, b: usize| (p[b].1 - p[a].1) / (p[b].0 - p[a].0);
    if i == 0 {
        return slope(0, 1);
    }
    if i + 1 == p.len() {
        return slope(i - 1, i);
    }
    let a = slope(i - 1, i);
    let b = slope(i, i + 1);
    if a * b <= 0. {
        0.
    } else {
        2. * a * b / (a + b)
    }
}
fn sample(p: &[(f32, f32)], i: usize, t: f32) -> (f32, f32, f32) {
    let (z, start_value) = p[i];
    let (next_z, next_value) = p[i + 1];
    let dz = next_z - z;
    let a = tangent(p, i) * dz;
    let b = tangent(p, i + 1) * dz;
    let value = (2. * t * t * t - 3. * t * t + 1.) * start_value
        + (t * t * t - 2. * t * t + t) * a
        + (-2. * t * t * t + 3. * t * t) * next_value
        + (t * t * t - t * t) * b;
    let derivative = (6. * t * t - 6. * t) * start_value
        + (3. * t * t - 4. * t + 1.) * a
        + (-6. * t * t + 6. * t) * next_value
        + (3. * t * t - 2. * t) * b;
    (z + t * dz, value, derivative / dz)
}
fn sample_radius(p: &[(f32, f32)], i: usize, t: f32) -> (f32, f32, f32) {
    let (z, radius, derivative) = sample(p, i, t);
    if radius <= 0. {
        // The bounded profile is flat at the axis; its normals must use the bounded slope too.
        (z, 0., 0.)
    } else {
        (z, radius, derivative)
    }
}
fn lathe(p: &[(f32, f32)]) -> Mesh {
    let mut rows = Vec::new();
    for i in 0..p.len() - 1 {
        for j in 0..8 {
            rows.push(sample_radius(p, i, j as f32 / 8.));
        }
    }
    rows.push(sample_radius(p, p.len() - 2, 1.));
    let mut m = empty();
    let sides = 96;
    for (row, &(z, radius, slope)) in rows.iter().enumerate() {
        for s in 0..=sides {
            let a = TAU * s as f32 / sides as f32;
            let n = if radius < 0.001 {
                Vec3::new(0., 0., if row == 0 { -1. } else { 1. })
            } else {
                Vec3::new(a.cos(), a.sin(), -slope).normalize()
            };
            m.append(
                [radius * a.cos(), radius * a.sin(), z],
                n.to_array(),
                [s as f32 / sides as f32, z / 550.],
            );
        }
    }
    for r in 0..rows.len() - 1 {
        for s in 0..sides {
            let a = (r * (sides + 1) + s) as u32;
            let b = a + sides as u32 + 1;
            if rows[r].1 > 0.001 {
                m.tri(a, a + 1, b);
            }
            if rows[r + 1].1 > 0.001 {
                m.tri(a + 1, b + 1, b);
            }
        }
    }
    for (row, sign) in [(0, -1.), (rows.len() - 1, 1.)] {
        let (z, radius, _) = rows[row];
        if radius < 0.001 {
            continue;
        }
        let base = m.positions.len() as u32;
        m.append([0., 0., z], [0., 0., sign], [0.5, 0.5]);
        for s in 0..=sides {
            let a = TAU * s as f32 / sides as f32;
            m.append(
                [radius * a.cos(), radius * a.sin(), z],
                [0., 0., sign],
                [a.cos() * 0.5 + 0.5, a.sin() * 0.5 + 0.5],
            );
        }
        for s in 0..sides as u32 {
            if sign > 0. {
                m.tri(base, base + s + 1, base + s + 2);
            } else {
                m.tri(base, base + s + 2, base + s + 1);
            }
        }
    }
    m
}
fn body(radius: f32, shoulder: f32, stem: f32, collar: f32) -> Mesh {
    lathe(&[
        (0., radius * 0.91),
        (5., radius * 0.98),
        (12., radius),
        (20., radius),
        (26., radius * 0.975),
        (31., radius * 0.93),
        (36., radius * 0.94),
        (42., radius * 0.975),
        (49., radius * 0.96),
        (56., radius * 0.89),
        (64., radius * 0.86),
        (70., radius * 0.87),
        (76., radius * 0.85),
        (86., radius * 0.76),
        (105., radius * 0.62),
        (shoulder, stem * 1.17),
        (collar - 37., stem),
        (collar - 23., stem * 1.08),
        (collar - 16., stem * 1.39),
        (collar - 10., stem * 1.48),
        (collar - 4., stem * 1.4),
        (collar, stem * 1.12),
    ])
}
pub(super) fn pawn_mesh() -> Mesh {
    let mut m = body(350., 150., 102., 245.);
    ellipsoid(&mut m, [0., 0., 294.], [161., 161., 54.]);
    m
}
pub(super) fn rook_mesh() -> Mesh {
    let mut m = body(385., 145., 205., 325.);
    merge(
        &mut m,
        lathe(&[
            (320., 260.),
            (327., 290.),
            (337., 313.),
            (345., 316.),
            (366., 316.),
            (374., 304.),
            (375., 238.),
        ]),
    );
    // Thick radial merlons, rather than an eight-post fence.
    for i in 0..6 {
        let angle = TAU * i as f32 / 6.;
        let mut block = rounded_box([276., 0., 391.], [50., 83., 37.], 12.);
        for (p, n) in block.positions.iter_mut().zip(&mut block.normals) {
            let rotate = |v: [f32; 3]| {
                [
                    v[0] * angle.cos() - v[1] * angle.sin(),
                    v[0] * angle.sin() + v[1] * angle.cos(),
                    v[2],
                ]
            };
            *p = rotate(*p);
            *n = rotate(*n);
        }
        merge(&mut m, block);
    }
    m
}
pub(super) fn bishop_mesh() -> Mesh {
    let mut m = body(368., 164., 106., 309.);
    let head = lathe(&[
        (309., 65.),
        (321., 107.),
        (344., 146.),
        (370., 164.),
        (388., 159.),
        (414., 125.),
        (440., 72.),
        (458., 26.),
        (465., 0.),
    ]);
    // A real diagonal open slot, with polished planar cut walls and a closed floor.
    let bottom = clip(&head, Vec3::Z, 385.);
    let upper = clip(&head, -Vec3::Z, -385.);
    let plane = Vec3::new(1., 0., -1.3).normalize();
    let offset = -1.3 * 400. / (1_f32 + 1.3 * 1.3).sqrt();
    merge(&mut m, bottom);
    merge(&mut m, clip(&upper, plane, offset - 11.));
    merge(&mut m, clip(&upper, -plane, -offset - 11.));
    ellipsoid(&mut m, [0., 0., 468.], [27., 27., 9.]);
    m
}
pub(super) fn queen_mesh() -> Mesh {
    let mut m = body(382., 165., 134., 342.);
    merge(
        &mut m,
        lathe(&[
            (338., 191.),
            (347., 213.),
            (360., 217.),
            (378., 230.),
            (395., 252.),
            (406., 253.),
            (412., 244.),
            (414., 204.),
        ]),
    );
    for i in 0..8 {
        let a = TAU * i as f32 / 8.;
        // Rounded tapered crown points rise from a substantial coronet.
        let mut point = empty();
        ellipsoid(
            &mut point,
            [238. * a.cos(), 238. * a.sin(), 426.],
            [32., 32., 30.],
        );
        merge(&mut m, point);
        ellipsoid(
            &mut m,
            [245. * a.cos(), 245. * a.sin(), 455.],
            [36., 36., 12.],
        );
    }
    ellipsoid(&mut m, [0., 0., 430.], [76., 76., 26.]);
    ellipsoid(&mut m, [0., 0., 465.], [36., 36., 12.]);
    m
}
pub(super) fn king_mesh() -> Mesh {
    let mut m = body(388., 177., 139., 363.);
    merge(
        &mut m,
        lathe(&[
            (359., 199.),
            (368., 225.),
            (380., 233.),
            (389., 213.),
            (399., 162.),
            (407., 112.),
        ]),
    );
    ellipsoid(&mut m, [0., 0., 411.], [112., 112., 22.]);
    merge(&mut m, rounded_box([0., 0., 476.], [39., 43., 69.], 12.));
    merge(&mut m, rounded_box([0., 0., 484.], [113., 43., 19.], 11.));
    m
}
pub(super) fn knight_mesh() -> Mesh {
    let mut m = body(378., 143., 146., 238.);
    loft(
        &mut m,
        &[
            (223., -20., 156., 118.),
            (255., -40., 137., 105.),
            (305., -66., 111., 88.),
            (356., -62., 96., 77.),
            (402., -25., 99., 73.),
            (439., 12., 79., 66.),
            (461., 12., 52., 48.),
        ],
    );
    let mut head = empty();
    loft(
        &mut head,
        &[
            (-40., 432., 25., 47.),
            (-10., 438., 39., 65.),
            (50., 432., 42., 78.),
            (115., 415., 33., 67.),
            (185., 395., 23., 64.),
            (248., 390., 17., 48.),
        ],
    );
    // Reorient the section sweep along the muzzle, preserving outward handedness.
    for (p, n) in head.positions.iter_mut().zip(&mut head.normals) {
        p.swap(0, 2);
        n.swap(0, 2);
    }
    for triangle in head.indices.chunks_exact_mut(3) {
        triangle.swap(1, 2);
    }
    merge(&mut m, head);
    for y in [-44., 44.] {
        loft_offset(
            &mut m,
            &[
                (449., -8., 28., 23.),
                (475., -17., 21., 16.),
                (501., -28., 5., 5.),
            ],
            y,
        );
    }
    // Swept mane follows the rear neck; small scallops catch light without becoming a flat fin.
    loft(
        &mut m,
        &[
            (236., -137., 27., 60.),
            (285., -161., 25., 52.),
            (340., -158., 24., 45.),
            (388., -119., 24., 39.),
            (432., -69., 23., 35.),
            (463., -28., 15., 27.),
        ],
    );
    for i in 0..7 {
        let t = i as f32 / 6.;
        ellipsoid(
            &mut m,
            [-168. + 110. * t * t, 0., 280. + 166. * t],
            [9., 40. - 16. * t, 7.],
        );
    }
    m
}

#[path = "sculpt_surface.rs"]
mod surface;
pub(super) use surface::ellipsoid;
use surface::{clip, loft, loft_offset, rounded_box};
