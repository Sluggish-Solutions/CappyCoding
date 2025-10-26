use alloc::{borrow::ToOwned, format, vec::Vec};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use once_cell::sync::OnceCell;
use ratatui::{
    Frame,
    style::{Style, Stylize},
    widgets::{Block, Paragraph, Wrap},
};

use crate::wifi::{CommitData, PullRequestData, WorkflowData};

// pub struct CapyRenderInfo: OnceCell<Mutex<>>;
//
pub struct CapyState {
    pub commits: CommitData,
    pub pr: Vec<PullRequestData>,
    pub workflow: Vec<WorkflowData>,
}
// static
static STATE: OnceCell<Mutex<CriticalSectionRawMutex, Option<CapyState>>> = OnceCell::new();

// Initialize the singleton (call once at startup)
pub fn init_capy_state() {
    let state = None;
    STATE.get_or_init(|| Mutex::new(state));
}

// Get a reference to the singleton
pub fn get_capy_state() -> &'static Mutex<CriticalSectionRawMutex, Option<CapyState>> {
    STATE
        .get()
        .expect("State not initialized! Call init_state() first")
}

/// The root of the widget tree that draws everything else;
pub fn root_draw(
    frame: &mut Frame,

    config: embassy_sync::mutex::MutexGuard<
        '_,
        embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        Option<crate::CapyConfig>,
    >,
    state: embassy_sync::mutex::MutexGuard<
        '_,
        embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        Option<crate::ui::CapyState>,
    >,
) {
    if config.is_some() {
        if let Some(ref state) = *state {
            //MAIN loop
            let text = format!("Data Loaded: total commits: {}!", state.commits.total);
            let paragraph = Paragraph::new(text.dark_gray()).wrap(Wrap { trim: true });
            let bordered_block = Block::bordered()
                .border_style(Style::new().yellow())
                .title("Mousefood");
            frame.render_widget(paragraph.block(bordered_block), frame.area());
        } else {
            let text = "Loading Data".to_owned();
            let paragraph = Paragraph::new(text.dark_gray()).wrap(Wrap { trim: true });
            let bordered_block = Block::bordered()
                .border_style(Style::new().yellow())
                .title("Mousefood");
            frame.render_widget(paragraph.block(bordered_block), frame.area());
        }
    } else {
        let text = "Please configure me in the app!".to_owned();
        let paragraph = Paragraph::new(text.dark_gray()).wrap(Wrap { trim: true });
        let bordered_block = Block::bordered()
            .border_style(Style::new().yellow())
            .title("Mousefood");
        frame.render_widget(paragraph.block(bordered_block), frame.area());
    };
}
