use cosmos_sdk_proto::cosmos::bank::v1beta1::MsgSend;
use deep_space::{u256, Coin, Fee, MessageArgs, Msg, PrivateKey};
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

pub fn precheck_all_batches(records: &[Record]) -> Result<()> {
    //let mnemonic = MNEMONIC;
    //let mnemonic = mnemonic.trim();
    //let private_key = PrivateKey::from_hd_wallet_path("m/84'/0'/0'/0/0",
    // mnemonic, "").stack()?;
    let private_key = PrivateKey::from_secret(&[9u8; 32]);
    let _ = get_tx_batches("test-chain-id", private_key, records).stack()?;
    Ok(())
}

pub fn get_tx_batches(
    chain_id: &str,
    private_key: PrivateKey,
    records: &[Record],
) -> Result<Vec<Vec<u8>>> {
    let public_key = private_key.to_public_key("cosmospub").stack()?;
    let from_address = public_key.to_address();

    const BATCH_SIZE: usize = 100;
    let mut batches = vec![];

    let mut i = 0;
    loop {
        if i >= records.len() {
            break
        }

        let mut msgs = vec![];

        loop {
            if (msgs.len() >= BATCH_SIZE) || (i >= records.len()) {
                break
            }

            let record = &records[i];

            let coins = vec![Coin {
                denom: "anom".to_string(),
                amount: u256!(1),
            }
            .into()];
            let send = MsgSend {
                amount: coins,
                from_address: from_address.to_string(),
                to_address: record.addr.clone(),
            };
            let msg = Msg::new("/cosmos.bank.v1beta1.MsgSend", send);
            msgs.push(msg);

            i += 1;
        }

        let fee = Fee {
            amount: vec![Coin {
                denom: "anom".to_string(),
                amount: u256!(1_000_000),
            }],
            gas_limit: 1_000_000,
            granter: None,
            payer: None,
        };
        let args = MessageArgs {
            sequence: 0,
            account_number: 0,
            chain_id: chain_id.to_string(),
            fee,
            timeout_height: 100,
        };

        //let tx = private_key.get_signed_tx(&msgs, args, "").stack()?;
        let tx = private_key.sign_std_msg(&msgs, args, "").stack()?;

        batches.push(tx);
    }

    Ok(batches)
}
