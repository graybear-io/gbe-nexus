use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio_util::sync::CancellationToken;

use gbe_transport::TransportError;

pub(crate) struct RedisSubscription {
    pub(crate) token: CancellationToken,
    pub(crate) active: Arc<AtomicBool>,
}

#[async_trait]
impl gbe_transport::Subscription for RedisSubscription {
    async fn unsubscribe(&self) -> Result<(), TransportError> {
        self.token.cancel();
        self.active.store(false, Ordering::Release);
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}
