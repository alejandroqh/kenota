//! Charts: a `<table data-chart="…">` read as data.
//!
//! The first row names the series; its first cell is a corner and is
//! ignored. Every other row is a category: its first cell is the label (the
//! X value for `xy`), the rest are one value per series. Empty cells are
//! missing values. Numbers may use thousands separators, a currency sign or
//! a `%` sign.

use crate::style::StyledNode;

/// The chart types of MS Works.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Bars side by side, one per series.
    Bar,
    /// One bar per category, the series stacked.
    Stacked,
    /// Stacked bars that all reach 100%.
    Stacked100,
    Line,
    /// Stacked lines with the space under each filled.
    Area,
    /// A line from the lowest to the highest value of each category; the
    /// third series is marked as the close, the fourth as the open.
    HiLo,
    /// The first series as slices.
    Pie,
    /// Points placed by the numbers in the first column and each series.
    Xy,
}

impl Kind {
    pub const NAMES: [&str; 8] = [
        "bar",
        "stacked",
        "stacked100",
        "line",
        "area",
        "hilo",
        "pie",
        "xy",
    ];

    pub fn parse(name: &str) -> Option<Kind> {
        let kinds = [
            Kind::Bar,
            Kind::Stacked,
            Kind::Stacked100,
            Kind::Line,
            Kind::Area,
            Kind::HiLo,
            Kind::Pie,
            Kind::Xy,
        ];
        Self::NAMES
            .iter()
            .position(|known| known.eq_ignore_ascii_case(name.trim()))
            .map(|i| kinds[i])
    }
}

/// The chart type of `node`, if it is a table with a known `data-chart`.
pub fn kind(node: &StyledNode) -> Option<Kind> {
    if node.tag() != Some("table") {
        return None;
    }
    Kind::parse(node.attr("data-chart")?)
}

#[derive(Debug)]
pub struct Series<'a> {
    pub name: String,
    /// The header cell; its `background` and `--fill` pick the series' look.
    pub header: Option<&'a StyledNode>,
    /// One per category.
    pub values: Vec<Option<f64>>,
}

#[derive(Debug)]
pub struct Category<'a> {
    pub label: String,
    /// The first cell of the row; for pie slices, its `background` and
    /// `--fill` pick the slice's look.
    pub cell: &'a StyledNode,
}

impl Category<'_> {
    /// The label as a number, for the X axis of `xy` charts.
    pub fn x(&self) -> Option<f64> {
        number(&self.label)
    }
}

#[derive(Debug)]
pub struct Chart<'a> {
    pub kind: Kind,
    pub caption: Option<&'a StyledNode>,
    pub categories: Vec<Category<'a>>,
    pub series: Vec<Series<'a>>,
}

impl<'a> Chart<'a> {
    /// Reads the chart of a table with a known `data-chart` type.
    pub fn from_table(table: &'a StyledNode) -> Option<Self> {
        let kind = kind(table)?;
        let caption = table.children.iter().find(|c| c.tag() == Some("caption"));
        let rows = rows(table);
        let Some((header, body)) = rows.split_first() else {
            return Some(Self {
                kind,
                caption,
                categories: Vec::new(),
                series: Vec::new(),
            });
        };
        let count = rows
            .iter()
            .map(|row| row.len().saturating_sub(1))
            .max()
            .unwrap_or(0);
        let series = (0..count)
            .map(|i| {
                let header = header.get(i + 1).copied();
                let name = header
                    .map(StyledNode::text)
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| format!("Series {}", i + 1));
                let values = body
                    .iter()
                    .map(|row| row.get(i + 1).and_then(|cell| number(&cell.text())))
                    .collect();
                Series {
                    name,
                    header,
                    values,
                }
            })
            .collect();
        let categories = body
            .iter()
            .map(|row| Category {
                label: row[0].text(),
                cell: row[0],
            })
            .collect();
        Some(Self {
            kind,
            caption,
            categories,
            series,
        })
    }
}

/// The cells of each non-empty row, header, body and footer in that order.
fn rows(table: &StyledNode) -> Vec<Vec<&StyledNode>> {
    fn cells(row: &StyledNode) -> Vec<&StyledNode> {
        row.children
            .iter()
            .filter(|cell| matches!(cell.tag(), Some("td" | "th")))
            .collect()
    }
    let (mut head, mut body, mut foot) = (Vec::new(), Vec::new(), Vec::new());
    for child in &table.children {
        match child.tag() {
            Some("tr") => body.push(cells(child)),
            Some(section @ ("thead" | "tbody" | "tfoot")) => {
                let rows = child
                    .children
                    .iter()
                    .filter(|row| row.tag() == Some("tr"))
                    .map(cells);
                match section {
                    "thead" => head.extend(rows),
                    "tfoot" => foot.extend(rows),
                    _ => body.extend(rows),
                }
            }
            _ => {}
        }
    }
    head.into_iter()
        .chain(body)
        .chain(foot)
        .filter(|row| !row.is_empty())
        .collect()
}

/// A cell's number: `1,250`, `$12.50`, `45%` and `−3` all count.
pub fn number(text: &str) -> Option<f64> {
    let cleaned: String = text
        .trim()
        .trim_end_matches('%')
        .chars()
        .filter(|c| !matches!(c, ',' | '_' | ' ' | '$' | '€' | '£' | '¥'))
        .map(|c| if c == '−' { '-' } else { c })
        .collect();
    cleaned.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Adds a warning for each chart problem under `node`: unknown types and
/// cells that are not numbers.
pub fn check(node: &StyledNode, warnings: &mut Vec<String>) {
    let mut warn = |warning: String| {
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    };
    if node.tag() == Some("table")
        && let Some(name) = node.attr("data-chart")
    {
        if Kind::parse(name).is_none() {
            warn(format!(
                "unknown chart type `{}` drawn as a table (types: {})",
                name.trim(),
                Kind::NAMES.join(", ")
            ));
        } else if let Some(chart) = Chart::from_table(node) {
            if chart.categories.is_empty() || chart.series.is_empty() {
                warn("chart without data: add a header row and a row per category".into());
            }
            let rows = rows(node);
            for row in rows.iter().skip(1) {
                for (i, cell) in row.iter().enumerate() {
                    let text = cell.text();
                    let numeric = i > 0 || chart.kind == Kind::Xy;
                    if numeric && !text.is_empty() && number(&text).is_none() {
                        warn(format!("chart value `{text}` is not a number"));
                    }
                }
            }
        }
    }
    for child in &node.children {
        check(child, warnings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html;

    fn table(source: &str) -> StyledNode {
        let slide = html::parse(source, None);
        slide.root.find("table").unwrap().clone()
    }

    #[test]
    fn reads_series_and_categories() {
        let table = table(
            "<table data-chart='Bar'>
               <caption>Sales</caption>
               <thead><tr><th></th><th>North</th><th style='background: red'>South</th></tr></thead>
               <tr><td>Q1</td><td>1,200</td><td>$8.5</td></tr>
               <tr><td>Q2</td><td></td><td>45%</td></tr>
             </table>",
        );
        let chart = Chart::from_table(&table).unwrap();
        assert_eq!(chart.kind, Kind::Bar);
        assert_eq!(chart.caption.unwrap().text(), "Sales");
        let labels: Vec<_> = chart.categories.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["Q1", "Q2"]);
        assert_eq!(chart.series.len(), 2);
        assert_eq!(chart.series[0].name, "North");
        assert_eq!(chart.series[0].values, [Some(1200.0), None]);
        assert_eq!(chart.series[1].values, [Some(8.5), Some(45.0)]);
        assert!(chart.series[1].header.unwrap().style.background.is_some());
    }

    #[test]
    fn unnamed_series_get_numbers() {
        let table = table(
            "<table data-chart='line'><tr><td></td><td>A</td></tr><tr><td>x</td><td>1</td><td>2</td></tr></table>",
        );
        let chart = Chart::from_table(&table).unwrap();
        assert_eq!(chart.series[1].name, "Series 2");
        assert_eq!(chart.series[1].values, [Some(2.0)]);
    }

    #[test]
    fn numbers() {
        assert_eq!(number(" 12 "), Some(12.0));
        assert_eq!(number("−3.5"), Some(-3.5));
        assert_eq!(number("1 000"), Some(1000.0));
        assert_eq!(number("n/a"), None);
        assert_eq!(number(""), None);
        assert_eq!(number("inf"), None);
    }

    #[test]
    fn only_known_types_on_tables() {
        assert_eq!(
            kind(&table("<table data-chart='pie'></table>")),
            Some(Kind::Pie)
        );
        assert_eq!(kind(&table("<table data-chart='radar'></table>")), None);
        assert_eq!(kind(&table("<table></table>")), None);
    }

    #[test]
    fn checks_report_problems() {
        let slide = html::parse(
            "<table data-chart='radar'></table>
             <table data-chart='bar'><tr><th></th><th>A</th></tr><tr><td>x</td><td>n/a</td></tr></table>
             <table data-chart='xy'><tr><th>X</th><th>Y</th></tr><tr><td>one</td><td>1</td></tr></table>
             <table data-chart='pie'></table>",
            None,
        );
        let mut warnings = Vec::new();
        check(&slide.root, &mut warnings);
        assert_eq!(
            warnings,
            [
                "unknown chart type `radar` drawn as a table (types: bar, stacked, stacked100, line, area, hilo, pie, xy)",
                "chart value `n/a` is not a number",
                "chart value `one` is not a number",
                "chart without data: add a header row and a row per category",
            ]
        );
    }
}
