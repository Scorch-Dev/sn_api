// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under the MIT license <LICENSE-MIT
// http://opensource.org/licenses/MIT> or the Modified BSD license <LICENSE-BSD
// https://opensource.org/licenses/BSD-3-Clause>, at your option. This file may not be copied,
// modified, or distributed except according to those terms. Please review the Licences for the
// specific language governing permissions and limitations relating to use of the SAFE Network
// Software.

use super::{
    helpers::{get_from_arg_or_stdin, get_secret_key, serialise_output},
    keys::{create_new_key, print_new_key_output},
    OutputFmt,
};
use anyhow::{anyhow, Result};
use log::debug;
use sn_api::{ed_sk_from_hex, sk_to_hex, Keypair, Safe, SecretKey};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub enum WalletSubCommands {
    #[structopt(name = "insert")]
    /// Insert a spendable balance into a Wallet
    Insert {
        /// The target Wallet to insert the spendable balance
        target: String,
        /// The secret key of a SafeKey for paying the operation costs. If not provided, the application's default wallet will be used
        #[structopt(short = "w", long = "pay-with")]
        pay_with: Option<String>,
        /// Pass the secret key needed to make the balance spendable, it will be prompted if not provided
        #[structopt(long = "sk")]
        secret_key: Option<String>,
        /// The name to give this spendable balance
        #[structopt(long = "name")]
        name: Option<String>,
        /// The SafeKey's safe://xor-url to verify it matches/corresponds to the secret key provided. The corresponding secret key will be prompted if not provided with '--sk'
        #[structopt(long = "keyurl")]
        keyurl: Option<String>,
        /// Set the inserted SafeKey as the default one in the target Wallet
        #[structopt(long = "default")]
        default: bool,
    },
    #[structopt(name = "balance")]
    /// Query a Wallet's total balance
    Balance {
        /// The target Wallet to check the total balance
        target: Option<String>,
    },
    #[structopt(name = "create")]
    /// Create a new Wallet
    Create {
        /// The secret key of a SafeKey for paying the operation costs
        #[structopt(short = "w", long = "pay-with")]
        pay_with: Option<String>,
        /// If true, do not create a spendable balance
        #[structopt(long = "no-balance")]
        no_balance: bool,
        /// The name to give the spendable balance
        #[structopt(long = "name")]
        name: Option<String>,
        /// An existing SafeKey's safe://xor-url. If this is not supplied, a new SafeKey will be automatically generated and inserted. The corresponding secret key will be prompted if not provided with '--sk'
        #[structopt(long = "keyurl")]
        keyurl: Option<String>,
        /// Pass the secret key needed to make the balance spendable, it will be prompted if not provided
        #[structopt(long = "sk")]
        secret_key: Option<String>,
        /// Create a SafeKey, allocate test-coins onto it, and add the SafeKey to the Wallet
        #[structopt(long = "test-coins")]
        test_coins: bool,
        /// Preload with a balance
        #[structopt(long = "preload")]
        preload: Option<String>,
    },
    #[structopt(name = "transfer")]
    /// Transfer safecoins from one Wallet to another, or to a SafeKey
    Transfer {
        /// Number of safecoins to transfer
        amount: String,
        /// Source Wallet URL
        #[structopt(long = "from")]
        from: String,
        /// The receiving Wallet/SafeKey URL or public key, otherwise pulled from stdin if not provided
        #[structopt(long = "to")]
        to: Option<String>,
    },
}

pub async fn wallet_commander(
    cmd: WalletSubCommands,
    output_fmt: OutputFmt,
    safe: &mut Safe,
) -> Result<()> {
    match cmd {
        WalletSubCommands::Create {
            preload,
            test_coins,
            no_balance,
            keyurl,
            name,
            pay_with,
            secret_key,
        } => {
            // create wallet
            let wallet_xorurl = safe.wallet_create().await?;
            let mut key_generated_output: (String, Option<Keypair>, String) = Default::default();

            if !no_balance {
                // get or create keypair
                let sk = match keyurl {
                    Some(linked_key) => {
                        let sk_hex =
                            get_secret_key(&linked_key, secret_key, "the SafeKey to insert")?;
                        let sk = SecretKey::Ed25519(ed_sk_from_hex(&sk_hex)?);

                        let _ = safe.validate_sk_for_url(&sk, &linked_key).await?;

                        sk_hex
                    }
                    None => match secret_key {
                        Some(sk) => sk,
                        None => {
                            key_generated_output =
                                create_new_key(safe, test_coins, pay_with, preload, None).await?;
                            let unwrapped_key_pair = key_generated_output
                                .1
                                .clone()
                                .ok_or_else(|| anyhow!("Failed to read the generated key pair"))?;
                            let sk = unwrapped_key_pair.secret_key()?;

                            sk_to_hex(sk)
                        }
                    },
                };

                // insert and set as default
                safe.wallet_insert(&wallet_xorurl, name.as_deref(), true, &sk)
                    .await?;
            }

            if OutputFmt::Pretty == output_fmt {
                println!("Wallet created at: \"{}\"", wallet_xorurl);
                if !key_generated_output.0.is_empty() {
                    print_new_key_output(
                        output_fmt,
                        key_generated_output.0,
                        key_generated_output.1,
                        key_generated_output.2,
                        test_coins,
                    );
                }
            } else if let Some(pair) = &key_generated_output.1 {
                println!(
                    "{}",
                    serialise_output(&(&wallet_xorurl, &key_generated_output.0, pair), output_fmt)
                );
            } else {
                println!(
                    "{}",
                    serialise_output(&(&wallet_xorurl, &key_generated_output.0), output_fmt)
                );
            }

            Ok(())
        }
        WalletSubCommands::Balance { target } => {
            let target = get_from_arg_or_stdin(
                target,
                Some("...awaiting Wallet address/location from STDIN stream..."),
            )?;

            debug!("Got target location {:?}", target);
            let balance = safe.wallet_balance(&target).await?;

            if OutputFmt::Pretty == output_fmt {
                let xorurl_encoder = Safe::parse_url(&target)?;
                if xorurl_encoder.path().is_empty() {
                    println!(
                        "Wallet at \"{}\" has a total balance of {} safecoins",
                        target, balance
                    );
                } else {
                    println!(
                        "Wallet's spendable balance at \"{}\" has a balance of {} safecoins",
                        target, balance
                    );
                }
            } else {
                println!("{}", balance);
            }

            Ok(())
        }
        WalletSubCommands::Insert {
            target,
            keyurl,
            name,
            default,
            secret_key,
            pay_with,
        } => {
            if pay_with.is_some() {
                println!("The '--pay-with' argument is being ignored for now as it's not supported yet for this command.");
            }

            let sk = match keyurl {
                Some(linked_key) => {
                    let sk_hex = get_secret_key(&linked_key, secret_key, "the SafeKey to insert")?;
                    let sk = SecretKey::Ed25519(ed_sk_from_hex(&sk_hex)?);

                    let _ = safe.validate_sk_for_url(&sk, &linked_key).await?;

                    sk_hex
                }
                None => get_secret_key("", secret_key, "the SafeKey to insert")?,
            };

            let the_name = safe
                .wallet_insert(&target, name.as_deref(), default, &sk)
                .await?;

            if OutputFmt::Pretty == output_fmt {
                println!(
                    "Spendable balance inserted with name '{}' in Wallet located at \"{}\"",
                    the_name, target
                );
            } else {
                println!("{}", target);
            }

            Ok(())
        }
        WalletSubCommands::Transfer { amount, from, to } => {
            let destination = get_from_arg_or_stdin(
                to,
                Some("...awaiting destination Wallet/SafeKey URL, or public key, from STDIN stream..."),
            )?;

            let tx_id = safe.wallet_transfer(&amount, &from, &destination).await?;

            if OutputFmt::Pretty == output_fmt {
                println!("Success. TX_ID: {}", tx_id);
            } else {
                println!("{}", tx_id)
            }

            Ok(())
        }
    }
}
