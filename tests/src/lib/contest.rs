use cosmos_sdk_proto::cosmos::bank::v1beta1::MsgSend;
use deep_space::{u256, Coin, Msg, PrivateKey};
use onomy_test_lib::{
    reprefix_bech32,
    super_orchestrator::stacked_errors::{Result, StackableErr},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawRecord {
    // could be parsed with `chrono` if needed
    pub timestamp: String,
    pub discord_user: String,
    pub addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub timestamp: String,
    pub discord_user: String,
    pub addr: String,
}

impl Record {
    pub fn from_raw_record(raw: &RawRecord) -> Option<Self> {
        use bech32::Variant;
        match bech32::decode(&raw.addr) {
            Ok((_, data, variant)) => {
                if (variant == Variant::Bech32) && (data.len() == 32) {
                    // reprefix for some people
                    let reprefixed = reprefix_bech32(&raw.addr, "onomy").unwrap();
                    return Some(Self {
                        timestamp: raw.timestamp.clone(),
                        discord_user: raw.discord_user.clone(),
                        addr: reprefixed,
                    })
                }
                //dbg!(prefix, data.len(), variant);
            }
            Err(_) => {
                //dbg!(&raw.addr);
            }
        }
        None
    }

    pub fn verify(&self, prefix: &str) -> Result<()> {
        use bech32::Variant;
        let (prefix1, data, variant) = bech32::decode(&self.addr).stack()?;
        assert_eq!(prefix1, prefix);
        assert_eq!(variant, Variant::Bech32);
        assert_eq!(data.len(), 32);
        Ok(())
    }
}

pub fn get_txs(private_key: PrivateKey, records: &[Record]) -> Result<Vec<Msg>> {
    let from_address = private_key
        .to_address("onomy")
        .stack()?
        .to_bech32("onomy")
        .stack()?;

    let mut msgs = vec![];

    for record in records {
        /*
        allotment:
        500 BTC
        10000 NOM
        2M USDC
        2M USDT
        1500 ETH
            */
        let coins = vec![
            Coin {
                denom: "abtc".to_string(),
                amount: u256!(500_000000000000000000),
            }
            .into(),
            Coin {
                denom: "anom".to_string(),
                amount: u256!(10000_000000000000000000),
            }
            .into(),
            Coin {
                denom: "ausdc".to_string(),
                amount: u256!(2000000_000000000000000000),
            }
            .into(),
            Coin {
                denom: "ausdt".to_string(),
                amount: u256!(2000000_000000000000000000),
            }
            .into(),
            Coin {
                denom: "wei".to_string(),
                amount: u256!(1500_000000000000000000),
            }
            .into(),
        ];
        let send = MsgSend {
            amount: coins,
            from_address: from_address.to_string(),
            to_address: record.addr.clone(),
        };
        let msg = Msg::new("/cosmos.bank.v1beta1.MsgSend", send);
        msgs.push(msg);
    }

    Ok(msgs)
}
