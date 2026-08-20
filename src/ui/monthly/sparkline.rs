use crate::data::SingleMonth;

const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
const SPARK_MONTHS: usize = 6;

pub(super) fn sparkline_str(values: &[f64]) -> String {
    if values.is_empty() {
        return String::new();
    }
    let max = values.iter().cloned().fold(0.0f64, f64::max);
    if max <= 0.0 {
        return "▁".repeat(values.len());
    }
    values
        .iter()
        .map(|&v| BLOCKS[((v / max * 7.0) as usize).min(7)])
        .collect()
}

pub(super) fn category_expense_history(
    months: &[SingleMonth],
    through_month: &str,
    category: &str,
) -> Vec<f64> {
    let end = months
        .iter()
        .position(|m| m.month_name == through_month)
        .map(|i| i + 1)
        .unwrap_or(months.len());
    let start = end.saturating_sub(SPARK_MONTHS);
    months[start..end]
        .iter()
        .map(|m| {
            m.expenses
                .iter()
                .find(|(name, _)| name == category)
                .map(|(_, amount)| *amount)
                .unwrap_or(0.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_str_empty() {
        assert_eq!(sparkline_str(&[]), "");
    }

    #[test]
    fn sparkline_str_all_zero() {
        assert_eq!(sparkline_str(&[0.0, 0.0, 0.0]), "▁▁▁");
    }

    #[test]
    fn sparkline_str_ascending() {
        let chars: Vec<char> = sparkline_str(&[0.0, 50.0, 100.0]).chars().collect();
        assert!(chars[0] <= chars[1]);
        assert!(chars[1] <= chars[2]);
        assert_eq!(chars[2], '█');
    }

    #[test]
    fn sparkline_str_single_max() {
        assert_eq!(sparkline_str(&[100.0]), "█");
    }

    fn month(name: &str, category: &str, amount: f64) -> SingleMonth {
        SingleMonth {
            month_name: name.to_string(),
            expenses: vec![(category.to_string(), amount)],
            ..SingleMonth::default()
        }
    }

    #[test]
    fn category_history_through_selected_month() {
        let months = vec![
            month("January", "expenses:food", 10.0),
            month("February", "expenses:food", 20.0),
            month("March", "expenses:food", 30.0),
        ];
        let got = category_expense_history(&months, "February", "expenses:food");
        assert_eq!(got, vec![10.0, 20.0]);
    }

    #[test]
    fn category_history_missing_month_is_zero() {
        let months = vec![
            month("January", "expenses:food", 10.0),
            SingleMonth {
                month_name: "February".to_string(),
                ..SingleMonth::default()
            },
        ];
        let got = category_expense_history(&months, "February", "expenses:food");
        assert_eq!(got, vec![10.0, 0.0]);
    }
}
