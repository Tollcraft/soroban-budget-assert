/// Colour policy for the plain-text `--check` output.
#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorChoice {
    /// Colour only when stdout is a terminal and `NO_COLOR` is unset or
    /// empty (the no-color.org convention).
    #[default]
    Auto,
    /// Always emit colour, even into pipes and files.
    Always,
    /// Never emit colour.
    Never,
}
