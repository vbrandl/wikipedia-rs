use crate::{r#async::AsyncHttpClient, Error, Result, LANGUAGE_URL_MARKER};

#[derive(Debug)]
pub struct Wikipedia<A: AsyncHttpClient> {
    /// HttpClient struct.
    pub client: A,
    /// Url is created by concatenating `pre_language_url` + `language` + `post_language_url`.
    pub pre_language_url: String,
    pub post_language_url: String,
    pub language: String,
    /// Number of results to fetch when searching.
    pub search_results: u32,
    /// Number of images to fetch in each request when calling `get_images`.
    /// The iterator will go through all of them, fetching pages of this size.
    /// It can be the string "max" to fetch as many as possible on every request.
    pub images_results: String,
    /// Like `images_results`, for links and references.
    pub links_results: String,
    /// Like `images_results`, for categories.
    pub categories_results: String,
}

impl<A: AsyncHttpClient + Default> Default for Wikipedia<A> {
    fn default() -> Self {
        Wikipedia::new(A::default())
    }
}

impl<'a, A: AsyncHttpClient + 'a> Wikipedia<A> {
    /// Creates a new object using the provided client and default values.
    pub fn new(client: A) -> Self {
        Wikipedia {
            client,
            pre_language_url: "https://".to_owned(),
            post_language_url: ".wikipedia.org/w/api.php".to_owned(),
            language: "en".to_owned(),
            search_results: 10,
            images_results: "max".to_owned(),
            links_results: "max".to_owned(),
            categories_results: "max".to_owned(),
        }
    }

    /// Returns a list of languages in the form of (`identifier`, `language`),
    /// for example [("en", "English"), ("es", "Español")]
    pub async fn get_languages(&'a self) -> Result<Vec<(String, String)>> {
        let q = self
            .query(|| {
                vec![
                    ("meta", "siteinfo"),
                    ("siprop", "languages"),
                    ("format", "json"),
                    ("action", "query"),
                ]
            })
            .await?;
        Ok(q.as_object()
            .and_then(|x| x.get("query"))
            .and_then(|x| x.as_object())
            .and_then(|x| x.get("languages"))
            .and_then(|x| x.as_array())
            .ok_or(Error::JSONPathError)?
            .iter()
            .filter_map(|x| {
                let o = x.as_object();
                Some((
                    match o
                        .and_then(|x| x.get("code"))
                        .and_then(|x| x.as_str())
                        .map(|x| x.to_owned())
                    {
                        Some(v) => v,
                        None => return None,
                    },
                    match o
                        .and_then(|x| x.get("*"))
                        .and_then(|x| x.as_str())
                        .map(|x| x.to_owned())
                    {
                        Some(v) => v,
                        None => return None,
                    },
                ))
            })
            .collect())
    }

    /// Returns the api url
    pub fn base_url(&self) -> String {
        format!(
            "{}{}{}",
            self.pre_language_url, self.language, self.post_language_url
        )
    }

    /// Updates the url format. The substring `{language}` will be replaced
    /// with the selected language.
    pub fn set_base_url(&mut self, base_url: &str) {
        let index = match base_url.find(LANGUAGE_URL_MARKER) {
            Some(i) => i,
            None => {
                self.pre_language_url = base_url.to_owned();
                self.language = "".to_owned();
                self.post_language_url = "".to_owned();
                return;
            }
        };
        self.pre_language_url = base_url[0..index].to_owned();
        self.post_language_url = base_url[index + LANGUAGE_URL_MARKER.len()..].to_owned();
    }

    async fn query<F, I, S>(&'a self, args: F) -> Result<serde_json::Value>
    where
        F: Fn() -> I,
        I: IntoIterator<Item = (&'a str, S)> + Send,
        S: AsRef<str> + 'a,
    {
        let result = self.client.get(&self.base_url(), args()).await?;
        Ok(serde_json::from_str(&result)?)
    }

    /// Searches for a string and returns a list of relevant page titles.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate wikipedia;
    ///
    /// let wiki = wikipedia::Wikipedia::<wikipedia::http::default::Client>::deflt();
    /// let results = wiki.search("keyboard").unwrap();
    /// assert!(results.contains(&"Computer keyboard".to_owned()));
    /// ```
    pub async fn search(&'a self, query: &'a str) -> Result<Vec<String>> {
        let data = self
            .query(move || {
                vec![
                    ("list", "search".to_string()),
                    ("srprop", "".to_string()),
                    ("srlimit", format!("{}", self.search_results)),
                    ("srsearch", query.to_string()),
                    ("format", "json".to_string()),
                    ("action", "query".to_string()),
                ]
            })
            .await?;
        Self::results(data, "search")
    }

    fn results(data: serde_json::Value, query_field: &str) -> Result<Vec<String>> {
        Ok(data
            .as_object()
            .and_then(|x| x.get("query"))
            .and_then(|x| x.as_object())
            .and_then(|x| x.get(query_field))
            .and_then(|x| x.as_array())
            .ok_or(Error::JSONPathError)?
            .iter()
            .filter_map(|i| {
                i.as_object()
                    .and_then(|i| i.get("title"))
                    .and_then(|s| s.as_str().map(|s| s.to_owned()))
            })
            .collect())
    }

    /// Search articles within `radius` meters of `latitude` and `longitude`.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate wikipedia;
    ///
    /// let wiki = wikipedia::Wikipedia::<wikipedia::http::default::Client>::default();
    /// let results = wiki.geosearch(40.750556,-73.993611, 20).unwrap();
    /// assert!(results.contains(&"Madison Square Garden".to_owned()));
    /// ```
    pub async fn geosearch(
        &'a self,
        latitude: f64,
        longitude: f64,
        radius: u16,
    ) -> Result<Vec<String>> {
        if !(-90.0..=90.0).contains(&latitude) {
            return Err(Error::InvalidParameter("latitude".to_string()));
        }
        if !(-180.0..=180.0).contains(&longitude) {
            return Err(Error::InvalidParameter("longitude".to_string()));
        }
        if !(10..=10000).contains(&radius) {
            return Err(Error::InvalidParameter("radius".to_string()));
        }
        let data = self
            .query(move || {
                let results = format!("{}", self.search_results);
                vec![
                    ("list", "geosearch".to_string()),
                    ("gsradius", format!("{radius}")),
                    ("gscoord", format!("{latitude}|{longitude}")),
                    ("gslimit", results),
                    ("format", "json".to_string()),
                    ("action", "query".to_string()),
                ]
            })
            .await?;
        Self::results(data, "geosearch")
    }

    /// Fetches `count` random articles' title.
    pub async fn random_count(&'a self, count: u8) -> Result<Vec<String>> {
        let data = self
            .query(move || {
                vec![
                    ("list", "random".to_string()),
                    ("rnnamespace", "0".to_string()),
                    ("rnlimit", format!("{count}")),
                    ("format", "json".to_string()),
                    ("action", "query".to_string()),
                ]
            })
            .await?;
        Self::results(data, "random")
    }

    /// Fetches a random article's title.
    pub async fn random(&'a self) -> Result<Option<String>> {
        Ok(self.random_count(1).await?.into_iter().next())
    }
}
