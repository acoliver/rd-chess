pub(crate) fn marker_mesh() -> crate::Mesh {
    let mut mesh = crate::Mesh {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        indices: Vec::new(),
    };
    for segment in 0_u16..32 {
        let angle = f32::from(segment) / 32. * std::f32::consts::TAU;
        for radius in [180., 250.] {
            mesh.positions
                .push([angle.cos() * radius, angle.sin() * radius, 0.]);
            mesh.normals.push([0., 0., 1.]);
            mesh.uvs.push([0.5, 0.5]);
        }
        let inner = u32::from(segment) * 2;
        let next = (inner + 2) % 64;
        mesh.indices
            .extend([inner, inner + 1, next + 1, inner, next + 1, next]);
    }
    mesh
}
