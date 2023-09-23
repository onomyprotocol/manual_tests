use bip32::XPrv;
use bip39::Mnemonic;
use deep_space::PrivateKey;
use onomy_test_lib::super_orchestrator::stacked_errors::{Result, StackableErr};
use sha2::Digest;

pub fn get_private_key(mnemonic: &str) -> Result<PrivateKey> {
    // for some reason, `deep_space::PrivateKey::from_phrase` or any of the
    // alternate ways is not working
    let mnemonic = Mnemonic::parse_in_normalized(bip39::Language::English, mnemonic).stack()?;
    let seed = mnemonic.to_seed("");
    let xprv = XPrv::derive_from_path(seed, &"m/44'/118'/0'/0/0".parse().unwrap()).unwrap();
    let xpub = xprv.public_key();
    //let xpub_str = xpub.to_string(bip32::Prefix::ZPUB);
    let extended_pub = xpub.to_extended_key(bip32::Prefix::ZPUB);
    let bytes = extended_pub.key_bytes;
    let mut hasher = sha2::Sha256::new();
    hasher.update(&bytes);
    let result = hasher.finalize();
    let mut hasher = ripemd::Ripemd160::new();
    hasher.update(result);
    //let result = hasher.finalize();
    //let data = bech32::ToBase32::to_base32(&result);
    // note: if adapting for Bitcoin you have to prepend a single zero byte here
    //let encoded = bech32::encode(bech32_prefix, data,
    // bech32::Variant::Bech32).unwrap();
    let private_key = PrivateKey::from_array(xprv.to_bytes());
    Ok(private_key)
}
