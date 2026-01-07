//! Tab management - switching and cycling

use super::{App, PrimaryTab};

impl App {
    pub(crate) fn switch_to_tab(&mut self, tab: PrimaryTab) {
        self.active_tab = tab;
    }

    pub(crate) fn cycle_tab(&mut self) {
        self.active_tab = match self.active_tab {
            PrimaryTab::Explore => PrimaryTab::Search,
            PrimaryTab::Search => PrimaryTab::Explore,
        };
    }
}
