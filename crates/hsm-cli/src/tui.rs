use std::{io, time::Duration};

use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect, Spacing},
    symbols::{self, merge::MergeStrategy},
    text::Line,
    widgets::{Block, Gauge, Padding, Widget},
};

pub fn app(terminal: &mut DefaultTerminal) -> io::Result<()> {
    App::default().run(terminal)
}

#[derive(Default)]
pub struct App {
    exit: bool,
}

impl App {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }

        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if crossterm::event::read()?.is_key_press() {
            self.exit = true;
        }

        Ok(())
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [header_area, main_area, status_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .areas::<3>(area);

        Header {}.render(header_area, buf);

        QueueView {}.render(main_area, buf);

        BottomStatus {
            track_name: "Artist Name - Song Name".into(),
            progress_duration: Duration::from_secs(32 + 4 * 60),
            track_total_duration: Duration::from_secs(12 + 18 * 60),
            queue_total_duration: Duration::from_secs(43 + 2 * 60 + 2 * 60 * 60),
            queue_num_tracks: 23,
        }
        .render(status_area, buf);
    }
}

struct QueueView {}

impl Widget for QueueView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [left_area, right_area] =
            Layout::horizontal([Constraint::Ratio(2, 5), Constraint::Ratio(3, 5)])
                .spacing(Spacing::Overlap(1))
                .areas::<2>(area);

        let left_block = Block::bordered()
            .border_set(symbols::border::ROUNDED)
            .merge_borders(MergeStrategy::Fuzzy);

        left_block.render(left_area, buf);

        let right_block = Block::bordered()
            .border_set(symbols::border::ROUNDED)
            .merge_borders(MergeStrategy::Fuzzy);

        right_block.render(right_area, buf);
    }
}

struct Header {}

impl Widget for Header {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_set(symbols::border::ROUNDED)
            .padding(Padding::horizontal(1));

        let tabs = [
            "Queue",
            "Artists",
            "Albums",
            "Tracks",
            "Providers",
            "Search",
        ];
        let layout = Layout::horizontal(
            tabs.iter()
                .map(|s| Constraint::Length(s.chars().count().try_into().unwrap())),
        )
        .spacing(1)
        .flex(Flex::SpaceEvenly)
        .split(block.inner(area));

        block.render(area, buf);

        for (tab, area) in tabs.iter().zip(layout.iter()) {
            Line::from(*tab).centered().render(*area, buf);
        }
    }
}

struct BottomStatus {
    track_name: String,
    progress_duration: Duration,
    track_total_duration: Duration,
    queue_total_duration: Duration,
    queue_num_tracks: usize,
}

impl BottomStatus {
    fn format_track_time(&self) -> String {
        let total_hours = self.track_total_duration.as_secs() / (60 * 60);
        let total_minutes = (self.track_total_duration.as_secs() / 60) % 60;
        let total_seconds = self.track_total_duration.as_secs() % 60;

        let progress_hours = self.progress_duration.as_secs() / (60 * 60);
        let progress_minutes = (self.progress_duration.as_secs() / 60) % 60;
        let progress_seconds = self.progress_duration.as_secs() % 60;

        if let Some(hour_width) = total_hours.checked_ilog10()
            && let Ok(hour_width) = usize::try_from(hour_width + 1)
        {
            format!(
                " {progress_hours:0hour_width$}:{progress_minutes:02}:{progress_seconds:02} / {total_hours:0hour_width$}:{total_minutes:02}:{total_seconds:02} "
            )
        } else {
            format!(
                " {progress_minutes:02}:{progress_seconds:02} / {total_minutes:02}:{total_seconds:02} "
            )
        }
    }

    fn format_queue_time(&self) -> String {
        let total_hours = self.queue_total_duration.as_secs() / (60 * 60);
        let total_minutes = (self.queue_total_duration.as_secs() / 60) % 60;
        let total_seconds = self.queue_total_duration.as_secs() % 60;

        let num_tracks = self.queue_num_tracks;
        if total_hours > 0 {
            format!(" {num_tracks} tracks / {total_hours}:{total_minutes:02}:{total_seconds:02} ")
        } else {
            format!(" {num_tracks} tracks / {total_minutes:02}:{total_seconds:02} ")
        }
    }
}

impl Widget for &BottomStatus {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title(format!(" {} ", self.track_name))
            .title(Line::from(self.format_queue_time()).right_aligned())
            .border_set(symbols::border::ROUNDED)
            .padding(Padding::horizontal(1));

        Gauge::default()
            .block(block)
            .ratio(self.progress_duration.as_secs_f64() / self.track_total_duration.as_secs_f64())
            .use_unicode(true)
            .label(self.format_track_time())
            .render(area, buf);
    }
}
