use std::collections::{HashMap, HashSet};

use iced::widget::image::Handle;
use photos_domain::ImageId;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct GalleryConfig {
    pub thumbnail_size: u32,
    pub max_loaded_thumbnails: usize,
    pub visible_margin_rows: usize,
    pub prefetch_rows: usize,
    pub evict_distance_rows: usize,
}

impl Default for GalleryConfig {
    fn default() -> Self {
        Self {
            thumbnail_size: 128,
            max_loaded_thumbnails: 512,
            visible_margin_rows: 1,
            prefetch_rows: 2,
            evict_distance_rows: 8,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ThumbnailStatus {
    Unloaded,
    Queued {
        request_id: u64,
    },
    Loading {
        request_id: u64,
    },
    Loaded {
        handle: Handle,
        last_visible_tick: u64,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug)]
pub struct ThumbnailStore {
    states: HashMap<ImageId, ThumbnailStatus>,
    cancels: HashMap<ImageId, CancellationToken>,
    request_seq: u64,
    tick: u64,
}

impl ThumbnailStore {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            cancels: HashMap::new(),
            request_seq: 0,
            tick: 0,
        }
    }

    pub fn clear(&mut self) {
        for (_, token) in self.cancels.drain() {
            token.cancel();
        }
        self.states.clear();
        self.request_seq = 0;
        self.tick = 0;
    }

    pub fn status(&self, image_id: &ImageId) -> ThumbnailStatus {
        self.states
            .get(image_id)
            .cloned()
            .unwrap_or(ThumbnailStatus::Unloaded)
    }

    pub fn mark_visible_and_collect(
        &mut self,
        image_ids: &[ImageId],
        columns: usize,
        first_row: usize,
        last_row: usize,
        config: &GalleryConfig,
    ) -> ThumbnailActions {
        self.tick = self.tick.saturating_add(1);
        let tick = self.tick;

        let total_rows = if columns == 0 {
            0
        } else {
            image_ids.len().div_ceil(columns)
        };
        if total_rows == 0 {
            return ThumbnailActions::default();
        }

        let target_start = first_row.saturating_sub(config.visible_margin_rows);
        let target_end = (last_row + config.visible_margin_rows).min(total_rows.saturating_sub(1));
        let prefetch_start = target_start.saturating_sub(config.prefetch_rows);
        let prefetch_end = (target_end + config.prefetch_rows).min(total_rows.saturating_sub(1));

        let mut target_set = HashSet::new();
        let mut prefetch_set = HashSet::new();

        for row in target_start..=target_end {
            let start = row * columns;
            let end = ((row + 1) * columns).min(image_ids.len());
            for id in &image_ids[start..end] {
                target_set.insert(*id);
                prefetch_set.insert(*id);
            }
        }

        for row in prefetch_start..=prefetch_end {
            let start = row * columns;
            let end = ((row + 1) * columns).min(image_ids.len());
            for id in &image_ids[start..end] {
                prefetch_set.insert(*id);
            }
        }

        let mut actions = ThumbnailActions::default();

        for id in &target_set {
            if let Some(ThumbnailStatus::Loaded {
                handle,
                last_visible_tick: _,
            }) = self.states.get(id).cloned()
            {
                self.states.insert(
                    *id,
                    ThumbnailStatus::Loaded {
                        handle,
                        last_visible_tick: tick,
                    },
                );
            }
        }

        for id in &prefetch_set {
            let should_request = match self.states.get(id) {
                None | Some(ThumbnailStatus::Unloaded) | Some(ThumbnailStatus::Failed { .. }) => {
                    true
                }
                Some(ThumbnailStatus::Queued { .. })
                | Some(ThumbnailStatus::Loading { .. })
                | Some(ThumbnailStatus::Loaded { .. }) => false,
            };

            if should_request {
                self.request_seq = self.request_seq.saturating_add(1);
                let request_id = self.request_seq;
                let cancel = CancellationToken::new();
                self.cancels.insert(*id, cancel.clone());
                self.states
                    .insert(*id, ThumbnailStatus::Queued { request_id });
                actions.requests.push(ThumbnailRequest {
                    image_id: *id,
                    request_id,
                    cancel,
                });
            }
        }

        for (idx, id) in image_ids.iter().enumerate() {
            let row = idx / columns.max(1);
            if let Some(token) = self.cancels.get(id)
                && !prefetch_set.contains(id)
                && row + config.evict_distance_rows < first_row
                && !token.is_cancelled()
            {
                token.cancel();
            }
        }

        self.evict(
            image_ids,
            columns,
            first_row,
            last_row,
            config,
            &mut actions,
        );
        actions
    }

    pub fn mark_loading(&mut self, image_id: ImageId, request_id: u64) {
        if matches!(
            self.states.get(&image_id),
            Some(ThumbnailStatus::Queued { request_id: rid }) if *rid == request_id
        ) {
            self.states
                .insert(image_id, ThumbnailStatus::Loading { request_id });
        }
    }

    pub fn finish_loaded(&mut self, image_id: ImageId, request_id: u64, handle: Handle) {
        let should_set = match self.states.get(&image_id) {
            Some(ThumbnailStatus::Queued { request_id: rid })
            | Some(ThumbnailStatus::Loading { request_id: rid }) => *rid == request_id,
            _ => false,
        };

        if should_set {
            self.cancels.remove(&image_id);
            self.states.insert(
                image_id,
                ThumbnailStatus::Loaded {
                    handle,
                    last_visible_tick: self.tick,
                },
            );
        }
    }

    pub fn finish_failed(&mut self, image_id: ImageId, request_id: u64, message: String) {
        let should_set = match self.states.get(&image_id) {
            Some(ThumbnailStatus::Queued { request_id: rid })
            | Some(ThumbnailStatus::Loading { request_id: rid }) => *rid == request_id,
            _ => false,
        };

        if should_set {
            self.cancels.remove(&image_id);
            self.states
                .insert(image_id, ThumbnailStatus::Failed { message });
        }
    }

    fn evict(
        &mut self,
        image_ids: &[ImageId],
        columns: usize,
        first_row: usize,
        last_row: usize,
        config: &GalleryConfig,
        actions: &mut ThumbnailActions,
    ) {
        let mut loaded: Vec<(ImageId, usize, u64)> = image_ids
            .iter()
            .enumerate()
            .filter_map(|(idx, id)| {
                if let Some(ThumbnailStatus::Loaded {
                    handle: _,
                    last_visible_tick,
                }) = self.states.get(id)
                {
                    Some((*id, idx / columns.max(1), *last_visible_tick))
                } else {
                    None
                }
            })
            .collect();

        let far_limit = last_row + config.evict_distance_rows;
        let mut evict_ids = Vec::new();

        for (id, row, _) in &loaded {
            if *row + config.evict_distance_rows < first_row || *row > far_limit {
                evict_ids.push(*id);
            }
        }

        for id in evict_ids {
            self.states.insert(id, ThumbnailStatus::Unloaded);
            actions.evicted.push(id);
        }

        loaded.retain(|(id, _, _)| {
            matches!(self.states.get(id), Some(ThumbnailStatus::Loaded { .. }))
        });

        if loaded.len() > config.max_loaded_thumbnails {
            loaded.sort_by_key(|(_, _, tick)| *tick);
            let overflow = loaded.len() - config.max_loaded_thumbnails;
            for (id, _, _) in loaded.into_iter().take(overflow) {
                self.states.insert(id, ThumbnailStatus::Unloaded);
                actions.evicted.push(id);
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct ThumbnailActions {
    pub requests: Vec<ThumbnailRequest>,
    pub evicted: Vec<ImageId>,
}

#[derive(Debug, Clone)]
pub struct ThumbnailRequest {
    pub image_id: ImageId,
    pub request_id: u64,
    pub cancel: CancellationToken,
}
