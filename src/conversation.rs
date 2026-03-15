use std::io::{self, Write};
use yoagent::*;

pub fn save_conversation(messages: &[AgentMessage], path: &str) -> io::Result<usize> {
    use std::fs::File;
    let mut file = File::create(path)?;
    let mut count = 0;
    for msg in messages {
        if let Some(llm_msg) = msg.as_llm() {
            let (role, contents) = match llm_msg {
                Message::User { content, .. } => ("User", content),
                Message::Assistant { content, .. } => ("Assistant", content),
                Message::ToolResult {
                    tool_name, content, ..
                } => {
                    writeln!(file, "## Tool Result: {tool_name}\n")?;
                    for c in content {
                        if let Content::Text { text } = c {
                            writeln!(file, "{text}\n")?;
                        }
                    }
                    writeln!(file, "---\n")?;
                    count += 1;
                    continue;
                }
            };
            writeln!(file, "## {role}\n")?;
            for c in contents {
                match c {
                    Content::Text { text } => writeln!(file, "{text}\n")?,
                    Content::ToolCall { name, arguments, .. } => {
                        writeln!(file, "**Tool call:** `{name}`\n```json\n{arguments}\n```\n")?
                    }
                    _ => {}
                }
            }
            writeln!(file, "---\n")?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_conversation_empty() {
        let messages: Vec<AgentMessage> = vec![];
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.md");
        let count = save_conversation(&messages, path.to_str().unwrap()).unwrap();
        assert_eq!(count, 0);
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.is_empty());
    }
}
