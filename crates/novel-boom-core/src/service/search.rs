//! Search use-cases.

use crate::config::Config;
use crate::error::{Error, Result};
use crate::model::SearchHit;
use crate::net::HttpClient;
use crate::parse::search_source;
use crate::rule::RuleCatalog;

/// Search within one book source by id.
pub async fn single_search(
    client: &HttpClient,
    catalog: &RuleCatalog,
    config: &Config,
    source_id: u32,
    keyword: &str,
) -> Result<Vec<SearchHit>> {
    let rule = catalog
        .get(source_id)
        .ok_or(Error::SourceNotFound(source_id))?;

    if rule.need_proxy && !config.proxy.enabled {
        return Err(Error::Message(format!(
            "书源「{}」标记为需要代理，请在 config.toml 启用 [proxy]\n\
             示例：\n\
             [proxy]\n\
             enabled = true\n\
             host = \"127.0.0.1\"\n\
             port = 7890",
            rule.name
        )));
    }

    search_source(client, rule, keyword, config.source.search_limit)
        .await
        .map_err(|err| annotate_search_error(err, &rule.name))
}

fn annotate_search_error(err: Error, source_name: &str) -> Error {
    match err {
        Error::Http(msg) => Error::Http(format!("书源「{source_name}」搜索失败\n{msg}")),
        other => other,
    }
}
