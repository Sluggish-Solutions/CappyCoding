use alloc::{borrow::ToOwned, format, string::ToString, vec::Vec};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use once_cell::sync::OnceCell;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::Text,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::wifi::{CommitData, PullRequestData, WorkflowData};

// pub struct CapyRenderInfo: OnceCell<Mutex<>>;
//
pub struct CapyState {
    pub commits: CommitData,
    pub pr: Vec<PullRequestData>,
    pub workflow: Vec<WorkflowData>,
    pub carousel_index: u32,
    pub used_tokens: u32,
    pub max_tokens: u32,
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
            let layout =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(frame.area());

            let views = [
                ("prs", "workflows"),
                ("workflows", "commits"),
                ("commits", "tokens"),
                ("tokens", "prs"),
            ];

            let current_view = views[(state.carousel_index % 4) as usize];

            match current_view {
                ("prs", "workflows") => {
                    draw_gh_prs(frame, layout[0], &state.pr);
                    draw_gh_workflows(frame, layout[1], &state.workflow);
                }
                ("workflows", "commits") => {
                    draw_gh_workflows(frame, layout[0], &state.workflow);
                    draw_commit_stats(frame, layout[1], &state.commits);
                }
                ("commits", "tokens") => {
                    draw_commit_stats(frame, layout[0], &state.commits);
                    draw_token_stats(frame, layout[1], state.used_tokens, state.max_tokens);
                }
                ("tokens", "prs") => {
                    draw_token_stats(frame, layout[0], state.used_tokens, state.max_tokens);
                    draw_gh_prs(frame, layout[1], &state.pr);
                }
                _ => {}
            }
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

// fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()>;

// fn draw_gh_prs(frame: &mut Frame, area: Rect, prs: &Vec<PullRequestData>) {
//     let layout =
//         Layout::vertical(&[Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);

//     let open_pr_rect = layout[1];
//     let closed_pr_rect = layout[0];

//     let open_prs: Vec<_> = prs.iter().filter(|e| e.state == "open").collect();
//     let open_prs: Vec<_> = prs.iter().filter(|e| e.state == "merged").collect();

//     frame.render_widget(widget, area);

// }

fn draw_gh_prs(frame: &mut Frame, area: Rect, prs: &Vec<PullRequestData>) {
    let layout =
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);

    let top_rect = layout[0];
    let bottom_rect = layout[1];

    // Filter PRs by state
    let open_prs: Vec<_> = prs.iter().filter(|pr| pr.state == "open").collect();
    let closed_prs: Vec<_> = prs
        .iter()
        .filter(|pr| pr.state == "closed" || pr.state == "merged")
        .collect();

    // Create widgets with borders
    let open_block = Block::default()
        .title("Open PRs")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let closed_block = Block::default()
        .title("Closed/Merged PRs")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    // Create list items
    let open_items: Vec<ListItem> = open_prs
        .iter()
        .map(|pr| ListItem::new(pr.title.as_str()))
        .collect();

    let closed_items: Vec<ListItem> = closed_prs
        .iter()
        .map(|pr| ListItem::new(pr.title.as_str()))
        .collect();

    // Create lists
    let open_list = List::new(open_items).block(open_block);
    let closed_list = List::new(closed_items).block(closed_block);

    // Render widgets
    frame.render_widget(closed_list, top_rect);
    frame.render_widget(open_list, bottom_rect);
}

fn draw_gh_workflows(frame: &mut Frame, area: Rect, workflows: &Vec<WorkflowData>) {
    let layout =
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);

    let top_rect = layout[0];
    let bottom_rect = layout[1];

    // Filter workflows by conclusion
    let passing_workflows: Vec<_> = workflows
        .iter()
        .filter(|wf| wf.conclusion == "success")
        .collect();

    let failing_workflows: Vec<_> = workflows
        .iter()
        .filter(|wf| {
            wf.conclusion == "failure"
                || wf.conclusion == "cancelled"
                || wf.conclusion == "timed_out"
        })
        .collect();

    // Create widgets with borders
    let passing_block = Block::default()
        .title("Passing Workflows")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let failing_block = Block::default()
        .title("Failing Workflows")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    // Create list items
    let passing_items: Vec<ListItem> = passing_workflows
        .iter()
        .map(|wf| ListItem::new(format!("{} ({})", wf.name, wf.status)))
        .collect();

    let failing_items: Vec<ListItem> = failing_workflows
        .iter()
        .map(|wf| ListItem::new(format!("{} - {} ({})", wf.name, wf.conclusion, wf.status)))
        .collect();

    // Create lists
    let passing_list = List::new(passing_items).block(passing_block);
    let failing_list = List::new(failing_items).block(failing_block);

    // Render widgets
    frame.render_widget(failing_list, top_rect);
    frame.render_widget(passing_list, bottom_rect);
}

fn draw_commit_stats(frame: &mut Frame, area: Rect, commits: &CommitData) {
    let layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
    ])
    .split(area);

    // Total commits block
    let total_block = Block::default()
        .title("Total Commits")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let total_text = Paragraph::new(commits.total.to_string())
        .block(total_block)
        .alignment(Alignment::Center);

    // Last week block
    let week_block = Block::default()
        .title("Last Week")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let week_text = Paragraph::new(commits.last_week.to_string())
        .block(week_block)
        .alignment(Alignment::Center);

    // Last month block
    let month_block = Block::default()
        .title("Last Month")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let month_text = Paragraph::new(commits.last_month.to_string())
        .block(month_block)
        .alignment(Alignment::Center);

    // Render widgets
    frame.render_widget(total_text, layout[0]);
    frame.render_widget(week_text, layout[1]);
    frame.render_widget(month_text, layout[2]);
}

fn draw_token_stats(frame: &mut Frame, area: Rect, used_tokens: u32, max_tokens: u32) {
    let layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
    ])
    .split(area);

    let percentage = if max_tokens > 0 {
        ((used_tokens as f64 / max_tokens as f64) * 100.0) as u16
    } else {
        0
    };

    let remaining = max_tokens.saturating_sub(used_tokens);

    // Determine color based on usage
    let usage_color = match percentage {
        0..=50 => Color::Green,
        51..=75 => Color::Yellow,
        76..=90 => Color::LightRed,
        _ => Color::Red,
    };

    // Used tokens block
    let used_block = Block::default()
        .title("Used Tokens")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(usage_color));
    let used_text = Paragraph::new(used_tokens.to_string())
        .block(used_block)
        .alignment(Alignment::Center);

    // Remaining tokens block
    let remaining_block = Block::default()
        .title("Remaining")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let remaining_text = Paragraph::new(remaining.to_string())
        .block(remaining_block)
        .alignment(Alignment::Center);

    // Percentage block
    let percentage_block = Block::default()
        .title("Usage")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(usage_color));
    let percentage_text = Paragraph::new(format!("{}%", percentage))
        .block(percentage_block)
        .alignment(Alignment::Center);

    // Render widgets
    frame.render_widget(used_text, layout[0]);
    frame.render_widget(remaining_text, layout[1]);
    frame.render_widget(percentage_text, layout[2]);
}
