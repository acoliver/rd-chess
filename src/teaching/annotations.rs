use super::course::{Result, require};
use serde::Deserialize;

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnnotationPalette {
    pub suggestion: [f32; 4],
    pub threat: [f32; 4],
}
impl AnnotationPalette {
    pub fn bundled() -> Result<Self> {
        let palette: Self = serde_json::from_str(include_str!("../../assets/annotations.json"))?;
        validate_colour(palette.suggestion)?;
        validate_colour(palette.threat)?;
        require(
            palette
                .suggestion
                .iter()
                .zip(palette.threat)
                .any(|(a, b)| (a - b).abs() > 0.1),
            "annotation colours must differ",
        )?;
        Ok(palette)
    }
}
pub(super) fn authored_colour() -> [f32; 4] {
    [0., 0.7, 0.8, 1.]
}
pub(super) fn validate_colour(colour: [f32; 4]) -> Result<()> {
    require(
        colour
            .iter()
            .all(|c| c.is_finite() && (0.0..=1.0).contains(c)),
        "annotation colour must be finite RGBA in 0..1",
    )
}
