use crate::{app::App, CompressionLevel};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Gauge, Paragraph, Widget},
};

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut constraints = vec![
            Constraint::Length(5), // Height for the description block (borders + text + padding)
            Constraint::Length(3), // Height for the compression level selector
            Constraint::Length(3), // Height for the status / instruction block
        ];

        let show_progress = self.is_compressing || self.progress > 0.0;
        if show_progress {
            constraints.push(Constraint::Length(3)); // Height for the progress block
        }
        constraints.push(Constraint::Min(0)); // The remaining empty space on the screen

        let chunks = Layout::vertical(constraints).split(area);

        // --- Title / description block ---
        let title = Line::from(" Freya - Lossless Compression for files ".bold());
        let instructions = Line::from(vec![
            " Open File ".into(),
            "<o>".blue().bold(),
            " | Decompress ".into(),
            "<d>".blue().bold(),
            " | Level ".into(),
            "<↑/↓>".blue().bold(),
            " | Quit ".into(),
            "<Q> ".blue().bold(),
        ]);
        let title_block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_style(Style::new().blue())
            .border_set(border::DOUBLE);

        let description_text = Text::from(vec![Line::from(vec![
            " Freya helps compress your file types without losing the quality of the files.".into(),
        ])]);

        Paragraph::new(description_text)
            .left_aligned()
            .block(title_block)
            .render(chunks[0], buf);

        // --- Compression level selector ---
        let levels = [
            CompressionLevel::Fast,
            CompressionLevel::Normal,
            CompressionLevel::Best,
        ];
        let level_line: Line = {
            let mut spans = vec![" Level: ".into()];
            for lvl in levels {
                if lvl == self.compression_level {
                    spans.push(format!(" [{}] ", lvl.label()).yellow().bold());
                } else {
                    spans.push(format!("  {}  ", lvl.label()).into());
                }
            }
            spans.push("  ↑/↓ to change".dark_gray());
            Line::from(spans)
        };
        let level_block = Block::bordered()
            .border_style(Style::new().blue())
            .border_set(border::DOUBLE);
        Paragraph::new(Text::from(vec![level_line]))
            .left_aligned()
            .block(level_block)
            .render(chunks[1], buf);

        // --- Status message ---
        let status_text = Text::from(vec![Line::from(vec![self
            .status_message
            .to_string()
            .yellow()])]);
        let status_block = Block::bordered()
            .border_style(Style::new().blue())
            .border_set(border::DOUBLE);
        Paragraph::new(status_text)
            .left_aligned()
            .block(status_block)
            .render(chunks[2], buf);

        // --- Progress gauge (only shown during / after compression) ---
        if show_progress {
            let percentage = (self.progress * 100.0).clamp(0.0, 100.0) as u16;
            let gauge = Gauge::default()
                .block(
                    Block::bordered()
                        .title(" Progress ")
                        .border_style(Style::default().blue()),
                )
                .gauge_style(Style::default().fg(ratatui::style::Color::Yellow))
                .ratio(self.progress.clamp(0.0, 1.0))
                .label(format!("{}%", percentage));
            gauge.render(chunks[3], buf);
        }
    }
}
