use alloc::{borrow::ToOwned, fmt::format};
use ratatui::{
    Frame,
    style::{Style, Stylize},
    widgets::{Block, Paragraph, Wrap},
};

/// The root of the widget tree that draws everything else;
pub fn root_draw(
    frame: &mut Frame,

    config: embassy_sync::mutex::MutexGuard<
        '_,
        embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        Option<crate::CapyConfig>,
    >,
) {
    let text = if let Some(ref config) = *config {
        format(format_args!(
            "github_token: {:#?}",
            config.api_tokens.github
        ))

        // format_args!("wifi_creds{:#?}")config.wifi_credentials
        // "Please connect to me!"
    } else {
        "CONFIG present!".to_owned()
    };

    let paragraph = Paragraph::new(text.dark_gray()).wrap(Wrap { trim: true });
    let bordered_block = Block::bordered()
        .border_style(Style::new().yellow())
        .title("Mousefood");
    frame.render_widget(paragraph.block(bordered_block), frame.area());
}
