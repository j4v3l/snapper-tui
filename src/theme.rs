use ratatui::{
    layout::Alignment,
    style::{Color, Modifier, Style},
    widgets::{
        block::{BorderType, Padding},
        Block, Borders,
    },
};

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub muted: Color,
    pub accent: Color,
    pub warn: Color,
    pub error: Color,
    pub header_bg: Color,
    pub header_fg: Color,
    pub border: Color,
    pub highlight: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::Gray,
            muted: Color::DarkGray,
            accent: Color::Cyan,
            warn: Color::Yellow,
            error: Color::Red,
            header_bg: Color::Reset,
            header_fg: Color::White,
            border: Color::Gray,
            highlight: Color::Cyan,
        }
    }
}

impl Theme {
    pub fn block<'a>(&self, title: impl Into<String>) -> Block<'a> {
        Block::default()
            .title(title.into())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.border))
    }

    // Modal blocks: rounded borders, centered title, subtle padding
    pub fn modal_block<'a>(&self, title: impl Into<String>) -> Block<'a> {
        Block::bordered()
            .title(title.into())
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .padding(Padding::new(2, 2, 1, 1))
            .border_style(Style::default().fg(self.border))
    }

    pub fn modal_warn_block<'a>(&self, title: impl Into<String>) -> Block<'a> {
        self.modal_block(title)
            .border_style(Style::default().fg(self.warn).add_modifier(Modifier::BOLD))
    }

    pub fn modal_error_block<'a>(&self, title: impl Into<String>) -> Block<'a> {
        self.modal_block(title)
            .border_style(Style::default().fg(self.error).add_modifier(Modifier::BOLD))
    }

    // inner_block removed (unused)

    pub fn header_style(&self) -> Style { Style::default().fg(self.header_fg).add_modifier(Modifier::BOLD) }
    pub fn muted_style(&self) -> Style { Style::default().fg(self.muted) }
    pub fn highlight_style(&self) -> Style { Style::default().fg(self.highlight).add_modifier(Modifier::BOLD) }
    pub fn warn_style(&self) -> Style { Style::default().fg(self.warn) }
    pub fn error_style(&self) -> Style { Style::default().fg(self.error) }
}

pub static THEME: Theme = Theme {
    bg: Color::Reset,
    fg: Color::Gray,
    muted: Color::DarkGray,
    accent: Color::Cyan,
    warn: Color::Yellow,
    error: Color::Red,
    header_bg: Color::Reset,
    header_fg: Color::White,
    border: Color::Gray,
    highlight: Color::Cyan,
};
