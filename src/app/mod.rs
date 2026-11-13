//! The dashboard: its state and the event loop that drives it.

mod event_loop;
mod state;

pub use event_loop::run;
pub use state::{App, SectionView};
