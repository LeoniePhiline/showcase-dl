use color_eyre::eyre::{bail, Result};
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{
    future::{AbortHandle, Abortable},
    stream, Future, StreamExt,
};
use std::{borrow::Cow, io, sync::Arc};
use tokio::{sync::RwLock, time::MissedTickBehavior};
use tui::{
    backend::CrosstermBackend,
    layout::Alignment,
    text::Span,
    widgets::{Block, BorderType, Borders, Gauge, Row, Table},
    Terminal,
};

use crate::state::{
    video::{progress::ProgressDetail, Stage as VideoStage, VideoRead},
    Stage, State,
};

mod layout;
mod style;

pub struct Ui;

impl Ui {
    pub fn new() -> Self {
        Ui
    }

    pub async fn event_loop(
        &self,
        state: Arc<State>,
        tick: u64,
        do_work: impl Future<Output = Result<()>>,
    ) -> Result<()> {
        let mut terminal = self.take_terminal()?;

        // Stream input events (Keyboard, Mouse, Resize)
        let mut reader = EventStream::new();

        // Prepare render tick interval
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(tick));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        self.render(&state, &mut terminal).await?;

        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        let do_work_abortable = Abortable::new(
            async {
                // Drive application process futures via closure.
                do_work.await
            },
            abort_registration,
        );

        tokio::try_join!(
            async {
                // Drive application process futures, aborting in reaction to user request.
                match do_work_abortable.await.ok() {
                    Some(result) => result,
                    // Swallow futures::future::Aborted error.
                    None => Ok(()),
                }
            },
            async {
                // Handle events or wait for next render tick.
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

                        // Note: We *might* also want to break out of the event loop
                        //       as soon as `state.stage()` switches to `Stage::Done`.
                        //       ...
                        // TODO: Implement that? Or prefer keeping the app open
                        //        until explicitly closed by the user? (Esc, Q or Ctrl+C)

                        // Render every N milliseconds
                        _ = interval.tick() => self.render(&state, &mut terminal).await?
                    }
                }

                // Abort the application process futures as soon
                // as the user requests the app to terminate.
                abort_handle.abort();

                Ok(())
            }
        )?;

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
            // select list items or scroll in long tables
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

        let app_title = match *state.stage().await {
            Stage::Initializing => Cow::Borrowed(" INITIALIZING ... "),
            Stage::FetchingSource(ref url) => {
                Cow::Owned(format!(" FETCHING SOURCE PAGE '{}' ... ", url))
            }
            Stage::Processing => Cow::Borrowed(" VIMEO SHOWCASE DOWNLOAD "),
            Stage::Done => Cow::Borrowed(" FINISHED! "),
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
                        Row::new(["Stage", "Progress", "Size", "Speed", "ETA", "Fragments"])
                            .style(style::table_header_style())
                            .bottom_margin(1),
                    )
                    .widths(&layout::video_progress_detail_table_layout())
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
                // TODO: Create a video widget?
                // TODO: Create a scrollable(!) "list of videos" widget

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
                let progress_detail_chunk = chunks[chunk_start + 1];
                let display_percent = video
                    .last_percent()
                    .unwrap_or_else(|| Self::video_percent_default(video.stage()));
                let maybe_progress_detail = video.progress_detail();
                if let Some(progress) = &maybe_progress_detail {
                    // Build two variants of details table, depending on if we have a
                    // `ProgressDetail::Raw(line)`, rendered as basics + unparsed `yt-dlp` output line,
                    //  or a `ProgressDetail::Parsed { .. }`, rendered as full table of download stats.
                    let mut row = Vec::with_capacity(match progress {
                        ProgressDetail::Raw(_) => 3,
                        ProgressDetail::Parsed { .. } => 6,
                    });

                    // Column "Stage"
                    row.push(Span::styled(
                        match video.stage() {
                            VideoStage::Initializing => "Intializing...",
                            VideoStage::Downloading => "Downloading...",
                            VideoStage::Finished => "Finished!",
                        },
                        style::video_stage_style(video.stage()),
                    ));

                    // Column "Progress", using the last known progress,
                    // as a fresh value can not in all cases be parsed from the current line.
                    row.push(Span::raw(Cow::Owned(format!("{:.1} %", display_percent))));

                    match progress {
                        ProgressDetail::Raw(line) => {
                            // Single column, spanning across "Size", "Speed", "ETA" and "Fragments"
                            row.push(Span::raw(match video.stage() {
                                // Avoid showing the last output line when video progress is entirely finished.
                                // Often this just says "Deleting output file [...]" after merging video
                                // and audio formats. Which is just confusing to end users.
                                VideoStage::Finished => "",
                                // Display the last raw output line as long as video progress is not yet finished.
                                _ => *line,
                            }));

                            f.render_widget(
                                Table::new([Row::new(row)])
                                    .widths(&layout::video_raw_progress_table_layout())
                                    .column_spacing(2),
                                progress_detail_chunk,
                            )
                        }
                        ProgressDetail::Parsed { .. } => {
                            // Columns "Size", "Speed", "ETA" and "Fragments"
                            row.append(
                                &mut progress
                                    .to_table_cells()
                                    // Unwrapping is oanic-safe here, as `.to_table_cells()`
                                    // always returns `Some([Cow<'a, str>; 4])`
                                    // for the `ProgressDetail::Parsed` enum variant.
                                    .unwrap()
                                    .into_iter()
                                    .map(Span::raw)
                                    .collect::<Vec<Span>>(),
                            );

                            f.render_widget(
                                Table::new([Row::new(row)])
                                    .widths(&layout::video_progress_detail_table_layout())
                                    .column_spacing(2),
                                progress_detail_chunk,
                            )
                        }
                    };
                };

                // Video progress bar
                let gauge = Gauge::default()
                    .gauge_style(style::gauge_style(video.stage()))
                    .use_unicode(true)
                    .ratio(display_percent / 100.0);

                f.render_widget(gauge, chunks[chunk_start + 2]);

                // Video bottom margin
                // (not rendered)
            }
        })?;

        Ok(())
    }

    fn video_percent_default(stage: &VideoStage) -> f64 {
        match stage {
            // When a video is already present before starting the app,
            // then this video will be finished without `video.last_percent`
            // ever having been set. In that case, display 100 % right away.
            VideoStage::Finished => 100.0,
            _ => 0.0,
        }
    }
}
