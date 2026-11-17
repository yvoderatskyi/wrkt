use std::any::Any;
use std::thread::ScopedJoinHandle;

use anyhow::{Error, Result, anyhow};

pub fn joined<Outcome>(handle: ScopedJoinHandle<'_, Result<Outcome>>) -> Result<Outcome> {
    handle
        .join()
        .unwrap_or_else(|payload| Err(WorkerPanic(payload).into_error()))
}

struct WorkerPanic(Box<dyn Any + Send>);

impl WorkerPanic {
    fn into_error(self) -> Error {
        let Some(message) = self.message() else {
            return anyhow!("the worker thread panicked");
        };
        anyhow!("the worker thread panicked: {message}")
    }

    fn message(&self) -> Option<&str> {
        let text = self.0.downcast_ref::<&str>().copied();
        text.or_else(|| self.0.downcast_ref::<String>().map(String::as_str))
    }
}

#[cfg(test)]
mod worker_panic {
    use super::WorkerPanic;

    #[test]
    fn keeps_message_when_payload_is_str() {
        // Arrange
        let panic = WorkerPanic(Box::new("index out of bounds"));

        // Act
        let error = panic.into_error();

        // Assert
        assert_eq!(
            error.to_string(),
            "the worker thread panicked: index out of bounds"
        );
    }

    #[test]
    fn keeps_message_when_payload_is_string() {
        // Arrange
        let panic = WorkerPanic(Box::new(String::from("Repo 'api' vanished")));

        // Act
        let error = panic.into_error();

        // Assert
        assert_eq!(
            error.to_string(),
            "the worker thread panicked: Repo 'api' vanished"
        );
    }

    #[test]
    fn reports_generic_message_when_payload_is_not_text() {
        // Arrange
        let panic = WorkerPanic(Box::new(42_u8));

        // Act
        let error = panic.into_error();

        // Assert
        assert_eq!(error.to_string(), "the worker thread panicked");
    }
}
