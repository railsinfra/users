use crate::config::Config;
use tonic::transport::Channel;
use tracing::warn;

pub mod proto {
    tonic::include_proto!("rails.accounts.v1");
}

use proto::accounts_service_client::AccountsServiceClient;
use proto::CreateDefaultAccountRequest;

#[derive(Clone)]
pub struct GrpcClients {
    pub(crate) accounts_client: Option<AccountsServiceClient<Channel>>,
}

impl GrpcClients {
    pub async fn create_default_account(
        &self,
        organization_id: uuid::Uuid,
        environment: &str,
        user_id: uuid::Uuid,
        admin_user_id: uuid::Uuid,
        currency: &str,
    ) -> Result<(uuid::Uuid, String), anyhow::Error> {
        let client = self
            .accounts_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Accounts gRPC client not initialized. Check that Accounts service is running and ACCOUNTS_GRPC_URL is correct."))?;

        let env_enum = match environment.to_lowercase().as_str() {
            "sandbox" => 1i32,  // Environment::Sandbox
            "production" => 2i32,  // Environment::Production
            _ => {
                warn!("Unknown environment '{}', defaulting to Sandbox", environment);
                1i32  // Environment::Sandbox
            }
        };

        let request = CreateDefaultAccountRequest {
            organization_id: organization_id.to_string(),
            environment: env_enum,
            user_id: user_id.to_string(),
            account_type: 1i32,  // CHECKING
            currency: currency.to_string(),
            admin_user_id: admin_user_id.to_string(),
        };

        let response = client
            .clone()
            .create_default_account(tonic::Request::new(request))
            .await?;

        let resp = response.into_inner();
        let account_id = uuid::Uuid::parse_str(&resp.account_id)?;

        Ok((account_id, resp.account_number))
    }
}

pub async fn init(config: &Config) -> Result<GrpcClients, tonic::transport::Error> {
    match AccountsServiceClient::connect(config.accounts_grpc_url.clone()).await {
        Ok(client) => {
            tracing::info!("Connected to Accounts gRPC service at {}", config.accounts_grpc_url);
            Ok(GrpcClients {
                accounts_client: Some(client),
            })
        }
        Err(e) => {
            tracing::warn!(
                "Failed to connect to Accounts gRPC service at {}: {}",
                config.accounts_grpc_url,
                e
            );
            tracing::warn!("Account creation will fail - ensure Accounts service is running");
            Ok(GrpcClients {
                accounts_client: None,
            })
        }
    }
}
