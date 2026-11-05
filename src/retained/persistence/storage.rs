use super::{GamePayload, GameSettings, PAYLOAD_VERSION, ProgressError};
use crate::teaching::Progress;
use std::{io::ErrorKind, path::PathBuf};
use worldrunner::world::worldrunner_save::{
    ApplicationStore, SaveError, SlotId, SlotMetadata, StoreLimits,
};

pub(crate) const LIMITS: StoreLimits = StoreLimits {
    payload_bytes: 1_048_576,
    label_bytes: 256,
    settings_bytes: 4096,
    value_bytes: 256,
    settings_entries: 16,
};
pub(crate) struct Storage {
    pub(super) store: ApplicationStore,
    pub progress: Progress,
    pub progress_error: Option<ProgressError>,
    pub root: PathBuf,
    pub current: Option<GamePayload>,
    pub labels: [String; 3],
    pub finished: [Option<GamePayload>; 4],
    pub error: Option<String>,
}
impl Storage {
    pub fn open(root: PathBuf) -> Self {
        let mut storage = Self {
            store: ApplicationStore::new(root.clone(), LIMITS).expect("fixed positive limits"),
            progress: Progress::default(),
            progress_error: None,
            root,
            current: None,
            labels: std::array::from_fn(|_| "Empty".into()),
            finished: std::array::from_fn(|_| None),
            error: None,
        };
        match storage.read_progress() {
            Ok(progress) => storage.progress = progress,
            Err(error) => storage.progress_error = Some(error),
        }
        storage
    }
    pub fn read(&self, name: &str) -> Result<Option<GamePayload>, String> {
        let contents = match self
            .store
            .read_slot(&SlotId::new(name).expect("fixed slot"))
        {
            Ok(value) => value,
            Err(error) if missing(&error) => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        if contents.metadata.payload_version != PAYLOAD_VERSION {
            return Err(format!(
                "unsupported chess payload version {}",
                contents.metadata.payload_version
            ));
        }
        let payload: GamePayload =
            serde_json::from_slice(&contents.payload).map_err(|e| e.to_string())?;
        payload.replay()?;
        Ok(Some(payload))
    }
    pub fn write(&self, name: &str, payload: &GamePayload) -> Result<(), String> {
        // A damaged or future-version save must remain available for inspection.
        self.read(name)?;
        let bytes = serde_json::to_vec(payload).map_err(|e| e.to_string())?;
        self.store
            .write_slot(
                &SlotId::new(name).expect("fixed slot"),
                &bytes,
                &SlotMetadata {
                    label: payload.label(),
                    payload_version: PAYLOAD_VERSION,
                },
            )
            .map_err(|e| e.to_string())
    }
    pub fn refresh(&mut self) {
        self.refresh_finished();
        self.current = None;
        match self.read("current") {
            Ok(Some(payload)) => match payload.resumable() {
                Ok(true) => self.current = Some(payload),
                Ok(false) => (),
                Err(error) => self.error = Some(error),
            },
            Ok(None) => (),
            Err(error) => self.error = Some(error),
        }
        self.labels = std::array::from_fn(|_| "Empty".into());
        for (index, label) in self.labels.iter_mut().enumerate() {
            let id = SlotId::new(&format!("slot-{}", index + 1)).expect("manual slot");
            // Only game slots supply game labels; progress has its own error channel.
            match self.store.read_slot(&id) {
                Ok(contents) => *label = contents.metadata.label,
                Err(error) if missing(&error) => (),
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }
    fn refresh_finished(&mut self) {
        for (index, name) in ["current", "slot-1", "slot-2", "slot-3"]
            .into_iter()
            .enumerate()
        {
            self.finished[index] = None;
            match self.read(name) {
                Ok(Some(payload)) => match payload.resumable() {
                    Ok(false) => self.finished[index] = Some(payload),
                    Ok(true) => (),
                    Err(error) => self.error = Some(error),
                },
                Ok(None) => (),
                Err(error) => self.error = Some(error),
            }
        }
    }
    pub fn settings(&self) -> Result<GameSettings, String> {
        match self.store.read_settings() {
            Ok(values) => GameSettings::decode(&values),
            Err(error) if missing(&error) => Ok(GameSettings::default()),
            Err(error) => Err(error.to_string()),
        }
    }
    pub fn save_settings(&self, settings: GameSettings) -> Result<(), String> {
        self.settings()?;
        self.store
            .write_settings(&settings.encode())
            .map_err(|e| e.to_string())
    }
}
pub(super) fn missing(error: &SaveError) -> bool {
    matches!(error, SaveError::Io(error) if error.kind() == ErrorKind::NotFound)
}
