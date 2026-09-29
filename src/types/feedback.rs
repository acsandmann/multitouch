use std::fmt;

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackPattern {
    Firm = 1,
    FirmStrong = 2,
    Medium = 3,
    MediumStrong = 4,
    Light = 5,
    LightStrong = 6,
    Click = 15,
    SecondaryClick = 16,
}

impl FeedbackPattern {
    pub const ALL: [Self; 8] = [
        Self::Firm,
        Self::FirmStrong,
        Self::Medium,
        Self::MediumStrong,
        Self::Light,
        Self::LightStrong,
        Self::Click,
        Self::SecondaryClick,
    ];
}

impl fmt::Display for FeedbackPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Firm => "firm",
            Self::FirmStrong => "firm, strong",
            Self::Medium => "medium",
            Self::MediumStrong => "medium, strong",
            Self::Light => "light",
            Self::LightStrong => "light, strong",
            Self::Click => "click",
            Self::SecondaryClick => "secondary click",
        })
    }
}
