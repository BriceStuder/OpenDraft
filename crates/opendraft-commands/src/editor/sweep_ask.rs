//! Asking for an object, or a sweep across many: TRIM and EXTEND.

use super::{Answer, Cancelled, Editor, Hint, Options, Picked, Reply, Swept, Wants};

impl Editor {
    /// Ask for an object as [`ask_object`](Self::ask_object) does, but a
    /// click in empty space or a path dragged from there answers too (TRIM
    /// and EXTEND take a crossing window or a freehand path that way).
    pub fn ask_object_or_sweep(
        &self,
        options: impl Into<Options<Swept>>,
    ) -> Result<Answer<Swept>, Cancelled> {
        self.ask_value(
            Wants::Object,
            &options.into().sweep(),
            |_| Hint::Text(String::new()),
            |r| match r {
                Reply::Object { id, at, shift } => Some(Swept::Object(Picked { id, at, shift })),
                Reply::Missed { at, shift } => Some(Swept::Missed { at, shift }),
                Reply::Path { points, shift } => Some(Swept::Path { points, shift }),
                _ => None,
            },
        )
    }
}
