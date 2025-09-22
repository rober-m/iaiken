use aiken_repl::{ReplError, ReplEvaluator};
use miette::{GraphicalReportHandler, GraphicalTheme};
use std::sync::{Mutex, OnceLock};

static EVALUATOR: OnceLock<Mutex<ReplEvaluator>> = OnceLock::new();

pub async fn execute_aiken_code(code: &str) -> String {
    let code = code.to_string();

    tokio::task::spawn_blocking(move || {
        let evaluator = EVALUATOR.get_or_init(|| Mutex::new(ReplEvaluator::new()));

        let mut eval = match evaluator.lock() {
            Ok(eval) => eval,
            Err(_) => return "Error: Failed to acquire evaluator lock".to_string(),
        };

        match eval.eval(&code) {
            Ok(result) => format!("{}", result),
            Err(error) => format_evaluation_error_in_task(error),
        }
    })
    .await
    .unwrap_or_else(|e| format!("Error: Task panicked: {}", e))
}

fn format_evaluation_error_in_task(error: ReplError) -> String {
    // Create a graphical report handler with colors enabled
    let handler = GraphicalReportHandler::new().with_theme(GraphicalTheme::default());

    // Format the error using miette's rich diagnostic formatting
    // We need to format the error without creating a Report since ReplError
    // contains non-Send types. We use miette's report formatting directly.
    // TODO: Should I be doing this differently?
    let mut output = String::new();
    match handler.render_report(&mut output, &error) {
        Ok(_) => output,
        Err(_) => {
            // Fallback to simple formatting if rendering fails
            format!("{}", error)
        }
    }
}
