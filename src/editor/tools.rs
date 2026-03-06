#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Rectangle,
    Arrow,
    Freehand,
    Text,
    Highlight,
}

#[derive(Debug, Clone, Copy)]
pub struct Tool {
    pub kind: ToolKind,
}

impl Tool {
    pub fn new(kind: ToolKind) -> Self {
        Self { kind }
    }
}

impl Default for Tool {
    fn default() -> Self {
        Self {
            kind: ToolKind::Rectangle,
        }
    }
}
