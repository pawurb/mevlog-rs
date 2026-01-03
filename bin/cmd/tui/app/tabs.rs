//! Tab management - switching and cycling

use super::{App, Tab};

impl App {
    pub(crate) fn switch_to_tab(&mut self, tab: Tab) {
        self.active_tab = tab;
    }

    pub(crate) fn cycle_tab(&mut self) {
        self.active_tab = match self.active_tab {
            Tab::Explore => Tab::Search,
            Tab::Search => Tab::Explore,
        };
    }
}
