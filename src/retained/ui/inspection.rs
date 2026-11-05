use super::{GameUi, Screen, menus};
use worldrunner::{simulation::ObjectHandle, ui::UiLayout};

/// Resolved title geometry in top-left physical pixels, shared with window tooling.
pub fn title_layout(viewport: [u32; 2]) -> Result<serde_json::Value, String> {
    resolve(viewport).map_err(|error| error.to_string())
}

fn resolve(viewport: [u32; 2]) -> Result<serde_json::Value, worldrunner::simulation::WorldError> {
    let mut ui = GameUi::new()?;
    let panel = ui
        .pages
        .iter()
        .find(|page| page.0 == Screen::Title)
        .unwrap()
        .1;
    ui.native.world.set_active(panel, true)?;
    let layout = ui
        .native
        .world
        .ui_layout_with(viewport.into(), &ui.native.router.layout)?;
    let status = ui.native.world.component(ui.status)?.object;
    let entries: serde_json::Map<_, _> = menus::TITLE
        .iter()
        .map(|&(label, action)| {
            let (&list, actions) = ui
                .bindings
                .iter()
                .find(|(_, actions)| actions.contains(&action))
                .expect("title action");
            let index = actions.iter().position(|&item| item == action).unwrap();
            (
                label.to_owned(),
                serde_json::json!(bounds(&layout, ui.rows[&list][index])),
            )
        })
        .collect();
    Ok(serde_json::json!({
        "panel": bounds(&layout, panel),
        "status": bounds(&layout, status),
        "entries": entries,
    }))
}

fn bounds(layout: &UiLayout, object: ObjectHandle) -> [f64; 4] {
    let corners = layout
        .rect(object)
        .expect("visible UI rectangle")
        .screen_corners
        .expect("screen-space UI");
    let height = f64::from(layout.viewport.y);
    corners.iter().fold(
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
        |[left, top, right, bottom], point| {
            let x = f64::from(point.x);
            let y = height - f64::from(point.y);
            [left.min(x), top.min(y), right.max(x), bottom.max(y)]
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspection_uses_authored_status_and_title_rows_at_each_viewport() {
        for viewport in [[960, 720], [1200, 900]] {
            let value = title_layout(viewport).unwrap();
            let entries = value["entries"].as_object().unwrap();
            assert_eq!(entries.len(), menus::TITLE.len());
            let mut previous_bottom = f64::NEG_INFINITY;
            for &(label, _) in menus::TITLE {
                let rect: [f64; 4] = serde_json::from_value(entries[label].clone()).unwrap();
                assert!(rect[1] >= previous_bottom);
                assert!(rect[3] > rect[1]);
                assert!((rect[0] + rect[2] - f64::from(viewport[0])).abs() < f64::EPSILON);
                previous_bottom = rect[3];
            }
            assert_eq!(value["status"][1], 12.);
            assert_eq!(value["status"][3], 102.);
        }
    }
}
