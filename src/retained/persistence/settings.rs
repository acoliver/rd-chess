use worldrunner::world::worldrunner_save::{SettingValue, Settings};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GameSettings {
    pub level: u8,
    pub side: usize,
    pub markers: bool,
    pub overhead: bool,
}
impl Default for GameSettings {
    fn default() -> Self {
        Self {
            level: 1,
            side: 0,
            markers: true,
            overhead: false,
        }
    }
}
impl GameSettings {
    pub(crate) fn validate(self) -> Result<(), String> {
        if !(1..=8).contains(&self.level) || self.side > 2 {
            return Err("invalid default computer level or side".into());
        }
        Ok(())
    }
    pub(super) fn encode(self) -> Settings {
        Settings::from([
            ("version".into(), SettingValue::Integer(1)),
            ("level".into(), SettingValue::Integer(i64::from(self.level))),
            (
                "side".into(),
                SettingValue::Integer(i64::try_from(self.side).expect("three sides")),
            ),
            ("markers".into(), SettingValue::Bool(self.markers)),
            (
                "camera".into(),
                SettingValue::String(if self.overhead { "overhead" } else { "hero" }.into()),
            ),
        ])
    }
    pub(super) fn decode(values: &Settings) -> Result<Self, String> {
        let invalid = || "invalid chess settings".to_owned();
        let Some(SettingValue::Integer(1)) = values.get("version") else {
            return Err(invalid());
        };
        let Some(SettingValue::Integer(level)) = values.get("level") else {
            return Err(invalid());
        };
        let Some(SettingValue::Integer(side)) = values.get("side") else {
            return Err(invalid());
        };
        let Some(SettingValue::Bool(markers)) = values.get("markers") else {
            return Err(invalid());
        };
        let Some(SettingValue::String(camera)) = values.get("camera") else {
            return Err(invalid());
        };
        if values.len() != 5 || !["hero", "overhead"].contains(&camera.as_str()) {
            return Err(invalid());
        }
        let settings = Self {
            level: u8::try_from(*level).map_err(|_| invalid())?,
            side: usize::try_from(*side).map_err(|_| invalid())?,
            markers: *markers,
            overhead: camera == "overhead",
        };
        settings.validate()?;
        Ok(settings)
    }
}
