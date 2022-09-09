use std::{borrow::Cow, fmt::Display, ops::Range};

pub enum ProgressDetail<'a> {
    Raw(&'a str),
    Parsed {
        line: &'a str,
        // Using f64 instead of f32 to match `tui::widget::Gauge.ratio`.
        percent: Option<f64>,

        size: Option<Range<usize>>,
        speed: Option<Range<usize>>,
        eta: Option<Range<usize>>,
        frag: Option<u16>,
        frag_total: Option<u16>,
    },
}

impl<'a> ProgressDetail<'a> {
    pub fn to_table_cells(&self) -> Option<[Cow<'a, str>; 4]> {
        match self {
            Self::Raw(_) => None,
            Self::Parsed {
                line,
                size,
                speed,
                eta,
                frag,
                frag_total,
                .. // `percent` is left out, as last known percent is rendered at all times.
            } => Some([
                Cow::Borrowed(match size {
                    Some(size) => &line[size.clone()],
                    None => "",
                }),
                Cow::Borrowed(match speed {
                    Some(speed) => &line[speed.clone()],
                    None => "",
                }),
                Cow::Borrowed(match eta {
                    Some(eta) => &line[eta.clone()],
                    None => "",
                }),
                match frag {
                    Some(frag) => Cow::Owned({
                        let mut sections = Vec::with_capacity(2);
                        sections.push(frag.to_string());
                        if let Some(frag_total) = frag_total {
                            sections.push(frag_total.to_string());
                        }
                        sections.join(" / ")
                    }),
                    None => Cow::Borrowed(""),
                },
            ]),
        }
    }
}

impl<'a> Display for ProgressDetail<'a> {
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
            ProgressDetail::Raw(line) => write!(f, "{line}")?,
        }

        Ok(())
    }
}
