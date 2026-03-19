use crate::config::Settings;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt::{self, writer::MakeWriterExt},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

pub fn init_logging(settings: &Settings) -> Result<(), Box<dyn std::error::Error>> {
    // 创建日志目录
    std::fs::create_dir_all(settings.logs_path())?;

    // 文件日志
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        settings.logs_path(),
        "agent-automation.log",
    );

    // 控制台日志
    let (console_non_blocking, _guard) = tracing_appender::non_blocking(std::io::stdout());

    // 环境过滤器
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&settings.logging.level));

    // 组合层
    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_writer(file_appender.and(console_non_blocking))
                .with_target(true)
                .with_thread_ids(true)
        )
        .init();

    Ok(())
}
