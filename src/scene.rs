use super::{
    Mesh, PieceKind, Position, SceneBuildError, Side, board_albedo, board_mesh, marble_albedo,
    piece_meshes,
};
use crate::geometry::{
    board_frame_mesh, board_inlay_mesh, knight_eye_mesh, stretch_detail, studio_floor_mesh,
};

/// Converts the game's authored indexed assets and current position into the
/// renderer's validated scene contract. No renderer-internal crate is exposed.
pub fn render_scene(
    position: &Position,
) -> Result<worldrunner::scene::plan::RenderScene, SceneBuildError> {
    build_render_scene(position, true)
}

/// Builds a board-free six-archetype display for asset inspection captures.
pub fn piece_lineup_scene(
    position: &Position,
) -> Result<worldrunner::scene::plan::RenderScene, SceneBuildError> {
    build_render_scene(position, false)
}

fn build_render_scene(
    position: &Position,
    include_board: bool,
) -> Result<worldrunner::scene::plan::RenderScene, SceneBuildError> {
    use worldrunner::scene::plan::{
        MaterialHandle, MeshHandle, RenderScene, SceneAlphaMode, SceneAmbient, SceneAntiAlias,
        SceneCamera, SceneColor, SceneColorGrading, SceneDirection, SceneEffects,
        SceneEffectsParts, SceneGeometry, SceneMaterial, SceneMaterialParts, SceneObject,
        SceneParts, SceneResources, SceneShadowCascades, SceneSky, SceneSun, TextureHandle,
    };
    position.validate()?;
    let mut meshes = Vec::new();
    meshes.push(scene_mesh(&board_mesh()));
    meshes.push(scene_mesh(&studio_floor_mesh()));
    for mesh in piece_meshes() {
        meshes.push(scene_mesh(&mesh));
    }
    meshes.push(scene_mesh(&board_frame_mesh()));
    meshes.push(scene_mesh(&stretch_detail(knight_eye_mesh())));
    meshes.push(scene_mesh(&board_inlay_mesh()));
    let textures = [
        texture(512, 512, board_albedo(position.seed, 512)),
        texture(
            256,
            256,
            marble_albedo(position.seed ^ 0x1a01, 256, [225, 215, 190]),
        ),
        texture(
            256,
            256,
            marble_albedo(position.seed ^ 0x0b17, 256, [76, 79, 78]),
        ),
        texture(
            256,
            256,
            marble_albedo(position.seed ^ 0x5700, 256, [62, 65, 70]),
        ),
        texture(8, 8, marble_albedo(position.seed ^ 0x5eed, 8, [10, 12, 17])),
        texture(
            256,
            256,
            marble_albedo(position.seed ^ 0xf4a0, 256, [31, 63, 59]),
        ),
        texture(1, 1, vec![174, 139, 78, 255]),
    ];
    let materials = [0u32, 1, 2, 3, 4, 5, 6]
        .into_iter()
        .map(|i| {
            SceneMaterial::new(SceneMaterialParts {
                albedo: TextureHandle::from_raw(i),
                normal: None,
                roughness_metallic: None,
                ambient_occlusion: None,
                self_illumination: None,
                roughness_micro: match i {
                    6 => 260_000,
                    3 => 620_000,
                    4 => 300_000,
                    _ => 230_000,
                },
                metalness_micro: if i == 6 { 1_000_000 } else { 0 },
                alpha_mode: SceneAlphaMode::Opaque,
                double_sided: true,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut objects = if include_board {
        vec![SceneObject::new(
            identity_transform([0, 0, 0]),
            MeshHandle::from_raw(0),
            MaterialHandle::from_raw(0),
        )]
    } else {
        Vec::new()
    };
    objects.push(SceneObject::new(
        identity_transform([0, 0, 0]),
        MeshHandle::from_raw(1),
        MaterialHandle::from_raw(3),
    ));
    if include_board {
        for (mesh, material) in [(8, 5), (10, 6)] {
            objects.push(SceneObject::new(
                identity_transform([0, 0, 0]),
                MeshHandle::from_raw(mesh),
                MaterialHandle::from_raw(material),
            ));
        }
    }
    for piece in &position.pieces {
        let kind = match piece.kind {
            PieceKind::Pawn => 2,
            PieceKind::Rook => 3,
            PieceKind::Knight => 4,
            PieceKind::Bishop => 5,
            PieceKind::Queen => 6,
            PieceKind::King => 7,
        };
        let x = i64::from(piece.square.file) * 1000 - 3500;
        let y = i64::from(piece.square.rank) * 1000 - 3500;
        let mat = if piece.side == Side::Ivory { 1 } else { 2 };
        objects.push(SceneObject::new(
            identity_transform([x, y, if include_board { 50 } else { -250 }]),
            MeshHandle::from_raw(kind),
            MaterialHandle::from_raw(mat),
        ));
        if piece.kind == PieceKind::Knight {
            objects.push(SceneObject::new(
                identity_transform([x, y, if include_board { 50 } else { -250 }]),
                MeshHandle::from_raw(9),
                MaterialHandle::from_raw(4),
            ));
        }
    }
    let camera = SceneCamera {
        vertical_fov_millidegrees: 43_000,
        near_plane_millimeters: 100,
        far_plane_millimeters: 100_000,
    };
    let effects = SceneEffects::new(SceneEffectsParts {
        water: None,
        sky: SceneSky::ProceduralHemisphere {
            zenith: SceneColor {
                red: 205,
                green: 214,
                blue: 228,
            },
            horizon: SceneColor {
                red: 156,
                green: 163,
                blue: 170,
            },
        },
        sun: SceneSun {
            direction: SceneDirection::new([-400_000, -300_000, 866_025])?,
            irradiance_micro: 1_800_000,
        },
        ambient: SceneAmbient {
            sky_irradiance_micro: 500_000,
            ground_irradiance_micro: 300_000,
        },
        shadows: SceneShadowCascades::new(vec![48_000], 0).expect("valid single shadow extent"),
        ssao: None,
        camera,
        grading: SceneColorGrading {
            exposure_micro: 1_000_000,
            contrast_micro: 1_000_000,
            saturation_micro: 1_000_000,
        },
        post: Vec::new(),
        anti_alias: SceneAntiAlias::Disabled,
    })?;
    Ok(RenderScene::new(SceneParts {
        resources: SceneResources::new(textures.to_vec(), materials)?,
        geometry: SceneGeometry::new(meshes, objects)?,
        terrain: None,
        effects: Some(effects),
    })?)
}
fn identity_transform(position_millimeters: [i64; 3]) -> worldrunner::scene::plan::SceneTransform {
    worldrunner::scene::plan::SceneTransform {
        position_millimeters,
        rotation: worldrunner::scene::plan::SceneRotation::IDENTITY,
        scale_millionths: 1_000_000,
    }
}
fn scene_mesh(mesh: &Mesh) -> worldrunner::scene::plan::SceneMesh {
    use worldrunner::scene::plan::{SceneMesh, SceneMeshLod, SceneVertex};
    let vertices: Vec<_> = mesh
        .positions
        .iter()
        .zip(&mesh.normals)
        .zip(&mesh.uvs)
        .map(|((p, n), uv)| SceneVertex {
            position_millimeters: p.map(|v| f64::from(v).round() as i64),
            normal_micro: n.map(|v| (f64::from(v) * 1_000_000.).round() as i64),
            texcoord_micro: uv.map(|v| (f64::from(v) * 1_000_000.).round() as i64),
        })
        .collect();
    // Millimeter quantization can collapse submillimeter pole/clip edges to a line.
    // Such triangles cover no area; omit them rather than submitting undefined faces.
    let indices = mesh
        .indices
        .chunks_exact(3)
        .filter(|triangle| {
            let [a, b, c] = std::array::from_fn(|i| {
                glam::DVec3::from_array(
                    vertices[triangle[i] as usize]
                        .position_millimeters
                        .map(|v| v as f64),
                )
            });
            (b - a).cross(c - a).length_squared() > 0.
        })
        .flatten()
        .copied()
        .collect();
    SceneMesh::new(vec![
        SceneMeshLod::new(0, vertices, indices, None).expect("authored mesh indices are valid"),
    ])
    .expect("one mesh LOD is valid")
}
fn texture(width: u32, height: u32, bytes: Vec<u8>) -> worldrunner::scene::plan::SceneTexture {
    use worldrunner::scene::plan::{SceneTexture, TexelFormat, TextureColorSpace};
    SceneTexture::Texels {
        format: TexelFormat::Rgba8Unorm,
        color_space: TextureColorSpace::Srgb,
        width,
        height,
        mips: 1,
        bytes: bytes.into_boxed_slice(),
    }
}
