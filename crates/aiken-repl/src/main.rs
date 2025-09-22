use aiken_repl::evaluator::{EvaluationResult, ReplError, ReplEvaluator};
use rustyline::{DefaultEditor, error::ReadlineError};

fn main() {
    println!("🎯 Aiken REPL");
    println!(
        "Evaluate Aiken expressions or definitions. Use :quit to exit and :help to view all commands"
    );
    println!();

    let mut repl = ReplEvaluator::new();
    //let mut line_number = 1;
    let mut rl = DefaultEditor::new().expect("Failed to create readline editor");

    // Load history if it exists
    let _ = rl.load_history(".aiken_repl_history");

    loop {
        // Create prompt
        //let prompt = format!("[{}]> ", line_number);
        let prompt = "λ> ";

        // Read input with readline
        let input = match rl.readline(&prompt) {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                println!("Goodbye! 👋");
                break;
            }
            Err(err) => {
                eprintln!("Error reading input: {}", err);
                continue;
            }
        };

        let input = input.trim();

        // Handle special commands
        match input {
            ":quit" | ":q" => {
                println!("Goodbye! 👋");
                break;
            }
            ":reset" => {
                repl.reset();
                println!("🗑️ Context reset");
                //line_number = 1;
                continue;
            }
            ":help" | ":h" => {
                print_help();
                continue;
            }
            ":context" | ":ctx" => {
                println!("{}", repl.context_info());
                continue;
            }
            "" => continue, // Empty line
            _ => {}
        }

        // Add to history if not empty and not a command
        if !input.is_empty() && !input.starts_with(':') {
            rl.add_history_entry(input).ok();
        }

        // Evaluate the input
        match repl.eval(input) {
            Ok(result) => {
                match result {
                    EvaluationResult::Value { .. } | EvaluationResult::Definition { .. } => {
                        println!("{}", result);
                    }
                    EvaluationResult::NoResult => {
                        println!("✓ Ok");
                    }
                }
                //line_number += 1;
            }
            Err(err) => {
                eprintln!("❌ Error: {}", err);
                // Check if it's a diagnostic error and print it nicely
                if let ReplError::ProjectError(project_err) = &err {
                    eprintln!("{:?}", project_err);
                }
            }
        }
    }

    // Save history before exiting
    let _ = rl.save_history(".aiken_repl_history");
}

fn print_help() {
    println!("🛟 Aiken REPL Help");
    println!();
    println!("Special commands:");
    println!("  :help, :h       - Show this help");
    println!("  :quit, :q       - Exit the REPL");
    println!("  :reset          - Clear all definitions and restart");
    println!("  :context, :ctx  - Show current context info");
    println!();
    println!("Examples:");
    println!("  True                          // Boolean literal");
    println!("  1 + 2                         // Arithmetic");
    println!("  pub const my_const = 42       // Define constant");
    println!("  pub fn add(x, y) {{ x + y }}    // Define function");
    println!("  add(2, 3)                     // Call function");
    println!();
}
