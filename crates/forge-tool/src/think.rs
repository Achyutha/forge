use std::collections::HashMap;

use crate::{
    model::{CallToolRequest, CallToolResponse, ToolResponseContent, ToolsListResponse},
    Tool,
};
use anyhow::Result;
use colorize::AnsiColor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThoughtData {
    thought: String,
    thought_number: i32,
    total_thoughts: i32,
    next_thought_needed: bool,
    is_revision: Option<bool>,
    revises_thought: Option<i32>,
    branch_from_thought: Option<i32>,
    branch_id: Option<String>,
    needs_more_thoughts: Option<bool>,
}

#[derive(Clone, Default)]
pub struct Think {
    thought_history: Vec<ThoughtData>,
    branches: HashMap<String, Vec<ThoughtData>>,
}

impl Think {
    fn validate_thought_data(&self, input: serde_json::Value) -> Result<ThoughtData> {
        let thought_data: ThoughtData = serde_json::from_value(input)?;

        if thought_data.thought_number <= 0 {
            return Err(anyhow::anyhow!("Invalid thoughtNumber: must be positive"));
        }
        if thought_data.total_thoughts <= 0 {
            return Err(anyhow::anyhow!("Invalid totalThoughts: must be positive"));
        }

        Ok(thought_data)
    }

    fn format_thought(&self, thought_data: &ThoughtData) -> String {
        let (prefix, context) = match (thought_data.is_revision, &thought_data.branch_from_thought)
        {
            (Some(true), _) => (
                "🔄 Revision".yellow().to_string(),
                format!(
                    " (revising thought {})",
                    thought_data.revises_thought.unwrap_or(0)
                ),
            ),
            (_, Some(branch)) => (
                "🌿 Branch".green().to_string(),
                format!(
                    " (from thought {}, ID: {})",
                    branch,
                    thought_data.branch_id.as_ref().unwrap_or(&String::new())
                ),
            ),
            _ => ("💭 Thought".blue().to_string(), String::new()),
        };

        let header = format!(
            "{} {}/{}{}",
            prefix, thought_data.thought_number, thought_data.total_thoughts, context
        );
        let border_len = header.len().max(thought_data.thought.len()) + 4;
        let border = "─".repeat(border_len);

        format!(
            "\n┌{}┐\n│ {} │\n├{}┤\n│ {} │\n└{}┘",
            border,
            header,
            border,
            format!("{:width$}", thought_data.thought, width = border_len - 2),
            border
        )
    }

    fn process_thought(&mut self, input: serde_json::Value) -> Result<serde_json::Value> {
        let mut thought_data = self.validate_thought_data(input)?;

        if thought_data.thought_number > thought_data.total_thoughts {
            thought_data.total_thoughts = thought_data.thought_number;
        }

        self.thought_history.push(thought_data.clone());

        if let (Some(branch_from), Some(branch_id)) =
            (thought_data.branch_from_thought, &thought_data.branch_id)
        {
            self.branches
                .entry(branch_id.clone())
                .or_default()
                .push(thought_data.clone());
        }

        eprintln!("{}", self.format_thought(&thought_data));

        let result = serde_json::json!({
            "thoughtNumber": thought_data.thought_number,
            "totalThoughts": thought_data.total_thoughts,
            "nextThoughtNeeded": thought_data.next_thought_needed,
            "branches": self.branches.keys().collect::<Vec<_>>(),
            "thoughtHistoryLength": self.thought_history.len()
        });

        Ok(result)
    }
}

fn call_tool(req: CallToolRequest, thinking_server: &mut Think) -> Result<CallToolResponse> {
    let name = req.name.as_str();
    let args = req.arguments.unwrap_or_default();
    let args = serde_json::to_value(args)?;
    let result = match name {
        "sequentialthinking" => {
            let thought_result = thinking_server.process_thought(args)?;
            ToolResponseContent::Text {
                text: serde_json::to_string(&thought_result)?,
            }
        }
        _ => return Err(anyhow::anyhow!("Unknown tool: {}", req.name)),
    };
    Ok(CallToolResponse {
        content: vec![result],
        is_error: None,
        meta: None,
    })
}

#[async_trait::async_trait]
impl Tool for Think {
    fn name(&self) -> &'static str {
        "think"
    }

    async fn tools_call(&self, input: CallToolRequest) -> Result<CallToolResponse, String> {
        call_tool(input, &mut self.clone()).map_err(|e| e.to_string())
    }

    fn tools_list(&self) -> ToolsListResponse {
        let response = include_str!("./think.schema.json");
        serde_json::from_str(response).unwrap()
    }
}
