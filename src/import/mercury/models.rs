use crate::util::date::Date;
use serde::{Deserialize, Serialize};

// -------------
// -- SENDING --
// -------------

#[derive(Debug, Serialize)]
pub struct AccountParams {}

#[derive(Debug, Serialize)]
pub struct AccountTransactionsParams {
	pub start: String,
	pub end: String,
}

// ---------------
// -- RECEIVING --
// ---------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsHolder {
	pub accounts: Vec<Account>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
	pub id: String,
	pub status: String, // "active" only for what we care about

	#[serde(rename = "type")]
	pub typ: String, // "mercury" only for what we care about

	nickname: Option<String>,
}

impl Account {
	pub fn name(&self) -> Option<String> {
		self.nickname.clone()
	}
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransactionHolder {
	pub total: i64,
	pub transactions: Vec<Transaction>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
	#[serde(deserialize_with = "deserialize_number_as_string")]
	pub amount: String,

	pub counterparty_name: String,
	pub counterparty_nickname: Option<String>,

	pub posted_at: Option<String>,

	pub status: String,
	// TODO: Implement convertedFromCurrency & convertedToCurrency
	//  (on currencyExchangeInfo sub-object)
}

impl Transaction {
	pub fn name(&self) -> String {
		if let Some(nickname) = &self.counterparty_nickname {
			nickname.clone()
		} else {
			self.counterparty_name.clone()
		}
	}

	/// Extracts first 10 characters, which is the ISO-8601 date.
	/// Will panic if posted_at is None.
	pub fn date(&self) -> Date {
		Date::from_str(&self.posted_at.clone().unwrap()[..10]).unwrap()
	}
}

// Custom deserialization function
fn deserialize_number_as_string<'de, D>(
	deserializer: D,
) -> Result<String, D::Error>
where
	D: serde::Deserializer<'de>,
{
	let value = serde_json::Value::deserialize(deserializer)?;
	match value {
		serde_json::Value::Number(num) => Ok(num.to_string()),
		_ => Err(serde::de::Error::custom("expected a number")),
	}
}
