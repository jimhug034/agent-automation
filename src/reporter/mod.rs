pub mod feishu;
pub mod html;

pub use feishu::send_feishu_notification;
pub use html::generate_html_report;
