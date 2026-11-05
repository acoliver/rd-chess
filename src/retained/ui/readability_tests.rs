use super::*;

fn study_state(next: bool) -> StudyState {
    StudyState {
        lessons: Vec::new(),
        bands: std::array::from_fn(|_| String::new()),
        panel: String::new(),
        controls: [true, next, true, true, true],
        why: false,
    }
}
fn study_panel_color(ui: &mut GameUi) -> [f32; 4] {
    ui.synchronize_study(Some(study_state(true)), Screen::Lesson)
        .unwrap();
    ui.synchronize(
        UiState {
            screen: Screen::Lesson,
            promotion: false,
            claim: false,
            undo: false,
            continuation: None,
            settings: super::super::persistence::GameSettings::default(),
            labels: std::array::from_fn(|_| "Empty".into()),
            setup: (0, 1),
            pgn_entries: Vec::new(),
        },
        [800, 600],
    )
    .unwrap();
    ui.overlay("", [800, 600]).unwrap();
    let plan = ui.plan.as_ref().unwrap();
    assert_eq!(plan.graphics.len(), 1);
    plan.graphics[0].graphic.quads[0].color
}

fn panel_color(ui: &GameUi, screen: Screen) -> [f32; 4] {
    let object = ui.pages.iter().find(|(page, _)| *page == screen).unwrap().1;
    ui.native
        .world
        .published_components()
        .find_map(|component| {
            if component.object != object {
                return None;
            }
            let ComponentDesc::CanvasRenderer(renderer) = &component.desc else {
                return None;
            };
            Some(renderer.geometry.as_ref().unwrap().vertices()[0].color)
        })
        .unwrap()
}
fn control_style(ui: &GameUi, action: MenuAction) -> worldrunner::ui::UiStyle {
    let (&list, actions) = ui
        .bindings
        .iter()
        .find(|(_, actions)| actions.contains(&action))
        .unwrap();
    let index = actions.iter().position(|&item| item == action).unwrap();
    let object = ui.rows[&list][index];
    let ComponentDesc::Text(run) = &ui
        .native
        .world
        .component(ui.native.text_handle(object))
        .unwrap()
        .desc
    else {
        panic!("control text")
    };
    let mut style = ui.native.router.styles[&object];
    for color in [
        &mut style.normal,
        &mut style.focused,
        &mut style.selected,
        &mut style.hovered,
        &mut style.pressed,
        &mut style.disabled,
    ] {
        *color = std::array::from_fn(|index| color[index] * run.color[index]);
    }
    style
}
fn luminance(color: [f32; 4]) -> f64 {
    // Native UI tints are display-linear; the sRGB attachment encodes them once.
    [0.2126, 0.7152, 0.0722]
        .into_iter()
        .zip(color)
        .map(|(weight, channel)| weight * f64::from(channel))
        .sum()
}
fn contrast(text: [f32; 4], panel: [f32; 4]) -> f64 {
    // White behind a translucent dark panel is the worst-case board colour.
    let background = panel.map(|channel| channel * panel[3] + 1. - panel[3]);
    let foreground =
        std::array::from_fn(|index| text[index] * text[3] + background[index] * (1. - text[3]));
    let text_luminance = luminance(foreground);
    let panel_luminance = luminance(background);
    (text_luminance.max(panel_luminance) + 0.05) / (text_luminance.min(panel_luminance) + 0.05)
}
#[test]
fn browsers_share_an_opaque_panel_background() {
    let ui = GameUi::new().unwrap();
    let lessons = panel_color(&ui, Screen::Lessons);
    let puzzles = panel_color(&ui, Screen::Puzzles);
    assert_eq!(lessons.map(f32::to_bits), puzzles.map(f32::to_bits));
    assert!((lessons[3] - widgets::PANEL_ALPHA).abs() < f32::EPSILON);
    assert!((lessons[3] - 1.).abs() < f32::EPSILON);
}
#[test]
fn locked_lesson_contrast_meets_aa() {
    let ui = GameUi::new().unwrap();
    let style = control_style(&ui, MenuAction::Lesson(1));
    let ratio = contrast(style.disabled, panel_color(&ui, Screen::Lessons));
    println!("locked lesson contrast: {ratio:.4}:1");
    assert!(ratio >= 4.5, "locked lesson contrast {ratio}:1");
    assert_ne!(
        style.normal.map(f32::to_bits),
        style.disabled.map(f32::to_bits)
    );
}
#[test]
fn next_control_contrast_meets_enabled_and_disabled_thresholds() {
    let mut ui = GameUi::new().unwrap();
    let panel = study_panel_color(&mut ui);
    let style = control_style(&ui, MenuAction::StudyNext);
    for color in [
        style.normal,
        style.focused,
        style.selected,
        style.hovered,
        style.pressed,
    ] {
        let ratio = contrast(color, panel);
        println!("enabled Next contrast: {ratio:.4}:1");
        assert!(ratio >= 4.5, "enabled Next contrast {ratio}:1");
        assert_ne!(color.map(f32::to_bits), style.disabled.map(f32::to_bits));
    }
    let ratio = contrast(style.disabled, panel);
    println!("disabled Next contrast: {ratio:.4}:1");
    assert!(ratio >= 3., "disabled Next contrast {ratio}:1");
}
#[test]
fn next_control_labels_distinguish_enabled_and_disabled() {
    let mut ui = GameUi::new().unwrap();
    let (&list, actions) = ui
        .bindings
        .iter()
        .find(|(_, actions)| actions.contains(&MenuAction::StudyNext))
        .unwrap();
    let index = actions
        .iter()
        .position(|&action| action == MenuAction::StudyNext)
        .unwrap();
    let object = ui.rows[&list][index];
    for enabled in [false, true] {
        ui.synchronize_study(Some(study_state(enabled)), Screen::Lesson)
            .unwrap();
        let ComponentDesc::Text(run) = &ui
            .native
            .world
            .component(ui.native.text_handle(object))
            .unwrap()
            .desc
        else {
            panic!("Next text")
        };
        assert_eq!(run.text, if enabled { "Next" } else { "Next [off]" });
        assert_eq!(ui.native.router.controls[&object].enabled, enabled);
    }
}
