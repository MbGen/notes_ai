use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WebResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub domain: String,
}

pub fn public_url(raw: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw).map_err(|_| "Invalid source address".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Only HTTP and HTTPS source links are supported".into());
    }
    Ok(url)
}

pub fn search_url(query: &str) -> Result<reqwest::Url, String> {
    let mut url = public_url("https://duckduckgo.com/")?;
    url.query_pairs_mut().append_pair("q", query);
    Ok(url)
}

pub fn parse_results(body: &str) -> Result<Vec<WebResult>, String> {
    let document = Html::parse_document(body);
    let cards = Selector::parse(".result").unwrap();
    let links = Selector::parse("a.result__a").unwrap();
    let snippets = Selector::parse(".result__snippet").unwrap();
    let text = |element: scraper::ElementRef<'_>| {
        element
            .text()
            .collect::<Vec<_>>()
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let base = public_url("https://duckduckgo.com/")?;
    let mut results = Vec::new();
    for card in document.select(&cards) {
        // Sponsored results are not included in the personal research view.
        if card.value().classes().any(|c| c == "result--ad") {
            continue;
        }
        let Some(link) = card.select(&links).next() else {
            continue;
        };
        let Some(href) = link.value().attr("href") else {
            continue;
        };
        let Ok(mut url) = base.join(href) else {
            continue;
        };
        if matches!(
            url.host_str(),
            Some("duckduckgo.com") | Some("www.duckduckgo.com")
        ) && url.path() == "/l/"
        {
            let Some(destination) = url
                .query_pairs()
                .find(|(key, _)| key == "uddg")
                .map(|(_, value)| value.into_owned())
            else {
                continue;
            };
            let Ok(destination) = public_url(&destination) else {
                continue;
            };
            url = destination;
        }
        let Ok(url) = public_url(url.as_str()) else {
            continue;
        };
        let title = text(link);
        if title.is_empty() || results.iter().any(|r: &WebResult| r.url == url.as_str()) {
            continue;
        }
        results.push(WebResult {
            title,
            url: url.to_string(),
            domain: url
                .host_str()
                .unwrap_or("")
                .trim_start_matches("www.")
                .to_string(),
            snippet: card.select(&snippets).next().map(text).unwrap_or_default(),
        });
        if results.len() == 10 {
            break;
        }
    }
    if results.is_empty()
        && (body.contains("anomaly.js")
            || body.contains("anomaly-modal")
            || body.contains("challenge-form"))
    {
        return Err("DuckDuckGo needs a browser check. Open this search in your browser, then try again later.".into());
    }
    // A changed page format must not look like a successful empty search.
    if results.is_empty() && !body.contains("no-results") && !body.contains("No results found") {
        return Err(
            "The search provider could not return results. Try again or open the browser search."
                .into(),
        );
    }
    Ok(results)
}

pub async fn search(query: &str) -> Result<Vec<WebResult>, String> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > 600 {
        return Err("Enter a query between 1 and 600 characters.".into());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("Mozilla/5.0 (compatible; NotesAI/0.1; personal web search)")
        .build()
        .map_err(|_| "Cannot start web search".to_string())?;
    let mut response = client
        .get("https://html.duckduckgo.com/html/")
        .query(&[("q", query)])
        .send()
        .await
        .map_err(|_| "Cannot reach DuckDuckGo. Check your connection and try again.".to_string())?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(format!(
            "Search provider returned {}. Try the browser search.",
            response.status()
        ));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "The connection interrupted. Try again.".to_string())?
    {
        if body.len() + chunk.len() > 2_000_000 {
            return Err("The search response is too large. Try the browser search.".into());
        }
        body.extend_from_slice(&chunk);
    }
    let body = String::from_utf8(body).map_err(|_| "Cannot read search results".to_string())?;
    parse_results(&body)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "Requires internet access to DuckDuckGo; not part of offline unit tests"]
    async fn live_duckduckgo_search() {
        let results = search("Rust programming language official").await.unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().all(|r| public_url(&r.url).is_ok()));
        println!(
            "Live search returned {} sources; first: {}",
            results.len(),
            results[0].title
        );
    }
    #[test]
    fn redirects_text_and_unsafe_links() {
        let html = r#"<div class="result"><a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.org%2Fguide%3Fa%3D1%26b%3D2">A &amp; B</a><a class="result__snippet">Useful <b>guide</b></a></div><div class="result"><a class="result__a" href="javascript:alert(1)">unsafe</a></div>"#;
        let results = parse_results(html).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "A & B");
        assert_eq!(results[0].url, "https://example.org/guide?a=1&b=2");
        assert_eq!(results[0].snippet, "Useful guide");
    }
    #[test]
    fn explicit_empty_is_distinct_from_a_block_or_changed_markup() {
        assert!(
            parse_results("<div class='no-results'>No results found</div>")
                .unwrap()
                .is_empty()
        );
        assert!(parse_results("<form id='challenge-form'></form>").is_err());
        assert!(parse_results("<html>unexpected content</html>").is_err());
    }
    #[test]
    fn browser_query_is_encoded_and_source_protocols_are_limited() {
        let url = search_url("a & b # Привіт").unwrap();
        assert_eq!(
            url.query_pairs().find(|(k, _)| k == "q").unwrap().1,
            "a & b # Привіт"
        );
        assert!(public_url("file:///C:/Windows").is_err());
        assert!(public_url("https://user:secret@example.org").is_err());
    }
}
