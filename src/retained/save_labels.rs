use super::{BoardDriver, Screen, persistence::GamePayload};
use worldrunner::ui::{TextReject, UiAction};

// Each 286px slot cell fits 24 Space Mono glyphs at 18px (259.2px).
pub(super) const LABEL_LIMIT: usize = 24;
pub(super) struct LabelEdit {
    slot: u8,
    payload: GamePayload,
}

pub(super) fn validate_label(text: &str) -> Result<String, String> {
    let label = text.trim();
    if label.is_empty() {
        return Err("Save label cannot be empty".into());
    }
    if label.chars().count() > LABEL_LIMIT {
        return Err(format!(
            "Save label must be {LABEL_LIMIT} characters or fewer"
        ));
    }
    if !label.bytes().all(|c| c.is_ascii_graphic() || c == b' ') {
        return Err("Save label supports printable ASCII characters only".into());
    }
    Ok(label.into())
}

impl BoardDriver {
    pub(super) fn open_save_label(&mut self, slot: u8, rename: bool) -> Result<(), String> {
        let payload = if rename {
            self.saved_payload(slot)
        } else {
            self.save_payload()
        };
        match payload {
            Ok(payload) => {
                self.ui
                    .as_mut()
                    .expect("configured UI")
                    .edit_label(&payload.label())?;
                self.label_edit = Some(LabelEdit { slot, payload });
                self.transition(Screen::SaveLabel)?;
            }
            Err(error) => self.pgn.message = Some(format!("Save error: {error}")),
        }
        Ok(())
    }
    pub(super) fn label_action(&mut self, action: UiAction) -> Result<bool, String> {
        if self.screen() != Screen::SaveLabel {
            return Ok(false);
        }
        match action {
            UiAction::TextSubmitted { .. } => self.commit_label()?,
            UiAction::TextCancelled { .. } => {
                self.label_edit = None;
                self.transition(Screen::Slots)?;
            }
            UiAction::TextRejected { reason, .. } => {
                self.pgn.message = Some(match reason {
                    TextReject::MaxLength => {
                        format!("Save label must be {LABEL_LIMIT} characters or fewer")
                    }
                    TextReject::Clipboard(error) => format!("Label clipboard error: {error:?}"),
                    _ => "Save label supports printable ASCII characters only".into(),
                });
            }
            UiAction::TextChanged { .. } => self.pgn.message = None,
            _ => return Ok(false),
        }
        Ok(true)
    }
    fn commit_label(&mut self) -> Result<(), String> {
        let result = self.write_label();
        if let Err(error) = result {
            self.pgn.message = Some(error);
        } else {
            self.label_edit = None;
            self.transition(Screen::Slots)?;
        }
        Ok(())
    }
    fn write_label(&mut self) -> Result<(), String> {
        let label = validate_label(self.ui.as_ref().expect("configured UI").label_value())?;
        let edit = self.label_edit.as_ref().expect("active label edit");
        let mut payload = edit.payload.clone();
        payload.label = Some(label);
        let storage = self.storage.as_mut().ok_or("no application store")?;
        storage.write(&format!("slot-{}", edit.slot), &payload)?;
        storage.refresh();
        Ok(())
    }
}
