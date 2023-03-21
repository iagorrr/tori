use std::time::{Duration, Instant};

use tui::{
    layout::Rect,
    style::{Color, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::MyBackend;

const WIDTH: u16 = 40;

#[derive(Debug)]
pub struct Notification {
    pub text: String,
    pub show_until: Instant,
    pub color: Color,
    height: u16,
}

impl Default for Notification {
    fn default() -> Self {
        Self {
            text: String::new(),
            show_until: Instant::now(),
            color: Color::White,
            height: 0,
        }
    }
}

impl Notification {
    pub fn new(text: String, duration: Duration) -> Self {
        let height = count_lines(&text) + 2;
        Self {
            text,
            show_until: Instant::now() + duration,
            height,
            ..Default::default()
        }
    }

    pub fn colored(mut self, c: Color) -> Self {
        self.color = c;
        self
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() > self.show_until
    }

    pub fn render(&self, frame: &mut Frame<'_, MyBackend>) {
        if self.is_expired() {
            return;
        }

        let size = frame.size();
        let chunk = Rect {
            x: size.width - WIDTH - 1,
            y: size.height - self.height - 1,
            width: WIDTH,
            height: self.height,
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(self.color));

        let text = Paragraph::new(self.text.as_ref())
            .block(block)
            .style(Style::default().fg(self.color))
            .wrap(Wrap { trim: true });

        frame.render_widget(Clear, chunk);
        frame.render_widget(text, chunk);
    }
}

/// Copied from tui::widgets::reflow because the module is private :(
mod reflow {
    use tui::text::StyledGrapheme;
    use unicode_width::UnicodeWidthStr;

    const NBSP: &str = "\u{00a0}";

    /// A state machine to pack styled symbols into lines.
    /// Cannot implement it as Iterator since it yields slices of the internal buffer (need streaming
    /// iterators for that).
    pub trait LineComposer<'a> {
        fn next_line(&mut self) -> Option<(&[StyledGrapheme<'a>], u16)>;
    }

    /// A state machine that wraps lines on word boundaries.
    pub struct WordWrapper<'a, 'b> {
        symbols: &'b mut dyn Iterator<Item = StyledGrapheme<'a>>,
        max_line_width: u16,
        current_line: Vec<StyledGrapheme<'a>>,
        next_line: Vec<StyledGrapheme<'a>>,
        /// Removes the leading whitespace from lines
        trim: bool,
    }

    impl<'a, 'b> WordWrapper<'a, 'b> {
        pub fn new(
            symbols: &'b mut dyn Iterator<Item = StyledGrapheme<'a>>,
            max_line_width: u16,
            trim: bool,
        ) -> WordWrapper<'a, 'b> {
            WordWrapper {
                symbols,
                max_line_width,
                current_line: vec![],
                next_line: vec![],
                trim,
            }
        }
    }

    impl<'a, 'b> LineComposer<'a> for WordWrapper<'a, 'b> {
        fn next_line(&mut self) -> Option<(&[StyledGrapheme<'a>], u16)> {
            if self.max_line_width == 0 {
                return None;
            }
            std::mem::swap(&mut self.current_line, &mut self.next_line);
            self.next_line.truncate(0);

            let mut current_line_width = self
                .current_line
                .iter()
                .map(|StyledGrapheme { symbol, .. }| symbol.width() as u16)
                .sum();

            let mut symbols_to_last_word_end: usize = 0;
            let mut width_to_last_word_end: u16 = 0;
            let mut prev_whitespace = false;
            let mut symbols_exhausted = true;
            for StyledGrapheme { symbol, style } in &mut self.symbols {
                symbols_exhausted = false;
                let symbol_whitespace = symbol.chars().all(&char::is_whitespace) && symbol != NBSP;

                // Ignore characters wider that the total max width.
                if symbol.width() as u16 > self.max_line_width
                    // Skip leading whitespace when trim is enabled.
                    || self.trim && symbol_whitespace && symbol != "\n" && current_line_width == 0
                {
                    continue;
                }

                // Break on newline and discard it.
                if symbol == "\n" {
                    if prev_whitespace {
                        current_line_width = width_to_last_word_end;
                        self.current_line.truncate(symbols_to_last_word_end);
                    }
                    break;
                }

                // Mark the previous symbol as word end.
                if symbol_whitespace && !prev_whitespace {
                    symbols_to_last_word_end = self.current_line.len();
                    width_to_last_word_end = current_line_width;
                }

                self.current_line.push(StyledGrapheme { symbol, style });
                current_line_width += symbol.width() as u16;

                if current_line_width > self.max_line_width {
                    // If there was no word break in the text, wrap at the end of the line.
                    let (truncate_at, truncated_width) = if symbols_to_last_word_end != 0 {
                        (symbols_to_last_word_end, width_to_last_word_end)
                    } else {
                        (self.current_line.len() - 1, self.max_line_width)
                    };

                    // Push the remainder to the next line but strip leading whitespace:
                    {
                        let remainder = &self.current_line[truncate_at..];
                        if let Some(remainder_nonwhite) =
                            remainder.iter().position(|StyledGrapheme { symbol, .. }| {
                                !symbol.chars().all(&char::is_whitespace)
                            })
                        {
                            self.next_line
                                .extend_from_slice(&remainder[remainder_nonwhite..]);
                        }
                    }
                    self.current_line.truncate(truncate_at);
                    current_line_width = truncated_width;
                    break;
                }

                prev_whitespace = symbol_whitespace;
            }

            // Even if the iterator is exhausted, pass the previous remainder.
            if symbols_exhausted && self.current_line.is_empty() {
                None
            } else {
                Some((&self.current_line[..], current_line_width))
            }
        }
    }
}

fn count_lines(text: &str) -> u16 {
    use reflow::LineComposer;

    let mut count = 0;
    let span = Span::raw(text);
    let mut graphemes = span.styled_graphemes(Style::default());
    let mut word_wrapper = reflow::WordWrapper::new(&mut graphemes, WIDTH, true);
    while let Some(_line) = word_wrapper.next_line() {
        count += 1;
    }
    count
}
