use color_eyre::eyre::{bail, Result};
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{stream, StreamExt};
use std::{borrow::Cow, io, sync::Arc};
use tokio::{sync::RwLock, time::MissedTickBehavior};
use tui::{
    backend::CrosstermBackend,
    layout::Alignment,
    text::Span,
    widgets::{Block, BorderType, Borders, Gauge, Paragraph, Row, Table},
    Terminal,
};

use crate::state::{
    video::{progress::VideoProgress, VideoRead},
    Progress, State,
};

mod layout;
mod style;

pub struct Ui;

impl Ui {
    pub fn new() -> Self {
        Ui
    }

    pub async fn event_loop(&self, state: &State, tick: u64) -> Result<()> {
        let mut terminal = self.take_terminal()?;

        // Stream input events (Keyboard, Mouse, Resize)
        let mut reader = EventStream::new();

        // Prepare render tick interval
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(tick));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        self.render(state, &mut terminal).await?;

        loop {
            tokio::select! {
                biased;

                // Handle streamed input events as they occur
                maybe_event = reader.next() => match maybe_event {
                    Some(Ok(event)) => if ! self.handle_event(event) { break },
                    // Event reader poll error, e.g. initialization failure, or interrupt
                    Some(Err(e)) => bail!(e),
                    // End of event stream
                    None => break,
                },

                // Render every N milliseconds
                _ = interval.tick() => self.render(state, &mut terminal).await?
            }
        }

        self.release_terminal(terminal)?;

        Ok(())
    }

    fn take_terminal(&self) -> Result<Terminal<CrosstermBackend<std::io::Stdout>>, io::Error> {
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);

        Terminal::new(backend)
    }

    fn release_terminal(
        &self,
        mut terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<(), io::Error> {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
        terminal.show_cursor()?;

        Ok(())
    }

    fn handle_event(&self, event: Event) -> bool {
        // TODO: Do I still want `clippy::match_like_matches_macro`?
        #[allow(clippy::match_like_matches_macro)]
        match event {
            // Handle keyboard event: Exit on Esc, Q or Ctrl+C
            Event::Key(KeyEvent {
                code: KeyCode::Esc,
                modifiers: _,
                ..
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: _,
                ..
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }) => false,

            // Handle other keyboard events later, e.g. to
            // select list itemsor scroll in long tables
            Event::Key(_) => true,

            // Mouse & Resize events
            _ => true,
        }
    }

    async fn render<'a, 's>(
        &self,
        state: &State,
        terminal: &'a mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        // The terminal's `draw()` method runs a sync closure, so we need to acquire all
        // read guards before we can start rendering.
        // First, the videos vec is locked to prevent new videos from being added.
        // Then, each video is asked to acquire read on its

        // Acquire read to the videos vec, to block new videos from being added while rendering.
        let videos = state.videos().await;

        let app_title = match *state.progress().await {
            Progress::Initializing => Cow::Borrowed(" INITIALIZING ... "),
            Progress::FetchingSourcePage(ref url) => {
                Cow::Owned(format!(" FETCHING SOURCE PAGE '{}' ... ", url))
            }
            Progress::ProcessingVideos => Cow::Borrowed(" VIMEO SHOWCASE DOWNLOAD "),
        };

        // Acquire read guards for all videos, to render full state.
        let videos_read: Arc<RwLock<Vec<VideoRead>>> =
            Arc::new(RwLock::new(Vec::with_capacity((*videos).len())));
        stream::iter(videos.iter())
            .for_each_concurrent(None, |video| {
                // Let each video acquire read as it sees fit. Wait for all to finish.
                let videos_read = videos_read.clone();
                async move {
                    let video_read = video.read().await;
                    let mut videos_read = videos_read.write().await;
                    (*videos_read).push(video_read);
                }
            })
            .await;

        // Acquire read on collected video read guards to render all in a sync(!) closure.
        let videos_read = (*videos_read).read().await;

        terminal.draw(|f| {
            let area = f.size();

            let chunks = layout::layout_chunks(area, &videos_read);

            f.render_widget(
                Table::new([])
                    .header(
                        Row::new(["Progress", "Size", "Speed", "ETA", "Fragments"])
                            .style(style::table_header_style())
                            .bottom_margin(1),
                    )
                    .widths(&layout::video_progress_table_layout())
                    .column_spacing(2)
                    .block(
                        Block::default()
                            .title(Span::styled(app_title, style::application_title_style()))
                            .title_alignment(Alignment::Center)
                            .borders(Borders::TOP)
                            .border_style(style::border_style())
                            .border_type(BorderType::Thick),
                    ),
                chunks[0],
            );

            for (i, video) in (*videos_read).iter().enumerate() {
                let chunk_start = 1 + i * layout::CHUNKS_PER_VIDEO;

                // Video title block
                f.render_widget(
                    Block::default()
                        .title(Span::styled(
                            format!(
                                "{} ",
                                match video.title() {
                                    Some(title) => title.as_str(),
                                    None => video.url(),
                                }
                            ),
                            style::video_title_style(),
                        ))
                        .borders(Borders::TOP)
                        .border_style(style::border_style())
                        .border_type(BorderType::Plain),
                    chunks[chunk_start],
                );

                // Video raw progress text or parsed progress
                let maybe_progress = video.progress();
                if let Some(progress) = &maybe_progress {
                    match progress {
                        VideoProgress::Raw(line) => {
                            f.render_widget(Paragraph::new(*line), chunks[chunk_start + 1])
                        }
                        VideoProgress::Parsed { .. } => f.render_widget(
                            Table::new([Row::new(progress.row().unwrap())])
                                .widths(&layout::video_progress_table_layout())
                                .column_spacing(2),
                            chunks[chunk_start + 1],
                        ),
                    };
                };

                // Video progress bar
                let gauge = Gauge::default()
                    .gauge_style(style::gauge_style())
                    .use_unicode(true);
                let gauge = match &maybe_progress {
                    Some(VideoProgress::Parsed { percent, .. }) => {
                        gauge.ratio(percent.unwrap_or(0.0) / 100.0)
                    }
                    _ => gauge,
                };

                f.render_widget(gauge, chunks[chunk_start + 2]);

                // Video bottom margin
                // (not rendered)
            }
        })?;

        Ok(())
    }
}
