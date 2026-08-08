mod app;
mod cache;
mod fullscreen;
mod gallery;
mod library_dialog;
mod message;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // tracing::info!("Starting Photos UI");

    iced::application(app::App::new, app::App::update, app::App::view)
        .subscription(app::App::subscription)
        .title(app::App::title)
        .window_size((1200.0, 800.0))
        .run()
}
