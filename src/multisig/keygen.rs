use super::service::MultisigService;
use crate::{proto::KeygenRequest, TofndResult};
use tofn::{ecdsa::keygen, sdk::api::serialize};

use anyhow::anyhow;

impl MultisigService {
    pub(super) async fn handle_keygen(&self, request: &KeygenRequest) -> TofndResult<Vec<u8>> {
        let session_nonce = request.key_uid.clone();
        let secret_recovery_key = self.kv_manager.seed().await?;

        // generate signing key and verifying key
        let key_pair = keygen(&secret_recovery_key, session_nonce.as_bytes())
            .map_err(|_| anyhow!("Cannot generate keypair"))?;

        // make reservation for signing key
        let reservation = self.kv_manager.kv().reserve_key(session_nonce).await?;

        // serialize signing key
        // SecretScalar is not exposed, so we need to serialize manually here
        let signing_key_bytes = serialize(key_pair.signing_key())
            .map_err(|_| anyhow!("Cannot serialize signing key"))?;

        // put signing key into kv store
        self.kv_manager
            .kv()
            .put(reservation, signing_key_bytes)
            .await?;

        // return verifying key
        Ok(key_pair.encoded_verifying_key().to_vec())
    }
}