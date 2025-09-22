//! REPL Evaluator for Aiken
//!
//! This crate provides a REPL-like evaluator for Aiken code that leverages
//! the existing Aiken Project infrastructure for compilation, type checking, and
//! error reporting. It maintains state between evaluations and supports both
//! expressions and function definitions.

use std::{
    collections::HashSet,
    fmt, fs,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use aiken_lang::{
    ast::{Definition, TraceLevel, Tracing},
    plutus_version::PlutusVersion,
    tipo::pretty::Printer,
};
use aiken_project::{
    Project,
    config::ProjectConfig,
    error::Error as ProjectError,
    module::CheckedModule,
    telemetry::{CoverageMode, EventListener},
};
use miette::Diagnostic;
use uplc::{
    ast::{Constant, NamedDeBruijn, Program, Term},
    machine::{cost_model::ExBudget, eval_result::EvalResult},
};

/// Errors that can occur during REPL evaluation
#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum ReplError {
    #[error("REPL evaluation failed")]
    #[diagnostic(transparent)]
    ProjectError(#[from] ProjectError),

    #[error("Failed to create temporary file: {0}")]
    TempFileError(#[from] std::io::Error),

    #[error("Evaluation produced no result")]
    NoResult,

    #[error("Expression evaluation failed: {message}")]
    EvaluationFailed { message: String },
}

/// The result of evaluating Aiken code in the REPL
#[derive(Debug, Clone)]
pub enum EvaluationResult {
    /// A value was computed and can be displayed
    Value {
        value: String,
        tipo: Rc<aiken_lang::tipo::Type>,
        uplc_result: Option<Constant>,
    },
    /// A definition was added (function, type, etc.)
    Definition {
        name: String,
        kind: DefinitionKind,
        tipo: Option<Rc<aiken_lang::tipo::Type>>,
    },
    /// No result (e.g., import statement)
    NoResult,
}

#[derive(Debug, Clone)]
pub enum DefinitionKind {
    Function,
    Type,
    Constant,
}

/// Helper struct that tracks definition names to avoid conflicts
#[derive(Debug, Default)]
pub struct DefinitionNames {
    pub functions: HashSet<String>,
    pub constants: HashSet<String>,
    pub types: HashSet<String>,
}

/// This is how we'll show the evaluation result in the repl
impl fmt::Display for EvaluationResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            // When printing a value, show both the value and the type
            EvaluationResult::Value { value, tipo, .. } => {
                let mut printer = Printer::new();
                let type_str = printer.pretty_print(tipo, 0);
                write!(f, "{} : {}", value, type_str)
            }
            // Provide some feedback when creating a definition
            EvaluationResult::Definition { name, kind, tipo } => {
                let kind_str = match kind {
                    DefinitionKind::Function => "function",
                    DefinitionKind::Type => "type",
                    DefinitionKind::Constant => "constant",
                };
                if let Some(t) = tipo {
                    let mut printer = Printer::new();
                    let type_str = printer.pretty_print(t, 0);
                    write!(f, "Defined {} {} : {}", kind_str, name, type_str)
                } else {
                    write!(f, "Defined {} {}", kind_str, name)
                }
            }
            EvaluationResult::NoResult => write!(f, ""),
        }
    }
}

struct NoEvent;
impl EventListener for NoEvent {}

/// REPL evaluator that maintains state using Aiken's Project infrastructure
pub struct ReplEvaluator {
    /// Temporary directory for REPL files
    temp_dir: tempfile::TempDir,
    /// Current accumulated definitions
    pub(crate) definitions: String,
    /// Counter for generating unique evaluation function names
    eval_counter: AtomicU64,
    /// Plutus version for evaluation
    plutus_version: PlutusVersion,
}

impl Default for ReplEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplEvaluator {
    /// Create a new REPL evaluator
    pub fn new() -> Self {
        Self::with_plutus_version(PlutusVersion::V3)
    }

    /// Create a new evaluator with a specific Plutus version
    pub fn with_plutus_version(plutus_version: PlutusVersion) -> Self {
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temporary directory");

        Self {
            temp_dir,
            definitions: String::new(),
            eval_counter: AtomicU64::new(0),
            plutus_version,
        }
    }

    /// Reset the evaluator context
    pub fn reset(&mut self) {
        self.definitions.clear();
        self.eval_counter.store(0, Ordering::Relaxed);
    }

    /// Get information about current context
    pub fn context_info(&self) -> String {
        if self.definitions.is_empty() {
            "Empty context".to_string()
        } else {
            format!("{}", self.definitions)
        }
    }

    /// Evaluate a piece of Aiken code
    pub fn eval(&mut self, code: &str) -> Result<EvaluationResult, ReplError> {
        // Determine if this is an expression or a module with definitions
        let is_expression = looks_like_expression(code);

        if is_expression {
            self.eval_expression(code)
        } else {
            self.eval_definitions(code)
        }
    }

    /// Evaluate expressions by wrapping them in a function
    fn eval_expression(&mut self, code: &str) -> Result<EvaluationResult, ReplError> {
        // Create unique evaluation function name
        let eval_count = self.eval_counter.fetch_add(1, Ordering::Relaxed);
        let eval_fn_name = format!("repl_eval_{}", eval_count);

        // Wrap the expression in a function for evaluation
        let wrapped_code = format!("pub fn {}() {{ {} }}", eval_fn_name, code);

        // Create complete module with accumulated definitions
        let module_code = format!("{}\n\n{}", self.definitions, wrapped_code);

        // Create a well-typed temporary project
        let mut project = self.create_temp_project(&module_code)?;

        // Find the REPL module
        let repl_module = project
            .modules()
            .into_iter()
            .find(|m| m.name == "repl")
            .ok_or_else(|| ReplError::EvaluationFailed {
                message: "Could not find repl module".to_string(),
            })?;

        // Find the evaluation function
        let eval_fn = repl_module
            .ast
            .definitions()
            .find_map(|def| match def {
                Definition::Fn(f) if f.name == eval_fn_name => Some(f.clone()),
                _ => None,
            })
            .ok_or_else(|| ReplError::EvaluationFailed {
                message: format!(
                    "Could not find evaluation function {}. This should never happen.",
                    eval_fn_name
                ),
            })?;

        // Generate UPLC and evaluate
        let eval_result = self.generate_and_eval(&mut project, repl_module, &eval_fn)?;

        // Extract and format the result
        match eval_result.result {
            Ok(term) => {
                let value_str = term_to_string(&term);
                Ok(EvaluationResult::Value {
                    value: value_str,
                    tipo: eval_fn.return_type,
                    uplc_result: self.extract_constant(&term),
                })
            }
            Err(err) => Err(ReplError::EvaluationFailed {
                message: format!("Evaluation failed: {:?}", err),
            }),
        }
    }

    /// Evaluate code as module definitions
    fn eval_definitions(&mut self, code: &str) -> Result<EvaluationResult, ReplError> {
        // Get all definition names from the new code
        let new_names = self.collect_definition_names(code);

        // Remove any existing definitions with the same names (allow re-defining)
        self.remove_existing_definitions(&new_names);

        let new_definitions = format!("{}\n\n{}", self.definitions, code);

        // Type check project with the new definitions
        let _project = self.create_temp_project(&new_definitions)?;

        // Add the definitions to our accumulated state
        self.definitions = new_definitions;

        // Extract what was actually defined for better feedback
        let defined_items: Vec<_> = [
            new_names
                .functions
                .iter()
                .map(|n| (n.clone(), DefinitionKind::Function))
                .collect::<Vec<_>>(),
            new_names
                .constants
                .iter()
                .map(|n| (n.clone(), DefinitionKind::Constant))
                .collect::<Vec<_>>(),
            new_names
                .types
                .iter()
                .map(|n| (n.clone(), DefinitionKind::Type))
                .collect::<Vec<_>>(),
        ]
        .concat();

        match defined_items.len() {
            0 => Ok(EvaluationResult::NoResult),
            1 => {
                let (name, kind) = defined_items.into_iter().next().unwrap();
                Ok(EvaluationResult::Definition {
                    name,
                    kind,
                    tipo: None,
                })
            }
            _ => {
                let names: Vec<_> = defined_items.iter().map(|(name, _)| name.clone()).collect();
                Ok(EvaluationResult::Definition {
                    name: format!("Multiple definitions: {}", names.join(", ")),
                    kind: DefinitionKind::Function, // Use as generic?
                    tipo: None,
                })
            }
        }
    }

    /// Create a well-typed temporary project for compilation and evaluation
    fn create_temp_project(&self, module_code: &str) -> Result<Project<NoEvent>, ReplError> {
        // Create temporary aiken.toml
        let aiken_toml = r#"
                            name = "repl/temp"
                            version = "0.0.0"
                            plutus = "v3"
                            "#;

        let aiken_toml_path = self.temp_dir.path().join("aiken.toml");
        fs::write(&aiken_toml_path, aiken_toml)?;

        // Create lib directory
        let lib_dir = self.temp_dir.path().join("lib");
        fs::create_dir_all(&lib_dir)?;

        // Write module to lib/repl.ak
        let module_path = lib_dir.join("repl.ak");
        fs::write(&module_path, module_code)?;

        // Load project config
        let config = ProjectConfig::load(self.temp_dir.path())?;

        // Create and check project
        let mut project = Project::new_with_config(
            config,
            self.temp_dir.path().to_path_buf(),
            NoEvent, // Use `Terminal::default()` to print compiler feedback (eg. "resolving dependencies")
        );

        // Type-check the whole project
        if let Err(errors) = project.check(
            true,  // skip_tests
            None,  // match_tests
            false, // verbose
            false, // exact_match
            0,     // seed
            100,   // property_max_success
            CoverageMode::default(),
            Tracing::All(TraceLevel::Compact),
            None,  // env
            false, // plain_numbers
        ) {
            // Convert the first error to our error type
            if let Some(first_error) = errors.into_iter().next() {
                return Err(ReplError::ProjectError(first_error));
            }
        }

        Ok(project)
    }

    /// Generate and evaluate UPLC
    fn generate_and_eval(
        &self,
        project: &mut Project<NoEvent>,
        repl_module: CheckedModule,
        eval_fn: &aiken_lang::ast::TypedFunction,
    ) -> Result<EvalResult, ReplError> {
        // Init a new code generator
        let mut generator = project.new_generator(Tracing::All(TraceLevel::Compact));

        // Generate UPLC for the function
        let program = generator.generate_raw(&eval_fn.body, &[], &repl_module.name);

        // Convert to NamedDeBruijn
        let named_program = Program::<NamedDeBruijn>::try_from(program).map_err(|err| {
            ReplError::EvaluationFailed {
                message: format!("Failed to convert to NamedDeBruijn: {:?}", err),
            }
        })?;

        // Evaluate Program
        let result = named_program.eval_version(ExBudget::max(), &self.plutus_version.into());

        Ok(result)
    }

    /// Collect new definition names
    fn collect_definition_names(&self, code: &str) -> DefinitionNames {
        let mut names = DefinitionNames::default();

        for line in code.lines() {
            let line = line.trim();

            // Extract function names
            if let Some(func_name) = extract_function_name(line) {
                names.functions.insert(func_name);
            }

            // Extract constant names
            if let Some(const_name) = extract_constant_name(line) {
                names.constants.insert(const_name);
            }

            // Extract type names
            if let Some(type_name) = extract_type_name(line) {
                names.types.insert(type_name);
            }
        }

        names
    }

    /// Remove existing definitions that would conflict with new ones (support interactive re-definition)
    /// TODO: For now I manipulate the text, but could I modify the AST directly instead?
    fn remove_existing_definitions(&mut self, new_names: &DefinitionNames) {
        let lines: Vec<String> = self.definitions.lines().map(|s| s.to_string()).collect();
        let mut filtered_lines = Vec::new();

        let mut i = 0;
        while i < lines.len() {
            let line = &lines[i];
            let trimmed = line.trim();

            // Check if this line starts a definition that we want to replace
            let should_remove = if let Some(func_name) = extract_function_name(trimmed) {
                new_names.functions.contains(&func_name)
            } else if let Some(const_name) = extract_constant_name(trimmed) {
                new_names.constants.contains(&const_name)
            } else if let Some(type_name) = extract_type_name(trimmed) {
                new_names.types.contains(&type_name)
            } else {
                false
            };

            if should_remove {
                // Skip this definition and any continuation lines
                i += 1;
                // Skip any lines that are part of the same definition (indented or containing braces)
                while i < lines.len() {
                    let next_line = lines[i].trim();
                    // Stop skipping if we hit another top-level definition or empty line
                    if !next_line.is_empty()
                        && !next_line.starts_with(' ')
                        && !next_line.starts_with('\t')
                        && !next_line.starts_with('}')
                        && (next_line.starts_with("pub ")
                            || next_line.starts_with("const ")
                            || next_line.starts_with("fn ")
                            || next_line.starts_with("type ")
                            || next_line.starts_with("use "))
                    {
                        break;
                    }
                    i += 1;
                }
            } else {
                filtered_lines.push(line.clone());
                i += 1;
            }
        }

        self.definitions = filtered_lines.join("\n");
    }

    /// Extract a constant from a term if possible
    fn extract_constant(&self, term: &Term<NamedDeBruijn>) -> Option<Constant> {
        match term {
            Term::Constant(c) => Some(c.as_ref().clone()),
            _ => None,
        }
    }
}

/// Check if the code looks like an expression vs definitions
fn looks_like_expression(code: &str) -> bool {
    let trimmed = code.trim();

    // Common definition keywords
    let def_keywords = [
        "fn ",
        "pub fn",
        "type ",
        "pub type",
        "const ",
        "pub const",
        "use ",
        "import ",
        "test ",
        "validator",
    ];

    // If it starts with a definition keyword, it's not an expression
    for keyword in &def_keywords {
        if trimmed.starts_with(keyword) {
            return false;
        }
    }

    // If it contains newlines and definition keywords, probably definitions
    if trimmed.contains('\n') {
        for keyword in &def_keywords {
            if trimmed.contains(keyword) {
                return false;
            }
        }
    }

    true
}

/// Convert a UPLC term to a display string
/// TODO: Isn't this already implemented in Aiken somewhere?
fn term_to_string(term: &Term<NamedDeBruijn>) -> String {
    match term {
        Term::Constant(c) => match c.as_ref() {
            Constant::Integer(i) => i.to_string(),
            Constant::ByteString(bs) => format!("#{}", hex::encode(bs)),
            Constant::String(s) => format!("\"{}\"", s),
            Constant::Bool(b) => if *b { "True" } else { "False" }.to_string(),
            Constant::Unit => "Void".to_string(),
            Constant::ProtoList(_, items) => {
                let item_strs: Vec<_> = items.iter().map(|item| format!("{:?}", item)).collect();
                format!("[{}]", item_strs.join(", "))
            }
            Constant::ProtoPair(_, _, first, second) => {
                format!("Pair({:?}, {:?})", first, second)
            }
            Constant::Data(d) => format!("{:?}", d),
            _ => format!("{:?}", c),
        },
        _ => format!("{:?}", term),
    }
}

fn extract_function_name(line: &str) -> Option<String> {
    if line.starts_with("pub fn ") {
        line.strip_prefix("pub fn ")
            .and_then(|rest| rest.split('(').next())
            .map(|name| name.trim().to_string())
    } else if line.starts_with("fn ") {
        line.strip_prefix("fn ")
            .and_then(|rest| rest.split('(').next())
            .map(|name| name.trim().to_string())
    } else {
        None
    }
}

fn extract_constant_name(line: &str) -> Option<String> {
    if line.starts_with("pub const ") {
        line.strip_prefix("pub const ")
            .and_then(|rest| rest.split_whitespace().next())
            .map(|name| name.trim().to_string())
    } else if line.starts_with("const ") {
        line.strip_prefix("const ")
            .and_then(|rest| rest.split_whitespace().next())
            .map(|name| name.trim().to_string())
    } else {
        None
    }
}

fn extract_type_name(line: &str) -> Option<String> {
    if line.starts_with("pub type ") {
        line.strip_prefix("pub type ")
            .and_then(|rest| rest.split_whitespace().next())
            .map(|name| name.trim().to_string())
    } else if line.starts_with("type ") {
        line.strip_prefix("type ")
            .and_then(|rest| rest.split_whitespace().next())
            .map(|name| name.trim().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod test {
    use crate::evaluator::{EvaluationResult, ReplEvaluator, looks_like_expression};

    #[test]
    fn test_simple_expression() {
        let mut repl = ReplEvaluator::new();

        // Test simple boolean expression
        let result = repl.eval("True");
        assert!(result.is_ok());

        if let Ok(EvaluationResult::Value { value, .. }) = result {
            assert_eq!(value, "True");
        } else {
            panic!("Expected value result, got: {:?}", result);
        }
    }

    #[test]
    fn test_arithmetic_expression() {
        let mut repl = ReplEvaluator::new();

        // Test simple arithmetic
        let result = repl.eval("1 + 2");
        assert!(result.is_ok());

        if let Ok(EvaluationResult::Value { value, .. }) = result {
            assert_eq!(value, "3");
        } else {
            panic!("Expected value result, got: {:?}", result);
        }
    }

    #[test]
    fn test_expression_detection() {
        // These should be detected as expressions
        assert!(looks_like_expression("1 + 2"));
        assert!(looks_like_expression("True"));
        assert!(looks_like_expression("\"hello\""));

        // These should be detected as definitions
        assert!(!looks_like_expression("fn add(x, y) { x + y }"));
        assert!(!looks_like_expression("pub const X = 42"));
        assert!(!looks_like_expression("type Option<a> { Some(a) | None }"));
    }

    #[test]
    fn test_definition_addition() {
        let mut repl = ReplEvaluator::new();

        // Add a simple constant definition
        let result = repl.eval("pub const my_const = 42");
        assert!(result.is_ok());

        // Should be able to use it in an expression
        let result = repl.eval("my_const + 1");
        assert!(result.is_ok());

        if let Ok(EvaluationResult::Value { value, .. }) = result {
            assert_eq!(value, "43");
        } else {
            panic!("Expected value result, got: {:?}", result);
        }
    }

    #[test]
    fn test_function_definition_and_call() {
        let mut repl = ReplEvaluator::new();

        // Add a function definition
        let result = repl.eval("pub fn add(x: Int, y: Int) -> Int { x + y }");
        assert!(result.is_ok());

        // Should be able to call it
        let result = repl.eval("add(2, 3)");
        assert!(result.is_ok());

        if let Ok(EvaluationResult::Value { value, .. }) = result {
            assert_eq!(value, "5");
        } else {
            panic!("Expected value result, got: {:?}", result);
        }
    }

    #[test]
    fn test_reset() {
        let mut repl = ReplEvaluator::new();

        // Add some definitions
        let _result = repl.eval("pub const my_const = 42");
        assert!(!repl.definitions.is_empty());

        // Reset should clear everything
        repl.reset();
        assert!(repl.definitions.is_empty());

        // Should no longer be able to use the constant
        let result = repl.eval("my_const");
        assert!(result.is_err());
    }

    #[test]
    fn test_redefinition_support() {
        let mut repl = ReplEvaluator::new();

        // Define a constant
        let result = repl.eval("const something = 3");
        assert!(result.is_ok());

        // Use it
        let result = repl.eval("something");
        assert!(result.is_ok());
        if let Ok(EvaluationResult::Value { value, .. }) = result {
            assert_eq!(value, "3");
        }

        // Redefine with different type and value
        let result = repl.eval("const something = \"hello\"");
        assert!(result.is_ok());

        // Use the new value
        let result = repl.eval("something");
        assert!(result.is_ok());
        if let Ok(EvaluationResult::Value { value, .. }) = result {
            assert!(value.contains("68656c6c6f")); // ByteArray hex representation of "hello"
        }
    }

    #[test]
    fn test_function_redefinition() {
        let mut repl = ReplEvaluator::new();

        // Define a function
        let result = repl.eval("pub fn double(x: Int) -> Int { x * 2 }");
        assert!(result.is_ok());

        // Call it
        let result = repl.eval("double(5)");
        assert!(result.is_ok());
        if let Ok(EvaluationResult::Value { value, .. }) = result {
            assert_eq!(value, "10");
        }

        // Redefine the function
        let result = repl.eval("pub fn double(x: Int) -> Int { x * 3 }");
        assert!(result.is_ok());

        // Call with new behavior
        let result = repl.eval("double(5)");
        assert!(result.is_ok());
        if let Ok(EvaluationResult::Value { value, .. }) = result {
            assert_eq!(value, "15");
        }
    }
}
