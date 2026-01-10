use eframe::egui;

/// dynamic grid
pub struct DynamicGrid<ItemId, ItemData> {
    pub n_columns: usize,
    desired_item_size: f32,
    _marker: std::marker::PhantomData<(ItemId, ItemData)>,
}

impl<ItemId: Copy + std::hash::Hash + Eq, ItemData> DynamicGrid<ItemId, ItemData> {
    pub fn new(desired_item_size: f32) -> Self {
        Self {
            n_columns: 2,
            desired_item_size,
            _marker: std::marker::PhantomData,
        }
    }

    /// Shows the grid
    ///
    /// - `ui` / `ctx` → egui context
    /// - `items` → slice of item ids
    /// - `get_item_data` → closure to fetch item data
    /// - `render_item` → closure to render an item given texture
    /// - `on_item_clicked` → closure called when item is clicked
    pub fn show<FGet, FRender, FClick>(
        &mut self,
        ui: &mut egui::Ui,
        _ctx: &egui::Context,
        items: &[ItemId],
        mut get_item_data: FGet,
        mut render_item: FRender,
        mut on_item_clicked: FClick,
        stick_to_bottom: bool,
    ) where
        FGet: FnMut(&ItemId) -> Option<ItemData>,
        FRender: FnMut(
            &mut egui::Ui,
            bool, // visible
            (f32, f32),
            Option<ItemData>,
            &mut dyn FnMut(),
        ),
        FClick: FnMut(usize),
    {
        let available_width = ui.available_width();
        let spacing = ui.style().spacing.item_spacing.x;

        self.n_columns = ((available_width + spacing) / (self.desired_item_size + spacing))
            .floor()
            .max(1.0) as usize;

        let actual_size =
            ((available_width + spacing) / self.n_columns as f32 - spacing).clamp(50.0, 500.0);

        let item_height = actual_size;
        let total_rows = items.len().div_ceil(self.n_columns);

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(stick_to_bottom)
            .show(ui, |ui| {
                let clip_rect = ui.clip_rect();
                let scroll_y = ui.min_rect().top();
                let visible_start = ((clip_rect.top() - scroll_y)
                    / (item_height + ui.style().spacing.item_spacing.y))
                    .floor() as isize;
                let visible_end = ((clip_rect.bottom() - scroll_y)
                    / (item_height + ui.style().spacing.item_spacing.y))
                    .ceil() as isize;

                let margin = 1;
                let start_row = (visible_start - margin).max(0) as usize;
                let end_row = ((visible_end + margin) as usize).min(total_rows);

                let mut start_index = start_row * self.n_columns;
                let mut end_index = (end_row * self.n_columns).min(items.len());

                if start_index > end_index {
                    std::mem::swap(&mut start_index, &mut end_index);
                }

                let mut i = 0;
                for chunk in items.chunks(self.n_columns) {
                    ui.horizontal(|ui| {
                        for item_id in chunk {
                            let is_visible = start_index <= i && i <= end_index;
                            let mut click_callback = || on_item_clicked(i);
                            let data = if is_visible {
                                get_item_data(item_id)
                            } else {
                                None
                            };
                            render_item(
                                ui,
                                is_visible,
                                (actual_size, actual_size),
                                data,
                                &mut click_callback,
                            );
                            i += 1;
                        }
                    });
                }
            });
    }
}
