use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio_util::sync::CancellationToken;

use gbe_nexus::TransportError;

pub(crate) struct MemorySubscription {
    pub(crate) token: CancellationToken,
    pub(crate) active: Arc<AtomicBool>,
}

#[async_trait]
impl gbe_nexus::Subscription for MemorySubscription {
    async fn unsubscribe(&self) -> Result<(), TransportError> {
        self.token.cancel();
        self.active.store(false, Ordering::Release);
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}
