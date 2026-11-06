use super::*;
use crate::retained::capture_frame;
use worldrunner::scene::viewer::RetainedHost;
fn srgb_byte(linear: f32) -> u8 {
    let encoded = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1. / 2.4) - 0.055
    };
    (encoded * 255.).round() as u8
}

#[test]
#[ignore = "requires the final windowless Metal suite"]
fn link_style_colour_is_drawn_inside_its_expected_underline_rect() {
    let mut driver = driver();
    study(&mut driver, true);
    let (object, id, _) = link(&mut driver, "fork");
    if driver.ui.as_ref().unwrap().native.router.focused_link() == Some((object, id)) {
        key(&mut driver, "tab");
        driver.ui.as_mut().unwrap().overlay("", SIZE).unwrap();
    }
    let ui = driver.ui.as_ref().unwrap();
    let underline = ui
        .plan
        .as_ref()
        .unwrap()
        .graphics
        .iter()
        .find(|graphic| graphic.graphic.object == object && graphic.link == Some(id))
        .expect("engine link underline")
        .graphic
        .quads[0];
    let min = underline.corners[0];
    let max = underline.corners[2];
    let color = ui.native.router.link_styles[&(object, id)].normal;
    assert_eq!(underline.color.map(f32::to_bits), color.map(f32::to_bits));
    assert!(max[0] > min[0] + 10. && max[1] >= min[1] + 1.);
    let options = viewer_options(
        driver.board().unwrap().scene(),
        CameraPreset::Overhead,
        SIZE,
    );
    let host = RetainedHost::new(driver.board().unwrap().scene().clone(), options).unwrap();
    let queue = driver.queue();
    let inbox = driver.captures();
    let mut run = host.windowless(&mut driver).expect("GPU required");
    let image = capture_frame(&mut run, &queue, &inbox).unwrap();
    let expected = [
        srgb_byte(color[0]),
        srgb_byte(color[1]),
        srgb_byte(color[2]),
        255,
    ];
    let y = (min[1] + 0.5).floor() as u32;
    let mut samples = 0;
    for x in (min[0] + 0.5).ceil() as u32..(max[0] - 0.5).floor() as u32 {
        let at = (((SIZE[1] - 1 - y) * SIZE[0] + x) * 4) as usize;
        assert_eq!(&image.pixels[at..at + 4], &expected, "link pixel {x},{y}");
        samples += 1;
    }
    assert!(samples > 10);
    run.shutdown().unwrap();
}
