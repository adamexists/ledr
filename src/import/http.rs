use anyhow::bail;
use reqwest::Method;
use serde::{Deserialize, Serialize};

pub struct Client {
	client: reqwest::blocking::Client,
	base_url: String,
	api_key: String,
}

impl Client {
	pub fn new(base_url: &str, api_key: String) -> Self {
		Client {
			client: reqwest::blocking::Client::new(),
			base_url: base_url.to_string(),
			api_key,
		}
	}

	/// Sends a GET and handle the response. Errors on non-2xx response codes.
	pub fn get<Q, R>(
		&self,
		endpoint: &str,
		query_params: Option<Q>,
	) -> Result<R, anyhow::Error>
	where
		Q: Serialize,
		R: for<'de> Deserialize<'de>,
	{
		let url = format!("{}/{}", self.base_url, endpoint);

		let mut request = self
			.client
			.request(Method::GET, &url)
			.header("Authorization", format!("Bearer {}", self.api_key));

		if let Some(query_params) = query_params {
			request = request.query(&query_params);
		}

		println!("Sending GET to {}", url);
		let response = request.send()?;

		// Handle non-2xx response codes
		if !response.status().is_success() {
			bail!("Request failed with status: {}", response.status());
		}

		let response_data: R = response.json()?;
		Ok(response_data)
	}
}
