use crate::error::AppError;
use crate::models::{StepStatus, TestReport, TestStep};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// 生成 HTML 测试报告
///
/// # 参数
/// * `report` - 测试报告数据
/// * `template_path` - HTML 模板文件路径
/// * `output_path` - 输出 HTML 文件路径
///
/// # 返回
/// 成功时返回 Ok(())，失败时返回 AppError
pub fn generate_html_report(
    report: &TestReport,
    template_path: &Path,
    output_path: &Path,
) -> Result<(), AppError> {
    // 读取模板文件
    let template_content = fs::read_to_string(template_path)
        .map_err(|e| AppError::ReportGenerationFailed(format!("读取模板失败: {}", e)))?;

    // 构建模板变量
    let mut context = HashMap::new();

    // 基本信息
    context.insert("task_id".to_string(), report.task_id.clone());
    context.insert("package_url".to_string(), report.package_url.clone());
    context.insert(
        "start_time".to_string(),
        report
            .start_time
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string(),
    );
    context.insert(
        "end_time".to_string(),
        report.end_time.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
    );
    context.insert(
        "duration_secs".to_string(),
        report.duration_secs.to_string(),
    );

    // 摘要信息
    context.insert("total".to_string(), report.summary.total.to_string());
    context.insert("passed".to_string(), report.summary.passed.to_string());
    context.insert("failed".to_string(), report.summary.failed.to_string());
    context.insert("skipped".to_string(), report.summary.skipped.to_string());
    context.insert(
        "pass_rate".to_string(),
        format!("{:.1}", report.summary.pass_rate * 100.0),
    );
    context.insert(
        "status".to_string(),
        if report.summary.failed > 0 {
            "FAILED".to_string()
        } else {
            "PASSED".to_string()
        },
    );
    context.insert(
        "status_class".to_string(),
        if report.summary.failed > 0 {
            "failed".to_string()
        } else {
            "passed".to_string()
        },
    );

    // 生成步骤列表 HTML
    let steps_html = generate_steps_html(&report.steps);
    context.insert("steps_html".to_string(), steps_html);

    // 替换模板变量
    let mut result = template_content;
    for (key, value) in &context {
        result = result.replace(&format!("{{{{{}}}}}", key), value);
    }

    // 写入输出文件
    fs::write(output_path, result)
        .map_err(|e| AppError::ReportGenerationFailed(format!("写入报告失败: {}", e)))?;

    Ok(())
}

/// 生成测试步骤的 HTML 片段
fn generate_steps_html(steps: &[TestStep]) -> String {
    let mut html = String::from("<div class=\"steps-container\">");

    for (index, step) in steps.iter().enumerate() {
        let status_class = match step.status {
            StepStatus::Passed => "passed",
            StepStatus::Failed => "failed",
            StepStatus::Skipped => "skipped",
            StepStatus::Running => "running",
            StepStatus::Pending => "pending",
        };

        let status_icon = match step.status {
            StepStatus::Passed => "&#10004;", // Checkmark
            StepStatus::Failed => "&#10008;", // X mark
            StepStatus::Skipped => "#8628;",  // Right arrow
            StepStatus::Running => "&#9881;", // Gear
            StepStatus::Pending => "&#9744;", // Checkbox
        };

        let hardware_badge = if step.is_hardware_related {
            "<span class=\"badge hardware\">Hardware</span>"
        } else {
            ""
        };

        let error_section = if let Some(error) = &step.error {
            format!(
                "<div class=\"error-message\"><strong>Error:</strong> {}</div>",
                escape_html(error)
            )
        } else {
            String::new()
        };

        let action_display = format_action_display(&step.action);

        html.push_str(&format!(
            r#"<div class="step {status_class}">
                <div class="step-header">
                    <span class="step-icon">{status_icon}</span>
                    <span class="step-number">{}</span>
                    <span class="step-description">{}</span>
                    {hardware_badge}
                    <span class="step-action">{}</span>
                </div>
                {error_section}
            </div>"#,
            index + 1,
            escape_html(&step.description),
            escape_html(&action_display)
        ));
    }

    html.push_str("</div>");
    html
}

/// 格式化测试动作用于显示
fn format_action_display(action: &crate::models::TestAction) -> String {
    match action {
        crate::models::TestAction::Click { ref_id } => {
            format!("Click: {}", ref_id)
        }
        crate::models::TestAction::Input { ref_id, text } => {
            format!("Input: {} = \"{}\"", ref_id, text)
        }
        crate::models::TestAction::Wait { duration_ms } => {
            format!("Wait: {}ms", duration_ms)
        }
        crate::models::TestAction::Navigate { url } => {
            format!("Navigate: {}", url)
        }
        crate::models::TestAction::Assert { condition } => {
            format!("Assert: {}", condition)
        }
        crate::models::TestAction::Skip { reason } => {
            format!("Skip: {}", reason)
        }
    }
}

/// HTML 转义
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
