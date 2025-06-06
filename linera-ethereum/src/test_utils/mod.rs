// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use alloy::{
    network::{Ethereum, EthereumWallet},
    node_bindings::{Anvil, AnvilInstance},
    providers::{
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
            WalletFiller,
        },
        ProviderBuilder, RootProvider,
    },
    signers::local::PrivateKeySigner,
    sol,
};
use alloy_primitives::{Address, U256};
use linera_base::port::get_free_port;
use url::Url;

use crate::{
    client::EthereumQueries, common::EthereumServiceError, provider::EthereumClientSimplified,
};

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    SimpleTokenContract,
    "./contracts/SimpleToken.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    EventNumericsContract,
    "./contracts/EventNumerics.json"
);

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct EthereumClient {
    pub provider: FillProvider<
        JoinFill<
            JoinFill<
                alloy::providers::Identity,
                JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
            >,
            WalletFiller<EthereumWallet>,
        >,
        RootProvider<Ethereum>,
    >,
}

impl EthereumClient {
    /// Connects to an existing Ethereum node and creates an `EthereumClient`
    /// if successful.
    pub fn new(url: String) -> Result<Self, EthereumServiceError> {
        let rpc_url = Url::parse(&url)?;
        // this address is in the anvil test.
        let pk: PrivateKeySigner =
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                .parse()
                .unwrap();
        let wallet = EthereumWallet::from(pk);
        let provider = ProviderBuilder::new()
            .wallet(wallet.clone())
            .connect_http(rpc_url);
        let endpoint = Self { provider };
        Ok(endpoint)
    }
}

pub struct AnvilTest {
    pub anvil_instance: AnvilInstance,
    pub endpoint: String,
    pub ethereum_client: EthereumClient,
    pub rpc_url: Url,
}

pub async fn get_anvil() -> anyhow::Result<AnvilTest> {
    let port = get_free_port().await?;
    let anvil_instance = Anvil::new().port(port).try_spawn()?;
    let endpoint = anvil_instance.endpoint();
    let ethereum_client = EthereumClient::new(endpoint.clone())?;
    let rpc_url = Url::parse(&endpoint)?;
    Ok(AnvilTest {
        anvil_instance,
        endpoint,
        ethereum_client,
        rpc_url,
    })
}

impl AnvilTest {
    pub fn get_address(&self, index: usize) -> String {
        let address = self.anvil_instance.addresses()[index];
        format!("{:?}", address)
    }
}

pub struct SimpleTokenContractFunction {
    pub contract_address: String,
    pub anvil_test: AnvilTest,
}

impl SimpleTokenContractFunction {
    pub async fn new(anvil_test: AnvilTest) -> anyhow::Result<Self> {
        // 2: initializing the contract
        let initial_supply = U256::from(1000);
        let simple_token =
            SimpleTokenContract::deploy(&anvil_test.ethereum_client.provider, initial_supply)
                .await?;
        let contract_address = simple_token.address();
        let contract_address = format!("{:?}", contract_address);
        Ok(Self {
            contract_address,
            anvil_test,
        })
    }

    // Only the balanceOf operation is of interest for this contract
    pub async fn balance_of(&self, to: &str, block: u64) -> anyhow::Result<U256> {
        // Getting the simple_token
        let contract_address = self.contract_address.parse::<Address>()?;
        let simple_token = SimpleTokenContract::new(
            contract_address,
            self.anvil_test.ethereum_client.provider.clone(),
        );
        // Creating the calldata
        let to_address = to.parse::<Address>()?;
        let data = simple_token.balanceOf(to_address).calldata().clone();
        // Using the Ethereum client simplified.
        let ethereum_client_simp = EthereumClientSimplified::new(self.anvil_test.endpoint.clone());
        let answer = ethereum_client_simp
            .non_executive_call(&self.contract_address, data, to, block)
            .await?;
        // Converting the output
        let mut vec = [0_u8; 32];
        for (i, val) in vec.iter_mut().enumerate() {
            *val = answer.0[i];
        }
        let balance = U256::from_be_bytes(vec);
        Ok(balance)
    }

    pub async fn transfer(&self, from: &str, to: &str, value: U256) -> anyhow::Result<()> {
        // Getting the simple_token
        let contract_address = self.contract_address.parse::<Address>()?;
        let to_address = to.parse::<Address>()?;
        let from_address = from.parse::<Address>()?;
        let simple_token = SimpleTokenContract::new(
            contract_address,
            self.anvil_test.ethereum_client.provider.clone(),
        );
        // Doing the transfer
        let builder = simple_token.transfer(to_address, value).from(from_address);
        let _receipt = builder.send().await?.get_receipt().await?;
        Ok(())
    }
}

pub struct EventNumericsContractFunction {
    pub contract_address: String,
    pub anvil_test: AnvilTest,
}

impl EventNumericsContractFunction {
    pub async fn new(anvil_test: AnvilTest) -> anyhow::Result<Self> {
        // Deploying the event numerics contract
        let initial_supply = U256::from(0);
        let event_numerics =
            EventNumericsContract::deploy(&anvil_test.ethereum_client.provider, initial_supply)
                .await?;
        // Getting the contract address
        let contract_address = event_numerics.address();
        let contract_address = format!("{:?}", contract_address);
        Ok(Self {
            contract_address,
            anvil_test,
        })
    }
}
