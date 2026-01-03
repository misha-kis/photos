use photos_workflow::errors::StepError;
use photos_workflow::{Step, StepContext, WorkflowEvent};
use std::path::PathBuf;

pub struct RegisterImagesStep {
    pub image_paths: Vec<PathBuf>,
}

#[async_trait::async_trait]
impl Step for RegisterImagesStep {
    async fn execute(&self, ctx: &StepContext) -> Result<(), StepError> {
        let mut image_records = Vec::new();
        let total_images = self.image_paths.len() as u64;
        tracing::info!("importing {total_images} images");

        for (processed_images, path) in self.image_paths.iter().enumerate() {
            if ctx.cancel.is_cancelled() {
                tracing::warn!("import workflow cancelled");
                return Err(StepError::Cancelled);
            }

            let services = ctx.services.clone();
            let path = path.clone();
            let image_record =
                tokio::task::spawn_blocking(move || services.image_repo().insert_image(&path))
                    .await
                    .map_err(|e| StepError::Failed(format!("spawn_blocking failed: {}", e)))?
                    .map_err(|e| StepError::Failed(e.to_string()))?;

            image_records.push(image_record);

            ctx.progress_reporter
                .send(WorkflowEvent::StepProgress {
                    job_id: ctx.job_id,
                    step: self.name(),
                    current: processed_images as u64 + 1,
                    total: total_images,
                })
                .await;
        }

        if ctx.cancel.is_cancelled() {
            return Err(StepError::Cancelled);
        }

        ctx.services
            .image_meta_repo()
            .add_image_record_bulk(&image_records)
            .await
            .map_err(|e| StepError::Failed(e.to_string()))?;

        Ok(())
    }

    fn name(&self) -> &'static str {
        "RegisterImagesStep"
    }
}
