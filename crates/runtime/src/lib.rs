use std::{error::Error, future::Future, io, time::Duration};

use ferrum_config::LogLevel;
use thiserror::Error;
use tokio::signal::unix::{Signal, SignalKind, signal};
use tokio_util::sync::CancellationToken;
use tracing::Level;

pub fn init_tracing(level: LogLevel) -> Result<(), Box<dyn Error + Send + Sync>> {
    let level = match level {
        LogLevel::Error => Level::ERROR,
        LogLevel::Warn => Level::WARN,
        LogLevel::Info => Level::INFO,
        LogLevel::Debug => Level::DEBUG,
        LogLevel::Trace => Level::TRACE,
    };
    tracing_subscriber::fmt()
        .json()
        .with_max_level(level)
        .with_writer(io::stderr)
        .try_init()
}

pub struct Shutdown {
    interrupt: Signal,
    terminate: Signal,
    cancelled: CancellationToken,
}

#[derive(Debug, Error)]
pub enum ServiceError<E> {
    #[error("service failed: {0}")]
    Service(E),
    #[error("graceful shutdown exceeded the configured timeout")]
    ShutdownTimeout,
}

impl Shutdown {
    pub fn install() -> io::Result<Self> {
        Ok(Self {
            interrupt: signal(SignalKind::interrupt())?,
            terminate: signal(SignalKind::terminate())?,
            cancelled: CancellationToken::new(),
        })
    }

    pub fn token(&self) -> CancellationToken {
        self.cancelled.clone()
    }

    pub async fn run<E>(
        mut self,
        timeout: Duration,
        service: impl Future<Output = Result<(), E>>,
    ) -> Result<(), ServiceError<E>> {
        tokio::pin!(service);
        let signal = tokio::select! {
            biased;
            _ = self.interrupt.recv() => "SIGINT",
            _ = self.terminate.recv() => "SIGTERM",
            result = &mut service => return result.map_err(ServiceError::Service),
        };
        self.cancelled.cancel();
        tracing::info!(event = "shutdown_started", signal);
        drain(timeout, service).await
    }
}

async fn drain<E>(
    timeout: Duration,
    service: impl Future<Output = Result<(), E>>,
) -> Result<(), ServiceError<E>> {
    match tokio::time::timeout(timeout, service).await {
        Ok(result) => result.map_err(ServiceError::Service),
        Err(_) => Err(ServiceError::ShutdownTimeout),
    }
}

#[cfg(test)]
mod tests {
    use std::{convert::Infallible, future};

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn unfinished_service_hits_shutdown_deadline() {
        let result = drain(
            Duration::from_millis(500),
            future::pending::<Result<(), Infallible>>(),
        )
        .await;
        assert!(matches!(result, Err(ServiceError::ShutdownTimeout)));
    }
}
