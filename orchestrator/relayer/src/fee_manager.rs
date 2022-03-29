use ethereum_gravity::{
    utils::GasCost,
};
use ethers::prelude::*;
use ethers::types::Address as EthAddress;
use gravity_utils::ethereum::{format_eth_address};
use gravity_utils::types::{ Erc20Token, };
use std::collections::HashMap;
use std::time::Duration;
use std::time::Instant;
use gravity_utils::types::config::RelayerMode;
use reqwest::Error;

pub struct FeeManager {
    token_price_map: HashMap<String, String>,
    token_api_url_map: HashMap<String, String>,
    next_batch_send_time: HashMap<EthAddress, Instant>,
    mode: RelayerMode,
}

#[derive(serde::Deserialize, Debug)]
struct ApiResponse {
    status: String,
    result: ApiResult,
}

#[derive(serde::Deserialize, Debug)]
struct ApiResult {
    tokenPairDecimal: u32,
    aggregatedPrice: f64,
}

impl FeeManager {
    pub async fn new_fee_manager(mode: RelayerMode) -> FeeManager {
        let mut fm =  Self {
            token_price_map: Default::default(),
            token_api_url_map: Default::default(),
            next_batch_send_time: HashMap::new(),
            mode
        };

        let success = fm.init()
            .await;

        if !success {
            panic!("Could not initialize fee manager");
        }
        return fm;
    }

    async fn init(&mut self) -> bool {
        match self.mode {
            RelayerMode::Api => {
                let token_addresses_path =
                    std::env::var("TOKEN_ADDRESSES_JSON").unwrap_or_else(|_| "token_addresses.json".to_owned());

                let token_addresses_str = match tokio::fs::read_to_string(token_addresses_path).await {
                    Err(err) => {
                        error!("Error while fetching token pair addresses {}", err);
                        return false;
                    }
                    Ok(value) => value,
                };

                let token_addresses: HashMap<String, String> = match serde_json::from_str(&token_addresses_str)
                {
                    Err(err) => {
                        error!("Error while parsing token pair addresses json configuration: {}", err);
                        return false;
                    }
                    Ok(token_addresses) => token_addresses,
                };

                let api_url =
                    std::env::var("TOKEN_API_URL").unwrap_or_else(|_| "https://cronos.org/gravity-testnet2/api/v1/oracle/quotes".to_owned());

                for (key, value) in token_addresses.into_iter() {
                    if value == "ETH" {
                        self.token_api_url_map.insert(key, String::from("ETH"));
                    } else {
                        let request_url = format!("{url}/{pair}",
                                                  url = api_url,
                                                  pair = value);

                        self.token_api_url_map.insert(key, request_url);
                    }
                }

                return true;
            }
            RelayerMode::File => {
                let config_file_path =
                    std::env::var("TOKEN_PRICES_JSON").unwrap_or_else(|_| "token_prices.json".to_owned());

                let config_str = match tokio::fs::read_to_string(config_file_path).await {
                    Err(err) => {
                        error!("Error while fetching token prices {}", err);
                        return false;
                    }
                    Ok(value) => value,
                };

                let config: HashMap<String, String> = match serde_json::from_str(&config_str)
                {
                    Err(err) => {
                        error!("Error while parsing token prices json configuration: {}", err);
                        return false;
                    }
                    Ok(config) => config,
                };

                self.token_price_map = config;
                return true;
            }
            _ => {
                return false;
            }
        }
    }

    // A batch can be send either if
    // - Mode is AlwaysRelay
    // - Mode is either API or File and the batch has a profitable cost
    // - Mode is either API or File and the batch has been waiting to be sent more than GRAVITY_BATCH_SENDING_SECS secs
    pub async fn can_send_batch(
        &mut self,
        estimated_cost: &GasCost,
        batch_fee: &Erc20Token,
        contract_address: &EthAddress,
    ) -> bool {
        if self.mode == RelayerMode::AlwaysRelay {
            return true;
        }

        match self.next_batch_send_time.get(contract_address) {
            Some(time) => {
                if *time < Instant::now() {
                    return true;
                }
            }
            None => self.update_next_batch_send_time(*contract_address),
        }

        let token_price = match self.get_token_price(&batch_fee.token_contract_address).await {
            Ok(token_price) => token_price,
            Err(_) => return false,
        };

        let estimated_fee = estimated_cost.get_total();
        let batch_value = batch_fee.amount.clone() * token_price;

        info!("estimate cost is {}, batch value is {}", estimated_fee, batch_value);
        batch_value >= estimated_fee
    }

    pub fn update_next_batch_send_time(
        &mut self,
        contract_address: EthAddress,
    ) {
        if self.mode == RelayerMode::AlwaysRelay {
            return;
        }

        let timeout_duration = std::env::var("GRAVITY_BATCH_SENDING_SECS")
            .map(|value| Duration::from_secs(value.parse().unwrap()))
            .unwrap_or_else(|_| Duration::from_secs(3600));

        self.next_batch_send_time.insert(contract_address, Instant::now() + timeout_duration);
    }

    async fn get_token_price(&mut self, contract_address: &EthAddress) -> Result<U256, ()> {
        match self.mode {
            RelayerMode::Api => {
                if self.token_api_url_map.contains_key(&*format_eth_address(*contract_address)) {
                    let api_url = self.token_api_url_map
                        .get(&*format_eth_address(*contract_address));

                    if api_url.is_none() {
                        log::error!("Api url expected to be string format");
                        return Err(());
                    }
                    let request_url = api_url.unwrap();

                    // Return because gas price is quoted in ETH
                    if request_url == "ETH" {
                        return Ok(U256::from(1))
                    }

                    let response = reqwest::get(request_url).await.expect("Cannot parse response from oracle");
                    let result : ApiResponse = response.json().await.expect("Cannot parse result from oracle");
                    // TODO to be updated with new oracle API
                    let token_price = result.result.aggregatedPrice * (i32::pow(10, result.result.tokenPairDecimal) as f64);
                    let token_price_u64=  U256::from_dec_str(&token_price.to_string()).expect("Cannot convert token price");
                    return Ok(token_price_u64);
                } else {
                    log::error!("contract address cannot be found in token pair");
                    return Err(());
                }
            }
            RelayerMode::File => {
                let token_price = self.token_price_map.get(&format_eth_address(*contract_address));

                if token_price.is_none() {
                    info!("Cannot find token price in map");
                    return Err(());
                }
                let token_price_str = token_price.unwrap();
                let token_price = U256::from_dec_str(token_price_str);

                if token_price.is_err() {
                    log::error!("Unable to parse token price");
                }
                return token_price.map_err(|_| ())

            }
            _ => { return Err(());}
        }
    }
}