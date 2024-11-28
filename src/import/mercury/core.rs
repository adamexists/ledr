/* Copyright © 2024 Adam House <adam@adamexists.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 */
use crate::config::config_file::Mercury;
use crate::gl::entry::Entry;
use crate::import::http::Client;
use crate::import::importer::PLACEHOLDER;
use crate::import::mercury::models::{
	Account, AccountParams, AccountTransactionsParams, AccountsHolder,
	Transaction, TransactionHolder, ACCOUNT_PREFIX,
};
use crate::util::amount::Amount;
use crate::util::date::Date;
use crate::util::quant::Quant;
use anyhow::{bail, Error};
use std::fs::OpenOptions;
use std::io::Write;

const MERCURY_API_URL: &str = "https://api.mercury.com/api/v1";

/// The importer that knows how to contact the Mercury API to grab
/// transactions. Read-only implementation.
pub struct MercuryImporter {
	http: Client,
}

impl MercuryImporter {
	pub fn new(config: Mercury) -> Result<Self, Error> {
		if config.api_key.is_empty() {
			bail!("no mercury api key in config");
		}

		let api_url = if let Some(url) = config.api_url {
			url
		} else {
			MERCURY_API_URL.to_owned()
		};

		Ok(MercuryImporter {
			http: Client::new(&api_url, config.api_key),
		})
	}

	pub fn run(
		&self,
		begin: Date,
		end: Date,
		file: String,
	) -> Result<(), Error> {
		// make sure we can append to destination file first
		let mut file =
			OpenOptions::new().append(true).create(true).open(file)?;

		let mut entries: Vec<Entry> = Vec::new();

		// get accounts
		// TODO: Implement filtering or specifying these.
		let resp: AccountsHolder =
			self.http.get("accounts", None::<AccountParams>)?;
		for account in &resp.accounts {
			if account.typ != "mercury" || account.status != "active" {
				continue;
			}

			// get transactions within range
			let resp: TransactionHolder = self.http.get(
				format!("account/{}/transactions", account.id).as_str(),
				Some(AccountTransactionsParams {
					start: begin.to_string(),
					end: end.to_string(),
				}),
			)?;

			if resp.total >= 500 {
				// TODO: Handle pagination; for now, bail
				// too many transactions to return at once
				bail!("Too many transactions in range; please shorten range and try again");
			}

			entries.extend(
				resp.transactions
					.into_iter()
					.filter_map(|t| {
						parse_transaction(account, t).unwrap_or(None)
					})
					.collect::<Vec<_>>(),
			);
		}

		entries.sort();
		for e in entries {
			writeln!(file, "{}", e)?;
		}

		Ok(())
	}
}

fn parse_transaction(
	a: &Account,
	t: Transaction,
) -> Result<Option<Entry>, Error> {
	if t.posted_at.is_none() || t.status == "cancelled" || t.status == "failed"
	{
		return Ok(None);
	}

	let entry_name = if t.kind == "internalTransfer" {
		"Internal Transfer".to_string()
	} else {
		t.name(false)
	};

	let mut entry = Entry::new(t.date(), entry_name, 0);

	let amount = Quant::from_str(&t.amount)?;
	let account_name = a.name();

	let counterparty = if t.kind == "internalTransfer" {
		format!("{}:{}", ACCOUNT_PREFIX, t.name(true))
	} else {
		PLACEHOLDER.to_string()
	};

	entry.add_detail(&counterparty, Amount::new(-amount, "USD"))?;
	entry.add_detail(&account_name, Amount::new(amount, "USD"))?;

	Ok(Some(entry))
}
