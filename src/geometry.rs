/// Indexed triangle mesh with per-vertex smooth normals and UVs.
#[derive(Clone, Debug)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}
impl Mesh {
    fn append(&mut self, p: [f32; 3], n: [f32; 3], uv: [f32; 2]) {
        self.positions.push(p);
        self.normals.push(n);
        self.uvs.push(uv);
    }
    fn tri(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend([a, b, c]);
    }
}
/// Six reusable sculpted archetypes. The board and decorative frame are separate authored meshes.
pub fn piece_meshes() -> [Mesh; 6] {
    [
        pawn_mesh(),
        rook_mesh(),
        knight_mesh(),
        bishop_mesh(),
        queen_mesh(),
        king_mesh(),
    ]
    .map(stretch_detail)
}
#[path = "sculpt.rs"]
mod sculpt;
use sculpt::{bishop_mesh, king_mesh, knight_mesh, pawn_mesh, queen_mesh, rook_mesh};

pub(super) fn append_box(m: &mut Mesh, c: [f32; 3], h: [f32; 3]) {
    let faces = [
        (
            [-1., 0., 0.],
            [
                [-1., -1., -1.],
                [-1., 1., -1.],
                [-1., 1., 1.],
                [-1., -1., 1.],
            ],
        ),
        (
            [1., 0., 0.],
            [[1., -1., -1.], [1., -1., 1.], [1., 1., 1.], [1., 1., -1.]],
        ),
        (
            [0., -1., 0.],
            [
                [-1., -1., -1.],
                [-1., -1., 1.],
                [1., -1., 1.],
                [1., -1., -1.],
            ],
        ),
        (
            [0., 1., 0.],
            [[-1., 1., -1.], [1., 1., -1.], [1., 1., 1.], [-1., 1., 1.]],
        ),
        (
            [0., 0., -1.],
            [
                [-1., -1., -1.],
                [1., -1., -1.],
                [1., 1., -1.],
                [-1., 1., -1.],
            ],
        ),
        (
            [0., 0., 1.],
            [[-1., -1., 1.], [-1., 1., 1.], [1., 1., 1.], [1., -1., 1.]],
        ),
    ];
    for (normal, corners) in faces {
        let base = m.positions.len() as u32;
        for (index, v) in corners.into_iter().enumerate() {
            m.append(
                [c[0] + v[0] * h[0], c[1] + v[1] * h[1], c[2] + v[2] * h[2]],
                normal,
                [[0., 0.], [1., 0.], [1., 1.], [0., 1.]][index],
            );
        }
        m.tri(base, base + 2, base + 1);
        m.tri(base, base + 3, base + 2);
    }
}
/// The 8x8 playing surface; scene assembly adds a solid frame and inset metal rails.
pub fn board_mesh() -> Mesh {
    let mut m = Mesh {
        positions: vec![],
        normals: vec![],
        uvs: vec![],
        indices: vec![],
    };
    for rank in 0..8 {
        for file in 0..8 {
            let x = file as f32 * 1000. - 4000.;
            let y = rank as f32 * 1000. - 4000.;
            append_tile(
                &mut m,
                x,
                y,
                1000.,
                1000.,
                50.,
                file as f32 / 8.,
                rank as f32 / 8.,
                1. / 8.,
                1. / 8.,
            );
        }
    }
    m
}
fn append_tile(
    m: &mut Mesh,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    z: f32,
    u: f32,
    v: f32,
    du: f32,
    dv: f32,
) {
    let b = m.positions.len() as u32;
    for (px, py, uv) in [
        (x, y, [u, v]),
        (x + w, y, [u + du, v]),
        (x + w, y + h, [u + du, v + dv]),
        (x, y + h, [u, v + dv]),
    ] {
        m.append([px, py, z], [0., 0., 1.], uv);
    }
    m.tri(b, b + 1, b + 2);
    m.tri(b, b + 2, b + 3);
}

pub(super) fn studio_floor_mesh() -> Mesh {
    let mut m = Mesh {
        positions: vec![],
        normals: vec![],
        uvs: vec![],
        indices: vec![],
    };
    append_tile(
        &mut m, -200_000., -200_000., 400_000., 400_000., -250., 0., 0., 1., 1.,
    );
    m
}

pub(super) fn stretch_detail(mut mesh: Mesh) -> Mesh {
    for p in &mut mesh.positions {
        p[2] *= 3.;
    }
    for n in &mut mesh.normals {
        *n = glam::Vec3::new(n[0], n[1], n[2] / 3.)
            .normalize()
            .to_array();
    }
    mesh
}

pub(super) fn board_frame_mesh() -> Mesh {
    let mut m = Mesh {
        positions: vec![],
        normals: vec![],
        uvs: vec![],
        indices: vec![],
    };
    append_box(&mut m, [0., 0., -125.], [4400., 4400., 125.]);
    // A stepped rim remains below the tile surface and outside the playing area.
    for edge in [-4250., 4250.] {
        append_box(&mut m, [edge, 0., 0.], [150., 4400., 40.]);
        append_box(&mut m, [0., edge, 0.], [4100., 150., 40.]);
    }
    m
}
pub(super) fn board_inlay_mesh() -> Mesh {
    let mut m = Mesh {
        positions: vec![],
        normals: vec![],
        uvs: vec![],
        indices: vec![],
    };
    for edge in [-4340., -4080., 4080., 4340.] {
        append_box(&mut m, [edge, 0., 42.], [9., 4349., 3.]);
        append_box(&mut m, [0., edge, 42.], [4349., 9., 3.]);
    }
    m
}

pub(super) fn knight_eye_mesh() -> Mesh {
    let mut m = Mesh {
        positions: vec![],
        normals: vec![],
        uvs: vec![],
        indices: vec![],
    };
    for side in [-1., 1.] {
        sculpt::ellipsoid(&mut m, [54., side * 76., 440.], [14., 7., 6.]);
    }
    m
}
