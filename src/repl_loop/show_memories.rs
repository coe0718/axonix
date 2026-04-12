//! Handler for the ShowRecentMemories command in the REPL input loop.

use axonix::render::*;

/// Display recent semantic memories from the DB.
///
/// Prints the 5 most recently accessed hot memories.
pub(super) fn handle_show_recent_memories() {
    let sep = "─".repeat(55);
    match axonix::db::AxonixDb::open_default() {
        Err(e) => {
            println!("{RED}  ✗ DB error: {e}{RESET}\n");
        }
        Ok(db) => match db.hot_memories_list(5) {
            Err(e) => {
                println!("{RED}  ✗ memory error: {e}{RESET}\n");
            }
            Ok(rows) if rows.is_empty() => {
                println!("  No semantic memories stored yet.\n");
            }
            Ok(rows) => {
                println!("  Recent semantic memories ({})", rows.len());
                println!("  {sep}");
                for row in &rows {
                    let date = &row.created_at[..10.min(row.created_at.len())];
                    let preview: String = row.content.chars().take(200).collect();
                    let topics = if row.topics.is_empty() || row.topics == "[]" {
                        String::new()
                    } else {
                        format!("  topics: {}", row.topics)
                    };
                    println!("  [{}] importance: {:.2}", date, row.importance);
                    println!("    {preview}");
                    if !topics.is_empty() {
                        println!("{topics}");
                    }
                    println!("  {sep}");
                }
                println!(
                    "  ({} entr{})\n",
                    rows.len(),
                    if rows.len() == 1 { "y" } else { "ies" }
                );
            }
        },
    }
}
