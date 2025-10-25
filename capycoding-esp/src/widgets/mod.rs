use ratatui::{
    Frame,
    style::{Style, Stylize},
    widgets::{Block, Paragraph, Wrap},
};

pub fn root_draw(frame: &mut Frame) {
    let text = "Ratatui on embedded devices! ";
    let paragraph = Paragraph::new(text.dark_gray()).wrap(Wrap { trim: true });
    let bordered_block = Block::bordered()
        .border_style(Style::new().yellow())
        .title("Mousefood");
    frame.render_widget(paragraph.block(bordered_block), frame.area());
}
