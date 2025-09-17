use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    middleware::utils::db_utils::{ViewFieldSelector, ViewRelateField},
    models::view::{discussion::DiscussionView, post::LatestPostView},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscussionUserView {
    pub discussion: DiscussionView,
    pub latest_post: Option<LatestPostView>,
    pub nr_unread: u32,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

impl ViewFieldSelector for DiscussionUserView {
    fn get_select_query_fields() -> String {
        let disc_view_fields = DiscussionView::get_fields();
        let post_view_fields = LatestPostView::get_fields();
        format!("*, in.{{{disc_view_fields}}} as discussion,  latest_post.{{{post_view_fields}}} as latest_post")
    }
}
