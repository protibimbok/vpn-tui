//! Sort order and table column density.

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::ui::servers) enum SortMode {
    Title,
    Load,
    Latency,
}

impl SortMode {
    pub(in crate::ui::servers) fn label(self) -> &'static str {
        match self {
            SortMode::Title => "title",
            SortMode::Load => "load",
            SortMode::Latency => "latency",
        }
    }

    pub(in crate::ui::servers) fn next(self) -> Self {
        match self {
            SortMode::Title => SortMode::Load,
            SortMode::Load => SortMode::Latency,
            SortMode::Latency => SortMode::Title,
        }
    }
}

/// Column layout chosen from terminal width.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::ui::servers) enum Density {
    Compact,
    Comfortable,
    Wide,
}

impl Density {
    pub(in crate::ui::servers) fn from_width(width: u16) -> Self {
        if width < 72 {
            Density::Compact
        } else if width < 110 {
            Density::Comfortable
        } else {
            Density::Wide
        }
    }
}
