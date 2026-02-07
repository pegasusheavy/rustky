use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LineStyle {
    pub fg_color: Option<String>,
    pub bg_color: Option<String>,
    pub font_size: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyledLine {
    pub text: String,
    pub style: LineStyle,
}

impl StyledLine {
    pub fn plain(text: String) -> Self {
        Self {
            text,
            style: LineStyle::default(),
        }
    }

    pub fn styled(text: String, style: LineStyle) -> Self {
        Self { text, style }
    }
}

impl From<String> for StyledLine {
    fn from(text: String) -> Self {
        Self::plain(text)
    }
}
