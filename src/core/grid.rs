#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self { x, y, width, height }
    }
}

/// Layout areas calculated for rendering dashboard components.
#[derive(Debug, Clone, Copy, Default)]
pub struct DashboardLayout {
    pub header: Rect,
    pub cpu_panel: Rect,
    pub mem_panel: Rect,
    pub net_panel: Rect,
    pub filter_bar: Rect,
    pub process_table: Rect,
    pub footer: Rect,
}

pub struct LayoutEngine;

impl LayoutEngine {
    /// Computes panel coordinates based on total available terminal bounds.
    pub fn compute(screen: Rect, show_filter_bar: bool) -> DashboardLayout {
        // Fallback for extremely small terminal viewports
        if screen.width < 20 || screen.height < 6 {
            return DashboardLayout {
                header: screen,
                ..Default::default()
            };
        }

        let header_height = 3u16;
        let footer_height = 1u16;
        let metrics_height = 8u16.min(screen.height / 3);

        let header = Rect::new(screen.x, screen.y, screen.width, header_height);
        
        let body_y = screen.y + header_height;
        let body_height = screen.height.saturating_sub(header_height + footer_height);

        // Calculate top metrics panel layout (CPU | Memory | Network)
        let metrics_y = body_y;
        let col_width = screen.width / 3;
        let rem_width = screen.width % 3;

        let cpu_panel = Rect::new(screen.x, metrics_y, col_width, metrics_height);
        let mem_panel = Rect::new(screen.x + col_width, metrics_y, col_width, metrics_height);
        let net_panel = Rect::new(
            screen.x + (col_width * 2),
            metrics_y,
            col_width + rem_width,
            metrics_height,
        );

        // Calculate process list & filter bar layout
        let main_content_y = body_y + metrics_height;
        let main_content_height = body_height.saturating_sub(metrics_height);

        let (filter_bar, process_table) = if show_filter_bar {
            let filter_h = 3u16.min(main_content_height);
            let proc_h = main_content_height.saturating_sub(filter_h);

            let filter_rect = Rect::new(screen.x, main_content_y, screen.width, filter_h);
            let proc_rect = Rect::new(screen.x, main_content_y + filter_h, screen.width, proc_h);

            (filter_rect, proc_rect)
        } else {
            (
                Rect::default(),
                Rect::new(screen.x, main_content_y, screen.width, main_content_height),
            )
        };

        let footer = Rect::new(
            screen.x,
            screen.y + screen.height - footer_height,
            screen.width,
            footer_height,
        );

        DashboardLayout {
            header,
            cpu_panel,
            mem_panel,
            net_panel,
            filter_bar,
            process_table,
            footer,
        }
    }

    /// Calculates row scroll offsets for navigating the filtered process table.
    pub fn calculate_scroll(
        selected_index: usize,
        total_items: usize,
        visible_height: usize,
    ) -> (usize, usize) {
        if total_items == 0 || visible_height == 0 {
            return (0, 0);
        }

        let scroll_offset = if selected_index >= visible_height {
            selected_index - visible_height + 1
        } else {
            0
        };

        let visible_end = (scroll_offset + visible_height).min(total_items);
        (scroll_offset, visible_end)
    }
}