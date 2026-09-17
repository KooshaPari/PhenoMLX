//! Parser trait + dispatcher.

use crate::WorkPackage;

/// All four supported input formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// GSD ("Get Shit Done") format
    Gsd,
    /// OpenSpec format
    OpenSpec,
    /// BMAD-Method format
    Bmad,
    /// Spec-Kitty format
    Kitty,
}

impl Format {
    /// Stringify for the `source_format` field on `WorkPackage`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Format::Gsd => "gsd",
            Format::OpenSpec => "openspec",
            Format::Bmad => "bmad",
            Format::Kitty => "kitty",
        }
    }
}

pub mod gsd;
pub mod openspec;
pub mod bmad;
pub mod kitty;

/// A single parser, one per format.
pub trait Parser {
    fn parse(&self, text: &str) -> Result<Vec<WorkPackage>, String>;
}

/// Dispatch helper: pick the right parser for `format` and call it.
pub fn parse_for(text: &str, format: Format) -> Result<Vec<WorkPackage>, String> {
    match format {
        Format::Gsd => gsd::GsdParser.parse(text),
        Format::OpenSpec => openspec::OpenSpecParser.parse(text),
        Format::Bmad => bmad::BmadParser.parse(text),
        Format::Kitty => kitty::KittyParser.parse(text),
    }
}
