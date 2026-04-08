#[derive(Debug, Clone, Default)]
pub struct SearchParams {
    pub query: Option<String>,
    pub scope: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub match_all_tags: bool,
    pub limit: Option<usize>,
}
