use std::{fmt::Display, ops::Range};

pub enum Progress {
    Raw(String),
    Parsed {
        line: String,
        percent: Option<f32>,
        size: Option<Range<usize>>,
        speed: Option<Range<usize>>,
        eta: Option<Range<usize>>,
        frag: Option<u16>,
        frag_total: Option<u16>,
    },
}

impl Display for Progress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parsed {
                line,
                percent,
                size,
                speed,
                eta,
                frag,
                frag_total,
            } => {
                write!(f, "Parsed progress: ")?;
                if let Some(percent) = percent {
                    write!(f, "{:.1} % done. ", percent)?;
                }
                if let Some(size) = &size {
                    write!(f, "file size: {}. ", &line[size.clone()])?;
                }
                if let Some(speed) = &speed {
                    write!(f, "download speed: {}. ", &line[speed.clone()])?;
                }
                if let Some(eta) = &eta {
                    write!(f, "ETA: {}. ", &line[eta.clone()])?;
                }
                if let Some(frag) = frag {
                    write!(f, "fragments: {}", frag)?;
                    if let Some(frag_total) = frag_total {
                        write!(f, " / {}", frag_total)?;
                    }
                    write!(f, ". ")?;
                }
            }
            Progress::Raw(line) => write!(f, "Raw progress: {line}")?,
        }

        Ok(())
    }
}
