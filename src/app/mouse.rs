use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::{App, MonthlyFocus, NetWorthRange, PortfolioRange, Tab};

impl App {
    pub fn handle_mouse(&mut self, ev: MouseEvent) {
        if self.loading {
            return;
        }
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.on_left_click(ev.column, ev.row);
            }
            MouseEventKind::ScrollDown if !self.register_focus_query => {
                self.scroll_down();
            }
            MouseEventKind::ScrollUp if !self.register_focus_query => {
                self.scroll_up();
            }
            _ => {}
        }
    }

    fn on_left_click(&mut self, col: u16, row: u16) {
        if self.show_alerts {
            self.show_alerts = false;
            self.alert_dismissed = true;
            return;
        }

        if self.register_focus_query {
            return;
        }

        let tab_rects = self.geometry.tab_rects.clone();
        for (i, rect) in tab_rects.iter().enumerate() {
            if rect_contains(*rect, col, row) {
                self.select_tab(i);
                return;
            }
        }

        let range_rects = self.geometry.range_selector_rects.clone();
        for (i, rect) in range_rects.iter().enumerate() {
            if rect_contains(*rect, col, row) {
                self.select_range(i);
                return;
            }
        }

        if self.tab == Tab::Monthly && rect_contains(self.geometry.monthly_chart_area, col, row) {
            self.on_monthly_chart_click(col);
            return;
        }

        match self.tab {
            Tab::Monthly => {
                if rect_contains(self.geometry.income_table_area, col, row) {
                    if self.monthly_focus != MonthlyFocus::Income {
                        self.monthly_focus = MonthlyFocus::Income;
                    }
                    self.on_table_click(col, row);
                } else if rect_contains(self.geometry.expense_table_area, col, row) {
                    if self.monthly_focus != MonthlyFocus::Expenses {
                        self.monthly_focus = MonthlyFocus::Expenses;
                    }
                    self.on_table_click(col, row);
                }
            }
            Tab::Accounts => {
                if rect_contains(self.geometry.liability_table_area, col, row) {
                    if !self.liability_focus {
                        self.liability_focus = true;
                    }
                    self.on_table_click(col, row);
                } else if rect_contains(self.geometry.table_area, col, row) {
                    if self.liability_focus {
                        self.liability_focus = false;
                    }
                    self.on_table_click(col, row);
                }
            }
            Tab::Portfolio | Tab::Register => {
                if rect_contains(self.geometry.table_area, col, row) {
                    self.on_table_click(col, row);
                }
            }
        }
    }

    fn on_table_click(&mut self, _col: u16, row: u16) {
        let area = match self.tab {
            Tab::Monthly => match self.monthly_focus {
                MonthlyFocus::Income => self.geometry.income_table_area,
                MonthlyFocus::Expenses => self.geometry.expense_table_area,
            },
            Tab::Accounts => {
                if self.liability_focus {
                    self.geometry.liability_table_area
                } else {
                    self.geometry.table_area
                }
            }
            Tab::Portfolio | Tab::Register => self.geometry.table_area,
        };
        // border (1) + header row (1) + header bottom_margin (1) = 3 rows before data
        let content_y = area.y + 3;
        if row < content_y {
            return;
        }
        let clicked_idx = (row - content_y) as usize;
        match self.tab {
            Tab::Accounts => {
                if self.liability_focus {
                    let offset = self.liability_state.offset();
                    let abs = clicked_idx + offset;
                    if abs < self.liabilities.len() {
                        self.liability_state.select(Some(abs));
                    }
                } else {
                    let offset = self.account_state.offset();
                    let abs = clicked_idx + offset;
                    let filtered_len = self.filtered_accounts().len();
                    if abs < filtered_len {
                        self.account_state.select(Some(abs));
                    }
                }
            }
            Tab::Monthly => match self.monthly_focus {
                MonthlyFocus::Income => {
                    let offset = self.income_state.offset();
                    let abs = clicked_idx + offset;
                    let len = self.current_month().map(|m| m.income.len()).unwrap_or(0);
                    if abs < len {
                        self.income_state.select(Some(abs));
                    }
                }
                MonthlyFocus::Expenses => {
                    let offset = self.expense_state.offset();
                    let abs = clicked_idx + offset;
                    let len = self.current_month().map(|m| m.expenses.len()).unwrap_or(0);
                    if abs < len {
                        self.expense_state.select(Some(abs));
                    }
                }
            },
            Tab::Portfolio => {
                let abs = clicked_idx + self.portfolio_scroll_offset;
                if abs < self.holdings.len() {
                    self.selected_holding = abs;
                }
            }
            Tab::Register => {
                let offset = self.register_table.offset();
                let abs = clicked_idx + offset;
                if abs < self.register_rows.len() {
                    self.register_select_txn_at(abs);
                    self.register_focus_query = false;
                }
            }
        }
    }

    fn on_monthly_chart_click(&mut self, col: u16) {
        let area = self.geometry.monthly_chart_area;
        // inner x: border (1) + left padding (1) = +2
        let inner_x = area.x + 2;
        if col < inner_x {
            return;
        }
        // Each group spans `MONTHLY_GROUP_WIDTH` chars — shared with the
        // BarChart builder in `ui::monthly` so render and hit-test stay in
        // sync if the bar/group dimensions ever change.
        let slot = (col - inner_x) / crate::ui::MONTHLY_GROUP_WIDTH;
        if (slot as usize) < self.combined_months.len() {
            self.combined_selected = slot as usize;
        }
    }

    fn select_range(&mut self, idx: usize) {
        match self.tab {
            Tab::Portfolio => {
                let ranges = [
                    PortfolioRange::Month3,
                    PortfolioRange::Month6,
                    PortfolioRange::Ytd,
                    PortfolioRange::All,
                ];
                if let Some(&r) = ranges.get(idx) {
                    self.portfolio_range = r;
                }
            }
            Tab::Accounts => {
                let ranges = [
                    NetWorthRange::Ytd,
                    NetWorthRange::Year1,
                    NetWorthRange::Year2,
                    NetWorthRange::Year5,
                    NetWorthRange::All,
                ];
                if let Some(&r) = ranges.get(idx) {
                    self.nw_range = r;
                    self.reload_net_worth();
                }
            }
            Tab::Monthly | Tab::Register => {}
        }
    }
}

fn rect_contains(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}
