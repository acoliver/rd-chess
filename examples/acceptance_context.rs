//! Acceptance-only adapter receipt and projected board centers, using the real facade.
use chess3d::{Position, Square, board_mesh, render_scene};
use glam::Vec3;
use serde_json::json;
use worldrunner::scene::gpu;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = pollster::block_on(gpu::device::GpuContext::request_with_features(
        gpu::wgpu::Features::empty(),
    ))?;
    let info = context.adapter().get_info();
    let instance = gpu::wgpu::Instance::new(gpu::wgpu::InstanceDescriptor {
        backends: gpu::wgpu::Backends::PRIMARY,
        ..gpu::wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let available = pollster::block_on(instance.enumerate_adapters(gpu::wgpu::Backends::PRIMARY));
    let available_names: Vec<_> = available.iter().map(|a| a.get_info().name).collect();
    if available_names.len() != 1 || available_names[0] != info.name {
        return Err(format!(
            "acceptance requires one unambiguous adapter; found {available_names:?}"
        )
        .into());
    }
    let centers = overhead_centers()?;
    println!(
        "{}",
        json!({
            "adapter": { "name": info.name, "vendor": info.vendor, "device": info.device,
                "device_type": format!("{:?}", info.device_type),
                "backend": format!("{:?}", info.backend),
                "driver": info.driver, "driver_info": info.driver_info },
            "available_adapter_names": available_names,
            "overhead_centers": centers,
            "provenance": "GpuContext::request_with_features(empty), same acquisition API as CLI; separate probe process, not CLI instrumentation; exactly one PRIMARY adapter required"
        })
    );
    Ok(())
}

fn overhead_centers() -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    let scene = render_scene(&Position::initial(342))?;
    let view = gpu::SceneView::new(
        chess3d::CameraPreset::Overhead.pose(),
        *scene.effects().camera(),
        gpu::Viewport {
            width: 1200,
            height: 1200,
        },
    )?;
    let mesh = board_mesh();
    let mut centers = Vec::new();
    for rank in 2..6u8 {
        for file in 0..8u8 {
            let offset = usize::from(rank * 8 + file) * 4;
            let center = mesh.positions[offset..offset + 4]
                .iter()
                .copied()
                .map(Vec3::from)
                .sum::<Vec3>()
                / 4000.0;
            let clip = view.view_projection_matrix() * center.extend(1.0);
            let ndc = clip.truncate() / clip.w;
            centers.push(json!({
                "file": file, "rank": rank,
                "dark": Square::new(file, rank)?.is_dark(),
                "pixel": [(ndc.x + 1.0) * 600.0, (1.0 - ndc.y) * 600.0],
            }));
        }
    }
    Ok(centers)
}
