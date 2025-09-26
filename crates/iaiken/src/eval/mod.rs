use aiken_repl::evaluator::{ReplError, ReplEvaluator};
use miette::{GraphicalReportHandler, GraphicalTheme};
use std::sync::{Mutex, OnceLock};

static EVALUATOR: OnceLock<Mutex<ReplEvaluator>> = OnceLock::new();

pub async fn execute_aiken_code(code: &str) -> Result<String, String> {
    println!("execute_aiken_code with code: {code}");
    let code = code.to_string();

    // Eval code making sure I'm propagating all errors
    let task_result = tokio::task::spawn_blocking(move || {
        let evaluator = EVALUATOR.get_or_init(|| Mutex::new(ReplEvaluator::new()));

        let mut eval = match evaluator.lock() {
            Ok(eval) => eval,
            Err(_) => return Err("Error: Failed to acquire evaluator lock".to_string()),
        };

        eval.eval(&code)
            .map(|r| format!("{}", r))
            .map_err(|e| format_evaluation_error_in_task(e))
    })
    .await;

    task_result.map_err(|e| format!("Error: Task panicked: {}", e))?
}

pub async fn evaluate_user_expressions(
    expressions: &std::collections::HashMap<String, String>,
) -> std::collections::HashMap<String, serde_json::Value> {
    println!(
        "evaluate_user_expressions with expressions: {:?}",
        expressions
    );
    let expressions = expressions.clone();
    let mut results = std::collections::HashMap::new();

    let task_result = tokio::task::spawn_blocking(move || {
        let evaluator = EVALUATOR.get_or_init(|| Mutex::new(ReplEvaluator::new()));

        let mut eval = match evaluator.lock() {
            Ok(eval) => eval,
            Err(_) => return results,
        };

        for (name, expr) in expressions {
            match eval.eval(&expr) {
                Ok(result) => {
                    let display_result = format!("{}", result);
                    let mut mime_bundle = serde_json::Map::new();
                    mime_bundle.insert(
                        "text/plain".to_string(),
                        serde_json::Value::String(display_result),
                    );
                    results.insert(name, serde_json::Value::Object(mime_bundle));
                }
                Err(_) => {
                    // On error, return an error message as text/plain
                    let mut mime_bundle = serde_json::Map::new();
                    mime_bundle.insert(
                        "text/plain".to_string(),
                        serde_json::Value::String("Error evaluating expression".to_string()),
                    );
                    results.insert(name, serde_json::Value::Object(mime_bundle));
                }
            }
        }

        results
    })
    .await;

    task_result.unwrap_or_default()
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
