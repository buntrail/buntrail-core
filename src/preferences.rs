use crate::contracts::TerminalStyle;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Appearance {
    System,
    Light,
    Dark,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TerminalScheme {
    Automatic,
    Graphite,
    Paper,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Preferences {
    pub appearance: Appearance,
    pub terminal_scheme: TerminalScheme,
    pub show_icons: bool,
    pub terminal: TerminalStyle,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            appearance: Appearance::System,
            terminal_scheme: TerminalScheme::Automatic,
            show_icons: true,
            terminal: TerminalStyle {
                font_family: "Menlo".into(),
                font_size: 12.0,
                ansi_palette: [
                    0x000000, 0xcd0000, 0x00cd00, 0xcdcd00, 0x0000ee, 0xcd00cd, 0x00cdcd, 0xe5e5e5,
                    0x7f7f7f, 0xff0000, 0x00ff00, 0xffff00, 0x5c5cff, 0xff00ff, 0x00ffff, 0xffffff,
                ],
                foreground: 0xe5e5e5,
                background: 0x000000,
                cursor: 0xffffff,
            },
        }
    }
}
impl Preferences {
    pub fn valid(&self) -> bool {
        !self.terminal.font_family.is_empty()
            && self.terminal.font_family.len() <= 256
            && self.terminal.font_size.is_finite()
            && (8.0..=48.0).contains(&self.terminal.font_size)
            && self
                .terminal
                .ansi_palette
                .iter()
                .chain([
                    &self.terminal.foreground,
                    &self.terminal.background,
                    &self.terminal.cursor,
                ])
                .all(|color| *color <= 0xffffff)
    }
    pub fn apply(&mut self, overrides: &PreferencesOverride) {
        if let Some(v) = overrides.appearance {
            self.appearance = v;
        }
        if let Some(v) = &overrides.terminal_scheme {
            self.terminal_scheme = v.clone();
        }
        if let Some(v) = &overrides.terminal {
            self.terminal = v.clone();
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PreferencesOverride {
    pub terminal_scheme: Option<TerminalScheme>,
    pub appearance: Option<Appearance>,
    pub terminal: Option<TerminalStyle>,
}
