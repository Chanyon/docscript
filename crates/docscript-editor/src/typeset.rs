//! Text layout primitives: HarfBuzz-shaped glyphs are fed into a
//! Knuth–Plass optimal line breaker.
//!
//! The caller shapes text into glyph boxes (with glue between words) and
//! penalties at allowed break opportunities. This module then minimizes the
//! classic badness + demerits objective over all legal break sequences.

#[derive(Debug, Clone)]
pub enum Item<T> {
    /// An unbreakable box carrying a shaped glyph or glyph cluster.
    Box(T, f32),
    /// Inter-word glue: width, stretchability, shrinkability.
    Glue {
        width: f32,
        stretch: f32,
        shrink: f32,
    },
    /// A legal break opportunity. `penalty` < 0 marks a forced break;
    /// very large positive penalties are emergency breaks inside words.
    Penalty { width: f32, penalty: f32 },
}

impl<T> Item<T> {
    pub fn width(&self) -> f32 {
        match self {
            Item::Box(_, width) => *width,
            Item::Glue { width, .. } => *width,
            Item::Penalty { width, .. } => *width,
        }
    }

    pub fn is_glue(&self) -> bool {
        matches!(self, Item::Glue { .. })
    }

    pub fn penalty(&self) -> Option<f32> {
        match self {
            Item::Penalty { penalty, .. } => Some(*penalty),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Line<T> {
    pub items: Vec<Item<T>>,
    pub width: f32,
    /// Adjustment ratio used for the line (negative = stretched/shrunk).
    pub ratio: f32,
}

/// Break `items` into lines no wider than `line_width` when possible.
///
/// The input should contain at least a forced penalty at the beginning and at
/// the end. If not, forced boundaries are added automatically.
pub fn break_lines<T: Clone>(items: &[Item<T>], line_width: f32) -> Vec<Line<T>> {
    if items.is_empty() || line_width <= 0.0 {
        return Vec::new();
    }

    let mut work = items.to_vec();
    if !matches!(work.first(), Some(Item::Penalty { penalty, .. }) if *penalty < 0.0) {
        work.insert(
            0,
            Item::Penalty {
                width: 0.0,
                penalty: -10_000.0,
            },
        );
    }
    if !matches!(work.last(), Some(Item::Penalty { penalty, .. }) if *penalty < 0.0) {
        work.push(Item::Penalty {
            width: 0.0,
            penalty: -10_000.0,
        });
    }

    let forced: Vec<usize> = work
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match item {
            Item::Penalty { penalty, .. } if *penalty < 0.0 => Some(index),
            _ => None,
        })
        .collect();

    let mut lines = Vec::new();
    for pair in forced.windows(2) {
        let start = pair[0];
        let end = pair[1];
        if end <= start + 1 {
            lines.push(Line {
                items: Vec::new(),
                width: 0.0,
                ratio: 0.0,
            });
            continue;
        }
        lines.extend(break_segment(&work[start..=end], line_width));
    }
    lines
}

fn break_segment<T: Clone>(items: &[Item<T>], line_width: f32) -> Vec<Line<T>> {
    if items.len() < 2 {
        return Vec::new();
    }

    let mut prefix_width = vec![0.0f32; items.len() + 1];
    let mut prefix_stretch = vec![0.0f32; items.len() + 1];
    let mut prefix_shrink = vec![0.0f32; items.len() + 1];
    for (index, item) in items.iter().enumerate() {
        prefix_width[index + 1] = prefix_width[index] + item.width();
        if let Item::Glue {
            stretch, shrink, ..
        } = item
        {
            prefix_stretch[index + 1] = prefix_stretch[index] + stretch;
            prefix_shrink[index + 1] = prefix_shrink[index] + shrink;
        } else {
            prefix_stretch[index + 1] = prefix_stretch[index];
            prefix_shrink[index + 1] = prefix_shrink[index];
        }
    }

    let breaks: Vec<usize> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| item.penalty().map(|_| index))
        .collect();
    if breaks.len() < 2 {
        return Vec::new();
    }

    let start = breaks[0];
    let end = *breaks.last().unwrap();
    let mut best = vec![f64::INFINITY; items.len()];
    let mut prev: Vec<Option<usize>> = vec![None; items.len()];
    best[start] = 0.0;

    for (break_index, &current) in breaks.iter().enumerate().skip(1) {
        let is_last = current == end;
        let mut fallback: Option<(usize, f64)> = None;

        for &previous in breaks[..break_index].iter().rev() {
            if best[previous].is_infinite() {
                continue;
            }
            let (width, stretch, shrink) = line_metrics(
                items,
                &prefix_width,
                &prefix_stretch,
                &prefix_shrink,
                previous,
                current,
            );
            if width > line_width * 2.2 && !is_last {
                break;
            }

            let ratio = if width > line_width {
                if shrink > 0.0 {
                    (line_width - width) / shrink
                } else {
                    -10.0
                }
            } else if is_last {
                0.0
            } else if stretch > 0.0 {
                (line_width - width) / stretch
            } else {
                10.0
            };

            if ratio < -1.0 {
                continue;
            }

            let badness = (100.0 * ratio.abs().powi(3)).min(100_000.0);
            let penalty = items[current].penalty().unwrap_or(0.0).max(0.0);
            let demerits =
                best[previous] + (10.0 + badness as f64).powi(2) + (penalty * penalty) as f64;

            if fallback.is_none_or(|(_, fallback_demerits)| demerits < fallback_demerits) {
                fallback = Some((previous, demerits));
            }
            if demerits < best[current] {
                best[current] = demerits;
                prev[current] = Some(previous);
            }
        }

        if best[current].is_infinite()
            && let Some((previous, demerits)) = fallback
        {
            best[current] = demerits + 1_000_000_000.0;
            prev[current] = Some(previous);
        }
    }

    if prev[end].is_none() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut cursor = end;
    while let Some(previous) = prev[cursor] {
        ranges.push((previous, cursor));
        cursor = previous;
        if cursor == start {
            break;
        }
    }
    ranges.reverse();

    let mut lines = Vec::with_capacity(ranges.len());
    for (from, to) in ranges {
        let (width, _, _) = line_metrics(
            items,
            &prefix_width,
            &prefix_stretch,
            &prefix_shrink,
            from,
            to,
        );
        let mut line_items: Vec<Item<T>> = items[from + 1..to]
            .iter()
            .filter(|item| !matches!(item, Item::Penalty { .. }))
            .cloned()
            .collect();
        while line_items.last().is_some_and(Item::is_glue) {
            line_items.pop();
        }
        lines.push(Line {
            items: line_items,
            width,
            ratio: 0.0,
        });
    }
    lines
}
fn line_metrics<T>(
    items: &[Item<T>],
    prefix_width: &[f32],
    prefix_stretch: &[f32],
    prefix_shrink: &[f32],
    from: usize,
    to: usize,
) -> (f32, f32, f32) {
    let mut width = prefix_width[to] - prefix_width[from + 1];
    let mut stretch = prefix_stretch[to] - prefix_stretch[from + 1];
    let mut shrink = prefix_shrink[to] - prefix_shrink[from + 1];

    if to > from + 1
        && let Item::Glue {
            width: glue_width,
            stretch: glue_stretch,
            shrink: glue_shrink,
        } = &items[to - 1]
    {
        width -= glue_width;
        stretch -= glue_stretch;
        shrink -= glue_shrink;
    }

    (width.max(0.0), stretch.max(0.0), shrink.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breaks_into_reasonable_lines() {
        let mut items = vec![Item::Penalty {
            width: 0.0,
            penalty: -10_000.0,
        }];
        for word in ["alpha", "beta", "gamma", "delta", "epsilon"] {
            items.push(Item::Box(word.to_string(), 38.0));
            items.push(Item::Glue {
                width: 8.0,
                stretch: 4.0,
                shrink: 2.0,
            });
            items.push(Item::Penalty {
                width: 0.0,
                penalty: 0.0,
            });
        }
        items.push(Item::Penalty {
            width: 0.0,
            penalty: -10_000.0,
        });

        let lines = break_lines(&items, 100.0);
        assert!(!lines.is_empty());
        assert!(lines.iter().all(|line| line.width < 120.0));
        let text: Vec<String> = lines
            .iter()
            .map(|line| {
                line.items
                    .iter()
                    .filter_map(|item| match item {
                        Item::Box(text, _) => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        assert_eq!(text.join(" "), "alpha beta gamma delta epsilon");
    }

    #[test]
    fn forced_break_is_respected() {
        let items = vec![
            Item::Penalty {
                width: 0.0,
                penalty: -10_000.0,
            },
            Item::Box("a", 10.0),
            Item::Penalty {
                width: 0.0,
                penalty: -10_000.0,
            },
            Item::Box("b", 10.0),
            Item::Penalty {
                width: 0.0,
                penalty: -10_000.0,
            },
        ];
        let lines = break_lines(&items, 100.0);
        assert_eq!(lines.len(), 2);
    }
}
