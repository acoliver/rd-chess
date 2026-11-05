use super::{Storage, storage::missing};
use crate::teaching::{Progress, TeachingError};
use thiserror::Error;
use worldrunner::world::worldrunner_save::{SaveError, SlotId, SlotMetadata};

const VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum ProgressError {
    #[error("training progress store: {0}")]
    Store(#[from] SaveError),
    #[error("unsupported training progress version {0}")]
    Version(u32),
    #[error("training progress UTF-8: {0}")]
    Encoding(#[from] std::str::Utf8Error),
    #[error("training progress payload: {0}")]
    Payload(#[from] TeachingError),
}

fn slot() -> SlotId {
    SlotId::new("training-progress").expect("fixed progress key")
}

impl Storage {
    pub(super) fn read_progress(&self) -> Result<Progress, ProgressError> {
        let contents = match self.store.read_slot(&slot()) {
            Ok(contents) => contents,
            Err(error) if missing(&error) => return Ok(Progress::default()),
            Err(error) => return Err(error.into()),
        };
        if contents.metadata.payload_version != VERSION {
            return Err(ProgressError::Version(contents.metadata.payload_version));
        }
        Ok(Progress::from_json(std::str::from_utf8(
            &contents.payload,
        )?)?)
    }

    pub fn save_progress(&mut self, progress: &Progress) -> Result<(), ProgressError> {
        // Preserve corrupt or future records, just as game slots do.
        self.read_progress()?;
        let payload = progress.to_json()?;
        self.store.write_slot(
            &slot(),
            payload.as_bytes(),
            &SlotMetadata {
                label: "Lesson and puzzle progress".into(),
                payload_version: VERSION,
            },
        )?;
        self.progress = progress.clone();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_progress_payloads_have_typed_errors_and_are_not_replaced() {
        let root = tempfile::tempdir().unwrap();
        let mut storage = Storage::open(root.path().to_path_buf());
        let cases = [
            (1, b"{broken".as_slice()),
            (1, b"\xff".as_slice()),
            (2, b"{}".as_slice()),
            (
                1,
                b"{\"version\":2,\"completed_lessons\":[],\"lessons\":{},\"bands\":{}}".as_slice(),
            ),
        ];
        for (index, (version, payload)) in cases.into_iter().enumerate() {
            storage
                .store
                .write_slot(
                    &slot(),
                    payload,
                    &SlotMetadata {
                        label: "test".into(),
                        payload_version: version,
                    },
                )
                .unwrap();
            let error = storage.read_progress().unwrap_err();
            assert!(match index {
                0 | 3 => matches!(error, ProgressError::Payload(_)),
                1 => matches!(error, ProgressError::Encoding(_)),
                2 => matches!(error, ProgressError::Version(2)),
                _ => unreachable!(),
            });
            let bytes = std::fs::read(root.path().join("slot-training-progress.rds")).unwrap();
            assert!(storage.save_progress(&Progress::default()).is_err());
            assert_eq!(
                std::fs::read(root.path().join("slot-training-progress.rds")).unwrap(),
                bytes
            );
        }
    }
}
