use crate::{http::default::Client, Error, Result};

use async_trait::async_trait;
use reqwest::{
    header::{self, HeaderValue},
    Client as RClient,
};

pub mod wikipedia;

#[async_trait]
pub trait AsyncHttpClient {
    /// Run an http request with the given url and args, returning
    /// the result as a string.
    async fn get<'a, I, S>(&self, base_url: &str, args: I) -> Result<String>
    where
        I: IntoIterator<Item = (&'a str, S)> + Send,
        S: AsRef<str> + 'a;
}

#[async_trait]
impl AsyncHttpClient for Client {
    async fn get<'a, I, S>(&self, base_url: &str, args: I) -> Result<String>
    where
        I: IntoIterator<Item = (&'a str, S)> + Send,
        S: AsRef<str> + 'a,
    {
        let user_agent = HeaderValue::from_str(&self.user_agent);
        let url = reqwest::Url::parse_with_params(base_url, args).map_err(|_| Error::URLError)?;
        let request = RClient::new().get(url);
        let request = if let Ok(user_agent) = user_agent {
            request.header(header::USER_AGENT, user_agent)
        } else {
            request
        };
        let response = request.send().await?;
        Ok(response.text().await?)
    }
}
