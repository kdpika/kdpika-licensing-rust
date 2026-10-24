//! Bounded worker-thread adapter. Blocking HTTP never runs/drops inside a Tokio runtime.
use crate::{ClientBuilder, Error, ErrorCode, LicenseResult};
use tokio::sync::{mpsc, oneshot};
use zeroize::Zeroizing;
enum Operation {
    Activate(Zeroizing<String>, Option<Zeroizing<String>>),
    Check,
    Validate,
    Online,
    Deactivate,
}
type Job = (Operation, oneshot::Sender<LicenseResult>);
/// Cloneable async handle to one serialized licensing worker. Enable feature `async`.
/// Dropping a future does NOT undo an already-started server operation. Do not blindly retry it.
#[derive(Clone)]
pub struct AsyncLicenseClient {
    sender: mpsc::Sender<Job>,
}
impl AsyncLicenseClient {
    pub async fn build(builder: ClientBuilder) -> Result<Self, Error> {
        let (tx, mut rx) = mpsc::channel::<Job>(16);
        let (ready_tx, ready_rx) = oneshot::channel();
        std::thread::Builder::new()
            .name("kdpika-licensing".into())
            .spawn(move || {
                let client = match builder.build() {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                if ready_tx.send(Ok(())).is_err() {
                    return;
                }
                while let Some((operation, reply)) = rx.blocking_recv() {
                    let result = match operation {
                        Operation::Activate(key, email) => {
                            client.activate(&key, email.as_ref().map(|s| s.as_str()))
                        }
                        Operation::Check => client.check(),
                        Operation::Validate => client.validate(),
                        Operation::Online => client.validate_online(),
                        Operation::Deactivate => client.deactivate(),
                    };
                    let _ = reply.send(result);
                }
            })
            .map_err(|_| Error(ErrorCode::Busy))?;
        ready_rx.await.map_err(|_| Error(ErrorCode::Busy))??;
        Ok(Self { sender: tx })
    }
    async fn run(&self, operation: Operation) -> Result<LicenseResult, Error> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send((operation, tx))
            .await
            .map_err(|_| Error(ErrorCode::Busy))?;
        rx.await.map_err(|_| Error(ErrorCode::Busy))
    }
    pub async fn activate(
        &self,
        key: impl Into<String>,
        email: Option<String>,
    ) -> Result<LicenseResult, Error> {
        self.run(Operation::Activate(
            Zeroizing::new(key.into()),
            email.map(Zeroizing::new),
        ))
        .await
    }
    pub async fn check(&self) -> Result<LicenseResult, Error> {
        self.run(Operation::Check).await
    }
    pub async fn validate(&self) -> Result<LicenseResult, Error> {
        self.run(Operation::Validate).await
    }
    pub async fn validate_online(&self) -> Result<LicenseResult, Error> {
        self.run(Operation::Online).await
    }
    pub async fn deactivate(&self) -> Result<LicenseResult, Error> {
        self.run(Operation::Deactivate).await
    }
}
